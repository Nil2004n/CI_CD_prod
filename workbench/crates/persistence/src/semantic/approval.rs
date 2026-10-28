//! Semantic rule bodies for approvals (Data Schema §11 rules 12, 13, 16, 17 and §18 rule 10).
//!
//! * `D11-12` grants are bound to a workflow revision or a run.
//! * `D11-13` status, decision time and consumption are consistent, so a pending or inactive record never
//!   reads as a grant.
//! * `D11-16` a model-transmission grant names the data categories it discloses.
//! * `D11-17` a grant carries bounded destinations that fit its capability.
//! * `D18-10` a model-transmission grant has an inference scope, every other grant has none.
//!
//! Matching a grant to an actual workflow revision, run or request happens at use time (P03). Every function
//! is pure over one [`AnyStore`] and ignores the roots its rule does not concern.

use domain::contract::ApprovalLifetime;
use domain::states::ApprovalState;
use domain::store::AnyStore;
use domain::vocab::Capability;

use super::rules::{duplicates, flag};
use crate::validate::Problems;

/// Rule `D11-12` (store part): `workflow_revision` lifetime grants carry a workflow ID and a revision of at
/// least 1, `run` lifetime grants carry a run ID, and a revision never appears without its workflow ID.
pub(crate) fn d11_12(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-12";
    let AnyStore::Approvals(f) = doc else { return };
    for (i, a) in f.approvals.iter().enumerate() {
        let at = format!("/approvals/{i}");
        if a.workflow_revision.is_some() && a.workflow_id.is_none() {
            flag(out, R, &format!("{at}/workflow_id"));
        }
        if a.workflow_revision == Some(0) {
            flag(out, R, &format!("{at}/workflow_revision"));
        }
        match a.lifetime {
            ApprovalLifetime::WorkflowRevision => {
                if a.workflow_id.is_none() {
                    flag(out, R, &format!("{at}/workflow_id"));
                }
                if a.workflow_revision.is_none() {
                    flag(out, R, &format!("{at}/workflow_revision"));
                }
            }
            ApprovalLifetime::Run => {
                if a.run_id.is_none() {
                    flag(out, R, &format!("{at}/run_id"));
                }
            }
            ApprovalLifetime::Once => {}
        }
    }
}

/// Rule `D11-13` (store part): status, decision time and consumption agree, so only a decided record can be
/// a grant and a pending request never is one.
///
/// `pending` has no decision time and is not consumed; `approved`, `denied`, `revoked` and `expired` have a
/// decision time; `denied` and `cancelled` were never granted and so are not consumed; only a single-use
/// (`once`) grant is ever consumed. Whether a stored `approved` grant is still current (not expired by the
/// clock, not consumed by a concurrent use) is decided at use time (P03).
pub(crate) fn d11_13(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-13";
    let AnyStore::Approvals(f) = doc else { return };
    for (i, a) in f.approvals.iter().enumerate() {
        let at = format!("/approvals/{i}");
        match a.status {
            ApprovalState::Pending => {
                if a.decided_at.is_some() {
                    flag(out, R, &format!("{at}/decided_at"));
                }
            }
            ApprovalState::Approved
            | ApprovalState::Denied
            | ApprovalState::Revoked
            | ApprovalState::Expired => {
                if a.decided_at.is_none() {
                    flag(out, R, &format!("{at}/decided_at"));
                }
            }
            ApprovalState::Cancelled => {}
        }
        let never_granted = matches!(
            a.status,
            ApprovalState::Pending | ApprovalState::Denied | ApprovalState::Cancelled
        );
        if a.consumed && (never_granted || a.lifetime != ApprovalLifetime::Once) {
            flag(out, R, &format!("{at}/consumed"));
        }
    }
}

/// Rule `D11-16` (store part): the type has no raw-secret data category; additionally a model-transmission
/// grant names at least one category, so it is a bounded disclosure approval. That the named categories are
/// redacted before they leave the machine is P03.
pub(crate) fn d11_16(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-16";
    let AnyStore::Approvals(f) = doc else { return };
    for (i, a) in f.approvals.iter().enumerate() {
        if a.capability == Capability::ModelTransmission && a.data_categories.is_empty() {
            flag(out, R, &format!("/approvals/{i}/data_categories"));
        }
    }
}

/// Whether `capability` reaches a network destination. Only `local_render` and `app_storage` stay on the
/// machine.
fn is_networked(capability: Capability) -> bool {
    !matches!(capability, Capability::LocalRender | Capability::AppStorage)
}

/// Rule `D11-17` (store part): a grant for a networked capability lists at least one destination and no
/// destination twice; a local capability lists none. Syntax of each destination (host and port, `https`, no
/// userinfo, query or fragment) is enforced by the type; redirect handling and request-time validation are
/// P03/P16.
pub(crate) fn d11_17(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-17";
    let AnyStore::Approvals(f) = doc else { return };
    for (i, a) in f.approvals.iter().enumerate() {
        let networked = is_networked(a.capability);
        if networked == a.destinations.is_empty() {
            flag(out, R, &format!("/approvals/{i}/destinations"));
        }
        duplicates(out, R, a.destinations.iter().map(|d| d.as_str()), |j| {
            format!("/approvals/{i}/destinations/{j}")
        });
    }
}

/// Rule `D18-10`: a `model_transmission` grant has an `inference_scope` and every other grant has none; an
/// inference scope lists each source ID once. Reusing a grant after a model, provider, privacy or context
/// change is prevented at use time by matching the request to the scope (P03).
pub(crate) fn d18_10(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-10";
    let AnyStore::Approvals(f) = doc else { return };
    for (i, a) in f.approvals.iter().enumerate() {
        let transmission = a.capability == Capability::ModelTransmission;
        if transmission != a.inference_scope.is_some() {
            flag(out, R, &format!("/approvals/{i}/inference_scope"));
        }
        if let Some(scope) = &a.inference_scope {
            duplicates(out, R, scope.source_ids.iter().copied(), |j| {
                format!("/approvals/{i}/inference_scope/source_ids/{j}")
            });
        }
    }
}

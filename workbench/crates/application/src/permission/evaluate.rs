//! The pure permission evaluator (`planning/P03-T01-request.md` §6, rows R1–R8 and G1–G14). Owner: W1, T05.

use std::collections::{BTreeSet, HashSet};

use domain::contract::{ApprovalLifetime, DataCategory};
use domain::ids::{ApprovalId, RepoId};
use domain::states::ApprovalState;
use domain::store::approval::Approval;
use domain::store::values::Digest;
use domain::vocab::Capability;

use super::{
    Evaluation, EvaluationContext, Grant, NeedReason, PermissionRequest, RefusalReason,
    RequestedData, ScopeDimension, fingerprint,
};

/// Evaluates `request` against the recorded `approvals` (rules 12–16). Request-level checks R1–R8 first;
/// then a grant authorises only if every dimension G1–G14 matches. Never returns `Permitted` on a
/// fingerprint match alone.
///
/// Order of results:
///
/// 1. R1–R6 look at the request alone (and the saved revision in `ctx`): the first failing row refuses.
/// 2. R7: a `denied` record with equal capability, fingerprint, workflow binding and run refuses
///    (`PreviouslyDenied`, the first such record in store order).
/// 3. R8: a `pending` record with equal capability, fingerprint, workflow binding, run and lifetime gives
///    `NeedsApproval(AlreadyPending)` (first in store order). R7 and R8 deliberately run before the grant
///    search, as the scope-match table orders them: an identical pending request is never answered by a
///    second prompt, and a denial is not overridden by another record.
/// 4. G1–G14 are evaluated for every approved record of the same capability. If several records match all
///    dimensions, the winner is the `workflow_revision` grant, then `run`, then `once`; then the earliest
///    `requested_at`; then the smallest `approval_id`.
/// 5. If none matches, `Mismatch` names the first failing dimension of the approved same-capability record
///    whose first failure is latest in table order (ties: store order). Without an approved record of the
///    same capability the result is `NoGrant`. `Status` and `Capability` are never reported: a record that
///    fails them is not a candidate.
///
/// Interpretation notes (the table is terse on these points):
///
/// - G4, G5: workflow id and revision are compared as options, so a grant bound to a workflow never matches
///   an unbound request and vice versa. G4 additionally fails for a `workflow_revision` grant whose
///   workflow id is null, whatever the request has (both `null` is allowed only for `once` and `run`).
///   G5 additionally compares the grant revision with the saved revision in `ctx` whenever the request is
///   bound. Rule 14 (a stale grant never matches) is enforced by R4, which refuses a request whose revision
///   differs from the saved one; the G5 comparison is defence in depth that `evaluate` cannot reach after R4.
/// - G6: a `run` grant needs equal run ids; for the other lifetimes a grant without a run id is not tied to
///   a run, and a grant with one must equal the request's.
/// - G12 is the lifetime-specific completeness check: a `workflow_revision` grant needs a workflow id and a
///   revision and the request needs a workflow binding; a `run` grant needs a run id and the request needs a
///   run. G4 and G5 already reject a `workflow_revision` grant without a workflow id or revision, so for that
///   lifetime the G12 check is defence in depth; in practice G12 reports a `run` grant without a run id
///   that G6 alone would accept for a request without a run.
/// - G13: a grant whose `expires_at` equals `now` is expired.
pub fn evaluate(
    request: &PermissionRequest,
    approvals: &[Approval],
    ctx: &EvaluationContext,
) -> Evaluation {
    // The fingerprint is pure, so computing it before R1–R5 changes no outcome; R6 reports its failure.
    let digest = fingerprint(request);
    if let Some(reason) = refusal(request, ctx, digest.is_ok()) {
        return Evaluation::Refused(reason);
    }
    let Ok(digest) = digest else {
        return Evaluation::Refused(RefusalReason::InvalidRequest);
    };

    // R7.
    if let Some(denied) = approvals.iter().find(|a| {
        a.status == ApprovalState::Denied
            && a.capability == request.capability
            && same_operation(a, request, &digest)
    }) {
        return Evaluation::Refused(RefusalReason::PreviouslyDenied(denied.approval_id));
    }
    // R8.
    if let Some(pending) = approvals.iter().find(|a| {
        a.status == ApprovalState::Pending
            && a.capability == request.capability
            && a.lifetime == request.lifetime
            && same_operation(a, request, &digest)
    }) {
        return Evaluation::NeedsApproval(NeedReason::AlreadyPending(pending.approval_id));
    }

    let view = RequestView::new(request, &digest, ctx);
    let mut winner: Option<&Approval> = None;
    let mut closest: Option<(ScopeDimension, ApprovalId)> = None;
    for approval in approvals {
        if approval.status != ApprovalState::Approved || approval.capability != request.capability {
            continue;
        }
        match first_failure(&view, approval) {
            None => {
                if winner.is_none_or(|w| preference(approval) < preference(w)) {
                    winner = Some(approval);
                }
            }
            Some(dimension) => {
                // Strictly later only, so the first record in store order keeps a tie.
                if closest.is_none_or(|(best, _)| dimension > best) {
                    closest = Some((dimension, approval.approval_id));
                }
            }
        }
    }
    if let Some(approval) = winner {
        return Evaluation::Permitted(Grant {
            approval_id: approval.approval_id,
            lifetime: approval.lifetime,
        });
    }
    match closest {
        Some((dimension, approval_id)) => Evaluation::NeedsApproval(NeedReason::Mismatch {
            approval_id,
            dimension,
        }),
        None => Evaluation::NeedsApproval(NeedReason::NoGrant),
    }
}

/// R1–R6: refusals that depend on the request (and the saved revision) only. `arguments_valid` is whether
/// the fingerprint could be computed (R6).
fn refusal(
    request: &PermissionRequest,
    ctx: &EvaluationContext,
    arguments_valid: bool,
) -> Option<RefusalReason> {
    // R1.
    if request.data.contains(&RequestedData::RawSecretValues) {
        return Some(RefusalReason::RawSecretRequested);
    }
    // R2.
    let network = !matches!(
        request.capability,
        Capability::LocalRender | Capability::AppStorage
    );
    if network && request.destinations.is_empty() {
        return Some(RefusalReason::MissingDestination);
    }
    if !network && !request.destinations.is_empty() {
        return Some(RefusalReason::UnexpectedDestination);
    }
    // R3.
    if (request.lifetime == ApprovalLifetime::WorkflowRevision && request.workflow.is_none())
        || (request.lifetime == ApprovalLifetime::Run && request.run_id.is_none())
    {
        return Some(RefusalReason::LifetimeBinding);
    }
    // R4.
    if let Some(binding) = request.workflow {
        match ctx.current_workflow_revision {
            None => return Some(RefusalReason::UnknownWorkflow),
            Some(current) if current != binding.revision => {
                return Some(RefusalReason::StaleWorkflowRevision);
            }
            Some(_) => {}
        }
    }
    // R5.
    let needs_scope = request.capability == Capability::ModelTransmission;
    if needs_scope != request.inference_scope.is_some() {
        return Some(RefusalReason::InferenceScope);
    }
    // R6 (also: a workflow revision below 1 is not a revision).
    if request
        .workflow
        .is_some_and(|binding| binding.revision == 0)
        || !arguments_valid
    {
        return Some(RefusalReason::InvalidRequest);
    }
    None
}

/// Equal workflow binding (id and revision) and run, and equal fingerprint (R7, R8). The capability and,
/// for R8, the lifetime are compared by the callers.
fn same_operation(approval: &Approval, request: &PermissionRequest, digest: &Digest) -> bool {
    approval.operation_fingerprint == *digest
        && approval.workflow_id == request.workflow.map(|w| w.workflow_id)
        && approval.workflow_revision == request.workflow.map(|w| u64::from(w.revision))
        && approval.run_id == request.run_id
}

/// Sort key of the tie-break among fully matching grants: non-consuming wider lifetimes first, then the
/// earliest request, then the identifier.
fn preference(approval: &Approval) -> (u8, domain::time::UtcTimestamp, ApprovalId) {
    let rank = match approval.lifetime {
        ApprovalLifetime::WorkflowRevision => 0,
        ApprovalLifetime::Run => 1,
        ApprovalLifetime::Once => 2,
    };
    (rank, approval.requested_at, approval.approval_id)
}

/// The request in the shapes G1–G14 compare.
struct RequestView<'a> {
    request: &'a PermissionRequest,
    digest: &'a Digest,
    ctx: &'a EvaluationContext,
    repos: BTreeSet<RepoId>,
    destinations: BTreeSet<String>,
    constraints: BTreeSet<&'a str>,
    categories: HashSet<DataCategory>,
}

impl<'a> RequestView<'a> {
    fn new(request: &'a PermissionRequest, digest: &'a Digest, ctx: &'a EvaluationContext) -> Self {
        Self {
            request,
            digest,
            ctx,
            repos: request.repo_ids.iter().copied().collect(),
            destinations: request
                .destinations
                .iter()
                .map(|d| d.to_store().as_str().to_owned())
                .collect(),
            constraints: request
                .resource_constraints
                .iter()
                .map(|c| c.as_str())
                .collect(),
            categories: request
                .data
                .iter()
                .filter_map(|d| match d {
                    RequestedData::Category(category) => Some(*category),
                    RequestedData::RawSecretValues => None,
                })
                .collect(),
        }
    }
}

/// The first dimension, in table order, on which `approval` does not match the request.
fn first_failure(view: &RequestView<'_>, approval: &Approval) -> Option<ScopeDimension> {
    ScopeDimension::ALL
        .into_iter()
        .find(|dimension| !holds(*dimension, view, approval))
}

/// Whether one dimension (G1–G14) matches.
fn holds(dimension: ScopeDimension, view: &RequestView<'_>, approval: &Approval) -> bool {
    let request = view.request;
    let binding = request.workflow;
    match dimension {
        ScopeDimension::Status => approval.status == ApprovalState::Approved,
        ScopeDimension::Capability => approval.capability == request.capability,
        ScopeDimension::Fingerprint => approval.operation_fingerprint == *view.digest,
        ScopeDimension::Workflow => {
            approval.workflow_id == binding.map(|b| b.workflow_id)
                && (approval.workflow_id.is_some()
                    || approval.lifetime != ApprovalLifetime::WorkflowRevision)
        }
        ScopeDimension::WorkflowRevision => {
            let revision = binding.map(|b| b.revision);
            approval.workflow_revision == revision.map(u64::from)
                && (revision.is_none() || view.ctx.current_workflow_revision == revision)
        }
        ScopeDimension::Run => match approval.lifetime {
            ApprovalLifetime::Run => approval.run_id == request.run_id,
            ApprovalLifetime::Once | ApprovalLifetime::WorkflowRevision => approval
                .run_id
                .is_none_or(|run| request.run_id == Some(run)),
        },
        ScopeDimension::Repos => {
            approval.repo_ids.iter().copied().collect::<BTreeSet<_>>() == view.repos
        }
        ScopeDimension::Destinations => {
            approval
                .destinations
                .iter()
                .map(|d| d.as_str().to_owned())
                .collect::<BTreeSet<_>>()
                == view.destinations
        }
        ScopeDimension::ResourceConstraints => {
            approval
                .resource_constraints
                .iter()
                .map(|c| c.as_str())
                .collect::<BTreeSet<_>>()
                == view.constraints
        }
        ScopeDimension::DataCategories => {
            approval
                .data_categories
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                == view.categories
        }
        ScopeDimension::InferenceScope => approval.inference_scope == request.inference_scope,
        ScopeDimension::Lifetime => match approval.lifetime {
            ApprovalLifetime::Once => true,
            ApprovalLifetime::Run => approval.run_id.is_some() && request.run_id.is_some(),
            ApprovalLifetime::WorkflowRevision => {
                approval.workflow_id.is_some()
                    && approval.workflow_revision.is_some()
                    && binding.is_some()
            }
        },
        ScopeDimension::Expiry => approval.expires_at.is_none_or(|end| end > view.ctx.now),
        ScopeDimension::Consumption => {
            !(approval.lifetime == ApprovalLifetime::Once && approval.consumed)
        }
    }
}

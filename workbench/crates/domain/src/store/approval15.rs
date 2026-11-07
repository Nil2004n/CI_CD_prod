//! Approval, ApprovalFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.5.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1b, T05b.
//!
//! The typed layer does not check `workflow_revision >= 1`, the consistency of status, decision time and
//! consumption, that a grant has an `inference_scope` exactly when the capability needs one, or the
//! uniqueness of IDs: the semantic rules (T09, Data §11) cover those.

use serde::{Deserialize, Serialize};

use super::plan::RefKind;
use super::provider::InferenceScope;
use super::values::GitRefName;
use super::values::{Destination, Digest};
use super::{ApprovalsStoreType, SchemaVersion15 as SchemaVersion};
use crate::contract::{ApprovalLifetime, DataCategory};
use crate::ids::{ApprovalId, RepoId, RunId, WorkflowId};
use crate::states::ApprovalState;
use crate::text::{RedactedText, SanitisedText};
use crate::time::UtcTimestamp;
use crate::vocab::Capability;

closed_enum!(
    /// Git operation stage.
    GitApprovalStage, 3, {
        Discover => "discover",
        Clone => "clone",
        Update => "update",
    }
);

/// Persisted Git request context, validated against the approval before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitApprovalContext {
    /// Operation stage.
    pub stage: GitApprovalStage,
    /// Bound repository.
    pub repo_id: RepoId,
    /// Reference kind, absent for discovery.
    pub ref_kind: Option<RefKind>,
    /// Reference value, absent for discovery.
    pub ref_value: Option<GitRefName>,
}

/// One permission request and its decision (`$defs/Approval`).
///
/// `purpose` and `actor` are FR-31 class R and are re-escaped on read; `resource_constraints` are text the
/// application composes (class S).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    /// Approval identifier.
    pub approval_id: ApprovalId,
    /// Decision state.
    pub status: ApprovalState,
    /// The capability this approval covers.
    pub capability: Capability,
    /// Workflow the approval was requested for, `null` for an ad-hoc run.
    pub workflow_id: Option<WorkflowId>,
    /// Workflow revision the approval was requested for, `null` when not bound to one. The schema minimum
    /// of 1 is not enforced here.
    pub workflow_revision: Option<u64>,
    /// Run the approval was requested for, `null` when not bound to a run.
    pub run_id: Option<RunId>,
    /// Repositories the approval covers.
    pub repo_ids: Vec<RepoId>,
    /// Why the operation is requested.
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub purpose: RedactedText,
    /// Digest binding the approval to one exact operation.
    pub operation_fingerprint: Digest,
    /// Network destinations the approval covers.
    pub destinations: Vec<Destination>,
    /// Limits on the approved operation, composed by the application.
    pub resource_constraints: Vec<SanitisedText>,
    /// Categories of data that may be disclosed.
    pub data_categories: Vec<DataCategory>,
    /// How long the grant lasts.
    pub lifetime: ApprovalLifetime,
    /// Time of the request.
    pub requested_at: UtcTimestamp,
    /// Time of the decision, `null` while pending.
    pub decided_at: Option<UtcTimestamp>,
    /// Time the grant ends, `null` if it does not expire by time.
    pub expires_at: Option<UtcTimestamp>,
    /// Who decided (author text).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub actor: RedactedText,
    /// Whether a single-use grant has been used.
    pub consumed: bool,
    /// Git review context; versioned semantic validation must verify its stage and scope.
    pub git_approval_context: Option<GitApprovalContext>,
    /// What may be sent to the model provider, `null` for approvals that involve no inference.
    pub inference_scope: Option<InferenceScope>,
}

/// The `approvals.json` root (`$defs/ApprovalFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalFile {
    /// Always `"1.5"`.
    pub schema_version: SchemaVersion,
    /// Always `"approvals"`.
    pub store_type: ApprovalsStoreType,
    /// Time of the last write.
    pub updated_at: UtcTimestamp,
    /// The approvals.
    pub approvals: Vec<Approval>,
}

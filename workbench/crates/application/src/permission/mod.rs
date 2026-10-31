//! Permission requests, the canonical operation fingerprint and the pure permission evaluator (Tech §11,
//! Architecture §10, Data §11 rules 12–17; frozen interface `planning/P03-T01-request.md` §5.2, scope-match
//! table §6).
//!
//! A [`PermissionRequest`] describes one concrete operation and its context. [`evaluate`] compares it with
//! the recorded approvals and returns [`Evaluation::Permitted`], [`Evaluation::NeedsApproval`] or
//! [`Evaluation::Refused`]. Evaluation is pure: no I/O, no clock (the time comes in the
//! [`EvaluationContext`]), no randomness. A `Permitted` result is not yet permission to act on a `once`
//! grant: only `approvals::ApprovalService::authorise`, which consumes such a grant durably first, yields
//! the proof P04/P16 accept.
//!
//! The [`fingerprint`] binds a record to canonical operation data (rule 15). It is not encryption, not
//! authorisation by itself and not proof of user intent.

mod destination;
mod evaluate;
mod fingerprint;

pub use destination::{DestinationError, MAX_DESTINATION_BYTES, NormalisedDestination, Scheme};
pub use evaluate::evaluate;
pub use fingerprint::{FINGERPRINT_PREFIX, fingerprint};

use domain::contract::{ApprovalLifetime, DataCategory};
use domain::ids::{ApprovalId, RepoId, RunId, WorkflowId};
use domain::store::provider::InferenceScope;
use domain::text::{RedactedText, SanitisedText};
use domain::time::UtcTimestamp;
use domain::vocab::Capability;

/// Longest operation or argument name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;
/// Longest argument value, in bytes. Values are hashed, never persisted.
pub const MAX_ARG_VALUE_BYTES: usize = 65_536;

/// Why an operation name, argument name or argument list was rejected. Fieldless: never echoes input.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArgError {
    /// An empty name.
    Empty,
    /// A name or value over its byte bound.
    TooLong,
    /// A name character outside `[a-z0-9._-]`.
    Character,
    /// Two arguments with the same name.
    Duplicate,
}

fn check_name(s: &str) -> Result<(), ArgError> {
    if s.is_empty() {
        return Err(ArgError::Empty);
    }
    if s.len() > MAX_NAME_BYTES {
        return Err(ArgError::TooLong);
    }
    if !s
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
    {
        return Err(ArgError::Character);
    }
    Ok(())
}

/// The operation's name (for example `git.clone`): `[a-z0-9._-]`, 1–64 bytes.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct OperationName(String);

impl OperationName {
    /// Validates `s`.
    pub fn parse(s: &str) -> Result<Self, ArgError> {
        check_name(s).map(|()| Self(s.to_owned()))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An argument's name: `[a-z0-9._-]`, 1–64 bytes.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ArgName(String);

impl ArgName {
    /// Validates `s`.
    pub fn parse(s: &str) -> Result<Self, ArgError> {
        check_name(s).map(|()| Self(s.to_owned()))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One normalised operation argument. The value is hashed into the fingerprint and never persisted; values
/// over [`MAX_ARG_VALUE_BYTES`] make the fingerprint fail with [`ArgError::TooLong`].
///
/// Callers must never put credential-bearing or other secret values into an argument: the fingerprint is an
/// unsalted SHA-256, so a low-entropy value can be confirmed offline by guessing (P03-D01, L5). `Debug`
/// shows the name and the value's byte length only, never the value (P03-D01, M1).
#[derive(Clone, PartialEq, Eq)]
pub struct OperationArg {
    /// Argument name.
    pub name: ArgName,
    /// Normalised argument value.
    pub value: String,
}

impl std::fmt::Debug for OperationArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationArg")
            .field("name", &self.name)
            .field("value_bytes", &self.value.len())
            .finish()
    }
}

/// The saved workflow and the revision a request belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WorkflowBinding {
    /// The saved workflow.
    pub workflow_id: WorkflowId,
    /// Its revision (≥ 1; `u32` because `ApprovalRequested.workflow_revision` is `Option<u32>`).
    pub revision: u32,
}

/// Data the operation would disclose. `RawSecretValues` exists so that a caller that would send raw
/// secrets is refused explicitly (rule 16); it can never be granted and has no store representation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RequestedData {
    /// A disclosable category.
    Category(DataCategory),
    /// Raw secret values: always refused.
    RawSecretValues,
}

/// One concrete permission request (Architecture §10).
#[derive(Clone, PartialEq, Debug)]
pub struct PermissionRequest {
    /// The capability needed.
    pub capability: Capability,
    /// What is done.
    pub operation: OperationName,
    /// Normalised arguments; names unique.
    pub arguments: Vec<OperationArg>,
    /// The saved workflow and revision, if any.
    pub workflow: Option<WorkflowBinding>,
    /// The run, if any.
    pub run_id: Option<RunId>,
    /// Target repositories.
    pub repo_ids: Vec<RepoId>,
    /// Network destinations (normalised; see [`NormalisedDestination`]).
    pub destinations: Vec<NormalisedDestination>,
    /// Limits on the operation, composed by the application.
    pub resource_constraints: Vec<SanitisedText>,
    /// Data the operation would disclose.
    pub data: Vec<RequestedData>,
    /// Why the operation is requested (already redacted by the caller).
    pub purpose: RedactedText,
    /// Requested grant lifetime.
    pub lifetime: ApprovalLifetime,
    /// Optional time bound of a grant, in seconds after the decision.
    pub validity_seconds: Option<u32>,
    /// What may be sent to the model provider; required exactly for `model_transmission`.
    pub inference_scope: Option<InferenceScope>,
}

/// Inputs of an evaluation besides the request and the approvals.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvaluationContext {
    /// The current time (injected clock).
    pub now: UtcTimestamp,
    /// The saved revision of the request's workflow, `None` if the request has no workflow or the
    /// workflow is not saved.
    pub current_workflow_revision: Option<u32>,
}

/// A matching, valid grant. Constructible only inside `permission`; not proof of authorisation for a
/// `once` grant until it is consumed (`approvals::ApprovalService::authorise`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Grant {
    approval_id: ApprovalId,
    lifetime: ApprovalLifetime,
}

impl Grant {
    /// The approval record that matched.
    pub fn approval_id(&self) -> ApprovalId {
        self.approval_id
    }

    /// Its lifetime; `once` grants must be consumed before use.
    pub fn lifetime(&self) -> ApprovalLifetime {
        self.lifetime
    }
}

/// The result of [`evaluate`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Evaluation {
    /// A valid grant matches every dimension.
    Permitted(Grant),
    /// No valid grant matches; a new explicit decision is needed.
    NeedsApproval(NeedReason),
    /// The request can never be granted as it stands.
    Refused(RefusalReason),
}

/// Why approval is needed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NeedReason {
    /// No approved record of the same capability exists.
    NoGrant,
    /// An identical request is already pending.
    AlreadyPending(ApprovalId),
    /// The closest approved record fails on this dimension (table order, request §6).
    Mismatch {
        /// The closest record.
        approval_id: ApprovalId,
        /// Its first failing dimension.
        dimension: ScopeDimension,
    },
}

/// Why a request is refused outright (request §6, rows R1–R7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RefusalReason {
    /// R1: raw secret values requested (rule 16).
    RawSecretRequested,
    /// R2: a network capability without a destination.
    MissingDestination,
    /// R2: a local capability with destinations.
    UnexpectedDestination,
    /// R3: lifetime without the binding it needs (rule 12).
    LifetimeBinding,
    /// R4: the bound workflow is not saved.
    UnknownWorkflow,
    /// R4: the bound revision is not the saved revision.
    StaleWorkflowRevision,
    /// R5: inference scope missing for `model_transmission` or present for another capability.
    InferenceScope,
    /// R7: the identical request was denied.
    PreviouslyDenied(ApprovalId),
    /// R6: malformed arguments.
    InvalidRequest,
}

/// A scope-match dimension, in reporting order (request §6, G1–G14).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum ScopeDimension {
    /// G1.
    Status,
    /// G2.
    Capability,
    /// G3.
    Fingerprint,
    /// G4.
    Workflow,
    /// G5.
    WorkflowRevision,
    /// G6.
    Run,
    /// G7.
    Repos,
    /// G8.
    Destinations,
    /// G9.
    ResourceConstraints,
    /// G10.
    DataCategories,
    /// G11.
    InferenceScope,
    /// G12.
    Lifetime,
    /// G13.
    Expiry,
    /// G14.
    Consumption,
}

impl ScopeDimension {
    /// Every dimension, in reporting order.
    pub const ALL: [Self; 14] = [
        Self::Status,
        Self::Capability,
        Self::Fingerprint,
        Self::Workflow,
        Self::WorkflowRevision,
        Self::Run,
        Self::Repos,
        Self::Destinations,
        Self::ResourceConstraints,
        Self::DataCategories,
        Self::InferenceScope,
        Self::Lifetime,
        Self::Expiry,
        Self::Consumption,
    ];
}

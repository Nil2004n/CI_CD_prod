//! Event envelope, Clock, IdSource, Emitter and the AppEvent -> store Event mapping (CA-04;
//! `planning/P02-T01-request.md` §5.8).
//!
//! [`Emitter::emit`] is the single place where an [`AppEvent`] becomes an [`EventEnvelope`]: it rejects
//! events that must not exist, then adds the event identifier (UUID v4 from an [`IdSource`]), the emission
//! time (from a [`Clock`]) and the severity level. [`to_store_event`] projects an envelope onto the
//! persisted `log.json` record, or returns `None` for events the store must not hold (T01-D9).
//!
//! Nothing here echoes event payload text into an error: [`EmitError`] and [`IdSourceError`] carry closed
//! enums and `std::io::ErrorKind` only.

use std::fmt;
use std::io::{ErrorKind, Read};

use domain::contract::{AppEvent, ContractStatus, DeliveryClass, FailureCode};
use domain::ids::{ApprovalId, EventId, RepoId, RunId, TaskId, WorkflowId};
use domain::states::RunState;
use domain::store::log::{Event, EventLevel, EventType};
use domain::store::values::EventCode;
use domain::time::UtcTimestamp;
use serde::Serialize;

/// The kernel random device read by [`UrandomIds`].
const URANDOM_PATH: &str = "/dev/urandom";

/// Source of the current time for envelope timestamps.
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// The current instant.
    fn now(&self) -> UtcTimestamp;
}

/// The system clock: [`UtcTimestamp::now`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UtcTimestamp {
        UtcTimestamp::now()
    }
}

/// Why a source of identifier bytes failed. Carries only a closed reason and a `std::io::ErrorKind`: no
/// path, no bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IdSourceError {
    /// The random device could not be opened.
    Open {
        /// Kind of the underlying I/O error.
        kind: ErrorKind,
    },
    /// Reading from the random device failed.
    Read {
        /// Kind of the underlying I/O error.
        kind: ErrorKind,
    },
    /// The random device ended before 16 bytes were read.
    ShortRead,
}

impl fmt::Display for IdSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { kind } => write!(f, "random source could not be opened ({kind})"),
            Self::Read { kind } => write!(f, "random source could not be read ({kind})"),
            Self::ShortRead => f.write_str("random source returned fewer than 16 bytes"),
        }
    }
}

impl std::error::Error for IdSourceError {}

/// Source of the 16 random bytes behind a new identifier.
pub trait IdSource: Send + std::fmt::Debug {
    /// The next 16 random bytes, or the reason none could be produced.
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError>;
}

/// Reads identifier bytes from `/dev/urandom` with `std::fs`. Any failure to open or to read all 16 bytes
/// is an [`IdSourceError`]; there is no retry beyond `read_exact` and no fallback generator of any kind.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrandomIds;

impl IdSource for UrandomIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        let mut device = std::fs::File::open(URANDOM_PATH)
            .map_err(|e| IdSourceError::Open { kind: e.kind() })?;
        let mut bytes = [0u8; 16];
        device.read_exact(&mut bytes).map_err(|e| match e.kind() {
            ErrorKind::UnexpectedEof => IdSourceError::ShortRead,
            kind => IdSourceError::Read { kind },
        })?;
        Ok(bytes)
    }
}

/// Which emission-time rule an event broke. Closed: never contains event text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InvalidEventReason {
    /// The event is reserved for deferred work (`AppEvent::status` is `Reserved`).
    ReservedEvent,
    /// `RunFinished.state` is not a terminal run state.
    RunFinishedNotTerminal,
    /// `CheckCompleted.outcome` is not a terminal check outcome.
    CheckCompletedNotTerminal,
    /// `RunStateChanged` names a transition the run state machine does not allow.
    IllegalRunTransition,
}

impl fmt::Display for InvalidEventReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReservedEvent => "event is reserved for deferred work",
            Self::RunFinishedNotTerminal => "run finished with a non-terminal state",
            Self::CheckCompletedNotTerminal => "check completed with a non-terminal outcome",
            Self::IllegalRunTransition => "run state transition is not allowed",
        })
    }
}

/// Why [`Emitter::emit`] produced no envelope. Never contains event payload text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EmitError {
    /// The event is not allowed to exist; no identifier was consumed.
    InvalidEvent {
        /// Which rule the event broke.
        reason: InvalidEventReason,
    },
    /// The identifier source failed.
    IdSource(IdSourceError),
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEvent { reason } => write!(f, "event rejected: {reason}"),
            Self::IdSource(e) => write!(f, "event identifier unavailable: {e}"),
        }
    }
}

impl std::error::Error for EmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidEvent { .. } => None,
            Self::IdSource(e) => Some(e),
        }
    }
}

impl From<IdSourceError> for EmitError {
    fn from(e: IdSourceError) -> Self {
        Self::IdSource(e)
    }
}

/// An [`AppEvent`] with the fields added at emission (CA-04). Serialises flat (T01-D10): `event_id`, `at`
/// and `level` come first, followed by `event_type` and the event's own fields.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct EventEnvelope {
    /// Unique identifier of this emission (UUID v4).
    pub event_id: EventId,
    /// When the event was emitted.
    pub at: UtcTimestamp,
    /// Severity.
    pub level: EventLevel,
    /// The event itself, flattened beside the envelope fields.
    #[serde(flatten)]
    pub event: AppEvent,
}

/// Severity of an event (CA-04 level table). Exhaustive over [`AppEvent`] and [`RunState`]: a new variant
/// fails to compile until it is classified. `debug` is unused.
fn level_of(ev: &AppEvent) -> EventLevel {
    match ev {
        AppEvent::StorageFailure { .. }
        | AppEvent::OperationFailed { .. }
        | AppEvent::ChatFailed { .. } => EventLevel::Error,
        AppEvent::TriggerSkipped { .. } => EventLevel::Warning,
        AppEvent::RunFinished { state, .. } => match state {
            RunState::Failed | RunState::Interrupted | RunState::Partial => EventLevel::Warning,
            RunState::Queued
            | RunState::AwaitingApproval
            | RunState::Running
            | RunState::Cancelling
            | RunState::Completed
            | RunState::Cancelled => EventLevel::Info,
        },
        AppEvent::RepositoryRegistered { .. }
        | AppEvent::RepositoryRemoved { .. }
        | AppEvent::RepositoryStatusChanged { .. }
        | AppEvent::InventoryUpdated { .. }
        | AppEvent::SelectionChanged { .. }
        | AppEvent::WorkflowSaved { .. }
        | AppEvent::WorkflowDeleted { .. }
        | AppEvent::WorkflowStateChanged { .. }
        | AppEvent::TriggerFired { .. }
        | AppEvent::RunPlanned { .. }
        | AppEvent::RunStarted { .. }
        | AppEvent::RunStateChanged { .. }
        | AppEvent::TaskStarted { .. }
        | AppEvent::CheckCompleted { .. }
        | AppEvent::TaskProgress { .. }
        | AppEvent::FindingObserved { .. }
        | AppEvent::ApprovalRequested { .. }
        | AppEvent::ApprovalResolved { .. }
        | AppEvent::ApprovalRevoked { .. }
        | AppEvent::ReportWritten { .. }
        | AppEvent::ExportStatusChanged { .. }
        | AppEvent::DependencyStatusChanged { .. }
        | AppEvent::CredentialStatusChanged { .. }
        | AppEvent::ChatRetrievalCompleted { .. }
        | AppEvent::ChatResponseStarted { .. }
        | AppEvent::ChatResponseFinished { .. }
        | AppEvent::ChatResponseChunk { .. }
        | AppEvent::SuppressionChanged
        | AppEvent::RunComparisonReady => EventLevel::Info,
    }
}

/// Emission-time rules. Checked before any identifier is consumed.
fn validate(ev: &AppEvent) -> Result<(), InvalidEventReason> {
    if ev.status() == ContractStatus::Reserved {
        return Err(InvalidEventReason::ReservedEvent);
    }
    match ev {
        AppEvent::RunFinished { state, .. } if !state.is_terminal() => {
            Err(InvalidEventReason::RunFinishedNotTerminal)
        }
        AppEvent::CheckCompleted { outcome, .. } if !outcome.is_terminal() => {
            Err(InvalidEventReason::CheckCompletedNotTerminal)
        }
        AppEvent::RunStateChanged { from, to, .. } if !from.can_transition(*to) => {
            Err(InvalidEventReason::IllegalRunTransition)
        }
        _ => Ok(()),
    }
}

/// Wraps events into envelopes: validates, then assigns `event_id`, `at` and `level`.
#[derive(Debug)]
pub struct Emitter<C: Clock, I: IdSource> {
    clock: C,
    ids: I,
}

impl<C: Clock, I: IdSource> Emitter<C, I> {
    /// An emitter over the given clock and identifier source.
    pub fn new(clock: C, ids: I) -> Self {
        Self { clock, ids }
    }

    /// Validates `ev` and wraps it in an envelope.
    ///
    /// Rejected with [`EmitError::InvalidEvent`] (before any identifier is consumed, so the
    /// [`IdSource`] is not advanced): a reserved event; `RunFinished` with a non-terminal `state`;
    /// `CheckCompleted` with a non-terminal `outcome`; `RunStateChanged` where `from` cannot move to `to`.
    /// An identifier source failure gives [`EmitError::IdSource`]. The identifier is a UUID v4 built with
    /// `EventId::from_random_bytes`; `at` is the clock reading taken after validation and identifier
    /// generation.
    pub fn emit(&mut self, ev: AppEvent) -> Result<EventEnvelope, EmitError> {
        validate(&ev).map_err(|reason| EmitError::InvalidEvent { reason })?;
        let event_id = EventId::from_random_bytes(self.ids.next_bytes()?);
        Ok(EventEnvelope {
            event_id,
            at: self.clock.now(),
            level: level_of(&ev),
            event: ev,
        })
    }
}

/// The store `Event` identifier slots and code an event fills.
#[derive(Default)]
struct StoreFields {
    run_id: Option<RunId>,
    repo_id: Option<RepoId>,
    workflow_id: Option<WorkflowId>,
    task_id: Option<TaskId>,
    approval_id: Option<ApprovalId>,
    code: Option<FailureCode>,
}

/// The `Event.code` text of a failure code; equal to its serialised (`snake_case`) form.
fn failure_code_wire(code: FailureCode) -> &'static str {
    match code {
        FailureCode::Deferred => "deferred",
        FailureCode::InvalidInput => "invalid_input",
        FailureCode::NotFound => "not_found",
        FailureCode::Conflict => "conflict",
        FailureCode::PermissionRequired => "permission_required",
        FailureCode::Internal => "internal",
    }
}

/// Identifier slots of `ev`; `None` for events that are never persisted by name (chat events, Data Schema
/// §11.24). Exhaustive over [`AppEvent`]: a new variant fails to compile until it is classified. Events
/// that are not persisted for delivery or status reasons still get their slots here; [`to_store_event`]
/// filters them from `AppEvent::delivery` and `AppEvent::status`.
fn store_fields(ev: &AppEvent) -> Option<StoreFields> {
    let fields = match ev {
        AppEvent::ChatRetrievalCompleted { .. }
        | AppEvent::ChatResponseStarted { .. }
        | AppEvent::ChatResponseFinished { .. }
        | AppEvent::ChatResponseChunk { .. }
        | AppEvent::ChatFailed { .. } => return None,
        AppEvent::RepositoryRegistered { repo_id, .. }
        | AppEvent::RepositoryRemoved { repo_id }
        | AppEvent::RepositoryStatusChanged { repo_id, .. }
        | AppEvent::InventoryUpdated { repo_id, .. } => StoreFields {
            repo_id: Some(*repo_id),
            ..StoreFields::default()
        },
        AppEvent::WorkflowSaved { workflow_id, .. }
        | AppEvent::WorkflowDeleted { workflow_id }
        | AppEvent::WorkflowStateChanged { workflow_id, .. }
        | AppEvent::TriggerFired { workflow_id, .. }
        | AppEvent::TriggerSkipped { workflow_id, .. } => StoreFields {
            workflow_id: Some(*workflow_id),
            ..StoreFields::default()
        },
        AppEvent::RunPlanned {
            run_id,
            workflow_id,
            ..
        } => StoreFields {
            run_id: Some(*run_id),
            workflow_id: *workflow_id,
            ..StoreFields::default()
        },
        AppEvent::RunStarted { run_id, .. }
        | AppEvent::RunStateChanged { run_id, .. }
        | AppEvent::RunFinished { run_id, .. } => StoreFields {
            run_id: Some(*run_id),
            ..StoreFields::default()
        },
        AppEvent::TaskStarted {
            run_id,
            task_id,
            repo_id,
            ..
        } => StoreFields {
            run_id: Some(*run_id),
            task_id: Some(*task_id),
            repo_id: Some(*repo_id),
            ..StoreFields::default()
        },
        AppEvent::CheckCompleted {
            run_id, task_id, ..
        }
        | AppEvent::TaskProgress {
            run_id, task_id, ..
        } => StoreFields {
            run_id: Some(*run_id),
            task_id: Some(*task_id),
            ..StoreFields::default()
        },
        AppEvent::FindingObserved {
            run_id, repo_id, ..
        }
        | AppEvent::ReportWritten {
            run_id, repo_id, ..
        } => StoreFields {
            run_id: Some(*run_id),
            repo_id: Some(*repo_id),
            ..StoreFields::default()
        },
        AppEvent::ApprovalRequested {
            approval_id,
            workflow_id,
            ..
        } => StoreFields {
            approval_id: Some(*approval_id),
            workflow_id: *workflow_id,
            ..StoreFields::default()
        },
        AppEvent::ApprovalResolved { approval_id, .. }
        | AppEvent::ApprovalRevoked { approval_id } => StoreFields {
            approval_id: Some(*approval_id),
            ..StoreFields::default()
        },
        AppEvent::OperationFailed { code, .. } => StoreFields {
            code: Some(*code),
            ..StoreFields::default()
        },
        AppEvent::SelectionChanged { .. }
        | AppEvent::ExportStatusChanged { .. }
        | AppEvent::DependencyStatusChanged { .. }
        | AppEvent::CredentialStatusChanged { .. }
        | AppEvent::StorageFailure { .. }
        | AppEvent::SuppressionChanged
        | AppEvent::RunComparisonReady => StoreFields::default(),
    };
    Some(fields)
}

/// Projects an envelope onto the persisted `log.json` event record (T01-D9, CA-04).
///
/// Returns `None` (the event is not persisted) exactly when the event is
/// * a chat event (`ChatRetrievalCompleted`, `ChatResponseStarted`, `ChatResponseFinished`,
///   `ChatResponseChunk`, `ChatFailed`; Data Schema §11.24), or
/// * not `DeliveryClass::MustDeliver` (only must-deliver events are persisted; today `SelectionChanged`
///   and `TaskProgress`), or
/// * reserved (`ContractStatus::Reserved`; today `SuppressionChanged` and `RunComparisonReady`).
///
/// Otherwise `event_id`, `at`, `level` and `event_type` are carried over, and:
/// * `run_id`, `repo_id`, `workflow_id`, `task_id` and `approval_id` are filled where the event has a
///   single such identifier. List-valued identifiers (`RunPlanned.required_approval_ids`,
///   `ApprovalRequested.repo_ids`) have no single store slot and leave the field `None`; identifiers with
///   no store slot (`report_id`, `check_id`, `finding_id`, ...) are not carried.
/// * `message_redacted` is `AppEvent::store_message` (the event's own redacted detail or a fixed
///   application-authored summary; no payload text is interpolated here).
/// * `code` is the failure code of `OperationFailed` (its serialised form) and `None` otherwise.
pub fn to_store_event(env: &EventEnvelope) -> Option<Event> {
    let ev = &env.event;
    if ev.status() == ContractStatus::Reserved || ev.delivery() != DeliveryClass::MustDeliver {
        return None;
    }
    let fields = store_fields(ev)?;
    let event_type = EventType::from_wire(ev.event_type())?;
    let code = match fields.code {
        Some(c) => Some(EventCode::parse(failure_code_wire(c)).ok()?),
        None => None,
    };
    Some(Event {
        event_id: env.event_id,
        at: env.at,
        level: env.level,
        event_type,
        run_id: fields.run_id,
        repo_id: fields.repo_id,
        workflow_id: fields.workflow_id,
        task_id: fields.task_id,
        approval_id: fields.approval_id,
        message_redacted: ev.store_message(),
        code,
    })
}

//! Event, EventLevel, EventType, RunSummary, ReportManifest, LogFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//! `log.json` is the activity log: run summaries, persisted events and report manifests (Data Schema §10).

use serde::{Deserialize, Serialize};

use super::plan::PlanSnapshot;
use super::report::CoverageSummary;
use super::values::{AppRelPath, Digest, EventCode, VersionLabel};
use super::{LogStoreType, SchemaVersion};
use crate::ids::{ApprovalId, EventId, RepoId, ReportId, RunId, TaskId, WorkflowId};
use crate::states::RunState;
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

closed_enum!(
    /// Severity of a persisted event (`Event.level`).
    EventLevel, 4, {
        /// Diagnostic detail.
        Debug => "debug",
        /// Normal operation.
        Info => "info",
        /// Something degraded but the operation continued.
        Warning => "warning",
        /// An operation failed.
        Error => "error",
    }
);

closed_enum!(
    /// The Architecture §6 event name of a persisted event (`Event.event_type`).
    ///
    /// One value per name returned by `contract::AppEvent::event_type`, serialised verbatim (PascalCase).
    /// The two lists are kept equal by a test (`store_models_b.rs`). Whether a given name may be persisted,
    /// for example the chat events, is a semantic rule (Data Schema §11, T09), not a type rule.
    EventType, 34, {
        /// The `RepositoryRegistered` event (Architecture §6).
        RepositoryRegistered => "RepositoryRegistered",
        /// The `RepositoryRemoved` event (Architecture §6).
        RepositoryRemoved => "RepositoryRemoved",
        /// The `RepositoryStatusChanged` event (Architecture §6).
        RepositoryStatusChanged => "RepositoryStatusChanged",
        /// The `InventoryUpdated` event (Architecture §6).
        InventoryUpdated => "InventoryUpdated",
        /// The `SelectionChanged` event (Architecture §6).
        SelectionChanged => "SelectionChanged",
        /// The `WorkflowSaved` event (Architecture §6).
        WorkflowSaved => "WorkflowSaved",
        /// The `WorkflowDeleted` event (Architecture §6).
        WorkflowDeleted => "WorkflowDeleted",
        /// The `WorkflowStateChanged` event (Architecture §6).
        WorkflowStateChanged => "WorkflowStateChanged",
        /// The `TriggerFired` event (Architecture §6).
        TriggerFired => "TriggerFired",
        /// The `TriggerSkipped` event (Architecture §6).
        TriggerSkipped => "TriggerSkipped",
        /// The `RunPlanned` event (Architecture §6).
        RunPlanned => "RunPlanned",
        /// The `RunStarted` event (Architecture §6).
        RunStarted => "RunStarted",
        /// The `RunStateChanged` event (Architecture §6).
        RunStateChanged => "RunStateChanged",
        /// The `TaskStarted` event (Architecture §6).
        TaskStarted => "TaskStarted",
        /// The `CheckCompleted` event (Architecture §6).
        CheckCompleted => "CheckCompleted",
        /// The `TaskProgress` event (Architecture §6).
        TaskProgress => "TaskProgress",
        /// The `FindingObserved` event (Architecture §6).
        FindingObserved => "FindingObserved",
        /// The `RunFinished` event (Architecture §6).
        RunFinished => "RunFinished",
        /// The `ApprovalRequested` event (Architecture §6).
        ApprovalRequested => "ApprovalRequested",
        /// The `ApprovalResolved` event (Architecture §6).
        ApprovalResolved => "ApprovalResolved",
        /// The `ApprovalRevoked` event (Architecture §6).
        ApprovalRevoked => "ApprovalRevoked",
        /// The `ReportWritten` event (Architecture §6).
        ReportWritten => "ReportWritten",
        /// The `ExportStatusChanged` event (Architecture §6).
        ExportStatusChanged => "ExportStatusChanged",
        /// The `DependencyStatusChanged` event (Architecture §6).
        DependencyStatusChanged => "DependencyStatusChanged",
        /// The `CredentialStatusChanged` event (Architecture §6).
        CredentialStatusChanged => "CredentialStatusChanged",
        /// The `ChatRetrievalCompleted` event (Architecture §6).
        ChatRetrievalCompleted => "ChatRetrievalCompleted",
        /// The `ChatResponseStarted` event (Architecture §6).
        ChatResponseStarted => "ChatResponseStarted",
        /// The `ChatResponseFinished` event (Architecture §6).
        ChatResponseFinished => "ChatResponseFinished",
        /// The `ChatResponseChunk` event (Architecture §6).
        ChatResponseChunk => "ChatResponseChunk",
        /// The `ChatFailed` event (Architecture §6).
        ChatFailed => "ChatFailed",
        /// The `StorageFailure` event (Architecture §6).
        StorageFailure => "StorageFailure",
        /// The `OperationFailed` event (Architecture §6).
        OperationFailed => "OperationFailed",
        /// The `SuppressionChanged` event (Architecture §6).
        SuppressionChanged => "SuppressionChanged",
        /// The `RunComparisonReady` event (Architecture §6).
        RunComparisonReady => "RunComparisonReady",
    }
);

closed_enum!(
    /// What started a run (`RunSummary.trigger`).
    RunTrigger, 3, {
        /// Started by the user.
        Manual => "manual",
        /// Started by an interval schedule.
        Interval => "interval",
        /// Started by a daily schedule.
        Daily => "daily",
    }
);

/// The record of one run in the activity log (`$defs/RunSummary`).
///
/// Not enforced by the typed layer: cross-field and cross-record semantic rules (Data Schema §11/§18, T09),
/// for example timestamp order and agreement of `execution` with the timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    /// Identifier of the run.
    pub run_id: RunId,
    /// What started the run.
    pub trigger: RunTrigger,
    /// Execution state.
    pub execution: RunState,
    /// When the run was created.
    pub created_at: UtcTimestamp,
    /// When execution began, if it did.
    pub started_at: Option<UtcTimestamp>,
    /// When execution ended, if it did.
    pub finished_at: Option<UtcTimestamp>,
    /// The frozen plan of the run.
    pub plan: PlanSnapshot,
    /// What the run covered.
    pub coverage: CoverageSummary,
    /// Reports written for the run.
    pub report_ids: Vec<ReportId>,
    /// When the run last recorded progress, if it did.
    pub last_checkpoint_at: Option<UtcTimestamp>,
}

/// One persisted application event (`$defs/Event`, contract amendment CA-04).
///
/// Not enforced by the typed layer: which event names may be persisted and which identifiers each requires
/// (Data Schema §11, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Identifier of the event.
    pub event_id: EventId,
    /// When the event was emitted.
    pub at: UtcTimestamp,
    /// Severity.
    pub level: EventLevel,
    /// Architecture §6 event name.
    pub event_type: EventType,
    /// Run the event belongs to, if any.
    pub run_id: Option<RunId>,
    /// Repository the event belongs to, if any.
    pub repo_id: Option<RepoId>,
    /// Workflow the event belongs to, if any.
    pub workflow_id: Option<WorkflowId>,
    /// Task the event belongs to, if any.
    pub task_id: Option<TaskId>,
    /// Approval the event belongs to, if any.
    pub approval_id: Option<ApprovalId>,
    /// Detail text or a fixed summary (class R: detail text can carry tool or repository text).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub message_redacted: RedactedText,
    /// Failure code, if the event reports a failure.
    pub code: Option<EventCode>,
}

/// Where the report files of one run and repository are and how writing them went (`$defs/ReportManifest`).
///
/// Not enforced by the typed layer: the cross-field rules between the `*_written`, `*_digest`, `pdf_*`
/// fields and the paths (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportManifest {
    /// Identifier of the report.
    pub report_id: ReportId,
    /// Run the report describes.
    pub run_id: RunId,
    /// Repository the report describes.
    pub repo_id: RepoId,
    /// Path of the JSON evidence file, relative to the app-data root.
    pub json_path: AppRelPath,
    /// Path of the Markdown report, relative to the app-data root.
    pub md_path: AppRelPath,
    /// Whether the JSON file was written.
    pub json_written: bool,
    /// Whether the Markdown file was written.
    pub md_written: bool,
    /// Digest of the written JSON file, if it was written.
    pub json_digest: Option<Digest>,
    /// When the report was generated.
    pub generated_at: UtcTimestamp,
    /// Path of the PDF export, if one was requested, relative to the app-data root.
    pub pdf_path: Option<AppRelPath>,
    /// Whether the PDF was written.
    pub pdf_written: bool,
    /// Digest of the written PDF, if it was written.
    pub pdf_digest: Option<Digest>,
    /// Why the PDF export failed (class R: renderer output), if it did.
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    pub pdf_error_redacted: Option<RedactedText>,
    /// Version of the PDF renderer, if one ran.
    pub pdf_renderer_version: Option<VersionLabel>,
}

/// Root of `log.json` (`$defs/LogFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogFile {
    /// Schema contract version, always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Root kind, always `"log"`.
    pub store_type: LogStoreType,
    /// When the file was last written.
    pub updated_at: UtcTimestamp,
    /// Run summaries.
    pub runs: Vec<RunSummary>,
    /// Persisted events.
    pub events: Vec<Event>,
    /// Report manifests.
    pub reports: Vec<ReportManifest>,
}

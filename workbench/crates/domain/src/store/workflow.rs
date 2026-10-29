//! Schedule, Workflow, WorkflowFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1b, T05b.
//!
//! The typed layer checks shapes, closed value sets and newtype syntax. It does not check `revision >= 1`,
//! `interval_seconds >= 1`, `output_formats` `minItems`/`uniqueItems`, or which schedule fields must be set for
//! which `kind`: schema validation (T08) and the semantic rules (T09, Data §11) cover those.

use serde::{Deserialize, Serialize};

use super::plan::{RepoTarget, ScanSettings};
use super::values::{IanaTimezone, LocalTime};
use super::{SchemaVersion, WorkflowsStoreType};
use crate::contract::ExportFormat;
use crate::ids::{ApprovalId, WorkflowId};
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

closed_enum!(
    /// How a workflow is triggered (`Schedule.kind`).
    ScheduleKind, 3, {
        /// Started by the user only.
        Manual => "manual",
        /// Started every `interval_seconds` while the application is open.
        Interval => "interval",
        /// Started once a day at `daily_local_time` in `timezone` while the application is open.
        Daily => "daily",
    }
);

closed_enum!(
    /// What happens when a scheduled run is due while the previous one is still running
    /// (`Schedule.overlap_policy`).
    OverlapPolicy, 1, {
        /// The due run is skipped.
        SkipIfRunning => "skip_if_running",
    }
);

closed_enum!(
    /// What happens to runs that fell due while the application was closed (`Schedule.missed_policy`).
    MissedPolicy, 1, {
        /// The miss is recorded and not replayed.
        RecordNoReplay => "record_no_replay",
    }
);

/// When a workflow runs (`$defs/Schedule`). Schedules execute only while the application is open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// Trigger kind.
    pub kind: ScheduleKind,
    /// Period in seconds for `interval` schedules; `null` otherwise. The schema minimum of 1 is not
    /// enforced here.
    pub interval_seconds: Option<u64>,
    /// Local wall-clock time (`HH:MM`) for `daily` schedules; `null` otherwise.
    pub daily_local_time: Option<LocalTime>,
    /// IANA time zone the local time refers to (syntax only; membership is checked later).
    pub timezone: IanaTimezone,
    /// Next due instant, `null` when not scheduled or not yet computed.
    pub next_due_at: Option<UtcTimestamp>,
    /// Overlap handling.
    pub overlap_policy: OverlapPolicy,
    /// Missed-run handling.
    pub missed_policy: MissedPolicy,
}

/// A saved, revisioned assessment definition (`$defs/Workflow`).
///
/// `name` and `description` are author text (FR-31 class R) and are re-escaped on read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    /// Workflow identifier.
    pub workflow_id: WorkflowId,
    /// Revision counter; the schema minimum of 1 is not enforced here.
    pub revision: u64,
    /// Display name.
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub name: RedactedText,
    /// Free-text description.
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub description: RedactedText,
    /// Whether the workflow may be started.
    pub enabled: bool,
    /// Creation time.
    pub created_at: UtcTimestamp,
    /// Time of the last change.
    pub updated_at: UtcTimestamp,
    /// Repositories, references and file selections to assess.
    pub targets: Vec<RepoTarget>,
    /// What to assess and how.
    pub scan_settings: ScanSettings,
    /// When to run.
    pub schedule: Schedule,
    /// Approvals this workflow relies on.
    pub approval_ids: Vec<ApprovalId>,
    /// Report formats to produce. The schema's `minItems: 1` and `uniqueItems` are not enforced here.
    pub output_formats: Vec<ExportFormat>,
}

/// The `workflows.json` root (`$defs/WorkflowFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFile {
    /// Always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Always `"workflows"`.
    pub store_type: WorkflowsStoreType,
    /// Time of the last write.
    pub updated_at: UtcTimestamp,
    /// The saved workflows.
    pub workflows: Vec<Workflow>,
}

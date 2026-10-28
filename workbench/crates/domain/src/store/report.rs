//! Check, Finding, CoverageSummary, AIEnrichment, ReportFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//! A report file is the per-run, per-repository evidence document (Data Schema §13).

use serde::{Deserialize, Serialize};

use super::evidence::{Artifact, Engine, SourceRef};
use super::plan::PlanSnapshot;
use super::provider::{OpenRouterId, ProviderPolicy};
use super::values::{
    Digest, FrameworkId, GitOid, GitRefName, ModelId, ProviderId, RedactionVersion,
};
use super::{ReportStoreType, SchemaVersion};
use crate::contract::AdapterId;
use crate::ids::{
    ApprovalId, CheckId, FileId, FindingId, RepoId, ReportId, RunId, SourceId, SuppressionId,
    TaskId,
};
use crate::states::{CheckOutcome, RunState};
use crate::text::{RedactedText, SanitisedText};
use crate::time::UtcTimestamp;
use crate::vocab::{Assessment, Category, Coverage, Severity};

closed_enum!(
    /// How the tool process of a check ended (`Check.native_status`).
    NativeStatus, 6, {
        /// The tool was never started.
        NotStarted => "not_started",
        /// The tool ran and exited on its own.
        Exited => "exited",
        /// The tool was stopped at its time limit.
        TimedOut => "timed_out",
        /// The tool was stopped by cancellation.
        Cancelled => "cancelled",
        /// The tool could not be launched.
        LaunchFailed => "launch_failed",
        /// The tool was stopped at its output limit.
        OutputLimitExceeded => "output_limit_exceeded",
    }
);

closed_enum!(
    /// How certain the tool is about a finding (`Finding.confidence`).
    Confidence, 4, {
        /// High confidence.
        High => "high",
        /// Medium confidence.
        Medium => "medium",
        /// Low confidence.
        Low => "low",
        /// The tool does not state a confidence.
        Unknown => "unknown",
    }
);

closed_enum!(
    /// Triage state of a finding (`Finding.review_status`).
    ReviewStatus, 3, {
        /// Not seen in an earlier run.
        New => "new",
        /// Seen in an earlier run.
        Existing => "existing",
        /// Matched by a suppression.
        Suppressed => "suppressed",
    }
);

closed_enum!(
    /// Outcome of the optional model enrichment of a report (`AIEnrichment.status`).
    EnrichmentStatus, 5, {
        /// No enrichment was requested.
        NotRequested => "not_requested",
        /// The enrichment completed.
        Completed => "completed",
        /// The request failed.
        Failed => "failed",
        /// No compliant route or credential was available.
        Unavailable => "unavailable",
        /// The user denied the transmission.
        Denied => "denied",
    }
);

/// One check of a run: a category assessed by one tool over one file or the whole target (`$defs/Check`).
///
/// Not enforced by the typed layer: `term_signal` is at least 1 (schema `minimum`); cross-field and
/// cross-record semantic rules (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// Identifier of the check.
    pub check_id: CheckId,
    /// Task the check ran under.
    pub task_id: TaskId,
    /// File the check assessed, or `None` for a whole-target check.
    pub file_id: Option<FileId>,
    /// Assessment category.
    pub category: Category,
    /// Framework the check applies to, if it is framework-specific.
    pub framework: Option<FrameworkId>,
    /// Tool and rule data used.
    pub engine: Engine,
    /// Result of the check.
    pub outcome: CheckOutcome,
    /// Why the check did not produce a clean result (class R: may quote tool output).
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    pub reason_redacted: Option<RedactedText>,
    /// When the check started, if it did.
    pub started_at: Option<UtcTimestamp>,
    /// When the check finished, if it did.
    pub finished_at: Option<UtcTimestamp>,
    /// Findings produced by the check.
    pub finding_ids: Vec<FindingId>,
    /// Evidence the check examined.
    pub source_refs: Vec<SourceRef>,
    /// Application adapter that ran the tool.
    pub adapter_id: AdapterId,
    /// Digest of the tool configuration used, if any.
    pub config_digest: Option<Digest>,
    /// Digest of the tool manifest entry used, if known.
    pub manifest_digest: Option<Digest>,
    /// Exit code of the tool process, if it exited.
    pub exit_code: Option<i64>,
    /// Signal that ended the tool process, if any.
    pub term_signal: Option<u64>,
    /// How the tool process ended.
    pub native_status: NativeStatus,
    /// Approvals the check ran under.
    pub approval_ids: Vec<ApprovalId>,
}

/// A security or quality observation of a check (`$defs/Finding`).
///
/// Not enforced by the typed layer: cross-field and cross-record semantic rules (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// Identifier of the finding.
    pub finding_id: FindingId,
    /// Stable digest identifying the same issue across runs.
    pub fingerprint: Digest,
    /// Check that produced the finding.
    pub check_id: CheckId,
    /// Task the check ran under.
    pub task_id: TaskId,
    /// Repository the finding belongs to.
    pub repo_id: RepoId,
    /// Run the finding belongs to.
    pub run_id: RunId,
    /// Commit the finding was observed at.
    pub commit_oid: GitOid,
    /// Assessment category.
    pub category: Category,
    /// Severity.
    pub severity: Severity,
    /// Confidence of the tool.
    pub confidence: Confidence,
    /// Tool rule identifier (class R: tools and repositories can supply it).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub rule_id: RedactedText,
    /// Short title (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub title: RedactedText,
    /// Description (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub description_redacted: RedactedText,
    /// Evidence excerpt (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub evidence_redacted: RedactedText,
    /// Remediation advice (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub remediation_redacted: RedactedText,
    /// Tool and rule data that produced the finding.
    pub engine: Engine,
    /// Evidence the finding rests on.
    pub sources: Vec<SourceRef>,
    /// Triage state.
    pub review_status: ReviewStatus,
    /// Suppression that matched, if any.
    pub suppression_id: Option<SuppressionId>,
    /// When the finding was observed.
    pub observed_at: UtcTimestamp,
}

/// Counts and verdict describing what a run covered (`$defs/CoverageSummary`).
///
/// Counters carry no upper bound in the schema and are `u64`. Not enforced by the typed layer: the
/// relations between the counters, `coverage` and `assessment` (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageSummary {
    /// Files found in the repositories.
    pub discovered_files: u64,
    /// Files selected for assessment.
    pub selected_files: u64,
    /// Files that were assessed.
    pub assessed_files: u64,
    /// Files excluded by selection or policy.
    pub excluded_files: u64,
    /// Selected paths that were not found.
    pub missing_files: u64,
    /// Checks that passed.
    pub checks_passed: u64,
    /// Checks that produced findings.
    pub checks_with_findings: u64,
    /// Checks that failed.
    pub checks_failed: u64,
    /// Checks the tools do not support for the target.
    pub checks_unsupported: u64,
    /// Checks that were skipped.
    pub checks_skipped: u64,
    /// Checks with partial evidence only.
    pub checks_incomplete: u64,
    /// Checks that were cancelled.
    pub checks_cancelled: u64,
    /// How much of the selection was covered.
    pub coverage: Coverage,
    /// Overall assessment, never a bare "secure".
    pub assessment: Assessment,
}

/// The optional model-written narrative of a report and how it was obtained (`$defs/AIEnrichment`).
///
/// `routing_policy`, when present, must be the fixed policy; [`ProviderPolicy`] enforces that. Not enforced by
/// the typed layer: the agreement of the other fields with `status` (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AIEnrichment {
    /// How the enrichment ended.
    pub status: EnrichmentStatus,
    /// Provider used, if a request was made.
    pub provider_id: Option<OpenRouterId>,
    /// Model the user asked for, if a request was made.
    pub requested_model: Option<ModelId>,
    /// Model that answered, if one did.
    pub actual_model: Option<ModelId>,
    /// Upstream provider that answered, if known.
    pub actual_provider: Option<ProviderId>,
    /// Approvals the transmission ran under.
    pub approval_ids: Vec<ApprovalId>,
    /// Evidence that was transmitted.
    pub source_ids: Vec<SourceId>,
    /// Routing and privacy policy applied, if a request was made.
    pub routing_policy: Option<ProviderPolicy>,
    /// Redaction version applied to the transmitted context.
    pub redaction_version: RedactionVersion,
    /// When the narrative was generated, if it was.
    pub generated_at: Option<UtcTimestamp>,
    /// The narrative (class R: model output; empty when none was produced).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub narrative_redacted: RedactedText,
    /// Why no narrative exists (class R), if so.
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    pub reason_redacted: Option<RedactedText>,
}

/// The evidence document of one run and one repository, `backup/` report (`$defs/ReportFile`).
///
/// Not enforced by the typed layer: cross-field and cross-record semantic rules, including reference
/// resolution within the document and agreement with `log.json` (Data Schema §11/§18, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportFile {
    /// Schema contract version, always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Root kind, always `"report"`.
    pub store_type: ReportStoreType,
    /// When the file was last written.
    pub updated_at: UtcTimestamp,
    /// Identifier of the report.
    pub report_id: ReportId,
    /// Run the report describes.
    pub run_id: RunId,
    /// Repository the report describes.
    pub repo_id: RepoId,
    /// Repository name (class R: user-supplied or derived from the remote).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub repo_name: RedactedText,
    /// Remote identifier with credentials stripped (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub remote_identifier_redacted: RedactedText,
    /// Reference that was assessed.
    pub selected_ref: GitRefName,
    /// Commit the reference resolved to, if it did.
    pub commit_oid: Option<GitOid>,
    /// Execution state of the run when the report was written.
    pub execution: RunState,
    /// The plan the run followed.
    pub plan: PlanSnapshot,
    /// When the report was generated.
    pub generated_at: UtcTimestamp,
    /// What the run covered.
    pub coverage: CoverageSummary,
    /// Checks of the run.
    pub checks: Vec<Check>,
    /// Findings of the run.
    pub findings: Vec<Finding>,
    /// Artifacts of the run.
    pub artifacts: Vec<Artifact>,
    /// Limitations the application states about the result (class S: application-composed text).
    pub limitations: Vec<SanitisedText>,
    /// Optional model enrichment.
    pub ai_enrichment: AIEnrichment,
}

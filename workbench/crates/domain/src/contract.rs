//! Application command/event contract v1.0 (Architecture §6; frozen surface
//! `planning/P01-T01-request.md` §5.5). Payload field list frozen by the lead at baseline B1.
//!
//! Commands are in-process requests from the UI reducer to the coordinator: they may carry raw user
//! input ([`RawInput`]) and the BYOK key ([`CredentialSecret`]); both mask their `Debug`, and commands have no serde. Events are
//! projections back to the UI and to the activity log: free text is [`RedactedText`] or
//! [`crate::text::SanitisedText`] only, and events serialise with `event_type` equal to the Architecture §6 name.
//! No event carries key material, raw secrets or raw child-process output.

use crate::ids::{
    ApprovalId, CheckId, ConversationId, FileId, FindingId, MessageId, RepoId, ReportId, RunId,
    SourceId, TaskId, WorkflowId,
};
use crate::states::{ApprovalState, CheckOutcome, RunState};
use crate::text::RedactedText;
use crate::time::UtcTimestamp;
use crate::vocab::{Assessment, Capability, Category, Coverage, Severity};
use serde::Serialize;
use std::fmt;

/// Application command/event contract version (Architecture §6, A-11).
pub const APP_CONTRACT_VERSION: &str = "1.0";

/// Whether a command or event is implemented in the pilot or only reserved.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ContractStatus {
    /// Implemented in the pilot.
    Pilot,
    /// Name reserved for deferred work; dispatching it fails with code `deferred`.
    Reserved,
}

/// Delivery rule for an event (Architecture §6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeliveryClass {
    /// Never dropped or coalesced.
    MustDeliver,
    /// Intermediate values may be replaced by the latest.
    LatestValue,
    /// May be merged without losing content.
    Coalescible,
}

/// The BYOK key as typed by the user. No serde, no `Clone`; `Debug` never shows the value; the buffer
/// is overwritten on drop as a best effort only (earlier reallocations are not covered; not a
/// secure-erase guarantee — KEY-01 hardening is P03).
pub struct CredentialSecret(String);

impl CredentialSecret {
    /// Wraps the key typed into the credential dialog.
    pub fn new(secret: String) -> Self {
        Self(secret)
    }

    /// The key, for the credential subsystem only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialSecret(<redacted>)")
    }
}

impl Drop for CredentialSecret {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        let capacity = bytes.capacity();
        bytes.clear();
        bytes.resize(capacity, 0);
        std::hint::black_box(&bytes);
    }
}

/// Raw text typed by the user (URL, names, chat message). Validated, sanitised or redacted by the
/// subsystem that consumes it; never logged: `Debug` shows only the length. No serde, no `Display`.
pub struct RawInput(String);

impl RawInput {
    /// Wraps user-typed text.
    pub fn new(text: String) -> Self {
        Self(text)
    }

    /// The raw text, for the consuming subsystem only.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RawInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawInput(<{} bytes>)", self.0.len())
    }
}

/// Credential storage mode (KEY-01; OS store is conditional on a verified backend).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialMode {
    /// Held in memory for this session only.
    SessionOnly,
    /// OS credential store (only if verified; never a plaintext fallback).
    OsStore,
}

/// Credential operation requested by `ConfigureCredential`.
#[derive(Debug)]
pub enum CredentialAction {
    /// Store a new key.
    Set {
        /// The key.
        secret: CredentialSecret,
        /// Where to keep it.
        mode: CredentialMode,
    },
    /// Replace the current key.
    Replace {
        /// The new key.
        secret: CredentialSecret,
        /// Where to keep it.
        mode: CredentialMode,
    },
    /// Validate the current key with the provider.
    Validate,
    /// Delete the current key.
    Delete,
}

/// A selection change (DISC-03). Filtering is a UI concern and never changes selection by itself.
#[derive(Debug)]
pub enum SelectionChange {
    /// Select or clear one file.
    File {
        /// Repository of the file.
        repo_id: RepoId,
        /// The file.
        file_id: FileId,
        /// New state.
        selected: bool,
    },
    /// Select or clear every eligible file of a repository.
    Repository {
        /// The repository.
        repo_id: RepoId,
        /// New state.
        selected: bool,
    },
    /// Select or clear exactly the listed (visible) files.
    Visible {
        /// Files currently visible after filtering.
        file_ids: Vec<FileId>,
        /// New state.
        selected: bool,
    },
    /// Select or clear everything eligible.
    All {
        /// New state.
        selected: bool,
    },
    /// Set a repository's selector mode (schema `Selector.mode`).
    Mode {
        /// The repository.
        repo_id: RepoId,
        /// New mode.
        mode: SelectorMode,
    },
    /// Set a repository's include/exclude patterns (`patterns` mode); validated as non-shell
    /// patterns by the planner (Data §11.7).
    Patterns {
        /// The repository.
        repo_id: RepoId,
        /// Include patterns as typed.
        include: Vec<RawInput>,
        /// Exclude patterns as typed.
        exclude: Vec<RawInput>,
    },
}

/// File selector mode (schema `Selector.mode`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorMode {
    /// Every eligible YAML file, including newly discovered ones.
    AllYaml,
    /// Exactly the chosen files.
    Explicit,
    /// Files matching include/exclude patterns.
    Patterns,
}

/// Decision on a pending approval request (UI Schema §10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Allow this one operation.
    ApproveOnce,
    /// Allow within the saved workflow's visible revision.
    ApproveWorkflowScope,
    /// Refuse.
    Deny,
}

/// Report export format (RES-04, RES-08).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// Authoritative JSON report.
    Json,
    /// Deterministic Markdown (store value `md`).
    #[serde(rename = "md")]
    Markdown,
    /// Restricted local PDF.
    Pdf,
}

/// Commands from the UI reducer to the coordinator (Architecture §6). In-process only; no serde.
#[derive(Debug)]
pub enum AppCommand {
    /// Validate a URL, register it and request acquisition approval (REP-01).
    AddRepository { url: RawInput },
    /// Clone if absent, otherwise fast-forward update under lock (REP-02, REP-05).
    RefreshRepository { repo_id: RepoId },
    /// Deregister with explicit retention choices (Data §14).
    RemoveRepository {
        repo_id: RepoId,
        delete_checkout: bool,
        purge_derived_index: bool,
        retain_reports: bool,
    },
    /// Re-inventory a repository (DISC-01–DISC-05).
    DiscoverFiles { repo_id: RepoId },
    /// Apply a tristate selection change (DISC-03).
    UpdateSelection { change: SelectionChange },
    /// Build and freeze a reviewable plan; lists required approvals.
    ReviewScanPlan {
        workflow_id: Option<WorkflowId>,
        repo_ids: Vec<RepoId>,
        file_ids: Vec<FileId>,
        categories: Vec<Category>,
    },
    /// Execute a reviewed plan (the run announced by `RunPlanned`).
    StartScan { run_id: RunId },
    /// Request supervisor cancellation of a run (WF-05).
    CancelRun { run_id: RunId },
    /// Create or update a workflow; the coordinator increments the revision (WF-01). Selector, schedule
    /// and settings fields are added with the workflow model (P14) under a contract minor version.
    SaveWorkflow {
        workflow_id: Option<WorkflowId>,
        expected_revision: Option<u32>,
        name: RawInput,
        repo_ids: Vec<RepoId>,
        categories: Vec<Category>,
    },
    /// Remove a workflow definition; history retained.
    DeleteWorkflow { workflow_id: WorkflowId },
    /// Enable or disable app-open triggers (WF-02).
    SetWorkflowEnabled {
        workflow_id: WorkflowId,
        enabled: bool,
    },
    /// Manual run of a saved workflow.
    RunWorkflow { workflow_id: WorkflowId },
    /// Approve once, approve workflow scope, or deny a pending request.
    ResolveApproval {
        approval_id: ApprovalId,
        decision: ApprovalDecision,
    },
    /// Revoke an existing grant.
    RevokeApproval { approval_id: ApprovalId },
    /// Set, validate, replace or delete the BYOK key (KEY-01).
    ConfigureCredential { action: CredentialAction },
    /// Set the explicit model ID and optional provider allowlist (KEY-02).
    ConfigureProvider {
        model_id: RawInput,
        provider_allowlist: Vec<RawInput>,
    },
    /// Choose the repo/run/finding context for retrieval (RAG-01, RAG-07).
    SetChatContext {
        conversation_id: ConversationId,
        repo_ids: Vec<RepoId>,
        run_id: Option<RunId>,
        file_ids: Vec<FileId>,
        finding_ids: Vec<FindingId>,
    },
    /// Retrieve, redact, request approval if needed, send (RAG).
    SendChatMessage {
        conversation_id: ConversationId,
        text: RawInput,
    },
    /// Stop an in-flight streamed response.
    CancelChatResponse {
        conversation_id: ConversationId,
        message_id: MessageId,
    },
    /// Locate an existing report export (no external open).
    ExportReport {
        report_id: ReportId,
        format: ExportFormat,
    },
    /// Re-derive a Markdown or PDF export from retained JSON (RES-08).
    RegenerateExport {
        report_id: ReportId,
        format: ExportFormat,
    },
    /// Re-run preflight and refresh the tools view (TOOL-01).
    InspectDependencies,
    /// Cancel work, persist, restore the terminal.
    RequestShutdown,
    /// Reserved (RES-06 deferred).
    SuppressFinding,
    /// Reserved (RES-06 deferred).
    UpdateSuppression,
    /// Reserved (history comparison deferred).
    CompareRuns,
    /// Reserved (AI report narration deferred, Q-02).
    GenerateReportNarrative,
    /// Reserved (chat persistence deferred).
    PersistConversation,
    /// Reserved (SCAN-06 deferred).
    StartLiveVerification,
}

/// Repository operation reported by `RepositoryStatusChanged`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoOperation {
    /// Initial clone.
    Clone,
    /// Fast-forward update.
    Update,
}

/// Progress of a repository operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoOperationStatus {
    /// Started.
    Started,
    /// Finished successfully.
    Succeeded,
    /// Failed; repository skipped for this run.
    Failed,
    /// Not attempted (e.g. unsafe local state, approval denied).
    Skipped,
}

/// Why a trigger did not start a run (WF-06).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSkipReason {
    /// The same workflow was already running.
    Overlap,
    /// The due time passed while the application was closed.
    MissedWhileClosed,
    /// The workflow is disabled.
    Disabled,
    /// Required approvals are pending.
    AwaitingApproval,
}

/// Per-format export status (Data §12.5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    /// Written and verified.
    Written,
    /// Writing failed; regeneration possible from retained JSON.
    Failed,
    /// Not yet written.
    Pending,
    /// Not part of this export set.
    NotRequested,
}

/// Data category an approval may disclose (schema `Approval.data_categories`). Raw secrets are never a
/// category (Data §11.16).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataCategory {
    /// Repository metadata.
    RepositoryMetadata,
    /// Redacted source excerpts.
    RedactedSource,
    /// Redacted findings.
    RedactedFindings,
    /// Schema documents.
    SchemaDocuments,
    /// Public artifact identifiers.
    PublicArtifactIdentifiers,
    /// Redacted chat context.
    RedactedChatContext,
    /// Application metadata.
    AppMetadata,
}

/// Grant lifetime (schema `Approval.lifetime`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLifetime {
    /// One operation.
    Once,
    /// One run.
    Run,
    /// The saved workflow's current revision.
    WorkflowRevision,
}

/// Planned adapter identifiers (Data Schema §21). A closed set: no free-text tool names in events.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum AdapterId {
    /// Git.
    #[serde(rename = "git")]
    Git,
    /// yamllint.
    #[serde(rename = "yamllint")]
    Yamllint,
    /// Gitleaks.
    #[serde(rename = "gitleaks")]
    Gitleaks,
    /// Checkov.
    #[serde(rename = "checkov")]
    Checkov,
    /// actionlint.
    #[serde(rename = "actionlint")]
    Actionlint,
    /// kubeconform.
    #[serde(rename = "kubeconform")]
    Kubeconform,
    /// zizmor (conditional X1).
    #[serde(rename = "zizmor")]
    Zizmor,
    /// Docker Compose (conditional X2).
    #[serde(rename = "docker-compose")]
    DockerCompose,
    /// Helm (conditional X3).
    #[serde(rename = "helm")]
    Helm,
    /// Trivy (conditional X4).
    #[serde(rename = "trivy")]
    Trivy,
    /// In-process Rust schema validator.
    #[serde(rename = "rust-schema")]
    RustSchema,
}

/// Tool availability as shown in diagnostics (UI Schema §5 `tools.states`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Installed, compatible and verified.
    Ready,
    /// Not installed.
    Missing,
    /// Installed version not compatible.
    Incompatible,
    /// Present but not usable.
    Unavailable,
    /// Use not permitted.
    Denied,
    /// Capability deferred.
    Deferred,
}

/// Credential state; never key material (KEY-01).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    /// No key configured.
    Absent,
    /// Key held for this session only.
    SessionOnly,
    /// Key held in a verified OS credential store.
    Stored,
    /// The provider rejected the key.
    Invalid,
    /// The OS store is unavailable; no plaintext fallback.
    StoreUnavailable,
}

/// How a chat response ended.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatOutcome {
    /// Completed normally.
    Completed,
    /// Cancelled by the user.
    Cancelled,
    /// Stream ended early.
    Incomplete,
}

/// Typed chat failure (Tech §20).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatFailure {
    /// Invalid key.
    InvalidKey,
    /// Insufficient credits.
    Credits,
    /// Rate limited.
    RateLimit,
    /// No route satisfies the privacy policy.
    NoCompliantRoute,
    /// Request timed out.
    Timeout,
    /// Streamed response truncated.
    Truncation,
    /// Provider unavailable (UI Schema §15 chat states).
    ProviderOutage,
}

/// Failure code of `OperationFailed`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// The command is reserved for deferred work (Architecture §6).
    Deferred,
    /// The command's input was rejected.
    InvalidInput,
    /// A referenced entity does not exist.
    NotFound,
    /// The operation conflicts with current state (e.g. overlap, stale revision).
    Conflict,
    /// A permission grant is required first.
    PermissionRequired,
    /// Unexpected internal failure.
    Internal,
}

/// Events from the coordinator to the UI and activity log (Architecture §6).
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "event_type")]
pub enum AppEvent {
    /// A repository was registered.
    RepositoryRegistered {
        repo_id: RepoId,
        display_name: RedactedText,
    },
    /// A repository was deregistered.
    RepositoryRemoved { repo_id: RepoId },
    /// Clone/update started, succeeded, failed or was skipped.
    RepositoryStatusChanged {
        repo_id: RepoId,
        operation: RepoOperation,
        status: RepoOperationStatus,
        reason: Option<RedactedText>,
    },
    /// Discovery finished for a repository.
    InventoryUpdated {
        repo_id: RepoId,
        discovered_files: u32,
        excluded_paths: u32,
        discovery_errors: u32,
    },
    /// Selection counts changed (latest value).
    SelectionChanged {
        selected_repositories: u32,
        selected_files: u32,
    },
    /// A workflow was saved.
    WorkflowSaved {
        workflow_id: WorkflowId,
        revision: u32,
    },
    /// A workflow was deleted.
    WorkflowDeleted { workflow_id: WorkflowId },
    /// A workflow was enabled or disabled.
    WorkflowStateChanged {
        workflow_id: WorkflowId,
        enabled: bool,
    },
    /// A trigger fired.
    TriggerFired {
        workflow_id: WorkflowId,
        at: UtcTimestamp,
    },
    /// A trigger did not start a run.
    TriggerSkipped {
        workflow_id: WorkflowId,
        at: UtcTimestamp,
        reason: TriggerSkipReason,
    },
    /// A run plan was frozen.
    RunPlanned {
        run_id: RunId,
        workflow_id: Option<WorkflowId>,
        required_approval_ids: Vec<ApprovalId>,
    },
    /// A run started.
    RunStarted { run_id: RunId, at: UtcTimestamp },
    /// A run changed state (includes cancelling and awaiting_approval).
    RunStateChanged {
        run_id: RunId,
        from: RunState,
        to: RunState,
    },
    /// A task started.
    TaskStarted {
        run_id: RunId,
        task_id: TaskId,
        repo_id: RepoId,
        category: Category,
    },
    /// A check reached a terminal outcome.
    CheckCompleted {
        run_id: RunId,
        task_id: TaskId,
        check_id: CheckId,
        outcome: CheckOutcome,
        findings: u32,
    },
    /// Task progress (coalescible).
    TaskProgress {
        run_id: RunId,
        task_id: TaskId,
        completed_units: u32,
        total_units: Option<u32>,
    },
    /// A finding was recorded.
    FindingObserved {
        run_id: RunId,
        check_id: CheckId,
        finding_id: FindingId,
        repo_id: RepoId,
        file_id: Option<FileId>,
        severity: Severity,
        category: Category,
        title: RedactedText,
    },
    /// A run reached a terminal state.
    RunFinished {
        run_id: RunId,
        state: RunState,
        assessment: Assessment,
        coverage: Coverage,
    },
    /// A permission request awaits a decision.
    ApprovalRequested {
        approval_id: ApprovalId,
        capability: Capability,
        purpose: RedactedText,
        workflow_id: Option<WorkflowId>,
        workflow_revision: Option<u32>,
        repo_ids: Vec<RepoId>,
        destinations: Vec<RedactedText>,
        data_categories: Vec<DataCategory>,
        lifetime: ApprovalLifetime,
    },
    /// A permission request was decided.
    ApprovalResolved {
        approval_id: ApprovalId,
        state: ApprovalState,
    },
    /// A grant was revoked.
    ApprovalRevoked { approval_id: ApprovalId },
    /// A report export set was written (per-format status).
    ReportWritten {
        report_id: ReportId,
        run_id: RunId,
        repo_id: RepoId,
        json: ExportStatus,
        md: ExportStatus,
        pdf: ExportStatus,
    },
    /// One export format changed status (regeneration outcome).
    ExportStatusChanged {
        report_id: ReportId,
        format: ExportFormat,
        status: ExportStatus,
        location: Option<RedactedText>,
    },
    /// A tool's availability changed.
    DependencyStatusChanged {
        adapter_id: AdapterId,
        status: ToolStatus,
    },
    /// Credential state changed; never key material.
    CredentialStatusChanged { status: CredentialStatus },
    /// Retrieval selected these sources.
    ChatRetrievalCompleted {
        conversation_id: ConversationId,
        source_ids: Vec<SourceId>,
    },
    /// A streamed response started.
    ChatResponseStarted {
        conversation_id: ConversationId,
        message_id: MessageId,
    },
    /// A streamed response ended.
    ChatResponseFinished {
        conversation_id: ConversationId,
        message_id: MessageId,
        outcome: ChatOutcome,
    },
    /// A chunk of model text (coalescible by merging, never dropped).
    ChatResponseChunk {
        conversation_id: ConversationId,
        message_id: MessageId,
        text: RedactedText,
    },
    /// A chat request failed.
    ChatFailed {
        conversation_id: ConversationId,
        reason: ChatFailure,
    },
    /// Durable storage failed.
    StorageFailure { detail: RedactedText },
    /// A command failed.
    OperationFailed {
        code: FailureCode,
        detail: RedactedText,
    },
    /// Reserved (RES-06 deferred).
    SuppressionChanged,
    /// Reserved (history comparison deferred).
    RunComparisonReady,
}

impl AppCommand {
    /// Pilot or reserved (Architecture §6).
    #[allow(clippy::todo)]
    pub fn status(&self) -> ContractStatus {
        todo!("P01-T08")
    }
}

impl AppEvent {
    /// Pilot or reserved (Architecture §6).
    #[allow(clippy::todo)]
    pub fn status(&self) -> ContractStatus {
        todo!("P01-T08")
    }

    /// Delivery rule (Architecture §6).
    #[allow(clippy::todo)]
    pub fn delivery(&self) -> DeliveryClass {
        todo!("P01-T08")
    }
}

/// Rejects reserved commands with `OperationFailed { code: Deferred }` (Architecture §6).
#[allow(
    clippy::result_large_err,
    reason = "signature frozen by planning/P01-T01-request.md §5.5; called once per dispatched command"
)]
#[allow(clippy::todo, unused_variables)]
pub fn dispatch_guard(cmd: &AppCommand) -> Result<(), AppEvent> {
    todo!("P01-T08")
}

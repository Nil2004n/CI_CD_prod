//! P01-T08 tests: application command/event contract v1.0 behaviour
//! (`planning/P01-T01-request.md` §5.5 and §6.3 row T08; Architecture §6 tables).
//!
//! Expected tables are written out literally and are not derived from the implementation. The
//! compile-time guards (no `Clone`/`PartialEq`/`Hash`/`Display`/serde on the secret and raw-input
//! wrappers, no serde on commands, no `Deserialize` on events, no `String` or `SanitisedText` as an
//! event's `RedactedText`) are the `compile_fail` doctests in `src/contract.rs`, each paired with a
//! passing positive control that names the same type.
//! All secrets below are synthetic, recognisably fake markers.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use domain::contract::{
    APP_CONTRACT_VERSION, AdapterId, AppCommand, AppEvent, ApprovalDecision, ApprovalLifetime,
    ChatFailure, ChatOutcome, ContractStatus, CredentialAction, CredentialMode, CredentialSecret,
    CredentialStatus, DataCategory, DeliveryClass, ExportFormat, ExportStatus, FailureCode,
    RawInput, RepoOperation, RepoOperationStatus, SelectionChange, SelectorMode, ToolAvailability,
    TriggerSkipReason, dispatch_guard,
};
use domain::ids::{
    ApprovalId, CheckId, ConversationId, FileId, FindingId, MessageId, RepoId, ReportId, RunId,
    SourceId, TaskId, WorkflowId,
};
use domain::states::{ApprovalState, CheckOutcome, RunState};
use domain::text::{RedactedText, Redactor, redact};
use domain::time::UtcTimestamp;
use domain::vocab::{Assessment, Capability, Category, Coverage, Severity};
use serde::Serialize;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------------------------
// Literal expected tables (Architecture §6).
// ---------------------------------------------------------------------------------------------

use ContractStatus::{Pilot, Reserved};
use DeliveryClass::{Coalescible, LatestValue, MustDeliver};

/// The 29 commands with their status.
const COMMAND_TABLE: [(&str, ContractStatus); 29] = [
    ("AddRepository", Pilot),
    ("RefreshRepository", Pilot),
    ("RemoveRepository", Pilot),
    ("DiscoverFiles", Pilot),
    ("UpdateSelection", Pilot),
    ("ReviewScanPlan", Pilot),
    ("StartScan", Pilot),
    ("CancelRun", Pilot),
    ("SaveWorkflow", Pilot),
    ("DeleteWorkflow", Pilot),
    ("SetWorkflowEnabled", Pilot),
    ("RunWorkflow", Pilot),
    ("ResolveApproval", Pilot),
    ("RevokeApproval", Pilot),
    ("ConfigureCredential", Pilot),
    ("ConfigureProvider", Pilot),
    ("SetChatContext", Pilot),
    ("SendChatMessage", Pilot),
    ("CancelChatResponse", Pilot),
    ("ExportReport", Pilot),
    ("RegenerateExport", Pilot),
    ("InspectDependencies", Pilot),
    ("RequestShutdown", Pilot),
    ("SuppressFinding", Reserved),
    ("UpdateSuppression", Reserved),
    ("CompareRuns", Reserved),
    ("GenerateReportNarrative", Reserved),
    ("PersistConversation", Reserved),
    ("StartLiveVerification", Reserved),
];

/// The 34 events with their status and delivery class.
const EVENT_TABLE: [(&str, ContractStatus, DeliveryClass); 34] = [
    ("RepositoryRegistered", Pilot, MustDeliver),
    ("RepositoryRemoved", Pilot, MustDeliver),
    ("RepositoryStatusChanged", Pilot, MustDeliver),
    ("InventoryUpdated", Pilot, MustDeliver),
    ("SelectionChanged", Pilot, LatestValue),
    ("WorkflowSaved", Pilot, MustDeliver),
    ("WorkflowDeleted", Pilot, MustDeliver),
    ("WorkflowStateChanged", Pilot, MustDeliver),
    ("TriggerFired", Pilot, MustDeliver),
    ("TriggerSkipped", Pilot, MustDeliver),
    ("RunPlanned", Pilot, MustDeliver),
    ("RunStarted", Pilot, MustDeliver),
    ("RunStateChanged", Pilot, MustDeliver),
    ("TaskStarted", Pilot, MustDeliver),
    ("CheckCompleted", Pilot, MustDeliver),
    ("TaskProgress", Pilot, Coalescible),
    ("FindingObserved", Pilot, MustDeliver),
    ("RunFinished", Pilot, MustDeliver),
    ("ApprovalRequested", Pilot, MustDeliver),
    ("ApprovalResolved", Pilot, MustDeliver),
    ("ApprovalRevoked", Pilot, MustDeliver),
    ("ReportWritten", Pilot, MustDeliver),
    ("ExportStatusChanged", Pilot, MustDeliver),
    ("DependencyStatusChanged", Pilot, MustDeliver),
    ("CredentialStatusChanged", Pilot, MustDeliver),
    ("ChatRetrievalCompleted", Pilot, MustDeliver),
    ("ChatResponseStarted", Pilot, MustDeliver),
    ("ChatResponseFinished", Pilot, MustDeliver),
    ("ChatResponseChunk", Pilot, Coalescible),
    ("ChatFailed", Pilot, MustDeliver),
    ("StorageFailure", Pilot, MustDeliver),
    ("OperationFailed", Pilot, MustDeliver),
    ("SuppressionChanged", Reserved, MustDeliver),
    ("RunComparisonReady", Reserved, MustDeliver),
];

/// The exact `detail` of the `OperationFailed` returned for a reserved command.
const DEFERRED_DETAIL: &str =
    "command is reserved for deferred work and is not available in the pilot";

const SECRET_MARKER: &str = "FAKE-SECRET-0000";
const TOKEN_MARKER: &str = "FAKE-TOKEN-0000";

// ---------------------------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------------------------

const U_REPO: &str = "00000000-0000-4000-8000-000000000001";
const U_FILE: &str = "00000000-0000-4000-8000-000000000002";
const U_RUN: &str = "00000000-0000-4000-8000-000000000003";
const U_TASK: &str = "00000000-0000-4000-8000-000000000004";
const U_CHECK: &str = "00000000-0000-4000-8000-000000000005";
const U_FINDING: &str = "00000000-0000-4000-8000-000000000006";
const U_WORKFLOW: &str = "00000000-0000-4000-8000-000000000007";
const U_APPROVAL: &str = "00000000-0000-4000-8000-000000000008";
const U_REPORT: &str = "00000000-0000-4000-8000-000000000009";
const U_CONVERSATION: &str = "00000000-0000-4000-8000-00000000000a";
const U_MESSAGE: &str = "00000000-0000-4000-8000-00000000000b";
const U_SOURCE: &str = "00000000-0000-4000-8000-00000000000c";

fn repo() -> RepoId {
    RepoId::parse(U_REPO).expect("repo id")
}
fn file() -> FileId {
    FileId::parse(U_FILE).expect("file id")
}
fn run() -> RunId {
    RunId::parse(U_RUN).expect("run id")
}
fn task() -> TaskId {
    TaskId::parse(U_TASK).expect("task id")
}
fn check() -> CheckId {
    CheckId::parse(U_CHECK).expect("check id")
}
fn finding() -> FindingId {
    FindingId::parse(U_FINDING).expect("finding id")
}
fn workflow() -> WorkflowId {
    WorkflowId::parse(U_WORKFLOW).expect("workflow id")
}
fn approval() -> ApprovalId {
    ApprovalId::parse(U_APPROVAL).expect("approval id")
}
fn report() -> ReportId {
    ReportId::parse(U_REPORT).expect("report id")
}
fn conversation() -> ConversationId {
    ConversationId::parse(U_CONVERSATION).expect("conversation id")
}
fn message() -> MessageId {
    MessageId::parse(U_MESSAGE).expect("message id")
}
fn source() -> SourceId {
    SourceId::parse(U_SOURCE).expect("source id")
}

/// 2023-11-14T22:13:20Z.
fn at() -> UtcTimestamp {
    UtcTimestamp::from_unix(1_700_000_000, 0).expect("timestamp")
}

/// 2023-11-14T22:13:20.5Z.
fn at_fraction() -> UtcTimestamp {
    UtcTimestamp::from_unix(1_700_000_000, 500_000_000).expect("timestamp")
}

/// Leaves text unchanged.
struct Identity;

impl Redactor for Identity {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(raw)
    }
}

/// Replaces both synthetic markers.
struct MarkerRedactor;

impl Redactor for MarkerRedactor {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        if raw.contains(SECRET_MARKER) || raw.contains(TOKEN_MARKER) {
            Cow::Owned(
                raw.replace(SECRET_MARKER, "[REDACTED]")
                    .replace(TOKEN_MARKER, "[REDACTED]"),
            )
        } else {
            Cow::Borrowed(raw)
        }
    }
}

fn rt(text: &str) -> RedactedText {
    redact(text, &Identity)
}

fn raw(text: &str) -> RawInput {
    RawInput::new(String::from(text))
}

/// One value of every `AppCommand` variant, in table order.
fn all_commands() -> Vec<AppCommand> {
    vec![
        AppCommand::AddRepository {
            url: raw("https://example.invalid/r.git"),
        },
        AppCommand::RefreshRepository { repo_id: repo() },
        AppCommand::RemoveRepository {
            repo_id: repo(),
            delete_checkout: true,
            purge_derived_index: false,
            retain_reports: true,
        },
        AppCommand::DiscoverFiles { repo_id: repo() },
        AppCommand::UpdateSelection {
            change: SelectionChange::All { selected: true },
        },
        AppCommand::ReviewScanPlan {
            workflow_id: None,
            repo_ids: vec![repo()],
            file_ids: vec![file()],
            categories: vec![Category::Syntax],
        },
        AppCommand::StartScan { run_id: run() },
        AppCommand::CancelRun { run_id: run() },
        AppCommand::SaveWorkflow {
            workflow_id: None,
            expected_revision: Some(2),
            name: raw("nightly"),
            repo_ids: vec![repo()],
            categories: vec![Category::Secrets],
        },
        AppCommand::DeleteWorkflow {
            workflow_id: workflow(),
        },
        AppCommand::SetWorkflowEnabled {
            workflow_id: workflow(),
            enabled: true,
        },
        AppCommand::RunWorkflow {
            workflow_id: workflow(),
        },
        AppCommand::ResolveApproval {
            approval_id: approval(),
            decision: ApprovalDecision::ApproveOnce,
        },
        AppCommand::RevokeApproval {
            approval_id: approval(),
        },
        AppCommand::ConfigureCredential {
            action: CredentialAction::Delete,
        },
        AppCommand::ConfigureProvider {
            model_id: raw("example/model"),
            provider_allowlist: vec![raw("provider-a")],
        },
        AppCommand::SetChatContext {
            conversation_id: conversation(),
            repo_ids: vec![repo()],
            run_id: Some(run()),
            file_ids: vec![file()],
            finding_ids: vec![finding()],
        },
        AppCommand::SendChatMessage {
            conversation_id: conversation(),
            text: raw("hello"),
        },
        AppCommand::CancelChatResponse {
            conversation_id: conversation(),
            message_id: message(),
        },
        AppCommand::ExportReport {
            report_id: report(),
            format: ExportFormat::Json,
        },
        AppCommand::RegenerateExport {
            report_id: report(),
            format: ExportFormat::Pdf,
        },
        AppCommand::InspectDependencies,
        AppCommand::RequestShutdown,
        AppCommand::SuppressFinding,
        AppCommand::UpdateSuppression,
        AppCommand::CompareRuns,
        AppCommand::GenerateReportNarrative,
        AppCommand::PersistConversation,
        AppCommand::StartLiveVerification,
    ]
}

/// Test-side exhaustive name: a new command variant fails to compile here until it is listed.
fn command_name(cmd: &AppCommand) -> &'static str {
    match cmd {
        AppCommand::AddRepository { .. } => "AddRepository",
        AppCommand::RefreshRepository { .. } => "RefreshRepository",
        AppCommand::RemoveRepository { .. } => "RemoveRepository",
        AppCommand::DiscoverFiles { .. } => "DiscoverFiles",
        AppCommand::UpdateSelection { .. } => "UpdateSelection",
        AppCommand::ReviewScanPlan { .. } => "ReviewScanPlan",
        AppCommand::StartScan { .. } => "StartScan",
        AppCommand::CancelRun { .. } => "CancelRun",
        AppCommand::SaveWorkflow { .. } => "SaveWorkflow",
        AppCommand::DeleteWorkflow { .. } => "DeleteWorkflow",
        AppCommand::SetWorkflowEnabled { .. } => "SetWorkflowEnabled",
        AppCommand::RunWorkflow { .. } => "RunWorkflow",
        AppCommand::ResolveApproval { .. } => "ResolveApproval",
        AppCommand::RevokeApproval { .. } => "RevokeApproval",
        AppCommand::ConfigureCredential { .. } => "ConfigureCredential",
        AppCommand::ConfigureProvider { .. } => "ConfigureProvider",
        AppCommand::SetChatContext { .. } => "SetChatContext",
        AppCommand::SendChatMessage { .. } => "SendChatMessage",
        AppCommand::CancelChatResponse { .. } => "CancelChatResponse",
        AppCommand::ExportReport { .. } => "ExportReport",
        AppCommand::RegenerateExport { .. } => "RegenerateExport",
        AppCommand::InspectDependencies => "InspectDependencies",
        AppCommand::RequestShutdown => "RequestShutdown",
        AppCommand::SuppressFinding => "SuppressFinding",
        AppCommand::UpdateSuppression => "UpdateSuppression",
        AppCommand::CompareRuns => "CompareRuns",
        AppCommand::GenerateReportNarrative => "GenerateReportNarrative",
        AppCommand::PersistConversation => "PersistConversation",
        AppCommand::StartLiveVerification => "StartLiveVerification",
    }
}

/// Test-side exhaustive name for events (must equal the serialised `event_type`).
fn event_name(ev: &AppEvent) -> &'static str {
    match ev {
        AppEvent::RepositoryRegistered { .. } => "RepositoryRegistered",
        AppEvent::RepositoryRemoved { .. } => "RepositoryRemoved",
        AppEvent::RepositoryStatusChanged { .. } => "RepositoryStatusChanged",
        AppEvent::InventoryUpdated { .. } => "InventoryUpdated",
        AppEvent::SelectionChanged { .. } => "SelectionChanged",
        AppEvent::WorkflowSaved { .. } => "WorkflowSaved",
        AppEvent::WorkflowDeleted { .. } => "WorkflowDeleted",
        AppEvent::WorkflowStateChanged { .. } => "WorkflowStateChanged",
        AppEvent::TriggerFired { .. } => "TriggerFired",
        AppEvent::TriggerSkipped { .. } => "TriggerSkipped",
        AppEvent::RunPlanned { .. } => "RunPlanned",
        AppEvent::RunStarted { .. } => "RunStarted",
        AppEvent::RunStateChanged { .. } => "RunStateChanged",
        AppEvent::TaskStarted { .. } => "TaskStarted",
        AppEvent::CheckCompleted { .. } => "CheckCompleted",
        AppEvent::TaskProgress { .. } => "TaskProgress",
        AppEvent::FindingObserved { .. } => "FindingObserved",
        AppEvent::RunFinished { .. } => "RunFinished",
        AppEvent::ApprovalRequested { .. } => "ApprovalRequested",
        AppEvent::ApprovalResolved { .. } => "ApprovalResolved",
        AppEvent::ApprovalRevoked { .. } => "ApprovalRevoked",
        AppEvent::ReportWritten { .. } => "ReportWritten",
        AppEvent::ExportStatusChanged { .. } => "ExportStatusChanged",
        AppEvent::DependencyStatusChanged { .. } => "DependencyStatusChanged",
        AppEvent::CredentialStatusChanged { .. } => "CredentialStatusChanged",
        AppEvent::ChatRetrievalCompleted { .. } => "ChatRetrievalCompleted",
        AppEvent::ChatResponseStarted { .. } => "ChatResponseStarted",
        AppEvent::ChatResponseFinished { .. } => "ChatResponseFinished",
        AppEvent::ChatResponseChunk { .. } => "ChatResponseChunk",
        AppEvent::ChatFailed { .. } => "ChatFailed",
        AppEvent::StorageFailure { .. } => "StorageFailure",
        AppEvent::OperationFailed { .. } => "OperationFailed",
        AppEvent::SuppressionChanged => "SuppressionChanged",
        AppEvent::RunComparisonReady => "RunComparisonReady",
    }
}

fn to_json(ev: &AppEvent) -> Value {
    serde_json::to_value(ev).expect("event serialises")
}

fn event_type(ev: &AppEvent) -> String {
    to_json(ev)
        .get("event_type")
        .and_then(Value::as_str)
        .expect("event_type string")
        .to_owned()
}

/// Leading identifier of a `Debug` string (the variant name).
fn debug_prefix(dbg: &str) -> &str {
    let end = dbg
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(dbg.len());
    &dbg[..end]
}

/// One event of every variant with its literal expected JSON (Option `None` -> `null`).
fn golden_cases() -> Vec<(AppEvent, Value)> {
    vec![
        (
            AppEvent::RepositoryRegistered {
                repo_id: repo(),
                display_name: rt("example-repo"),
            },
            json!({"event_type": "RepositoryRegistered", "repo_id": U_REPO, "display_name": "example-repo"}),
        ),
        (
            AppEvent::RepositoryRemoved { repo_id: repo() },
            json!({"event_type": "RepositoryRemoved", "repo_id": U_REPO}),
        ),
        (
            AppEvent::RepositoryStatusChanged {
                repo_id: repo(),
                operation: RepoOperation::Clone,
                status: RepoOperationStatus::Succeeded,
                reason: None,
            },
            json!({"event_type": "RepositoryStatusChanged", "repo_id": U_REPO, "operation": "clone", "status": "succeeded", "reason": null}),
        ),
        (
            AppEvent::InventoryUpdated {
                repo_id: repo(),
                discovered_files: 12,
                excluded_paths: 3,
                discovery_errors: 1,
            },
            json!({"event_type": "InventoryUpdated", "repo_id": U_REPO, "discovered_files": 12, "excluded_paths": 3, "discovery_errors": 1}),
        ),
        (
            AppEvent::SelectionChanged {
                selected_repositories: 2,
                selected_files: 40,
            },
            json!({"event_type": "SelectionChanged", "selected_repositories": 2, "selected_files": 40}),
        ),
        (
            AppEvent::WorkflowSaved {
                workflow_id: workflow(),
                revision: 4,
            },
            json!({"event_type": "WorkflowSaved", "workflow_id": U_WORKFLOW, "revision": 4}),
        ),
        (
            AppEvent::WorkflowDeleted {
                workflow_id: workflow(),
            },
            json!({"event_type": "WorkflowDeleted", "workflow_id": U_WORKFLOW}),
        ),
        (
            AppEvent::WorkflowStateChanged {
                workflow_id: workflow(),
                enabled: false,
            },
            json!({"event_type": "WorkflowStateChanged", "workflow_id": U_WORKFLOW, "enabled": false}),
        ),
        (
            AppEvent::TriggerFired {
                workflow_id: workflow(),
                fired_at: at(),
            },
            json!({"event_type": "TriggerFired", "workflow_id": U_WORKFLOW, "fired_at": "2023-11-14T22:13:20Z"}),
        ),
        (
            AppEvent::TriggerSkipped {
                workflow_id: workflow(),
                skipped_at: at_fraction(),
                reason: TriggerSkipReason::MissedWhileClosed,
            },
            json!({"event_type": "TriggerSkipped", "workflow_id": U_WORKFLOW, "skipped_at": "2023-11-14T22:13:20.5Z", "reason": "missed_while_closed"}),
        ),
        (
            AppEvent::RunPlanned {
                run_id: run(),
                workflow_id: None,
                required_approval_ids: vec![approval()],
            },
            json!({"event_type": "RunPlanned", "run_id": U_RUN, "workflow_id": null, "required_approval_ids": [U_APPROVAL]}),
        ),
        (
            AppEvent::RunStarted {
                run_id: run(),
                started_at: at(),
            },
            json!({"event_type": "RunStarted", "run_id": U_RUN, "started_at": "2023-11-14T22:13:20Z"}),
        ),
        (
            AppEvent::RunStateChanged {
                run_id: run(),
                from: RunState::Running,
                to: RunState::Cancelling,
            },
            json!({"event_type": "RunStateChanged", "run_id": U_RUN, "from": "running", "to": "cancelling"}),
        ),
        (
            AppEvent::TaskStarted {
                run_id: run(),
                task_id: task(),
                repo_id: repo(),
                category: Category::Schema,
            },
            json!({"event_type": "TaskStarted", "run_id": U_RUN, "task_id": U_TASK, "repo_id": U_REPO, "category": "schema"}),
        ),
        (
            AppEvent::CheckCompleted {
                run_id: run(),
                task_id: task(),
                check_id: check(),
                outcome: CheckOutcome::Findings,
                findings: 5,
            },
            json!({"event_type": "CheckCompleted", "run_id": U_RUN, "task_id": U_TASK, "check_id": U_CHECK, "outcome": "findings", "findings": 5}),
        ),
        (
            AppEvent::TaskProgress {
                run_id: run(),
                task_id: task(),
                completed_units: 7,
                total_units: None,
            },
            json!({"event_type": "TaskProgress", "run_id": U_RUN, "task_id": U_TASK, "completed_units": 7, "total_units": null}),
        ),
        (
            AppEvent::FindingObserved {
                run_id: run(),
                check_id: check(),
                finding_id: finding(),
                repo_id: repo(),
                file_id: None,
                severity: Severity::High,
                category: Category::Secrets,
                title: rt("hardcoded token"),
            },
            json!({"event_type": "FindingObserved", "run_id": U_RUN, "check_id": U_CHECK, "finding_id": U_FINDING, "repo_id": U_REPO, "file_id": null, "severity": "high", "category": "secrets", "title": "hardcoded token"}),
        ),
        (
            AppEvent::RunFinished {
                run_id: run(),
                state: RunState::Completed,
                assessment: Assessment::Findings,
                coverage: Coverage::Partial,
            },
            json!({"event_type": "RunFinished", "run_id": U_RUN, "state": "completed", "assessment": "findings", "coverage": "partial"}),
        ),
        (
            AppEvent::ApprovalRequested {
                approval_id: approval(),
                capability: Capability::ModelTransmission,
                purpose: rt("send redacted context"),
                workflow_id: None,
                workflow_revision: None,
                repo_ids: vec![repo()],
                destinations: vec![rt("openrouter.example.invalid")],
                data_categories: vec![DataCategory::RedactedSource, DataCategory::AppMetadata],
                lifetime: ApprovalLifetime::WorkflowRevision,
            },
            json!({"event_type": "ApprovalRequested", "approval_id": U_APPROVAL, "capability": "model_transmission", "purpose": "send redacted context", "workflow_id": null, "workflow_revision": null, "repo_ids": [U_REPO], "destinations": ["openrouter.example.invalid"], "data_categories": ["redacted_source", "app_metadata"], "lifetime": "workflow_revision"}),
        ),
        (
            AppEvent::ApprovalResolved {
                approval_id: approval(),
                state: ApprovalState::Approved,
            },
            json!({"event_type": "ApprovalResolved", "approval_id": U_APPROVAL, "state": "approved"}),
        ),
        (
            AppEvent::ApprovalRevoked {
                approval_id: approval(),
            },
            json!({"event_type": "ApprovalRevoked", "approval_id": U_APPROVAL}),
        ),
        (
            AppEvent::ReportWritten {
                report_id: report(),
                run_id: run(),
                repo_id: repo(),
                json: ExportStatus::Written,
                md: ExportStatus::Failed,
                pdf: ExportStatus::NotRequested,
            },
            json!({"event_type": "ReportWritten", "report_id": U_REPORT, "run_id": U_RUN, "repo_id": U_REPO, "json": "written", "md": "failed", "pdf": "not_requested"}),
        ),
        (
            AppEvent::ExportStatusChanged {
                report_id: report(),
                format: ExportFormat::Markdown,
                status: ExportStatus::Pending,
                location: None,
            },
            json!({"event_type": "ExportStatusChanged", "report_id": U_REPORT, "format": "md", "status": "pending", "location": null}),
        ),
        (
            AppEvent::DependencyStatusChanged {
                adapter_id: AdapterId::DockerCompose,
                availability: ToolAvailability::Unconfigured,
            },
            json!({"event_type": "DependencyStatusChanged", "adapter_id": "docker-compose", "availability": "unconfigured"}),
        ),
        (
            AppEvent::CredentialStatusChanged {
                status: CredentialStatus::StoreUnavailable,
            },
            json!({"event_type": "CredentialStatusChanged", "status": "store_unavailable"}),
        ),
        (
            AppEvent::ChatRetrievalCompleted {
                conversation_id: conversation(),
                source_ids: vec![source()],
            },
            json!({"event_type": "ChatRetrievalCompleted", "conversation_id": U_CONVERSATION, "source_ids": [U_SOURCE]}),
        ),
        (
            AppEvent::ChatResponseStarted {
                conversation_id: conversation(),
                message_id: message(),
            },
            json!({"event_type": "ChatResponseStarted", "conversation_id": U_CONVERSATION, "message_id": U_MESSAGE}),
        ),
        (
            AppEvent::ChatResponseFinished {
                conversation_id: conversation(),
                message_id: message(),
                outcome: ChatOutcome::Incomplete,
            },
            json!({"event_type": "ChatResponseFinished", "conversation_id": U_CONVERSATION, "message_id": U_MESSAGE, "outcome": "incomplete"}),
        ),
        (
            AppEvent::ChatResponseChunk {
                conversation_id: conversation(),
                message_id: message(),
                text: rt("partial answer"),
            },
            json!({"event_type": "ChatResponseChunk", "conversation_id": U_CONVERSATION, "message_id": U_MESSAGE, "text": "partial answer"}),
        ),
        (
            AppEvent::ChatFailed {
                conversation_id: conversation(),
                reason: ChatFailure::NoCompliantRoute,
            },
            json!({"event_type": "ChatFailed", "conversation_id": U_CONVERSATION, "reason": "no_compliant_route"}),
        ),
        (
            AppEvent::StorageFailure {
                detail: rt("write failed"),
            },
            json!({"event_type": "StorageFailure", "detail": "write failed"}),
        ),
        (
            AppEvent::OperationFailed {
                code: FailureCode::InvalidInput,
                detail: rt("bad url"),
            },
            json!({"event_type": "OperationFailed", "code": "invalid_input", "detail": "bad url"}),
        ),
        (
            AppEvent::SuppressionChanged,
            json!({"event_type": "SuppressionChanged"}),
        ),
        (
            AppEvent::RunComparisonReady,
            json!({"event_type": "RunComparisonReady"}),
        ),
    ]
}

fn all_events() -> Vec<AppEvent> {
    golden_cases().into_iter().map(|(ev, _)| ev).collect()
}

fn serialised<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("serialises")
}

// ---------------------------------------------------------------------------------------------
// Name completeness.
// ---------------------------------------------------------------------------------------------

/// The envelope fields added at emission (CA-04, Architecture §6). An event payload that used one of these
/// names would be overwritten or duplicated when the envelope is flattened beside it (Q-T04-1, Q-D03-1).
const ENVELOPE_FIELDS: [&str; 3] = ["event_id", "at", "level"];

#[test]
fn no_event_payload_uses_an_envelope_field_name() {
    // `golden_cases` covers every event name (`constructed_events_cover_the_architecture_names_exactly`).
    for (event, _) in golden_cases() {
        let json = to_json(&event);
        let obj = json.as_object().expect("events serialise as objects");
        for name in ENVELOPE_FIELDS {
            assert!(
                !obj.contains_key(name),
                "{} uses envelope field `{name}`",
                event_type(&event)
            );
        }
    }
}

#[test]
fn contract_version_is_1_1() {
    // 1.0 -> 1.1 by CA-04 (Data Schema §25), applied 2026-10-27.
    assert_eq!(APP_CONTRACT_VERSION, "1.1");
}

#[test]
fn literal_tables_have_expected_sizes_and_unique_names() {
    assert_eq!(COMMAND_TABLE.len(), 29);
    assert_eq!(EVENT_TABLE.len(), 34);
    let commands: BTreeSet<_> = COMMAND_TABLE.iter().map(|r| r.0).collect();
    let events: BTreeSet<_> = EVENT_TABLE.iter().map(|r| r.0).collect();
    assert_eq!(commands.len(), 29);
    assert_eq!(events.len(), 34);
    assert_eq!(COMMAND_TABLE.iter().filter(|r| r.1 == Pilot).count(), 23);
    assert_eq!(COMMAND_TABLE.iter().filter(|r| r.1 == Reserved).count(), 6);
    assert_eq!(EVENT_TABLE.iter().filter(|r| r.1 == Pilot).count(), 32);
    assert_eq!(EVENT_TABLE.iter().filter(|r| r.1 == Reserved).count(), 2);
}

#[test]
fn constructed_commands_cover_the_architecture_names_exactly() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 29);
    let built: Vec<&str> = cmds.iter().map(command_name).collect();
    let expected: Vec<&str> = COMMAND_TABLE.iter().map(|r| r.0).collect();
    assert_eq!(built, expected, "order and content");
    // Debug prefix is the variant name for every command, independent of the test-side match.
    for cmd in &cmds {
        let dbg = format!("{cmd:?}");
        assert_eq!(debug_prefix(&dbg), command_name(cmd));
    }
}

#[test]
fn constructed_events_cover_the_architecture_names_exactly() {
    let evs = all_events();
    assert_eq!(evs.len(), 34);
    let built: Vec<String> = evs.iter().map(event_type).collect();
    let expected: Vec<&str> = EVENT_TABLE.iter().map(|r| r.0).collect();
    assert_eq!(built, expected, "serialised event_type, order and content");
    for ev in &evs {
        assert_eq!(event_type(ev), event_name(ev));
        let dbg = format!("{ev:?}");
        assert_eq!(debug_prefix(&dbg), event_name(ev));
    }
}

// ---------------------------------------------------------------------------------------------
// status(), delivery(), dispatch_guard().
// ---------------------------------------------------------------------------------------------

#[test]
fn command_status_matches_the_literal_table() {
    for cmd in all_commands() {
        let name = command_name(&cmd);
        let expected = COMMAND_TABLE
            .iter()
            .find(|r| r.0 == name)
            .unwrap_or_else(|| panic!("{name} missing from table"))
            .1;
        assert_eq!(cmd.status(), expected, "{name}");
    }
}

#[test]
fn event_status_and_delivery_match_the_literal_table() {
    for ev in all_events() {
        let name = event_name(&ev);
        let row = EVENT_TABLE
            .iter()
            .find(|r| r.0 == name)
            .unwrap_or_else(|| panic!("{name} missing from table"));
        assert_eq!(ev.status(), row.1, "{name} status");
        assert_eq!(ev.delivery(), row.2, "{name} delivery");
    }
}

#[test]
fn delivery_classes_are_pinned() {
    let evs = all_events();
    let names_of = |class: DeliveryClass| -> BTreeSet<&'static str> {
        evs.iter()
            .filter(|e| e.delivery() == class)
            .map(event_name)
            .collect()
    };
    assert_eq!(names_of(LatestValue), BTreeSet::from(["SelectionChanged"]));
    assert_eq!(
        names_of(Coalescible),
        BTreeSet::from(["TaskProgress", "ChatResponseChunk"])
    );
    assert_eq!(names_of(MustDeliver).len(), 31);
}

#[test]
fn dispatch_guard_accepts_every_pilot_command() {
    let mut pilot = 0;
    for cmd in all_commands() {
        if cmd.status() == Pilot {
            assert!(dispatch_guard(&cmd).is_ok(), "{}", command_name(&cmd));
            pilot += 1;
        }
    }
    assert_eq!(pilot, 23);
}

#[test]
fn dispatch_guard_rejects_every_reserved_command_with_deferred() {
    let mut reserved = 0;
    for cmd in all_commands() {
        if cmd.status() != Reserved {
            continue;
        }
        reserved += 1;
        let name = command_name(&cmd);
        let Err(ev) = dispatch_guard(&cmd) else {
            panic!("{name} was accepted");
        };
        let AppEvent::OperationFailed { code, detail } = &ev else {
            panic!("{name}: wrong event {ev:?}");
        };
        assert_eq!(*code, FailureCode::Deferred, "{name}");
        assert_eq!(detail.as_str(), DEFERRED_DETAIL, "{name}");
        assert!(!detail.as_str().contains(['{', '}']), "{name}");
        assert!(!detail.as_str().contains(name), "{name}: name leaked");
        assert_eq!(
            to_json(&ev),
            json!({"event_type": "OperationFailed", "code": "deferred", "detail": DEFERRED_DETAIL}),
            "{name}"
        );
        assert_eq!(ev.status(), Pilot);
        assert_eq!(ev.delivery(), MustDeliver);
    }
    assert_eq!(reserved, 6);
}

#[test]
fn dispatch_guard_agrees_with_status_for_every_command() {
    for cmd in all_commands() {
        assert_eq!(
            dispatch_guard(&cmd).is_ok(),
            cmd.status() == Pilot,
            "{}",
            command_name(&cmd)
        );
    }
}

#[test]
fn dispatch_guard_is_repeatable_and_does_not_consume_the_command() {
    let cmd = AppCommand::CompareRuns;
    assert_eq!(dispatch_guard(&cmd), dispatch_guard(&cmd));
}

// ---------------------------------------------------------------------------------------------
// Serialisation.
// ---------------------------------------------------------------------------------------------

#[test]
fn every_event_variant_serialises_to_its_golden_json() {
    for (ev, expected) in golden_cases() {
        assert_eq!(to_json(&ev), expected, "{}", event_name(&ev));
    }
}

#[test]
fn optional_fields_serialise_when_present() {
    let ev = AppEvent::RepositoryStatusChanged {
        repo_id: repo(),
        operation: RepoOperation::Update,
        status: RepoOperationStatus::Failed,
        reason: Some(rt("network unreachable")),
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "RepositoryStatusChanged", "repo_id": U_REPO, "operation": "update", "status": "failed", "reason": "network unreachable"})
    );

    let ev = AppEvent::TaskProgress {
        run_id: run(),
        task_id: task(),
        completed_units: 3,
        total_units: Some(10),
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "TaskProgress", "run_id": U_RUN, "task_id": U_TASK, "completed_units": 3, "total_units": 10})
    );

    let ev = AppEvent::RunPlanned {
        run_id: run(),
        workflow_id: Some(workflow()),
        required_approval_ids: vec![],
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "RunPlanned", "run_id": U_RUN, "workflow_id": U_WORKFLOW, "required_approval_ids": []})
    );

    let ev = AppEvent::FindingObserved {
        run_id: run(),
        check_id: check(),
        finding_id: finding(),
        repo_id: repo(),
        file_id: Some(file()),
        severity: Severity::Info,
        category: Category::Style,
        title: rt("t"),
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "FindingObserved", "run_id": U_RUN, "check_id": U_CHECK, "finding_id": U_FINDING, "repo_id": U_REPO, "file_id": U_FILE, "severity": "info", "category": "style", "title": "t"})
    );

    let ev = AppEvent::ApprovalRequested {
        approval_id: approval(),
        capability: Capability::GitAcquire,
        purpose: rt("clone"),
        workflow_id: Some(workflow()),
        workflow_revision: Some(3),
        repo_ids: vec![],
        destinations: vec![],
        data_categories: vec![],
        lifetime: ApprovalLifetime::Once,
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "ApprovalRequested", "approval_id": U_APPROVAL, "capability": "git_acquire", "purpose": "clone", "workflow_id": U_WORKFLOW, "workflow_revision": 3, "repo_ids": [], "destinations": [], "data_categories": [], "lifetime": "once"})
    );

    let ev = AppEvent::ExportStatusChanged {
        report_id: report(),
        format: ExportFormat::Pdf,
        status: ExportStatus::Written,
        location: Some(rt("reports/r1.pdf")),
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "ExportStatusChanged", "report_id": U_REPORT, "format": "pdf", "status": "written", "location": "reports/r1.pdf"})
    );
}

#[test]
fn events_with_control_characters_serialise_escaped_text() {
    let ev = AppEvent::StorageFailure {
        detail: rt("a\u{1b}[31mb"),
    };
    assert_eq!(
        to_json(&ev),
        json!({"event_type": "StorageFailure", "detail": "a<U+001B>[31mb"})
    );
}

#[test]
fn adapter_ids_serialise_to_the_data_registry_ids() {
    let table = [
        (AdapterId::Git, "git"),
        (AdapterId::Yamllint, "yamllint"),
        (AdapterId::Gitleaks, "gitleaks"),
        (AdapterId::Checkov, "checkov"),
        (AdapterId::Actionlint, "actionlint"),
        (AdapterId::Kubeconform, "kubeconform"),
        (AdapterId::Zizmor, "zizmor"),
        (AdapterId::DockerCompose, "docker-compose"),
        (AdapterId::Helm, "helm"),
        (AdapterId::Trivy, "trivy"),
        (AdapterId::RustSchema, "rust-schema"),
    ];
    assert_eq!(table.len(), 11);
    for (id, expected) in table {
        assert_eq!(serialised(id), json!(expected));
    }
}

#[test]
fn contract_enums_serialise_to_snake_case_strings() {
    macro_rules! check {
        ($($value:expr => $text:expr),+ $(,)?) => {
            $( assert_eq!(serialised($value), json!($text)); )+
        };
    }
    check!(
        CredentialMode::SessionOnly => "session_only",
        CredentialMode::OsStore => "os_store",
        SelectorMode::AllYaml => "all_yaml",
        SelectorMode::Explicit => "explicit",
        SelectorMode::Patterns => "patterns",
        ApprovalDecision::ApproveOnce => "approve_once",
        ApprovalDecision::ApproveWorkflowScope => "approve_workflow_scope",
        ApprovalDecision::Deny => "deny",
        ExportFormat::Json => "json",
        ExportFormat::Markdown => "md",
        ExportFormat::Pdf => "pdf",
        RepoOperation::Clone => "clone",
        RepoOperation::Update => "update",
        RepoOperationStatus::Started => "started",
        RepoOperationStatus::Succeeded => "succeeded",
        RepoOperationStatus::Failed => "failed",
        RepoOperationStatus::Skipped => "skipped",
        TriggerSkipReason::Overlap => "overlap",
        TriggerSkipReason::MissedWhileClosed => "missed_while_closed",
        TriggerSkipReason::Disabled => "disabled",
        TriggerSkipReason::AwaitingApproval => "awaiting_approval",
        ExportStatus::Written => "written",
        ExportStatus::Failed => "failed",
        ExportStatus::Pending => "pending",
        ExportStatus::NotRequested => "not_requested",
        DataCategory::RepositoryMetadata => "repository_metadata",
        DataCategory::RedactedSource => "redacted_source",
        DataCategory::RedactedFindings => "redacted_findings",
        DataCategory::SchemaDocuments => "schema_documents",
        DataCategory::PublicArtifactIdentifiers => "public_artifact_identifiers",
        DataCategory::RedactedChatContext => "redacted_chat_context",
        DataCategory::AppMetadata => "app_metadata",
        ApprovalLifetime::Once => "once",
        ApprovalLifetime::Run => "run",
        ApprovalLifetime::WorkflowRevision => "workflow_revision",
        ToolAvailability::Ready => "ready",
        ToolAvailability::Missing => "missing",
        ToolAvailability::Incompatible => "incompatible",
        ToolAvailability::Unconfigured => "unconfigured",
        ToolAvailability::Unavailable => "unavailable",
        ToolAvailability::Deferred => "deferred",
        CredentialStatus::Absent => "absent",
        CredentialStatus::SessionOnly => "session_only",
        CredentialStatus::Stored => "stored",
        CredentialStatus::Invalid => "invalid",
        CredentialStatus::StoreUnavailable => "store_unavailable",
        ChatOutcome::Completed => "completed",
        ChatOutcome::Cancelled => "cancelled",
        ChatOutcome::Incomplete => "incomplete",
        ChatFailure::InvalidKey => "invalid_key",
        ChatFailure::Credits => "credits",
        ChatFailure::RateLimit => "rate_limit",
        ChatFailure::NoCompliantRoute => "no_compliant_route",
        ChatFailure::Timeout => "timeout",
        ChatFailure::Truncation => "truncation",
        ChatFailure::ProviderOutage => "provider_outage",
        FailureCode::Deferred => "deferred",
        FailureCode::InvalidInput => "invalid_input",
        FailureCode::NotFound => "not_found",
        FailureCode::Conflict => "conflict",
        FailureCode::PermissionRequired => "permission_required",
        FailureCode::Internal => "internal",
    );
}

#[test]
fn event_clone_and_equality_hold() {
    for ev in all_events() {
        assert_eq!(ev.clone(), ev);
    }
}

// ---------------------------------------------------------------------------------------------
// Secrecy.
// ---------------------------------------------------------------------------------------------

fn assert_no_markers(label: &str, text: &str) {
    assert!(!text.contains(SECRET_MARKER), "{label} leaks the secret");
    assert!(!text.contains(TOKEN_MARKER), "{label} leaks the token");
}

fn assert_debug_clean<T: std::fmt::Debug>(label: &str, value: &T) {
    assert_no_markers(label, &format!("{value:?}"));
    assert_no_markers(label, &format!("{value:#?}"));
}

#[test]
fn credential_secret_debug_hides_the_key() {
    let secret = CredentialSecret::new(String::from(SECRET_MARKER));
    assert_debug_clean("CredentialSecret", &secret);
    assert_eq!(format!("{secret:?}"), "CredentialSecret(<redacted>)");
    assert_eq!(secret.expose(), SECRET_MARKER);
}

/// P03 KEY-01 hardening (`planning/P03-T01-request.md` §5.1): the length is no longer shown either. Fails if
/// `Debug` shows the text, its byte length or anything but the fixed placeholder.
#[test]
fn raw_input_debug_shows_neither_text_nor_length() {
    let input = raw(SECRET_MARKER);
    assert_debug_clean("RawInput", &input);
    let shown = format!("{input:?}");
    assert_eq!(shown, "RawInput(<redacted>)");
    assert!(!shown.contains(&SECRET_MARKER.len().to_string()));
    assert_eq!(input.as_str(), SECRET_MARKER);
}

#[test]
fn credential_commands_debug_hides_the_key() {
    for mode in [CredentialMode::SessionOnly, CredentialMode::OsStore] {
        let set = AppCommand::ConfigureCredential {
            action: CredentialAction::Set {
                secret: CredentialSecret::new(String::from(SECRET_MARKER)),
                mode,
            },
        };
        let replace = AppCommand::ConfigureCredential {
            action: CredentialAction::Replace {
                secret: CredentialSecret::new(String::from(SECRET_MARKER)),
                mode,
            },
        };
        assert_debug_clean("ConfigureCredential::Set", &set);
        assert_debug_clean("ConfigureCredential::Replace", &replace);
        assert!(format!("{set:?}").contains("<redacted>"));
    }
    assert_debug_clean(
        "CredentialAction::Set",
        &CredentialAction::Set {
            secret: CredentialSecret::new(String::from(SECRET_MARKER)),
            mode: CredentialMode::SessionOnly,
        },
    );
}

#[test]
fn raw_input_commands_debug_hides_the_text() {
    let url = format!("https://user:{TOKEN_MARKER}@example.invalid/r");
    let commands = vec![
        AppCommand::AddRepository { url: raw(&url) },
        AppCommand::SendChatMessage {
            conversation_id: conversation(),
            text: raw(&format!("please use {SECRET_MARKER}")),
        },
        AppCommand::SaveWorkflow {
            workflow_id: None,
            expected_revision: None,
            name: raw(&format!("name {SECRET_MARKER}")),
            repo_ids: vec![],
            categories: vec![],
        },
        AppCommand::ConfigureProvider {
            model_id: raw(&format!("model-{SECRET_MARKER}")),
            provider_allowlist: vec![raw(&format!("prov-{TOKEN_MARKER}"))],
        },
        AppCommand::UpdateSelection {
            change: SelectionChange::Patterns {
                repo_id: repo(),
                include: vec![raw(&format!("inc-{SECRET_MARKER}"))],
                exclude: vec![raw(&format!("exc-{TOKEN_MARKER}"))],
            },
        },
    ];
    // P03 KEY-01 hardening: the masked placeholder must be shown, and no byte length (was "<N bytes>").
    for cmd in &commands {
        assert_debug_clean(command_name(cmd), cmd);
        let shown = format!("{cmd:?}");
        assert!(
            shown.contains("RawInput(<redacted>)"),
            "{}",
            command_name(cmd)
        );
        assert!(!shown.contains("bytes>"), "{}", command_name(cmd));
    }
}

#[test]
fn a_redactor_keeps_markers_out_of_serialised_events() {
    let dirty =
        |label: &str| format!("{label} {SECRET_MARKER} and https://u:{TOKEN_MARKER}@h.invalid");
    let r = |label: &str| redact(&dirty(label), &MarkerRedactor);
    let events = vec![
        AppEvent::RepositoryRegistered {
            repo_id: repo(),
            display_name: r("name"),
        },
        AppEvent::RepositoryStatusChanged {
            repo_id: repo(),
            operation: RepoOperation::Clone,
            status: RepoOperationStatus::Failed,
            reason: Some(r("reason")),
        },
        AppEvent::FindingObserved {
            run_id: run(),
            check_id: check(),
            finding_id: finding(),
            repo_id: repo(),
            file_id: None,
            severity: Severity::High,
            category: Category::Secrets,
            title: r("title"),
        },
        AppEvent::ApprovalRequested {
            approval_id: approval(),
            capability: Capability::ModelTransmission,
            purpose: r("purpose"),
            workflow_id: None,
            workflow_revision: None,
            repo_ids: vec![],
            destinations: vec![r("dest")],
            data_categories: vec![],
            lifetime: ApprovalLifetime::Once,
        },
        AppEvent::ExportStatusChanged {
            report_id: report(),
            format: ExportFormat::Json,
            status: ExportStatus::Written,
            location: Some(r("location")),
        },
        AppEvent::ChatResponseChunk {
            conversation_id: conversation(),
            message_id: message(),
            text: r("chunk"),
        },
        AppEvent::StorageFailure {
            detail: r("storage"),
        },
        AppEvent::OperationFailed {
            code: FailureCode::Internal,
            detail: r("operation"),
        },
    ];
    for ev in &events {
        let name = event_name(ev);
        let json_text = serde_json::to_string(ev).expect("serialises");
        assert_no_markers(name, &json_text);
        assert_no_markers(name, &format!("{ev:?}"));
        assert!(
            json_text.contains("[REDACTED]"),
            "{name}: redactor was applied"
        );
    }
}

#[test]
fn tool_availability_values_equal_the_store_schema_enum_in_order() {
    const STORE_SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");
    let schema: Value = serde_json::from_str(STORE_SCHEMA).expect("store schema parses");
    let schema_enum: Vec<&str> =
        schema["$defs"]["ToolManifest"]["properties"]["availability"]["enum"]
            .as_array()
            .expect("availability.enum is an array")
            .iter()
            .map(|v| v.as_str().expect("enum member is a string"))
            .collect();

    let all = [
        ToolAvailability::Ready,
        ToolAvailability::Missing,
        ToolAvailability::Incompatible,
        ToolAvailability::Unconfigured,
        ToolAvailability::Unavailable,
        ToolAvailability::Deferred,
    ];
    // Exhaustive guard: adding a variant without updating `all` fails to compile here.
    for value in all {
        match value {
            ToolAvailability::Ready
            | ToolAvailability::Missing
            | ToolAvailability::Incompatible
            | ToolAvailability::Unconfigured
            | ToolAvailability::Unavailable
            | ToolAvailability::Deferred => {}
        }
    }
    let code: Vec<String> = all
        .iter()
        .map(|v| {
            serialised(*v)
                .as_str()
                .expect("serialises to a string")
                .to_owned()
        })
        .collect();
    assert_eq!(code, schema_enum);
    assert!(
        !schema_enum.contains(&"denied"),
        "denied is not a tool state"
    );
}

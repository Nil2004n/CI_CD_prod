//! P02-T11 tests: the event envelope, `Emitter`, `IdSource`/`UrandomIds`, the level table and the
//! `AppEvent` -> store `Event` mapping (`planning/P02-T01-request.md` §5.8, §9 row T11, T01-D9, T01-D10).
//!
//! Expected values (golden JSON, the level table, the persisted set, the identifier slots) are written out
//! literally and are not derived from the code under test. Every test says what would make it fail. All
//! texts are synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::emit::{
    Clock, EmitError, Emitter, EventEnvelope, IdSource, IdSourceError, InvalidEventReason,
    SystemClock, UrandomIds, to_store_event,
};
use domain::contract::{
    AdapterId, AppEvent, ApprovalLifetime, ChatFailure, ChatOutcome, CredentialStatus,
    DataCategory, DeliveryClass, ExportFormat, ExportStatus, FailureCode, RepoOperation,
    RepoOperationStatus, ToolAvailability, TriggerSkipReason,
};
use domain::ids::{
    ApprovalId, CheckId, ConversationId, EventId, FileId, FindingId, MessageId, RepoId, ReportId,
    RunId, SourceId, TaskId, WorkflowId,
};
use domain::states::{ApprovalState, CheckOutcome, RunState};
use domain::store::StoreKind;
use domain::store::log::{Event, EventLevel, EventType, LogFile};
use domain::store::{LogStoreType, SchemaVersion};
use domain::text::{RedactedText, Redactor, redact};
use domain::time::UtcTimestamp;
use domain::vocab::{Assessment, Capability, Category, Coverage, Severity};
use persistence::schema::SchemaValidator;
use persistence::validate::{DocumentValidator, Layer, ProblemCode, Verdict};

// ---------------------------------------------------------------------------------------------------------
// Fixed values and helpers.
// ---------------------------------------------------------------------------------------------------------

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

/// Identifier of an envelope built by [`FixedIds`] from `[0x11; 16]` (version and variant bits applied).
const FIXED_EVENT_ID: &str = "11111111-1111-4111-9111-111111111111";

/// A recognisable string planted into payload texts; it must not reach an error or a persisted message.
const MARKER: &str = "SEEDEDMARKER-4f2a";

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

/// 2026-10-28T12:00:00Z, the instant every [`FixedClock`] reports.
fn fixed_now() -> UtcTimestamp {
    UtcTimestamp::parse_rfc3339("2026-10-28T12:00:00Z").expect("timestamp")
}

/// 2026-10-28T12:00:01Z.
fn later() -> UtcTimestamp {
    UtcTimestamp::parse_rfc3339("2026-10-28T12:00:01Z").expect("timestamp")
}

/// 2026-10-28T11:59:59.5Z.
fn earlier_fraction() -> UtcTimestamp {
    UtcTimestamp::parse_rfc3339("2026-10-28T11:59:59.5Z").expect("timestamp")
}

/// Leaves text unchanged.
struct Identity;

impl Redactor for Identity {
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(raw)
    }
}

fn rt(text: &str) -> RedactedText {
    redact(text, &Identity)
}

/// A clock that always reports [`fixed_now`].
#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> UtcTimestamp {
        fixed_now()
    }
}

/// An identifier source that always returns the same bytes and counts its calls.
#[derive(Debug)]
struct FixedIds {
    bytes: [u8; 16],
    calls: Arc<AtomicUsize>,
}

impl FixedIds {
    fn new(byte: u8) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let ids = Self {
            bytes: [byte; 16],
            calls: Arc::clone(&calls),
        };
        (ids, calls)
    }
}

impl IdSource for FixedIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.bytes)
    }
}

/// An identifier source that always fails with a fixed error and counts its calls.
#[derive(Debug)]
struct FailingIds {
    error: IdSourceError,
    calls: Arc<AtomicUsize>,
}

impl IdSource for FailingIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(self.error)
    }
}

/// An identifier source that returns distinct bytes on every call.
#[derive(Debug, Default)]
struct CounterIds(u64);

impl IdSource for CounterIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        self.0 += 1;
        let mut bytes = [0xA5u8; 16];
        if let Some(tail) = bytes.get_mut(8..) {
            tail.copy_from_slice(&self.0.to_be_bytes());
        }
        Ok(bytes)
    }
}

fn fixed_emitter() -> (Emitter<FixedClock, FixedIds>, Arc<AtomicUsize>) {
    let (ids, calls) = FixedIds::new(0x11);
    (Emitter::new(FixedClock, ids), calls)
}

fn emit_one(ev: AppEvent) -> EventEnvelope {
    let (mut emitter, _) = fixed_emitter();
    emitter.emit(ev).expect("event is emitted")
}

fn to_json(env: &EventEnvelope) -> String {
    serde_json::to_string(env).expect("envelope serialises")
}

/// Written independently of `AppEvent::event_type` so that a wrong name there is caught. Exhaustive with no
/// wildcard: a new `AppEvent` variant fails to compile here until it is added.
fn name_of(ev: &AppEvent) -> &'static str {
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

/// One sample of every `AppEvent` variant, in Architecture §6 order. Payload texts carry [`MARKER`] where
/// the variant has free text. `RunFinished` is `completed`, `RunStateChanged` is `running -> cancelling`.
fn all_events() -> Vec<AppEvent> {
    vec![
        AppEvent::RepositoryRegistered {
            repo_id: repo(),
            display_name: rt(&format!("name {MARKER}")),
        },
        AppEvent::RepositoryRemoved { repo_id: repo() },
        AppEvent::RepositoryStatusChanged {
            repo_id: repo(),
            operation: RepoOperation::Clone,
            status: RepoOperationStatus::Failed,
            reason: Some(rt(&format!("reason {MARKER}"))),
        },
        AppEvent::InventoryUpdated {
            repo_id: repo(),
            discovered_files: 12,
            excluded_paths: 3,
            discovery_errors: 1,
        },
        AppEvent::SelectionChanged {
            selected_repositories: 2,
            selected_files: 40,
        },
        AppEvent::WorkflowSaved {
            workflow_id: workflow(),
            revision: 4,
        },
        AppEvent::WorkflowDeleted {
            workflow_id: workflow(),
        },
        AppEvent::WorkflowStateChanged {
            workflow_id: workflow(),
            enabled: false,
        },
        AppEvent::TriggerFired {
            workflow_id: workflow(),
            fired_at: later(),
        },
        AppEvent::TriggerSkipped {
            workflow_id: workflow(),
            skipped_at: earlier_fraction(),
            reason: TriggerSkipReason::Overlap,
        },
        AppEvent::RunPlanned {
            run_id: run(),
            workflow_id: Some(workflow()),
            required_approval_ids: vec![approval()],
        },
        AppEvent::RunStarted {
            run_id: run(),
            started_at: later(),
        },
        AppEvent::RunStateChanged {
            run_id: run(),
            from: RunState::Running,
            to: RunState::Cancelling,
        },
        AppEvent::TaskStarted {
            run_id: run(),
            task_id: task(),
            repo_id: repo(),
            category: Category::Schema,
        },
        AppEvent::CheckCompleted {
            run_id: run(),
            task_id: task(),
            check_id: check(),
            outcome: CheckOutcome::Findings,
            findings: 5,
        },
        AppEvent::TaskProgress {
            run_id: run(),
            task_id: task(),
            completed_units: 7,
            total_units: Some(9),
        },
        AppEvent::FindingObserved {
            run_id: run(),
            check_id: check(),
            finding_id: finding(),
            repo_id: repo(),
            file_id: Some(file()),
            severity: Severity::High,
            category: Category::Secrets,
            title: rt(&format!("title {MARKER}")),
        },
        AppEvent::RunFinished {
            run_id: run(),
            state: RunState::Completed,
            assessment: Assessment::Findings,
            coverage: Coverage::Partial,
        },
        AppEvent::ApprovalRequested {
            approval_id: approval(),
            capability: Capability::ModelTransmission,
            purpose: rt(&format!("purpose {MARKER}")),
            workflow_id: Some(workflow()),
            workflow_revision: Some(2),
            repo_ids: vec![repo()],
            destinations: vec![rt(&format!("host.invalid {MARKER}"))],
            data_categories: vec![DataCategory::RedactedSource],
            lifetime: ApprovalLifetime::Once,
        },
        AppEvent::ApprovalResolved {
            approval_id: approval(),
            state: ApprovalState::Approved,
        },
        AppEvent::ApprovalRevoked {
            approval_id: approval(),
        },
        AppEvent::ReportWritten {
            report_id: report(),
            run_id: run(),
            repo_id: repo(),
            json: ExportStatus::Written,
            md: ExportStatus::Written,
            pdf: ExportStatus::NotRequested,
        },
        AppEvent::ExportStatusChanged {
            report_id: report(),
            format: ExportFormat::Markdown,
            status: ExportStatus::Failed,
            location: Some(rt(&format!("location {MARKER}"))),
        },
        AppEvent::DependencyStatusChanged {
            adapter_id: AdapterId::Gitleaks,
            availability: ToolAvailability::Missing,
        },
        AppEvent::CredentialStatusChanged {
            status: CredentialStatus::Stored,
        },
        AppEvent::ChatRetrievalCompleted {
            conversation_id: conversation(),
            source_ids: vec![source()],
        },
        AppEvent::ChatResponseStarted {
            conversation_id: conversation(),
            message_id: message(),
        },
        AppEvent::ChatResponseFinished {
            conversation_id: conversation(),
            message_id: message(),
            outcome: ChatOutcome::Completed,
        },
        AppEvent::ChatResponseChunk {
            conversation_id: conversation(),
            message_id: message(),
            text: rt(&format!("chunk {MARKER}")),
        },
        AppEvent::ChatFailed {
            conversation_id: conversation(),
            reason: ChatFailure::Timeout,
        },
        AppEvent::StorageFailure {
            detail: rt(&format!("storage detail {MARKER}")),
        },
        AppEvent::OperationFailed {
            code: FailureCode::Conflict,
            detail: rt(&format!("operation detail {MARKER}")),
        },
        AppEvent::SuppressionChanged,
        AppEvent::RunComparisonReady,
    ]
}

/// The two reserved variants (Architecture §6).
fn reserved_events() -> Vec<AppEvent> {
    vec![AppEvent::SuppressionChanged, AppEvent::RunComparisonReady]
}

/// Every sample the emitter accepts, in the order of [`all_events`].
fn pilot_events() -> Vec<AppEvent> {
    all_events()
        .into_iter()
        .filter(|e| name_of(e) != "SuppressionChanged" && name_of(e) != "RunComparisonReady")
        .collect()
}

/// The event names that must have no persisted form, written out literally: the five chat events (Data
/// Schema §11.24), the two non-must-deliver events and the two reserved events.
const NOT_PERSISTED: [&str; 9] = [
    "ChatRetrievalCompleted",
    "ChatResponseStarted",
    "ChatResponseFinished",
    "ChatResponseChunk",
    "ChatFailed",
    "SelectionChanged",
    "TaskProgress",
    "SuppressionChanged",
    "RunComparisonReady",
];

/// An envelope built by hand (the fields are public), so reserved events can be projected too.
fn manual_envelope(event: AppEvent) -> EventEnvelope {
    EventEnvelope {
        event_id: EventId::from_random_bytes([0x22; 16]),
        at: fixed_now(),
        level: EventLevel::Info,
        event,
    }
}

// ---------------------------------------------------------------------------------------------------------
// Sample completeness.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn samples_cover_every_event_name_exactly_once_in_schema_order() {
    // Fails if a sample is missing or duplicated, or if `AppEvent::event_type` disagrees with the
    // independently written `name_of` or with `EventType::ALL` (the persisted vocabulary).
    let events = all_events();
    assert_eq!(events.len(), 34);
    let names: Vec<&str> = events.iter().map(name_of).collect();
    let schema: Vec<&str> = EventType::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(names, schema);
    for ev in &events {
        assert_eq!(ev.event_type(), name_of(ev));
    }
    assert_eq!(names.iter().collect::<BTreeSet<_>>().len(), 34);
}

// ---------------------------------------------------------------------------------------------------------
// Envelope JSON.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn envelope_json_is_flat_and_exact_for_an_info_event() {
    // Fails if the envelope is nested (`"event": {...}`), if the field order or names change, or if the
    // payload is not flattened beside event_id, at and level (T01-D10).
    let env = emit_one(AppEvent::RunStarted {
        run_id: run(),
        started_at: later(),
    });
    assert_eq!(
        to_json(&env),
        format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\"info\",\
             \"event_type\":\"RunStarted\",\"run_id\":\"{U_RUN}\",\"started_at\":\"2026-10-28T12:00:01Z\"}}"
        )
    );
}

#[test]
fn envelope_json_is_exact_for_a_warning_event() {
    // Fails if TriggerSkipped is not level `warning`, or if a fractional payload timestamp or an enum
    // payload value is not carried flat.
    let env = emit_one(AppEvent::TriggerSkipped {
        workflow_id: workflow(),
        skipped_at: earlier_fraction(),
        reason: TriggerSkipReason::MissedWhileClosed,
    });
    assert_eq!(
        to_json(&env),
        format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\"warning\",\
             \"event_type\":\"TriggerSkipped\",\"workflow_id\":\"{U_WORKFLOW}\",\
             \"skipped_at\":\"2026-10-28T11:59:59.5Z\",\"reason\":\"missed_while_closed\"}}"
        )
    );
}

#[test]
fn envelope_json_is_exact_for_an_error_event() {
    // Fails if OperationFailed is not level `error`, or if `code` and `detail` are not carried flat.
    let env = emit_one(AppEvent::OperationFailed {
        code: FailureCode::InvalidInput,
        detail: rt("bad url"),
    });
    assert_eq!(
        to_json(&env),
        format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\"error\",\
             \"event_type\":\"OperationFailed\",\"code\":\"invalid_input\",\"detail\":\"bad url\"}}"
        )
    );
}

#[test]
fn envelope_json_places_optional_and_list_payloads_flat() {
    // Fails if `null` options or list payloads are dropped or nested, or if a RunFinished that failed does
    // not report `warning`.
    let env = emit_one(AppEvent::RunPlanned {
        run_id: run(),
        workflow_id: None,
        required_approval_ids: vec![approval()],
    });
    assert_eq!(
        to_json(&env),
        format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\"info\",\
             \"event_type\":\"RunPlanned\",\"run_id\":\"{U_RUN}\",\"workflow_id\":null,\
             \"required_approval_ids\":[\"{U_APPROVAL}\"]}}"
        )
    );
    let failed = emit_one(AppEvent::RunFinished {
        run_id: run(),
        state: RunState::Failed,
        assessment: Assessment::Unknown,
        coverage: Coverage::None,
    });
    assert_eq!(
        to_json(&failed),
        format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\"warning\",\
             \"event_type\":\"RunFinished\",\"run_id\":\"{U_RUN}\",\"state\":\"failed\",\
             \"assessment\":\"unknown\",\"coverage\":\"none\"}}"
        )
    );
}

#[test]
fn every_emitted_envelope_serialises_with_the_envelope_fields_first() {
    // Fails if any pilot event cannot be serialised after wrapping, or if the three envelope fields do
    // not lead the object in the order event_id, at, level followed by event_type.
    for ev in pilot_events() {
        let name = name_of(&ev);
        let json = to_json(&emit_one(ev));
        let prefix = format!(
            "{{\"event_id\":\"{FIXED_EVENT_ID}\",\"at\":\"2026-10-28T12:00:00Z\",\"level\":\""
        );
        assert!(json.starts_with(&prefix), "{name}: {json}");
        assert!(
            json.contains(&format!("\"event_type\":\"{name}\"")),
            "{name}: {json}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// Identifier and clock.
// ---------------------------------------------------------------------------------------------------------

/// Whether `id` is a canonical UUID with version nibble 4 and RFC 9562 variant `10`.
fn is_v4(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 36
        && bytes.get(14) == Some(&b'4')
        && matches!(bytes.get(19), Some(b'8' | b'9' | b'a' | b'b'))
}

#[test]
fn event_id_is_version_4_with_rfc_9562_variant_and_never_nil() {
    // Fails if the version/variant bits are not applied (all-zero input would then stay nil, all-0xFF
    // input would keep version f and variant f).
    for (byte, expected) in [
        (0x00u8, "00000000-0000-4000-8000-000000000000"),
        (0xFFu8, "ffffffff-ffff-4fff-bfff-ffffffffffff"),
        (0x11u8, FIXED_EVENT_ID),
    ] {
        let (ids, _) = FixedIds::new(byte);
        let mut emitter = Emitter::new(FixedClock, ids);
        let env = emitter
            .emit(AppEvent::RepositoryRemoved { repo_id: repo() })
            .expect("emitted");
        let text = env.event_id.to_string();
        assert_eq!(text, expected);
        assert!(is_v4(&text), "{text}");
        assert_ne!(text, "00000000-0000-0000-0000-000000000000");
    }
}

#[test]
fn event_ids_are_version_4_and_distinct_for_a_run_of_distinct_sources() {
    // Fails if two emissions share an identifier or any identifier of 500 is not v4.
    let mut emitter = Emitter::new(FixedClock, CounterIds::default());
    let mut seen = BTreeSet::new();
    for _ in 0..500 {
        let env = emitter
            .emit(AppEvent::RepositoryRemoved { repo_id: repo() })
            .expect("emitted");
        let text = env.event_id.to_string();
        assert!(is_v4(&text), "{text}");
        assert!(seen.insert(text));
    }
    assert_eq!(seen.len(), 500);
}

#[test]
fn at_is_the_clock_reading() {
    // Fails if `at` is taken from anywhere but the injected clock (for example `UtcTimestamp::now`).
    let env = emit_one(AppEvent::RepositoryRemoved { repo_id: repo() });
    assert_eq!(env.at, fixed_now());
}

#[test]
fn system_clock_reports_a_recent_instant() {
    // Fails if SystemClock returns a constant or an instant outside a loose window around the test run
    // (after 2026-01-01, before 9999).
    let now = SystemClock.now();
    let floor = UtcTimestamp::parse_rfc3339("2026-01-01T00:00:00Z").expect("timestamp");
    assert!(now.unix_seconds() >= floor.unix_seconds());
    assert!(now.unix_seconds() < 253_402_300_800);
}

// ---------------------------------------------------------------------------------------------------------
// Level table.
// ---------------------------------------------------------------------------------------------------------

/// The expected level of a pilot sample, written out literally (RunFinished sample is `completed`).
fn expected_level(name: &str) -> EventLevel {
    match name {
        "StorageFailure" | "OperationFailed" | "ChatFailed" => EventLevel::Error,
        "TriggerSkipped" => EventLevel::Warning,
        _ => EventLevel::Info,
    }
}

#[test]
fn level_table_holds_for_every_pilot_event() {
    // Fails if any pilot event has a level other than the CA-04 table: error for StorageFailure,
    // OperationFailed and ChatFailed; warning for TriggerSkipped; info for everything else; never debug.
    for ev in pilot_events() {
        let name = name_of(&ev);
        let env = emit_one(ev);
        assert_eq!(env.level, expected_level(name), "{name}");
        assert_ne!(env.level, EventLevel::Debug, "{name}");
    }
}

fn run_finished(state: RunState) -> AppEvent {
    AppEvent::RunFinished {
        run_id: run(),
        state,
        assessment: Assessment::NoFindings,
        coverage: Coverage::Complete,
    }
}

#[test]
fn run_finished_level_depends_on_the_terminal_state() {
    // Fails if failed, interrupted or partial runs are not `warning`, or if completed or cancelled runs
    // (the non-warning terminal states) are anything but `info`.
    for (state, level) in [
        (RunState::Failed, EventLevel::Warning),
        (RunState::Interrupted, EventLevel::Warning),
        (RunState::Partial, EventLevel::Warning),
        (RunState::Completed, EventLevel::Info),
        (RunState::Cancelled, EventLevel::Info),
    ] {
        assert!(state.is_terminal());
        assert_eq!(emit_one(run_finished(state)).level, level, "{state:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------
// Emission-time validation.
// ---------------------------------------------------------------------------------------------------------

fn expect_invalid(ev: AppEvent, reason: InvalidEventReason) {
    let (mut emitter, calls) = fixed_emitter();
    let name = name_of(&ev);
    let err = emitter.emit(ev).expect_err("event must be rejected");
    assert_eq!(err, EmitError::InvalidEvent { reason }, "{name}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "{name}: an identifier was consumed for a rejected event"
    );
}

#[test]
fn run_finished_with_a_non_terminal_state_is_rejected_without_consuming_an_id() {
    // Fails if any non-terminal RunState is accepted, or if the id source is advanced before validation.
    for state in RunState::ALL {
        if !state.is_terminal() {
            expect_invalid(
                run_finished(state),
                InvalidEventReason::RunFinishedNotTerminal,
            );
        }
    }
}

#[test]
fn check_completed_with_a_non_terminal_outcome_is_rejected() {
    // Fails if `pending` or `running` is accepted as a completed check outcome, or if a terminal outcome
    // is rejected.
    for outcome in CheckOutcome::ALL {
        let ev = AppEvent::CheckCompleted {
            run_id: run(),
            task_id: task(),
            check_id: check(),
            outcome,
            findings: 0,
        };
        if outcome.is_terminal() {
            assert_eq!(emit_one(ev).level, EventLevel::Info, "{outcome:?}");
        } else {
            expect_invalid(ev, InvalidEventReason::CheckCompletedNotTerminal);
        }
    }
}

#[test]
fn illegal_run_state_changes_are_rejected_and_legal_ones_accepted() {
    // Fails if any (from, to) pair disagrees with `RunState::can_transition`, including self-transitions
    // and transitions out of a terminal state.
    let mut rejected = 0usize;
    let mut accepted = 0usize;
    for from in RunState::ALL {
        for to in RunState::ALL {
            let ev = AppEvent::RunStateChanged {
                run_id: run(),
                from,
                to,
            };
            if from.can_transition(to) {
                assert_eq!(emit_one(ev).level, EventLevel::Info, "{from:?}->{to:?}");
                accepted += 1;
            } else {
                expect_invalid(ev, InvalidEventReason::IllegalRunTransition);
                rejected += 1;
            }
        }
    }
    assert_eq!(accepted + rejected, 81);
    assert!(accepted > 0 && rejected > 0, "both branches are exercised");
    // Two pairs pinned literally, independent of `can_transition`.
    assert!(
        emit_one(AppEvent::RunStateChanged {
            run_id: run(),
            from: RunState::Running,
            to: RunState::Cancelling,
        })
        .level
            == EventLevel::Info
    );
    expect_invalid(
        AppEvent::RunStateChanged {
            run_id: run(),
            from: RunState::Completed,
            to: RunState::Running,
        },
        InvalidEventReason::IllegalRunTransition,
    );
}

#[test]
fn reserved_events_are_rejected_without_consuming_an_id() {
    // Fails if SuppressionChanged or RunComparisonReady can be emitted.
    let reserved = reserved_events();
    assert_eq!(reserved.len(), 2);
    for ev in reserved {
        expect_invalid(ev, InvalidEventReason::ReservedEvent);
    }
}

// ---------------------------------------------------------------------------------------------------------
// Identifier source failures.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn id_source_failure_gives_id_source_error_and_no_envelope() {
    // Fails if a failing source yields an envelope, a different error variant, or a changed inner error.
    for error in [
        IdSourceError::Open {
            kind: ErrorKind::PermissionDenied,
        },
        IdSourceError::Read {
            kind: ErrorKind::Interrupted,
        },
        IdSourceError::ShortRead,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let ids = FailingIds {
            error,
            calls: Arc::clone(&calls),
        };
        let mut emitter = Emitter::new(FixedClock, ids);
        let result = emitter.emit(AppEvent::RepositoryRemoved { repo_id: repo() });
        assert_eq!(result, Err(EmitError::IdSource(error)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn urandom_ids_produce_sixteen_bytes_that_differ_between_calls() {
    // Smoke test against the real /dev/urandom. Fails if the read errors, or if 16 successive reads are
    // all equal (probability 2^-128 per pair for a working generator), or if the output is all zero.
    let mut ids = UrandomIds;
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        let bytes = ids.next_bytes().expect("urandom is readable");
        assert_eq!(bytes.len(), 16);
        assert_ne!(bytes, [0u8; 16]);
        seen.insert(bytes);
    }
    assert!(seen.len() > 1, "successive reads must differ");
}

#[test]
fn an_emitter_over_urandom_produces_distinct_v4_event_ids() {
    // Fails if the real source is not wired through `from_random_bytes` or repeats an identifier.
    let mut emitter = Emitter::new(SystemClock, UrandomIds);
    let a = emitter
        .emit(AppEvent::RepositoryRemoved { repo_id: repo() })
        .expect("emitted");
    let b = emitter
        .emit(AppEvent::RepositoryRemoved { repo_id: repo() })
        .expect("emitted");
    assert_ne!(a.event_id, b.event_id);
    assert!(is_v4(&a.event_id.to_string()));
    assert!(is_v4(&b.event_id.to_string()));
}

// ---------------------------------------------------------------------------------------------------------
// No payload text in errors.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn seeded_payload_text_never_reaches_an_error_message() {
    // Fails if EmitError or IdSourceError Display/Debug ever includes payload text. The marker is planted
    // in the events whose emission fails (failing id source) and the errors of every variant are rendered.
    let carriers = [
        AppEvent::OperationFailed {
            code: FailureCode::Internal,
            detail: rt(MARKER),
        },
        AppEvent::StorageFailure { detail: rt(MARKER) },
        AppEvent::RepositoryRegistered {
            repo_id: repo(),
            display_name: rt(MARKER),
        },
    ];
    let mut errors = Vec::new();
    for ev in carriers {
        let ids = FailingIds {
            error: IdSourceError::ShortRead,
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut emitter = Emitter::new(FixedClock, ids);
        errors.push(emitter.emit(ev).expect_err("id source fails"));
    }
    for reason in [
        InvalidEventReason::ReservedEvent,
        InvalidEventReason::RunFinishedNotTerminal,
        InvalidEventReason::CheckCompletedNotTerminal,
        InvalidEventReason::IllegalRunTransition,
    ] {
        errors.push(EmitError::InvalidEvent { reason });
    }
    for error in [
        IdSourceError::Open {
            kind: ErrorKind::NotFound,
        },
        IdSourceError::Read {
            kind: ErrorKind::Other,
        },
        IdSourceError::ShortRead,
    ] {
        let text = format!("{error} {error:?}");
        assert!(!text.contains(MARKER), "{text}");
        assert!(!text.contains("urandom"), "path leaked: {text}");
        errors.push(EmitError::IdSource(error));
    }
    assert_eq!(errors.len(), 10);
    for error in errors {
        let text = format!("{error} | {error:?}");
        assert!(!text.contains(MARKER), "{text}");
        assert!(!text.is_empty());
    }
}

#[test]
fn emit_error_source_chain_reaches_the_id_source_error() {
    // Fails if `source()` is missing for IdSource errors or present for validation errors.
    use std::error::Error as _;
    let inner = IdSourceError::ShortRead;
    let outer = EmitError::IdSource(inner);
    assert_eq!(
        outer.source().map(ToString::to_string),
        Some(inner.to_string())
    );
    let invalid = EmitError::InvalidEvent {
        reason: InvalidEventReason::ReservedEvent,
    };
    assert!(invalid.source().is_none());
}

// ---------------------------------------------------------------------------------------------------------
// Store mapping.
// ---------------------------------------------------------------------------------------------------------

/// The identifier slots and code of a persisted event as text, so that the value (not just presence) is
/// compared.
#[derive(Default, Debug, PartialEq)]
struct Slots {
    run: Option<String>,
    repo: Option<String>,
    workflow: Option<String>,
    task: Option<String>,
    approval: Option<String>,
    code: Option<String>,
}

fn slots_of(event: &Event) -> Slots {
    Slots {
        run: event.run_id.map(|i| i.to_string()),
        repo: event.repo_id.map(|i| i.to_string()),
        workflow: event.workflow_id.map(|i| i.to_string()),
        task: event.task_id.map(|i| i.to_string()),
        approval: event.approval_id.map(|i| i.to_string()),
        code: event.code.as_ref().map(|c| String::from(c.as_str())),
    }
}

fn expected_slots(name: &str) -> Slots {
    let repo = Some(String::from(U_REPO));
    let run = Some(String::from(U_RUN));
    let workflow = Some(String::from(U_WORKFLOW));
    let task = Some(String::from(U_TASK));
    let approval = Some(String::from(U_APPROVAL));
    match name {
        "RepositoryRegistered"
        | "RepositoryRemoved"
        | "RepositoryStatusChanged"
        | "InventoryUpdated" => Slots {
            repo,
            ..Slots::default()
        },
        "WorkflowSaved"
        | "WorkflowDeleted"
        | "WorkflowStateChanged"
        | "TriggerFired"
        | "TriggerSkipped" => Slots {
            workflow,
            ..Slots::default()
        },
        // The sample has `workflow_id: Some(..)`; the approval list has no single slot.
        "RunPlanned" => Slots {
            run,
            workflow,
            ..Slots::default()
        },
        "RunStarted" | "RunStateChanged" | "RunFinished" => Slots {
            run,
            ..Slots::default()
        },
        "TaskStarted" => Slots {
            run,
            task,
            repo,
            ..Slots::default()
        },
        "CheckCompleted" => Slots {
            run,
            task,
            ..Slots::default()
        },
        "FindingObserved" | "ReportWritten" => Slots {
            run,
            repo,
            ..Slots::default()
        },
        // The sample has `workflow_id: Some(..)`; `repo_ids` is a list and has no single slot.
        "ApprovalRequested" => Slots {
            approval,
            workflow,
            ..Slots::default()
        },
        "ApprovalResolved" | "ApprovalRevoked" => Slots {
            approval,
            ..Slots::default()
        },
        "OperationFailed" => Slots {
            code: Some(String::from("conflict")),
            ..Slots::default()
        },
        "ExportStatusChanged"
        | "DependencyStatusChanged"
        | "CredentialStatusChanged"
        | "StorageFailure" => Slots::default(),
        other => panic!("{other} is not a persisted sample"),
    }
}

#[test]
fn persisted_set_is_exactly_the_documented_set() {
    // Fails if any chat, latest-value/coalescible or reserved event has a store form, or if any other
    // event lacks one. The expected set is the literal NOT_PERSISTED list (9 of 34 names).
    let mut none: Vec<&str> = Vec::new();
    let mut some: Vec<&str> = Vec::new();
    for ev in all_events() {
        let name = name_of(&ev);
        let env = manual_envelope(ev);
        if to_store_event(&env).is_none() {
            none.push(name);
        } else {
            some.push(name);
        }
    }
    assert_eq!(none, {
        // Architecture §6 order.
        let mut ordered = NOT_PERSISTED.to_vec();
        let order: Vec<&str> = EventType::ALL.iter().map(|t| t.as_str()).collect();
        ordered.sort_by_key(|n| order.iter().position(|o| o == n));
        ordered
    });
    assert_eq!(some.len(), 25);
    assert_eq!(none.len() + some.len(), 34);
}

#[test]
fn non_persisted_events_are_those_that_are_chat_not_must_deliver_or_reserved() {
    // Fails if the rule of `to_store_event` is not "chat, or not MustDeliver, or reserved": the
    // classification is recomputed here from `delivery()` and `status()` plus a literal chat list.
    const CHAT: [&str; 5] = [
        "ChatRetrievalCompleted",
        "ChatResponseStarted",
        "ChatResponseFinished",
        "ChatResponseChunk",
        "ChatFailed",
    ];
    for ev in all_events() {
        let name = name_of(&ev);
        let must_deliver = ev.delivery() == DeliveryClass::MustDeliver;
        let reserved = reserved_events().iter().any(|r| name_of(r) == name);
        let expect_none = CHAT.contains(&name) || !must_deliver || reserved;
        assert_eq!(
            to_store_event(&manual_envelope(ev)).is_none(),
            expect_none,
            "{name}"
        );
        assert_eq!(expect_none, NOT_PERSISTED.contains(&name), "{name}");
    }
}

#[test]
fn store_event_carries_envelope_fields_event_type_and_identifier_slots() {
    // Fails if event_id, at, level or event_type are not carried over, if any identifier slot differs from
    // the literal table (including list-valued ids staying None), or if `code` is wrong.
    for ev in pilot_events() {
        let name = name_of(&ev);
        if NOT_PERSISTED.contains(&name) {
            continue;
        }
        let env = emit_one(ev);
        let stored = to_store_event(&env).unwrap_or_else(|| panic!("{name} has a store form"));
        assert_eq!(stored.event_id, env.event_id, "{name}");
        assert_eq!(stored.at, env.at, "{name}");
        assert_eq!(stored.level, env.level, "{name}");
        assert_eq!(stored.event_type.as_str(), name, "{name}");
        assert_eq!(slots_of(&stored), expected_slots(name), "{name}");
    }
}

#[test]
fn store_event_codes_follow_the_failure_code() {
    // Fails if `code` is not the serialised failure code for every FailureCode, or if a non-failure event
    // carries a code. `FailureCode` has six values.
    let expected = [
        (FailureCode::Deferred, "deferred"),
        (FailureCode::InvalidInput, "invalid_input"),
        (FailureCode::NotFound, "not_found"),
        (FailureCode::Conflict, "conflict"),
        (FailureCode::PermissionRequired, "permission_required"),
        (FailureCode::Internal, "internal"),
    ];
    for (code, wire) in expected {
        let env = emit_one(AppEvent::OperationFailed {
            code,
            detail: rt("failed"),
        });
        let stored = to_store_event(&env).expect("OperationFailed is persisted");
        assert_eq!(stored.code.as_ref().map(|c| c.as_str()), Some(wire));
        let serialised = serde_json::to_value(code).expect("serialises");
        assert_eq!(serialised, serde_json::Value::String(String::from(wire)));
    }
    let storage = emit_one(AppEvent::StorageFailure { detail: rt("x") });
    assert!(to_store_event(&storage).expect("persisted").code.is_none());
}

#[test]
fn store_message_is_the_detail_for_failures_and_a_fixed_summary_otherwise() {
    // Fails if StorageFailure/OperationFailed lose their detail, or if free payload text (display name,
    // reason, title, purpose, destinations, location) leaks into the persisted message of any other event.
    for ev in pilot_events() {
        let name = name_of(&ev);
        if NOT_PERSISTED.contains(&name) {
            continue;
        }
        let env = emit_one(ev);
        let stored = to_store_event(&env).unwrap_or_else(|| panic!("{name} has a store form"));
        let message = stored.message_redacted.as_str();
        match name {
            "StorageFailure" => assert_eq!(message, format!("storage detail {MARKER}")),
            "OperationFailed" => assert_eq!(message, format!("operation detail {MARKER}")),
            _ => {
                assert!(!message.contains(MARKER), "{name}: {message}");
                assert!(!message.is_empty(), "{name}");
            }
        }
    }
    // Control: the marker is present in the serialised envelope of a free-text event, so its absence in the
    // store record above is the mapping's doing and not an artefact of the sample.
    let registered = emit_one(all_events().remove(0));
    assert!(to_json(&registered).contains(MARKER));
    assert_eq!(
        to_store_event(&registered)
            .expect("persisted")
            .message_redacted
            .as_str(),
        "Repository registered"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Persisted form against the real validator.
// ---------------------------------------------------------------------------------------------------------

fn log_file(events: Vec<Event>) -> serde_json::Value {
    let file = LogFile {
        schema_version: SchemaVersion,
        store_type: LogStoreType,
        updated_at: fixed_now(),
        runs: Vec::new(),
        events,
        reports: Vec::new(),
    };
    serde_json::to_value(&file).expect("log file serialises")
}

fn persisted_store_events() -> Vec<Event> {
    let mut emitter = Emitter::new(FixedClock, CounterIds::default());
    pilot_events()
        .into_iter()
        .filter_map(|ev| {
            let env = emitter.emit(ev).expect("emitted");
            to_store_event(&env)
        })
        .collect()
}

#[test]
fn every_persisted_event_validates_through_the_real_schema_validator() {
    // Uses `SchemaValidator::new()` through `DocumentValidator::validate(StoreKind::Log, ..)`, which runs all
    // layers (precheck, JSON Schema, typed decode, semantic rules including D11-01 duplicate event ids,
    // D11-24 no chat events and D11-25 failure level). Fails if any projected event breaks the schema, the
    // typed record or a semantic rule.
    let events = persisted_store_events();
    assert_eq!(events.len(), 25);
    let validator = SchemaValidator::new();
    let doc = log_file(events);
    assert_eq!(validator.validate(StoreKind::Log, &doc), Verdict::Valid);
}

#[test]
fn each_persisted_event_validates_on_its_own() {
    // Fails if any single event only passed above because of its neighbours, and names the offender.
    let validator = SchemaValidator::new();
    for event in persisted_store_events() {
        let name = event.event_type.as_str();
        let doc = log_file(vec![event]);
        assert_eq!(
            validator.validate(StoreKind::Log, &doc),
            Verdict::Valid,
            "{name}"
        );
    }
}

fn rules_of(verdict: Verdict) -> Vec<&'static str> {
    match verdict {
        Verdict::Invalid(problems) => problems
            .items
            .iter()
            .filter(|p| p.layer == Layer::Semantic && p.code == ProblemCode::Rule)
            .filter_map(|p| p.rule.map(|r| r.0))
            .collect(),
        Verdict::Valid => Vec::new(),
        Verdict::SchemaUnavailable => panic!("embedded schema must be available"),
    }
}

#[test]
fn the_validator_rejects_what_the_mapping_never_produces() {
    // Negative controls: the validator is able to fail on exactly the three properties the mapping is meant
    // to guarantee. Fails if the validator accepts (a) a chat event in the log (D11-24), (b) a failure event
    // below level error (D11-25), (c) a duplicated event id (D11-01).
    let validator = SchemaValidator::new();
    let events = persisted_store_events();
    let template = events
        .iter()
        .find(|e| e.event_type == EventType::OperationFailed)
        .expect("OperationFailed is persisted")
        .clone();

    let mut chat = template.clone();
    chat.event_type = EventType::ChatFailed;
    chat.level = EventLevel::Error;
    chat.code = None;
    let verdict = validator.validate(StoreKind::Log, &log_file(vec![chat]));
    assert!(rules_of(verdict).contains(&"D11-24"));

    let mut quiet = template.clone();
    quiet.level = EventLevel::Info;
    let verdict = validator.validate(StoreKind::Log, &log_file(vec![quiet]));
    assert!(rules_of(verdict).contains(&"D11-25"));

    let verdict = validator.validate(StoreKind::Log, &log_file(vec![template.clone(), template]));
    assert!(rules_of(verdict).contains(&"D11-01"));
}

#[test]
fn emitted_failure_events_satisfy_the_failure_level_rule() {
    // Fails if an emitted StorageFailure or OperationFailed is not level `error` once persisted (D11-25),
    // checked through the real validator on the emitter's own output.
    let validator = SchemaValidator::new();
    let events: Vec<Event> = [
        AppEvent::StorageFailure { detail: rt("disk") },
        AppEvent::OperationFailed {
            code: FailureCode::Internal,
            detail: rt("op"),
        },
    ]
    .into_iter()
    .map(|ev| {
        let mut emitter = Emitter::new(FixedClock, CounterIds::default());
        let env = emitter.emit(ev).expect("emitted");
        assert_eq!(env.level, EventLevel::Error);
        to_store_event(&env).expect("persisted")
    })
    .collect();
    // Both events came from fresh counters and share one identifier: give the second a distinct one.
    let mut events = events;
    if let Some(second) = events.get_mut(1) {
        second.event_id = EventId::from_random_bytes([0x33; 16]);
    }
    assert_eq!(
        validator.validate(StoreKind::Log, &log_file(events)),
        Verdict::Valid
    );
}

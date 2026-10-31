//! P03-T08 tests: the approval lifecycle over the real `StoreWriter` (`planning/P03-T01-request.md` §5.4
//! including the 2026-10-31 amendment, §8 "T08").
//!
//! Every test drives `application::approvals` against a private directory under `CARGO_TARGET_TMPDIR`, with
//! the real `SchemaValidator` (every write is validated by the store, so a record or event that breaks the
//! schema or a semantic rule fails the test through the returned error). Expected values are written out
//! literally. Every test says what would make it fail. All data is synthetic.
//!
//! The fault-injection matrix for the consumption write of a `once` grant is in `p03_approvals_faults.rs`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};

use application::approvals::{
    self, Actor, ApprovalError, ApprovalService, AuthOutcome, RequestOutcome,
};
use application::dev_store::{self, DevError, FailureClass};
use application::emit::{Clock, Emitter, IdSource, IdSourceError};
use application::permission::{
    ArgName, NeedReason, NormalisedDestination, OperationArg, OperationName, PermissionRequest,
    RefusalReason, RequestedData, ScopeDimension, WorkflowBinding, fingerprint,
};
use domain::contract::{ApprovalDecision, ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{ApprovalId, RunId, WorkflowId};
use domain::states::ApprovalState;
use domain::store::approval::Approval;
use domain::store::log::Event;
use domain::store::provider::InferenceScope;
use domain::store::{AnyStore, StoreKind};
use domain::text::sanitise;
use domain::time::UtcTimestamp;
use domain::vocab::Capability;
use persistence::error::WriteStep;
use persistence::fs::{FileInfo, FsOps, LockHandle, RealFs};
use persistence::paths::DataDir;
use persistence::read::{SchemaValidator, decode};
use persistence::semantic::{RuleStatus, rule_table};
use persistence::validate::{DocumentValidator, Verdict};
use persistence::writer::StoreWriter;

pub(crate) const T0: i64 = 1_800_000_000;
/// The workflow revision the callers below report as the saved one.
const REV: u32 = 3;
/// The workflow of the CLI tests; its fixture file holds it at revision 1.
const FIXTURE_WORKFLOW: &str = "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d";
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../persistence/tests/fixtures/store"
);

// ---------------------------------------------------------------------------------------------------
// helpers

fn id<T>(parse: fn(&str) -> Result<T, domain::ids::IdError>, n: u32) -> T {
    parse(&format!("00000000-0000-4000-8000-{n:012x}")).expect("id")
}

fn repo(n: u32) -> domain::ids::RepoId {
    id(domain::ids::RepoId::parse, 0x100 + n)
}

fn workflow(n: u32) -> WorkflowId {
    id(WorkflowId::parse, 0x200 + n)
}

fn run(n: u32) -> RunId {
    id(RunId::parse, 0x300 + n)
}

fn ts(offset: i64) -> UtcTimestamp {
    UtcTimestamp::from_unix(T0 + offset, 0).expect("timestamp")
}

fn dest(s: &str) -> NormalisedDestination {
    NormalisedDestination::parse(s).expect("destination")
}

/// A shared, settable clock: [`Clk::set`] moves every clone.
#[derive(Debug, Clone)]
pub(crate) struct Clk(Arc<AtomicI64>);

impl Clk {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicI64::new(T0)))
    }

    /// A clock an hour before the real time, for tests that continue through the system clock (the CLI
    /// entry points): decisions made later by the real clock must not precede the requests.
    fn past() -> Self {
        Self(Arc::new(AtomicI64::new(
            UtcTimestamp::now().unix_seconds() - 3600,
        )))
    }

    /// Sets the time to `T0 + offset` seconds.
    pub(crate) fn set(&self, offset: i64) {
        self.0.store(T0 + offset, Ordering::SeqCst);
    }
}

impl Clock for Clk {
    fn now(&self) -> UtcTimestamp {
        UtcTimestamp::from_unix(self.0.load(Ordering::SeqCst), 0).expect("timestamp")
    }
}

/// Deterministic identifier bytes: `[prefix, counter (u32 big-endian), 0...]`.
#[derive(Debug)]
pub(crate) struct SeqIds {
    prefix: u8,
    next: u32,
}

impl SeqIds {
    pub(crate) fn new(prefix: u8) -> Self {
        Self { prefix, next: 1 }
    }
}

impl IdSource for SeqIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        let mut bytes = [0u8; 16];
        bytes[0] = self.prefix;
        bytes[1..5].copy_from_slice(&self.next.to_be_bytes());
        self.next += 1;
        Ok(bytes)
    }
}

/// An identifier source that always fails.
#[derive(Debug)]
struct DeadIds;

impl IdSource for DeadIds {
    fn next_bytes(&mut self) -> Result<[u8; 16], IdSourceError> {
        Err(IdSourceError::ShortRead)
    }
}

/// A private (0700) data directory removed on drop.
pub(crate) struct Tmp {
    pub(crate) root: PathBuf,
}

impl Tmp {
    pub(crate) fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p03appr-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let tmp = Self { root };
        assert_eq!(dev_store::init(&tmp.root).failure, None);
        tmp
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub(crate) fn bytes(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.path(name)).unwrap()
    }

    /// The validated, decoded approvals of the directory (read straight from the file).
    pub(crate) fn approvals(&self) -> Vec<Approval> {
        match decode(
            StoreKind::Approvals,
            &self.bytes("approvals.json"),
            &SchemaValidator::new(),
        )
        .expect("approvals.json is valid")
        {
            AnyStore::Approvals(file) => file.approvals,
            other => panic!("not approvals: {:?}", other.kind()),
        }
    }

    /// The validated, decoded persisted events of the directory.
    pub(crate) fn events(&self) -> Vec<Event> {
        match decode(
            StoreKind::Log,
            &self.bytes("log.json"),
            &SchemaValidator::new(),
        )
        .expect("log.json is valid")
        {
            AnyStore::Log(file) => file.events,
            other => panic!("not a log: {:?}", other.kind()),
        }
    }

    pub(crate) fn approval(&self, id: ApprovalId) -> Approval {
        self.approvals()
            .into_iter()
            .find(|a| a.approval_id == id)
            .expect("approval exists")
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The wire names of the persisted events, in order.
pub(crate) fn event_names(events: &[Event]) -> Vec<&'static str> {
    events.iter().map(|e| e.event_type.as_str()).collect()
}

/// One open writer with an emitter, clock and approval-ID source.
pub(crate) struct Env {
    pub(crate) writer: StoreWriter,
    emitter: Emitter<Clk, SeqIds>,
    pub(crate) clk: Clk,
    ids: SeqIds,
}

impl Env {
    /// Opens `tmp` over `fs`. `prefix` seeds the approval IDs (event IDs use `prefix + 0x80`), so
    /// that two openings of one directory never repeat an ID.
    pub(crate) fn open(tmp: &Tmp, fs: Arc<dyn FsOps>, clk: &Clk, prefix: u8) -> Self {
        let data = DataDir::new(&*fs, &tmp.root).unwrap();
        let (writer, _) = StoreWriter::open(data, fs, Arc::new(SchemaValidator::new())).unwrap();
        Self {
            writer,
            emitter: Emitter::new(clk.clone(), SeqIds::new(prefix.wrapping_add(0x80))),
            clk: clk.clone(),
            ids: SeqIds::new(prefix),
        }
    }

    pub(crate) fn real(tmp: &Tmp, clk: &Clk) -> Self {
        Self::open(tmp, Arc::new(RealFs), clk, 0x10)
    }

    pub(crate) fn svc(&mut self) -> ApprovalService<'_, Clk, SeqIds> {
        ApprovalService::new(
            &mut self.writer,
            &mut self.emitter,
            &self.clk,
            &mut self.ids,
        )
    }

    /// Requests `req` and expects a new or reused pending approval.
    pub(crate) fn pending(&mut self, req: &PermissionRequest, rev: Option<u32>) -> ApprovalId {
        match self.svc().request(req, rev).expect("request") {
            RequestOutcome::Pending(id) => id,
            other => panic!("expected a pending request, got {other:?}"),
        }
    }

    /// Requests and resolves `req`; the approval ID.
    pub(crate) fn grant(
        &mut self,
        req: &PermissionRequest,
        decision: ApprovalDecision,
        rev: Option<u32>,
    ) -> ApprovalId {
        let approval = self.pending(req, rev);
        let state = self
            .svc()
            .resolve(approval, decision, Actor::LocalUserCli, rev)
            .expect("resolve");
        assert_eq!(state, ApprovalState::Approved);
        approval
    }
}

fn binding(n: u32, revision: u32) -> Option<WorkflowBinding> {
    Some(WorkflowBinding {
        workflow_id: workflow(n),
        revision,
    })
}

/// A network request (`git_acquire`, one repository, one destination, one category) with the context a
/// `lifetime` needs: `run` gets run 1, `workflow_revision` gets workflow 1 at [`REV`].
pub(crate) fn net(lifetime: ApprovalLifetime) -> PermissionRequest {
    PermissionRequest {
        capability: Capability::GitAcquire,
        operation: OperationName::parse("git.clone").unwrap(),
        arguments: vec![OperationArg {
            name: ArgName::parse("branch").unwrap(),
            value: "main".to_owned(),
        }],
        workflow: (lifetime == ApprovalLifetime::WorkflowRevision)
            .then(|| binding(1, REV))
            .flatten(),
        run_id: (lifetime == ApprovalLifetime::Run).then(|| run(1)),
        repo_ids: vec![repo(1)],
        destinations: vec![dest("https://github.com/example/repo")],
        resource_constraints: vec![sanitise("max_size_mb=100")],
        data: vec![RequestedData::Category(DataCategory::RepositoryMetadata)],
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime,
        validity_seconds: None,
        inference_scope: None,
    }
}

fn scope() -> InferenceScope {
    serde_json::from_value(serde_json::json!({
        "provider_id": "openrouter",
        "model_id": "example/model",
        "routing_policy": {"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": ["openrouter"]},
        "source_ids": ["01234567-89ab-4cde-8f01-23456789abcd"],
        "redaction_version": "2026.10-a"
    }))
    .expect("inference scope")
}

/// Everything the lifecycle must leave alone when it writes nothing.
fn store_bytes(tmp: &Tmp) -> BTreeMap<&'static str, Vec<u8>> {
    ["approvals.json", "log.json", "workflows.json"]
        .into_iter()
        .map(|name| (name, tmp.bytes(name)))
        .collect()
}

fn needs(outcome: Result<AuthOutcome, ApprovalError>) -> (NeedReason, ApprovalId) {
    match outcome.expect("authorise") {
        AuthOutcome::NeedsApproval(reason, id) => (reason, id),
        other => panic!("expected NeedsApproval, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------
// Display

/// Fails if an `ApprovalError` text carries a record value (an ID, a reason payload, a store detail) or is
/// empty, or if two variants share a text.
#[test]
fn display_is_fixed_english_without_record_contents() {
    let marker = approval_marker();
    let errors = [
        ApprovalError::Store(DevError::Locked),
        ApprovalError::Store(DevError::Malformed(StoreKind::Approvals)),
        ApprovalError::NotFound,
        ApprovalError::NotPending,
        ApprovalError::NotRevocable,
        ApprovalError::WorkflowScopeUnavailable,
        ApprovalError::Refused(RefusalReason::PreviouslyDenied(marker.0)),
        ApprovalError::Fingerprint(application::permission::ArgError::Duplicate),
        ApprovalError::Emit,
    ];
    let mut seen = Vec::new();
    for error in errors {
        let text = error.to_string();
        assert!(!text.is_empty());
        assert!(!text.contains(&marker.1), "{error:?} leaks an ID");
        assert!(!text.contains("Duplicate") && !text.contains("PreviouslyDenied"));
        assert!(text.is_ascii());
        seen.push(text);
    }
    let unique: std::collections::BTreeSet<_> = seen.iter().collect();
    assert_eq!(unique.len(), seen.len(), "texts must differ: {seen:?}");
    assert_eq!(
        ApprovalError::NotPending.to_string(),
        "the approval is not pending"
    );
}

fn approval_marker() -> (ApprovalId, String) {
    let marker = id(ApprovalId::parse, 0xbeef);
    (marker, "00000000-0000-4000-8000-00000000beef".to_owned())
}

// ---------------------------------------------------------------------------------------------------
// request

/// Fails if a new request is not recorded exactly once with the pending-record shape of §5.4 (requester
/// actor, no decision time, not consumed, fingerprint of the request, store destination form), if its
/// event is missing or not the only one, or if the request gets another status.
#[test]
fn request_records_one_pending_approval_and_one_event() {
    let tmp = Tmp::new("req-new");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut req = net(ApprovalLifetime::WorkflowRevision);
    req.destinations = vec![
        dest("https://github.com/example/repo"),
        dest("https://GITHUB.com:443/example/repo"),
    ];
    let approval = env.pending(&req, Some(REV));

    let stored = tmp.approvals();
    assert_eq!(stored.len(), 1);
    let a = &stored[0];
    assert_eq!(a.approval_id, approval);
    assert_eq!(a.status, ApprovalState::Pending);
    assert_eq!(a.capability, Capability::GitAcquire);
    assert_eq!(a.workflow_id, Some(workflow(1)));
    assert_eq!(a.workflow_revision, Some(u64::from(REV)));
    assert_eq!(a.run_id, None);
    assert_eq!(a.repo_ids, vec![repo(1)]);
    assert_eq!(a.purpose.as_str(), "developer permission check");
    assert_eq!(a.operation_fingerprint, fingerprint(&req).unwrap());
    // Two spellings of one destination are stored once, in the store form.
    assert_eq!(a.destinations.len(), 1);
    assert_eq!(
        a.destinations[0],
        dest("https://github.com/example/repo").to_store()
    );
    assert_eq!(a.resource_constraints.len(), 1);
    assert_eq!(a.data_categories, vec![DataCategory::RepositoryMetadata]);
    assert_eq!(a.lifetime, ApprovalLifetime::WorkflowRevision);
    assert_eq!(a.requested_at, ts(0));
    assert_eq!(a.decided_at, None);
    assert_eq!(a.expires_at, None);
    assert_eq!(a.actor.as_str(), "requester");
    assert!(!a.consumed);
    assert_eq!(a.inference_scope, None);

    let events = tmp.events();
    assert_eq!(event_names(&events), vec!["ApprovalRequested"]);
    assert_eq!(events[0].approval_id, Some(approval));
    assert_eq!(events[0].workflow_id, Some(workflow(1)));
}

/// Fails if an identical pending request creates a second record or a second event, or answers anything but
/// the first request's ID (also from `authorise`).
#[test]
fn identical_pending_request_is_reused() {
    let tmp = Tmp::new("req-reuse");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Once);
    let first = env.pending(&req, None);
    let before = store_bytes(&tmp);
    clk.set(5);
    assert_eq!(env.pending(&req, None), first);
    let (reason, again) = needs(env.svc().authorise(&req, None));
    assert_eq!(again, first);
    assert_eq!(reason, NeedReason::AlreadyPending(first));
    assert_eq!(tmp.approvals().len(), 1);
    assert_eq!(event_names(&tmp.events()), vec!["ApprovalRequested"]);
    assert_eq!(store_bytes(&tmp), before, "a reused request writes nothing");
}

/// Fails if a request covered by a valid grant records anything, consumes a `once` grant, or answers other
/// than `Permitted`.
#[test]
fn request_covered_by_a_grant_is_permitted_and_writes_nothing() {
    let tmp = Tmp::new("req-permitted");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let once = net(ApprovalLifetime::Once);
    let granted = env.grant(&once, ApprovalDecision::ApproveOnce, None);
    let before = store_bytes(&tmp);
    assert_eq!(
        env.svc().request(&once, None).unwrap(),
        RequestOutcome::Permitted
    );
    assert_eq!(store_bytes(&tmp), before);
    assert!(!tmp.approval(granted).consumed, "request must not consume");
    // The grant is still there for the one authorisation.
    assert!(matches!(
        env.svc().authorise(&once, None).unwrap(),
        AuthOutcome::Authorised(_)
    ));
}

/// Fails if any refusal path of the evaluator writes a byte or emits an event, from `request` or from
/// `authorise`, or maps to another reason.
#[test]
fn every_refusal_reason_writes_nothing() {
    let tmp = Tmp::new("req-refused");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);

    // R7 needs a denial first.
    let denied_req = {
        let mut r = net(ApprovalLifetime::Once);
        r.operation = OperationName::parse("git.denied").unwrap();
        r
    };
    let denied = env.pending(&denied_req, None);
    env.svc()
        .resolve(denied, ApprovalDecision::Deny, Actor::LocalUserTui, None)
        .unwrap();
    let before = store_bytes(&tmp);

    let mut raw = net(ApprovalLifetime::Once);
    raw.data.push(RequestedData::RawSecretValues);
    let mut missing = net(ApprovalLifetime::Once);
    missing.destinations.clear();
    let mut unexpected = net(ApprovalLifetime::Once);
    unexpected.capability = Capability::LocalRender;
    let mut no_binding = net(ApprovalLifetime::Once);
    no_binding.lifetime = ApprovalLifetime::WorkflowRevision;
    let mut no_run = net(ApprovalLifetime::Once);
    no_run.lifetime = ApprovalLifetime::Run;
    let unknown_workflow = net(ApprovalLifetime::WorkflowRevision);
    let stale = net(ApprovalLifetime::WorkflowRevision);
    let mut bad_scope = net(ApprovalLifetime::Once);
    bad_scope.inference_scope = Some(scope());
    let mut bad_args = net(ApprovalLifetime::Once);
    bad_args.arguments.push(bad_args.arguments[0].clone());

    let cases: Vec<(&str, PermissionRequest, Option<u32>, RefusalReason)> = vec![
        ("R1", raw, None, RefusalReason::RawSecretRequested),
        (
            "R2 missing",
            missing,
            None,
            RefusalReason::MissingDestination,
        ),
        (
            "R2 unexpected",
            unexpected,
            None,
            RefusalReason::UnexpectedDestination,
        ),
        (
            "R3 revision",
            no_binding,
            None,
            RefusalReason::LifetimeBinding,
        ),
        ("R3 run", no_run, None, RefusalReason::LifetimeBinding),
        (
            "R4 unknown",
            unknown_workflow,
            None,
            RefusalReason::UnknownWorkflow,
        ),
        (
            "R4 stale",
            stale,
            Some(REV + 1),
            RefusalReason::StaleWorkflowRevision,
        ),
        ("R5", bad_scope, None, RefusalReason::InferenceScope),
        ("R6", bad_args, None, RefusalReason::InvalidRequest),
        (
            "R7",
            denied_req,
            None,
            RefusalReason::PreviouslyDenied(denied),
        ),
    ];
    for (name, req, rev, reason) in cases {
        assert_eq!(
            env.svc().request(&req, rev).unwrap(),
            RequestOutcome::Refused(reason),
            "request {name}"
        );
        assert_eq!(
            env.svc().authorise(&req, rev).unwrap(),
            AuthOutcome::Refused(reason),
            "authorise {name}"
        );
        assert_eq!(store_bytes(&tmp), before, "{name} wrote something");
    }
}

/// Fails if a deferred capability reaches the store (the store would reject it, D18-06a) or is refused with
/// another reason, from `request` or `authorise`. The requests are otherwise valid.
#[test]
fn deferred_capabilities_are_refused_before_any_write() {
    let tmp = Tmp::new("req-deferred");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let before = store_bytes(&tmp);
    for capability in [
        Capability::DocumentationFetch,
        Capability::EmbeddingTransmission,
        Capability::NonMutatingVerification,
    ] {
        let mut req = net(ApprovalLifetime::Once);
        req.capability = capability;
        assert_eq!(
            env.svc().request(&req, None).unwrap(),
            RequestOutcome::Refused(RefusalReason::InvalidRequest),
            "{capability:?}"
        );
        assert_eq!(
            env.svc().authorise(&req, None).unwrap(),
            AuthOutcome::Refused(RefusalReason::InvalidRequest),
            "{capability:?}"
        );
    }
    assert_eq!(store_bytes(&tmp), before);
    // The neighbouring non-deferred network capability is not refused.
    let mut allowed = net(ApprovalLifetime::Once);
    allowed.capability = Capability::SchemaFetch;
    assert!(matches!(
        env.svc().request(&allowed, None).unwrap(),
        RequestOutcome::Pending(_)
    ));
}

/// Fails if a `model_transmission` request without a data category (a record rule D11-16 rejects) is
/// recorded or ends in a store error, or if a complete one is not recorded with its inference scope.
#[test]
fn model_transmission_needs_a_category_and_keeps_its_scope() {
    let tmp = Tmp::new("req-model");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut req = net(ApprovalLifetime::Once);
    req.capability = Capability::ModelTransmission;
    req.inference_scope = Some(scope());
    req.data.clear();
    let before = store_bytes(&tmp);
    assert_eq!(
        env.svc().request(&req, None).unwrap(),
        RequestOutcome::Refused(RefusalReason::InvalidRequest)
    );
    assert_eq!(store_bytes(&tmp), before);

    req.data = vec![
        RequestedData::Category(DataCategory::RedactedFindings),
        RequestedData::Category(DataCategory::RedactedFindings),
    ];
    let approval = env.pending(&req, None);
    let stored = tmp.approval(approval);
    assert_eq!(stored.inference_scope, Some(scope()));
    assert_eq!(stored.data_categories, vec![DataCategory::RedactedFindings]);
}

/// Fails if the validity of a request is lost between request and decision (grant expiry must be
/// `decided_at + validity_seconds`, not `requested_at + ...`), or a request without validity gets an expiry.
#[test]
fn grant_expiry_is_decision_time_plus_validity() {
    let tmp = Tmp::new("validity");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut timed = net(ApprovalLifetime::Run);
    timed.validity_seconds = Some(3600);
    let timed_id = env.pending(&timed, None);
    let untimed = net(ApprovalLifetime::Once);
    let untimed_id = env.pending(&untimed, None);
    clk.set(100);
    env.svc()
        .resolve(
            timed_id,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None,
        )
        .unwrap();
    env.svc()
        .resolve(
            untimed_id,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None,
        )
        .unwrap();
    let timed_after = tmp.approval(timed_id);
    assert_eq!(timed_after.decided_at, Some(ts(100)));
    assert_eq!(timed_after.expires_at, Some(ts(3700)));
    assert_eq!(timed_after.requested_at, ts(0));
    assert_eq!(tmp.approval(untimed_id).expires_at, None);
}

/// Fails if a denial keeps the pending record's validity as an expiry.
#[test]
fn a_denial_carries_no_expiry() {
    let tmp = Tmp::new("deny-expiry");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut req = net(ApprovalLifetime::Once);
    req.validity_seconds = Some(60);
    let approval = env.pending(&req, None);
    assert_eq!(tmp.approval(approval).expires_at, Some(ts(60)));
    env.svc()
        .resolve(approval, ApprovalDecision::Deny, Actor::LocalUserCli, None)
        .unwrap();
    let after = tmp.approval(approval);
    assert_eq!(after.expires_at, None);
    assert_eq!(after.status, ApprovalState::Denied);
}

// ---------------------------------------------------------------------------------------------------
// resolve: the decision table

struct Row {
    name: &'static str,
    request: PermissionRequest,
    decision: ApprovalDecision,
    current: Option<u32>,
    /// `Ok((state, lifetime, run_id_is_some))` or the error.
    expect: Result<(ApprovalState, ApprovalLifetime, bool), ApprovalError>,
}

/// Fails if any row of the §5.4 decision table gives another state, lifetime or run binding, or if a
/// refusing row changes the store.
#[test]
fn decision_table_rows() {
    use ApprovalDecision::{ApproveOnce, ApproveWorkflowScope, Deny};
    use ApprovalLifetime::{Once, Run, WorkflowRevision};
    use ApprovalState::{Approved, Denied};
    let bound_once = {
        let mut r = net(Once);
        r.workflow = binding(1, REV);
        r
    };
    let bound_run = {
        let mut r = net(Run);
        r.workflow = binding(1, REV);
        r
    };
    let rows = vec![
        Row {
            name: "once/once",
            request: net(Once),
            decision: ApproveOnce,
            current: None,
            expect: Ok((Approved, Once, false)),
        },
        Row {
            name: "once/workflow_revision",
            request: net(WorkflowRevision),
            decision: ApproveOnce,
            current: Some(REV),
            expect: Ok((Approved, Once, false)),
        },
        Row {
            name: "once/run",
            request: net(Run),
            decision: ApproveOnce,
            current: None,
            expect: Ok((Approved, Run, true)),
        },
        Row {
            name: "scope/once with binding",
            request: bound_once.clone(),
            decision: ApproveWorkflowScope,
            current: Some(REV),
            expect: Ok((Approved, WorkflowRevision, false)),
        },
        Row {
            name: "scope/run with binding drops the run",
            request: bound_run.clone(),
            decision: ApproveWorkflowScope,
            current: Some(REV),
            expect: Ok((Approved, WorkflowRevision, false)),
        },
        Row {
            name: "scope/workflow_revision",
            request: net(WorkflowRevision),
            decision: ApproveWorkflowScope,
            current: Some(REV),
            expect: Ok((Approved, WorkflowRevision, false)),
        },
        Row {
            name: "scope/no binding",
            request: net(Once),
            decision: ApproveWorkflowScope,
            current: Some(REV),
            expect: Err(ApprovalError::WorkflowScopeUnavailable),
        },
        Row {
            name: "scope/no binding, run request",
            request: net(Run),
            decision: ApproveWorkflowScope,
            current: Some(REV),
            expect: Err(ApprovalError::WorkflowScopeUnavailable),
        },
        Row {
            name: "scope/stale revision",
            request: net(WorkflowRevision),
            decision: ApproveWorkflowScope,
            current: Some(REV + 1),
            expect: Err(ApprovalError::WorkflowScopeUnavailable),
        },
        Row {
            name: "scope/older revision",
            request: net(WorkflowRevision),
            decision: ApproveWorkflowScope,
            current: Some(REV - 1),
            expect: Err(ApprovalError::WorkflowScopeUnavailable),
        },
        Row {
            name: "scope/workflow not saved",
            request: net(WorkflowRevision),
            decision: ApproveWorkflowScope,
            current: None,
            expect: Err(ApprovalError::WorkflowScopeUnavailable),
        },
        Row {
            name: "deny/once",
            request: net(Once),
            decision: Deny,
            current: None,
            expect: Ok((Denied, Once, false)),
        },
        Row {
            name: "deny/run",
            request: net(Run),
            decision: Deny,
            current: None,
            expect: Ok((Denied, Run, true)),
        },
        Row {
            name: "deny/workflow_revision",
            request: net(WorkflowRevision),
            decision: Deny,
            current: Some(REV),
            expect: Ok((Denied, WorkflowRevision, false)),
        },
    ];
    for row in rows {
        let tmp = Tmp::new("table");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        // The request is made when the workflow is at REV, so it is accepted; the decision then sees `current`.
        let approval = env.pending(&row.request, Some(REV));
        let before = store_bytes(&tmp);
        clk.set(10);
        let got = env
            .svc()
            .resolve(approval, row.decision, Actor::LocalUserCli, row.current);
        match row.expect {
            Ok((state, lifetime, has_run)) => {
                assert_eq!(got, Ok(state), "{}", row.name);
                let after = tmp.approval(approval);
                assert_eq!(after.status, state, "{}", row.name);
                assert_eq!(after.lifetime, lifetime, "{}", row.name);
                assert_eq!(after.run_id.is_some(), has_run, "{}", row.name);
                assert_eq!(after.decided_at, Some(ts(10)), "{}", row.name);
                assert_eq!(after.actor.as_str(), "local-user/cli", "{}", row.name);
                assert!(!after.consumed, "{}", row.name);
                let names = event_names(&tmp.events());
                assert_eq!(names, vec!["ApprovalRequested", "ApprovalResolved"]);
                let resolved = &tmp.events()[1];
                assert_eq!(resolved.approval_id, Some(approval), "{}", row.name);
            }
            Err(error) => {
                assert_eq!(got, Err(error), "{}", row.name);
                assert_eq!(store_bytes(&tmp), before, "{} wrote something", row.name);
                assert_eq!(tmp.approval(approval).status, ApprovalState::Pending);
                // The request is still decidable.
                assert_eq!(
                    env.svc().resolve(
                        approval,
                        ApprovalDecision::Deny,
                        Actor::LocalUserCli,
                        row.current
                    ),
                    Ok(ApprovalState::Denied),
                    "{}",
                    row.name
                );
            }
        }
    }
}

/// Fails if `approve_workflow_scope` keeps a `run_id` (the amendment: stored null whatever the request
/// carried) or `approve_once` for a `run` request drops it.
#[test]
fn workflow_scope_stores_a_null_run_and_once_keeps_the_run() {
    let tmp = Tmp::new("scope-run");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut run_with_binding = net(ApprovalLifetime::Run);
    run_with_binding.workflow = binding(1, REV);
    let scoped = env.grant(
        &run_with_binding,
        ApprovalDecision::ApproveWorkflowScope,
        Some(REV),
    );
    assert_eq!(tmp.approval(scoped).run_id, None);
    let mut other = run_with_binding.clone();
    other.operation = OperationName::parse("git.fetch").unwrap();
    let once = env.grant(&other, ApprovalDecision::ApproveOnce, Some(REV));
    assert_eq!(tmp.approval(once).run_id, Some(run(1)));
    assert_eq!(tmp.approval(once).lifetime, ApprovalLifetime::Run);
}

/// Fails if the actor text does not follow the `Actor`, if a decision leaves the requester text, or if the
/// pending record is not still `requester`.
#[test]
fn actor_text_follows_the_actor() {
    let tmp = Tmp::new("actor");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut a = net(ApprovalLifetime::Once);
    a.operation = OperationName::parse("op.a").unwrap();
    let mut b = a.clone();
    b.operation = OperationName::parse("op.b").unwrap();
    let ida = env.pending(&a, None);
    let idb = env.pending(&b, None);
    assert_eq!(tmp.approval(ida).actor.as_str(), "requester");
    env.svc()
        .resolve(
            ida,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None,
        )
        .unwrap();
    env.svc()
        .resolve(
            idb,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserTui,
            None,
        )
        .unwrap();
    assert_eq!(tmp.approval(ida).actor.as_str(), "local-user/cli");
    assert_eq!(tmp.approval(idb).actor.as_str(), "local-user/tui");
    env.svc().revoke(idb, Actor::LocalUserCli).unwrap();
    assert_eq!(tmp.approval(idb).actor.as_str(), "local-user/cli");
}

/// Fails if resolve or revoke accept an unknown ID, a non-pending approval (resolve) or a non-approved one
/// (revoke), or write anything when they refuse.
#[test]
fn resolve_and_revoke_errors_write_nothing() {
    let tmp = Tmp::new("errors");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = |name: &str| {
        let mut r = net(ApprovalLifetime::Once);
        r.operation = OperationName::parse(name).unwrap();
        r
    };
    let pending = env.pending(&req("op.pending"), None);
    let denied = env.pending(&req("op.denied"), None);
    env.svc()
        .resolve(denied, ApprovalDecision::Deny, Actor::LocalUserCli, None)
        .unwrap();
    let approved = env.grant(&req("op.approved"), ApprovalDecision::ApproveOnce, None);
    let revoked = env.grant(&req("op.revoked"), ApprovalDecision::ApproveOnce, None);
    env.svc().revoke(revoked, Actor::LocalUserCli).unwrap();
    let mut timed = req("op.expired");
    timed.validity_seconds = Some(10);
    let expired = env.grant(&timed, ApprovalDecision::ApproveOnce, None);
    clk.set(100);
    assert_eq!(env.svc().expire_due().unwrap(), 1);
    assert_eq!(tmp.approval(expired).status, ApprovalState::Expired);

    let before = store_bytes(&tmp);
    let unknown = id(ApprovalId::parse, 0xdead);
    for decision in [
        ApprovalDecision::ApproveOnce,
        ApprovalDecision::ApproveWorkflowScope,
        ApprovalDecision::Deny,
    ] {
        assert_eq!(
            env.svc()
                .resolve(unknown, decision, Actor::LocalUserCli, Some(REV)),
            Err(ApprovalError::NotFound)
        );
        for done in [denied, approved, revoked, expired] {
            assert_eq!(
                env.svc()
                    .resolve(done, decision, Actor::LocalUserCli, Some(REV)),
                Err(ApprovalError::NotPending),
                "{done:?} {decision:?}"
            );
        }
    }
    assert_eq!(
        env.svc().revoke(unknown, Actor::LocalUserCli),
        Err(ApprovalError::NotFound)
    );
    for not_a_grant in [pending, denied, revoked, expired] {
        assert_eq!(
            env.svc().revoke(not_a_grant, Actor::LocalUserCli),
            Err(ApprovalError::NotRevocable),
            "{not_a_grant:?}"
        );
    }
    assert_eq!(store_bytes(&tmp), before);
    assert_eq!(tmp.approval(approved).status, ApprovalState::Approved);
}

/// Fails if `revoke` does not end an approved grant (it must then not authorise), keeps the decision time,
/// or its event is missing.
#[test]
fn revoke_ends_the_grant_and_emits() {
    let tmp = Tmp::new("revoke");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = {
        let mut r = net(ApprovalLifetime::Run);
        r.validity_seconds = Some(1000);
        r
    };
    let approval = env.grant(&req, ApprovalDecision::ApproveOnce, None);
    clk.set(20);
    env.svc().revoke(approval, Actor::LocalUserTui).unwrap();
    let after = tmp.approval(approval);
    assert_eq!(after.status, ApprovalState::Revoked);
    assert_eq!(after.decided_at, Some(ts(0)));
    assert_eq!(
        event_names(&tmp.events()),
        vec!["ApprovalRequested", "ApprovalResolved", "ApprovalRevoked"]
    );
    assert_eq!(tmp.events()[2].approval_id, Some(approval));
    // A revoked grant does not authorise; a new request is recorded.
    let (reason, fresh) = needs(env.svc().authorise(&req, None));
    assert_ne!(fresh, approval);
    assert_eq!(reason, NeedReason::NoGrant);
}

/// Fails if a pending request authorises, or if a denied one does.
#[test]
fn pending_and_denied_requests_never_authorise() {
    let tmp = Tmp::new("pending-never");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Run);
    let first = env.pending(&req, None);
    for _ in 0..3 {
        let (reason, id) = needs(env.svc().authorise(&req, None));
        assert_eq!(id, first);
        assert_eq!(reason, NeedReason::AlreadyPending(first));
    }
    env.svc()
        .resolve(first, ApprovalDecision::Deny, Actor::LocalUserCli, None)
        .unwrap();
    assert_eq!(
        env.svc().authorise(&req, None).unwrap(),
        AuthOutcome::Refused(RefusalReason::PreviouslyDenied(first))
    );
    assert_eq!(tmp.approvals().len(), 1);
}

// ---------------------------------------------------------------------------------------------------
// authorise: lifetimes

/// Fails if a `once` grant authorises twice, is not recorded consumed before `Authorised` is seen, or the
/// second ask does not record a new pending request with the consumption reason.
#[test]
fn a_once_grant_is_consumed_exactly_once() {
    let tmp = Tmp::new("once");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Once);
    let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
    assert!(!tmp.approval(granted).consumed);

    clk.set(7);
    let AuthOutcome::Authorised(auth) = env.svc().authorise(&req, None).unwrap() else {
        panic!("first authorise must succeed");
    };
    assert_eq!(auth.approval_id(), granted);
    assert_eq!(auth.lifetime(), ApprovalLifetime::Once);
    assert!(tmp.approval(granted).consumed);

    let (reason, fresh) = needs(env.svc().authorise(&req, None));
    assert_ne!(fresh, granted);
    assert_eq!(
        reason,
        NeedReason::Mismatch {
            approval_id: granted,
            dimension: ScopeDimension::Consumption
        }
    );
    assert_eq!(tmp.approval(fresh).status, ApprovalState::Pending);
    assert_eq!(tmp.approvals().len(), 2);
    // Asking again reuses the new pending request.
    let (again, same) = needs(env.svc().authorise(&req, None));
    assert_eq!(same, fresh);
    assert_eq!(again, NeedReason::AlreadyPending(fresh));
    // Resolving the new request yields one more single use.
    env.svc()
        .resolve(
            fresh,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None,
        )
        .unwrap();
    assert!(matches!(
        env.svc().authorise(&req, None).unwrap(),
        AuthOutcome::Authorised(_)
    ));
    assert!(matches!(
        env.svc().authorise(&req, None).unwrap(),
        AuthOutcome::NeedsApproval(..)
    ));
}

/// Fails if `run` and `workflow_revision` grants are consumed, authorise in another run, or the second run
/// is not asked again with a `Run` mismatch.
#[test]
fn a_run_grant_serves_its_run_repeatedly_and_no_other_run() {
    let tmp = Tmp::new("run");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Run);
    let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
    for _ in 0..3 {
        let AuthOutcome::Authorised(auth) = env.svc().authorise(&req, None).unwrap() else {
            panic!("a run grant authorises repeatedly");
        };
        assert_eq!(auth.approval_id(), granted);
        assert_eq!(auth.lifetime(), ApprovalLifetime::Run);
    }
    assert!(!tmp.approval(granted).consumed);

    let mut other = req.clone();
    other.run_id = Some(run(2));
    let (reason, fresh) = needs(env.svc().authorise(&other, None));
    assert_eq!(
        reason,
        NeedReason::Mismatch {
            approval_id: granted,
            dimension: ScopeDimension::Run
        }
    );
    assert_ne!(fresh, granted);
    assert_eq!(tmp.approval(fresh).run_id, Some(run(2)));
}

/// Fails if `expire_run` expires a grant of another run, of another lifetime or a non-approved one, miscounts,
/// leaves the grant usable, or writes when nothing matches.
#[test]
fn expire_run_ends_only_that_runs_grants() {
    let tmp = Tmp::new("expire-run");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let run1 = net(ApprovalLifetime::Run);
    let mut run1_second = run1.clone();
    run1_second.operation = OperationName::parse("git.fetch").unwrap();
    let mut run2 = run1.clone();
    run2.run_id = Some(run(2));
    let mut once = net(ApprovalLifetime::Once);
    once.operation = OperationName::parse("op.once").unwrap();
    let mut scoped = net(ApprovalLifetime::WorkflowRevision);
    scoped.operation = OperationName::parse("op.scoped").unwrap();
    let g1 = env.grant(&run1, ApprovalDecision::ApproveOnce, None);
    let g1b = env.grant(&run1_second, ApprovalDecision::ApproveOnce, None);
    let g2 = env.grant(&run2, ApprovalDecision::ApproveOnce, None);
    let g_once = env.grant(&once, ApprovalDecision::ApproveOnce, None);
    let g_scope = env.grant(&scoped, ApprovalDecision::ApproveWorkflowScope, Some(REV));
    let revoked = {
        let mut r = run1.clone();
        r.operation = OperationName::parse("git.old").unwrap();
        let id = env.grant(&r, ApprovalDecision::ApproveOnce, None);
        env.svc().revoke(id, Actor::LocalUserCli).unwrap();
        id
    };

    clk.set(5);
    assert_eq!(env.svc().expire_run(run(1)).unwrap(), 2);
    for ended in [g1, g1b] {
        assert_eq!(tmp.approval(ended).status, ApprovalState::Expired);
    }
    for untouched in [g2, g_once, g_scope] {
        assert_eq!(tmp.approval(untouched).status, ApprovalState::Approved);
    }
    assert_eq!(tmp.approval(revoked).status, ApprovalState::Revoked);
    // The only approved grant with that fingerprint belongs to run 2: asked again, with the run as reason.
    let (reason, _) = needs(env.svc().authorise(&run1, None));
    assert_eq!(
        reason,
        NeedReason::Mismatch {
            approval_id: g2,
            dimension: ScopeDimension::Run
        }
    );
    assert!(matches!(
        env.svc().authorise(&run2, None).unwrap(),
        AuthOutcome::Authorised(_)
    ));
    // Expiry emits no event; the only events are the requests and decisions.
    assert!(
        event_names(&tmp.events()).iter().all(|n| [
            "ApprovalRequested",
            "ApprovalResolved",
            "ApprovalRevoked"
        ]
        .contains(n))
    );
    let events_before = tmp.events().len();
    let before = store_bytes(&tmp);
    assert_eq!(env.svc().expire_run(run(1)).unwrap(), 0);
    assert_eq!(env.svc().expire_run(run(9)).unwrap(), 0);
    assert_eq!(store_bytes(&tmp), before, "no match, no write");
    assert_eq!(tmp.events().len(), events_before);
}

/// Fails if a workflow-scope grant is consumed, is tied to a run, does not serve two different runs at its
/// revision, or survives a revision change.
#[test]
fn a_workflow_scope_grant_serves_every_run_at_its_revision_only() {
    let tmp = Tmp::new("scope");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut with_run1 = net(ApprovalLifetime::WorkflowRevision);
    with_run1.run_id = Some(run(1));
    let granted = env.grant(
        &with_run1,
        ApprovalDecision::ApproveWorkflowScope,
        Some(REV),
    );
    let stored = tmp.approval(granted);
    assert_eq!(stored.run_id, None);
    assert_eq!(stored.lifetime, ApprovalLifetime::WorkflowRevision);
    assert_eq!(stored.workflow_revision, Some(u64::from(REV)));

    // Two different runs, and one without a run, are all served; nothing is consumed.
    for run_id in [Some(run(1)), Some(run(2)), None] {
        let mut req = with_run1.clone();
        req.run_id = run_id;
        let AuthOutcome::Authorised(auth) = env.svc().authorise(&req, Some(REV)).unwrap() else {
            panic!("run {run_id:?} must be served");
        };
        assert_eq!(auth.approval_id(), granted);
        assert_eq!(auth.lifetime(), ApprovalLifetime::WorkflowRevision);
    }
    assert!(!tmp.approval(granted).consumed);

    // The workflow moves to revision 4. A request still bound to 3 is refused (rule 14).
    assert_eq!(
        env.svc().authorise(&with_run1, Some(REV + 1)).unwrap(),
        AuthOutcome::Refused(RefusalReason::StaleWorkflowRevision)
    );
    // A request bound to revision 4 is asked again, with the revision as the reason.
    let mut rebound = with_run1.clone();
    rebound.workflow = binding(1, REV + 1);
    let (reason, fresh) = needs(env.svc().authorise(&rebound, Some(REV + 1)));
    assert_eq!(
        reason,
        NeedReason::Mismatch {
            approval_id: granted,
            dimension: ScopeDimension::WorkflowRevision
        }
    );
    assert_eq!(
        tmp.approval(fresh).workflow_revision,
        Some(u64::from(REV + 1))
    );
    assert_eq!(tmp.approval(granted).status, ApprovalState::Approved);
}

// ---------------------------------------------------------------------------------------------------
// expire_due

/// Fails if `expire_due` expires a future, null-expiry, pending, denied or revoked record, misses a past or
/// equal one, miscounts, writes when nothing is due, or lets an expired grant authorise.
#[test]
fn expire_due_ends_only_approved_grants_past_their_time() {
    let tmp = Tmp::new("due");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let op = |name: &str, validity: Option<u32>| {
        let mut r = net(ApprovalLifetime::Run);
        r.operation = OperationName::parse(name).unwrap();
        r.validity_seconds = validity;
        r
    };
    let past = env.grant(
        &op("op.past", Some(10)),
        ApprovalDecision::ApproveOnce,
        None,
    );
    let equal = env.grant(
        &op("op.equal", Some(50)),
        ApprovalDecision::ApproveOnce,
        None,
    );
    let future = env.grant(
        &op("op.future", Some(500)),
        ApprovalDecision::ApproveOnce,
        None,
    );
    let forever = env.grant(&op("op.forever", None), ApprovalDecision::ApproveOnce, None);
    // A pending request keeps its validity in `expires_at`; a late clock must not touch it.
    let pending = env.pending(&op("op.pending", Some(1)), None);
    let denied = env.pending(&op("op.denied", Some(1)), None);
    env.svc()
        .resolve(denied, ApprovalDecision::Deny, Actor::LocalUserCli, None)
        .unwrap();
    let revoked = env.grant(
        &op("op.revoked", Some(1)),
        ApprovalDecision::ApproveOnce,
        None,
    );
    env.svc().revoke(revoked, Actor::LocalUserCli).unwrap();

    clk.set(50);
    let events_before = tmp.events().len();
    assert_eq!(env.svc().expire_due().unwrap(), 2, "past and equal");
    assert_eq!(tmp.events().len(), events_before, "expiry emits no event");
    for (id, state) in [
        (past, ApprovalState::Expired),
        (equal, ApprovalState::Expired),
        (future, ApprovalState::Approved),
        (forever, ApprovalState::Approved),
        (pending, ApprovalState::Pending),
        (denied, ApprovalState::Denied),
        (revoked, ApprovalState::Revoked),
    ] {
        assert_eq!(tmp.approval(id).status, state, "{id:?}");
    }
    // An expired grant never authorises.
    let (reason, _) = needs(env.svc().authorise(&op("op.past", Some(10)), None));
    assert!(
        matches!(
            reason,
            NeedReason::Mismatch {
                dimension: ScopeDimension::Fingerprint,
                ..
            }
        ),
        "{reason:?}"
    );
    // Nothing more is due: no count, no write.
    let before = store_bytes(&tmp);
    assert_eq!(env.svc().expire_due().unwrap(), 0);
    assert_eq!(store_bytes(&tmp), before);
    // Later, the future grant goes too.
    clk.set(600);
    assert_eq!(env.svc().expire_due().unwrap(), 1);
    assert_eq!(tmp.approval(future).status, ApprovalState::Expired);
    assert_eq!(tmp.approval(forever).status, ApprovalState::Approved);
}

/// Fails if a grant authorises at or after its `expires_at` before `expire_due` has run (the evaluator's
/// G13 must hold through the service as well).
#[test]
fn a_grant_past_its_time_does_not_authorise_even_before_expire_due() {
    let tmp = Tmp::new("late");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut req = net(ApprovalLifetime::Run);
    req.validity_seconds = Some(30);
    env.grant(&req, ApprovalDecision::ApproveOnce, None);
    clk.set(29);
    assert!(matches!(
        env.svc().authorise(&req, None).unwrap(),
        AuthOutcome::Authorised(_)
    ));
    clk.set(30);
    let (reason, _) = needs(env.svc().authorise(&req, None));
    assert!(matches!(
        reason,
        NeedReason::Mismatch {
            dimension: ScopeDimension::Expiry,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------------------------------
// failures

/// An identifier source failure for the approval ID is `Emit`, and nothing is written.
/// Fails if a request without an ID creates a record.
#[test]
fn an_identifier_failure_writes_nothing() {
    let tmp = Tmp::new("id-fail");
    let clk = Clk::new();
    let data = DataDir::new(&RealFs, &tmp.root).unwrap();
    let (mut writer, _) =
        StoreWriter::open(data, Arc::new(RealFs), Arc::new(SchemaValidator::new())).unwrap();
    let mut emitter = Emitter::new(clk.clone(), SeqIds::new(0x90));
    let mut ids = DeadIds;
    let before = store_bytes(&tmp);
    let outcome = ApprovalService::new(&mut writer, &mut emitter, &clk, &mut ids)
        .request(&net(ApprovalLifetime::Once), None);
    assert_eq!(outcome, Err(ApprovalError::Emit));
    assert_eq!(store_bytes(&tmp), before);
}

/// An event-ID failure after the approvals write: the error is `Emit` and the approval stays. Fails if the
/// approval is rolled back or the failure is swallowed.
#[test]
fn an_event_failure_keeps_the_approval_state_already_written() {
    let tmp = Tmp::new("event-fail");
    let clk = Clk::new();
    let data = DataDir::new(&RealFs, &tmp.root).unwrap();
    let (mut writer, _) =
        StoreWriter::open(data, Arc::new(RealFs), Arc::new(SchemaValidator::new())).unwrap();
    let mut emitter = Emitter::new(clk.clone(), DeadIds);
    let mut ids = SeqIds::new(0x20);
    let outcome = ApprovalService::new(&mut writer, &mut emitter, &clk, &mut ids)
        .request(&net(ApprovalLifetime::Once), None);
    assert_eq!(outcome, Err(ApprovalError::Emit));
    assert!(!writer.is_halted());
    drop(writer);
    let stored = tmp.approvals();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, ApprovalState::Pending);
    assert!(tmp.events().is_empty());
}

/// Fails if a malformed `approvals.json` is read as an empty ledger or overwritten (the request must fail
/// closed with `Store(Malformed)` and leave the file as it was).
#[test]
fn a_malformed_ledger_fails_closed() {
    let tmp = Tmp::new("malformed");
    std::fs::write(tmp.path("approvals.json"), b"{ not json").unwrap();
    std::fs::set_permissions(
        tmp.path("approvals.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let outcome = env.svc().request(&net(ApprovalLifetime::Once), None);
    assert_eq!(
        outcome,
        Err(ApprovalError::Store(DevError::Malformed(
            StoreKind::Approvals
        )))
    );
    assert_eq!(
        env.svc().authorise(&net(ApprovalLifetime::Once), None),
        Err(ApprovalError::Store(DevError::Malformed(
            StoreKind::Approvals
        )))
    );
    assert_eq!(tmp.bytes("approvals.json"), b"{ not json");
}

// ---------------------------------------------------------------------------------------------------
// CLI entry points

fn seed_workflows(tmp: &Tmp, revision: u64) {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join("data-s9-workflows.json")).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    doc["workflows"][0]["revision"] = serde_json::json!(revision);
    let path = tmp.path("workflows.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn fixture_workflow() -> WorkflowId {
    WorkflowId::parse(FIXTURE_WORKFLOW).unwrap()
}

fn bound_to_fixture(revision: u32, name: &str) -> PermissionRequest {
    let mut r = net(ApprovalLifetime::WorkflowRevision);
    r.operation = OperationName::parse(name).unwrap();
    r.workflow = Some(WorkflowBinding {
        workflow_id: fixture_workflow(),
        revision,
    });
    r
}

/// Fails if `list` reorders, drops or invents approvals, writes, or fails on an empty store.
#[test]
fn cli_list_returns_store_order_and_writes_nothing() {
    let tmp = Tmp::new("cli-list");
    assert_eq!(approvals::list(&tmp.root).unwrap(), Vec::new());
    let clk = Clk::past();
    let ids: Vec<ApprovalId> = {
        let mut env = Env::real(&tmp, &clk);
        ["op.c", "op.a", "op.b"]
            .into_iter()
            .map(|name| {
                let mut r = net(ApprovalLifetime::Once);
                r.operation = OperationName::parse(name).unwrap();
                env.pending(&r, None)
            })
            .collect()
    };
    let before = store_bytes(&tmp);
    let listed = approvals::list(&tmp.root).unwrap();
    assert_eq!(
        listed.iter().map(|a| a.approval_id).collect::<Vec<_>>(),
        ids
    );
    assert!(listed.iter().all(|a| a.status == ApprovalState::Pending));
    assert_eq!(store_bytes(&tmp), before);
}

/// Fails if the CLI `resolve` ignores the workflow's saved revision (scope must follow `workflows.json`), maps
/// a missing workflow to a grant, or records the wrong actor or event.
#[test]
fn cli_resolve_reads_the_saved_workflow_revision() {
    let tmp = Tmp::new("cli-resolve");
    seed_workflows(&tmp, 1);
    let clk = Clk::past();
    let (at_one, at_one_again, unbound, once) = {
        let mut env = Env::real(&tmp, &clk);
        (
            env.pending(&bound_to_fixture(1, "op.one"), Some(1)),
            env.pending(&bound_to_fixture(1, "op.two"), Some(1)),
            env.pending(&net(ApprovalLifetime::Once), None),
            env.pending(&bound_to_fixture(1, "op.three"), Some(1)),
        )
    };
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            at_one,
            ApprovalDecision::ApproveWorkflowScope,
            Actor::LocalUserCli
        ),
        Ok(ApprovalState::Approved)
    );
    let granted = tmp.approval(at_one);
    assert_eq!(granted.lifetime, ApprovalLifetime::WorkflowRevision);
    assert_eq!(granted.actor.as_str(), "local-user/cli");
    // No workflow id on the approval: scope unavailable.
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            unbound,
            ApprovalDecision::ApproveWorkflowScope,
            Actor::LocalUserTui
        ),
        Err(ApprovalError::WorkflowScopeUnavailable)
    );
    // The workflow moves to revision 2: the request for revision 1 can no longer be scoped.
    seed_workflows(&tmp, 2);
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            at_one_again,
            ApprovalDecision::ApproveWorkflowScope,
            Actor::LocalUserCli
        ),
        Err(ApprovalError::WorkflowScopeUnavailable)
    );
    assert_eq!(tmp.approval(at_one_again).status, ApprovalState::Pending);
    // A once decision does not need the revision.
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            once,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserTui
        ),
        Ok(ApprovalState::Approved)
    );
    assert_eq!(tmp.approval(once).actor.as_str(), "local-user/tui");
    // The workflow disappears: scope unavailable again (absent workflow gives no revision).
    let empty = {
        let text = std::fs::read_to_string(tmp.path("workflows.json")).unwrap();
        let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        doc["workflows"] = serde_json::json!([]);
        doc
    };
    std::fs::write(
        tmp.path("workflows.json"),
        serde_json::to_vec_pretty(&empty).unwrap(),
    )
    .unwrap();
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            at_one_again,
            ApprovalDecision::ApproveWorkflowScope,
            Actor::LocalUserCli
        ),
        Err(ApprovalError::WorkflowScopeUnavailable)
    );
    let names = event_names(&tmp.events());
    assert_eq!(
        names.iter().filter(|n| **n == "ApprovalResolved").count(),
        2
    );
}

/// Fails if the CLI `revoke` or `resolve` mis-classify errors: unknown ID, a grant that is not revocable, a
/// missing directory, a locked store, a malformed ledger.
#[test]
fn cli_errors_are_typed_and_classified() {
    let tmp = Tmp::new("cli-errors");
    let clk = Clk::past();
    let pending = {
        let mut env = Env::real(&tmp, &clk);
        env.pending(&net(ApprovalLifetime::Once), None)
    };
    let unknown = id(ApprovalId::parse, 0xdead);
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            unknown,
            ApprovalDecision::Deny,
            Actor::LocalUserCli
        ),
        Err(ApprovalError::NotFound)
    );
    assert_eq!(
        approvals::revoke(&tmp.root, unknown, Actor::LocalUserCli),
        Err(ApprovalError::NotFound)
    );
    assert_eq!(
        approvals::revoke(&tmp.root, pending, Actor::LocalUserCli),
        Err(ApprovalError::NotRevocable)
    );
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            pending,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli
        ),
        Ok(ApprovalState::Approved)
    );
    assert_eq!(
        approvals::resolve(
            &tmp.root,
            pending,
            ApprovalDecision::Deny,
            Actor::LocalUserCli
        ),
        Err(ApprovalError::NotPending)
    );
    assert_eq!(
        approvals::revoke(&tmp.root, pending, Actor::LocalUserTui),
        Ok(())
    );
    assert_eq!(tmp.approval(pending).status, ApprovalState::Revoked);
    assert_eq!(
        event_names(&tmp.events()),
        vec!["ApprovalRequested", "ApprovalResolved", "ApprovalRevoked"]
    );

    // A directory that does not exist.
    let missing = tmp.path("no-such-dir");
    let error = approvals::list(&missing).unwrap_err();
    assert_eq!(error, ApprovalError::Store(DevError::DataDirRejected));
    assert_eq!(
        approvals::revoke(&missing, pending, Actor::LocalUserCli),
        Err(ApprovalError::Store(DevError::DataDirRejected))
    );
    assert_eq!(
        dev_error(&error).class(),
        FailureClass::Operation,
        "a rejected directory is an operation failure"
    );

    // A store locked by another open writer.
    let held = Env::real(&tmp, &clk);
    for outcome in [
        approvals::list(&tmp.root).map(|_| ()),
        approvals::revoke(&tmp.root, pending, Actor::LocalUserCli),
        approvals::resolve(
            &tmp.root,
            pending,
            ApprovalDecision::Deny,
            Actor::LocalUserCli,
        )
        .map(|_| ()),
    ] {
        assert_eq!(outcome, Err(ApprovalError::Store(DevError::Locked)));
    }
    drop(held);

    // A malformed ledger.
    std::fs::write(tmp.path("approvals.json"), b"[]").unwrap();
    assert_eq!(
        approvals::list(&tmp.root),
        Err(ApprovalError::Store(DevError::Malformed(
            StoreKind::Approvals
        )))
    );
    assert_eq!(
        approvals::revoke(&tmp.root, pending, Actor::LocalUserCli),
        Err(ApprovalError::Store(DevError::Malformed(
            StoreKind::Approvals
        )))
    );
    assert_eq!(tmp.bytes("approvals.json"), b"[]");
}

fn dev_error(error: &ApprovalError) -> DevError {
    match error {
        ApprovalError::Store(e) => *e,
        other => panic!("not a store error: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------
// events

/// Fails if the persisted events of a full lifecycle are not exactly the three approval events in order, or
/// if the log with them stops validating against the real schema and semantic rules.
#[test]
fn lifecycle_events_validate_against_the_real_schema() {
    let tmp = Tmp::new("events");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::WorkflowRevision);
    let approval = env.grant(&req, ApprovalDecision::ApproveWorkflowScope, Some(REV));
    env.svc().revoke(approval, Actor::LocalUserCli).unwrap();
    let denied = {
        let mut r = net(ApprovalLifetime::Once);
        r.operation = OperationName::parse("op.other").unwrap();
        let id = env.pending(&r, None);
        env.svc()
            .resolve(id, ApprovalDecision::Deny, Actor::LocalUserTui, None)
            .unwrap();
        id
    };
    let events = tmp.events();
    assert_eq!(
        event_names(&events),
        vec![
            "ApprovalRequested",
            "ApprovalResolved",
            "ApprovalRevoked",
            "ApprovalRequested",
            "ApprovalResolved"
        ]
    );
    assert_eq!(
        events.iter().map(|e| e.approval_id).collect::<Vec<_>>(),
        vec![
            Some(approval),
            Some(approval),
            Some(approval),
            Some(denied),
            Some(denied)
        ]
    );
    // The messages are fixed texts: no purpose, no destination, no operation.
    for event in &events {
        let text = event.message_redacted.as_str();
        assert!(!text.contains("github"), "{text}");
        assert!(!text.contains("developer permission check"), "{text}");
    }
    let log: serde_json::Value = serde_json::from_slice(&tmp.bytes("log.json")).unwrap();
    assert_eq!(
        SchemaValidator::new().validate(StoreKind::Log, &log),
        Verdict::Valid
    );
    let approvals_doc: serde_json::Value =
        serde_json::from_slice(&tmp.bytes("approvals.json")).unwrap();
    assert_eq!(
        SchemaValidator::new().validate(StoreKind::Approvals, &approvals_doc),
        Verdict::Valid
    );
}

/// Fails if `updated_at` of the written roots is not the clock reading of the write.
#[test]
fn written_roots_carry_the_clock_time() {
    let tmp = Tmp::new("updated-at");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    clk.set(42);
    env.pending(&net(ApprovalLifetime::Once), None);
    for name in ["approvals.json", "log.json"] {
        let doc: serde_json::Value = serde_json::from_slice(&tmp.bytes(name)).unwrap();
        assert_eq!(doc["updated_at"], ts(42).to_rfc3339(), "{name}");
    }
}

// ---------------------------------------------------------------------------------------------------
// semantic registry

/// Fails if one of the four approval rules is promoted without its count assertion changing, loses its
/// check, or still names P03 as the place where the rest is enforced (P03 now implements the lifecycle),
/// or if a text is empty.
#[test]
fn approval_rule_rows_name_what_is_left_after_p03() {
    let want = [
        ("D11-12", "expire_run"),
        ("D11-13", "approval service"),
        ("D11-16", "redaction"),
        ("D11-17", "redirect"),
    ];
    for (rule, needle) in want {
        let info = rule_table().find(|r| r.id.0 == rule).expect(rule);
        let RuleStatus::Partial { rest_in } = info.status else {
            panic!("{rule} is expected to stay partial: {:?}", info.status);
        };
        assert!(rest_in.contains(needle), "{rule}: {rest_in}");
        assert!(!rest_in.contains("P03"), "{rule}: {rest_in}");
    }
}

// ---------------------------------------------------------------------------------------------------
// fault injection

/// A real file system that fails the `nth` call (1-based, counted from [`ArmedFs::arm`]) of one protocol
/// step before it runs. With `crash` set the process is then "dead": every later call fails too, including
/// `remove_file`, so no cleanup can run and a half-written temp file stays, as after a kill.
#[derive(Debug)]
struct ArmedFs {
    inner: RealFs,
    step: WriteStep,
    nth: u32,
    crash: bool,
    armed: AtomicBool,
    seen: AtomicU32,
    fired: AtomicBool,
    dead: AtomicBool,
}

impl ArmedFs {
    /// `nth == 0` never fires; the calls of `step` are still counted.
    fn new(step: WriteStep, nth: u32, crash: bool) -> Self {
        Self {
            inner: RealFs,
            step,
            nth,
            crash,
            armed: AtomicBool::new(false),
            seen: AtomicU32::new(0),
            fired: AtomicBool::new(false),
            dead: AtomicBool::new(false),
        }
    }

    fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }

    fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }

    /// Calls of the planned step since arming.
    fn calls(&self) -> u32 {
        self.seen.load(Ordering::SeqCst)
    }

    fn alive(&self) -> io::Result<()> {
        if self.dead.load(Ordering::SeqCst) {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            Ok(())
        }
    }

    fn gate(&self, step: WriteStep) -> io::Result<()> {
        self.alive()?;
        if !self.armed.load(Ordering::SeqCst) || step != self.step {
            return Ok(());
        }
        let n = self.seen.fetch_add(1, Ordering::SeqCst) + 1;
        if self.nth != 0 && n == self.nth {
            self.fired.store(true, Ordering::SeqCst);
            if self.crash {
                self.dead.store(true, Ordering::SeqCst);
                return Err(io::ErrorKind::Interrupted.into());
            }
            return Err(io::ErrorKind::StorageFull.into());
        }
        Ok(())
    }
}

impl FsOps for ArmedFs {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        self.gate(WriteStep::Read)?;
        self.inner.read(path, max_bytes)
    }

    fn create_new(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        self.gate(step)?;
        self.inner.create_new(step, path, mode)
    }

    fn write_all(&self, step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.gate(step)?;
        self.inner.write_all(step, path, bytes)
    }

    fn sync_file(&self, step: WriteStep, path: &Path) -> io::Result<()> {
        self.gate(step)?;
        self.inner.sync_file(step, path)
    }

    fn hard_link(&self, step: WriteStep, existing: &Path, new: &Path) -> io::Result<()> {
        self.gate(step)?;
        self.inner.hard_link(step, existing, new)
    }

    fn rename(&self, step: WriteStep, from: &Path, to: &Path) -> io::Result<()> {
        self.gate(step)?;
        self.inner.rename(step, from, to)
    }

    fn sync_dir(&self, step: WriteStep, dir: &Path) -> io::Result<()> {
        self.gate(step)?;
        self.inner.sync_dir(step, dir)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.alive()?;
        self.inner.remove_file(path)
    }

    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.alive()?;
        self.inner.list_dir(dir)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo> {
        self.alive()?;
        self.inner.symlink_metadata(path)
    }

    fn create_dir(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        self.gate(step)?;
        self.inner.create_dir(step, path, mode)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.alive()?;
        self.inner.canonicalize(path)
    }

    fn random_suffix(&self) -> io::Result<[u8; 8]> {
        self.alive()?;
        self.inner.random_suffix()
    }

    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle> {
        self.gate(WriteStep::Lock)?;
        self.inner.open_lock_file(path, mode)
    }

    fn effective_uid(&self) -> io::Result<u32> {
        self.inner.effective_uid()
    }
}

/// Every protocol step, so that a step the operation does not reach is noticed rather than assumed.
const ALL_STEPS: [WriteStep; 10] = [
    WriteStep::CreateTemp,
    WriteStep::WriteTemp,
    WriteStep::SyncTemp,
    WriteStep::LinkPrevious,
    WriteStep::ReplacePrevious,
    WriteStep::Replace,
    WriteStep::SyncDir,
    WriteStep::Read,
    WriteStep::Lock,
    WriteStep::CreateDir,
];

fn label(outcome: &Result<AuthOutcome, ApprovalError>) -> String {
    match outcome {
        Ok(AuthOutcome::Authorised(_)) => "Authorised".to_owned(),
        Ok(AuthOutcome::Refused(r)) => format!("Refused({r:?})"),
        Ok(AuthOutcome::NeedsApproval(NeedReason::NoGrant, _)) => "Needs(NoGrant)".to_owned(),
        Ok(AuthOutcome::NeedsApproval(NeedReason::AlreadyPending(_), _)) => {
            "Needs(AlreadyPending)".to_owned()
        }
        Ok(AuthOutcome::NeedsApproval(NeedReason::Mismatch { dimension, .. }, _)) => {
            format!("Needs(Mismatch {dimension:?})")
        }
        Err(e) => format!("Err({e:?})"),
    }
}

/// One cell of the matrix: a `once` grant, the consumption write failing at the `nth` call of `step`, then a
/// restart (fresh writer on the same directory) and a second `authorise`. `None` if the step is not reached
/// that many times. Asserts the invariants of §5.4 for the cell and returns its one-line description.
fn once_cell(step: WriteStep, nth: u32, crash: bool) -> Option<String> {
    let tmp = Tmp::new("matrix");
    let clk = Clk::new();
    let fs = Arc::new(ArmedFs::new(step, nth, crash));
    let req = net(ApprovalLifetime::Once);
    let (granted, first, halted) = {
        let mut env = Env::open(&tmp, fs.clone(), &clk, 0x10);
        let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
        fs.arm();
        let first = env.svc().authorise(&req, None);
        (granted, first, env.writer.is_halted())
    };
    if !fs.fired() {
        // The step was not reached `nth` times: the authorisation went through untouched.
        assert!(
            matches!(first, Ok(AuthOutcome::Authorised(_))),
            "{step:?}#{nth}"
        );
        return None;
    }
    let first_authorised = matches!(first, Ok(AuthOutcome::Authorised(_)));
    let disk_consumed = tmp.approval(granted).consumed;

    // Restart: a fresh writer (reconciliation removes what a crash left) and the same question again.
    let mut env2 = Env::open(&tmp, Arc::new(RealFs), &clk, 0x50);
    let second = env2.svc().authorise(&req, None);
    let second_authorised = matches!(second, Ok(AuthOutcome::Authorised(_)));
    drop(env2);

    let cell = format!("{step:?}#{nth} {}", if crash { "crash" } else { "fail" });
    assert!(
        !(first_authorised && second_authorised),
        "{cell}: Authorised received twice"
    );
    if first_authorised {
        assert!(
            disk_consumed,
            "{cell}: Authorised before consumption was durable"
        );
    } else {
        assert!(
            first.is_err(),
            "{cell}: a fault must be reported, got {first:?}"
        );
        // Consumed on disk (the caller never got Authorised): never granted again; unconsumed: still grantable.
        assert_eq!(
            second_authorised,
            !disk_consumed,
            "{cell}: consumed={disk_consumed} but second={}",
            label(&second)
        );
    }
    if disk_consumed && !second_authorised {
        assert_eq!(
            label(&second),
            "Needs(Mismatch Consumption)",
            "{cell}: a consumed grant must be asked for again explicitly"
        );
    }
    assert!(tmp.approval(granted).consumed || !second_authorised || !first_authorised);
    // After both calls the grant is consumed whenever anything was ever authorised.
    if first_authorised || second_authorised {
        assert!(tmp.approval(granted).consumed, "{cell}");
    }
    Some(format!(
        "{cell}: first={} halted={halted} consumed_on_disk={disk_consumed} second={}",
        label(&first),
        label(&second)
    ))
}

/// Fails if, at any injection point of the consumption write (every `WriteStep` the `authorise` reaches, every
/// call of it, as a reported I/O error and as a crash that also blocks cleanup), `Authorised` is received
/// twice, is received before the consumption is durable, or a consumed grant is granted again after a
/// restart. The expected matrix is pinned literally below, so an unexpected change of the write protocol or
/// of the order inside `authorise` is noticed.
#[test]
fn once_grant_consumption_survives_a_fault_at_every_write_step() {
    let mut matrix = Vec::new();
    for crash in [false, true] {
        for step in ALL_STEPS {
            let mut nth = 1;
            while let Some(line) = once_cell(step, nth, crash) {
                matrix.push(line);
                nth += 1;
                assert!(
                    nth < 20,
                    "{step:?}: the step is reached an implausible number of times"
                );
            }
        }
    }
    let expected: Vec<&str> = EXPECTED_MATRIX.to_vec();
    assert_eq!(
        matrix.iter().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
    // Both outcomes of the interruption exist in the matrix: lost-but-never-reused and not consumed.
    assert!(
        matrix
            .iter()
            .any(|l| l.contains("consumed_on_disk=true") && l.contains("first=Err"))
    );
    assert!(
        matrix
            .iter()
            .any(|l| l.contains("consumed_on_disk=false") && l.contains("second=Authorised"))
    );
}

const EXPECTED_MATRIX: &[&str] = &[
    "CreateTemp#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "WriteTemp#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "SyncTemp#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "LinkPrevious#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "ReplacePrevious#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "Replace#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "SyncDir#1 fail: first=Err(Store(Storage)) halted=true consumed_on_disk=true second=Needs(Mismatch Consumption)",
    "Read#1 fail: first=Err(Store(Storage)) halted=false consumed_on_disk=false second=Authorised",
    "CreateTemp#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "WriteTemp#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "SyncTemp#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "LinkPrevious#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "ReplacePrevious#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "Replace#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=false second=Authorised",
    "SyncDir#1 crash: first=Err(Store(Storage)) halted=true consumed_on_disk=true second=Needs(Mismatch Consumption)",
    "Read#1 crash: first=Err(Store(Storage)) halted=false consumed_on_disk=false second=Authorised",
];

/// Fails if the clean path of the matrix (no fault) ever stops authorising exactly once.
#[test]
fn without_a_fault_the_consumption_is_durable_and_final() {
    let tmp = Tmp::new("no-fault");
    let clk = Clk::new();
    let req = net(ApprovalLifetime::Once);
    let granted = {
        let mut env = Env::real(&tmp, &clk);
        let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
        assert!(matches!(
            env.svc().authorise(&req, None).unwrap(),
            AuthOutcome::Authorised(_)
        ));
        granted
    };
    assert!(tmp.approval(granted).consumed);
    let mut again = Env::open(&tmp, Arc::new(RealFs), &clk, 0x60);
    assert!(matches!(
        again.svc().authorise(&req, None).unwrap(),
        AuthOutcome::NeedsApproval(..)
    ));
}

/// Counts the `Replace` calls of one operation on a fresh directory.
fn replace_calls(op: impl Fn(&mut Env)) -> u32 {
    let tmp = Tmp::new("count");
    let clk = Clk::new();
    let fs = Arc::new(ArmedFs::new(WriteStep::Replace, 0, false));
    let mut env = Env::open(&tmp, fs.clone(), &clk, 0x10);
    fs.arm();
    op(&mut env);
    fs.calls()
}

/// Fails if a failure of the log write (the last `Replace` of `request`) is swallowed, does not halt the
/// writer, rolls the approval back, or lets the halted writer write again. After a restart the pending
/// request is reused; its `ApprovalRequested` event is lost (recorded limitation), never duplicated.
#[test]
fn a_log_write_failure_halts_the_writer_and_keeps_the_approval_state() {
    let req = net(ApprovalLifetime::Once);
    let total = replace_calls(|env| {
        env.pending(&net(ApprovalLifetime::Once), None);
    });
    assert!(total >= 2, "approvals and log are both written: {total}");

    let tmp = Tmp::new("log-fail");
    let clk = Clk::new();
    let fs = Arc::new(ArmedFs::new(WriteStep::Replace, total, false));
    let mut env = Env::open(&tmp, fs.clone(), &clk, 0x10);
    fs.arm();
    let outcome = env.svc().request(&req, None);
    assert!(fs.fired());
    assert_eq!(outcome, Err(ApprovalError::Store(DevError::Storage)));
    assert!(env.writer.is_halted());
    assert_eq!(
        env.svc().request(&net(ApprovalLifetime::Run), None),
        Err(ApprovalError::Store(DevError::Halted)),
        "a halted writer must not write"
    );
    drop(env);

    let stored = tmp.approvals();
    assert_eq!(stored.len(), 1, "the approval written before the log stays");
    assert_eq!(stored[0].status, ApprovalState::Pending);
    assert!(tmp.events().is_empty());

    let mut restarted = Env::open(&tmp, Arc::new(RealFs), &clk, 0x50);
    assert_eq!(
        restarted.pending(&req, None),
        stored[0].approval_id,
        "the pending request is reused, not duplicated"
    );
    assert_eq!(tmp.approvals().len(), 1);
}

/// Fails if a failed approvals write leaves a record, an event, or a half-decided state, for `request`,
/// `resolve` and `revoke`, or if the operation succeeds after a restart.
#[test]
fn a_failed_approvals_write_changes_nothing() {
    let req = net(ApprovalLifetime::Once);

    // request
    {
        let tmp = Tmp::new("fail-request");
        let clk = Clk::new();
        let fs = Arc::new(ArmedFs::new(WriteStep::Replace, 1, false));
        let mut env = Env::open(&tmp, fs.clone(), &clk, 0x10);
        fs.arm();
        assert_eq!(
            env.svc().request(&req, None),
            Err(ApprovalError::Store(DevError::Storage))
        );
        assert!(env.writer.is_halted());
        drop(env);
        assert!(tmp.approvals().is_empty());
        assert!(tmp.events().is_empty());
    }
    // resolve and revoke
    for revoke in [false, true] {
        let tmp = Tmp::new("fail-decide");
        let clk = Clk::new();
        let fs = Arc::new(ArmedFs::new(WriteStep::Replace, 1, false));
        let mut env = Env::open(&tmp, fs.clone(), &clk, 0x10);
        let approval = if revoke {
            env.grant(&req, ApprovalDecision::ApproveOnce, None)
        } else {
            env.pending(&req, None)
        };
        let events_before = tmp.events().len();
        fs.arm();
        let result = if revoke {
            env.svc().revoke(approval, Actor::LocalUserCli)
        } else {
            env.svc()
                .resolve(
                    approval,
                    ApprovalDecision::ApproveOnce,
                    Actor::LocalUserCli,
                    None,
                )
                .map(|_| ())
        };
        assert_eq!(result, Err(ApprovalError::Store(DevError::Storage)));
        assert!(env.writer.is_halted());
        drop(env);
        let expected = if revoke {
            ApprovalState::Approved
        } else {
            ApprovalState::Pending
        };
        assert_eq!(tmp.approval(approval).status, expected, "revoke={revoke}");
        assert_eq!(tmp.events().len(), events_before, "revoke={revoke}");

        let mut restarted = Env::open(&tmp, Arc::new(RealFs), &clk, 0x50);
        let retried = if revoke {
            restarted.svc().revoke(approval, Actor::LocalUserCli)
        } else {
            restarted
                .svc()
                .resolve(
                    approval,
                    ApprovalDecision::ApproveOnce,
                    Actor::LocalUserCli,
                    None,
                )
                .map(|_| ())
        };
        assert_eq!(retried, Ok(()), "revoke={revoke}");
    }
}

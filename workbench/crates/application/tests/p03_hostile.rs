//! P03-T11 hostile fixtures: end-to-end adversarial scenarios over the accepted permission, approval,
//! child-environment and credential code (`planning/P03-T01-request.md` §5.2 to §5.6, §6, §7;
//! `planning/P03-packets/T11-W1.md`).
//!
//! Everything goes through the public API only: `ApprovalService` over a private data directory under
//! `CARGO_TARGET_TMPDIR` (real `StoreWriter`, real schema validation), `evaluate`,
//! `NormalisedDestination::parse`, `fingerprint`, `child_env::build`, `SessionCredentialStore` and
//! `read_key`. The unit-level tables are in `p03_destination`, `p03_fingerprint`, `p03_evaluate`,
//! `p03_approvals`, `p03_child_env` and `p03_credential`; this file targets combinations and adversarial
//! inputs they do not cover. Every test says which requirement row it attacks and what would make it fail.
//! All keys are synthetic (`pwt-test-FAKE-...`); no assertion message prints a key.
//!
//! Groups: (1) grant abuse, (2) lifecycle abuse, (3) refusals, (4) seeded-key sweep, (5) store tampering.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use application::approvals::{
    self, Actor, ApprovalError, ApprovalService, AuthOutcome, RequestOutcome,
};
use application::child_env::{
    EnvError, HostEnv, Origin, SecretProbe, ToolDirs, ToolProfile, build,
};
use application::credential::{CredentialStore, KeyFormatError, SessionCredentialStore, read_key};
use application::dev_store::{self, DevError};
use application::emit::{Clock, Emitter, IdSource, IdSourceError};
use application::permission::{
    ArgError, ArgName, DestinationError, Evaluation, EvaluationContext, NeedReason,
    NormalisedDestination, OperationArg, OperationName, PermissionRequest, RefusalReason,
    RequestedData, ScopeDimension, WorkflowBinding, evaluate, fingerprint,
};
use domain::contract::{AppEvent, ApprovalDecision, ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{ApprovalId, RepoId, RunId, WorkflowId};
use domain::states::ApprovalState;
use domain::store::approval::Approval;
use domain::store::log::Event;
use domain::store::provider::InferenceScope;
use domain::store::{AnyStore, StoreKind};
use domain::text::sanitise;
use domain::time::UtcTimestamp;
use domain::vocab::Capability;
use persistence::error::StoreError;
use persistence::fs::{FsOps, RealFs};
use persistence::paths::DataDir;
use persistence::read::{SchemaValidator, decode};
use persistence::writer::StoreWriter;

const T0: i64 = 1_800_000_000;
/// The workflow revision the callers below report as the saved one.
const REV: u32 = 3;

/// The synthetic session key of the sweep, and a fragment of it that catches a truncated leak.
const KEY: &str = "pwt-test-FAKE-sweep-Zq9f7Lk2Xv4m";
const KEY_FRAGMENT: &str = "Zq9f7Lk2Xv4m";

// ---------------------------------------------------------------------------------------------------
// helpers

fn id<T>(parse: fn(&str) -> Result<T, domain::ids::IdError>, n: u32) -> T {
    parse(&format!("00000000-0000-4000-8000-{n:012x}")).expect("id")
}

fn repo(n: u32) -> RepoId {
    id(RepoId::parse, 0x100 + n)
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

fn arg(name: &str, value: &str) -> OperationArg {
    OperationArg {
        name: ArgName::parse(name).expect("argument name"),
        value: value.to_owned(),
    }
}

/// A shared, settable clock: [`Clk::set`] moves every clone.
#[derive(Debug, Clone)]
struct Clk(Arc<AtomicI64>);

impl Clk {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(T0)))
    }

    /// Sets the time to `T0 + offset` seconds.
    fn set(&self, offset: i64) {
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
struct SeqIds {
    prefix: u8,
    next: u32,
}

impl SeqIds {
    fn new(prefix: u8) -> Self {
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

/// A private (0700) data directory removed on drop.
struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p03host-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let tmp = Self { root };
        assert_eq!(dev_store::init(&tmp.root).failure, None);
        tmp
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn bytes(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.path(name)).unwrap()
    }

    /// The validated, decoded approvals of the directory (read straight from the file).
    fn approvals(&self) -> Vec<Approval> {
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

    fn events(&self) -> Vec<Event> {
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

    fn approval(&self, id: ApprovalId) -> Approval {
        self.approvals()
            .into_iter()
            .find(|a| a.approval_id == id)
            .expect("approval exists")
    }

    /// Hand-edits `approvals.json` in place (the writer, if open, keeps its lock and re-reads the file on
    /// its next call).
    fn tamper(&self, edit: impl FnOnce(&mut serde_json::Value)) {
        let path = self.path("approvals.json");
        let mut doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).expect("approvals.json is JSON");
        edit(&mut doc);
        std::fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The files the lifecycle writes; everything a refusal must leave alone.
fn store_bytes(tmp: &Tmp) -> BTreeMap<&'static str, Vec<u8>> {
    ["approvals.json", "log.json", "workflows.json"]
        .into_iter()
        .map(|name| (name, tmp.bytes(name)))
        .collect()
}

/// One open writer with an emitter, clock and approval-ID source.
struct Env {
    writer: StoreWriter,
    emitter: Emitter<Clk, SeqIds>,
    clk: Clk,
    ids: SeqIds,
}

impl Env {
    /// Opens the directory `root`. `prefix` seeds the approval IDs (event IDs use `prefix + 0x80`), so
    /// that two openings of one directory never repeat an ID.
    fn try_open(root: &Path, clk: &Clk, prefix: u8) -> Result<Self, StoreError> {
        let fs: Arc<dyn FsOps> = Arc::new(RealFs);
        let data = DataDir::new(&*fs, root)?;
        let (writer, _) = StoreWriter::open(data, fs, Arc::new(SchemaValidator::new()))?;
        Ok(Self {
            writer,
            emitter: Emitter::new(clk.clone(), SeqIds::new(prefix.wrapping_add(0x80))),
            clk: clk.clone(),
            ids: SeqIds::new(prefix),
        })
    }

    fn open(tmp: &Tmp, clk: &Clk, prefix: u8) -> Self {
        Self::try_open(&tmp.root, clk, prefix).expect("open")
    }

    fn real(tmp: &Tmp, clk: &Clk) -> Self {
        Self::open(tmp, clk, 0x10)
    }

    fn svc(&mut self) -> ApprovalService<'_, Clk, SeqIds> {
        ApprovalService::new(
            &mut self.writer,
            &mut self.emitter,
            &self.clk,
            &mut self.ids,
        )
    }

    fn pending(&mut self, req: &PermissionRequest, rev: Option<u32>) -> ApprovalId {
        match self.svc().request(req, rev).expect("request") {
            RequestOutcome::Pending(id) => id,
            other => panic!("expected a pending request, got {other:?}"),
        }
    }

    /// Requests and resolves `req`; the approval ID.
    fn grant(
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

    fn authorise(
        &mut self,
        req: &PermissionRequest,
        rev: Option<u32>,
    ) -> Result<AuthOutcome, ApprovalError> {
        self.svc().authorise(req, rev)
    }

    fn ctx(&self, rev: Option<u32>) -> EvaluationContext {
        EvaluationContext {
            now: self.clk.now(),
            current_workflow_revision: rev,
        }
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
fn net(lifetime: ApprovalLifetime) -> PermissionRequest {
    PermissionRequest {
        capability: Capability::GitAcquire,
        operation: OperationName::parse("git.clone").unwrap(),
        arguments: vec![arg("branch", "main")],
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

fn variant(
    base: &PermissionRequest,
    edit: impl FnOnce(&mut PermissionRequest),
) -> PermissionRequest {
    let mut req = base.clone();
    edit(&mut req);
    req
}

fn scope_with(model: &str, sources: &[&str], redaction: &str) -> InferenceScope {
    serde_json::from_value(serde_json::json!({
        "provider_id": "openrouter",
        "model_id": model,
        "routing_policy": {"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": ["openrouter"]},
        "source_ids": sources,
        "redaction_version": redaction
    }))
    .expect("inference scope")
}

const SOURCE_A: &str = "01234567-89ab-4cde-8f01-23456789abcd";
const SOURCE_B: &str = "11234567-89ab-4cde-8f01-23456789abcd";

fn scope() -> InferenceScope {
    scope_with("example/model", &[SOURCE_A], "2026.10-a")
}

/// A `model_transmission` request (run lifetime, so that a grant is not consumed by probing).
fn model(scope: Option<InferenceScope>) -> PermissionRequest {
    PermissionRequest {
        capability: Capability::ModelTransmission,
        operation: OperationName::parse("model.send").unwrap(),
        arguments: Vec::new(),
        workflow: None,
        run_id: Some(run(1)),
        repo_ids: vec![repo(1)],
        destinations: vec![dest("https://openrouter.ai/api/v1")],
        resource_constraints: Vec::new(),
        data: vec![RequestedData::Category(DataCategory::RedactedFindings)],
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: ApprovalLifetime::Run,
        validity_seconds: None,
        inference_scope: scope,
    }
}

fn is_authorised(outcome: &Result<AuthOutcome, ApprovalError>) -> bool {
    matches!(outcome, Ok(AuthOutcome::Authorised(_)))
}

/// The dimension of a `NeedsApproval(Mismatch { .. })` outcome.
fn mismatch(outcome: &Result<AuthOutcome, ApprovalError>) -> Option<ScopeDimension> {
    match outcome {
        Ok(AuthOutcome::NeedsApproval(NeedReason::Mismatch { dimension, .. }, _)) => {
            Some(*dimension)
        }
        _ => None,
    }
}

/// Asserts that `req` is authorised by no entry point: not by the pure evaluator, not by `authorise`, not by
/// `request`. Returns the `authorise` outcome (the first call that sees the request, so its reason is the
/// evaluator's, not `AlreadyPending`).
fn assert_not_authorised(
    env: &mut Env,
    tmp: &Tmp,
    label: &str,
    req: &PermissionRequest,
    rev: Option<u32>,
) -> AuthOutcome {
    let ctx = env.ctx(rev);
    assert!(
        !matches!(
            evaluate(req, &tmp.approvals(), &ctx),
            Evaluation::Permitted(_)
        ),
        "{label}: evaluate permitted it"
    );
    let outcome = env.authorise(req, rev).expect("authorise");
    assert!(
        !matches!(outcome, AuthOutcome::Authorised(_)),
        "{label}: authorise authorised it"
    );
    let requested = env.svc().request(req, rev).expect("request");
    assert_ne!(
        requested,
        RequestOutcome::Permitted,
        "{label}: request permitted it"
    );
    outcome
}

// ---------------------------------------------------------------------------------------------------
// Group 1 - grant abuse

/// Attacks G2 to G10 and R1/R2/R5 (rows of request §6). One approved `once` grant; every single change to the
/// operation must leave it unused: no entry point authorises, and afterwards the grant is still unconsumed and
/// authorises the original request exactly once. Fails if any dimension is dropped from the match (the
/// variant would be authorised), or if probing consumes or alters the grant.
#[test]
fn a_once_grant_does_not_cover_any_single_change_of_the_operation() {
    let tmp = Tmp::new("g1-once");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let base = net(ApprovalLifetime::Once);
    let granted = env.grant(&base, ApprovalDecision::ApproveOnce, None);

    let cap = |c: Capability| variant(&base, |r| r.capability = c);
    let variants: Vec<(&str, PermissionRequest, Option<u32>)> = vec![
        // Destination: host, subdomain, port, scheme, path (case, trailing slash, extension).
        (
            "host",
            variant(&base, |r| {
                r.destinations = vec![dest("https://gitlab.com/example/repo")];
            }),
            None,
        ),
        (
            "subdomain",
            variant(&base, |r| {
                r.destinations = vec![dest("https://api.github.com/example/repo")];
            }),
            None,
        ),
        (
            "port",
            variant(&base, |r| {
                r.destinations = vec![dest("https://github.com:8443/example/repo")];
            }),
            None,
        ),
        (
            "scheme",
            variant(&base, |r| {
                r.destinations = vec![dest("ssh://github.com/example/repo")];
            }),
            None,
        ),
        (
            "path",
            variant(&base, |r| {
                r.destinations = vec![dest("https://github.com/example/other")];
            }),
            None,
        ),
        (
            "path case",
            variant(&base, |r| {
                r.destinations = vec![dest("https://github.com/Example/Repo")];
            }),
            None,
        ),
        (
            "path trailing slash",
            variant(&base, |r| {
                r.destinations = vec![dest("https://github.com/example/repo/")];
            }),
            None,
        ),
        (
            "path extension",
            variant(&base, |r| {
                r.destinations = vec![dest("https://github.com/example/repo/extra")];
            }),
            None,
        ),
        (
            "extra destination",
            variant(&base, |r| {
                r.destinations.push(dest("https://gitlab.com/example/repo"));
            }),
            None,
        ),
        (
            "no destination",
            variant(&base, |r| r.destinations.clear()),
            None,
        ),
        // Repositories.
        (
            "extra repo",
            variant(&base, |r| r.repo_ids.push(repo(2))),
            None,
        ),
        ("removed repo", variant(&base, |r| r.repo_ids.clear()), None),
        (
            "other repo",
            variant(&base, |r| r.repo_ids = vec![repo(2)]),
            None,
        ),
        // Data categories.
        (
            "extra category",
            variant(&base, |r| {
                r.data
                    .push(RequestedData::Category(DataCategory::RedactedSource));
            }),
            None,
        ),
        ("removed category", variant(&base, |r| r.data.clear()), None),
        (
            "other category",
            variant(&base, |r| {
                r.data = vec![RequestedData::Category(DataCategory::AppMetadata)];
            }),
            None,
        ),
        (
            "raw secret added",
            variant(&base, |r| r.data.push(RequestedData::RawSecretValues)),
            None,
        ),
        // Arguments (fingerprint): value, case, whitespace, control characters, count, name.
        (
            "argument value",
            variant(&base, |r| r.arguments = vec![arg("branch", "develop")]),
            None,
        ),
        (
            "argument value case",
            variant(&base, |r| r.arguments = vec![arg("branch", "Main")]),
            None,
        ),
        (
            "argument value trailing space",
            variant(&base, |r| r.arguments = vec![arg("branch", "main ")]),
            None,
        ),
        (
            "argument value NUL suffix",
            variant(&base, |r| r.arguments = vec![arg("branch", "main\u{0}")]),
            None,
        ),
        (
            "argument value newline",
            variant(&base, |r| r.arguments = vec![arg("branch", "main\n--evil")]),
            None,
        ),
        (
            "argument value empty",
            variant(&base, |r| r.arguments = vec![arg("branch", "")]),
            None,
        ),
        (
            "extra argument",
            variant(&base, |r| r.arguments.push(arg("depth", "1"))),
            None,
        ),
        (
            "removed argument",
            variant(&base, |r| r.arguments.clear()),
            None,
        ),
        (
            "renamed argument",
            variant(&base, |r| r.arguments = vec![arg("ref", "main")]),
            None,
        ),
        // Operation and constraints.
        (
            "operation",
            variant(&base, |r| {
                r.operation = OperationName::parse("git.fetch").unwrap();
            }),
            None,
        ),
        (
            "constraint value",
            variant(&base, |r| {
                r.resource_constraints = vec![sanitise("max_size_mb=999")];
            }),
            None,
        ),
        (
            "extra constraint",
            variant(&base, |r| {
                r.resource_constraints.push(sanitise("timeout_s=600"));
            }),
            None,
        ),
        (
            "removed constraint",
            variant(&base, |r| r.resource_constraints.clear()),
            None,
        ),
        // Capability: every other networked pilot capability.
        ("capability git_update", cap(Capability::GitUpdate), None),
        (
            "capability external_reference",
            cap(Capability::ExternalReference),
            None,
        ),
        (
            "capability schema_fetch",
            cap(Capability::SchemaFetch),
            None,
        ),
        (
            "capability vulnerability_lookup",
            cap(Capability::VulnerabilityLookup),
            None,
        ),
        (
            "inference scope on a non-model capability",
            variant(&base, |r| r.inference_scope = Some(scope())),
            None,
        ),
        // Context: a once grant without a workflow does not cover a workflow-bound request.
        (
            "workflow binding added",
            variant(&base, |r| r.workflow = binding(1, REV)),
            Some(REV),
        ),
    ];
    let count = variants.len();
    assert!(count >= 36, "the table shrank to {count}");
    for (label, req, rev) in &variants {
        assert_not_authorised(&mut env, &tmp, label, req, *rev);
    }

    // The grant is untouched by all of it and still serves the original request exactly once.
    let record = tmp.approval(granted);
    assert_eq!(record.status, ApprovalState::Approved);
    assert!(!record.consumed, "probing must not consume the grant");
    match env.authorise(&base, None).expect("authorise") {
        AuthOutcome::Authorised(proof) => assert_eq!(proof.approval_id(), granted),
        other => panic!("the original request must be authorised once, got {other:?}"),
    }
    assert!(tmp.approval(granted).consumed);
    assert!(!is_authorised(&env.authorise(&base, None)));
}

/// Positive control for the test above and for C6: spellings that normalise to the granted operation DO match,
/// and so does a re-ordered or duplicated list (F4 to F7 are sets). Fails if the match became too strict (a
/// legitimate spelling would be asked again) or if normalisation differs between `parse` and the store form.
#[test]
fn equivalent_spellings_of_the_granted_operation_are_authorised() {
    let two = |r: &mut PermissionRequest| {
        r.destinations = vec![
            dest("https://github.com/example/repo"),
            dest("https://gitlab.com/example/repo"),
        ];
        r.repo_ids = vec![repo(1), repo(2)];
        r.resource_constraints = vec![sanitise("max_size_mb=100"), sanitise("timeout_s=600")];
        r.data = vec![
            RequestedData::Category(DataCategory::RepositoryMetadata),
            RequestedData::Category(DataCategory::RedactedSource),
        ];
        r.arguments = vec![arg("branch", "main"), arg("depth", "1")];
    };
    let base = variant(&net(ApprovalLifetime::Once), two);
    let spellings: Vec<(&str, PermissionRequest)> = vec![
        (
            "host case, trailing dot, default port, scheme case",
            variant(&base, |r| {
                r.destinations = vec![
                    dest("HTTPS://GitHub.COM.:443/example/repo"),
                    dest("https://gitlab.com/example/repo"),
                ];
            }),
        ),
        (
            "reversed lists",
            variant(&base, |r| {
                r.destinations.reverse();
                r.repo_ids.reverse();
                r.resource_constraints.reverse();
                r.data.reverse();
                r.arguments.reverse();
            }),
        ),
        (
            "duplicated entries",
            variant(&base, |r| {
                let d = r.destinations.clone();
                r.destinations.extend(d);
                let p = r.repo_ids.clone();
                r.repo_ids.extend(p);
                let c = r.data.clone();
                r.data.extend(c);
            }),
        ),
    ];
    for (label, spelling) in spellings {
        let tmp = Tmp::new("g1-spell");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let granted = env.grant(&base, ApprovalDecision::ApproveOnce, None);
        match env.authorise(&spelling, None).expect(label) {
            AuthOutcome::Authorised(proof) => assert_eq!(proof.approval_id(), granted, "{label}"),
            other => panic!("{label}: expected Authorised, got {other:?}"),
        }
        assert!(!is_authorised(&env.authorise(&spelling, None)), "{label}");
    }
}

/// Attacks destination normalisation C6/G8 (request §6). A `run` grant (not consumed by probing) for
/// `https://api.example.com/v1`: look-alike hosts and paths are not covered, spellings that C6 declares
/// equal (case, ONE trailing dot, default port) are, and the inputs C6 rejects (double dot, `xn--`, Unicode
/// homoglyph, zero-width character, IPv4 shortcut forms, IPv6, userinfo tricks, query tricks) never become a
/// destination at all, with the most specific error. Fails if a look-alike is authorised, an equal spelling
/// is not, or a rejected spelling parses.
#[test]
fn lookalike_hosts_are_not_covered_and_c6_equal_spellings_are() {
    let tmp = Tmp::new("g1-lookalike");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let at = |d: &str| {
        variant(&net(ApprovalLifetime::Run), |r| {
            r.destinations = vec![dest(d)]
        })
    };
    let base = at("https://api.example.com/v1");
    let granted = env.grant(&base, ApprovalDecision::ApproveOnce, None);

    // Parse fine, different destination: never covered.
    for input in [
        "https://api-example.com/v1",
        "https://api.example.com.evil.test/v1",
        "https://example.com/v1",
        "https://apiexample.com/v1",
        "https://xapi.example.com/v1",
        "https://api.example.co/v1",
        "https://api.example.com:8443/v1",
        "https://api.example.com/v1/",
        "https://api.example.com/v2",
        "https://api.example.com/V1",
        "https://api.example.com/",
        "https://api.example.com",
        "https://127.0.0.1/v1",
        "ssh://api.example.com/v1",
    ] {
        assert_not_authorised(&mut env, &tmp, input, &at(input), None);
    }

    // C6-equal spellings: covered (the run grant is not consumed, so they can all be tried).
    for input in [
        "https://API.Example.COM/v1",
        "https://api.example.com./v1",
        "https://api.example.com:443/v1",
        "HTTPS://api.example.com/v1",
        "https://Api.Example.Com.:443/v1",
    ] {
        match env.authorise(&at(input), None).expect(input) {
            AuthOutcome::Authorised(proof) => assert_eq!(proof.approval_id(), granted, "{input}"),
            other => panic!("{input}: expected Authorised, got {other:?}"),
        }
    }

    // Rejected by C6: no destination, hence no request.
    for (input, expected) in [
        ("https://api.example.com../v1", DestinationError::Syntax),
        (
            "https://xn--pexample-q1a.com/v1",
            DestinationError::Punycode,
        ),
        ("https://XN--abc.example.com/v1", DestinationError::Punycode),
        (
            "https://\u{430}pi.example.com/v1",
            DestinationError::NonAscii,
        ),
        (
            "https://api.\u{435}xample.com/v1",
            DestinationError::NonAscii,
        ),
        (
            "https://\u{ff41}pi.example.com/v1",
            DestinationError::NonAscii,
        ),
        (
            "https://api.example.com\u{200b}/v1",
            DestinationError::NonAscii,
        ),
        ("https://127.1/v1", DestinationError::IpLiteral),
        ("https://0x7f.0.0.1/v1", DestinationError::IpLiteral),
        ("https://2130706433/v1", DestinationError::IpLiteral),
        ("https://0177.0.0.1/v1", DestinationError::IpLiteral),
        ("https://999.1.1.1/v1", DestinationError::IpLiteral),
        ("https://1.2.3.4.5/v1", DestinationError::IpLiteral),
        ("https://[::1]/v1", DestinationError::IpLiteral),
        ("https://[::1]:443/v1", DestinationError::IpLiteral),
        ("https://::1/v1", DestinationError::IpLiteral),
        (
            "https://api.example.com:443:443/v1",
            DestinationError::IpLiteral,
        ),
        (
            "https://api.example.com@evil.test/v1",
            DestinationError::Userinfo,
        ),
        (
            "https://evil.test\\@api.example.com/v1",
            DestinationError::Userinfo,
        ),
        (
            "https://api.example.com#@evil.test/v1",
            DestinationError::Query,
        ),
        ("https://api.example.com?x=1", DestinationError::Query),
        (
            "https://api.example.com%2eevil.test/v1",
            DestinationError::Syntax,
        ),
        (
            "https://api.example.com\\.evil.test/v1",
            DestinationError::Syntax,
        ),
        ("https://api_example.com/v1", DestinationError::Syntax),
        ("https://api.example.com\t/v1", DestinationError::Syntax),
        ("http://api.example.com/v1", DestinationError::Scheme),
        ("ftp://api.example.com/v1", DestinationError::Scheme),
        ("file:///etc/passwd", DestinationError::Scheme),
        ("https:api.example.com", DestinationError::Scheme),
        ("//api.example.com/v1", DestinationError::Scheme),
        ("https://api.example.com:0/v1", DestinationError::Port),
        ("https://api.example.com:65536/v1", DestinationError::Port),
        ("https://api.example.com:-1/v1", DestinationError::Port),
        ("https://api.example.com:/v1", DestinationError::Port),
    ] {
        assert_eq!(
            NormalisedDestination::parse(input),
            Err(expected),
            "{input:?}"
        );
    }

    // The grant itself was not used up or changed by any of it.
    let record = tmp.approval(granted);
    assert_eq!(record.status, ApprovalState::Approved);
    assert_eq!(record.lifetime, ApprovalLifetime::Run);
}

/// Recorded limitation, not a defect (P03-D01 B5/H4; owner-deferred to P04/P16): destination normalisation
/// accepts private, loopback and link-local IPv4 literals and `localhost`; whether a connection may go there
/// is decided by the P04 egress policy and the P16 provider client. What must hold today is that a grant for
/// a public host never covers such a destination. Fails if `parse` starts rejecting them (the P04/P16 plan
/// would change and this note must be revisited) or if a public-host grant authorises an internal address.
#[test]
fn internal_addresses_parse_but_are_never_covered_by_a_public_host_grant() {
    let tmp = Tmp::new("g1-internal");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let base = net(ApprovalLifetime::Run);
    let granted = env.grant(&base, ApprovalDecision::ApproveOnce, None);
    for input in [
        "https://127.0.0.1/example/repo",
        "https://10.0.0.1/example/repo",
        "https://169.254.169.254/latest/meta-data",
        "https://192.168.0.1/example/repo",
        "https://0.0.0.0/example/repo",
        "https://localhost/example/repo",
    ] {
        let destination = NormalisedDestination::parse(input).expect("accepted (limitation B5/H4)");
        let req = variant(&base, |r| r.destinations = vec![destination]);
        assert_not_authorised(&mut env, &tmp, input, &req, None);
    }
    assert!(is_authorised(&env.authorise(&base, None)));
    assert_eq!(tmp.approval(granted).status, ApprovalState::Approved);
}

/// Attacks G4/G5 and R4 (rule 14): a workflow-scope grant at revision 3. A stale or unknown revision, a
/// rebound revision, another workflow, an unbound request and revision 0 never use it; after the saved
/// revision moves on, the old grant stays unused and a stale pending request can no longer be approved at
/// workflow scope. Fails if any of those is authorised, or if a refusal reports another reason.
#[test]
fn a_workflow_scope_grant_never_serves_another_revision_or_workflow() {
    let tmp = Tmp::new("g1-workflow");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let base = net(ApprovalLifetime::WorkflowRevision);
    let granted = env.grant(&base, ApprovalDecision::ApproveWorkflowScope, Some(REV));
    assert_eq!(
        tmp.approval(granted).lifetime,
        ApprovalLifetime::WorkflowRevision
    );

    // Control: the grant is not consumed and serves the bound request repeatedly.
    for _ in 0..3 {
        assert!(is_authorised(&env.authorise(&base, Some(REV))));
    }
    assert!(!tmp.approval(granted).consumed);

    // Request bound to revision 3 while the saved revision is 4 (rule 14), or while it is unknown.
    for (label, rev, reason) in [
        (
            "saved revision moved",
            Some(REV + 1),
            RefusalReason::StaleWorkflowRevision,
        ),
        (
            "saved revision u32::MAX",
            Some(u32::MAX),
            RefusalReason::StaleWorkflowRevision,
        ),
        ("workflow not saved", None, RefusalReason::UnknownWorkflow),
    ] {
        assert_eq!(
            assert_not_authorised(&mut env, &tmp, label, &base, rev),
            AuthOutcome::Refused(reason),
            "{label}"
        );
    }

    // Request rebound to the new revision: the old grant's revision no longer matches (G5).
    let rebound = variant(&base, |r| r.workflow = binding(1, REV + 1));
    let outcome = assert_not_authorised(&mut env, &tmp, "rebound", &rebound, Some(REV + 1));
    assert!(
        matches!(
            outcome,
            AuthOutcome::NeedsApproval(
                NeedReason::Mismatch { approval_id, dimension: ScopeDimension::WorkflowRevision },
                _
            ) if approval_id == granted
        ),
        "rebound: {outcome:?}"
    );

    // Another workflow at the same revision (G4), and an unbound request for the same operation (G4).
    let other_workflow = variant(&base, |r| r.workflow = binding(2, REV));
    let outcome =
        assert_not_authorised(&mut env, &tmp, "other workflow", &other_workflow, Some(REV));
    assert_eq!(
        mismatch(&Ok(outcome)),
        Some(ScopeDimension::Workflow),
        "other workflow"
    );
    let unbound = variant(&base, |r| {
        r.workflow = None;
        r.lifetime = ApprovalLifetime::Once;
    });
    let outcome = assert_not_authorised(&mut env, &tmp, "unbound", &unbound, None);
    assert_eq!(
        mismatch(&Ok(outcome)),
        Some(ScopeDimension::Workflow),
        "unbound"
    );

    // Revision 0 is not a revision.
    let zero = variant(&base, |r| r.workflow = binding(1, 0));
    assert_eq!(
        assert_not_authorised(&mut env, &tmp, "revision 0", &zero, Some(0)),
        AuthOutcome::Refused(RefusalReason::InvalidRequest)
    );

    // The saved revision moves to 4. A pending request bound to 3 can no longer get workflow scope, and
    // writes nothing when it fails. (Approving it once is allowed; the grant is then unusable, see below.)
    let stale_request = variant(&base, |r| {
        r.operation = OperationName::parse("git.stale").unwrap();
    });
    let stale_pending = env.pending(&stale_request, Some(REV));
    let before = store_bytes(&tmp);
    for rev in [Some(REV + 1), None, Some(0)] {
        assert_eq!(
            env.svc().resolve(
                stale_pending,
                ApprovalDecision::ApproveWorkflowScope,
                Actor::LocalUserCli,
                rev
            ),
            Err(ApprovalError::WorkflowScopeUnavailable),
            "{rev:?}"
        );
    }
    assert_eq!(store_bytes(&tmp), before);
    assert_eq!(
        env.svc().resolve(
            stale_pending,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            Some(REV + 1)
        ),
        Ok(ApprovalState::Approved)
    );
    assert_eq!(
        assert_not_authorised(
            &mut env,
            &tmp,
            "stale once grant",
            &stale_request,
            Some(REV + 1)
        ),
        AuthOutcome::Refused(RefusalReason::StaleWorkflowRevision)
    );
    assert!(!tmp.approval(stale_pending).consumed);
}

/// Attacks G6/G12 and R3: a `run` grant for run 1. Another run, no run, another capability and a
/// non-matching lifetime binding never use it; the run's end (`expire_run`) ends it, for run 1 only. Fails if
/// another run (or none) is authorised, if `expire_run` of another run ends it, or if it survives its own
/// run.
#[test]
fn a_run_grant_serves_its_run_only_and_ends_with_it() {
    let tmp = Tmp::new("g1-run");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let base = net(ApprovalLifetime::Run);
    let granted = env.grant(&base, ApprovalDecision::ApproveOnce, None);
    assert_eq!(tmp.approval(granted).lifetime, ApprovalLifetime::Run);

    let other_run = variant(&base, |r| r.run_id = Some(run(2)));
    let outcome = assert_not_authorised(&mut env, &tmp, "other run", &other_run, None);
    assert_eq!(mismatch(&Ok(outcome)), Some(ScopeDimension::Run));
    let no_run_once = variant(&base, |r| {
        r.run_id = None;
        r.lifetime = ApprovalLifetime::Once;
    });
    let outcome = assert_not_authorised(&mut env, &tmp, "no run", &no_run_once, None);
    assert_eq!(mismatch(&Ok(outcome)), Some(ScopeDimension::Run));
    let no_run_bound = variant(&base, |r| r.run_id = None);
    assert_eq!(
        assert_not_authorised(
            &mut env,
            &tmp,
            "run lifetime without run",
            &no_run_bound,
            None
        ),
        AuthOutcome::Refused(RefusalReason::LifetimeBinding)
    );
    let other_capability = variant(&base, |r| r.capability = Capability::GitUpdate);
    let outcome = assert_not_authorised(&mut env, &tmp, "capability", &other_capability, None);
    assert!(
        matches!(outcome, AuthOutcome::NeedsApproval(NeedReason::NoGrant, _)),
        "capability: {outcome:?}"
    );

    // Control, then the end of the run.
    assert!(is_authorised(&env.authorise(&base, None)));
    assert!(is_authorised(&env.authorise(&base, None)));
    assert_eq!(env.svc().expire_run(run(2)), Ok(0));
    assert!(is_authorised(&env.authorise(&base, None)));
    assert_eq!(env.svc().expire_run(run(1)), Ok(1));
    assert_eq!(tmp.approval(granted).status, ApprovalState::Expired);
    assert!(!is_authorised(&env.authorise(&base, None)));
    assert_eq!(env.svc().expire_run(run(1)), Ok(0));
}

// ---------------------------------------------------------------------------------------------------
// Group 2 - lifecycle abuse

/// Attacks G13 (expiry boundary) and rule 13: a `run` grant with a 60 s validity decided at +10 s ends at
/// +70 s. +69 s authorises, exactly the end (`expires_at == now`) and one second after do not, `expire_due`
/// marks it `expired` once, and moving the clock back afterwards does not revive an expired record. Fails
/// if the boundary is off by one in either direction, if `expire_due` misses it, or if an `expired` record
/// authorises after a clock rollback.
#[test]
fn an_expired_grant_never_authorises_even_after_a_clock_rollback() {
    let tmp = Tmp::new("g2-expiry");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = variant(&net(ApprovalLifetime::Run), |r| {
        r.validity_seconds = Some(60)
    });
    let approval = env.pending(&req, None);
    clk.set(10);
    assert_eq!(
        env.svc().resolve(
            approval,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None
        ),
        Ok(ApprovalState::Approved)
    );
    assert_eq!(tmp.approval(approval).expires_at, Some(ts(70)));

    clk.set(69);
    assert!(is_authorised(&env.authorise(&req, None)), "one second left");
    clk.set(70);
    assert_eq!(
        mismatch(&env.authorise(&req, None)),
        Some(ScopeDimension::Expiry),
        "expires_at == now is expired"
    );
    clk.set(71);
    assert!(!is_authorised(&env.authorise(&req, None)), "now - 1 s");
    assert_eq!(env.svc().expire_due(), Ok(1));
    assert_eq!(tmp.approval(approval).status, ApprovalState::Expired);
    assert_eq!(env.svc().expire_due(), Ok(0));

    clk.set(0);
    assert!(
        !is_authorised(&env.authorise(&req, None)),
        "a rolled-back clock must not revive an expired record"
    );
    clk.set(10);
    assert!(!is_authorised(&env.authorise(&req, None)));
}

/// Attacks G13 at its edge: a validity of 0 s gives a grant whose end is its decision time, so it never
/// authorises (not even at the instant of the decision). Fails if a zero validity is treated as "none" (a
/// grant without end) or if the grant is usable for one instant.
#[test]
fn a_zero_second_validity_is_an_already_expired_grant() {
    let tmp = Tmp::new("g2-zero");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = variant(&net(ApprovalLifetime::Run), |r| {
        r.validity_seconds = Some(0)
    });
    let approval = env.pending(&req, None);
    assert_eq!(
        env.svc().resolve(
            approval,
            ApprovalDecision::ApproveOnce,
            Actor::LocalUserCli,
            None
        ),
        Ok(ApprovalState::Approved)
    );
    assert_eq!(tmp.approval(approval).expires_at, Some(ts(0)));
    assert_eq!(
        mismatch(&env.authorise(&req, None)),
        Some(ScopeDimension::Expiry)
    );
    clk.set(1);
    assert!(!is_authorised(&env.authorise(&req, None)));
}

/// Attacks G14 and the consumption guarantee (§5.4): a `once` grant authorises exactly once. Sequential
/// reuse on one writer gives `Mismatch { Consumption }`; a second writer on the same directory is refused
/// the lock while the first is open, and after the first closes it sees the consumption. Fails if the
/// second call authorises, if the consumption is not durable across a reopen, or if two writers can be open.
#[test]
fn a_consumed_once_grant_cannot_be_reused_on_the_same_store_or_after_reopen() {
    let tmp = Tmp::new("g2-consumed");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Once);
    let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);

    assert!(is_authorised(&env.authorise(&req, None)));
    assert!(tmp.approval(granted).consumed);
    let second = env.authorise(&req, None);
    assert_eq!(mismatch(&second), Some(ScopeDimension::Consumption));
    assert!(!is_authorised(&env.authorise(&req, None)));
    assert_ne!(
        env.svc().request(&req, None).expect("request"),
        RequestOutcome::Permitted
    );

    // A second writer cannot open the directory while the first one holds the lock.
    assert!(matches!(
        Env::try_open(&tmp.root, &clk, 0x20),
        Err(StoreError::Locked)
    ));
    drop(env);
    let mut again = Env::open(&tmp, &clk, 0x30);
    assert!(
        !is_authorised(&again.authorise(&req, None)),
        "the consumption must survive a reopen"
    );
    assert_eq!(
        tmp.approvals().iter().filter(|a| a.consumed).count(),
        1,
        "exactly one record was consumed"
    );
}

/// Attacks G14 under contention: eight threads race to authorise one `once` grant, each opening the
/// directory in turn (the data-directory lock serialises them). Exactly one gets `Authorised`. Fails if the
/// consumption is not written before `Authorised` is returned, or if the lock lets two writers work at once.
#[test]
fn racing_writers_authorise_a_once_grant_exactly_once() {
    let tmp = Tmp::new("g2-race");
    let clk = Clk::new();
    let req = net(ApprovalLifetime::Once);
    {
        let mut env = Env::real(&tmp, &clk);
        env.grant(&req, ApprovalDecision::ApproveOnce, None);
    }
    let authorised = AtomicU32::new(0);
    let finished = AtomicU32::new(0);
    std::thread::scope(|scope| {
        for worker in 0..8u8 {
            let (root, clk, req) = (&tmp.root, &clk, &req);
            let (authorised, finished) = (&authorised, &finished);
            scope.spawn(move || {
                for _ in 0..4000 {
                    match Env::try_open(root, clk, 0x40 + worker) {
                        Ok(mut env) => {
                            if is_authorised(&env.authorise(req, None)) {
                                authorised.fetch_add(1, Ordering::SeqCst);
                            }
                            finished.fetch_add(1, Ordering::SeqCst);
                            return;
                        }
                        Err(StoreError::Locked) => {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        Err(other) => panic!("unexpected open failure: {other:?}"),
                    }
                }
            });
        }
    });
    assert!(finished.load(Ordering::SeqCst) >= 1, "no writer got in");
    assert_eq!(
        authorised.load(Ordering::SeqCst),
        1,
        "exactly one authorisation"
    );
    assert_eq!(tmp.approvals().iter().filter(|a| a.consumed).count(), 1);
}

/// Attacks R7 and rule 13 end to end: a denied request stays refused for the identical request (also after
/// a reopen), a changed request is asked afresh, and a denial cannot be decided again or revoked. Fails if
/// the identical request ever reaches `Pending` or `Authorised` again, or if the changed request is refused
/// by the old denial.
#[test]
fn a_denied_request_stays_refused_and_cannot_be_redecided() {
    let tmp = Tmp::new("g2-denied");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Once);
    let denied = env.pending(&req, None);
    assert_eq!(
        env.svc()
            .resolve(denied, ApprovalDecision::Deny, Actor::LocalUserCli, None),
        Ok(ApprovalState::Denied)
    );
    let before = store_bytes(&tmp);
    let refused = RefusalReason::PreviouslyDenied(denied);
    assert_eq!(
        env.svc().request(&req, None),
        Ok(RequestOutcome::Refused(refused))
    );
    assert_eq!(env.authorise(&req, None), Ok(AuthOutcome::Refused(refused)));
    assert_eq!(store_bytes(&tmp), before, "a refusal writes nothing");

    for decision in [
        ApprovalDecision::ApproveOnce,
        ApprovalDecision::ApproveWorkflowScope,
        ApprovalDecision::Deny,
    ] {
        assert_eq!(
            env.svc()
                .resolve(denied, decision, Actor::LocalUserTui, Some(REV)),
            Err(ApprovalError::NotPending),
            "{decision:?}"
        );
    }
    assert_eq!(
        env.svc().revoke(denied, Actor::LocalUserCli),
        Err(ApprovalError::NotRevocable)
    );
    assert_eq!(store_bytes(&tmp), before);

    // A changed request is a different operation and is asked afresh.
    let changed = variant(&req, |r| r.arguments = vec![arg("branch", "develop")]);
    assert!(matches!(
        env.authorise(&changed, None),
        Ok(AuthOutcome::NeedsApproval(NeedReason::NoGrant, _))
    ));

    // The denial survives a reopen.
    drop(env);
    let mut again = Env::open(&tmp, &clk, 0x50);
    assert_eq!(
        again.authorise(&req, None),
        Ok(AuthOutcome::Refused(refused))
    );
}

/// Attacks rule 13 and R8: a pending request is never a grant. Repeated `authorise` calls only answer
/// `AlreadyPending` and write nothing; a revoke of a pending record is refused; workflow scope for an
/// unbound pending request is unavailable. Fails if a pending record ever authorises or if repeated calls
/// write more records or events.
#[test]
fn a_pending_request_never_authorises_however_often_it_is_presented() {
    let tmp = Tmp::new("g2-pending");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let req = net(ApprovalLifetime::Once);
    let pending = env.pending(&req, None);
    let before = store_bytes(&tmp);
    for _ in 0..5 {
        assert_eq!(
            env.authorise(&req, None),
            Ok(AuthOutcome::NeedsApproval(
                NeedReason::AlreadyPending(pending),
                pending
            ))
        );
        assert_eq!(
            env.svc().request(&req, None),
            Ok(RequestOutcome::Pending(pending))
        );
    }
    assert_eq!(store_bytes(&tmp), before);
    assert_eq!(
        env.svc().revoke(pending, Actor::LocalUserCli),
        Err(ApprovalError::NotRevocable)
    );
    assert_eq!(
        env.svc().resolve(
            pending,
            ApprovalDecision::ApproveWorkflowScope,
            Actor::LocalUserCli,
            Some(REV)
        ),
        Err(ApprovalError::WorkflowScopeUnavailable),
        "an unbound request has no workflow scope"
    );
    assert_eq!(store_bytes(&tmp), before);
    assert_eq!(tmp.approval(pending).status, ApprovalState::Pending);
}

/// Attacks rule 13 for revocation: a revoked grant of every lifetime never authorises again, cannot be
/// revived by a resolve, and cannot be revoked twice; an `expired` or `denied` record is not revocable.
/// Fails if any revoked grant authorises, or if a terminal record changes state.
#[test]
fn a_revoked_grant_stays_dead_in_every_lifetime() {
    let tmp = Tmp::new("g2-revoked");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let cases: Vec<(&str, PermissionRequest, ApprovalDecision, Option<u32>)> = vec![
        (
            "once",
            net(ApprovalLifetime::Once),
            ApprovalDecision::ApproveOnce,
            None,
        ),
        (
            "run",
            net(ApprovalLifetime::Run),
            ApprovalDecision::ApproveOnce,
            None,
        ),
        (
            "workflow",
            net(ApprovalLifetime::WorkflowRevision),
            ApprovalDecision::ApproveWorkflowScope,
            Some(REV),
        ),
    ];
    for (label, req, decision, rev) in cases {
        let granted = env.grant(&req, decision, rev);
        assert_eq!(
            env.svc().revoke(granted, Actor::LocalUserTui),
            Ok(()),
            "{label}"
        );
        assert_eq!(
            tmp.approval(granted).status,
            ApprovalState::Revoked,
            "{label}"
        );
        let outcome = env.authorise(&req, rev);
        assert!(!is_authorised(&outcome), "{label}");
        assert!(
            matches!(
                outcome,
                Ok(AuthOutcome::NeedsApproval(NeedReason::NoGrant, _))
            ),
            "{label}: {outcome:?}"
        );
        for decision in [ApprovalDecision::ApproveOnce, ApprovalDecision::Deny] {
            assert_eq!(
                env.svc()
                    .resolve(granted, decision, Actor::LocalUserCli, rev),
                Err(ApprovalError::NotPending),
                "{label}"
            );
        }
        assert_eq!(
            env.svc().revoke(granted, Actor::LocalUserCli),
            Err(ApprovalError::NotRevocable),
            "{label}"
        );
        // Revoking does not delete: the next request for it starts a fresh decision.
        let fresh = env.pending(&req, rev);
        assert_ne!(fresh, granted, "{label}");
        assert_eq!(env.svc().expire_due(), Ok(0));
    }
    // Expired records are not revocable either.
    let run_req = variant(&net(ApprovalLifetime::Run), |r| {
        r.run_id = Some(run(9));
        r.operation = OperationName::parse("git.other").unwrap();
    });
    let granted = env.grant(&run_req, ApprovalDecision::ApproveOnce, None);
    assert_eq!(env.svc().expire_run(run(9)), Ok(1));
    assert_eq!(
        env.svc().revoke(granted, Actor::LocalUserCli),
        Err(ApprovalError::NotRevocable)
    );
    assert_eq!(tmp.approval(granted).status, ApprovalState::Expired);
}

/// Attacks the run-end rule of the amendment (P03-D01 H2): `expire_run` ends `run` grants of that run only.
/// A workflow-scope grant stores no run and is deliberately NOT ended by it, and a `once` grant of the run
/// is untouched. Fails if `expire_run` ends a workflow-scope or `once` grant, or misses the run grant.
#[test]
fn expire_run_ends_the_run_grant_and_leaves_workflow_scope_and_once_grants() {
    let tmp = Tmp::new("g2-expire-run");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let run_req = net(ApprovalLifetime::Run);
    let workflow_req = variant(&net(ApprovalLifetime::WorkflowRevision), |r| {
        r.run_id = Some(run(1));
        r.operation = OperationName::parse("git.workflow").unwrap();
    });
    let once_req = variant(&net(ApprovalLifetime::Once), |r| {
        r.run_id = Some(run(1));
        r.operation = OperationName::parse("git.once").unwrap();
    });
    let run_grant = env.grant(&run_req, ApprovalDecision::ApproveOnce, None);
    let workflow_grant = env.grant(
        &workflow_req,
        ApprovalDecision::ApproveWorkflowScope,
        Some(REV),
    );
    let once_grant = env.grant(&once_req, ApprovalDecision::ApproveOnce, None);
    assert_eq!(tmp.approval(workflow_grant).run_id, None, "D01 A1");

    assert_eq!(env.svc().expire_run(run(1)), Ok(1));
    assert_eq!(tmp.approval(run_grant).status, ApprovalState::Expired);
    assert_eq!(tmp.approval(workflow_grant).status, ApprovalState::Approved);
    assert_eq!(tmp.approval(once_grant).status, ApprovalState::Approved);
    assert!(!is_authorised(&env.authorise(&run_req, None)));
    // The workflow-scope grant still serves a request that names the ended run (it is not tied to a run).
    assert!(is_authorised(&env.authorise(&workflow_req, Some(REV))));
    assert!(is_authorised(&env.authorise(&once_req, None)));
}

/// Attacks rule 13/G1 at the store level: an approved grant whose record a hand edit turns into any other
/// status never authorises (`cancelled` has no service path at all and exists only through such an edit).
/// Fails if a record of status pending, denied, cancelled, revoked or expired authorises, or if `denied`
/// and `pending` stop giving their R7/R8 answers.
#[test]
fn no_status_other_than_approved_authorises() {
    for status in ["pending", "denied", "cancelled", "revoked", "expired"] {
        let tmp = Tmp::new("g2-status");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
        tmp.tamper(|doc| {
            let record = &mut doc["approvals"][0];
            record["status"] = serde_json::json!(status);
            if status == "pending" {
                record["decided_at"] = serde_json::Value::Null;
            }
        });
        let outcome = env.authorise(&req, None);
        assert!(!is_authorised(&outcome), "{status}");
        match status {
            "denied" => assert_eq!(
                outcome,
                Ok(AuthOutcome::Refused(RefusalReason::PreviouslyDenied(
                    granted
                )))
            ),
            "pending" => assert_eq!(
                outcome,
                Ok(AuthOutcome::NeedsApproval(
                    NeedReason::AlreadyPending(granted),
                    granted
                ))
            ),
            _ => assert!(
                matches!(
                    outcome,
                    Ok(AuthOutcome::NeedsApproval(NeedReason::NoGrant, _))
                ),
                "{status}: {outcome:?}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Group 3 - refusals

/// Attacks R1 (rule 16): a request for raw secret values is refused under every grant lifetime, alone or
/// mixed with a category, even when a grant for the same operation without the secret exists, and writes
/// nothing. The pure evaluator refuses it for every capability. Fails if any combination reaches `Pending`
/// or `Authorised`, or writes a byte.
#[test]
fn raw_secret_values_are_refused_under_every_lifetime_and_capability() {
    let tmp = Tmp::new("g3-raw");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    for (lifetime, rev, decision) in [
        (ApprovalLifetime::Once, None, ApprovalDecision::ApproveOnce),
        (ApprovalLifetime::Run, None, ApprovalDecision::ApproveOnce),
        (
            ApprovalLifetime::WorkflowRevision,
            Some(REV),
            ApprovalDecision::ApproveWorkflowScope,
        ),
    ] {
        // One operation name per lifetime: a `once` grant also serves a `run` request of the same
        // operation (the lifetime of the REQUEST is not a scope dimension), which would end the test early.
        let clean = variant(&net(lifetime), |r| {
            r.operation =
                OperationName::parse(&format!("git.raw-{lifetime:?}").to_lowercase()).unwrap();
        });
        let granted = env.grant(&clean, decision, rev);
        let before = store_bytes(&tmp);
        for (label, data) in [
            ("alone", vec![RequestedData::RawSecretValues]),
            (
                "mixed",
                vec![
                    RequestedData::Category(DataCategory::RepositoryMetadata),
                    RequestedData::RawSecretValues,
                ],
            ),
            (
                "twice",
                vec![
                    RequestedData::RawSecretValues,
                    RequestedData::RawSecretValues,
                ],
            ),
        ] {
            let req = variant(&clean, |r| r.data = data);
            let refused = RefusalReason::RawSecretRequested;
            assert_eq!(
                env.svc().request(&req, rev),
                Ok(RequestOutcome::Refused(refused)),
                "{lifetime:?} {label}"
            );
            assert_eq!(
                env.authorise(&req, rev),
                Ok(AuthOutcome::Refused(refused)),
                "{lifetime:?} {label}"
            );
            assert_eq!(store_bytes(&tmp), before, "{lifetime:?} {label}");
        }
        assert!(!tmp.approval(granted).consumed);
    }
    // Pure evaluator, every capability (the deferred ones included: R1 is checked before anything else).
    let ctx = env.ctx(None);
    for capability in Capability::ALL {
        let req = variant(&net(ApprovalLifetime::Once), |r| {
            r.capability = capability;
            r.data.push(RequestedData::RawSecretValues);
        });
        assert_eq!(
            evaluate(&req, &tmp.approvals(), &ctx),
            Evaluation::Refused(RefusalReason::RawSecretRequested),
            "{capability:?}"
        );
    }
}

/// Attacks §5.4 amendment/D18-06a: the deferred capabilities are refused for every lifetime and for both
/// entry points before anything is written, even when a record for them is planted in the store by hand.
/// Fails if a deferred capability reaches `Pending`, `Authorised` or a store error.
#[test]
fn deferred_capabilities_are_refused_for_every_lifetime_and_even_with_a_planted_grant() {
    let tmp = Tmp::new("g3-deferred");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let seed = net(ApprovalLifetime::Run);
    env.grant(&seed, ApprovalDecision::ApproveOnce, None);
    // Plant: the approved GitAcquire record now claims a deferred capability (the store may or may not
    // accept it; the service must refuse before it matters).
    tmp.tamper(|doc| {
        doc["approvals"][0]["capability"] = serde_json::json!("documentation_fetch");
    });
    let before = store_bytes(&tmp);
    for capability in [
        Capability::DocumentationFetch,
        Capability::EmbeddingTransmission,
        Capability::NonMutatingVerification,
    ] {
        for (lifetime, rev) in [
            (ApprovalLifetime::Once, None),
            (ApprovalLifetime::Run, None),
            (ApprovalLifetime::WorkflowRevision, Some(REV)),
        ] {
            let req = variant(&net(lifetime), |r| r.capability = capability);
            let refused = RefusalReason::InvalidRequest;
            assert_eq!(
                env.svc().request(&req, rev),
                Ok(RequestOutcome::Refused(refused)),
                "{capability:?} {lifetime:?}"
            );
            assert_eq!(
                env.authorise(&req, rev),
                Ok(AuthOutcome::Refused(refused)),
                "{capability:?} {lifetime:?}"
            );
        }
    }
    assert_eq!(store_bytes(&tmp), before);
}

/// Attacks R5/D18-10/G11: `model_transmission` without an inference scope, and any other capability with
/// one, is refused; a scope of a different model, source set or redaction version does not use a grant for
/// another scope; a request without a data category is refused before any write (D11-16). Fails if a
/// changed scope is authorised or an invalid combination reaches the store.
#[test]
fn model_transmission_is_bound_to_its_inference_scope() {
    let tmp = Tmp::new("g3-model");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);

    let without = model(None);
    assert_eq!(
        env.authorise(&without, None),
        Ok(AuthOutcome::Refused(RefusalReason::InferenceScope))
    );
    let non_model_with_scope = variant(&net(ApprovalLifetime::Once), |r| {
        r.inference_scope = Some(scope());
    });
    assert_eq!(
        env.svc().request(&non_model_with_scope, None),
        Ok(RequestOutcome::Refused(RefusalReason::InferenceScope))
    );
    let no_data = variant(&model(Some(scope())), |r| r.data.clear());
    let before = store_bytes(&tmp);
    assert_eq!(
        env.authorise(&no_data, None),
        Ok(AuthOutcome::Refused(RefusalReason::InvalidRequest))
    );
    assert_eq!(store_bytes(&tmp), before, "nothing is written for D11-16");

    // A changed scope never uses a grant for another scope. One fresh store per variant: a pending request
    // for one scope answers every later request of the same operation (see the R7/R8 observation below).
    for (label, changed) in [
        (
            "model id",
            scope_with("example/other-model", &[SOURCE_A], "2026.10-a"),
        ),
        (
            "source set",
            scope_with("example/model", &[SOURCE_B], "2026.10-a"),
        ),
        (
            "extra source",
            scope_with("example/model", &[SOURCE_A, SOURCE_B], "2026.10-a"),
        ),
        ("no source", scope_with("example/model", &[], "2026.10-a")),
        (
            "redaction version",
            scope_with("example/model", &[SOURCE_A], "2026.11-a"),
        ),
    ] {
        let tmp = Tmp::new("g3-model-scope");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let granted_request = model(Some(scope()));
        let granted = env.grant(&granted_request, ApprovalDecision::ApproveOnce, None);
        assert_eq!(tmp.approval(granted).inference_scope, Some(scope()));
        assert!(
            is_authorised(&env.authorise(&granted_request, None)),
            "{label}: control"
        );
        let req = model(Some(changed));
        let outcome = assert_not_authorised(&mut env, &tmp, label, &req, None);
        assert_eq!(
            mismatch(&Ok(outcome)),
            Some(ScopeDimension::InferenceScope),
            "{label}"
        );
    }
}

/// Recorded observation on the spec as written (not a security defect; it fails closed): the fingerprint
/// excludes the inference scope and rows R7 (denied) and R8 (pending) compare capability, fingerprint,
/// workflow, run (and lifetime for R8) but not the scope. So a pending request for scope A answers an
/// otherwise identical request for scope B with `AlreadyPending(A)` and B is not recorded, and a denial of
/// scope A refuses scope B permanently (`PreviouslyDenied`). Nothing is ever authorised by it. Pinned so
/// that a decision to add the scope to R7/R8 changes this test consciously. Fails when R7/R8 start to
/// distinguish scopes.
#[test]
fn r7_and_r8_do_not_distinguish_inference_scopes_recorded_observation() {
    let tmp = Tmp::new("g3-model-r78");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let a = model(Some(scope()));
    let b = model(Some(scope_with(
        "example/other-model",
        &[SOURCE_A],
        "2026.10-a",
    )));
    let pending_a = env.pending(&a, None);
    assert_eq!(
        env.svc().request(&b, None),
        Ok(RequestOutcome::Pending(pending_a))
    );
    assert_eq!(tmp.approvals().len(), 1, "scope B was not recorded");
    assert_eq!(tmp.approval(pending_a).inference_scope, Some(scope()));
    assert_eq!(
        env.svc()
            .resolve(pending_a, ApprovalDecision::Deny, Actor::LocalUserCli, None),
        Ok(ApprovalState::Denied)
    );
    assert_eq!(
        env.svc().request(&b, None),
        Ok(RequestOutcome::Refused(RefusalReason::PreviouslyDenied(
            pending_a
        )))
    );

    // The same blindness hides a VALID grant: with a grant for scope A, a request for scope B is recorded
    // as pending, and until it is decided the grant for A is answered with `AlreadyPending(B)`; once B is
    // denied, A is refused as `PreviouslyDenied(B)` (R7 runs before the grant search, as the table orders
    // it). Fail-closed, but a switch of model can make a granted operation unusable (owner question
    // B1/H3 of P03-D01, here with the scope as the distinguishing field).
    let tmp = Tmp::new("g3-model-shadow");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let granted = env.grant(&a, ApprovalDecision::ApproveOnce, None);
    assert!(is_authorised(&env.authorise(&a, None)));
    let outcome = env.authorise(&b, None);
    let pending_b = match outcome {
        Ok(AuthOutcome::NeedsApproval(
            NeedReason::Mismatch {
                approval_id,
                dimension: ScopeDimension::InferenceScope,
            },
            pending,
        )) if approval_id == granted => pending,
        other => panic!("expected a scope mismatch, got {other:?}"),
    };
    assert_eq!(
        env.authorise(&a, None),
        Ok(AuthOutcome::NeedsApproval(
            NeedReason::AlreadyPending(pending_b),
            pending_b
        ))
    );
    assert_eq!(
        env.svc()
            .resolve(pending_b, ApprovalDecision::Deny, Actor::LocalUserCli, None),
        Ok(ApprovalState::Denied)
    );
    assert_eq!(
        env.authorise(&a, None),
        Ok(AuthOutcome::Refused(RefusalReason::PreviouslyDenied(
            pending_b
        )))
    );
}

/// Attacks C6 wildcard/empty rows and R2: no wildcard or empty destination can be formed, and a networked
/// capability without a destination (or a local one with one) is refused without a write. Fails if any
/// wildcard spelling parses, if an empty destination list is accepted for a networked capability, or if a
/// local capability may carry a destination.
#[test]
fn wildcard_and_empty_destinations_cannot_form_a_request() {
    for (input, expected) in [
        ("*", DestinationError::Scheme),
        ("", DestinationError::Scheme),
        (" ", DestinationError::Scheme),
        ("https://*", DestinationError::Wildcard),
        ("https://*.example.com", DestinationError::Wildcard),
        ("https://exa*mple.com", DestinationError::Wildcard),
        ("https://example.*", DestinationError::Wildcard),
        ("https://example.com:*", DestinationError::Wildcard),
        ("https://*/path", DestinationError::Wildcard),
        ("https://", DestinationError::Syntax),
        ("https:///path", DestinationError::Syntax),
        ("https://:443", DestinationError::Syntax),
        ("https://.", DestinationError::Syntax),
    ] {
        assert_eq!(
            NormalisedDestination::parse(input),
            Err(expected),
            "{input:?}"
        );
    }
    // A `*` in the resource path is a literal path character, never a host wildcard: it stays a distinct,
    // exact destination.
    let star = dest("https://example.com/*");
    assert_eq!(star.host(), "example.com");
    assert_eq!(star.canonical(), "https://example.com:443/*");
    assert_ne!(star, dest("https://example.com/other"));
    assert_ne!(star, dest("https://example.com"));

    let tmp = Tmp::new("g3-dest");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let before = store_bytes(&tmp);
    for capability in [
        Capability::GitAcquire,
        Capability::GitUpdate,
        Capability::ExternalReference,
        Capability::SchemaFetch,
        Capability::VulnerabilityLookup,
    ] {
        let req = variant(&net(ApprovalLifetime::Once), |r| {
            r.capability = capability;
            r.destinations.clear();
        });
        assert_eq!(
            env.svc().request(&req, None),
            Ok(RequestOutcome::Refused(RefusalReason::MissingDestination)),
            "{capability:?}"
        );
        assert_eq!(
            env.authorise(&req, None),
            Ok(AuthOutcome::Refused(RefusalReason::MissingDestination)),
            "{capability:?}"
        );
    }
    for capability in [Capability::LocalRender, Capability::AppStorage] {
        let req = variant(&net(ApprovalLifetime::Once), |r| r.capability = capability);
        assert_eq!(
            env.authorise(&req, None),
            Ok(AuthOutcome::Refused(RefusalReason::UnexpectedDestination)),
            "{capability:?}"
        );
    }
    assert_eq!(store_bytes(&tmp), before);
}

/// Attacks the argument and operation name rows (§5.2, R6): names with path traversal, NUL, control
/// characters, shell syntax, upper case, non-ASCII, 65 bytes or nothing never form a name; 64 bytes do.
/// Duplicate names (equal or different values) and an oversized value are refused as `InvalidRequest`
/// without a write, and boundary-shifting splits of one value into two arguments give a different
/// fingerprint. Fails if a bad name parses, a duplicate or oversized argument is recorded, or two different
/// argument lists share a fingerprint.
#[test]
fn argument_names_cannot_carry_injection_and_lists_cannot_collide() {
    let long_ok = "a".repeat(64);
    let long_bad = "a".repeat(65);
    let long_bad_char = format!("{}!", "a".repeat(64));
    for (input, expected) in [
        ("", Err(ArgError::Empty)),
        ("../etc/passwd", Err(ArgError::Character)),
        ("..", Ok(())),
        ("a/b", Err(ArgError::Character)),
        ("a\\b", Err(ArgError::Character)),
        ("a\u{0}b", Err(ArgError::Character)),
        ("a\nb", Err(ArgError::Character)),
        ("a\tb", Err(ArgError::Character)),
        ("a\u{7f}b", Err(ArgError::Character)),
        ("a b", Err(ArgError::Character)),
        ("a=b", Err(ArgError::Character)),
        ("$(id)", Err(ArgError::Character)),
        ("`id`", Err(ArgError::Character)),
        ("a;b", Err(ArgError::Character)),
        ("Branch", Err(ArgError::Character)),
        ("br\u{e4}nch", Err(ArgError::Character)),
        ("a\u{200b}b", Err(ArgError::Character)),
        (long_ok.as_str(), Ok(())),
        (long_bad.as_str(), Err(ArgError::TooLong)),
        (long_bad_char.as_str(), Err(ArgError::TooLong)),
    ] {
        assert_eq!(
            ArgName::parse(input).map(|_| ()),
            expected,
            "argument name {input:?}"
        );
        assert_eq!(
            OperationName::parse(input).map(|_| ()),
            expected,
            "operation name {input:?}"
        );
    }

    let tmp = Tmp::new("g3-args");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let before = store_bytes(&tmp);
    let invalid = RefusalReason::InvalidRequest;
    let oversized = "v".repeat(65_537);
    for (label, arguments) in [
        (
            "duplicate equal",
            vec![arg("branch", "main"), arg("branch", "main")],
        ),
        (
            "duplicate different",
            vec![arg("branch", "main"), arg("branch", "evil")],
        ),
        ("oversized value", vec![arg("branch", oversized.as_str())]),
    ] {
        let req = variant(&net(ApprovalLifetime::Once), |r| r.arguments = arguments);
        assert_eq!(
            env.svc().request(&req, None),
            Ok(RequestOutcome::Refused(invalid)),
            "{label}"
        );
        assert_eq!(
            env.authorise(&req, None),
            Ok(AuthOutcome::Refused(invalid)),
            "{label}"
        );
    }
    assert_eq!(store_bytes(&tmp), before);

    // A value of exactly the bound is accepted (and only hashed, never stored).
    let at_bound = "v".repeat(65_536);
    let req = variant(&net(ApprovalLifetime::Once), |r| {
        r.arguments = vec![arg("branch", at_bound.as_str())];
    });
    let pending = env.pending(&req, None);
    assert!(
        !tmp.bytes("approvals.json")
            .windows(1024)
            .any(|w| w == &at_bound.as_bytes()[..1024]),
        "argument values are never persisted"
    );
    assert_eq!(tmp.approval(pending).status, ApprovalState::Pending);

    // Boundary shifting: the length prefixes keep these apart.
    let fp = |arguments: Vec<OperationArg>| {
        fingerprint(&variant(&net(ApprovalLifetime::Once), |r| {
            r.arguments = arguments
        }))
        .expect("fingerprint")
    };
    let digests = [
        fp(vec![arg("ab", "c")]),
        fp(vec![arg("a", "bc")]),
        fp(vec![arg("a", "b"), arg("c", "")]),
        fp(vec![arg("a", "b\u{0}c")]),
        fp(vec![arg("a", "b"), arg("c", "d")]),
        fp(vec![arg("a", "bcd")]),
        fp(vec![]),
    ];
    for (i, a) in digests.iter().enumerate() {
        for (j, b) in digests.iter().enumerate() {
            assert_eq!(i == j, a == b, "digests {i} and {j}");
        }
    }
    // Order of arguments does not matter; duplicates are an error whatever the values.
    assert_eq!(
        fp(vec![arg("a", "1"), arg("b", "2")]),
        fp(vec![arg("b", "2"), arg("a", "1")])
    );
    assert_eq!(
        fingerprint(&variant(&net(ApprovalLifetime::Once), |r| {
            r.arguments = vec![arg("a", "1"), arg("a", "2")];
        })),
        Err(ArgError::Duplicate)
    );
}

// ---------------------------------------------------------------------------------------------------
// Group 4 - seeded-key sweep

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect_files(&entry.path(), out);
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Every file under `root` (names and contents) that holds one of `needles`, as paths relative to `root`;
/// also returns how many files were visited.
fn sweep(root: &Path, needles: &[&[u8]]) -> (usize, Vec<String>) {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    let mut hits = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::read(file).unwrap();
        let named = needles.iter().any(|n| contains(relative.as_bytes(), n));
        if named || needles.iter().any(|n| contains(&bytes, n)) {
            hits.push(relative);
        }
    }
    (files.len(), hits)
}

fn needles() -> [&'static [u8]; 2] {
    [KEY.as_bytes(), KEY_FRAGMENT.as_bytes()]
}

/// Collects the compact and pretty `Debug` text of `value`.
fn debug_texts<T: std::fmt::Debug>(out: &mut Vec<String>, value: &T) {
    out.push(format!("{value:?}"));
    out.push(format!("{value:#?}"));
}

fn os(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}

fn pair(name: &str, value: &str) -> (OsString, OsString) {
    (OsString::from(name), OsString::from(value))
}

/// Self-check of the sweep (a sweep that cannot find a key proves nothing): a key planted in a file, in a
/// nested file name and in a nested file is found; a clean directory is clean. Fails if the sweep stops
/// walking subdirectories, ignores file names, or only matches the full key.
#[test]
fn the_sweep_finds_a_planted_key_and_a_truncated_leak() {
    let tmp = Tmp::new("g4-selfcheck");
    let (visited, hits) = sweep(&tmp.root, &needles());
    assert!(
        visited >= 2,
        "the initialised store has files, saw {visited}"
    );
    assert!(hits.is_empty());

    std::fs::create_dir_all(tmp.path("nested/deeper")).unwrap();
    std::fs::write(tmp.path("nested/deeper/data.bin"), format!("x{KEY}y")).unwrap();
    std::fs::write(tmp.path(&format!("nested/{KEY_FRAGMENT}.txt")), "clean").unwrap();
    std::fs::write(tmp.path("fragment.txt"), format!("..{KEY_FRAGMENT}..")).unwrap();
    let (_, hits) = sweep(&tmp.root, &needles());
    assert_eq!(hits.len(), 3, "{hits:?}");
}

/// Attacks KEY-01 across the whole application: a synthetic key is held in the session store, passed as the
/// value of an operation argument, placed in a host environment under innocent and key-like names, and put
/// into a `ToolDirs` path. The full lifecycle runs (request, resolve, authorise, revoke, expire, deny) and
/// every tool profile is built. Afterwards the key (and a fragment of it) appears in no file under the data
/// directory, in no `Debug` text of any public type touched, and in no child-environment name, value or
/// preview line. Fails if any layer writes, formats or forwards the key.
#[test]
fn a_seeded_key_reaches_no_file_no_debug_text_and_no_child_environment() {
    let tmp = Tmp::new("g4-sweep");
    let clk = Clk::new();
    let mut env = Env::real(&tmp, &clk);
    let mut debug: Vec<String> = Vec::new();

    // The key enters through the only reader and is held by the session store.
    let secret = read_key(&mut Cursor::new(format!("{KEY}\n")), false).expect("key accepted");
    debug_texts(&mut debug, &secret);
    let mut credentials = SessionCredentialStore::new();
    let status = credentials.set(secret);
    assert_eq!(
        format!("{credentials:?}"),
        "SessionCredentialStore { status: SessionOnly }"
    );
    debug_texts(&mut debug, &credentials);
    debug_texts(&mut debug, &status);
    let event = AppEvent::CredentialStatusChanged { status };
    let event_json = serde_json::to_string(&event).unwrap();
    assert!(!contains(event_json.as_bytes(), KEY.as_bytes()));
    debug_texts(&mut debug, &event);
    // A rejected read (the key with trailing junk) fails without echoing anything.
    let rejected = read_key(&mut Cursor::new(format!("{KEY} not-a-key\n")), false);
    assert_eq!(rejected.as_ref().err(), Some(&KeyFormatError::Character));
    debug_texts(&mut debug, &rejected);
    assert_eq!(
        read_key(&mut Cursor::new(KEY.to_owned()), true).err(),
        Some(KeyFormatError::TerminalInput)
    );

    // The key as the value of an argument, through the whole approval lifecycle.
    let seeded = |lifetime, operation: &str| {
        variant(&net(lifetime), |r| {
            r.operation = OperationName::parse(operation).unwrap();
            r.arguments = vec![arg("note", KEY), arg("branch", "main")];
        })
    };
    let once = seeded(ApprovalLifetime::Once, "git.once");
    let run_req = seeded(ApprovalLifetime::Run, "git.run");
    let workflow_req = seeded(ApprovalLifetime::WorkflowRevision, "git.workflow");
    let denied_req = seeded(ApprovalLifetime::Once, "git.denied");
    let mut timed = seeded(ApprovalLifetime::Run, "git.timed");
    timed.validity_seconds = Some(30);
    debug_texts(&mut debug, &once);
    debug_texts(&mut debug, &once.arguments);
    debug_texts(&mut debug, &fingerprint(&once));

    let g_once = env.grant(&once, ApprovalDecision::ApproveOnce, None);
    let g_run = env.grant(&run_req, ApprovalDecision::ApproveOnce, None);
    let g_workflow = env.grant(
        &workflow_req,
        ApprovalDecision::ApproveWorkflowScope,
        Some(REV),
    );
    let denied = env.pending(&denied_req, None);
    debug_texts(
        &mut debug,
        &env.svc()
            .resolve(denied, ApprovalDecision::Deny, Actor::LocalUserTui, None),
    );
    for req in [&once, &run_req, &workflow_req, &denied_req, &timed] {
        let rev = req.workflow.map(|_| REV);
        let outcome = env.authorise(req, rev);
        debug_texts(&mut debug, &outcome);
        debug_texts(&mut debug, &env.svc().request(req, rev));
        debug_texts(&mut debug, &evaluate(req, &tmp.approvals(), &env.ctx(rev)));
    }
    clk.set(5);
    let timed_id = env.grant(&timed, ApprovalDecision::ApproveOnce, None);
    clk.set(60);
    assert_eq!(env.svc().expire_due(), Ok(1));
    assert_eq!(env.svc().revoke(g_run, Actor::LocalUserCli), Ok(()));
    debug_texts(&mut debug, &env.svc().revoke(g_run, Actor::LocalUserCli));
    assert_eq!(env.svc().expire_run(run(1)), Ok(0));
    assert_eq!(tmp.approval(timed_id).status, ApprovalState::Expired);
    assert_eq!(tmp.approval(g_once).status, ApprovalState::Approved);
    assert_eq!(tmp.approval(g_workflow).status, ApprovalState::Approved);
    for approval in tmp.approvals() {
        debug_texts(&mut debug, &approval);
    }
    for event in tmp.events() {
        debug_texts(&mut debug, &event);
    }
    for error in [
        ApprovalError::NotFound,
        ApprovalError::NotPending,
        ApprovalError::NotRevocable,
        ApprovalError::WorkflowScopeUnavailable,
        ApprovalError::Fingerprint(ArgError::Duplicate),
        ApprovalError::Emit,
    ] {
        debug.push(error.to_string());
    }
    drop(env);

    // CLI entry point over the same directory (read-only list).
    let listed = approvals::list(&tmp.root).expect("list");
    debug_texts(&mut debug, &listed);

    // The key in a host environment, under innocent and key-like names, plus awkward encodings.
    let embedded = format!("see {KEY} here");
    let multi_line = format!("line1\n{KEY}\nline3");
    let mut pairs = vec![
        pair("TZ", KEY),
        pair("EDITOR", KEY),
        pair("PROJECT_NOTES", &embedded),
        pair("LC_ALL", KEY),
        pair("PATH", &format!("/x:{KEY}")),
        pair("HOME", KEY),
        pair("LANG", KEY),
        pair("TMPDIR", KEY),
        pair("MULTI", &multi_line),
        pair("A=B", KEY),
        pair("OPENROUTER_API_KEY", KEY),
        pair("openrouter_api_key", KEY),
        pair("API_TOKEN", KEY),
        pair("Gh_Token", KEY),
        pair("GITHUB_TOKEN", KEY),
        pair("AWS_SECRET_ACCESS_KEY", KEY),
        pair("NPM_TOKEN", KEY),
        pair("SSH_AUTH_SOCK", KEY),
        pair("HTTPS_PROXY", KEY),
        pair("http_proxy", KEY),
        pair("GIT_ASKPASS", KEY),
        pair("LD_PRELOAD", KEY),
        pair("PYTHONPATH", KEY),
        pair("x_password", KEY),
        pair("session_id", KEY),
        pair("TZ", "UTC"),
    ];
    let mut raw_name = vec![0xff];
    raw_name.extend_from_slice(KEY.as_bytes());
    let mut raw_value = vec![0xfe];
    raw_value.extend_from_slice(KEY.as_bytes());
    pairs.push((os(&raw_name), os(&raw_value)));
    pairs.push((OsString::from("RAW_VALUE"), os(&raw_value)));
    let host = HostEnv::from_pairs(pairs);
    debug_texts(&mut debug, &host);

    let dirs = ToolDirs::new(
        Path::new("/w/home"),
        Path::new("/w/config"),
        Path::new("/w/cache"),
        Path::new("/w/tmp"),
    )
    .expect("dirs");
    for profile in ToolProfile::ALL {
        let child = build(profile, &dirs, &host, &credentials).expect("profile builds");
        debug_texts(&mut debug, &child);
        let preview = child.preview();
        debug_texts(&mut debug, &preview);
        for (name, value) in child.vars() {
            assert!(
                !contains(name.as_encoded_bytes(), KEY.as_bytes())
                    && !contains(name.as_encoded_bytes(), KEY_FRAGMENT.as_bytes()),
                "{profile:?}: a name holds the key"
            );
            assert!(
                !contains(value.as_encoded_bytes(), KEY.as_bytes())
                    && !contains(value.as_encoded_bytes(), KEY_FRAGMENT.as_bytes()),
                "{profile:?}: a value holds the key ({})",
                name.to_string_lossy()
            );
        }
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for (name, value) in child.vars() {
            seen.insert(
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            );
        }
        // Nothing from the host leaks into the fixed names, and nothing key-like exists at all.
        assert_eq!(seen.get("PATH").map(String::as_str), Some("/usr/bin:/bin"));
        assert_eq!(seen.get("LANG").map(String::as_str), Some("C.UTF-8"));
        for name in seen.keys() {
            let upper = name.to_ascii_uppercase();
            assert!(
                ![
                    "KEY", "TOKEN", "SECRET", "PASSW", "AUTH", "SESSION", "PROXY"
                ]
                .iter()
                .any(|part| upper.contains(part)),
                "{profile:?}: {name}"
            );
        }
        if profile == ToolProfile::Internal {
            // Positive control: inheritance works, so the dropped `TZ` below is not vacuous. The first
            // `TZ` entry of the host holds the key and wins (first occurrence), so none is inherited.
            assert_eq!(
                seen.get("TZ"),
                None,
                "first TZ holds the key and is dropped"
            );
        }
        assert!(
            preview
                .iter()
                .all(|line| !line.name.as_str().contains(KEY_FRAGMENT)),
            "{profile:?}"
        );
    }
    // Positive control for the inheritance path itself.
    let clean_host = HostEnv::from_pairs(vec![pair("TZ", "UTC"), pair("EDITOR", "vi")]);
    let internal = build(ToolProfile::Internal, &dirs, &clean_host, &credentials).expect("builds");
    let tz = internal
        .vars()
        .find(|(name, _)| *name == "TZ")
        .map(|(_, v)| v.to_string_lossy().into_owned());
    assert_eq!(tz.as_deref(), Some("UTC"));
    assert!(
        internal
            .preview()
            .iter()
            .any(|l| l.name.as_str() == "TZ" && l.origin == Origin::Inherited)
    );

    // The key in a ToolDirs path: the build fails closed for every profile that uses that directory (all use
    // `home`; `git` has no `tmp`). The error carries nothing, and neither does the `Debug` of the ToolDirs.
    let keyed = format!("/w/{KEY}");
    for (label, home, config, cache, tmpdir, unaffected) in [
        (
            "home",
            keyed.as_str(),
            "/w/config",
            "/w/cache",
            "/w/tmp",
            vec![],
        ),
        (
            "tmp",
            "/w/home",
            "/w/config",
            "/w/cache",
            keyed.as_str(),
            vec![ToolProfile::Git],
        ),
    ] {
        let dirs = ToolDirs::new(
            Path::new(home),
            Path::new(config),
            Path::new(cache),
            Path::new(tmpdir),
        )
        .expect("an absolute path is a valid ToolDirs");
        for profile in ToolProfile::ALL {
            let result = build(profile, &dirs, &host, &credentials);
            if unaffected.contains(&profile) {
                assert!(result.is_ok(), "{label} {profile:?}");
            } else {
                assert_eq!(
                    result.err(),
                    Some(EnvError::SecretValue),
                    "{label} {profile:?}"
                );
            }
        }
        debug_texts(
            &mut debug,
            &build(ToolProfile::Yamllint, &dirs, &host, &credentials).err(),
        );
        // P03-D02 B2: a ToolDirs over a key-bearing path is part of the Debug sweep.
        debug_texts(&mut debug, &dirs);
    }
    let under = ToolDirs::under(Path::new(&keyed), ToolProfile::Git).expect("under");
    debug_texts(&mut debug, &under);
    assert_eq!(
        build(ToolProfile::Git, &under, &host, &credentials).err(),
        Some(EnvError::SecretValue)
    );

    // Nothing anywhere holds the key: Debug texts, then every file of the data directory.
    for (index, text) in debug.iter().enumerate() {
        assert!(
            !text.contains(KEY) && !text.contains(KEY_FRAGMENT),
            "Debug/Display text #{index} holds the key"
        );
    }
    assert!(
        debug.len() > 60,
        "the sweep collected {} texts",
        debug.len()
    );
    let (visited, hits) = sweep(&tmp.root, &needles());
    assert!(visited >= 5, "visited {visited} files");
    assert!(hits.is_empty(), "the key reached files: {hits:?}");
    let names: Vec<_> = {
        let mut files = Vec::new();
        collect_files(&tmp.root, &mut files);
        files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    for expected in ["approvals.json", "log.json"] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} not visited"
        );
    }
    // The grants and denial are really in the store (the argument value was hashed, not dropped).
    assert!(tmp.approvals().len() >= 6);

    // The store really held the key throughout (so the clean sweep above is not vacuous); after `delete`
    // it recognises nothing.
    assert!(credentials.appears_in(KEY.as_bytes()));
    assert_eq!(
        credentials.delete(),
        domain::contract::CredentialStatus::Absent
    );
    assert!(!credentials.appears_in(KEY.as_bytes()));
}

// ---------------------------------------------------------------------------------------------------
// Group 5 - store tampering

/// Attacks D11-13 / rule 12 (only a decision makes a grant): a pending request that a hand edit marks
/// `approved` without a decision time makes the file invalid, so every entry point fails closed with a
/// store error and the CLI listing refuses it too; a `denied` or `approved` record without `decided_at`
/// fails the same way. Nothing is repaired or rewritten. Fails if such a file is accepted, or any call
/// returns `Authorised` or `Permitted`.
#[test]
fn a_pending_request_edited_to_approved_without_a_decision_fails_closed() {
    for (label, status) in [("approved", "approved"), ("denied", "denied")] {
        let tmp = Tmp::new("g5-nodecision");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        let pending = env.pending(&req, None);
        tmp.tamper(|doc| doc["approvals"][0]["status"] = serde_json::json!(status));
        let edited = tmp.bytes("approvals.json");

        let malformed = ApprovalError::Store(DevError::Malformed(StoreKind::Approvals));
        assert_eq!(env.authorise(&req, None), Err(malformed), "{label}");
        assert_eq!(env.svc().request(&req, None), Err(malformed), "{label}");
        assert_eq!(
            env.svc().resolve(
                pending,
                ApprovalDecision::ApproveOnce,
                Actor::LocalUserCli,
                None
            ),
            Err(malformed),
            "{label}"
        );
        assert_eq!(
            env.svc().revoke(pending, Actor::LocalUserCli),
            Err(malformed),
            "{label}"
        );
        assert_eq!(
            tmp.bytes("approvals.json"),
            edited,
            "{label}: nothing is repaired"
        );
        drop(env);
        assert_eq!(approvals::list(&tmp.root), Err(malformed), "{label}");
    }
}

/// Attacks D11-13/D11-12/D11-17 further: hand edits that break a store rule are refused whole, in the
/// direction that matters for authority. `consumed` on a `run` grant or on a pending record, a `once` grant
/// turned into `workflow_revision`/`run` lifetime without its binding, a wildcard destination, truncated
/// and empty files, and foreign JSON all give a store error and never an authorisation. Fails if any is
/// accepted as a grant.
#[test]
fn hand_edits_that_break_a_store_rule_are_never_a_grant() {
    type Edit = fn(&mut serde_json::Value);
    let edits: Vec<(&str, Edit)> = vec![
        ("consumed run grant", |d| {
            d["approvals"][0]["consumed"] = serde_json::json!(true);
        }),
        ("once turned into workflow_revision", |d| {
            d["approvals"][0]["lifetime"] = serde_json::json!("workflow_revision");
        }),
        ("run grant without run id", |d| {
            d["approvals"][0]["run_id"] = serde_json::Value::Null;
        }),
        ("wildcard destination", |d| {
            d["approvals"][0]["destinations"] = serde_json::json!(["*.github.com:443"]);
        }),
        ("empty destinations on a networked grant", |d| {
            d["approvals"][0]["destinations"] = serde_json::json!([]);
        }),
        ("unknown status", |d| {
            d["approvals"][0]["status"] = serde_json::json!("granted");
        }),
        ("extra field", |d| {
            d["approvals"][0]["bypass"] = serde_json::json!(true);
        }),
        ("raw secret category", |d| {
            d["approvals"][0]["data_categories"] = serde_json::json!(["raw_secret_values"]);
        }),
        ("foreign store type", |d| {
            d["store_type"] = serde_json::json!("log");
        }),
        ("future schema version", |d| {
            d["schema_version"] = serde_json::json!("9.9");
        }),
    ];
    for (label, edit) in edits {
        let tmp = Tmp::new("g5-edits");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        env.grant(&req, ApprovalDecision::ApproveOnce, None);
        tmp.tamper(edit);
        let edited = tmp.bytes("approvals.json");
        let outcome = env.authorise(&req, None);
        assert!(
            matches!(outcome, Err(ApprovalError::Store(_))),
            "{label}: {outcome:?}"
        );
        assert!(
            matches!(env.svc().request(&req, None), Err(ApprovalError::Store(_))),
            "{label}"
        );
        assert_eq!(
            tmp.bytes("approvals.json"),
            edited,
            "{label}: nothing is repaired"
        );
    }
    // File-level damage.
    for (label, content) in [
        ("empty", Vec::new()),
        (
            "truncated",
            b"{\"schema_version\": \"1.4\", \"store_t".to_vec(),
        ),
        ("not an object", b"[]".to_vec()),
        ("empty object", b"{}".to_vec()),
        ("binary", vec![0xff, 0xfe, 0x00, 0x01]),
    ] {
        let tmp = Tmp::new("g5-damage");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        env.grant(&req, ApprovalDecision::ApproveOnce, None);
        std::fs::write(tmp.path("approvals.json"), &content).unwrap();
        let outcome = env.authorise(&req, None);
        assert!(
            matches!(outcome, Err(ApprovalError::Store(_))),
            "{label}: {outcome:?}"
        );
        assert_eq!(tmp.bytes("approvals.json"), content, "{label}");
    }
}

/// Attacks G3/G7 to G10 (defence in depth, request §6): a valid-looking record whose operation fingerprint
/// still equals the request's but whose repositories, destinations, constraints or data categories were
/// edited does not authorise, and the reason names the edited dimension; a record whose fingerprint was
/// edited reports `Fingerprint`. Fails if the explicit set comparisons G7 to G10 are dropped (the
/// fingerprint alone would then authorise), or the reason is wrong.
#[test]
fn an_edited_scope_is_caught_by_the_explicit_set_checks_not_only_the_fingerprint() {
    type Edit = fn(&mut serde_json::Value);
    let cases: Vec<(&str, Edit, ScopeDimension)> = vec![
        (
            "repository",
            |d| {
                d["approvals"][0]["repo_ids"] =
                    serde_json::json!(["00000000-0000-4000-8000-0000000009ff"]);
            },
            ScopeDimension::Repos,
        ),
        (
            "destination",
            |d| {
                d["approvals"][0]["destinations"] =
                    serde_json::json!(["https://gitlab.com:443/example/repo"]);
            },
            ScopeDimension::Destinations,
        ),
        (
            "constraint",
            |d| {
                d["approvals"][0]["resource_constraints"] = serde_json::json!(["max_size_mb=999"]);
            },
            ScopeDimension::ResourceConstraints,
        ),
        (
            "data category",
            |d| {
                d["approvals"][0]["data_categories"] = serde_json::json!(["redacted_source"]);
            },
            ScopeDimension::DataCategories,
        ),
        (
            "fingerprint",
            |d| {
                d["approvals"][0]["operation_fingerprint"] = serde_json::json!(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                );
            },
            ScopeDimension::Fingerprint,
        ),
    ];
    for (label, edit, dimension) in cases {
        let tmp = Tmp::new("g5-scope");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
        tmp.tamper(edit);
        let outcome = env.authorise(&req, None);
        assert!(!is_authorised(&outcome), "{label}");
        assert!(
            matches!(
                outcome,
                Ok(AuthOutcome::NeedsApproval(NeedReason::Mismatch { approval_id, dimension: d }, _))
                    if approval_id == granted && d == dimension
            ),
            "{label}: {outcome:?}"
        );
    }
}

/// Recorded limitation (not a defect): the store has no integrity binding. A hand edit that keeps the file
/// valid is trusted at face value, because the schema and the semantic rules (D11-12/13) check consistency,
/// not provenance. (a) A pending record edited to `approved` WITH a decision time authorises although its
/// actor still reads `requester`; (b) a consumed `once` grant edited back to `consumed = false`
/// authorises again. The data directory is private to the user (0700, single-writer lock), so an attacker
/// who can edit the file already runs as the user; this test pins today's behaviour so that a later
/// hardening (for example a semantic rule "approved implies actor is not requester", or a MAC over the
/// ledger) must change it consciously. Fails when that behaviour changes either way.
#[test]
fn a_valid_looking_hand_edit_is_trusted_because_the_store_has_no_integrity_binding() {
    // (a) pending -> approved with decided_at.
    {
        let tmp = Tmp::new("g5-trust-a");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Run);
        let pending = env.pending(&req, None);
        tmp.tamper(|doc| {
            doc["approvals"][0]["status"] = serde_json::json!("approved");
            doc["approvals"][0]["decided_at"] = serde_json::json!("2027-01-15T08:00:10Z");
        });
        assert_eq!(tmp.approval(pending).actor.as_str(), "requester");
        match env.authorise(&req, None).expect("authorise") {
            AuthOutcome::Authorised(proof) => assert_eq!(proof.approval_id(), pending),
            other => panic!("pinned limitation (a) changed: {other:?}"),
        }
    }
    // (b) consumed once grant edited back to unconsumed.
    {
        let tmp = Tmp::new("g5-trust-b");
        let clk = Clk::new();
        let mut env = Env::real(&tmp, &clk);
        let req = net(ApprovalLifetime::Once);
        let granted = env.grant(&req, ApprovalDecision::ApproveOnce, None);
        assert!(is_authorised(&env.authorise(&req, None)));
        assert!(tmp.approval(granted).consumed);
        tmp.tamper(|doc| doc["approvals"][0]["consumed"] = serde_json::json!(false));
        match env.authorise(&req, None).expect("authorise") {
            AuthOutcome::Authorised(proof) => assert_eq!(proof.approval_id(), granted),
            other => panic!("pinned limitation (b) changed: {other:?}"),
        }
        // The reverse edit fails closed: marking an unconsumed once grant consumed ends it.
        let req2 = variant(&req, |r| {
            r.operation = OperationName::parse("git.second").unwrap()
        });
        let second = env.grant(&req2, ApprovalDecision::ApproveOnce, None);
        tmp.tamper(|doc| {
            let record = doc["approvals"]
                .as_array_mut()
                .expect("array")
                .iter_mut()
                .find(|r| r["approval_id"] == serde_json::json!(second.to_string()))
                .expect("record");
            record["consumed"] = serde_json::json!(true);
        });
        assert_eq!(
            mismatch(&env.authorise(&req2, None)),
            Some(ScopeDimension::Consumption)
        );
    }
}

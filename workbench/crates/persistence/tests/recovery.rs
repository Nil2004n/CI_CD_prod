//! P02-T10 tests for startup reconciliation (`StoreWriter::open`, request §6.5, T01-D8): interrupted runs,
//! pending approvals, leftover temp files, missing and unreferenced reports, malformed roots, and the
//! failure behaviour of the reconciliation write.
//!
//! The validator is a fake. Fixtures are synthetic. Every test uses its own fresh directory under
//! `CARGO_TARGET_TMPDIR`, removed on drop. For each assertion that matters the comment says what change in
//! the implementation would make it fail.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use domain::ids::{ReportId, RunId};
use domain::store::{AnyStore, StoreKind};
use domain::time::UtcTimestamp;
use persistence::error::{StoreError, WriteStep};
use persistence::fault::{FaultFs, FaultPlan};
use persistence::fs::RealFs;
use persistence::read::decode;
use persistence::recovery::ReconcileReport;
use persistence::validate::{
    DocumentValidator, Layer, Problem, ProblemCode, Problems, SchemaKeyword, Verdict,
};
use persistence::writer::StoreWriter;
use serde_json::{Value, json};

const TS: &str = "2026-10-16T05:25:00Z";
const TS_START: &str = "2026-10-16T05:30:00Z";
const TS_DONE: &str = "2026-10-16T06:00:00Z";
const FUTURE: &str = "2999-01-01T00:00:00Z";
const FUTURE_CHECKPOINT: &str = "2998-06-01T12:00:00Z";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------------------------------------------
// helpers

struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("recovery-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        set_mode(&root, 0o700);
        Self { root }
    }

    fn data_dir(&self) -> persistence::paths::DataDir {
        persistence::paths::DataDir::new(&RealFs, &self.root).unwrap()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn private_dir(&self, rel: &str) {
        std::fs::create_dir_all(self.path(rel)).unwrap();
        set_mode(&self.path(rel), 0o700);
    }

    fn write(&self, rel: &str, bytes: &[u8]) {
        std::fs::write(self.path(rel), bytes).unwrap();
    }

    fn write_json(&self, rel: &str, doc: &Value) {
        self.write(rel, &serde_json::to_vec_pretty(doc).unwrap());
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    fn read_json(&self, rel: &str) -> Value {
        serde_json::from_slice(&self.read(rel)).unwrap()
    }

    /// Whether `rel` exists without following a final symlink.
    fn exists_nofollow(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.path(rel)).is_ok()
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// Relative path -> content for every entry under `root` except the lock file; directories as `<path>/`,
/// symlinks as `<path> -> <target>` (never followed).
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel == ".app.lock" {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_symlink() {
                let target = std::fs::read_link(&path).unwrap();
                out.insert(format!("{rel} -> {}", target.display()), Vec::new());
            } else if meta.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(base, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn run_id(n: u32) -> RunId {
    RunId::parse(&uid(n)).unwrap()
}

fn report_id(n: u32) -> ReportId {
    ReportId::parse(&uid(n)).unwrap()
}

// --- fake validator -------------------------------------------------------------------------------

const MODE_VALID: u8 = 0;
const MODE_UNAVAILABLE: u8 = 2;
/// Rejects a `log` document that contains an `interrupted` run (the result of reconciliation).
const MODE_NO_INTERRUPTED_RUNS: u8 = 4;

#[derive(Debug, Default)]
struct Switch {
    mode: AtomicU8,
}

impl Switch {
    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }
}

fn marker_problems() -> Problems {
    Problems::one(Problem::new(
        Layer::Semantic,
        ProblemCode::Keyword(SchemaKeyword::Other),
        "/runs",
        None,
    ))
}

impl DocumentValidator for Switch {
    fn validate(&self, kind: StoreKind, doc: &Value) -> Verdict {
        match self.mode.load(Ordering::SeqCst) {
            MODE_UNAVAILABLE => Verdict::SchemaUnavailable,
            MODE_NO_INTERRUPTED_RUNS
                if kind == StoreKind::Log
                    && doc["runs"].as_array().is_some_and(|runs| {
                        runs.iter().any(|r| r["execution"] == "interrupted")
                    }) =>
            {
                Verdict::Invalid(marker_problems())
            }
            _ => Verdict::Valid,
        }
    }
}

fn valid() -> Arc<Switch> {
    Arc::new(Switch::default())
}

fn open(tmp: &Tmp, v: &Arc<Switch>) -> Result<(StoreWriter, ReconcileReport), StoreError> {
    StoreWriter::open(tmp.data_dir(), Arc::new(RealFs), v.clone())
}

/// `init` a store (the five roots), then release the lock.
fn init_store(tmp: &Tmp, v: &Arc<Switch>) {
    drop(StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap());
}

// --- fixtures -------------------------------------------------------------------------------------

fn coverage() -> Value {
    json!({
        "discovered_files": 0, "selected_files": 0, "assessed_files": 0, "excluded_files": 0,
        "missing_files": 0, "checks_passed": 0, "checks_with_findings": 0, "checks_failed": 0,
        "checks_unsupported": 0, "checks_skipped": 0, "checks_incomplete": 0, "checks_cancelled": 0,
        "coverage": "none", "assessment": "unknown"
    })
}

fn plan() -> Value {
    json!({
        "workflow_id": null, "workflow_revision": null, "targets": [],
        "scan_settings": {
            "categories": ["syntax"], "secret_scope": "checkout", "template_mode": "both",
            "schema_mode": "both", "schema_bindings": [], "renderers": [],
            "allow_reference_context": true, "adapter_ids": []
        },
        "approval_ids": [], "created_at": TS, "plan_digest": DIGEST,
        "target_snapshots": [], "engine_snapshots": []
    })
}

struct Run {
    n: u32,
    execution: &'static str,
    started_at: Option<&'static str>,
    finished_at: Option<&'static str>,
    last_checkpoint_at: Option<&'static str>,
}

fn run(n: u32, execution: &'static str) -> Run {
    Run {
        n,
        execution,
        started_at: None,
        finished_at: None,
        last_checkpoint_at: None,
    }
}

fn run_json(r: &Run) -> Value {
    json!({
        "run_id": uid(r.n), "trigger": "manual", "execution": r.execution, "created_at": TS,
        "started_at": r.started_at, "finished_at": r.finished_at, "plan": plan(),
        "coverage": coverage(), "report_ids": [], "last_checkpoint_at": r.last_checkpoint_at
    })
}

fn manifest_json(n: u32, json_path: &str, json_written: bool) -> Value {
    json!({
        "report_id": uid(110 + n), "run_id": uid(20 + n), "repo_id": uid(10),
        "json_path": json_path, "md_path": json_path.replace(".json", ".md"),
        "json_written": json_written, "md_written": false,
        "json_digest": if json_written { json!(DIGEST) } else { Value::Null },
        "generated_at": TS, "pdf_path": null, "pdf_written": false, "pdf_digest": null,
        "pdf_error_redacted": null, "pdf_renderer_version": null
    })
}

fn log_json(runs: &[Value], reports: &[Value]) -> Value {
    json!({"schema_version": "1.4", "store_type": "log", "updated_at": TS,
           "runs": runs, "events": [], "reports": reports})
}

fn approval_json(n: u32, status: &str) -> Value {
    json!({
        "approval_id": uid(40 + n), "status": status, "capability": "git_acquire",
        "workflow_id": null, "workflow_revision": null, "run_id": null, "repo_ids": [],
        "purpose": "Clone the registered repository", "operation_fingerprint": DIGEST,
        "destinations": [], "resource_constraints": [], "data_categories": [], "lifetime": "once",
        "requested_at": TS, "decided_at": if status == "pending" { Value::Null } else { json!(TS_START) },
        "expires_at": null, "actor": "local user", "consumed": false, "inference_scope": null
    })
}

fn file_doc(store_type: &str, member: &str, members: &[Value]) -> Value {
    json!({"schema_version": "1.4", "store_type": store_type, "updated_at": TS, member: members})
}

/// A report file name the application would generate: stamp, slug and run ID `n`.
fn report_name(stamp: &str, n: u32, ext: &str) -> String {
    format!("{stamp}_demo-repo_{}.{ext}", uid(20 + n))
}

const STAMP: &str = "20261016T052500Z";

// ---------------------------------------------------------------------------------------------------
// interrupted runs

#[test]
fn each_in_flight_state_becomes_interrupted_and_terminal_runs_are_untouched() {
    let tmp = Tmp::new("interrupt");
    let v = valid();
    let t0 = UtcTimestamp::now();
    let in_flight = [
        run(21, "queued"),
        run(22, "awaiting_approval"),
        // Clock skew: a run that already carries a timestamp after "now". finish must not precede it.
        Run {
            started_at: Some(FUTURE),
            ..run(23, "running")
        },
        Run {
            started_at: Some(TS_START),
            last_checkpoint_at: Some(FUTURE_CHECKPOINT),
            ..run(24, "cancelling")
        },
    ];
    let terminal = ["completed", "partial", "failed", "cancelled", "interrupted"]
        .into_iter()
        .zip(25..)
        .map(|(state, n)| Run {
            started_at: Some(TS_START),
            finished_at: Some(TS_DONE),
            ..run(n, state)
        })
        .collect::<Vec<_>>();
    let runs: Vec<Value> = in_flight
        .iter()
        .chain(terminal.iter())
        .map(run_json)
        .collect();
    let original = log_json(&runs, &[]);
    tmp.write_json("log.json", &original);
    let original_bytes = tmp.read("log.json");

    let (w, rec) = open(&tmp, &v).unwrap();
    // Fails if any of the four states is missed, or a terminal run is touched.
    assert_eq!(
        rec.runs_interrupted,
        [run_id(21), run_id(22), run_id(23), run_id(24)]
    );
    assert!(!w.is_halted());
    drop(w);

    let now = tmp.read_json("log.json");
    let after = now["runs"].as_array().unwrap();
    assert_eq!(after.len(), 9);
    for (i, run) in after.iter().enumerate().take(4) {
        assert_eq!(run["execution"], "interrupted", "run {i}");
        let finished = UtcTimestamp::parse_rfc3339(run["finished_at"].as_str().unwrap()).unwrap();
        assert!(
            finished >= t0,
            "run {i}: finished_at {finished:?} is before the reconciliation"
        );
        // Everything else about the run is unchanged.
        let mut expect = original["runs"][i].clone();
        expect["execution"] = json!("interrupted");
        expect["finished_at"] = run["finished_at"].clone();
        assert_eq!(run, &expect, "run {i}");
    }
    // Data Schema §11.18: finish is not before start / checkpoint, even when they are in the future.
    assert_eq!(after[2]["finished_at"], FUTURE);
    assert_eq!(after[3]["finished_at"], FUTURE_CHECKPOINT);
    for (i, run) in after.iter().enumerate().skip(4) {
        assert_eq!(run, &original["runs"][i], "terminal run {i} was modified");
    }
    // Written through the replace protocol: the previous generation is kept.
    assert_eq!(tmp.read("log.json.prev"), original_bytes);
    assert!(!snapshot(&tmp.root).keys().any(|k| k.contains(".tmp-")));
    assert_eq!(now["events"], original["events"]);
    assert_eq!(now["reports"], original["reports"]);

    // Reconciliation is idempotent: the second open finds nothing in flight and rewrites nothing.
    let bytes = tmp.read("log.json");
    let before = snapshot(&tmp.root);
    let (_w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec, ReconcileReport::default());
    assert_eq!(snapshot(&tmp.root), before);
    assert_eq!(tmp.read("log.json"), bytes);
}

#[test]
fn a_log_with_only_terminal_runs_is_not_rewritten() {
    let tmp = Tmp::new("terminal-only");
    let v = valid();
    let runs: Vec<Value> = ["completed", "partial", "failed", "cancelled", "interrupted"]
        .into_iter()
        .zip(21..)
        .map(|(state, n)| {
            run_json(&Run {
                started_at: Some(TS_START),
                finished_at: Some(TS_DONE),
                ..run(n, state)
            })
        })
        .collect();
    tmp.write_json("log.json", &log_json(&runs, &[]));
    let before = snapshot(&tmp.root);
    let (_w, rec) = open(&tmp, &v).unwrap();
    assert!(rec.runs_interrupted.is_empty());
    // Fails if reconciliation rewrites the log unconditionally (it would also create log.json.prev).
    assert_eq!(snapshot(&tmp.root), before);
}

// ---------------------------------------------------------------------------------------------------
// approvals

#[test]
fn pending_approvals_are_counted_and_left_pending() {
    let tmp = Tmp::new("approvals");
    let v = valid();
    let approvals = [
        approval_json(1, "pending"),
        approval_json(2, "approved"),
        approval_json(3, "pending"),
        approval_json(4, "denied"),
    ];
    tmp.write_json(
        "approvals.json",
        &file_doc("approvals", "approvals", &approvals),
    );
    // An in-flight run makes the log get rewritten in the same open: approvals must still not change.
    tmp.write_json(
        "log.json",
        &log_json(&[run_json(&run(21, "awaiting_approval"))], &[]),
    );
    let before = tmp.read("approvals.json");

    let (_w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.approvals_pending, 2);
    assert_eq!(rec.runs_interrupted, [run_id(21)]);
    // Fails if a pending request is turned into a grant, expired or rewritten (Data Schema §12.7).
    assert_eq!(tmp.read("approvals.json"), before);
    assert!(!tmp.exists_nofollow("approvals.json.prev"));
    let statuses: Vec<Value> = tmp.read_json("approvals.json")["approvals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["status"].clone())
        .collect();
    assert_eq!(statuses, ["pending", "approved", "pending", "denied"]);
}

// ---------------------------------------------------------------------------------------------------
// temp files

#[test]
fn only_the_applications_exact_temp_pattern_is_removed() {
    let tmp = Tmp::new("temps");
    let v = valid();
    tmp.private_dir("rag");
    tmp.private_dir("backup");
    let s0 = report_name(STAMP, 0, "").trim_end_matches('.').to_owned();
    let s1 = report_name(STAMP, 1, "").trim_end_matches('.').to_owned();

    let removed = [
        // data directory: every single-segment store file name, with and without `.prev`
        ".repo.json.tmp-0123456789abcdef".to_owned(),
        ".repo.json.prev.tmp-fedcba9876543210".to_owned(),
        ".workflows.json.tmp-0000000000000000".to_owned(),
        ".approvals.json.tmp-1111111111111111".to_owned(),
        ".log.json.prev.tmp-2222222222222222".to_owned(),
        ".config.json.tmp-3333333333333333".to_owned(),
        ".suppressions.json.tmp-4444444444444444".to_owned(),
        ".tools.json.tmp-5555555555555555".to_owned(),
        // rag/
        "rag/.index.json.tmp-6666666666666666".to_owned(),
        "rag/.conversations.json.prev.tmp-7777777777777777".to_owned(),
        // backup/: a generated report file name with each reserved extension
        format!("backup/.{s0}.json.tmp-8888888888888888"),
        format!("backup/.{s0}.md.tmp-9999999999999999"),
        format!("backup/.{s0}.pdf.prev.tmp-aaaaaaaaaaaaaaaa"),
    ];
    let kept = [
        // wrong hex: upper case, 15, 17 digits, empty
        ".repo.json.tmp-0123456789abcdeF",
        ".repo.json.tmp-0123456789abcde",
        ".repo.json.tmp-0123456789abcdef0",
        ".repo.json.tmp-",
        // wrong shape around the pattern
        "repo.json.tmp-0123456789abcdef",
        ".repo.json.tmp-0123456789abcdef.bak",
        ".repo.json.prev.prev.tmp-0123456789abcdef",
        ".tmp-0123456789abcdef",
        // not a name the application writes in this directory
        ".unknown.json.tmp-0123456789abcdef",
        ".index.json.tmp-0123456789abcdef",
        ".report.json.tmp-0123456789abcdef",
        "rag/.repo.json.tmp-0123456789abcdef",
        "rag/.log.json.tmp-0123456789abcdef",
        "backup/.notes.json.tmp-0123456789abcdef",
        "backup/.x_y_z.json.tmp-0123456789abcdef",
        "backup/.repo.json.tmp-0123456789abcdef",
        // foreign files and the store's own files
        "notes.txt",
        ".hidden",
        "repo.json.prev",
        "rag/other.json",
    ];
    for name in removed.iter().map(String::as_str).chain(kept) {
        tmp.write(name, name.as_bytes());
    }
    // Look-alike report names in backup/: a bad ID, a bad extension, an upper-case run ID.
    let lookalikes = [
        format!("backup/.{STAMP}_demo-repo_not-a-uuid.json.tmp-0123456789abcdef"),
        format!("backup/.{s0}.txt.tmp-0123456789abcdef"),
        format!("backup/.{}.json.tmp-0123456789abcdef", s0.to_uppercase()),
    ];
    for name in &lookalikes {
        tmp.write(name, b"look-alike");
    }
    // A real report (not a temp file) must stay too.
    let real_report = format!("backup/{s0}.json");
    tmp.write(&real_report, b"report");

    // Entries with a matching name that are not regular files: a symlink must not be followed or removed
    // (its target is outside the store), a directory must stay with its content.
    tmp.write("outside.txt", b"outside target");
    std::os::unix::fs::symlink(
        tmp.path("outside.txt"),
        tmp.path(".log.json.tmp-1111111111111111"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        tmp.path("outside.txt"),
        tmp.path(&format!("backup/.{s1}.json.tmp-cccccccccccccccc")),
    )
    .unwrap();
    std::fs::create_dir(tmp.path(".tools.json.tmp-2222222222222222")).unwrap();
    tmp.write(".tools.json.tmp-2222222222222222/inner.txt", b"inner");

    let before = snapshot(&tmp.root);
    let (w, rec) = open(&tmp, &v).unwrap();
    drop(w);
    // Fails if the pattern is matched too loosely (anything of the removed-looking kind is deleted) or too
    // narrowly (a known name, `.prev` form, `rag/` or `backup/` form is missed).
    assert_eq!(rec.temp_files_removed, 13);

    let after = snapshot(&tmp.root);
    let expected: BTreeMap<String, Vec<u8>> = before
        .iter()
        .filter(|(k, _)| !removed.contains(k))
        .map(|(k, val)| (k.clone(), val.clone()))
        .collect();
    assert_eq!(
        after, expected,
        "something other than the 13 temp files changed"
    );
    for name in &removed {
        assert!(!tmp.exists_nofollow(name), "{name} was not removed");
    }
    // The symlinks are still links and their target is untouched; the directory kept its file.
    assert!(
        std::fs::symlink_metadata(tmp.path(".log.json.tmp-1111111111111111"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(tmp.read("outside.txt"), b"outside target");
    assert_eq!(
        tmp.read(".tools.json.tmp-2222222222222222/inner.txt"),
        b"inner"
    );
    // Other findings: the real report has no manifest (no log) and nothing is malformed.
    assert_eq!(rec.reports_without_manifest, 1);
    assert!(rec.malformed_roots.is_empty());

    // A second open has nothing left to remove.
    let (_w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.temp_files_removed, 0);
}

// ---------------------------------------------------------------------------------------------------
// reports

#[test]
fn missing_and_unreferenced_reports_are_reported_and_never_changed() {
    let tmp = Tmp::new("reports");
    let v = valid();
    tmp.private_dir("backup");
    let rel = |n: u32| format!("backup/{}", report_name(STAMP, n, "json"));

    // M0 present; M1 missing; M2 missing but never claimed written; M3 points outside backup/ (the file
    // there is not a report this application wrote); M4 is a directory, not a file.
    tmp.write(&rel(0), b"report 0");
    tmp.write("elsewhere.json", b"not a report");
    std::fs::create_dir(tmp.path(&rel(4))).unwrap();
    let manifests = [
        manifest_json(0, &rel(0), true),
        manifest_json(1, &rel(1), true),
        manifest_json(2, &rel(2), false),
        manifest_json(3, "elsewhere.json", true),
        manifest_json(4, &rel(4), true),
    ];
    tmp.write_json("log.json", &log_json(&[], &manifests));

    // Unreferenced: two real report files count. These do not: other extensions, foreign names, an
    // upper-case run ID, a symlink and a directory with a generated-looking name.
    tmp.write(&rel(10), b"orphan 10");
    tmp.write(&rel(11), b"orphan 11");
    tmp.write(
        &format!("backup/{}", report_name(STAMP, 12, "md")),
        b"markdown",
    );
    tmp.write(&format!("backup/{}", report_name(STAMP, 12, "pdf")), b"pdf");
    tmp.write("backup/notes.json", b"notes");
    tmp.write(
        &format!("backup/{}", report_name(STAMP, 13, "json").to_uppercase()),
        b"upper",
    );
    std::os::unix::fs::symlink(tmp.path("elsewhere.json"), tmp.path(&rel(14))).unwrap();
    std::fs::create_dir(tmp.path(&rel(15))).unwrap();

    let before = snapshot(&tmp.root);
    let (w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(
        rec.manifests_without_report,
        [report_id(111), report_id(113), report_id(114)]
    );
    // Fails if the count includes non-reports, links or directories, or omits a real orphan.
    assert_eq!(rec.reports_without_manifest, 2);
    assert!(rec.malformed_roots.is_empty() && rec.runs_interrupted.is_empty());
    assert!(!w.is_halted());
    // Reported, not mutated, not deleted: the whole tree is byte-identical (no log.json.prev either).
    assert_eq!(snapshot(&tmp.root), before);
}

// ---------------------------------------------------------------------------------------------------
// malformed roots

fn config_doc() -> Value {
    json!({"schema_version": "1.4", "store_type": "config", "updated_at": TS, "config": {
        "app_data_root": "/home/user/.local/share/pwt-example", "backup_directory": "backup",
        "active_theme": "default", "max_active_repositories": 1, "max_scanner_processes": 1,
        "history_retention_days": null, "chat_persistence_enabled": false, "provider": null,
        "telemetry_enabled": false, "redaction_version": "v1"}})
}

#[test]
fn malformed_and_foreign_roots_are_reported_blocked_and_never_rewritten() {
    let tmp = Tmp::new("malformed");
    let v = valid();
    init_store(&tmp, &v);
    let healthy_prev = tmp.read("repo.json");
    // A valid previous generation next to a bad current file: still must not be rotated or replaced.
    tmp.write("repo.json.prev", &healthy_prev);
    tmp.write("repo.json", b"{ not json");
    // Another store's document in workflows.json (foreign), an older schema version in approvals.json.
    tmp.write_json("workflows.json", &file_doc("approvals", "approvals", &[]));
    let mut old = file_doc("approvals", "approvals", &[]);
    old["schema_version"] = json!("1.3");
    tmp.write_json("approvals.json", &old);
    // tools.json is a symlink to a valid document elsewhere: not followed, so foreign.
    tmp.write_json("elsewhere.json", &file_doc("tool_manifest", "tools", &[]));
    std::fs::remove_file(tmp.path("tools.json")).unwrap();
    std::os::unix::fs::symlink(tmp.path("elsewhere.json"), tmp.path("tools.json")).unwrap();
    // config.json is a directory.
    std::fs::create_dir(tmp.path("config.json")).unwrap();
    let before = snapshot(&tmp.root);

    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(
        rec.malformed_roots,
        [
            StoreKind::Config,
            StoreKind::Repositories,
            StoreKind::Workflows,
            StoreKind::Approvals,
            StoreKind::ToolManifest,
        ]
    );
    // Reported only: nothing was repaired, rotated, quarantined or deleted.
    assert_eq!(snapshot(&tmp.root), before);

    // Writes to each blocked root are refused; the bytes stay exactly as they were (D02-9: no `.prev`
    // rotation, no overwrite). Fails if `replace` consults only the file system and not the block list.
    let docs = [
        (StoreKind::Config, config_doc()),
        (
            StoreKind::Repositories,
            file_doc("repositories", "repositories", &[]),
        ),
        (
            StoreKind::Workflows,
            file_doc("workflows", "workflows", &[]),
        ),
        (
            StoreKind::Approvals,
            file_doc("approvals", "approvals", &[]),
        ),
        (
            StoreKind::ToolManifest,
            file_doc("tool_manifest", "tools", &[]),
        ),
    ];
    for (kind, doc) in docs {
        let any = decode(kind, &serde_json::to_vec(&doc).unwrap(), &*valid()).unwrap();
        assert_eq!(w.replace(&any).unwrap_err(), StoreError::Malformed(kind));
    }
    assert_eq!(
        snapshot(&tmp.root),
        before,
        "a refused replace changed the store"
    );
    assert!(!w.is_halted(), "a blocked root is not a storage error");

    // The healthy log is not blocked.
    let log = decode(
        StoreKind::Log,
        &serde_json::to_vec(&log_json(&[], &[])).unwrap(),
        &*valid(),
    )
    .unwrap();
    w.replace(&log).unwrap();
    assert!(matches!(w.read(StoreKind::Log), Ok(AnyStore::Log(_))));
}

#[test]
fn a_malformed_log_blocks_the_log_and_skips_the_checks_that_need_it() {
    let tmp = Tmp::new("malformed-log");
    let v = valid();
    tmp.private_dir("backup");
    tmp.write("log.json", b"[1, 2, 3]");
    tmp.write(
        &format!("backup/{}", report_name(STAMP, 0, "json")),
        b"a report",
    );
    tmp.write(
        &format!(
            "backup/.{}.tmp-0123456789abcdef",
            report_name(STAMP, 0, "json")
        ),
        b"temp",
    );
    let before = snapshot(&tmp.root);

    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.malformed_roots, [StoreKind::Log]);
    assert!(rec.runs_interrupted.is_empty());
    // Without a readable log "no manifest" would be a guess: not counted, not reported.
    assert_eq!(rec.reports_without_manifest, 0);
    assert!(rec.manifests_without_report.is_empty());
    assert_eq!(
        rec.temp_files_removed, 1,
        "temp cleanup does not depend on the log"
    );
    let mut expected = before.clone();
    expected.retain(|k, _| !k.contains(".tmp-"));
    assert_eq!(snapshot(&tmp.root), expected);
    assert_eq!(tmp.read("log.json"), b"[1, 2, 3]");

    let any = decode(
        StoreKind::Log,
        &serde_json::to_vec(&log_json(&[], &[])).unwrap(),
        &*valid(),
    )
    .unwrap();
    assert_eq!(
        w.replace(&any).unwrap_err(),
        StoreError::Malformed(StoreKind::Log)
    );
}

// ---------------------------------------------------------------------------------------------------
// failure behaviour of open

#[test]
fn open_fails_closed_when_the_schema_is_unavailable() {
    let tmp = Tmp::new("unavailable");
    let v = valid();
    init_store(&tmp, &v);
    let before = snapshot(&tmp.root);
    v.set(MODE_UNAVAILABLE);
    // Existing roots cannot be validated, so nothing may be classified as fine or malformed (FR-15).
    assert_eq!(open(&tmp, &v).unwrap_err(), StoreError::SchemaUnavailable);
    assert_eq!(snapshot(&tmp.root), before);
    // The failed open released the lock.
    v.set(MODE_VALID);
    drop(open(&tmp, &v).unwrap());

    // An empty directory has nothing to validate, so it opens (and `init` would then refuse to write).
    let empty = Tmp::new("unavailable-empty");
    let u = valid();
    u.set(MODE_UNAVAILABLE);
    let (_w, rec) = open(&empty, &u).unwrap();
    assert_eq!(rec, ReconcileReport::default());
}

#[test]
fn open_fails_when_the_interrupted_log_does_not_validate_and_changes_nothing() {
    let tmp = Tmp::new("invalid-result");
    let v = valid();
    tmp.write_json("log.json", &log_json(&[run_json(&run(21, "running"))], &[]));
    let before = snapshot(&tmp.root);
    // The fake rejects any log that contains an interrupted run, i.e. exactly the document that
    // reconciliation wants to write. Fails if the result of reconciliation is written unvalidated.
    v.set(MODE_NO_INTERRUPTED_RUNS);
    let err = open(&tmp, &v).unwrap_err();
    assert_eq!(err, StoreError::Invalid(StoreKind::Log, marker_problems()));
    assert_eq!(snapshot(&tmp.root), before);
    // And the lock was released.
    v.set(MODE_VALID);
    let (_w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.runs_interrupted, [run_id(21)]);
}

#[test]
fn a_storage_error_while_interrupting_runs_fails_open_and_a_retry_completes() {
    let steps = [
        (WriteStep::CreateTemp, false),
        (WriteStep::WriteTemp, false),
        (WriteStep::SyncTemp, false),
        (WriteStep::LinkPrevious, false),
        (WriteStep::ReplacePrevious, false),
        (WriteStep::Replace, false),
        // After the rename the new log is already in place.
        (WriteStep::SyncDir, true),
    ];
    for (step, written) in steps {
        for kind in [ErrorKind::StorageFull, ErrorKind::PermissionDenied] {
            let tmp = Tmp::new("fault");
            let v = valid();
            tmp.write_json("log.json", &log_json(&[run_json(&run(21, "running"))], &[]));
            let original = tmp.read("log.json");
            let fault = Arc::new(FaultFs::new(FaultPlan {
                step,
                nth: 1,
                error: kind,
            }));
            let context = format!("{step:?} {kind:?}");

            let err = StoreWriter::open(tmp.data_dir(), fault.clone(), v.clone()).unwrap_err();
            // Fails if the error is swallowed (Data Schema §11.25) or reported as another step.
            assert_eq!(err, StoreError::Io { step, kind }, "{context}");
            assert!(fault.fired(), "{context}");
            let now = tmp.read_json("log.json");
            if written {
                assert_eq!(now["runs"][0]["execution"], "interrupted", "{context}");
            } else {
                assert_eq!(
                    tmp.read("log.json"),
                    original,
                    "{context}: the previous log changed"
                );
            }
            assert!(
                !snapshot(&tmp.root).keys().any(|k| k.contains(".tmp-")),
                "{context}"
            );

            // The lock is free again and the retry converges on the same final state.
            let (_w, rec) = open(&tmp, &v).unwrap();
            let expected_ids: Vec<RunId> = if written {
                Vec::new()
            } else {
                vec![run_id(21)]
            };
            assert_eq!(rec.runs_interrupted, expected_ids, "{context}");
            assert_eq!(
                tmp.read_json("log.json")["runs"][0]["execution"],
                "interrupted",
                "{context}"
            );
        }
    }
}

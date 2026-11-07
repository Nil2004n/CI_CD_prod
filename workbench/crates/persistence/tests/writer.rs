//! P02-T10 tests for `StoreWriter`: validate-before-write, halt on storage errors, `init`, `replace`,
//! `read`, `write_report`, `restore_previous` and the single-instance lock. Reconciliation (`open`) has its
//! own file, `tests/recovery.rs`; report stems are in `tests/report_name.rs`.
//!
//! The validator is always a fake (`Switch`): the embedded-schema validator is T08's and is exercised with
//! the writer by the lead's integration tests. Fixtures are synthetic. Every test uses its own fresh
//! directory under `CARGO_TARGET_TMPDIR`, removed on drop; nothing touches the network or any other path.
//! For each assertion that matters the comment says what change in the writer would make it fail.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use domain::store::report::ReportFile;
use domain::store::values::Slug;
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
use sha2::{Digest as _, Sha256};

const TS: &str = "2026-10-16T05:25:00Z";
const TS_LATER: &str = "2026-10-17T06:00:00Z";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The five roots `init` creates, in `InitReport` order.
const INIT_FILES: [(StoreKind, &str); 5] = [
    (StoreKind::Repositories, "repo.json"),
    (StoreKind::Workflows, "workflows.json"),
    (StoreKind::Approvals, "approvals.json"),
    (StoreKind::Log, "log.json"),
    (StoreKind::ToolManifest, "tools.json"),
];

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
            .join(format!("writer-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // The data directory must be private (P02-D03); the default umask would leave it 0755.
        set_mode(&root, 0o700);
        Self { root }
    }

    fn data_dir(&self) -> persistence::paths::DataDir {
        persistence::paths::DataDir::new(&RealFs, &self.root).unwrap()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
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

fn mode_of(path: &Path) -> u32 {
    std::fs::symlink_metadata(path).unwrap().mode() & 0o7777
}

/// Relative path -> content for every file under `root` (directories as `<path>/` -> empty), except the
/// lock file, whose diagnostic line changes with every open.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel == ".app.lock" {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(base, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// `snapshot` without the `.prev` generations (a fault after the previous generation was rotated leaves a
/// new `.prev` that is a valid copy of the current file).
fn snapshot_without_prev(root: &Path) -> BTreeMap<String, Vec<u8>> {
    snapshot(root)
        .into_iter()
        .filter(|(k, _)| !k.ends_with(".prev"))
        .collect()
}

fn has_temp(snap: &BTreeMap<String, Vec<u8>>) -> bool {
    snap.keys().any(|k| k.contains(".tmp-"))
}

fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// --- fake validator -------------------------------------------------------------------------------

const MODE_VALID: u8 = 0;
const MODE_INVALID: u8 = 1;
const MODE_UNAVAILABLE: u8 = 2;
/// Rejects only a `log` document that already carries a report manifest (the log *after* `write_report`
/// appended one), so the existing, manifest-free log still reads.
const MODE_LOG_WITH_MANIFEST: u8 = 3;

/// A validator whose verdict can be flipped between calls, and that records every call.
#[derive(Debug, Default)]
struct Switch {
    mode: AtomicU8,
    /// Only documents of this kind are rejected in `MODE_INVALID` (all kinds if `None`).
    only_kind: Mutex<Option<StoreKind>>,
    calls: Mutex<Vec<(StoreKind, Value)>>,
}

impl Switch {
    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }

    fn reject_only(&self, kind: StoreKind) {
        *self.only_kind.lock().unwrap() = Some(kind);
        self.set(MODE_INVALID);
    }

    fn calls(&self) -> Vec<(StoreKind, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

fn marker_problems() -> Problems {
    Problems::one(Problem::new(
        Layer::Schema,
        ProblemCode::Keyword(SchemaKeyword::Required),
        "/runs",
        None,
    ))
}

impl DocumentValidator for Switch {
    fn validate(&self, kind: StoreKind, doc: &Value) -> Verdict {
        self.calls.lock().unwrap().push((kind, doc.clone()));
        let rejected = self
            .only_kind
            .lock()
            .unwrap()
            .is_none_or(|only| only == kind);
        match self.mode.load(Ordering::SeqCst) {
            MODE_INVALID if rejected => Verdict::Invalid(marker_problems()),
            MODE_UNAVAILABLE => Verdict::SchemaUnavailable,
            MODE_LOG_WITH_MANIFEST
                if kind == StoreKind::Log
                    && doc["reports"].as_array().is_some_and(|a| !a.is_empty()) =>
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

// --- fixtures -------------------------------------------------------------------------------------

fn file_doc(store_type: &str, member: &str) -> Value {
    json!({"schema_version": "1.4", "store_type": store_type, "updated_at": TS, member: []})
}

fn log_doc(updated_at: &str) -> Value {
    json!({"schema_version": "1.4", "store_type": "log", "updated_at": updated_at,
           "runs": [], "events": [], "reports": []})
}

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

/// A minimal report; `n` selects distinct report and run IDs.
fn report_doc(n: u32, generated_at: &str) -> Value {
    json!({
        "schema_version": "1.4", "store_type": "report", "updated_at": TS,
        "report_id": uid(110 + n), "run_id": uid(20 + n), "repo_id": uid(10),
        "repo_name": "demo-repo", "remote_identifier_redacted": "example.invalid/demo",
        "selected_ref": "main", "commit_oid": null, "execution": "queued", "plan": plan(),
        "generated_at": generated_at, "coverage": coverage(), "checks": [], "findings": [],
        "artifacts": [], "limitations": [],
        "ai_enrichment": {
            "status": "not_requested", "provider_id": null, "requested_model": null,
            "actual_model": null, "actual_provider": null, "approval_ids": [], "source_ids": [],
            "routing_policy": null, "redaction_version": "redact-1", "generated_at": null,
            "narrative_redacted": "", "reason_redacted": null
        }
    })
}

fn any(kind: StoreKind, doc: &Value) -> AnyStore {
    decode(kind, &serde_json::to_vec(doc).unwrap(), &*valid())
        .unwrap_or_else(|e| panic!("fixture does not decode: {e:?}"))
}

fn report_file(n: u32, generated_at: &str) -> ReportFile {
    match any(StoreKind::Report, &report_doc(n, generated_at)) {
        AnyStore::Report(file) => *file,
        other => panic!("not a report: {other:?}"),
    }
}

fn slug() -> Slug {
    Slug::parse("demo-repo").unwrap()
}

fn open(tmp: &Tmp, v: &Arc<Switch>) -> Result<(StoreWriter, ReconcileReport), StoreError> {
    StoreWriter::open(tmp.data_dir(), Arc::new(RealFs), v.clone())
}

fn init_writer(tmp: &Tmp, v: &Arc<Switch>) -> StoreWriter {
    StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone())
        .unwrap()
        .0
}

/// The path (relative to the data directory) of the report JSON for `generated_at` compact stamp.
fn report_rel(stamp: &str, n: u32) -> String {
    format!("backup/{stamp}_demo-repo_{}.json", uid(20 + n))
}

// ---------------------------------------------------------------------------------------------------
// validate before write

#[test]
fn invalid_or_unavailable_schema_writes_nothing() {
    let tmp = Tmp::new("validate");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let before = snapshot(&tmp.root);
    let doc = any(StoreKind::Log, &log_doc(TS_LATER));

    // Fails if `replace` publishes the file before (or without) asking the validator.
    v.set(MODE_INVALID);
    let err = w.replace(&doc).unwrap_err();
    assert_eq!(
        err,
        StoreError::Invalid(StoreKind::Log, marker_problems()),
        "the validator's problems are returned unchanged"
    );
    assert_eq!(snapshot(&tmp.root), before, "Invalid wrote something");
    assert!(
        !w.is_halted(),
        "a validation failure is not a storage error"
    );

    // FR-15: an unavailable schema is never treated as valid.
    v.set(MODE_UNAVAILABLE);
    assert_eq!(w.replace(&doc).unwrap_err(), StoreError::SchemaUnavailable);
    assert_eq!(
        snapshot(&tmp.root),
        before,
        "SchemaUnavailable wrote something"
    );
    assert!(!w.is_halted());

    // The same call succeeds once the validator agrees, so the failures above were the validator's doing.
    v.set(MODE_VALID);
    w.replace(&doc).unwrap();
    assert_ne!(snapshot(&tmp.root), before);
}

#[test]
fn the_validator_sees_the_serialised_document_of_the_right_kind() {
    let tmp = Tmp::new("validator-input");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let calls_after_init = v.calls().len();
    let doc = any(StoreKind::Log, &log_doc(TS_LATER));
    w.replace(&doc).unwrap();
    let calls = v.calls();
    // One validation per replace (init validated its five empty roots, then `read` of nothing else).
    let new_calls = &calls[calls_after_init..];
    assert_eq!(
        new_calls.len(),
        1,
        "exactly one validation per write: {new_calls:?}"
    );
    let (kind, value) = &new_calls[0];
    assert_eq!(*kind, StoreKind::Log);
    // Fails if the writer validated a different serialisation than the one it writes.
    assert_eq!(value, &log_doc(TS_LATER));
    let on_disk: Value =
        serde_json::from_slice(&std::fs::read(tmp.path("log.json")).unwrap()).unwrap();
    assert_eq!(
        &on_disk, value,
        "the file holds exactly the validated value"
    );
}

#[test]
fn replace_keeps_the_previous_generation_and_private_modes() {
    let tmp = Tmp::new("replace");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let first = std::fs::read(tmp.path("log.json")).unwrap();
    let doc = any(StoreKind::Log, &log_doc(TS_LATER));
    w.replace(&doc).unwrap();

    assert_eq!(w.read(StoreKind::Log).unwrap(), doc);
    // Fails if the rotation into `.prev` is skipped or `.prev` holds the new generation.
    assert_eq!(std::fs::read(tmp.path("log.json.prev")).unwrap(), first);
    assert_eq!(mode_of(&tmp.path("log.json")), 0o600);
    assert_eq!(mode_of(&tmp.path("log.json.prev")), 0o600);
    let snap = snapshot(&tmp.root);
    assert!(
        !has_temp(&snap),
        "temp files left behind: {:?}",
        snap.keys()
    );
}

#[test]
fn replace_creates_optional_roots_with_private_directories() {
    let tmp = Tmp::new("replace-new");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    // config.json is not created by init; the first replace creates it (P03 setup).
    let config = json!({"schema_version": "1.4", "store_type": "config", "updated_at": TS, "config": {
        "app_data_root": "/home/user/.local/share/pwt-example", "backup_directory": "backup",
        "active_theme": "default", "max_active_repositories": 1, "max_scanner_processes": 1,
        "history_retention_days": null, "chat_persistence_enabled": false, "provider": null,
        "telemetry_enabled": false, "redaction_version": "v1"}});
    w.replace(&any(StoreKind::Config, &config)).unwrap();
    assert!(tmp.path("config.json").is_file());

    // A root in rag/ creates rag/ with mode 0700 (R-D03-8) and the file with 0600.
    let rag = file_doc("rag_index", "chunks");
    w.replace(&any(StoreKind::RagIndex, &rag)).unwrap();
    assert_eq!(
        mode_of(&tmp.path("rag")),
        0o700,
        "fails if rag/ is created with the umask default"
    );
    assert_eq!(mode_of(&tmp.path("rag/index.json")), 0o600);
    assert_eq!(
        w.read(StoreKind::RagIndex).unwrap(),
        any(StoreKind::RagIndex, &rag)
    );
}

#[test]
fn replace_into_a_loose_existing_rag_directory_is_refused() {
    let tmp = Tmp::new("rag-loose");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    // The lock checks `rag/` at open time; a directory that becomes loose afterwards is caught by the
    // per-write check (R-D03-8).
    std::fs::create_dir(tmp.path("rag")).unwrap();
    set_mode(&tmp.path("rag"), 0o755);
    let rag = any(StoreKind::RagIndex, &file_doc("rag_index", "chunks"));
    assert_eq!(w.replace(&rag).unwrap_err(), StoreError::PathRejected);
    assert!(
        !tmp.path("rag/index.json").exists(),
        "wrote into a group-readable directory"
    );
    assert!(!w.is_halted());
}

// ---------------------------------------------------------------------------------------------------
// read and the roots that are not writable

#[test]
fn read_reports_missing_roots_and_rejects_reports() {
    let tmp = Tmp::new("read");
    let v = valid();
    let w = init_writer(&tmp, &v);
    assert_eq!(
        w.read(StoreKind::Config).unwrap_err(),
        StoreError::NotInitialised(StoreKind::Config),
        "init never creates config.json"
    );
    assert_eq!(
        w.read(StoreKind::Suppressions).unwrap_err(),
        StoreError::NotInitialised(StoreKind::Suppressions)
    );
    // A report is not a root.
    assert_eq!(
        w.read(StoreKind::Report).unwrap_err(),
        StoreError::PathRejected
    );
    for (kind, _) in INIT_FILES {
        assert_eq!(w.read(kind).unwrap().kind(), kind);
    }
}

#[test]
fn read_goes_through_decode_and_surfaces_validation_problems() {
    let tmp = Tmp::new("read-invalid");
    let v = valid();
    let w = init_writer(&tmp, &v);
    // Wrong store_type in the file: the version/type gate of `decode` runs even with a fake validator.
    std::fs::write(
        tmp.path("repo.json"),
        serde_json::to_vec(&log_doc(TS)).unwrap(),
    )
    .unwrap();
    match w.read(StoreKind::Repositories).unwrap_err() {
        StoreError::Invalid(StoreKind::Repositories, p) => {
            assert_eq!(p.items.len(), 1);
            assert_eq!(p.items[0].code, ProblemCode::StoreTypeMismatch);
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    // A schema that cannot validate never yields a document.
    v.set(MODE_UNAVAILABLE);
    assert_eq!(
        w.read(StoreKind::Log).unwrap_err(),
        StoreError::SchemaUnavailable
    );
}

#[test]
fn read_of_a_symlink_root_is_malformed_and_not_followed() {
    let tmp = Tmp::new("read-symlink");
    let v = valid();
    let w = init_writer(&tmp, &v);
    let outside = tmp.path("outside.json");
    std::fs::write(
        &outside,
        serde_json::to_vec(&file_doc("workflows", "workflows")).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(tmp.path("workflows.json")).unwrap();
    std::os::unix::fs::symlink(&outside, tmp.path("workflows.json")).unwrap();
    // Fails if the reader follows the link (it would then decode the valid document behind it).
    assert_eq!(
        w.read(StoreKind::Workflows).unwrap_err(),
        StoreError::Malformed(StoreKind::Workflows)
    );
}

#[test]
fn suppressions_and_reports_are_never_writable() {
    let tmp = Tmp::new("not-writable");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let before = snapshot(&tmp.root);
    let calls = v.calls().len();

    let suppressions = any(
        StoreKind::Suppressions,
        &file_doc("suppressions", "suppressions"),
    );
    assert_eq!(
        w.replace(&suppressions).unwrap_err(),
        StoreError::PathRejected
    );
    let report = any(StoreKind::Report, &report_doc(0, TS));
    assert_eq!(w.replace(&report).unwrap_err(), StoreError::PathRejected);
    assert_eq!(
        w.restore_previous(StoreKind::Suppressions).unwrap_err(),
        StoreError::PathRejected
    );
    assert_eq!(
        w.restore_previous(StoreKind::Report).unwrap_err(),
        StoreError::PathRejected
    );
    // Fails if any of the four calls reached the validator or the disk before refusing.
    assert_eq!(snapshot(&tmp.root), before);
    assert_eq!(v.calls().len(), calls, "refused calls must not validate");
    assert!(!tmp.path("suppressions.json").exists());
    assert!(!w.is_halted(), "a refused path is not a storage error");

    // Even when the file exists, restoring it is refused.
    std::fs::write(tmp.path("suppressions.json.prev"), b"{}").unwrap();
    assert_eq!(
        w.restore_previous(StoreKind::Suppressions).unwrap_err(),
        StoreError::PathRejected
    );
}

// ---------------------------------------------------------------------------------------------------
// init

#[test]
fn init_creates_exactly_the_five_roots_with_private_modes() {
    let tmp = Tmp::new("init");
    let v = valid();
    let (w, report) = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap();
    // Taken before the reads below, which validate again.
    let validated: Vec<StoreKind> = v.calls().iter().map(|(k, _)| *k).collect();
    let created: Vec<StoreKind> = INIT_FILES.iter().map(|(k, _)| *k).collect();
    assert_eq!(report.created, created);
    assert!(report.already_present.is_empty());

    let mut names: Vec<String> = std::fs::read_dir(&tmp.root)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    // Fails if init also creates config.json / rag / backup / suppressions.json (T01-D7) or leaves a temp.
    assert_eq!(
        names,
        [
            ".app.lock",
            "approvals.json",
            "log.json",
            "repo.json",
            "tools.json",
            "workflows.json"
        ]
    );
    for (kind, file) in INIT_FILES {
        assert_eq!(mode_of(&tmp.path(file)), 0o600, "{file}");
        // Each root decodes to an empty document of its own kind.
        assert_eq!(w.read(kind).unwrap().kind(), kind);
        let value: Value = serde_json::from_slice(&std::fs::read(tmp.path(file)).unwrap()).unwrap();
        assert_eq!(value["schema_version"], "1.4");
        assert_eq!(value["store_type"], kind.store_type());
        let collection =
            value.as_object().unwrap().iter().filter(|(k, _)| {
                !matches!(k.as_str(), "schema_version" | "store_type" | "updated_at")
            });
        for (name, members) in collection {
            assert_eq!(
                members,
                &json!([]),
                "{file}.{name} is not an empty collection"
            );
        }
        UtcTimestamp::parse_rfc3339(value["updated_at"].as_str().unwrap()).unwrap();
    }
    assert_eq!(
        mode_of(&tmp.root),
        0o700,
        "init must not loosen the data directory"
    );

    // Every default was validated before it was written, once per root and with its own kind.
    assert_eq!(validated, created);
}

#[test]
fn init_is_idempotent_and_leaves_existing_roots_unchanged() {
    let tmp = Tmp::new("init-twice");
    let v = valid();
    drop(init_writer(&tmp, &v));
    // A user-edited but valid root must survive a second init byte for byte.
    let edited = serde_json::to_vec_pretty(&file_doc("workflows", "workflows")).unwrap();
    std::fs::write(tmp.path("workflows.json"), &edited).unwrap();
    let before = snapshot(&tmp.root);

    let (_w, report) = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap();
    assert!(
        report.created.is_empty(),
        "second init created {:?}",
        report.created
    );
    let present: Vec<StoreKind> = INIT_FILES.iter().map(|(k, _)| *k).collect();
    assert_eq!(report.already_present, present);
    assert_eq!(snapshot(&tmp.root), before);
    assert_eq!(std::fs::read(tmp.path("workflows.json")).unwrap(), edited);
}

#[test]
fn init_completes_a_partial_store() {
    let tmp = Tmp::new("init-partial");
    let v = valid();
    drop(init_writer(&tmp, &v));
    std::fs::remove_file(tmp.path("approvals.json")).unwrap();
    std::fs::remove_file(tmp.path("tools.json")).unwrap();
    let (_w, report) = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap();
    assert_eq!(
        report.created,
        [StoreKind::Approvals, StoreKind::ToolManifest]
    );
    assert_eq!(
        report.already_present,
        [
            StoreKind::Repositories,
            StoreKind::Workflows,
            StoreKind::Log
        ]
    );
}

#[test]
fn init_with_an_invalid_default_or_unavailable_schema_creates_nothing() {
    let tmp = Tmp::new("init-invalid");
    let v = valid();
    v.set(MODE_INVALID);
    let err = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap_err();
    assert!(
        matches!(err, StoreError::Invalid(StoreKind::Repositories, _)),
        "{err:?}"
    );
    v.set(MODE_UNAVAILABLE);
    let err = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap_err();
    assert_eq!(err, StoreError::SchemaUnavailable);
    // Fails if a root is written before its validation. Only the lock file may exist.
    let names: Vec<String> = snapshot(&tmp.root).into_keys().collect();
    assert!(names.is_empty(), "init wrote {names:?}");
}

#[test]
fn init_does_not_overwrite_or_rotate_a_malformed_root() {
    let tmp = Tmp::new("init-malformed");
    let v = valid();
    drop(init_writer(&tmp, &v));
    std::fs::write(tmp.path("repo.json"), b"{ this is not json").unwrap();
    let before = snapshot(&tmp.root);
    let (mut w, report) = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap();
    assert!(report.created.is_empty());
    assert!(report.already_present.contains(&StoreKind::Repositories));
    // The root is blocked: replace is refused and nothing (not even `.prev`) changes (D02-9).
    let doc = any(
        StoreKind::Repositories,
        &file_doc("repositories", "repositories"),
    );
    assert_eq!(
        w.replace(&doc).unwrap_err(),
        StoreError::Malformed(StoreKind::Repositories)
    );
    assert_eq!(snapshot(&tmp.root), before);
    assert!(!w.is_halted());
}

// ---------------------------------------------------------------------------------------------------
// lock

#[test]
fn a_second_open_or_init_on_the_same_directory_is_locked() {
    let tmp = Tmp::new("lock");
    let v = valid();
    let first = init_writer(&tmp, &v);
    assert_eq!(open(&tmp, &v).unwrap_err(), StoreError::Locked);
    assert_eq!(
        StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), v.clone()).unwrap_err(),
        StoreError::Locked
    );
    // The failed attempts must not disturb the first writer.
    assert!(!first.is_halted());
    drop(first);
    // The lock is released with the writer.
    drop(open(&tmp, &v).unwrap());
}

// ---------------------------------------------------------------------------------------------------
// halt on storage errors

const KINDS: [ErrorKind; 2] = [ErrorKind::StorageFull, ErrorKind::PermissionDenied];

#[test]
fn replace_halts_on_a_storage_error_at_every_step() {
    let steps = [
        WriteStep::CreateTemp,
        WriteStep::WriteTemp,
        WriteStep::SyncTemp,
        WriteStep::LinkPrevious,
        WriteStep::ReplacePrevious,
        WriteStep::Replace,
        WriteStep::SyncDir,
    ];
    for step in steps {
        for kind in KINDS {
            let tmp = Tmp::new("halt-replace");
            let v = valid();
            drop(init_writer(&tmp, &v));
            let fault = Arc::new(FaultFs::new(FaultPlan {
                step,
                nth: 1,
                error: kind,
            }));
            let (mut w, rec) = StoreWriter::open(tmp.data_dir(), fault.clone(), v.clone()).unwrap();
            assert_eq!(rec, ReconcileReport::default());
            assert!(!fault.fired(), "the fault fired during open");
            let before = snapshot_without_prev(&tmp.root);
            let doc = any(StoreKind::Log, &log_doc(TS_LATER));
            let context = format!("{step:?} {kind:?}");

            let err = w.replace(&doc).unwrap_err();
            assert_eq!(err, StoreError::Io { step, kind }, "{context}");
            assert!(
                fault.fired(),
                "{context}: the planned step was never reached"
            );
            // Fails if the writer keeps accepting writes after a storage error (Data Schema §11.25).
            assert!(w.is_halted(), "{context}");

            let after = snapshot_without_prev(&tmp.root);
            assert!(!has_temp(&after), "{context}: temp left behind");
            if step == WriteStep::SyncDir {
                // The rename already happened: the new generation is in place and complete.
                assert_eq!(w.read(StoreKind::Log).unwrap(), doc, "{context}");
            } else {
                assert_eq!(after, before, "{context}: the previous file changed");
                assert_ne!(w.read(StoreKind::Log).unwrap(), doc, "{context}");
            }

            // Later mutating calls are refused and write nothing; reads still work.
            let frozen = snapshot(&tmp.root);
            let again = any(StoreKind::Log, &log_doc("2026-10-18T00:00:00Z"));
            assert_eq!(
                w.replace(&again).unwrap_err(),
                StoreError::Halted,
                "{context}"
            );
            assert_eq!(
                w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
                StoreError::Halted,
                "{context}"
            );
            assert_eq!(
                w.restore_previous(StoreKind::Log).unwrap_err(),
                StoreError::Halted,
                "{context}"
            );
            assert_eq!(
                snapshot(&tmp.root),
                frozen,
                "{context}: a refused call wrote"
            );
            assert!(w.read(StoreKind::Log).is_ok(), "{context}");
            drop(w);

            // Reopening clears the halt.
            let (mut again_w, rec) = open(&tmp, &v).unwrap();
            assert!(!again_w.is_halted(), "{context}");
            assert_eq!(rec.temp_files_removed, 0, "{context}");
            again_w.replace(&again).unwrap();
            assert_eq!(again_w.read(StoreKind::Log).unwrap(), again, "{context}");
        }
    }
}

/// The five initialized roots are read once by the version gate and once by reconciliation.
const READS_AT_OPEN: u32 = 10;

#[test]
fn write_report_halts_on_a_storage_error_at_every_step() {
    // (step, nth call, report file exists afterwards, log has the manifest afterwards). The nth call
    // counts the report's own write (`write_new`) first and the log replace second.
    let plan: [(WriteStep, u32, bool, bool); 14] = [
        (WriteStep::Read, READS_AT_OPEN + 1, false, false),
        (WriteStep::CreateDir, 1, false, false),
        (WriteStep::CreateTemp, 1, false, false),
        (WriteStep::WriteTemp, 1, false, false),
        (WriteStep::SyncTemp, 1, false, false),
        (WriteStep::Replace, 1, false, false),
        (WriteStep::SyncDir, 1, true, false),
        (WriteStep::CreateTemp, 2, true, false),
        (WriteStep::WriteTemp, 2, true, false),
        (WriteStep::SyncTemp, 2, true, false),
        (WriteStep::LinkPrevious, 1, true, false),
        (WriteStep::ReplacePrevious, 1, true, false),
        (WriteStep::Replace, 2, true, false),
        (WriteStep::SyncDir, 2, true, true),
    ];
    for (step, nth, report_written, manifest_written) in plan {
        for kind in KINDS {
            let tmp = Tmp::new("halt-report");
            let v = valid();
            drop(init_writer(&tmp, &v));
            let fault = Arc::new(FaultFs::new(FaultPlan {
                step,
                nth,
                error: kind,
            }));
            let (mut w, _) = StoreWriter::open(tmp.data_dir(), fault.clone(), v.clone()).unwrap();
            assert!(!fault.fired(), "fired during open");
            if step == WriteStep::Read {
                assert_eq!(
                    fault.calls_at_step(),
                    READS_AT_OPEN,
                    "startup read count changed"
                );
            }
            let log_before = std::fs::read(tmp.path("log.json")).unwrap();
            let context = format!("{step:?}#{nth} {kind:?}");

            let err = w.write_report(&report_file(0, TS), &slug()).unwrap_err();
            assert_eq!(err, StoreError::Io { step, kind }, "{context}");
            assert!(fault.fired(), "{context}");
            assert!(w.is_halted(), "{context}");

            let rel = report_rel("20261016T052500Z", 0);
            assert_eq!(
                tmp.path(&rel).exists(),
                report_written,
                "{context}: report file"
            );
            let snap = snapshot(&tmp.root);
            assert!(!has_temp(&snap), "{context}: temp left behind");
            let log_now = w.read(StoreKind::Log).unwrap();
            let AnyStore::Log(log) = log_now else {
                panic!("not a log")
            };
            assert_eq!(
                !log.reports.is_empty(),
                manifest_written,
                "{context}: manifest"
            );
            if !manifest_written {
                assert_eq!(
                    std::fs::read(tmp.path("log.json")).unwrap(),
                    log_before,
                    "{context}"
                );
            }
            assert_eq!(
                w.write_report(&report_file(1, TS), &slug()).unwrap_err(),
                StoreError::Halted,
                "{context}"
            );
            drop(w);

            // After reopening, a report that was written without its manifest is reported (not deleted,
            // not guessed), exactly as the write_report documentation says.
            let (w2, rec) = open(&tmp, &v).unwrap();
            assert!(!w2.is_halted(), "{context}");
            let orphans = u32::from(report_written && !manifest_written);
            assert_eq!(rec.reports_without_manifest, orphans, "{context}");
            assert!(rec.manifests_without_report.is_empty(), "{context}");
            assert_eq!(
                tmp.path(&rel).exists(),
                report_written,
                "{context}: reopen changed reports"
            );
        }
    }
}

/// What a failed `restore_previous` leaves behind, by the step that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Restore {
    /// Nothing moved: the bad current file is in place and `quarantine/` has no entry.
    Untouched,
    /// The quarantine entry exists but the bad current file has not been removed yet.
    LinkedOnly,
    /// The bad file is in `quarantine/` and the root is absent; `.prev` is intact.
    MovedAside,
    /// The restored root is in place; only the final directory sync failed.
    Restored,
}

#[test]
fn restore_previous_halts_on_a_storage_error_and_can_be_completed_after_reopening() {
    let plan: [(WriteStep, u32, Restore); 8] = [
        (WriteStep::CreateDir, 1, Restore::Untouched),
        (WriteStep::LinkPrevious, 1, Restore::Untouched),
        (WriteStep::SyncDir, 1, Restore::LinkedOnly),
        (WriteStep::CreateTemp, 1, Restore::MovedAside),
        (WriteStep::WriteTemp, 1, Restore::MovedAside),
        (WriteStep::SyncTemp, 1, Restore::MovedAside),
        (WriteStep::Replace, 1, Restore::MovedAside),
        (WriteStep::SyncDir, 2, Restore::Restored),
    ];
    let bad: &[u8] = b"not json at all";
    for (step, nth, outcome) in plan {
        for kind in KINDS {
            let tmp = Tmp::new("halt-restore");
            let v = valid();
            let prev = store_with_bad_repo(&tmp, &v, bad);

            let fault = Arc::new(FaultFs::new(FaultPlan {
                step,
                nth,
                error: kind,
            }));
            let (mut w, rec) = StoreWriter::open(tmp.data_dir(), fault.clone(), v.clone()).unwrap();
            assert_eq!(rec.malformed_roots, [StoreKind::Repositories]);
            let context = format!("{step:?}#{nth} {kind:?}");

            let err = w.restore_previous(StoreKind::Repositories).unwrap_err();
            assert_eq!(err, StoreError::Io { step, kind }, "{context}");
            assert!(fault.fired() && w.is_halted(), "{context}");
            assert_eq!(
                w.restore_previous(StoreKind::Repositories).unwrap_err(),
                StoreError::Halted,
                "{context}"
            );
            // Whatever failed, the previous generation survives.
            assert_eq!(
                std::fs::read(tmp.path("repo.json.prev")).unwrap(),
                prev,
                "{context}"
            );
            let quarantined: Vec<Vec<u8>> = quarantine_entries(&tmp)
                .iter()
                .map(|n| std::fs::read(tmp.path(&format!("quarantine/{n}"))).unwrap())
                .collect();
            match outcome {
                Restore::Untouched => {
                    assert_eq!(
                        std::fs::read(tmp.path("repo.json")).unwrap(),
                        bad,
                        "{context}"
                    );
                    assert!(quarantined.is_empty(), "{context}");
                }
                Restore::LinkedOnly => {
                    assert_eq!(
                        std::fs::read(tmp.path("repo.json")).unwrap(),
                        bad,
                        "{context}"
                    );
                    assert_eq!(quarantined, [bad.to_vec()], "{context}");
                }
                Restore::MovedAside => {
                    assert!(!tmp.path("repo.json").exists(), "{context}");
                    assert_eq!(quarantined, [bad.to_vec()], "{context}");
                }
                Restore::Restored => {
                    assert_eq!(
                        std::fs::read(tmp.path("repo.json")).unwrap(),
                        prev,
                        "{context}"
                    );
                    assert_eq!(quarantined, [bad.to_vec()], "{context}");
                }
            }
            drop(w);

            // Reopening clears the halt; a moved-aside root is merely absent, not malformed.
            let (mut w2, rec) = open(&tmp, &v).unwrap();
            assert!(!w2.is_halted(), "{context}");
            match outcome {
                Restore::Untouched | Restore::LinkedOnly => {
                    assert_eq!(rec.malformed_roots, [StoreKind::Repositories], "{context}");
                }
                Restore::MovedAside | Restore::Restored => {
                    assert!(rec.malformed_roots.is_empty(), "{context}");
                }
            }
            // The job can be completed unless the first attempt already took this second's quarantine
            // name (LinkedOnly): then the no-overwrite rule refuses a second move in the same second
            // (tests/writer.rs, restore_previous_never_overwrites_a_quarantine_entry).
            match outcome {
                Restore::Untouched => {
                    assert!(
                        w2.restore_previous(StoreKind::Repositories)
                            .unwrap()
                            .quarantined,
                        "{context}"
                    );
                }
                Restore::MovedAside => {
                    assert!(
                        !w2.restore_previous(StoreKind::Repositories)
                            .unwrap()
                            .quarantined,
                        "{context}"
                    );
                }
                Restore::LinkedOnly | Restore::Restored => {}
            }
            if outcome != Restore::LinkedOnly {
                assert_eq!(
                    std::fs::read(tmp.path("repo.json")).unwrap(),
                    prev,
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn non_storage_errors_do_not_halt() {
    let tmp = Tmp::new("no-halt");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let doc = any(StoreKind::Log, &log_doc(TS_LATER));
    v.set(MODE_INVALID);
    assert!(w.replace(&doc).is_err());
    v.set(MODE_UNAVAILABLE);
    assert!(w.replace(&doc).is_err());
    v.set(MODE_VALID);
    assert!(w.restore_previous(StoreKind::Log).is_err(), "no .prev yet");
    assert!(
        !w.is_halted(),
        "validation and lookup errors are not storage errors"
    );
    // Fails if any of those errors latched the halt flag.
    w.replace(&doc).unwrap();
}

// ---------------------------------------------------------------------------------------------------
// write_report

#[test]
fn write_report_writes_the_json_file_and_appends_the_manifest() {
    let tmp = Tmp::new("report");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let report = report_file(0, TS);
    let manifest = w.write_report(&report, &slug()).unwrap();

    let rel = report_rel("20261016T052500Z", 0);
    let bytes = std::fs::read(tmp.path(&rel)).unwrap();
    assert_eq!(manifest.json_path.as_str(), rel);
    assert_eq!(manifest.md_path.as_str(), rel.replace(".json", ".md"));
    assert_eq!(
        manifest.pdf_path.as_ref().map(|p| p.as_str().to_owned()),
        Some(rel.replace(".json", ".pdf"))
    );
    assert!(manifest.json_written);
    assert!(!manifest.md_written && !manifest.pdf_written);
    assert!(manifest.pdf_digest.is_none() && manifest.pdf_error_redacted.is_none());
    assert!(manifest.pdf_renderer_version.is_none());
    // Fails if the digest is computed over anything but the exact bytes on disk.
    assert_eq!(
        manifest.json_digest.as_ref().map(|d| d.as_str().to_owned()),
        Some(sha256_hex(&bytes))
    );
    assert_eq!(manifest.report_id, report.report_id);
    assert_eq!(manifest.run_id, report.run_id);
    assert_eq!(manifest.repo_id, report.repo_id);
    assert_eq!(manifest.generated_at, report.generated_at);

    // The file is the validated report document, read back through the same decode path.
    let on_disk = decode(StoreKind::Report, &bytes, &*valid()).unwrap();
    assert_eq!(on_disk, AnyStore::Report(Box::new(report)));
    assert_eq!(mode_of(&tmp.path("backup")), 0o700);
    assert_eq!(mode_of(&tmp.path(&rel)), 0o600);

    // The manifest was appended to log.json, which kept a previous generation.
    let AnyStore::Log(log) = w.read(StoreKind::Log).unwrap() else {
        panic!("not a log")
    };
    assert_eq!(log.reports, [manifest]);
    assert!(tmp.path("log.json.prev").is_file());
    assert!(!has_temp(&snapshot(&tmp.root)));
    // Only the .json is written in P02.
    assert!(!tmp.path(&rel.replace(".json", ".md")).exists());
    assert!(!tmp.path(&rel.replace(".json", ".pdf")).exists());
}

#[test]
fn reports_in_the_same_second_get_distinct_names_and_exact_repeats_collide() {
    let tmp = Tmp::new("report-collide");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let m0 = w.write_report(&report_file(0, TS), &slug()).unwrap();
    // Same repository, same second, different run (and report) ID.
    let m1 = w.write_report(&report_file(1, TS), &slug()).unwrap();
    assert_ne!(
        m0.json_path, m1.json_path,
        "fails if the run ID is left out of the name"
    );
    assert!(tmp.path(m0.json_path.as_str()).is_file());
    assert!(tmp.path(m1.json_path.as_str()).is_file());

    let first_bytes = std::fs::read(tmp.path(m0.json_path.as_str())).unwrap();
    let log_before = std::fs::read(tmp.path("log.json")).unwrap();
    let snap_before = snapshot(&tmp.root);
    // An exact repeat (same run, same second) must not overwrite.
    assert_eq!(
        w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
        StoreError::ReportNameCollision
    );
    assert_eq!(
        std::fs::read(tmp.path(m0.json_path.as_str())).unwrap(),
        first_bytes
    );
    assert_eq!(std::fs::read(tmp.path("log.json")).unwrap(), log_before);
    assert_eq!(
        snapshot(&tmp.root),
        snap_before,
        "a collision changed the store"
    );
    assert!(!w.is_halted(), "a name collision is not a storage error");

    let AnyStore::Log(log) = w.read(StoreKind::Log).unwrap() else {
        panic!("not a log")
    };
    assert_eq!(
        log.reports,
        [m0, m1],
        "manifests are appended in write order"
    );
}

#[test]
fn an_existing_report_file_is_never_overwritten() {
    let tmp = Tmp::new("report-exists");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    // A file with the generated name exists, but the log knows nothing about it (for example after a
    // crash between the two steps): the file system, not the log, is the last line of defence.
    std::fs::create_dir(tmp.path("backup")).unwrap();
    set_mode(&tmp.path("backup"), 0o700);
    let rel = report_rel("20261016T052500Z", 0);
    std::fs::write(tmp.path(&rel), b"first generation").unwrap();
    let before = snapshot(&tmp.root);
    assert_eq!(
        w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
        StoreError::ReportNameCollision
    );
    assert_eq!(std::fs::read(tmp.path(&rel)).unwrap(), b"first generation");
    assert_eq!(
        snapshot(&tmp.root),
        before,
        "temp file or log change left behind"
    );
    assert!(!w.is_halted());
}

#[test]
fn write_report_validates_the_report_and_the_new_log_before_writing_anything() {
    let tmp = Tmp::new("report-validate");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let before = snapshot(&tmp.root);

    // An invalid report: nothing at all is written, not even backup/.
    v.reject_only(StoreKind::Report);
    let err = w.write_report(&report_file(0, TS), &slug()).unwrap_err();
    assert_eq!(
        err,
        StoreError::Invalid(StoreKind::Report, marker_problems())
    );
    assert_eq!(snapshot(&tmp.root), before);

    // A valid report whose new log would be invalid (for example a duplicate ID): the report file must not
    // be written either, so no orphan is created. Fails if the log is validated after the report is on disk.
    // (The current, manifest-free log still reads; only the log with the appended manifest is rejected.)
    v.set(MODE_LOG_WITH_MANIFEST);
    let err = w.write_report(&report_file(0, TS), &slug()).unwrap_err();
    assert_eq!(err, StoreError::Invalid(StoreKind::Log, marker_problems()));
    assert_eq!(snapshot(&tmp.root), before);

    v.set(MODE_UNAVAILABLE);
    assert_eq!(
        w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
        StoreError::SchemaUnavailable
    );
    assert_eq!(snapshot(&tmp.root), before);
    assert!(!w.is_halted());
}

#[test]
fn write_report_without_an_initialised_log_writes_nothing() {
    let tmp = Tmp::new("report-no-log");
    let v = valid();
    let (mut w, _) = open(&tmp, &v).unwrap();
    assert_eq!(
        w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
        StoreError::NotInitialised(StoreKind::Log)
    );
    assert!(
        snapshot(&tmp.root).is_empty(),
        "wrote {:?}",
        snapshot(&tmp.root).keys()
    );
}

#[test]
fn write_report_refuses_a_blocked_log() {
    let tmp = Tmp::new("report-blocked");
    let v = valid();
    drop(init_writer(&tmp, &v));
    std::fs::write(tmp.path("log.json"), b"[]").unwrap();
    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.malformed_roots, [StoreKind::Log]);
    let before = snapshot(&tmp.root);
    assert_eq!(
        w.write_report(&report_file(0, TS), &slug()).unwrap_err(),
        StoreError::Malformed(StoreKind::Log)
    );
    assert_eq!(snapshot(&tmp.root), before);
}

#[test]
fn the_longest_slug_gives_a_name_within_the_bound() {
    let tmp = Tmp::new("report-long");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let long = Slug::parse(&"a".repeat(48)).unwrap();
    let manifest = w.write_report(&report_file(0, TS), &long).unwrap();
    let name = manifest.json_path.as_str().strip_prefix("backup/").unwrap();
    assert!(
        name.len() <= persistence::paths::MAX_REPORT_NAME_BYTES,
        "{} bytes",
        name.len()
    );
    assert!(tmp.path(manifest.json_path.as_str()).is_file());
}

#[test]
fn a_distinct_second_gives_a_distinct_name_for_the_same_run() {
    let tmp = Tmp::new("report-seconds");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let a = w
        .write_report(&report_file(0, "2026-10-16T05:25:00Z"), &slug())
        .unwrap();
    let b = w
        .write_report(&report_file(0, "2026-10-16T05:25:01Z"), &slug())
        .unwrap();
    assert_ne!(a.json_path, b.json_path);
}

// ---------------------------------------------------------------------------------------------------
// restore_previous

/// A store whose `repo.json` holds `garbage` and whose `repo.json.prev` holds a valid earlier generation.
fn store_with_bad_repo(tmp: &Tmp, v: &Arc<Switch>, garbage: &[u8]) -> Vec<u8> {
    let mut w = init_writer(tmp, v);
    w.replace(&any(
        StoreKind::Repositories,
        &file_doc("repositories", "repositories"),
    ))
    .unwrap();
    drop(w);
    let prev = std::fs::read(tmp.path("repo.json.prev")).unwrap();
    std::fs::write(tmp.path("repo.json"), garbage).unwrap();
    prev
}

fn quarantine_entries(tmp: &Tmp) -> Vec<String> {
    match std::fs::read_dir(tmp.path("quarantine")) {
        Ok(rd) => rd
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn a_malformed_root_is_quarantined_and_restored_only_by_restore_previous() {
    let tmp = Tmp::new("restore");
    let v = valid();
    let prev = store_with_bad_repo(&tmp, &v, b"{ broken");
    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.malformed_roots, [StoreKind::Repositories]);

    // Before the explicit restore, nothing moves and writes are refused.
    assert_eq!(std::fs::read(tmp.path("repo.json")).unwrap(), b"{ broken");
    assert!(
        quarantine_entries(&tmp).is_empty(),
        "open must not quarantine"
    );
    let doc = any(
        StoreKind::Repositories,
        &file_doc("repositories", "repositories"),
    );
    assert_eq!(
        w.replace(&doc).unwrap_err(),
        StoreError::Malformed(StoreKind::Repositories)
    );
    assert_eq!(
        std::fs::read(tmp.path("repo.json")).unwrap(),
        b"{ broken",
        "replace must not touch it"
    );
    assert_eq!(
        std::fs::read(tmp.path("repo.json.prev")).unwrap(),
        prev,
        "no rotation into .prev (D02-9)"
    );

    let done = w.restore_previous(StoreKind::Repositories).unwrap();
    assert_eq!(
        (done.kind, done.quarantined),
        (StoreKind::Repositories, true)
    );
    // The current file is now exactly the validated previous generation.
    assert_eq!(std::fs::read(tmp.path("repo.json")).unwrap(), prev);
    assert_eq!(std::fs::read(tmp.path("repo.json.prev")).unwrap(), prev);
    // The bad bytes survive in quarantine/<name>.<UTC stamp>, in a private directory.
    let entries = quarantine_entries(&tmp);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let stamp = entries[0].strip_prefix("repo.json.").expect("name prefix");
    assert_eq!(stamp.len(), 16, "{stamp}");
    assert!(
        stamp.bytes().enumerate().all(|(i, b)| match i {
            8 => b == b'T',
            15 => b == b'Z',
            _ => b.is_ascii_digit(),
        }),
        "stamp shape: {stamp}"
    );
    assert_eq!(
        std::fs::read(tmp.path(&format!("quarantine/{}", entries[0]))).unwrap(),
        b"{ broken"
    );
    assert_eq!(mode_of(&tmp.path("quarantine")), 0o700);
    assert_eq!(mode_of(&tmp.path("repo.json")), 0o600);
    assert!(!has_temp(&snapshot(&tmp.root)));

    // The root is unblocked: a normal replace works again and reads return the restored document.
    assert_eq!(
        w.read(StoreKind::Repositories).unwrap(),
        decode(StoreKind::Repositories, &prev, &*valid()).unwrap()
    );
    w.replace(&doc).unwrap();
}

#[test]
fn restore_previous_with_an_invalid_or_missing_previous_changes_nothing() {
    for (label, prev_bytes) in [
        ("invalid", Some(b"{ also broken".to_vec())),
        ("foreign", Some(serde_json::to_vec(&log_doc(TS)).unwrap())),
        ("missing", None),
    ] {
        let tmp = Tmp::new("restore-invalid");
        let v = valid();
        drop(init_writer(&tmp, &v));
        std::fs::write(tmp.path("repo.json"), b"bad current").unwrap();
        if let Some(bytes) = &prev_bytes {
            std::fs::write(tmp.path("repo.json.prev"), bytes).unwrap();
        }
        let (mut w, _) = open(&tmp, &v).unwrap();
        let before = snapshot(&tmp.root);
        let err = w.restore_previous(StoreKind::Repositories).unwrap_err();
        match (label, &err) {
            ("missing", StoreError::NotInitialised(StoreKind::Repositories)) => {}
            ("invalid" | "foreign", StoreError::Invalid(StoreKind::Repositories, _)) => {}
            _ => panic!("{label}: unexpected {err:?}"),
        }
        // Fails if the current file is moved before the previous generation has been validated.
        assert_eq!(
            snapshot(&tmp.root),
            before,
            "{label}: restore changed something"
        );
        assert!(!tmp.path("quarantine").exists(), "{label}");
        assert!(!w.is_halted(), "{label}");
        // And the root stays blocked.
        let doc = any(
            StoreKind::Repositories,
            &file_doc("repositories", "repositories"),
        );
        assert_eq!(
            w.replace(&doc).unwrap_err(),
            StoreError::Malformed(StoreKind::Repositories),
            "{label}"
        );
    }
}

#[test]
fn restore_previous_never_overwrites_a_quarantine_entry() {
    let tmp = Tmp::new("restore-collide");
    let v = valid();
    store_with_bad_repo(&tmp, &v, b"{ broken");
    let (mut w, _) = open(&tmp, &v).unwrap();
    // Occupy every name the next restore could pick: the stamp is the current second, so cover a window
    // around it (the test would need to stall for seconds to step outside).
    std::fs::create_dir(tmp.path("quarantine")).unwrap();
    set_mode(&tmp.path("quarantine"), 0o700);
    let now = UtcTimestamp::now().unix_seconds();
    let mut decoys = Vec::new();
    for delta in -3..=5 {
        let at = UtcTimestamp::from_unix(now + delta, 0).unwrap();
        let stamp = at.to_rfc3339().replace(['-', ':'], "");
        let name = format!("repo.json.{stamp}");
        std::fs::write(
            tmp.path(&format!("quarantine/{name}")),
            format!("decoy {delta}"),
        )
        .unwrap();
        decoys.push((name, format!("decoy {delta}")));
    }
    let before = snapshot(&tmp.root);

    let err = w.restore_previous(StoreKind::Repositories).unwrap_err();
    // Fails if the move replaces an existing quarantine entry (plain rename instead of link-then-remove).
    assert_eq!(err, StoreError::PathRejected);
    assert_eq!(
        snapshot(&tmp.root),
        before,
        "a collision must change nothing"
    );
    for (name, content) in decoys {
        assert_eq!(
            std::fs::read(tmp.path(&format!("quarantine/{name}"))).unwrap(),
            content.as_bytes()
        );
    }
    assert!(!w.is_halted(), "a collision is not a storage error");
    assert_eq!(
        w.replace(&any(
            StoreKind::Repositories,
            &file_doc("repositories", "repositories")
        ))
        .unwrap_err(),
        StoreError::Malformed(StoreKind::Repositories),
        "still blocked"
    );
}

#[test]
fn restore_previous_refuses_a_group_readable_or_symlinked_quarantine_directory() {
    for symlink in [false, true] {
        let tmp = Tmp::new("restore-quarantine-dir");
        let v = valid();
        store_with_bad_repo(&tmp, &v, b"{ broken");
        let (mut w, _) = open(&tmp, &v).unwrap();
        if symlink {
            std::fs::create_dir(tmp.path("elsewhere")).unwrap();
            set_mode(&tmp.path("elsewhere"), 0o700);
            std::os::unix::fs::symlink(tmp.path("elsewhere"), tmp.path("quarantine")).unwrap();
        } else {
            std::fs::create_dir(tmp.path("quarantine")).unwrap();
            set_mode(&tmp.path("quarantine"), 0o755);
        }
        let before = snapshot(&tmp.root);
        // R-D03-8: an existing directory is used only if it is a private real directory.
        assert_eq!(
            w.restore_previous(StoreKind::Repositories).unwrap_err(),
            StoreError::PathRejected,
            "symlink={symlink}"
        );
        assert_eq!(
            snapshot(&tmp.root),
            before,
            "symlink={symlink}: the bad file must stay in place"
        );
    }
}

#[test]
fn restore_previous_without_a_current_file_just_restores() {
    let tmp = Tmp::new("restore-absent");
    let v = valid();
    let prev = store_with_bad_repo(&tmp, &v, b"x");
    std::fs::remove_file(tmp.path("repo.json")).unwrap();
    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert!(
        rec.malformed_roots.is_empty(),
        "an absent root is not malformed"
    );
    let done = w.restore_previous(StoreKind::Repositories).unwrap();
    assert!(!done.quarantined);
    assert_eq!(std::fs::read(tmp.path("repo.json")).unwrap(), prev);
    assert!(
        !tmp.path("quarantine").exists(),
        "nothing to quarantine, nothing created"
    );
}

#[test]
fn restore_previous_of_a_valid_root_quarantines_it_too() {
    let tmp = Tmp::new("restore-valid");
    let v = valid();
    let mut w = init_writer(&tmp, &v);
    let first = std::fs::read(tmp.path("log.json")).unwrap();
    w.replace(&any(StoreKind::Log, &log_doc(TS_LATER))).unwrap();
    let second = std::fs::read(tmp.path("log.json")).unwrap();
    let done = w.restore_previous(StoreKind::Log).unwrap();
    assert!(done.quarantined);
    assert_eq!(
        std::fs::read(tmp.path("log.json")).unwrap(),
        first,
        "rolled back to .prev"
    );
    let entries = quarantine_entries(&tmp);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        std::fs::read(tmp.path(&format!("quarantine/{}", entries[0]))).unwrap(),
        second
    );
}

#[test]
fn restore_previous_does_not_follow_a_symlinked_current_file() {
    let tmp = Tmp::new("restore-symlink");
    let v = valid();
    let prev = store_with_bad_repo(&tmp, &v, b"x");
    let target = tmp.path("target.txt");
    std::fs::write(&target, b"symlink target").unwrap();
    std::fs::remove_file(tmp.path("repo.json")).unwrap();
    std::os::unix::fs::symlink(&target, tmp.path("repo.json")).unwrap();
    let (mut w, rec) = open(&tmp, &v).unwrap();
    assert_eq!(rec.malformed_roots, [StoreKind::Repositories]);
    let done = w.restore_previous(StoreKind::Repositories).unwrap();
    assert!(done.quarantined);
    // The symlink itself is what moved; its target was neither read nor changed.
    assert_eq!(std::fs::read(&target).unwrap(), b"symlink target");
    assert_eq!(std::fs::read(tmp.path("repo.json")).unwrap(), prev);
    let entries = quarantine_entries(&tmp);
    assert_eq!(entries.len(), 1);
    assert!(
        std::fs::symlink_metadata(tmp.path(&format!("quarantine/{}", entries[0])))
            .unwrap()
            .is_symlink()
    );
}

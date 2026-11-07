//! P02-T14 failure scenarios for the `dev` store entry points (`planning/P02-T01-request.md` §6.3, §6.5,
//! §9 rows T07/T10/T12; closes L-T12-2 at the application level).
//!
//! Every test drives `application::dev_store::{init_with, validate_with, show_with, recover_with}` (the
//! fault-injection seam) over `persistence::fault::FaultFs` or a crash-simulating wrapper, with the real
//! `SchemaValidator`, against a private directory under `CARGO_TARGET_TMPDIR`. The CLI maps
//! [`FailureClass`] onto exit codes in `crates/cli`; this crate cannot depend on it, so the class half of
//! the "storage failure is exit 4" pair is asserted here and the exit-code half in
//! `crates/cli/tests/p02_failures.rs`.
//!
//! Expected values (which steps an operation reaches, how many times, how many temp files a crash leaves)
//! are written out literally. Each test says what would make it fail. All data is synthetic; nothing
//! touches the network or any path outside the test directories.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use application::dev_store::{
    self, DevError, DevOutcome, DevRecord, DevVerdict, FailureClass, init_with, recover_with,
    show_with, validate_with,
};
use domain::states::RunState;
use domain::store::values::Slug;
use domain::store::{AnyStore, StoreKind};
use persistence::atomic::is_temp_file_name;
use persistence::error::WriteStep;
use persistence::fault::{FaultFs, FaultPlan};
use persistence::fs::{FileInfo, FsOps, LockHandle, RealFs};
use persistence::paths::DataDir;
use persistence::read::{SchemaValidator, decode};
use persistence::validate::DocumentValidator;
use persistence::writer::StoreWriter;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../persistence/tests/fixtures/store"
);
const TS: &str = "2026-10-16T05:25:00Z";
const FIXED_MESSAGE: &str = "a storage operation failed";

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

/// ENOSPC, EACCES and EINTR as the kinds `std::io` reports them.
const ERRORS: [io::ErrorKind; 3] = [
    io::ErrorKind::StorageFull,
    io::ErrorKind::PermissionDenied,
    io::ErrorKind::Interrupted,
];

// ---------------------------------------------------------------------------------------------------
// helpers

/// A private (0700) data directory removed on drop.
struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("faults-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn data_dir(&self) -> DataDir {
        DataDir::new(&RealFs, &self.root).unwrap()
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn validator() -> Arc<dyn DocumentValidator> {
    Arc::new(SchemaValidator::new())
}

/// Relative path -> content of every file under `root`, directories as `<path>/`, except the lock file
/// (its diagnostic line is rewritten by every locking command).
type Snapshot = BTreeMap<String, Vec<u8>>;

fn snapshot(root: &Path) -> Snapshot {
    fn walk(base: &Path, dir: &Path, out: &mut Snapshot) {
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
            if std::fs::symlink_metadata(&path).unwrap().is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(base, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, root, &mut out);
    out
}

fn temp_names(snap: &Snapshot) -> Vec<&String> {
    snap.keys().filter(|k| k.contains(".tmp-")).collect()
}

/// Fails if a file that existed before is missing or different afterwards, other than the `except` names.
fn assert_preserved(before: &Snapshot, after: &Snapshot, except: &[&str], context: &str) {
    for (name, bytes) in before {
        if except.contains(&name.as_str()) {
            continue;
        }
        match after.get(name) {
            Some(now) => assert_eq!(now, bytes, "{context}: {name} changed"),
            None => panic!("{context}: {name} disappeared"),
        }
    }
}

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn write_private(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

/// A valid log holding one run in state `running` (the `log.max` run template), so that reconciliation has
/// something to rewrite.
fn log_with_running_run() -> Value {
    let mut log = fixture("log.max.json");
    let mut run = log["runs"][0].clone();
    run["run_id"] = json!("00000000-0000-4000-8000-000000000901");
    run["execution"] = json!("running");
    run["created_at"] = json!(TS);
    run["started_at"] = json!(TS);
    run["finished_at"] = Value::Null;
    run["last_checkpoint_at"] = Value::Null;
    run["report_ids"] = json!([]);
    log["runs"] = json!([run]);
    log["events"] = json!([]);
    log["reports"] = json!([]);
    log
}

/// `init` on a fresh directory, asserted to succeed.
fn init_clean(tmp: &Tmp) {
    let outcome = dev_store::init(&tmp.root);
    assert_eq!(outcome.failure, None);
}

/// `init`, then a `log.json` with one in-flight run: reconciliation must rewrite the log.
fn seed_recoverable(tmp: &Tmp) {
    init_clean(tmp);
    write_private(
        &tmp.path("log.json"),
        &serde_json::to_vec_pretty(&log_with_running_run()).unwrap(),
    );
}

/// `init`, then `log.json` and `tools.json` removed: three roots exist, two are missing.
fn seed_partial_init(tmp: &Tmp) {
    init_clean(tmp);
    std::fs::remove_file(tmp.path("log.json")).unwrap();
    std::fs::remove_file(tmp.path("tools.json")).unwrap();
}

/// What a clean `recover` reported.
struct Reconciled {
    temp_files_removed: u32,
    runs_interrupted: usize,
    malformed: usize,
}

/// A clean (fault-free) `recover` over the real file system, asserted to succeed.
fn clean_recover(tmp: &Tmp) -> Reconciled {
    let outcome = recover_with(Arc::new(RealFs), validator(), &tmp.root, None);
    assert_eq!(outcome.failure, None, "a clean recover must succeed");
    assert_eq!(outcome.records.len(), 1);
    match &outcome.records[0] {
        DevRecord::StoreReconciled {
            temp_files_removed,
            runs_interrupted,
            malformed_roots,
            ..
        } => Reconciled {
            temp_files_removed: *temp_files_removed,
            runs_interrupted: runs_interrupted.len(),
            malformed: malformed_roots.len(),
        },
        other => panic!("expected StoreReconciled, got {other:?}"),
    }
}

/// Which roots `validate` finds valid, by name, after asserting it succeeds.
fn valid_roots(tmp: &Tmp) -> Vec<&'static str> {
    let outcome = dev_store::validate(&tmp.root, None);
    assert_eq!(outcome.failure, None);
    outcome
        .records
        .iter()
        .filter_map(|r| match r {
            DevRecord::StoreValidated { kind, verdict, .. } => {
                assert_ne!(*verdict, DevVerdict::Invalid, "{kind:?} is invalid");
                (*verdict == DevVerdict::Valid).then(|| dev_store::root_name(*kind))
            }
            _ => None,
        })
        .collect()
}

/// The generic assertions every fired fault must satisfy, then the scenario's own `check`.
///
/// For every error kind, every step and every call index `nth` of that step: the fault is injected; the
/// outcome is exactly `DevError::Storage` (class `Storage`, fixed text, no path) with no record; and the
/// leftover state is judged by `check`. A step that is never reached must leave the operation successful.
/// Returns, per step, how many calls fired (the number of `nth` values that fired), after asserting that
/// the three error kinds reach the same calls.
fn matrix(
    label: &str,
    setup: &dyn Fn(&Tmp),
    operate: &dyn Fn(&Tmp, Arc<FaultFs>) -> DevOutcome,
    check: &dyn Fn(&Tmp, &Snapshot, WriteStep),
) -> HashMap<WriteStep, u32> {
    let mut reached: Option<HashMap<WriteStep, u32>> = None;
    for error in ERRORS {
        let mut counts = HashMap::new();
        for step in ALL_STEPS {
            for nth in 1u32.. {
                assert!(nth < 64, "{label}: runaway fault index at {step:?}");
                let tmp = Tmp::new(label);
                setup(&tmp);
                let before = snapshot(&tmp.root);
                let fault = Arc::new(FaultFs::new(FaultPlan { step, nth, error }));
                let outcome = operate(&tmp, Arc::clone(&fault));
                let context = format!("{label}: {error:?} at {step:?} call {nth}");
                if !fault.fired() {
                    assert_eq!(
                        outcome.failure, None,
                        "{context}: no fault fired but the operation failed"
                    );
                    break;
                }
                let failure = outcome
                    .failure
                    .unwrap_or_else(|| panic!("{context}: an injected fault was swallowed"));
                assert_eq!(failure, DevError::Storage, "{context}");
                assert_eq!(failure.class(), FailureClass::Storage, "{context}");
                assert_eq!(failure.to_string(), FIXED_MESSAGE, "{context}");
                assert!(outcome.records.is_empty(), "{context}: records on failure");
                let after = snapshot(&tmp.root);
                assert!(
                    temp_names(&after).is_empty(),
                    "{context}: the writer left a temp file after cleanup: {:?}",
                    temp_names(&after)
                );
                check(&tmp, &before, step);
                *counts.entry(step).or_insert(0u32) += 1;
            }
        }
        match &reached {
            None => reached = Some(counts),
            Some(first) => assert_eq!(first, &counts, "{label}: {error:?} reaches other calls"),
        }
    }
    reached.unwrap()
}

// ---------------------------------------------------------------------------------------------------
// 1 + 2: write faults (ENOSPC, EACCES, EINTR) at every step of init and recover

/// Fails if `init` over an empty directory (a) reaches a different set of protocol calls than the five
/// root writes plus the lock (the literal table below), (b) maps any fault at any reached call to anything
/// but `Storage`, (c) leaves a temp file, a partial root or a root the validator rejects, or (d) cannot be
/// completed by a clean `init` afterwards. A step the table does not list must not fire (a changed protocol
/// is noticed).
#[test]
fn init_on_an_empty_directory_survives_every_fault() {
    let reached = matrix(
        "init-empty",
        &|_| {},
        &|tmp, fault| init_with(fault, validator(), &tmp.root),
        &|tmp, before, step| {
            let after = snapshot(&tmp.root);
            assert_preserved(before, &after, &[], "init-empty");
            // Whatever was created before the fault is a complete, valid root: no partial file exists.
            let rec = clean_recover(tmp);
            assert_eq!(rec.temp_files_removed, 0, "{step:?}");
            assert_eq!(rec.malformed, 0, "{step:?}: a root was left malformed");
            assert_eq!(rec.runs_interrupted, 0);
            assert_no_temps(&snapshot(&tmp.root), "init-empty");
            // A repeat completes the store: five valid roots.
            assert_eq!(dev_store::init(&tmp.root).failure, None);
            let valid = valid_roots(tmp);
            assert_eq!(
                valid,
                [
                    "repositories",
                    "workflows",
                    "approvals",
                    "log",
                    "tool_manifest"
                ]
            );
        },
    );
    let expected: HashMap<WriteStep, u32> = [
        (WriteStep::Lock, 1),
        (WriteStep::CreateTemp, 5),
        (WriteStep::WriteTemp, 5),
        (WriteStep::SyncTemp, 5),
        (WriteStep::Replace, 5),
        (WriteStep::SyncDir, 5),
    ]
    .into_iter()
    .collect();
    assert_eq!(reached, expected);
}

/// Fails if `init` over a partly initialised store (repo, workflows, approvals present; log, tools
/// missing) changes a byte of an existing root under any fault, or reaches other calls than the literal
/// table (the three existing roots are read during the 1.5 check and again during initialization).
/// The three existing roots stay byte-identical.
#[test]
fn init_over_a_partial_store_never_touches_existing_roots() {
    let reached = matrix(
        "init-partial",
        &seed_partial_init,
        &|tmp, fault| init_with(fault, validator(), &tmp.root),
        &|tmp, before, step| {
            let after = snapshot(&tmp.root);
            assert_preserved(before, &after, &[], "init-partial");
            for name in ["repo.json", "workflows.json", "approvals.json"] {
                assert_eq!(after.get(name), before.get(name), "{step:?}: {name}");
            }
            let rec = clean_recover(tmp);
            assert_eq!(rec.temp_files_removed, 0, "{step:?}");
            assert_eq!(rec.malformed, 0);
            assert_no_temps(&snapshot(&tmp.root), "init-partial");
            assert_eq!(dev_store::init(&tmp.root).failure, None);
            assert_eq!(valid_roots(tmp).len(), 5);
        },
    );
    let expected: HashMap<WriteStep, u32> = [
        (WriteStep::Lock, 1),
        (WriteStep::Read, 6),
        (WriteStep::CreateTemp, 2),
        (WriteStep::WriteTemp, 2),
        (WriteStep::SyncTemp, 2),
        (WriteStep::Replace, 2),
        (WriteStep::SyncDir, 2),
    ]
    .into_iter()
    .collect();
    assert_eq!(reached, expected);
}

/// Fails if `recover` with an interrupted run (reconciliation rewrites `log.json` through the replace
/// protocol with a `.prev` generation) (a) reaches other calls than the literal table (five roots read
/// during the 1.5 check and again during reconciliation, and
/// each of the seven write steps once), (b) changes any file other than `log.json`, (c) changes `log.json`
/// under any fault except after the publishing rename (`SyncDir`, where the new content is in place and is a
/// valid log with the run interrupted), (d) leaves a `.prev` that is not a byte copy of the old log, or (e)
/// does not finish the job on the next clean `recover` (one run interrupted unless the SyncDir fault already
/// published it).
#[test]
fn recover_rewriting_the_log_survives_every_fault() {
    let reached = matrix(
        "recover",
        &seed_recoverable,
        &|tmp, fault| recover_with(fault, validator(), &tmp.root, None),
        &|tmp, before, step| {
            let after = snapshot(&tmp.root);
            let published = step == WriteStep::SyncDir;
            let except: &[&str] = if published { &["log.json"] } else { &[] };
            assert_preserved(before, &after, except, "recover");
            if let Some(prev) = after.get("log.json.prev") {
                assert_eq!(
                    Some(prev),
                    before.get("log.json"),
                    "{step:?}: .prev is not a copy of the old log"
                );
            }
            if published {
                let AnyStore::Log(log) =
                    decode(StoreKind::Log, &after["log.json"], &SchemaValidator::new())
                        .expect("the published log is valid")
                else {
                    panic!("log.json decodes as a log");
                };
                assert_eq!(log.runs.len(), 1);
                assert_eq!(log.runs[0].execution, RunState::Interrupted);
            }
            let rec = clean_recover(tmp);
            assert_eq!(rec.temp_files_removed, 0, "{step:?}");
            assert_eq!(rec.malformed, 0);
            assert_eq!(
                rec.runs_interrupted,
                usize::from(!published),
                "{step:?}: the next recover must finish the job exactly once"
            );
            // No temp file of any kind, also after a failure at Replace (finding F-1, fixed).
            assert_no_temps(&snapshot(&tmp.root), "recover");
            assert_eq!(valid_roots(tmp).len(), 5);
        },
    );
    let expected: HashMap<WriteStep, u32> = [
        (WriteStep::Lock, 1),
        (WriteStep::Read, 10),
        (WriteStep::CreateTemp, 1),
        (WriteStep::WriteTemp, 1),
        (WriteStep::SyncTemp, 1),
        (WriteStep::LinkPrevious, 1),
        (WriteStep::ReplacePrevious, 1),
        (WriteStep::Replace, 1),
        (WriteStep::SyncDir, 1),
    ]
    .into_iter()
    .collect();
    assert_eq!(reached, expected);
}

/// Finding F-1 (fixed in P02-D07): after a storage failure at the
/// `Replace` step of the reconciliation rewrite, `log.json.prev` is a hard link to the same inode as
/// `log.json`. The next successful rewrite hard-links the current log to `.log.json.prev.tmp-<hex>` and
/// renames that over `log.json.prev`; `rename(2)` of two names of one inode does nothing and succeeds, so the
/// temp name stayed behind although the rewrite reported success and the application forgot the name, a
/// violation of "no stray temp file after a successful write". Fails if `atomic::write_replace` leaves that
/// name.
#[test]
fn f1_a_rewrite_after_a_failed_replace_leaves_no_stray_prev_temp() {
    let tmp = Tmp::new("f1");
    seed_recoverable(&tmp);
    let fault = Arc::new(FaultFs::new(FaultPlan {
        step: WriteStep::Replace,
        nth: 1,
        error: io::ErrorKind::StorageFull,
    }));
    let failed = recover_with(fault, validator(), &tmp.root, None);
    assert_eq!(failed.failure, Some(DevError::Storage));
    assert!(temp_names(&snapshot(&tmp.root)).is_empty());
    assert_eq!(clean_recover(&tmp).runs_interrupted, 1);
    assert_eq!(
        temp_names(&snapshot(&tmp.root)),
        Vec::<&String>::new(),
        "a successful rewrite left a temp file behind"
    );
}

// ---------------------------------------------------------------------------------------------------
// 2: a process that dies at a step leaves temp files that the next recover reports and removes

/// A file system that behaves like [`RealFs`] until the `nth` call of `step`, which fails with
/// `Interrupted`; from then on **every** call fails, including `remove_file`, as if the process had died at
/// that point. The application's own cleanup therefore cannot run, which is what leaves a real temp file.
#[derive(Debug)]
struct CrashFs {
    inner: RealFs,
    step: WriteStep,
    nth: u32,
    seen: AtomicU32,
    dead: AtomicBool,
}

impl CrashFs {
    fn new(step: WriteStep, nth: u32) -> Self {
        Self {
            inner: RealFs,
            step,
            nth,
            seen: AtomicU32::new(0),
            dead: AtomicBool::new(false),
        }
    }

    fn died(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    fn gone() -> io::Error {
        io::ErrorKind::Interrupted.into()
    }

    /// Fails if the process is already dead, or dies now.
    fn gate(&self, step: WriteStep) -> io::Result<()> {
        if self.died() {
            return Err(Self::gone());
        }
        if step == self.step && self.seen.fetch_add(1, Ordering::SeqCst) + 1 == self.nth {
            self.dead.store(true, Ordering::SeqCst);
            return Err(Self::gone());
        }
        Ok(())
    }

    /// Fails only if the process is already dead (calls that are not protocol steps).
    fn alive(&self) -> io::Result<()> {
        if self.died() {
            Err(Self::gone())
        } else {
            Ok(())
        }
    }
}

impl FsOps for CrashFs {
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
}

/// Fails if, after a retry that succeeded, any temp file is left (no tolerance; the former finding F-1
/// allowance for a single `.log.json.prev.tmp-` stray was removed when the defect was fixed).
fn assert_no_temps(after: &Snapshot, context: &str) {
    let temps = temp_names(after);
    assert!(
        temps.is_empty(),
        "{context}: temp file left after a successful retry: {temps:?}"
    );
}

/// One row of the crash matrix: the process dies at the `nth` call of `step`, for every `nth`.
struct CrashRow {
    step: WriteStep,
    /// How many temp files the death leaves.
    temps_left: usize,
}

const fn row(step: WriteStep, temps_left: usize) -> CrashRow {
    CrashRow { step, temps_left }
}

/// Runs the crash matrix: for each row and every call index of its step, the process "dies" there, the
/// leftover temp files are counted, and a clean `recover` must report and remove exactly that many. A
/// second `recover` must find nothing left to remove. Every mismatch panics with the
/// step.
fn crash_rows(
    label: &str,
    setup: &dyn Fn(&Tmp),
    operate: &dyn Fn(&Tmp, Arc<CrashFs>) -> DevOutcome,
    rows: &[CrashRow],
    finish: &dyn Fn(&Tmp, &Snapshot, &Snapshot, WriteStep, &Reconciled),
) {
    for CrashRow { step, temps_left } in rows {
        let (step, temps_left) = (*step, *temps_left);
        let mut fired_any = false;
        for nth in 1u32.. {
            assert!(nth < 64, "{label}: runaway fault index at {step:?}");
            let tmp = Tmp::new(label);
            setup(&tmp);
            let before = snapshot(&tmp.root);
            let crash = Arc::new(CrashFs::new(step, nth));
            let outcome = operate(&tmp, Arc::clone(&crash));
            let context = format!("{label}: death at {step:?} call {nth}");
            if !crash.died() {
                assert_eq!(outcome.failure, None, "{context}");
                break;
            }
            fired_any = true;
            assert_eq!(outcome.failure, Some(DevError::Storage), "{context}");
            let after_crash = snapshot(&tmp.root);
            let temps = temp_names(&after_crash);
            assert_eq!(temps.len(), temps_left, "{context}: temps {temps:?}");
            for name in &temps {
                let base = name.rsplit('/').next().unwrap();
                assert!(
                    is_temp_file_name(base),
                    "{context}: {base} is not in the application's temp pattern"
                );
            }
            let rec = clean_recover(&tmp);
            assert_eq!(
                usize::try_from(rec.temp_files_removed).unwrap(),
                temps_left,
                "{context}: recover must report exactly the leftover temps"
            );
            assert_no_temps(&snapshot(&tmp.root), &context);
            assert_eq!(
                clean_recover(&tmp).temp_files_removed,
                0,
                "{context}: recover is not idempotent"
            );
            finish(&tmp, &before, &after_crash, step, &rec);
        }
        assert!(fired_any, "{label}: {step:?} was never reached");
    }
}

/// Fails if a death during `init` leaves a different number of temp files than the protocol implies (none
/// before the temp exists or after the publishing rename; one in between), if the next clean `recover` does
/// not report and remove exactly those, if an existing root changes, or if a repeated `init` does not finish
/// the store.
#[test]
fn init_that_dies_mid_write_leaves_temps_that_recover_removes() {
    crash_rows(
        "crash-init",
        &seed_partial_init,
        &|tmp, crash| init_with(crash, validator(), &tmp.root),
        &[
            row(WriteStep::Lock, 0),
            row(WriteStep::Read, 0),
            row(WriteStep::CreateTemp, 0),
            row(WriteStep::WriteTemp, 1),
            row(WriteStep::SyncTemp, 1),
            row(WriteStep::Replace, 1),
            row(WriteStep::SyncDir, 0),
        ],
        &|tmp, before, after_crash, step, _rec| {
            assert_preserved(before, after_crash, &[], &format!("{step:?}"));
            assert_eq!(dev_store::init(&tmp.root).failure, None);
            assert_eq!(valid_roots(tmp).len(), 5, "{step:?}");
        },
    );
}

/// Fails if a death during the reconciliation rewrite of `log.json` leaves other than: one temp for a
/// death at WriteTemp, SyncTemp, LinkPrevious and Replace; two (the new log and the `.prev` temp) for a
/// death between linking and renaming the previous generation; none before any temp exists or after the
/// publishing rename. Also fails if the old `log.json` changes before the publishing rename, or if the next
/// clean `recover` does not remove the temps and interrupt the run (exactly once overall).
#[test]
fn recover_that_dies_mid_rewrite_leaves_temps_that_the_next_recover_removes() {
    crash_rows(
        "crash-recover",
        &seed_recoverable,
        &|tmp, crash| recover_with(crash, validator(), &tmp.root, None),
        &[
            row(WriteStep::Lock, 0),
            row(WriteStep::Read, 0),
            row(WriteStep::CreateTemp, 0),
            row(WriteStep::WriteTemp, 1),
            row(WriteStep::SyncTemp, 1),
            row(WriteStep::LinkPrevious, 1),
            row(WriteStep::ReplacePrevious, 2),
            row(WriteStep::Replace, 1),
            row(WriteStep::SyncDir, 0),
        ],
        &|tmp, before, after_crash, step, rec| {
            let published = step == WriteStep::SyncDir;
            // Before the publishing rename the old log is intact; after it the new log is in place.
            let except: &[&str] = if published { &["log.json"] } else { &[] };
            assert_preserved(before, after_crash, except, &format!("{step:?}"));
            if let Some(prev) = after_crash.get("log.json.prev") {
                assert_eq!(
                    Some(prev),
                    before.get("log.json"),
                    "{step:?}: .prev is not a copy of the old log"
                );
            }
            // After the clean recover the log is valid with the run interrupted either way.
            let after = snapshot(&tmp.root);
            let AnyStore::Log(log) =
                decode(StoreKind::Log, &after["log.json"], &SchemaValidator::new())
                    .expect("the log is valid after recover")
            else {
                panic!("log.json decodes as a log");
            };
            assert_eq!(log.runs[0].execution, RunState::Interrupted, "{step:?}");
            assert_eq!(
                rec.runs_interrupted,
                usize::from(!published),
                "{step:?}: the run is interrupted once, by whichever recover got to it"
            );
            assert_eq!(valid_roots(tmp).len(), 5);
        },
    );
}

// ---------------------------------------------------------------------------------------------------
// 3: read faults and lock faults on the read-only commands

/// Fails if a fault at `Read` during `validate` is not `Storage` for every error kind, if the records
/// produced before the failing root are lost (the CLI prints what was done), if anything is written (the
/// whole directory, bar the lock file, is byte-identical), or if a fault index past the last read changes
/// the result. Expected: five roots exist (reads 1-5 = repositories, workflows, approvals, log, tools), so
/// a fault at read `n` keeps the records of the roots before it, `[1, 2, 3, 4, 8]` (missing roots produce a
/// record without a read), and read 6 is never made.
#[test]
fn validate_read_fault_is_a_storage_failure_that_keeps_earlier_records_and_writes_nothing() {
    let kept_before = [1usize, 2, 3, 4, 8];
    for error in ERRORS {
        for (i, expect) in kept_before.iter().enumerate() {
            let nth = u32::try_from(i).unwrap() + 1;
            let tmp = Tmp::new("validate-read");
            init_clean(&tmp);
            let before = snapshot(&tmp.root);
            let fault = FaultFs::new(FaultPlan {
                step: WriteStep::Read,
                nth,
                error,
            });
            let outcome = validate_with(&fault, &SchemaValidator::new(), &tmp.root, None);
            let context = format!("{error:?} at read {nth}");
            assert!(fault.fired(), "{context}");
            let failure = outcome.failure.expect("a read fault must fail the command");
            assert_eq!(failure, DevError::Storage, "{context}");
            assert_eq!(failure.class(), FailureClass::Storage, "{context}");
            assert_eq!(outcome.records.len(), *expect, "{context}");
            assert_eq!(snapshot(&tmp.root), before, "{context}: validate wrote");
        }
        // Read 6 does not exist: nothing fires and all nine roots are reported.
        let tmp = Tmp::new("validate-read-past");
        init_clean(&tmp);
        let fault = FaultFs::new(FaultPlan {
            step: WriteStep::Read,
            nth: 6,
            error,
        });
        let outcome = validate_with(&fault, &SchemaValidator::new(), &tmp.root, None);
        assert!(!fault.fired());
        assert_eq!(outcome.failure, None);
        assert_eq!(outcome.records.len(), 9);
    }
}

/// Fails if `show` of an existing root with a `Read` fault is not `Storage` with no record, if `show` of a
/// missing root (no read is made) is affected by it, if anything is written, or if a fault at the second
/// read (there is only one) fires.
#[test]
fn show_read_fault_is_a_storage_failure_and_writes_nothing() {
    for error in ERRORS {
        let tmp = Tmp::new("show-read");
        init_clean(&tmp);
        let before = snapshot(&tmp.root);
        let fault = FaultFs::new(FaultPlan {
            step: WriteStep::Read,
            nth: 1,
            error,
        });
        let outcome = show_with(&fault, &SchemaValidator::new(), &tmp.root, StoreKind::Log);
        assert!(fault.fired(), "{error:?}");
        assert_eq!(outcome.failure, Some(DevError::Storage), "{error:?}");
        assert!(outcome.records.is_empty());
        assert_eq!(snapshot(&tmp.root), before, "{error:?}: show wrote");

        let second = FaultFs::new(FaultPlan {
            step: WriteStep::Read,
            nth: 2,
            error,
        });
        let ok = show_with(&second, &SchemaValidator::new(), &tmp.root, StoreKind::Log);
        assert!(!second.fired());
        assert_eq!(ok.failure, None);

        // A missing root is found without reading, so a read fault cannot touch it.
        let missing = FaultFs::new(FaultPlan {
            step: WriteStep::Read,
            nth: 1,
            error,
        });
        let outcome = show_with(
            &missing,
            &SchemaValidator::new(),
            &tmp.root,
            StoreKind::Config,
        );
        assert!(!missing.fired());
        assert_eq!(outcome.failure, None);
        assert!(matches!(
            outcome.records.as_slice(),
            [DevRecord::StoreShown {
                verdict: DevVerdict::Missing,
                ..
            }]
        ));
    }
}

/// Fails if a fault while taking the lock is anything but `Storage` with no record for any command and
/// error kind, or if the failed command created or changed any file other than the lock file.
#[test]
fn lock_fault_is_a_storage_failure_for_every_command() {
    for error in ERRORS {
        let plan = FaultPlan {
            step: WriteStep::Lock,
            nth: 1,
            error,
        };
        let tmp = Tmp::new("lock-fault");
        init_clean(&tmp);
        let before = snapshot(&tmp.root);
        let outcomes = [
            validate_with(
                &FaultFs::new(plan),
                &SchemaValidator::new(),
                &tmp.root,
                None,
            ),
            show_with(
                &FaultFs::new(plan),
                &SchemaValidator::new(),
                &tmp.root,
                StoreKind::Log,
            ),
            recover_with(Arc::new(FaultFs::new(plan)), validator(), &tmp.root, None),
            init_with(Arc::new(FaultFs::new(plan)), validator(), &tmp.root),
        ];
        for (i, outcome) in outcomes.iter().enumerate() {
            assert_eq!(outcome.failure, Some(DevError::Storage), "{error:?} #{i}");
            assert!(outcome.records.is_empty(), "{error:?} #{i}");
        }
        assert_eq!(snapshot(&tmp.root), before, "{error:?}");
    }
}

// ---------------------------------------------------------------------------------------------------
// 9 (application half): same-second reports from the real writer

/// A report of `fixture("report.min.json")` with its run and report IDs replaced.
fn report_with(run: &str, report: &str) -> Box<domain::store::report::ReportFile> {
    let mut doc = fixture("report.min.json");
    doc["run_id"] = json!(run);
    doc["report_id"] = json!(report);
    let AnyStore::Report(file) = decode(
        StoreKind::Report,
        &serde_json::to_vec(&doc).unwrap(),
        &SchemaValidator::new(),
    )
    .expect("the report fixture is valid") else {
        panic!("a report decodes as a report");
    };
    file
}

/// Fails if two reports of the same repository generated in the same UTC second (one `generated_at`) do not
/// get distinct file names from the real writer, if `validate` does not find the log valid, or if
/// `recover` reports an orphan report file, a manifest without a file, a temp file or a malformed root; or
/// if it changes either report (reports are immutable). The orphan counter is shown to work by removing one
/// manifest afterwards.
#[test]
fn same_second_reports_from_the_real_writer_are_both_recognised() {
    let tmp = Tmp::new("same-second");
    let slug = Slug::parse("demo-repo").unwrap();
    let a = report_with(
        "00000000-0000-4000-8000-0000000000a1",
        "00000000-0000-4000-8000-0000000000b1",
    );
    let b = report_with(
        "00000000-0000-4000-8000-0000000000a2",
        "00000000-0000-4000-8000-0000000000b2",
    );
    assert_eq!(a.generated_at, b.generated_at, "the fixture must collide");
    let (manifest_a, manifest_b) = {
        let (mut writer, _) = StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator())
            .expect("init with the real validator");
        let ma = writer.write_report(&a, &slug).expect("first report");
        let mb = writer.write_report(&b, &slug).expect("second report");
        (ma, mb)
    };
    assert_ne!(manifest_a.json_path, manifest_b.json_path);
    let (name_a, name_b) = (
        manifest_a.json_path.as_str().to_owned(),
        manifest_b.json_path.as_str().to_owned(),
    );
    assert_eq!(
        name_a.get(..33),
        name_b.get(..33),
        "the two names must share the stamp and the slug prefix, or the test is not about one second"
    );
    let before = snapshot(&tmp.root);
    assert!(before.contains_key(&name_a) && before.contains_key(&name_b));

    let validated = dev_store::validate(&tmp.root, None);
    assert_eq!(validated.failure, None);
    let log_verdict = validated.records.iter().find_map(|r| match r {
        DevRecord::StoreValidated { kind, verdict, .. } if *kind == StoreKind::Log => {
            Some(*verdict)
        }
        _ => None,
    });
    assert_eq!(log_verdict, Some(DevVerdict::Valid));

    let recovered = dev_store::recover(&tmp.root, None);
    assert_eq!(recovered.failure, None);
    let Some(DevRecord::StoreReconciled {
        temp_files_removed,
        runs_interrupted,
        manifests_without_report,
        reports_without_manifest,
        malformed_roots,
        ..
    }) = recovered.records.first()
    else {
        panic!("recover must produce a reconciliation record");
    };
    assert_eq!(*temp_files_removed, 0);
    assert!(runs_interrupted.is_empty());
    assert!(manifests_without_report.is_empty());
    assert_eq!(*reports_without_manifest, 0);
    assert!(malformed_roots.is_empty());
    assert_eq!(snapshot(&tmp.root), before, "recover changed the store");

    // Non-vacuity: with one manifest dropped from the log, exactly that report file is an orphan.
    let mut log = fixture("log.min.json");
    log["reports"] = json!([serde_json::to_value(&manifest_a).unwrap()]);
    write_private(
        &tmp.path("log.json"),
        &serde_json::to_vec_pretty(&log).unwrap(),
    );
    let again = dev_store::recover(&tmp.root, None);
    assert_eq!(again.failure, None);
    let Some(DevRecord::StoreReconciled {
        reports_without_manifest,
        manifests_without_report,
        ..
    }) = again.records.first()
    else {
        panic!("recover must produce a reconciliation record");
    };
    assert_eq!(*reports_without_manifest, 1);
    assert!(manifests_without_report.is_empty());
}

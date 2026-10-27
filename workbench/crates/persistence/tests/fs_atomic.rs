//! P02-T07 tests: DataDir/StorePath confinement, the exclusive store lock, `write_replace` / `write_new`
//! under injected failure at every protocol step, modes, temp-file names and the bounded read.
//!
//! Every test uses its own fresh directory under `CARGO_TARGET_TMPDIR`, removed on drop. Nothing here touches
//! the network or any path outside that directory.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use domain::store::StoreKind;
use persistence::atomic::{
    DIR_MODE, FILE_MODE, ensure_parent, is_temp_file_name, write_new, write_replace,
};
use persistence::error::{StoreError, WriteStep};
use persistence::fault::{FaultFs, FaultPlan};
use persistence::fs::{FileInfo, FsOps, LockHandle, RealFs};
use persistence::lock::acquire;
use persistence::paths::{DataDir, MAX_REPORT_NAME_BYTES, StorePath};
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

const KINDS: [ErrorKind; 3] = [
    ErrorKind::Interrupted,
    ErrorKind::StorageFull,
    ErrorKind::PermissionDenied,
];
const BIG: u64 = 1 << 20;

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
            .join(format!("fs_atomic-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // The data directory must be private (P02-D03); the default umask would leave it 0755.
        set_mode(&root, 0o700);
        Self { root }
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

fn root_path(kind: StoreKind) -> StorePath {
    StorePath::root(kind).unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    RealFs
        .list_dir(dir)
        .unwrap()
        .into_iter()
        .map(|n| n.into_string().unwrap())
        .collect()
}

fn mode_of(path: &Path) -> u32 {
    std::fs::symlink_metadata(path).unwrap().mode() & 0o7777
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn plan(step: WriteStep, kind: ErrorKind) -> FaultPlan {
    FaultPlan {
        step,
        nth: 1,
        error: kind,
    }
}

fn is_hex16(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Records the arguments of `create_new` and `hard_link`, delegating everything to `RealFs`.
#[derive(Debug, Default)]
struct Recorder {
    created: Mutex<Vec<(WriteStep, PathBuf, u32)>>,
    linked: Mutex<Vec<(WriteStep, PathBuf, PathBuf)>>,
}

impl FsOps for Recorder {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        RealFs.read(path, max_bytes)
    }
    fn create_new(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        self.created
            .lock()
            .unwrap()
            .push((step, path.to_path_buf(), mode));
        RealFs.create_new(step, path, mode)
    }
    fn write_all(&self, step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()> {
        RealFs.write_all(step, path, bytes)
    }
    fn sync_file(&self, step: WriteStep, path: &Path) -> io::Result<()> {
        RealFs.sync_file(step, path)
    }
    fn hard_link(&self, step: WriteStep, existing: &Path, new: &Path) -> io::Result<()> {
        self.linked
            .lock()
            .unwrap()
            .push((step, existing.to_path_buf(), new.to_path_buf()));
        RealFs.hard_link(step, existing, new)
    }
    fn rename(&self, step: WriteStep, from: &Path, to: &Path) -> io::Result<()> {
        RealFs.rename(step, from, to)
    }
    fn sync_dir(&self, step: WriteStep, dir: &Path) -> io::Result<()> {
        RealFs.sync_dir(step, dir)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        RealFs.remove_file(path)
    }
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        RealFs.list_dir(dir)
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo> {
        RealFs.symlink_metadata(path)
    }
    fn create_dir(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        RealFs.create_dir(step, path, mode)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        RealFs.canonicalize(path)
    }
    fn random_suffix(&self) -> io::Result<[u8; 8]> {
        RealFs.random_suffix()
    }
    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle> {
        RealFs.open_lock_file(path, mode)
    }
}

// ---------------------------------------------------------------------------------------------------
// lock

#[test]
fn second_acquire_on_same_dir_is_locked() {
    let tmp = Tmp::new("lock2");
    let dir = tmp.data_dir();
    let first = acquire(&dir, &RealFs).unwrap();
    // Two handles in one process must conflict (flock is per open file description).
    assert_eq!(acquire(&dir, &RealFs).unwrap_err(), StoreError::Locked);
    // A failed attempt must not release or disturb the first lock.
    assert_eq!(acquire(&dir, &RealFs).unwrap_err(), StoreError::Locked);
    drop(first);
}

#[test]
fn lock_is_released_on_drop_and_reacquirable() {
    let tmp = Tmp::new("lockdrop");
    let dir = tmp.data_dir();
    let first = acquire(&dir, &RealFs).unwrap();
    drop(first);
    let second = acquire(&dir, &RealFs).unwrap();
    assert_eq!(acquire(&dir, &RealFs).unwrap_err(), StoreError::Locked);
    drop(second);
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn stale_unlocked_lock_file_is_reusable_and_content_is_ignored() {
    let tmp = Tmp::new("lockstale");
    std::fs::write(tmp.root.join(".app.lock"), b"pid=1 utc=1970 stale\nGARBAGE").unwrap();
    let dir = tmp.data_dir();
    let lock = acquire(&dir, &RealFs).unwrap();
    let content = std::fs::read_to_string(tmp.root.join(".app.lock")).unwrap();
    assert!(
        content.starts_with("pid="),
        "diagnostic line replaces stale content"
    );
    assert!(!content.contains("GARBAGE"));
    drop(lock);
}

#[test]
fn lock_file_is_created_with_mode_0600() {
    let tmp = Tmp::new("lockmode");
    let dir = tmp.data_dir();
    let _lock = acquire(&dir, &RealFs).unwrap();
    assert_eq!(mode_of(&tmp.root.join(".app.lock")), 0o600);
}

#[test]
fn lock_file_that_is_a_symlink_is_rejected() {
    let tmp = Tmp::new("locklink");
    let elsewhere = tmp.root.join("elsewhere");
    std::fs::write(&elsewhere, b"x").unwrap();
    symlink(&elsewhere, tmp.root.join(".app.lock")).unwrap();
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    assert_eq!(std::fs::read(&elsewhere).unwrap(), b"x");
}

#[test]
fn lock_open_failure_names_the_lock_step() {
    for kind in KINDS {
        let tmp = Tmp::new("lockfault");
        let dir = tmp.data_dir();
        let fs = FaultFs::new(plan(WriteStep::Lock, kind));
        assert_eq!(
            acquire(&dir, &fs).unwrap_err(),
            StoreError::Io {
                step: WriteStep::Lock,
                kind
            }
        );
        assert!(fs.fired());
        // Nothing was left locked.
        drop(acquire(&dir, &RealFs).unwrap());
    }
}

// ---------------------------------------------------------------------------------------------------
// write_replace: fault matrix

/// Seeds `repo.json` = "old" with `repo.json.prev` = "gen0".
fn seed(dir: &DataDir, path: &StorePath) {
    write_replace(&RealFs, dir, path, b"gen0", false).unwrap();
    write_replace(&RealFs, dir, path, b"old", true).unwrap();
}

#[test]
fn replace_fault_at_every_step_keeps_previous_file_and_names_the_step() {
    let steps = [
        WriteStep::CreateTemp,
        WriteStep::WriteTemp,
        WriteStep::SyncTemp,
        WriteStep::LinkPrevious,
        WriteStep::ReplacePrevious,
        WriteStep::Replace,
        WriteStep::SyncDir,
    ];
    let path = root_path(StoreKind::Repositories);
    for keep_previous in [true, false] {
        for step in steps {
            for kind in KINDS {
                let tmp = Tmp::new("replfault");
                let dir = tmp.data_dir();
                seed(&dir, &path);
                let target = dir.join(&path);
                let prev = dir.join(&path.previous());
                assert_eq!(std::fs::read(&prev).unwrap(), b"gen0");

                let fs = FaultFs::new(plan(step, kind));
                let res = write_replace(&fs, &dir, &path, b"new-content", keep_previous);
                let case = format!("keep_previous={keep_previous} step={step:?} kind={kind:?}");

                let step_used = keep_previous
                    || !matches!(step, WriteStep::LinkPrevious | WriteStep::ReplacePrevious);
                if !step_used {
                    // The protocol never reaches this step without keep_previous: the write succeeds.
                    assert_eq!(res, Ok(()), "{case}");
                    assert!(!fs.fired(), "{case}");
                    assert_eq!(fs.calls_at_step(), 0, "{case}");
                    assert_eq!(std::fs::read(&target).unwrap(), b"new-content", "{case}");
                    assert_eq!(std::fs::read(&prev).unwrap(), b"gen0", "{case}");
                    continue;
                }

                assert_eq!(res, Err(StoreError::Io { step, kind }), "{case}");
                assert!(fs.fired(), "{case}");

                // No temp file of any kind remains: only the target and its .prev.
                assert_eq!(names(&tmp.root), ["repo.json", "repo.json.prev"], "{case}");

                if step == WriteStep::SyncDir {
                    // The rename already happened: the new content is in place and readable.
                    assert_eq!(RealFs.read(&target, BIG).unwrap(), b"new-content", "{case}");
                } else {
                    assert_eq!(RealFs.read(&target, BIG).unwrap(), b"old", "{case}");
                    assert_eq!(mode_of(&target), 0o600, "{case}");
                }
                let rotated =
                    keep_previous && matches!(step, WriteStep::Replace | WriteStep::SyncDir);
                let expected_prev: &[u8] = if rotated { b"old" } else { b"gen0" };
                assert_eq!(RealFs.read(&prev, BIG).unwrap(), expected_prev, "{case}");
            }
        }
    }
}

#[test]
fn replace_fault_on_first_write_leaves_no_file_and_no_temp() {
    let path = root_path(StoreKind::Repositories);
    for step in [
        WriteStep::CreateTemp,
        WriteStep::WriteTemp,
        WriteStep::SyncTemp,
        WriteStep::Replace,
    ] {
        for kind in KINDS {
            let tmp = Tmp::new("replfirst");
            let dir = tmp.data_dir();
            let fs = FaultFs::new(plan(step, kind));
            let res = write_replace(&fs, &dir, &path, b"new", true);
            assert_eq!(res, Err(StoreError::Io { step, kind }), "{step:?} {kind:?}");
            assert!(names(&tmp.root).is_empty(), "{step:?} {kind:?}");
        }
    }
}

#[test]
fn replace_fault_in_sync_dir_on_first_write_leaves_the_new_file_only() {
    let path = root_path(StoreKind::Repositories);
    let tmp = Tmp::new("replfirstsync");
    let dir = tmp.data_dir();
    let fs = FaultFs::new(plan(WriteStep::SyncDir, ErrorKind::StorageFull));
    let res = write_replace(&fs, &dir, &path, b"new", true);
    assert_eq!(
        res,
        Err(StoreError::Io {
            step: WriteStep::SyncDir,
            kind: ErrorKind::StorageFull
        })
    );
    assert_eq!(names(&tmp.root), ["repo.json"]);
    assert_eq!(std::fs::read(tmp.root.join("repo.json")).unwrap(), b"new");
}

#[test]
fn replace_create_dir_fault_creates_nothing() {
    let path = root_path(StoreKind::RagIndex); // rag/index.json
    for kind in KINDS {
        let tmp = Tmp::new("replmkdir");
        let dir = tmp.data_dir();
        let fs = FaultFs::new(plan(WriteStep::CreateDir, kind));
        let res = write_replace(&fs, &dir, &path, b"x", true);
        assert_eq!(
            res,
            Err(StoreError::Io {
                step: WriteStep::CreateDir,
                kind
            })
        );
        assert!(names(&tmp.root).is_empty());
    }
}

#[test]
fn replace_fault_in_the_directory_sync_after_mkdir_leaves_no_temp() {
    let path = root_path(StoreKind::RagIndex);
    let tmp = Tmp::new("replmkdirsync");
    let dir = tmp.data_dir();
    // Second CreateDir-step call is the sync of the data dir after creating rag/.
    let fs = FaultFs::new(FaultPlan {
        step: WriteStep::CreateDir,
        nth: 2,
        error: ErrorKind::StorageFull,
    });
    let res = write_replace(&fs, &dir, &path, b"x", true);
    assert_eq!(
        res,
        Err(StoreError::Io {
            step: WriteStep::CreateDir,
            kind: ErrorKind::StorageFull
        })
    );
    assert_eq!(names(&tmp.root), ["rag"]);
    assert!(names(&tmp.root.join("rag")).is_empty());
}

#[test]
fn fault_on_a_later_call_does_not_fire_early() {
    let path = root_path(StoreKind::Repositories);
    let tmp = Tmp::new("nth");
    let dir = tmp.data_dir();
    // One write_replace makes exactly one Replace call, so nth = 2 never fires.
    let fs = FaultFs::new(FaultPlan {
        step: WriteStep::Replace,
        nth: 2,
        error: ErrorKind::Interrupted,
    });
    write_replace(&fs, &dir, &path, b"a", true).unwrap();
    assert!(!fs.fired());
    assert_eq!(fs.calls_at_step(), 1);
    // The second write makes the second Replace call and fails there; the first content survives.
    let res = write_replace(&fs, &dir, &path, b"b", true);
    assert_eq!(
        res,
        Err(StoreError::Io {
            step: WriteStep::Replace,
            kind: ErrorKind::Interrupted
        })
    );
    assert_eq!(std::fs::read(tmp.root.join("repo.json")).unwrap(), b"a");
    // The .prev rotation (step 4) completed before the failing Replace; no temp remains.
    assert_eq!(names(&tmp.root), ["repo.json", "repo.json.prev"]);
    assert_eq!(
        std::fs::read(tmp.root.join("repo.json.prev")).unwrap(),
        b"a"
    );
}

#[test]
fn passthrough_fault_fs_behaves_like_real_fs() {
    let path = root_path(StoreKind::Repositories);
    let tmp = Tmp::new("passthrough");
    let dir = tmp.data_dir();
    let fs = FaultFs::passthrough();
    write_replace(&fs, &dir, &path, b"a", true).unwrap();
    write_replace(&fs, &dir, &path, b"b", true).unwrap();
    assert!(!fs.fired());
    assert_eq!(std::fs::read(tmp.root.join("repo.json")).unwrap(), b"b");
    assert_eq!(
        std::fs::read(tmp.root.join("repo.json.prev")).unwrap(),
        b"a"
    );
}

// ---------------------------------------------------------------------------------------------------
// write_replace: generations, names, modes

#[test]
fn keep_previous_holds_the_previous_generation() {
    let path = root_path(StoreKind::Workflows);
    let tmp = Tmp::new("prev");
    let dir = tmp.data_dir();
    write_replace(&RealFs, &dir, &path, b"A", true).unwrap();
    assert_eq!(
        names(&tmp.root),
        ["workflows.json"],
        "no .prev without a previous file"
    );
    write_replace(&RealFs, &dir, &path, b"B", true).unwrap();
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json")).unwrap(),
        b"B"
    );
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json.prev")).unwrap(),
        b"A"
    );
    write_replace(&RealFs, &dir, &path, b"C", true).unwrap();
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json")).unwrap(),
        b"C"
    );
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json.prev")).unwrap(),
        b"B"
    );
    assert_eq!(names(&tmp.root), ["workflows.json", "workflows.json.prev"]);
    // Without keep_previous the .prev generation is left alone.
    write_replace(&RealFs, &dir, &path, b"D", false).unwrap();
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json")).unwrap(),
        b"D"
    );
    assert_eq!(
        std::fs::read(tmp.root.join("workflows.json.prev")).unwrap(),
        b"B"
    );
}

#[test]
fn replace_shorter_content_does_not_leave_old_tail() {
    let path = root_path(StoreKind::Approvals);
    let tmp = Tmp::new("shorter");
    let dir = tmp.data_dir();
    write_replace(&RealFs, &dir, &path, b"0123456789", false).unwrap();
    write_replace(&RealFs, &dir, &path, b"abc", false).unwrap();
    assert_eq!(
        std::fs::read(tmp.root.join("approvals.json")).unwrap(),
        b"abc"
    );
}

#[test]
fn temp_names_follow_the_documented_pattern_and_are_random() {
    let path = root_path(StoreKind::Repositories);
    let tmp = Tmp::new("tempname");
    let dir = tmp.data_dir();
    let rec = Recorder::default();
    write_replace(&rec, &dir, &path, b"one", true).unwrap();
    write_replace(&rec, &dir, &path, b"two", true).unwrap();

    let created = rec.created.lock().unwrap();
    assert_eq!(created.len(), 2);
    let mut seen = Vec::new();
    for (step, p, mode) in created.iter() {
        assert_eq!(*step, WriteStep::CreateTemp);
        assert_eq!(*mode, FILE_MODE);
        assert_eq!(
            p.parent().unwrap(),
            dir.as_path(),
            "temp in the same directory"
        );
        let name = p.file_name().unwrap().to_str().unwrap();
        let hex = name.strip_prefix(".repo.json.tmp-").expect(name);
        assert!(is_hex16(hex), "{name}");
        assert!(is_temp_file_name(name));
        seen.push(name.to_owned());
    }
    assert_ne!(seen[0], seen[1], "names are random");

    // Only the second write had an existing target to link.
    let linked = rec.linked.lock().unwrap();
    assert_eq!(linked.len(), 1);
    let (step, existing, new) = &linked[0];
    assert_eq!(*step, WriteStep::LinkPrevious);
    assert_eq!(existing, &dir.join(&path));
    assert_eq!(new.parent().unwrap(), dir.as_path());
    let name = new.file_name().unwrap().to_str().unwrap();
    assert!(
        is_hex16(name.strip_prefix(".repo.json.prev.tmp-").expect(name)),
        "{name}"
    );
    assert!(is_temp_file_name(name));
    assert_eq!(
        names(&tmp.root),
        ["repo.json", "repo.json.prev"],
        "no temp after success"
    );
}

#[test]
fn nested_root_temp_is_created_next_to_the_target() {
    let path = root_path(StoreKind::Conversations); // rag/conversations.json
    let tmp = Tmp::new("nested");
    let dir = tmp.data_dir();
    let rec = Recorder::default();
    write_replace(&rec, &dir, &path, b"x", true).unwrap();
    let created = rec.created.lock().unwrap();
    assert_eq!(
        created[0].1.parent().unwrap(),
        tmp.root.canonicalize().unwrap().join("rag")
    );
    assert_eq!(names(&tmp.root), ["rag"]);
    assert_eq!(names(&tmp.root.join("rag")), ["conversations.json"]);
}

#[test]
fn temp_name_recogniser_is_exact() {
    assert!(is_temp_file_name(".repo.json.tmp-0123456789abcdef"));
    assert!(is_temp_file_name(".repo.json.prev.tmp-0123456789abcdef"));
    assert!(
        !is_temp_file_name("repo.json.tmp-0123456789abcdef"),
        "needs the leading dot"
    );
    assert!(
        !is_temp_file_name(".repo.json.tmp-0123456789ABCDEF"),
        "lowercase only"
    );
    assert!(
        !is_temp_file_name(".repo.json.tmp-0123456789abcde"),
        "15 digits"
    );
    assert!(
        !is_temp_file_name(".repo.json.tmp-0123456789abcdef0"),
        "17 digits"
    );
    assert!(
        !is_temp_file_name(".repo.json.tmp-0123456789abcdeg"),
        "non-hex"
    );
    assert!(!is_temp_file_name(".tmp-0123456789abcdef"), "empty stem");
    assert!(!is_temp_file_name(".app.lock"));
    assert!(!is_temp_file_name("repo.json.prev"));
}

#[test]
fn created_files_are_0600_and_created_dirs_are_0700() {
    let tmp = Tmp::new("modes");
    let dir = tmp.data_dir();
    let root = root_path(StoreKind::RagIndex);
    write_replace(&RealFs, &dir, &root, b"x", true).unwrap();
    write_replace(&RealFs, &dir, &root, b"y", true).unwrap();
    write_new(&RealFs, &dir, &StorePath::report("r.json").unwrap(), b"z").unwrap();
    assert_eq!(mode_of(&tmp.root.join("rag")), DIR_MODE);
    assert_eq!(mode_of(&tmp.root.join("backup")), DIR_MODE);
    assert_eq!(mode_of(&tmp.root.join("rag/index.json")), FILE_MODE);
    assert_eq!(mode_of(&tmp.root.join("rag/index.json.prev")), FILE_MODE);
    assert_eq!(mode_of(&tmp.root.join("backup/r.json")), FILE_MODE);
    assert_eq!((DIR_MODE, FILE_MODE), (0o700, 0o600));
}

#[test]
fn replace_refuses_a_symlink_target_and_leaves_it_alone() {
    let path = root_path(StoreKind::Repositories);
    let tmp = Tmp::new("targetlink");
    let outside = tmp.root.join("outside.txt");
    std::fs::write(&outside, b"outside").unwrap();
    symlink(&outside, tmp.root.join("repo.json")).unwrap();
    let dir = tmp.data_dir();
    assert_eq!(
        write_replace(&RealFs, &dir, &path, b"new", true).unwrap_err(),
        StoreError::PathRejected
    );
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    assert_eq!(
        names(&tmp.root),
        ["outside.txt", "repo.json"],
        "no temp left"
    );
}

#[test]
fn ensure_parent_rejects_a_file_or_symlink_in_place_of_the_directory() {
    let tmp = Tmp::new("parentbad");
    let dir = tmp.data_dir();
    let report = StorePath::report("r.json").unwrap();

    std::fs::write(tmp.root.join("backup"), b"i am a file").unwrap();
    assert_eq!(
        ensure_parent(&RealFs, &dir, &report).unwrap_err(),
        StoreError::PathRejected
    );
    assert_eq!(
        write_new(&RealFs, &dir, &report, b"x").unwrap_err(),
        StoreError::PathRejected
    );
    std::fs::remove_file(tmp.root.join("backup")).unwrap();

    let elsewhere = tmp.root.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    symlink(&elsewhere, tmp.root.join("backup")).unwrap();
    assert_eq!(
        write_new(&RealFs, &dir, &report, b"x").unwrap_err(),
        StoreError::PathRejected
    );
    assert!(
        names(&elsewhere).is_empty(),
        "nothing written through the symlink"
    );

    // A root file has no parent directory to create.
    ensure_parent(&RealFs, &dir, &root_path(StoreKind::Repositories)).unwrap();
}

// ---------------------------------------------------------------------------------------------------
// write_new

#[test]
fn write_new_never_overwrites() {
    let tmp = Tmp::new("new");
    let dir = tmp.data_dir();
    let path = StorePath::report("20260101T000000Z_slug_run.json").unwrap();
    write_new(&RealFs, &dir, &path, b"original").unwrap();
    assert_eq!(
        write_new(&RealFs, &dir, &path, b"replacement").unwrap_err(),
        StoreError::ReportNameCollision
    );
    assert_eq!(
        std::fs::read(tmp.root.join("backup/20260101T000000Z_slug_run.json")).unwrap(),
        b"original"
    );
    assert_eq!(
        names(&tmp.root.join("backup")),
        ["20260101T000000Z_slug_run.json"],
        "no temp left"
    );
}

#[test]
fn write_new_does_not_overwrite_a_pre_existing_foreign_file() {
    let tmp = Tmp::new("newforeign");
    std::fs::create_dir(tmp.root.join("backup")).unwrap();
    std::fs::write(tmp.root.join("backup/r.json"), b"foreign").unwrap();
    let dir = tmp.data_dir();
    let path = StorePath::report("r.json").unwrap();
    assert_eq!(
        write_new(&RealFs, &dir, &path, b"mine").unwrap_err(),
        StoreError::ReportNameCollision
    );
    assert_eq!(
        std::fs::read(tmp.root.join("backup/r.json")).unwrap(),
        b"foreign"
    );
    assert_eq!(names(&tmp.root.join("backup")), ["r.json"]);
}

#[test]
fn write_new_uses_a_same_directory_temp_then_links_it() {
    let tmp = Tmp::new("newtemp");
    let dir = tmp.data_dir();
    let rec = Recorder::default();
    write_new(&rec, &dir, &StorePath::report("r.json").unwrap(), b"z").unwrap();
    let created = rec.created.lock().unwrap();
    let name = created[0].1.file_name().unwrap().to_str().unwrap();
    assert!(is_hex16(name.strip_prefix(".r.json.tmp-").expect(name)));
    assert_eq!(created[0].1.parent().unwrap(), dir.as_path().join("backup"));
    let linked = rec.linked.lock().unwrap();
    assert_eq!(linked[0].0, WriteStep::Replace);
    assert_eq!(linked[0].1, created[0].1);
    assert_eq!(linked[0].2, dir.as_path().join("backup/r.json"));
    assert_eq!(names(&tmp.root.join("backup")), ["r.json"]);
}

#[test]
fn write_new_faults_leave_no_partial_report_and_no_temp() {
    let path = StorePath::report("r.json").unwrap();
    let steps = [
        WriteStep::CreateDir,
        WriteStep::CreateTemp,
        WriteStep::WriteTemp,
        WriteStep::SyncTemp,
        WriteStep::Replace,
        WriteStep::SyncDir,
    ];
    for step in steps {
        for kind in KINDS {
            let tmp = Tmp::new("newfault");
            let dir = tmp.data_dir();
            let fs = FaultFs::new(plan(step, kind));
            let res = write_new(&fs, &dir, &path, b"report-bytes");
            let case = format!("{step:?} {kind:?}");
            assert_eq!(res, Err(StoreError::Io { step, kind }), "{case}");
            assert!(fs.fired(), "{case}");
            match step {
                WriteStep::CreateDir => assert!(names(&tmp.root).is_empty(), "{case}"),
                WriteStep::SyncDir => {
                    // Published before the directory sync: complete content, no temp.
                    assert_eq!(names(&tmp.root.join("backup")), ["r.json"], "{case}");
                    assert_eq!(
                        std::fs::read(tmp.root.join("backup/r.json")).unwrap(),
                        b"report-bytes"
                    );
                }
                _ => assert!(names(&tmp.root.join("backup")).is_empty(), "{case}"),
            }
        }
    }
}

#[test]
fn write_new_fault_does_not_disturb_an_existing_report() {
    let path = StorePath::report("r.json").unwrap();
    for step in [
        WriteStep::CreateTemp,
        WriteStep::WriteTemp,
        WriteStep::SyncTemp,
        WriteStep::Replace,
    ] {
        let tmp = Tmp::new("newfaultexisting");
        let dir = tmp.data_dir();
        write_new(&RealFs, &dir, &path, b"first").unwrap();
        let fs = FaultFs::new(plan(step, ErrorKind::StorageFull));
        assert!(write_new(&fs, &dir, &path, b"second").is_err());
        assert_eq!(
            std::fs::read(tmp.root.join("backup/r.json")).unwrap(),
            b"first"
        );
        assert_eq!(names(&tmp.root.join("backup")), ["r.json"], "{step:?}");
    }
}

// ---------------------------------------------------------------------------------------------------
// DataDir and StorePath

#[test]
fn data_dir_accepts_a_real_directory_in_canonical_form() {
    let tmp = Tmp::new("dirok");
    let dir = DataDir::new(&RealFs, &tmp.root).unwrap();
    assert_eq!(dir.as_path(), std::fs::canonicalize(&tmp.root).unwrap());
    // A trailing slash and a `.` segment normalise to the same directory.
    let with_slash = PathBuf::from(format!("{}/", tmp.root.display()));
    assert_eq!(DataDir::new(&RealFs, &with_slash).unwrap(), dir);
    assert_eq!(DataDir::new(&RealFs, &tmp.root.join(".")).unwrap(), dir);
}

#[test]
fn data_dir_rejects_relative_missing_file_and_symlinked_paths() {
    let tmp = Tmp::new("dirbad");
    let reject = |p: &Path| {
        assert_eq!(
            DataDir::new(&RealFs, p).unwrap_err(),
            StoreError::PathRejected,
            "{p:?}"
        )
    };

    reject(Path::new("relative/dir"));
    reject(Path::new("."));
    reject(Path::new(""));
    reject(&tmp.root.join("missing"));

    let file = tmp.root.join("file");
    std::fs::write(&file, b"x").unwrap();
    reject(&file);

    let real = tmp.root.join("real");
    std::fs::create_dir(&real).unwrap();
    set_mode(&real, 0o700);
    let link = tmp.root.join("link");
    symlink(&real, &link).unwrap();
    reject(&link);
    reject(&PathBuf::from(format!("{}/", link.display())));

    let dangling = tmp.root.join("dangling");
    symlink(tmp.root.join("nowhere"), &dangling).unwrap();
    reject(&dangling);

    // The real directory itself is fine.
    DataDir::new(&RealFs, &real).unwrap();
}

#[test]
fn store_path_root_maps_every_kind_to_its_file_name() {
    assert!(StorePath::root(StoreKind::Report).is_none());
    for kind in StoreKind::ALL {
        let expected = kind.file_name();
        assert_eq!(
            StorePath::root(kind).map(|p| p.as_str().to_owned()),
            expected.map(str::to_owned)
        );
    }
    let rag = root_path(StoreKind::RagIndex);
    assert_eq!((rag.parent(), rag.file_name()), (Some("rag"), "index.json"));
    let cfg = root_path(StoreKind::Config);
    assert_eq!((cfg.parent(), cfg.file_name()), (None, "config.json"));
    assert_eq!(cfg.previous().as_str(), "config.json.prev");
}

#[test]
fn store_path_report_rejects_hostile_names() {
    let long_ok = "a".repeat(MAX_REPORT_NAME_BYTES);
    let hostile = [
        "".to_owned(),
        "../x".to_owned(),
        "..".to_owned(),
        ".".to_owned(),
        "a/b".to_owned(),
        "/abs".to_owned(),
        "a\\b".to_owned(),
        ".hidden".to_owned(),
        ".tmp-0123456789abcdef".to_owned(),
        "a\0b".to_owned(),
        "a b".to_owned(),
        "a\nb".to_owned(),
        "a:b".to_owned(),
        "\u{e9}.json".to_owned(),
        "\u{202e}gpj.exe".to_owned(),
        "\u{ff0e}\u{ff0e}".to_owned(),
        "a".repeat(256),
        "a".repeat(MAX_REPORT_NAME_BYTES + 1),
    ];
    for name in &hostile {
        assert_eq!(
            StorePath::report(name).unwrap_err(),
            StoreError::PathRejected,
            "{name:?}"
        );
    }
    assert!(StorePath::report(&long_ok).is_ok());
    assert!(
        StorePath::report("20260101T000000Z_my-repo_01234567-89ab-4def-8123-456789abcdef.json")
            .is_ok()
    );
    assert!(StorePath::report("a.b_c-d").is_ok());
}

#[test]
fn every_accepted_or_generated_path_stays_inside_the_data_dir() {
    let tmp = Tmp::new("confine");
    let dir = tmp.data_dir();
    let mut paths: Vec<StorePath> = StoreKind::ALL
        .iter()
        .filter_map(|k| StorePath::root(*k))
        .collect();
    for name in [
        "x.json", "..x", "a..b", "-rf", "~", "a.prev", "CON", "x.json ",
    ] {
        if let Ok(p) = StorePath::report(name) {
            paths.push(p);
        }
    }
    assert!(paths.len() > 9);
    for p in &paths {
        let full = dir.join(p);
        assert!(full.starts_with(dir.as_path()), "{full:?}");
        assert!(
            full.components().all(|c| !matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )),
            "{full:?}"
        );
        let parent = dir.parent_of(p);
        assert!(
            parent == dir.as_path()
                || parent == dir.as_path().join("rag")
                || parent == dir.as_path().join("backup")
        );
        assert_eq!(full.parent().unwrap(), parent);
    }
}

// ---------------------------------------------------------------------------------------------------
// bounded read

#[test]
fn read_rejects_a_file_larger_than_max_bytes() {
    let tmp = Tmp::new("read");
    let file = tmp.root.join("f");
    std::fs::write(&file, b"0123456789").unwrap();
    assert_eq!(RealFs.read(&file, 10).unwrap(), b"0123456789");
    assert_eq!(RealFs.read(&file, 11).unwrap(), b"0123456789");
    assert_eq!(
        RealFs.read(&file, 9).unwrap_err().kind(),
        ErrorKind::FileTooLarge
    );
    assert_eq!(
        RealFs.read(&file, 0).unwrap_err().kind(),
        ErrorKind::FileTooLarge
    );
    std::fs::write(&file, b"").unwrap();
    assert_eq!(RealFs.read(&file, 0).unwrap(), b"");
    assert_eq!(
        RealFs
            .read(&tmp.root.join("absent"), 10)
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn read_step_can_be_fault_injected() {
    let tmp = Tmp::new("readfault");
    let file = tmp.root.join("f");
    std::fs::write(&file, b"abc").unwrap();
    let fs = FaultFs::new(plan(WriteStep::Read, ErrorKind::PermissionDenied));
    assert_eq!(
        fs.read(&file, 10).unwrap_err().kind(),
        ErrorKind::PermissionDenied
    );
    assert_eq!(
        fs.read(&file, 10).unwrap(),
        b"abc",
        "only the nth call fails"
    );
}

#[test]
fn random_suffix_is_not_constant() {
    let a = RealFs.random_suffix().unwrap();
    let b = RealFs.random_suffix().unwrap();
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------------------------------
// P02-D03: data-directory ownership/mode and open hardening

/// Delegates to `RealFs`, but reports a uid that differs from the real one for exactly one path.
#[derive(Debug)]
struct UidSpoof {
    target: PathBuf,
}

impl FsOps for UidSpoof {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        RealFs.read(path, max_bytes)
    }
    fn create_new(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        RealFs.create_new(step, path, mode)
    }
    fn write_all(&self, step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()> {
        RealFs.write_all(step, path, bytes)
    }
    fn sync_file(&self, step: WriteStep, path: &Path) -> io::Result<()> {
        RealFs.sync_file(step, path)
    }
    fn hard_link(&self, step: WriteStep, existing: &Path, new: &Path) -> io::Result<()> {
        RealFs.hard_link(step, existing, new)
    }
    fn rename(&self, step: WriteStep, from: &Path, to: &Path) -> io::Result<()> {
        RealFs.rename(step, from, to)
    }
    fn sync_dir(&self, step: WriteStep, dir: &Path) -> io::Result<()> {
        RealFs.sync_dir(step, dir)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        RealFs.remove_file(path)
    }
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        RealFs.list_dir(dir)
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo> {
        let mut info = RealFs.symlink_metadata(path)?;
        if path == self.target {
            info.uid = info.uid.wrapping_add(1);
        }
        Ok(info)
    }
    fn create_dir(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        RealFs.create_dir(step, path, mode)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        RealFs.canonicalize(path)
    }
    fn random_suffix(&self) -> io::Result<[u8; 8]> {
        RealFs.random_suffix()
    }
    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle> {
        RealFs.open_lock_file(path, mode)
    }
}

#[test]
fn data_dir_new_requires_no_group_or_other_bits() {
    let tmp = Tmp::new("dirmode");
    for mode in [0o755, 0o750, 0o701, 0o770, 0o707] {
        set_mode(&tmp.root, mode);
        assert_eq!(
            DataDir::new(&RealFs, &tmp.root).unwrap_err(),
            StoreError::PathRejected,
            "mode {mode:o}"
        );
    }
    set_mode(&tmp.root, 0o700);
    DataDir::new(&RealFs, &tmp.root).unwrap();
}

#[test]
fn acquire_rejects_a_non_private_rag_dir_and_does_not_leak_the_lock() {
    let tmp = Tmp::new("ragmode");
    let rag = tmp.root.join("rag");
    std::fs::create_dir(&rag).unwrap();
    set_mode(&rag, 0o755);
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    set_mode(&rag, 0o700);
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn acquire_rejects_a_non_private_backup_dir_and_does_not_leak_the_lock() {
    let tmp = Tmp::new("backupmode");
    let backup = tmp.root.join("backup");
    std::fs::create_dir(&backup).unwrap();
    set_mode(&backup, 0o750);
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    set_mode(&backup, 0o700);
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn acquire_rejects_a_symlinked_backup_dir_and_does_not_leak_the_lock() {
    let tmp = Tmp::new("backuplink");
    let elsewhere = tmp.root.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    set_mode(&elsewhere, 0o700);
    let backup = tmp.root.join("backup");
    symlink(&elsewhere, &backup).unwrap();
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    std::fs::remove_file(&backup).unwrap();
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn acquire_rejects_a_regular_file_in_place_of_backup() {
    let tmp = Tmp::new("backupfile");
    let backup = tmp.root.join("backup");
    std::fs::write(&backup, b"x").unwrap();
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    std::fs::remove_file(&backup).unwrap();
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn acquire_accepts_existing_private_rag_and_backup_dirs() {
    let tmp = Tmp::new("subok");
    for name in ["rag", "backup"] {
        let sub = tmp.root.join(name);
        std::fs::create_dir(&sub).unwrap();
        set_mode(&sub, 0o700);
    }
    let dir = tmp.data_dir();
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn acquire_rejects_a_hard_linked_lock_file() {
    let tmp = Tmp::new("lockhard");
    let lock = tmp.root.join(".app.lock");
    std::fs::write(&lock, b"").unwrap();
    std::fs::hard_link(&lock, tmp.root.join("other-name")).unwrap();
    let dir = tmp.data_dir();
    assert_eq!(
        acquire(&dir, &RealFs).unwrap_err(),
        StoreError::PathRejected
    );
    std::fs::remove_file(tmp.root.join("other-name")).unwrap();
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn open_lock_file_refuses_a_symlink_on_its_own() {
    // The handle-level O_NOFOLLOW refuses it (the path pre-check in `acquire` is not relied on here).
    let tmp = Tmp::new("locknofollow");
    let elsewhere = tmp.root.join("elsewhere");
    std::fs::write(&elsewhere, b"x").unwrap();
    let lock = tmp.root.join(".app.lock");
    symlink(&elsewhere, &lock).unwrap();
    assert!(RealFs.open_lock_file(&lock, 0o600).is_err());
    assert_eq!(std::fs::read(&elsewhere).unwrap(), b"x");
}

#[test]
fn acquire_rejects_a_uid_mismatch_on_the_data_dir_or_on_rag() {
    let tmp = Tmp::new("uid");
    let rag = tmp.root.join("rag");
    std::fs::create_dir(&rag).unwrap();
    set_mode(&rag, 0o700);
    let dir = tmp.data_dir();
    let spoof_dir = UidSpoof {
        target: dir.as_path().to_path_buf(),
    };
    assert_eq!(
        acquire(&dir, &spoof_dir).unwrap_err(),
        StoreError::PathRejected
    );
    let spoof_rag = UidSpoof {
        target: dir.as_path().join("rag"),
    };
    assert_eq!(
        acquire(&dir, &spoof_rag).unwrap_err(),
        StoreError::PathRejected
    );
    // Neither rejection left the lock held.
    drop(acquire(&dir, &RealFs).unwrap());
}

#[test]
fn owner_uid_equals_the_data_dir_uid() {
    let tmp = Tmp::new("owner");
    let dir = tmp.data_dir();
    let lock = acquire(&dir, &RealFs).unwrap();
    assert_eq!(
        lock.owner_uid(),
        std::fs::metadata(dir.as_path()).unwrap().uid()
    );
}

#[test]
fn read_refuses_a_symlink_and_returns_no_content() {
    let tmp = Tmp::new("readlink");
    let target = tmp.root.join("target");
    std::fs::write(&target, b"synthetic content").unwrap();
    let link = tmp.root.join("link");
    symlink(&target, &link).unwrap();
    assert!(RealFs.read(&link, BIG).is_err());
    assert_eq!(RealFs.read(&target, BIG).unwrap(), b"synthetic content");
}

#[test]
fn write_all_through_a_symlink_fails_and_leaves_the_target_alone() {
    let tmp = Tmp::new("writelink");
    let target = tmp.root.join("target");
    std::fs::write(&target, b"original").unwrap();
    let link = tmp.root.join("link");
    symlink(&target, &link).unwrap();
    assert!(
        RealFs
            .write_all(WriteStep::WriteTemp, &link, b"clobber")
            .is_err()
    );
    assert!(RealFs.sync_file(WriteStep::SyncTemp, &link).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
}

#[test]
fn read_of_a_fifo_fails_promptly_instead_of_blocking() {
    let tmp = Tmp::new("fifo");
    let fifo = tmp.root.join("fifo");
    let status = std::process::Command::new("/usr/bin/mkfifo")
        .arg(&fifo)
        .status()
        .expect("/usr/bin/mkfifo must be present to run this test");
    assert!(status.success(), "mkfifo failed");
    let (tx, rx) = std::sync::mpsc::channel();
    let path = fifo.clone();
    std::thread::spawn(move || {
        let _ = tx.send(RealFs.read(&path, BIG));
    });
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("read of a FIFO must not block");
    assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
    // The other file operations refuse it too (write: no reader, or non-regular).
    assert!(RealFs.write_all(WriteStep::WriteTemp, &fifo, b"x").is_err());
    assert!(RealFs.sync_file(WriteStep::SyncTemp, &fifo).is_err());
}

#[test]
fn a_file_with_two_hard_links_is_still_readable() {
    let tmp = Tmp::new("hardread");
    let file = tmp.root.join("f");
    std::fs::write(&file, b"two links").unwrap();
    std::fs::hard_link(&file, tmp.root.join("g")).unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().nlink(), 2);
    assert_eq!(RealFs.read(&file, BIG).unwrap(), b"two links");
    RealFs
        .write_all(WriteStep::WriteTemp, &file, b"TWO LINKS")
        .unwrap();
    RealFs.sync_file(WriteStep::SyncTemp, &file).unwrap();
}

#[test]
fn sync_dir_refuses_a_symlink_and_a_regular_file() {
    let tmp = Tmp::new("syncdir");
    let real = tmp.root.join("real");
    std::fs::create_dir(&real).unwrap();
    RealFs.sync_dir(WriteStep::SyncDir, &real).unwrap();
    let link = tmp.root.join("link");
    symlink(&real, &link).unwrap();
    assert!(RealFs.sync_dir(WriteStep::SyncDir, &link).is_err());
    let file = tmp.root.join("file");
    std::fs::write(&file, b"x").unwrap();
    assert!(RealFs.sync_dir(WriteStep::SyncDir, &file).is_err());
}

#[test]
fn symlink_metadata_reports_uid_and_nlink() {
    let tmp = Tmp::new("info");
    let file = tmp.root.join("f");
    std::fs::write(&file, b"x").unwrap();
    std::fs::hard_link(&file, tmp.root.join("g")).unwrap();
    let info = RealFs.symlink_metadata(&file).unwrap();
    let meta = std::fs::metadata(&file).unwrap();
    assert_eq!(info.uid, meta.uid());
    assert_eq!(info.nlink, 2);
}

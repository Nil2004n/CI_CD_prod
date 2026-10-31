//! P03-T09 tests: FR-33, refuse to run as root against a data directory that another user owns
//! (`planning/P03-T01-request.md` §5.6, §8 T09, lead decision T01-D3).
//!
//! Three groups: the pure `parse_status_euid` table, the real `/proc/self/status` reader, and `DataDir::new`
//! driven through an `FsOps` double that wraps the real file system and overrides the effective uid and the
//! reported owner of chosen paths. Expected values are written out literally. Each group says what would
//! make it fail. All data is synthetic; nothing touches the network or any path outside the test
//! directories.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use persistence::error::{StoreError, WriteStep};
use persistence::fs::{
    FileInfo, FsOps, LockHandle, RealFs, parse_status_euid, process_effective_uid,
};
use persistence::lock::acquire;
use persistence::paths::DataDir;
use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------------------------------
// parse_status_euid

/// A status text shaped like a real kernel one (tab separated, `Uid:` among many other lines).
fn status_with(uid_line: &str) -> Vec<u8> {
    format!(
        "Name:\tcargo\nUmask:\t0022\nState:\tR (running)\nTgid:\t4242\nNgid:\t0\nPid:\t4242\nPPid:\t4000\n\
         TracerPid:\t0\n{uid_line}\nGid:\t1000\t1000\t1000\t1000\nFDSize:\t256\nGroups:\t24 27 1000\n\
         NStgid:\t4242\nVmPeak:\t  123456 kB\nThreads:\t4\n"
    )
    .into_bytes()
}

#[test]
fn parse_status_euid_table() {
    let mut non_utf8_other_lines = b"Name:\t\xff\xfe\xc3\x28\nUmask:\t0022\n".to_vec();
    non_utf8_other_lines.extend_from_slice(b"Uid:\t1000\t1001\t1002\t1003\n");
    non_utf8_other_lines.extend_from_slice(b"Groups:\t\x80\x81\n");
    let mut no_trailing_newline = b"Name:\tx\n".to_vec();
    no_trailing_newline.extend_from_slice(b"Uid:\t5\t6\t7\t8");
    let max = u32::MAX.to_string();
    let over = (u64::from(u32::MAX) + 1).to_string();

    let cases: Vec<(&str, Vec<u8>, Option<u32>)> = vec![
        (
            "real shaped, all equal",
            status_with("Uid:\t1000\t1000\t1000\t1000"),
            Some(1000),
        ),
        (
            "real shaped, root",
            status_with("Uid:\t0\t0\t0\t0"),
            Some(0),
        ),
        (
            "euid differs from ruid (second field wins)",
            status_with("Uid:\t1000\t0\t1000\t1000"),
            Some(0),
        ),
        (
            "euid differs, other direction",
            status_with("Uid:\t0\t1000\t0\t0"),
            Some(1000),
        ),
        (
            "spaces instead of tabs",
            status_with("Uid: 7 8 9 10"),
            Some(8),
        ),
        (
            "mixed tabs and runs of spaces",
            status_with("Uid:\t 7 \t  8\t9\t10"),
            Some(8),
        ),
        ("exactly two fields", status_with("Uid:\t11\t12"), Some(12)),
        (
            "u32::MAX is accepted",
            status_with(&format!("Uid:\t1\t{max}\t1\t1")),
            Some(u32::MAX),
        ),
        (
            "u32::MAX + 1 overflows",
            status_with(&format!("Uid:\t1\t{over}\t1\t1")),
            None,
        ),
        (
            "overflow in a field other than the second",
            status_with(&format!("Uid:\t{over}\t1\t1\t1")),
            None,
        ),
        (
            "no Uid line",
            b"Name:\tcargo\nGid:\t0\t0\t0\t0\n".to_vec(),
            None,
        ),
        (
            "two Uid lines",
            [
                status_with("Uid:\t1\t2\t3\t4"),
                b"Uid:\t1\t2\t3\t4\n".to_vec(),
            ]
            .concat(),
            None,
        ),
        (
            "two Uid lines that disagree",
            [
                status_with("Uid:\t1\t2\t3\t4"),
                b"Uid:\t1\t0\t3\t4\n".to_vec(),
            ]
            .concat(),
            None,
        ),
        ("one field only", status_with("Uid:\t1000"), None),
        ("no field at all", status_with("Uid:"), None),
        ("tab only after the label", status_with("Uid:\t"), None),
        (
            "non-decimal letters",
            status_with("Uid:\t1000\tabc\t1000\t1000"),
            None,
        ),
        (
            "hexadecimal",
            status_with("Uid:\t1000\t0x10\t1000\t1000"),
            None,
        ),
        ("negative", status_with("Uid:\t1000\t-1\t1000\t1000"), None),
        (
            "leading plus",
            status_with("Uid:\t1000\t+5\t1000\t1000"),
            None,
        ),
        (
            "fractional",
            status_with("Uid:\t1000\t5.0\t1000\t1000"),
            None,
        ),
        (
            "malformed field after the second is not trusted",
            status_with("Uid:\t1000\t0\tzz\t1000"),
            None,
        ),
        (
            "carriage return after the last field",
            b"Uid:\t1\t2\t3\t4\r\n".to_vec(),
            None,
        ),
        (
            "label without a separator",
            status_with("Uid:1000\t1000\t1000\t1000"),
            None,
        ),
        ("empty input", Vec::new(), None),
        ("only a newline", b"\n".to_vec(), None),
        (
            "Uid: in the middle of a line is not a match",
            b"Name:\tUid:\t1\t2\t3\t4\nGid:\t0\t0\t0\t0\n".to_vec(),
            None,
        ),
        (
            "Uid: after leading whitespace is not a match",
            b" Uid:\t1\t2\t3\t4\n".to_vec(),
            None,
        ),
        (
            "lower case label is not a match",
            b"uid:\t1\t2\t3\t4\n".to_vec(),
            None,
        ),
        (
            "a mid-line Uid: does not hide the real one",
            [
                b"Name:\tUid:\t9\t9\t9\t9\n".to_vec(),
                b"Uid:\t1\t2\t3\t4\n".to_vec(),
            ]
            .concat(),
            Some(2),
        ),
        (
            "non-UTF-8 bytes on other lines",
            non_utf8_other_lines,
            Some(1001),
        ),
        (
            "non-UTF-8 byte inside a field",
            b"Uid:\t1\t\xff\t3\t4\n".to_vec(),
            None,
        ),
        ("no trailing newline", no_trailing_newline, Some(6)),
    ];
    assert!(cases.len() >= 12);
    for (label, input, expected) in cases {
        assert_eq!(parse_status_euid(&input), expected, "case: {label}");
    }
}

/// Would fail if the parser took the first field (real uid), the last field, or summed lines.
#[test]
fn parse_status_euid_returns_the_second_field_and_not_its_neighbours() {
    let text = status_with("Uid:\t11\t22\t33\t44");
    assert_eq!(parse_status_euid(&text), Some(22));
}

// ---------------------------------------------------------------------------------------------------
// process_effective_uid on the real host

struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p03_fr33-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        set_mode(&root, 0o700);
        Self { root }
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

/// A file this process creates is owned by its (effective, here also file-system) uid, so the two numbers
/// must agree. Would fail if the reader returned the real uid in a process where they differ, a constant,
/// or the saved uid. In the runner namespace both are 0; outside it both are the invoking user's uid.
#[test]
fn process_effective_uid_agrees_with_the_owner_of_a_file_just_created() {
    let tmp = Tmp::new("euid-real");
    let file = tmp.root.join("probe");
    std::fs::write(&file, b"x").unwrap();
    let owner = std::fs::metadata(&file).unwrap().uid();
    assert_eq!(process_effective_uid().unwrap(), owner);
    // The provided trait method returns the same value for the real file system.
    assert_eq!(RealFs.effective_uid().unwrap(), owner);
}

// ---------------------------------------------------------------------------------------------------
// DataDir::new through a double

/// Delegates to `RealFs`, but reports a chosen effective uid, optionally a different owner uid for chosen
/// paths, and can fail the n-th `symlink_metadata` call on a path.
#[derive(Debug)]
struct EuidFs {
    euid: Result<u32, ErrorKind>,
    /// Paths whose reported owner is replaced.
    owner_of: Vec<(PathBuf, u32)>,
    /// `(path, n)`: the n-th (1-based) `symlink_metadata` of `path` fails.
    fail_stat: Option<(PathBuf, u32)>,
    stat_calls: AtomicU32,
}

impl EuidFs {
    fn new(euid: Result<u32, ErrorKind>) -> Self {
        Self {
            euid,
            owner_of: Vec::new(),
            fail_stat: None,
            stat_calls: AtomicU32::new(0),
        }
    }

    fn owner(mut self, path: &Path, uid: u32) -> Self {
        self.owner_of.push((path.to_path_buf(), uid));
        self
    }
}

impl FsOps for EuidFs {
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
        if let Some((target, n)) = &self.fail_stat
            && path == target
        {
            let seen = self.stat_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if seen == *n {
                return Err(ErrorKind::PermissionDenied.into());
            }
        }
        let mut info = RealFs.symlink_metadata(path)?;
        if let Some((_, uid)) = self.owner_of.iter().find(|(p, _)| p == path) {
            info.uid = *uid;
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
    fn effective_uid(&self) -> io::Result<u32> {
        self.euid.map_err(io::Error::from)
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

/// Would fail if the check were missing (accepted), if it compared the wrong uid, or used the wrong error.
#[test]
fn root_against_a_foreign_owned_directory_is_refused() {
    let tmp = Tmp::new("root-foreign");
    let real = canonical(&tmp.root);
    for foreign in [1, 1000, 65534, u32::MAX] {
        let fs = EuidFs::new(Ok(0)).owner(&real, foreign);
        assert_eq!(
            DataDir::new(&fs, &tmp.root).unwrap_err(),
            StoreError::RootForeignDataDir,
            "owner {foreign}"
        );
    }
}

/// Would fail if root were refused for its own directories (every runner test would then fail).
#[test]
fn root_against_a_root_owned_directory_is_accepted() {
    let tmp = Tmp::new("root-own");
    let real = canonical(&tmp.root);
    let fs = EuidFs::new(Ok(0)).owner(&real, 0);
    assert_eq!(DataDir::new(&fs, &tmp.root).unwrap().as_path(), real);
}

/// Would fail if a non-root euid were refused or the owner compared against anything but 0 for root.
#[test]
fn non_root_keeps_the_p02_behaviour() {
    let tmp = Tmp::new("nonroot");
    let real = canonical(&tmp.root);

    // euid 1000, directory owned by 1000: accepted, and the whole open path works.
    let own = EuidFs::new(Ok(1000)).owner(&real, 1000);
    let dir = DataDir::new(&own, &tmp.root).unwrap();
    assert_eq!(dir.as_path(), real);

    // euid 1000, directory owned by someone else: `DataDir::new` still accepts (P02 behaviour); the owner
    // check stays in `lock::acquire`, which compares against the lock file's owner and refuses.
    let other = EuidFs::new(Ok(1000)).owner(&real, 4242);
    let dir = DataDir::new(&other, &tmp.root).unwrap();
    assert_eq!(acquire(&dir, &other).unwrap_err(), StoreError::PathRejected);

    // euid 1000, directory owned by root: accepted as before.
    let root_owned = EuidFs::new(Ok(1000)).owner(&real, 0);
    DataDir::new(&root_owned, &tmp.root).unwrap();

    // A non-root euid that is not 1000 behaves the same way (the check is "== 0", not "== 1000").
    let euid_1 = EuidFs::new(Ok(1)).owner(&real, 2);
    DataDir::new(&euid_1, &tmp.root).unwrap();
}

/// Would fail if an unreadable euid were treated as non-root (fail open) or mapped to another error.
#[test]
fn an_unreadable_effective_uid_fails_closed_with_path_rejected() {
    let tmp = Tmp::new("euid-error");
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::NotFound,
        ErrorKind::InvalidData,
        ErrorKind::FileTooLarge,
    ] {
        let fs = EuidFs::new(Err(kind));
        assert_eq!(
            DataDir::new(&fs, &tmp.root).unwrap_err(),
            StoreError::PathRejected,
            "{kind:?}"
        );
    }
}

/// The pre-existing checks still come first: a bad directory is `PathRejected` even for root and for a
/// foreign owner, and a non-private mode is not reported as FR-33. Would fail if the FR-33 check ran
/// before them or replaced them.
#[test]
fn existing_rejections_take_precedence_over_fr33() {
    let tmp = Tmp::new("precedence");
    let real = canonical(&tmp.root);
    let fs = EuidFs::new(Ok(0)).owner(&real, 1000);

    set_mode(&tmp.root, 0o755);
    assert_eq!(
        DataDir::new(&fs, &tmp.root).unwrap_err(),
        StoreError::PathRejected
    );
    set_mode(&tmp.root, 0o700);

    assert_eq!(
        DataDir::new(&fs, &tmp.root.join("missing")).unwrap_err(),
        StoreError::PathRejected
    );
    assert_eq!(
        DataDir::new(&fs, Path::new("relative")).unwrap_err(),
        StoreError::PathRejected
    );
    let file = tmp.root.join("file");
    std::fs::write(&file, b"x").unwrap();
    assert_eq!(
        DataDir::new(&fs, &file).unwrap_err(),
        StoreError::PathRejected
    );
    let link = tmp.root.join("link");
    symlink(&tmp.root, &link).unwrap();
    assert_eq!(
        DataDir::new(&fs, &link).unwrap_err(),
        StoreError::PathRejected
    );
}

/// The uid compared is the canonical directory's. The path given goes through a symlinked parent, so the
/// pre-canonical path reports root while the canonical directory reports a foreign owner. Would fail if the
/// owner were taken from the path as given.
#[test]
fn the_owner_checked_is_that_of_the_canonical_directory() {
    let tmp = Tmp::new("canonical-owner");
    let parent = tmp.root.join("parent");
    std::fs::create_dir(&parent).unwrap();
    let data = parent.join("data");
    std::fs::create_dir(&data).unwrap();
    set_mode(&data, 0o700);
    let via_link = tmp.root.join("via");
    symlink(&parent, &via_link).unwrap();
    let given = via_link.join("data");
    let real = canonical(&data);
    assert_ne!(given, real, "the test needs a symlinked parent");

    let fs = EuidFs::new(Ok(0)).owner(&real, 1000).owner(&given, 0);
    assert_eq!(
        DataDir::new(&fs, &given).unwrap_err(),
        StoreError::RootForeignDataDir
    );
    // And the reverse: foreign by the given path only, root-owned canonically, is accepted.
    let fs = EuidFs::new(Ok(0)).owner(&real, 0).owner(&given, 1000);
    assert_eq!(DataDir::new(&fs, &given).unwrap().as_path(), real);
}

/// Root's second stat (of the canonical directory) failing is a rejection, not an acceptance. Would fail if
/// the error were swallowed.
#[test]
fn root_fails_closed_when_the_canonical_directory_cannot_be_inspected() {
    let tmp = Tmp::new("stat-error");
    let real = canonical(&tmp.root);
    // `tmp.root` is already canonical, so the first stat (normalised path) and the second (canonical path)
    // are the 1st and 2nd calls on the same path.
    let mut fs = EuidFs::new(Ok(0)).owner(&real, 0);
    fs.fail_stat = Some((real.clone(), 2));
    assert_eq!(
        DataDir::new(&fs, &tmp.root).unwrap_err(),
        StoreError::PathRejected
    );
    // Control: without the failure the same setup is accepted.
    let fs = EuidFs::new(Ok(0)).owner(&real, 0);
    DataDir::new(&fs, &tmp.root).unwrap();
}

/// The real file system, in whatever uid namespace the tests run in, accepts a directory this process just
/// created (the runner relies on this: every other store test opens such a directory as root).
#[test]
fn the_real_file_system_accepts_a_directory_the_process_created() {
    let tmp = Tmp::new("real-fs");
    assert_eq!(
        DataDir::new(&RealFs, &tmp.root).unwrap().as_path(),
        canonical(&tmp.root)
    );
}

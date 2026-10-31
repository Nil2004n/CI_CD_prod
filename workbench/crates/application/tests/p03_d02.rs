//! P03-D02 tests (`planning/P03-packets/D02-W1.md`): B1 `approvals::list` classifies read failures exactly
//! like the writer path, and B2 `ToolDirs` prints no path in `Debug`. Every test says what would make it
//! fail. Keys are synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use application::approvals::{self, Actor, ApprovalError};
use application::child_env::ToolDirs;
use application::dev_store::{self, DevError, FailureClass};
use domain::contract::ApprovalDecision;
use domain::ids::ApprovalId;
use domain::store::StoreKind;

const KEY: &str = "pwt-test-FAKE-d02-Zq9f7Lk2Xv4m";

/// A private (0700) initialised data directory removed on drop.
struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p03d02-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let tmp = Self { root };
        assert_eq!(dev_store::init(&tmp.root).failure, None);
        tmp
    }

    fn approvals(&self) -> PathBuf {
        self.root.join("approvals.json")
    }

    /// Removes `approvals.json` (file, symlink or directory) so that a test can put something else there.
    fn clear(&self) {
        let path = self.approvals();
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path).unwrap(),
            Ok(_) => std::fs::remove_file(&path).unwrap(),
            Err(_) => {}
        }
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// What the writer path (`resolve`, which opens a `StoreWriter` and reads through `writer.read`) says about
/// the same directory.
fn via_resolve(tmp: &Tmp) -> ApprovalError {
    let unknown = ApprovalId::parse("00000000-0000-4000-8000-0000000000aa").unwrap();
    approvals::resolve(
        &tmp.root,
        unknown,
        ApprovalDecision::ApproveOnce,
        Actor::LocalUserCli,
    )
    .expect_err("the store file is unusable")
}

fn via_list(tmp: &Tmp) -> ApprovalError {
    approvals::list(&tmp.root).expect_err("the store file is unusable")
}

/// B1: `list` and the writer path must give the same error, and it must be the operation class (exit 5), not
/// the storage class (exit 4). Fails with the old mapping, where `list` reported every read error but
/// NotFound as `Storage`.
fn assert_same_malformed(tmp: &Tmp, label: &str) {
    let listed = via_list(tmp);
    let resolved = via_resolve(tmp);
    assert_eq!(
        listed,
        ApprovalError::Store(DevError::Malformed(StoreKind::Approvals)),
        "{label}: list"
    );
    assert_eq!(listed, resolved, "{label}: list and resolve differ");
    let ApprovalError::Store(error) = listed else {
        panic!("{label}: not a store error");
    };
    assert_eq!(error.class(), FailureClass::Operation, "{label}");
}

/// A file over the document bound (64 MiB, here a sparse 64 MiB + 1 byte file).
#[test]
fn an_oversized_approvals_file_is_malformed_from_list_as_from_resolve() {
    let tmp = Tmp::new("oversize");
    tmp.clear();
    let file = std::fs::File::create(tmp.approvals()).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert_same_malformed(&tmp, "oversize");
}

/// A symlink in place of `approvals.json`, to a valid regular file and to a missing target.
#[test]
fn a_symlinked_approvals_file_is_malformed_from_list_as_from_resolve() {
    let tmp = Tmp::new("symlink");
    let real = tmp.root.join("elsewhere.json");
    std::fs::copy(tmp.approvals(), &real).unwrap();
    tmp.clear();
    std::os::unix::fs::symlink(&real, tmp.approvals()).unwrap();
    assert_same_malformed(&tmp, "symlink to a file");
    tmp.clear();
    std::os::unix::fs::symlink(tmp.root.join("missing.json"), tmp.approvals()).unwrap();
    assert_same_malformed(&tmp, "dangling symlink");
}

/// A directory in place of `approvals.json`.
#[test]
fn a_directory_in_place_of_the_approvals_file_is_malformed_from_list_as_from_resolve() {
    let tmp = Tmp::new("directory");
    tmp.clear();
    std::fs::create_dir(tmp.approvals()).unwrap();
    assert_same_malformed(&tmp, "directory");
}

/// Controls: a missing file lists nothing, a regular file with invalid content is `Malformed` from both
/// paths (so the fix did not change the decode classification). Fails if the fix turned a missing file into an
/// error. (A mode-000 control is not possible: the runner executes tests as a mapped root, which ignores mode
/// bits.)
#[test]
fn missing_and_invalid_files_keep_their_classes() {
    let tmp = Tmp::new("controls");
    tmp.clear();
    assert_eq!(approvals::list(&tmp.root), Ok(Vec::new()), "missing file");
    std::fs::write(tmp.approvals(), b"{}").unwrap();
    assert_same_malformed(&tmp, "invalid content");
}

/// B2: `ToolDirs` over a key-bearing path shows no key and no path in `Debug`, compact or pretty. Fails with
/// the derived `Debug`, which prints the four paths.
#[test]
fn tool_dirs_debug_prints_no_path() {
    let keyed = format!("/w/{KEY}/home");
    let dirs = ToolDirs::new(
        Path::new(&keyed),
        Path::new("/w/config-marker"),
        Path::new("/w/cache-marker"),
        Path::new("/w/tmp-marker"),
    )
    .unwrap();
    for text in [format!("{dirs:?}"), format!("{dirs:#?}")] {
        assert!(!text.contains(KEY), "the key is printed");
        assert!(
            !text.contains("marker") && !text.contains("/w/"),
            "a path is printed"
        );
        assert_eq!(text, "ToolDirs(<paths withheld>)");
    }
    let under = ToolDirs::under(
        Path::new(&format!("/data/{KEY}")),
        application::child_env::ToolProfile::Git,
    )
    .unwrap();
    assert!(!format!("{under:?}").contains(KEY));
    // Equality still sees the paths (the Debug change is only a display change).
    let other = ToolDirs::new(
        Path::new("/w/other"),
        Path::new("/w/config-marker"),
        Path::new("/w/cache-marker"),
        Path::new("/w/tmp-marker"),
    )
    .unwrap();
    assert_ne!(dirs, other);
    assert_eq!(dirs, dirs.clone());
}

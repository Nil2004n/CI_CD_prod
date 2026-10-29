//! StoreLock: exclusive, non-blocking single-instance lock on `<data-dir>/.app.lock` (request §5.5).
//!
//! The lock is an OS advisory lock (`File::try_lock`, i.e. `flock`) on the open file, so it dies with the
//! process: an existing but unlocked `.app.lock` is simply reused. The file content is a diagnostic line
//! (pid and UTC time) and is never read or trusted.
//!
//! Before taking the lock, [`acquire`] checks the data directory (P02-D03): the lock file must be a regular
//! file with exactly one link, and the data directory and any existing `rag/` and `backup/` must be real
//! directories without group/other bits whose owner is the owner of the lock file. The process uid cannot be
//! obtained without `unsafe`, so the uid of the lock file this process just opened or created stands for
//! "owned by this process"; this is a proxy, not proof. The checks are path based and not atomic with later
//! writes, and a rejected `acquire` may still have created an empty `.app.lock` (it is opened first so that
//! its owner can be read); nothing else is created or changed and the lock is not held after an error.

use crate::error::{StoreError, WriteStep};
use crate::fs::{FsOps, LockHandle};
use crate::paths::{DataDir, StorePath, check_private_dir};
use domain::time::UtcTimestamp;

/// Permission bits of a newly created lock file.
const LOCK_FILE_MODE: u32 = 0o600;

/// Proof that this process holds the data-directory lock; released on drop.
#[derive(Debug)]
pub struct StoreLock {
    // Held only for its Drop (closing the file releases the lock).
    _handle: LockHandle,
    owner_uid: u32,
}

impl StoreLock {
    /// The uid of the lock file this process opened, which stands for the uid of the process (see the module
    /// documentation).
    pub fn owner_uid(&self) -> u32 {
        self.owner_uid
    }
}

/// Takes the lock without blocking. Held by another open handle or process → [`StoreError::Locked`].
pub fn acquire(dir: &DataDir, fs: &dyn FsOps) -> Result<StoreLock, StoreError> {
    let path = dir.join(&StorePath::lock_file());
    match fs.symlink_metadata(&path) {
        Ok(info) if info.is_regular() => {}
        Ok(_) => return Err(StoreError::PathRejected),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(StoreError::Io {
                step: WriteStep::Lock,
                kind: e.kind(),
            });
        }
    }
    let io_err = |e: std::io::Error| StoreError::Io {
        step: WriteStep::Lock,
        kind: e.kind(),
    };
    let handle = fs.open_lock_file(&path, LOCK_FILE_MODE).map_err(io_err)?;
    // Everything below runs before the lock is taken; an early return drops the handle, so no lock leaks.
    let lock_info = handle.file_info().map_err(io_err)?;
    if !lock_info.is_regular() || lock_info.nlink != 1 {
        return Err(StoreError::PathRejected);
    }
    let owner_uid = lock_info.uid;
    check_private_dir(fs, dir.as_path(), owner_uid)?;
    for name in DataDir::SUBDIRS {
        let sub = dir.as_path().join(name);
        match fs.symlink_metadata(&sub) {
            Ok(_) => check_private_dir(fs, &sub, owner_uid)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_err(e)),
        }
    }
    if !handle.try_lock_exclusive().map_err(io_err)? {
        return Err(StoreError::Locked);
    }
    // Best effort: a diagnostic for a human who finds the file; failure does not affect the lock.
    let line = format!(
        "pid={} utc={}\n",
        std::process::id(),
        UtcTimestamp::now().to_rfc3339()
    );
    let _ = handle.write_diagnostic(&line);
    Ok(StoreLock {
        _handle: handle,
        owner_uid,
    })
}

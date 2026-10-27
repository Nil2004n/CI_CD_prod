//! StoreLock: exclusive, non-blocking single-instance lock on `<data-dir>/.app.lock` (request §5.5).
//!
//! The lock is an OS advisory lock (`File::try_lock`, i.e. `flock`) on the open file, so it dies with the
//! process: an existing but unlocked `.app.lock` is simply reused. The file content is a diagnostic line
//! (pid and UTC time) and is never read or trusted.

use crate::error::{StoreError, WriteStep};
use crate::fs::{FsOps, LockHandle};
use crate::paths::{DataDir, StorePath};
use domain::time::UtcTimestamp;

/// Permission bits of a newly created lock file.
const LOCK_FILE_MODE: u32 = 0o600;

/// Proof that this process holds the data-directory lock; released on drop.
#[derive(Debug)]
pub struct StoreLock {
    // Held only for its Drop (closing the file releases the lock).
    _handle: LockHandle,
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
    Ok(StoreLock { _handle: handle })
}

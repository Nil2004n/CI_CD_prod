//! FaultFs: public test seam that fails one protocol step of the write path (request §5.5).
//!
//! `FaultFs` wraps [`RealFs`]. Every call that carries the planned [`WriteStep`] is counted; the `nth` call
//! (1-based) fails with the planned error kind *before* the real operation runs, so a failed call has no
//! effect on disk. Everything else passes through. Read-only queries, `remove_file` (cleanup) and
//! `random_suffix` are never injected.

use crate::error::WriteStep;
use crate::fs::{FileInfo, FsOps, LockHandle, RealFs};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Which call to fail and how.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FaultPlan {
    pub step: WriteStep,
    /// 1-based index of the failing call among the calls made with `step`; `0` never fires.
    pub nth: u32,
    pub error: io::ErrorKind,
}

/// A [`RealFs`] with one planned failure.
#[derive(Debug)]
pub struct FaultFs {
    inner: RealFs,
    plan: FaultPlan,
    seen: AtomicU32,
    fired: AtomicU32,
}

impl FaultFs {
    pub fn new(plan: FaultPlan) -> Self {
        Self {
            inner: RealFs,
            plan,
            seen: AtomicU32::new(0),
            fired: AtomicU32::new(0),
        }
    }

    /// A wrapper that never fails (the plan can never fire).
    pub fn passthrough() -> Self {
        Self::new(FaultPlan {
            step: WriteStep::Read,
            nth: 0,
            error: io::ErrorKind::Other,
        })
    }

    /// Number of calls made so far with the planned step.
    pub fn calls_at_step(&self) -> u32 {
        self.seen.load(Ordering::SeqCst)
    }

    /// Whether the planned failure has been injected.
    pub fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst) > 0
    }

    fn check(&self, step: WriteStep) -> io::Result<()> {
        if step != self.plan.step {
            return Ok(());
        }
        let n = self.seen.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        if self.plan.nth != 0 && n == self.plan.nth {
            self.fired.fetch_add(1, Ordering::SeqCst);
            return Err(self.plan.error.into());
        }
        Ok(())
    }
}

impl FsOps for FaultFs {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        self.check(WriteStep::Read)?;
        self.inner.read(path, max_bytes)
    }

    fn create_new(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        self.check(step)?;
        self.inner.create_new(step, path, mode)
    }

    fn write_all(&self, step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.check(step)?;
        self.inner.write_all(step, path, bytes)
    }

    fn sync_file(&self, step: WriteStep, path: &Path) -> io::Result<()> {
        self.check(step)?;
        self.inner.sync_file(step, path)
    }

    fn hard_link(&self, step: WriteStep, existing: &Path, new: &Path) -> io::Result<()> {
        self.check(step)?;
        self.inner.hard_link(step, existing, new)
    }

    fn rename(&self, step: WriteStep, from: &Path, to: &Path) -> io::Result<()> {
        self.check(step)?;
        self.inner.rename(step, from, to)
    }

    fn sync_dir(&self, step: WriteStep, dir: &Path) -> io::Result<()> {
        self.check(step)?;
        self.inner.sync_dir(step, dir)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_file(path)
    }

    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        self.inner.list_dir(dir)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo> {
        self.inner.symlink_metadata(path)
    }

    fn create_dir(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        self.check(step)?;
        self.inner.create_dir(step, path, mode)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.canonicalize(path)
    }

    fn random_suffix(&self) -> io::Result<[u8; 8]> {
        self.inner.random_suffix()
    }

    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle> {
        self.check(WriteStep::Lock)?;
        self.inner.open_lock_file(path, mode)
    }

    fn effective_uid(&self) -> io::Result<u32> {
        self.inner.effective_uid()
    }
}

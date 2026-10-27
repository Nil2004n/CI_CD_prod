//! FsOps seam and RealFs, the only std::fs user in production code (request §5.5; static check 4).
//!
//! Every method of [`FsOps`] is one file-system operation of the write protocol. The mutating methods take
//! the [`WriteStep`] they belong to, so [`crate::fault::FaultFs`] can fail exactly one protocol step.
//! Operations are path based (each opens and closes its own file); the data directory is application-owned
//! and mode 0700, so the window between two operations is not defended against a hostile local writer
//! (documented limitation, T07).

use crate::error::WriteStep;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// What kind of object a path names, as seen without following symlinks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Regular,
    Dir,
    Symlink,
    Other,
}

/// The few facts about a path that the store needs (own type, so that only this module names `std::fs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileInfo {
    pub kind: FileKind,
    pub len: u64,
    /// Permission bits only (`& 0o7777`).
    pub mode: u32,
}

impl FileInfo {
    pub fn is_regular(&self) -> bool {
        self.kind == FileKind::Regular
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Dir
    }

    pub fn is_symlink(&self) -> bool {
        self.kind == FileKind::Symlink
    }
}

/// An open lock file. It wraps the real file because `try_lock` needs one; the lock is released when the
/// handle is dropped (the OS also releases it if the process dies).
#[derive(Debug)]
pub struct LockHandle {
    file: std::fs::File,
}

impl LockHandle {
    /// Tries to take an exclusive lock without blocking. `Ok(true)` = acquired, `Ok(false)` = held elsewhere.
    pub fn try_lock_exclusive(&self) -> io::Result<bool> {
        match self.file.try_lock() {
            Ok(()) => Ok(true),
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// Replaces the file content with one diagnostic line. The content is never trusted by any reader.
    pub fn write_diagnostic(&self, line: &str) -> io::Result<()> {
        self.file.set_len(0)?;
        let mut handle = &self.file;
        handle.write_all(line.as_bytes())
    }
}

/// The file-system operations of the store. Implemented by [`RealFs`] and by the fault-injection wrapper.
pub trait FsOps: Send + Sync + std::fmt::Debug {
    /// Reads a whole file; fails with `ErrorKind::FileTooLarge` if it is larger than `max_bytes`.
    /// (Protocol step [`WriteStep::Read`].)
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>>;
    /// Creates an empty file with `O_CREAT|O_EXCL` and the given permission bits.
    fn create_new(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()>;
    /// Writes all bytes to an existing file, from offset 0.
    fn write_all(&self, step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn sync_file(&self, step: WriteStep, path: &Path) -> io::Result<()>;
    /// Hard-links `existing` as `new`; fails with `AlreadyExists` if `new` exists.
    fn hard_link(&self, step: WriteStep, existing: &Path, new: &Path) -> io::Result<()>;
    fn rename(&self, step: WriteStep, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&self, step: WriteStep, dir: &Path) -> io::Result<()>;
    /// Removes a file. Used only for best-effort cleanup, so it is never fault-injected.
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>>;
    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo>;
    /// Creates one directory (not recursive) with the given permission bits.
    fn create_dir(&self, step: WriteStep, path: &Path, mode: u32) -> io::Result<()>;
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    /// Eight random bytes from the operating system (temp-file names).
    fn random_suffix(&self) -> io::Result<[u8; 8]>;
    /// Opens (creating it with `mode` if absent, never truncating) the lock file. (Protocol step
    /// [`WriteStep::Lock`].)
    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle>;
}

/// The real file system. The only production code that touches `std::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealFs;

fn file_kind(t: std::fs::FileType) -> FileKind {
    if t.is_symlink() {
        FileKind::Symlink
    } else if t.is_dir() {
        FileKind::Dir
    } else if t.is_file() {
        FileKind::Regular
    } else {
        FileKind::Other
    }
}

impl FsOps for RealFs {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > max_bytes {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        // Bounded again while reading: the file may grow after the size check.
        let mut buf = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut buf)?;
        if u64::try_from(buf.len()).is_ok_and(|n| n <= max_bytes) {
            Ok(buf)
        } else {
            Err(io::ErrorKind::FileTooLarge.into())
        }
    }

    fn create_new(&self, _step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(path)
            .map(drop)
    }

    fn write_all(&self, _step: WriteStep, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
        file.write_all(bytes)
    }

    fn sync_file(&self, _step: WriteStep, path: &Path) -> io::Result<()> {
        std::fs::File::open(path)?.sync_all()
    }

    fn hard_link(&self, _step: WriteStep, existing: &Path, new: &Path) -> io::Result<()> {
        std::fs::hard_link(existing, new)
    }

    fn rename(&self, _step: WriteStep, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn sync_dir(&self, _step: WriteStep, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn list_dir(&self, dir: &Path) -> io::Result<Vec<OsString>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            names.push(entry?.file_name());
        }
        names.sort();
        Ok(names)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileInfo> {
        let meta = std::fs::symlink_metadata(path)?;
        Ok(FileInfo {
            kind: file_kind(meta.file_type()),
            len: meta.len(),
            mode: meta.mode() & 0o7777,
        })
    }

    fn create_dir(&self, _step: WriteStep, path: &Path, mode: u32) -> io::Result<()> {
        std::fs::DirBuilder::new().mode(mode).create(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn random_suffix(&self) -> io::Result<[u8; 8]> {
        let mut bytes = [0u8; 8];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn open_lock_file(&self, path: &Path, mode: u32) -> io::Result<LockHandle> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(mode)
            .open(path)?;
        Ok(LockHandle { file })
    }
}

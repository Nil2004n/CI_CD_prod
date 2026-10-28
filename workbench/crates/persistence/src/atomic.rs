//! Atomic write protocols: replace with one `.prev` generation, and never-overwrite (request §5.5, §6.3).
//!
//! `write_replace`: (1) create_new temp `.<name>.tmp-<16 hex>` in the same directory, mode 0600; (2) write;
//! (3) sync_file; (4) if `keep_previous` and the target exists, hard-link it to `.<name>.prev.tmp-<hex>` and
//! rename that over `<name>.prev`; (5) rename the temp over the target; (6) sync_dir.
//!
//! `write_new`: temp + write + sync, then `hard_link(temp, final)` (fails if `final` exists), remove the
//! temp, sync_dir. It never overwrites.
//!
//! On any error before the target changes, the app removes only the temp files it created in this call;
//! the previous target stays byte-identical. After the publishing step (rename / link) only `sync_dir` is
//! left; if it fails the new content is already in place and the error still names `SyncDir`.

use crate::error::{StoreError, WriteStep};
use crate::fs::FsOps;
use crate::paths::{DataDir, PRIVATE_DIR_MODE, StorePath};
use std::io;
use std::path::{Path, PathBuf};

/// Permission bits of every file the store creates.
pub const FILE_MODE: u32 = 0o600;
/// Permission bits of every directory the store creates: the one constant [`PRIVATE_DIR_MODE`]
/// (R-D03-4), so the mode a directory is created with and the mode `check_private_dir` accepts cannot
/// diverge.
pub const DIR_MODE: u32 = PRIVATE_DIR_MODE;

const HEX: &[u8; 16] = b"0123456789abcdef";

fn at(step: WriteStep) -> impl Fn(io::Error) -> StoreError {
    move |e| StoreError::Io {
        step,
        kind: e.kind(),
    }
}

/// Whether `name` has the shape of a temp file this module creates: `.<stem>.tmp-<16 lowercase hex>`
/// (the stem is `<name>` or `<name>.prev`).
pub fn is_temp_file_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.') else {
        return false;
    };
    let Some((stem, hex)) = rest.rsplit_once(".tmp-") else {
        return false;
    };
    !stem.is_empty()
        && hex.len() == 16
        && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Creates the directory that holds `path` (`backup/`, `rag/`) with mode 0700 if it is missing. An existing
/// entry must be a real directory (not a symlink). Does nothing for a file directly in the data directory.
pub fn ensure_parent(fs: &dyn FsOps, dir: &DataDir, path: &StorePath) -> Result<(), StoreError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let full = dir.as_path().join(parent);
    let is_real_dir = |fs: &dyn FsOps| match fs.symlink_metadata(&full) {
        Ok(info) if info.is_dir() => Ok(Some(true)),
        Ok(_) => Ok(Some(false)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(at(WriteStep::CreateDir)(e)),
    };
    match is_real_dir(fs)? {
        Some(true) => return Ok(()),
        Some(false) => return Err(StoreError::PathRejected),
        None => {}
    }
    match fs.create_dir(WriteStep::CreateDir, &full, DIR_MODE) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // Lost a creation race: acceptable only if the winner made a real directory.
            return match is_real_dir(fs)? {
                Some(true) => Ok(()),
                _ => Err(StoreError::PathRejected),
            };
        }
        Err(e) => return Err(at(WriteStep::CreateDir)(e)),
    }
    fs.sync_dir(WriteStep::CreateDir, dir.as_path())
        .map_err(at(WriteStep::CreateDir))
}

/// Temp files created by one call, removed (best effort, own files only) if the call fails.
struct Temps<'a> {
    fs: &'a dyn FsOps,
    paths: Vec<PathBuf>,
}

impl Temps<'_> {
    fn add(&mut self, path: &Path) {
        self.paths.push(path.to_path_buf());
    }

    fn forget(&mut self, path: &Path) {
        self.paths.retain(|p| p.as_path() != path);
    }

    fn remove_all(&mut self) {
        for path in self.paths.drain(..) {
            let _ = self.fs.remove_file(&path);
        }
    }
}

fn temp_path(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    infix: &str,
    step: WriteStep,
) -> Result<PathBuf, StoreError> {
    let random = fs.random_suffix().map_err(at(step))?;
    let mut hex = String::with_capacity(16);
    for byte in random {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    let name = format!(".{}{infix}.tmp-{hex}", path.file_name());
    Ok(dir.parent_of(path).join(name))
}

/// Steps 1-3: create the temp, write the bytes, sync it.
fn stage(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    bytes: &[u8],
    temps: &mut Temps<'_>,
) -> Result<PathBuf, StoreError> {
    let tmp = temp_path(fs, dir, path, "", WriteStep::CreateTemp)?;
    fs.create_new(WriteStep::CreateTemp, &tmp, FILE_MODE)
        .map_err(at(WriteStep::CreateTemp))?;
    temps.add(&tmp);
    fs.write_all(WriteStep::WriteTemp, &tmp, bytes)
        .map_err(at(WriteStep::WriteTemp))?;
    fs.sync_file(WriteStep::SyncTemp, &tmp)
        .map_err(at(WriteStep::SyncTemp))?;
    Ok(tmp)
}

fn replace_steps(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    bytes: &[u8],
    keep_previous: bool,
    temps: &mut Temps<'_>,
) -> Result<(), StoreError> {
    let target = dir.join(path);
    let tmp = stage(fs, dir, path, bytes, temps)?;

    let probe_step = if keep_previous {
        WriteStep::LinkPrevious
    } else {
        WriteStep::Replace
    };
    let exists = match fs.symlink_metadata(&target) {
        Ok(info) if info.is_regular() => true,
        Ok(_) => return Err(StoreError::PathRejected),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(at(probe_step)(e)),
    };

    if keep_previous && exists {
        let prev_tmp = temp_path(fs, dir, path, ".prev", WriteStep::LinkPrevious)?;
        fs.hard_link(WriteStep::LinkPrevious, &target, &prev_tmp)
            .map_err(at(WriteStep::LinkPrevious))?;
        temps.add(&prev_tmp);
        fs.rename(
            WriteStep::ReplacePrevious,
            &prev_tmp,
            &dir.join(&path.previous()),
        )
        .map_err(at(WriteStep::ReplacePrevious))?;
        // POSIX `rename(2)` between two links of the same inode succeeds and does nothing, so when
        // `<name>.prev` already names the target's inode (a previous call failed at `Replace` after its
        // `.prev` rename) the temp link is still there. Remove it here, before the main replace, so a
        // successful write never leaves a stray `.<name>.prev.tmp-<hex>`. It stays registered in `temps`
        // until it is gone, so a failure in this block still cleans it up.
        match fs.symlink_metadata(&prev_tmp) {
            Ok(_) => fs
                .remove_file(&prev_tmp)
                .map_err(at(WriteStep::ReplacePrevious))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(at(WriteStep::ReplacePrevious)(e)),
        }
        temps.forget(&prev_tmp);
    }

    fs.rename(WriteStep::Replace, &tmp, &target)
        .map_err(at(WriteStep::Replace))?;
    temps.forget(&tmp);

    fs.sync_dir(WriteStep::SyncDir, &dir.parent_of(path))
        .map_err(at(WriteStep::SyncDir))
}

/// Replaces `path` with `bytes` atomically (rename), optionally keeping the previous generation as
/// `<name>.prev`. On error the previous target is untouched, except after a `SyncDir` error, when the new
/// content is already in place.
pub fn write_replace(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    bytes: &[u8],
    keep_previous: bool,
) -> Result<(), StoreError> {
    ensure_parent(fs, dir, path)?;
    let mut temps = Temps {
        fs,
        paths: Vec::new(),
    };
    let result = replace_steps(fs, dir, path, bytes, keep_previous, &mut temps);
    if result.is_err() {
        temps.remove_all();
    }
    result
}

fn new_steps(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    bytes: &[u8],
    temps: &mut Temps<'_>,
) -> Result<(), StoreError> {
    let tmp = stage(fs, dir, path, bytes, temps)?;
    match fs.hard_link(WriteStep::Replace, &tmp, &dir.join(path)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(StoreError::ReportNameCollision);
        }
        Err(e) => return Err(at(WriteStep::Replace)(e)),
    }
    // The final name now holds the complete content; a leftover temp is harmless (startup cleanup).
    let _ = fs.remove_file(&tmp);
    temps.forget(&tmp);
    fs.sync_dir(WriteStep::SyncDir, &dir.parent_of(path))
        .map_err(at(WriteStep::SyncDir))
}

/// Writes `bytes` to a path that must not exist yet; an existing file is never overwritten
/// ([`StoreError::ReportNameCollision`]).
pub fn write_new(
    fs: &dyn FsOps,
    dir: &DataDir,
    path: &StorePath,
    bytes: &[u8],
) -> Result<(), StoreError> {
    ensure_parent(fs, dir, path)?;
    let mut temps = Temps {
        fs,
        paths: Vec::new(),
    };
    let result = new_steps(fs, dir, path, bytes, &mut temps);
    if result.is_err() {
        temps.remove_all();
    }
    result
}

//! DataDir and StorePath: confinement of every path inside the data directory (request §5.5, D-T04-1).
//!
//! A [`StorePath`] can only be built from a [`StoreKind`] file name or from a validated report file name,
//! so it is always one or two plain segments and can never contain `..`, a separator injection, an absolute
//! path or a hidden name. [`DataDir::join`] is the only way to turn one into a real path.

use crate::error::StoreError;
use crate::fs::FsOps;
use domain::store::StoreKind;
use std::path::{Path, PathBuf};

/// Longest accepted report file name in bytes. Below the 255-byte `NAME_MAX` so that the longest temp name
/// derived from it (`.<name>.prev.tmp-<16 hex>`, +27 bytes) still fits in a directory entry.
pub const MAX_REPORT_NAME_BYTES: usize = 200;

const BACKUP_DIR: &str = "backup";
const LOCK_FILE_NAME: &str = ".app.lock";

/// The application data directory: absolute, existing, a directory, not itself a symlink, in canonical form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DataDir(PathBuf);

impl DataDir {
    /// Validates `path` and returns its canonical form. Any failure is [`StoreError::PathRejected`].
    pub fn new(fs: &dyn FsOps, path: &Path) -> Result<Self, StoreError> {
        if !path.is_absolute() {
            return Err(StoreError::PathRejected);
        }
        // `components()` drops a trailing slash, which would make the OS follow a final symlink.
        let normalised: PathBuf = path.components().collect();
        let info = fs
            .symlink_metadata(&normalised)
            .map_err(|_| StoreError::PathRejected)?;
        if info.is_symlink() || !info.is_dir() {
            return Err(StoreError::PathRejected);
        }
        let canonical = fs
            .canonicalize(&normalised)
            .map_err(|_| StoreError::PathRejected)?;
        Ok(Self(canonical))
    }

    /// The canonical directory path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The real path of a confined store path.
    pub fn join(&self, path: &StorePath) -> PathBuf {
        self.0.join(&path.rel)
    }

    /// The real path of the directory that holds `path` (the data directory itself for a root file).
    pub fn parent_of(&self, path: &StorePath) -> PathBuf {
        match path.parent() {
            Some(parent) => self.0.join(parent),
            None => self.0.clone(),
        }
    }
}

/// A path relative to the data directory: a root file (`repo.json`, `rag/index.json`, …), a report file
/// (`backup/<name>`), a `.prev` generation of one of them, or the lock file. Not constructible otherwise.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct StorePath {
    rel: String,
}

impl StorePath {
    /// The root file of `kind`; `None` for `StoreKind::Report` (report names are generated).
    pub fn root(kind: StoreKind) -> Option<Self> {
        kind.file_name().map(|name| Self {
            rel: name.to_owned(),
        })
    }

    /// `backup/<file_name>` for a validated report file name: 1..=[`MAX_REPORT_NAME_BYTES`] bytes of
    /// `[A-Za-z0-9._-]`, not starting with `.`.
    pub fn report(file_name: &str) -> Result<Self, StoreError> {
        let bytes = file_name.as_bytes();
        let allowed = bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if bytes.is_empty()
            || bytes.len() > MAX_REPORT_NAME_BYTES
            || !allowed
            || bytes.first() == Some(&b'.')
        {
            return Err(StoreError::PathRejected);
        }
        Ok(Self {
            rel: format!("{BACKUP_DIR}/{file_name}"),
        })
    }

    /// The `<name>.prev` generation next to this file.
    pub fn previous(&self) -> Self {
        Self {
            rel: format!("{}.prev", self.rel),
        }
    }

    /// `.app.lock` in the data directory itself.
    pub(crate) fn lock_file() -> Self {
        Self {
            rel: LOCK_FILE_NAME.to_owned(),
        }
    }

    /// Forward-slash relative form, for diagnostics and manifests.
    pub fn as_str(&self) -> &str {
        &self.rel
    }

    /// The final segment.
    pub fn file_name(&self) -> &str {
        self.rel
            .rsplit_once('/')
            .map_or(self.rel.as_str(), |(_, name)| name)
    }

    /// The directory segment (`rag`, `backup`), or `None` for a file directly in the data directory.
    pub fn parent(&self) -> Option<&str> {
        self.rel.rsplit_once('/').map(|(parent, _)| parent)
    }
}

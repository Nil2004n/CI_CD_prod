//! Store errors (frozen, `planning/P02-T01-request.md` §5.4). `Display` is fixed English text: it never
//! contains paths, file content or values from a store document.

use crate::validate::Problems;
use domain::store::StoreKind;
use std::fmt;

/// A step of the write/read protocol, used to name where an I/O error happened and as the fault-injection
/// point of [`crate::fault::FaultFs`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WriteStep {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    LinkPrevious,
    ReplacePrevious,
    Replace,
    SyncDir,
    Read,
    Lock,
    CreateDir,
}

/// Why a store operation failed.
#[derive(Clone, PartialEq, Debug)]
pub enum StoreError {
    /// Another instance holds `.app.lock`.
    Locked,
    /// The root file does not exist.
    NotInitialised(StoreKind),
    /// An I/O error at the named step; the writer is halted afterwards (Data Schema §11.25).
    Io {
        step: WriteStep,
        kind: std::io::ErrorKind,
    },
    /// The document failed validation; nothing was written.
    Invalid(StoreKind, Problems),
    /// The embedded schema could not be used; nothing is valid and nothing is written (FR-15).
    SchemaUnavailable,
    /// A previous storage error halted the writer; reopen to continue.
    Halted,
    /// A report with the same generated name already exists; reports are never overwritten.
    ReportNameCollision,
    /// A path failed data-directory confinement or was not acceptable as a data directory.
    PathRejected,
    /// The existing root file is malformed or foreign; it is never overwritten (restore explicitly).
    Malformed(StoreKind),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Locked => "the data directory is locked by another instance",
            Self::NotInitialised(_) => "the store file does not exist",
            Self::Io { .. } => "a storage operation failed",
            Self::Invalid(..) => "the document failed validation",
            Self::SchemaUnavailable => "the store schema is unavailable",
            Self::Halted => "storage is halted after an earlier failure",
            Self::ReportNameCollision => "a report with the same name already exists",
            Self::PathRejected => "the path was rejected",
            Self::Malformed(_) => "the existing store file is malformed",
        };
        f.write_str(text)
    }
}

impl std::error::Error for StoreError {}

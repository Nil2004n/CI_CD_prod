//! Authorising repository service seam. Operations fail closed until P04-T09.

use std::path::PathBuf;

use domain::contract::RawInput;
use domain::ids::{ApprovalId, RepoId};
use domain::store::repository::Repository;
use persistence::writer::StoreWriter;
use repositories::git::ops::{GitRefusal, HardenedGit};
use repositories::lock::LockError;

use crate::emit::{Clock, Emitter, IdSource};

/// Removal choices; reports remain retained.
#[derive(Debug)]
pub struct RemoveOptions {
    pub delete_checkout: bool,
    pub purge_derived_index: bool,
    pub keep_reports: bool,
}

/// A safe, input-free service failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    Unavailable,
    Locked,
    Storage,
    Missing,
    Duplicate,
    Invalid,
    Unsafe,
    Operation,
}

/// Refresh result; approval is required before Git can run.
#[derive(Debug)]
pub enum RefreshOutcome {
    Completed,
    NeedsApproval(ApprovalId),
    Skipped(RefreshReason),
}

/// A closed reason for a skipped refresh.
#[derive(Debug)]
pub enum RefreshReason {
    Git(GitRefusal),
    Refused,
}

/// Checkout operations must bind their guard to the requested repository.
pub trait CheckoutAccess {
    type Guard;

    fn lock(&mut self, id: RepoId) -> Result<Self::Guard, LockError>;
    fn checkout_path(&self, id: RepoId, guard: &Self::Guard) -> Result<PathBuf, RepositoryError>;
    fn delete(&mut self, id: RepoId, guard: &Self::Guard) -> Result<(), RepositoryError>;
}

/// Holds the application lock before acquiring any checkout lock.
pub struct RepositoryService<'a, C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess> {
    _writer: &'a mut StoreWriter,
    _emitter: &'a mut Emitter<C, I>,
    _clock: &'a dyn Clock,
    _ids: &'a mut dyn IdSource,
    _git: &'a mut G,
    _checkouts: &'a mut L,
}

impl<C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess> std::fmt::Debug
    for RepositoryService<'_, C, I, G, L>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RepositoryService(<withheld>)")
    }
}

impl<'a, C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess>
    RepositoryService<'a, C, I, G, L>
{
    pub fn new(
        writer: &'a mut StoreWriter,
        emitter: &'a mut Emitter<C, I>,
        clock: &'a dyn Clock,
        ids: &'a mut dyn IdSource,
        git: &'a mut G,
        checkouts: &'a mut L,
    ) -> Self {
        Self {
            _writer: writer,
            _emitter: emitter,
            _clock: clock,
            _ids: ids,
            _git: git,
            _checkouts: checkouts,
        }
    }

    pub fn add(&mut self, _input: RawInput) -> Result<RepoId, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    pub fn list(&mut self) -> Result<Vec<Repository>, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    pub fn status(&mut self, _id: RepoId) -> Result<Repository, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    pub fn refresh(&mut self, _id: RepoId) -> Result<RefreshOutcome, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    pub fn remove(&mut self, _id: RepoId, _options: RemoveOptions) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
}

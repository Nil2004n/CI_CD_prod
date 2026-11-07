//! Authorising repository service seam. Operations fail closed until P04-T09.

use std::path::PathBuf;

use domain::contract::RawInput;
use domain::ids::{ApprovalId, RepoId};
use domain::store::repository::Repository;
use persistence::paths::DataDir;
use persistence::writer::StoreWriter;
use repositories::git::ops::{GitRefusal, HardenedGit};
use repositories::lock::{CheckoutLock, LockError, acquire_checkout_lock};

use crate::emit::{Clock, Emitter, IdSource};
use crate::repository_registry::{self, RegistryError};

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
    /// Registry replacement may have taken effect. Reconcile before further action.
    RegistryWriteUncertain,
    /// Registry was published but its event could not be recorded.
    PublishedEventUnavailable,
    /// Registry was published and event publication is uncertain.
    PublishedEventWriteUncertain,
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

/// Local checkout access rooted in a validated application data directory.
///
/// Callers must hold the application store lock before acquiring a checkout lock.
/// The underlying checks are path based and cannot prevent same-user replacement races.
pub struct LocalCheckoutAccess {
    data_dir: DataDir,
}

impl LocalCheckoutAccess {
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }
}

impl std::fmt::Debug for LocalCheckoutAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalCheckoutAccess(<withheld>)")
    }
}

impl CheckoutAccess for LocalCheckoutAccess {
    type Guard = CheckoutLock;

    fn lock(&mut self, id: RepoId) -> Result<Self::Guard, LockError> {
        acquire_checkout_lock(self.data_dir.as_path(), id)
    }

    fn checkout_path(&self, id: RepoId, guard: &Self::Guard) -> Result<PathBuf, RepositoryError> {
        let checkout = guard.checkout(id).map_err(|_| RepositoryError::Unsafe)?;
        let path = checkout.path().map_err(|_| RepositoryError::Unsafe)?;
        if path
            != self
                .data_dir
                .as_path()
                .join("checkouts")
                .join(id.to_string())
        {
            return Err(RepositoryError::Unsafe);
        }
        Ok(path.to_path_buf())
    }

    fn delete(&mut self, _id: RepoId, _guard: &Self::Guard) -> Result<(), RepositoryError> {
        // Recursive deletion is unavailable until race-resistant confinement is proved.
        Err(RepositoryError::Unsafe)
    }
}

/// Holds the application lock before acquiring any checkout lock.
pub struct RepositoryService<'a, C: Clock, I: IdSource, G: HardenedGit, L: CheckoutAccess> {
    writer: &'a mut StoreWriter,
    emitter: &'a mut Emitter<C, I>,
    clock: &'a dyn Clock,
    ids: &'a mut dyn IdSource,
    _git: &'a mut G,
    checkouts: &'a mut L,
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
            writer,
            emitter,
            clock,
            ids,
            _git: git,
            checkouts,
        }
    }

    pub fn add(&mut self, input: RawInput) -> Result<RepoId, RepositoryError> {
        if self.writer.is_v15() {
            let existing = repository_registry::list15(self.writer).map_err(registry_read_error)?;
            let prepared = crate::repository_intake::prepare(
                &input,
                existing.iter().map(|repo| &repo.remote_url),
            )
            .map_err(intake_error)?;
            let id = RepoId::from_random_bytes(
                self.ids
                    .next_bytes()
                    .map_err(|_| RepositoryError::Unavailable)?,
            );
            let record = crate::repository_record15::build(&prepared.remote, id, self.clock.now())
                .map_err(|_| RepositoryError::Invalid)?;
            repository_registry::insert15(self.writer, self.emitter, &ClockRef(self.clock), record)
                .map_err(registry_mutation_error)?;
            return Ok(id);
        }
        // Legacy stores remain read-only for registration until explicitly upgraded.
        let existing = repository_registry::list(self.writer).map_err(registry_read_error)?;
        crate::repository_intake::prepare(&input, existing.iter().map(|repo| &repo.remote_url))
            .map_err(intake_error)?;
        Err(RepositoryError::Unavailable)
    }

    /// List records from an explicitly upgraded 1.5 registry.
    pub fn list15(
        &mut self,
    ) -> Result<Vec<domain::store::repository15::Repository>, RepositoryError> {
        repository_registry::list15(self.writer).map_err(registry_read_error)
    }

    /// Read a record from an explicitly upgraded 1.5 registry.
    pub fn status15(
        &mut self,
        id: RepoId,
    ) -> Result<domain::store::repository15::Repository, RepositoryError> {
        repository_registry::read15(self.writer, id).map_err(registry_read_error)
    }

    pub fn list(&mut self) -> Result<Vec<Repository>, RepositoryError> {
        repository_registry::list(self.writer).map_err(registry_read_error)
    }

    pub fn status(&mut self, id: RepoId) -> Result<Repository, RepositoryError> {
        repository_registry::read(self.writer, id).map_err(registry_read_error)
    }

    pub fn refresh(&mut self, _id: RepoId) -> Result<RefreshOutcome, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }

    pub fn remove(&mut self, id: RepoId, options: RemoveOptions) -> Result<(), RepositoryError> {
        if !options.keep_reports {
            return Err(RepositoryError::Invalid);
        }
        // No checkout deletion implementation currently proves confinement.
        if options.delete_checkout {
            return Err(RepositoryError::Unsafe);
        }
        repository_registry::read(self.writer, id).map_err(registry_read_error)?;
        let _guard = self.checkouts.lock(id).map_err(|error| match error {
            LockError::Locked => RepositoryError::Locked,
            LockError::UnsafePath => RepositoryError::Unsafe,
            LockError::Io => RepositoryError::Storage,
        })?;
        // Purging an index is a documented no-op until an index exists.
        repository_registry::remove(self.writer, self.emitter, &ClockRef(self.clock), id)
            .map_err(registry_mutation_error)
    }
}

fn intake_error(error: crate::repository_intake::IntakeError) -> RepositoryError {
    match error {
        crate::repository_intake::IntakeError::Invalid
        | crate::repository_intake::IntakeError::Destination => RepositoryError::Invalid,
        crate::repository_intake::IntakeError::Duplicate => RepositoryError::Duplicate,
        crate::repository_intake::IntakeError::ExistingUnverifiable => RepositoryError::Unsafe,
    }
}

fn registry_read_error(error: RegistryError) -> RepositoryError {
    match error {
        RegistryError::Missing => RepositoryError::Missing,
        _ => RepositoryError::Storage,
    }
}

fn registry_mutation_error(error: RegistryError) -> RepositoryError {
    match error {
        RegistryError::Storage => RepositoryError::Storage,
        RegistryError::Missing => RepositoryError::Missing,
        RegistryError::Duplicate => RepositoryError::Duplicate,
        RegistryError::RegistryWriteUncertain => RepositoryError::RegistryWriteUncertain,
        RegistryError::Emit | RegistryError::PublishedLogUnavailable => {
            RepositoryError::PublishedEventUnavailable
        }
        RegistryError::PublishedLogWriteUncertain => RepositoryError::PublishedEventWriteUncertain,
    }
}

#[derive(Debug)]
struct ClockRef<'a>(&'a dyn Clock);
impl Clock for ClockRef<'_> {
    fn now(&self) -> domain::time::UtcTimestamp {
        self.0.now()
    }
}

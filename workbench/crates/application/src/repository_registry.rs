//! Repository registry seam. Operations fail closed until P04-T07 implements persistence.

use domain::ids::RepoId;
use domain::store::repository::Repository;
use persistence::writer::StoreWriter;

use crate::emit::{Clock, Emitter, IdSource};

/// Safe, input-free registry failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// Registry support is not implemented.
    Unavailable,
    /// Persistence failed.
    Storage,
    /// The requested repository does not exist.
    Missing,
    /// A repository with the same identity already exists.
    Duplicate,
    /// Event emission failed.
    Emit,
}

/// Read one repository under the existing application store lock.
pub fn read(_writer: &mut StoreWriter, _id: RepoId) -> Result<Repository, RegistryError> {
    Err(RegistryError::Unavailable)
}

/// List registered repositories.
pub fn list(_writer: &mut StoreWriter) -> Result<Vec<Repository>, RegistryError> {
    Err(RegistryError::Unavailable)
}

/// Persist a new repository before emitting its event.
pub fn insert<C: Clock, I: IdSource>(
    _writer: &mut StoreWriter,
    _emitter: &mut Emitter<C, I>,
    _repository: Repository,
) -> Result<(), RegistryError> {
    Err(RegistryError::Unavailable)
}

/// Persist a changed repository before emitting its event.
pub fn replace<C: Clock, I: IdSource>(
    _writer: &mut StoreWriter,
    _emitter: &mut Emitter<C, I>,
    _repository: Repository,
) -> Result<(), RegistryError> {
    Err(RegistryError::Unavailable)
}

/// Remove a repository from the store before emitting its event.
pub fn remove<C: Clock, I: IdSource>(
    _writer: &mut StoreWriter,
    _emitter: &mut Emitter<C, I>,
    _id: RepoId,
) -> Result<(), RegistryError> {
    Err(RegistryError::Unavailable)
}

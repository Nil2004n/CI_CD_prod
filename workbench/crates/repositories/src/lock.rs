//! Checkout lock seam. No lock can be acquired by this scaffold.
use domain::ids::RepoId;
use std::path::Path;

pub struct CheckoutLock {
    _private: (),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockError {
    Locked,
    UnsafePath,
    Io,
}
pub fn acquire_checkout_lock(
    _data_dir: &Path,
    _repo_id: RepoId,
) -> Result<CheckoutLock, LockError> {
    Err(LockError::Io)
}

masked_debug!(CheckoutLock);

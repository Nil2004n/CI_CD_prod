//! Pure construction of an unresolved 1.5 repository record.

use domain::contract::FixedText;
use domain::ids::RepoId;
use domain::store::repository15::{CheckoutState, Repository};
use domain::store::values::AppRelPath;
use domain::time::UtcTimestamp;
use repositories::remote::RemoteSpec;

/// Input-free failure constructing a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError {
    /// The application-owned checkout path could not be represented.
    InvalidPath,
}

/// Build metadata only; this does not register a repository or access storage.
pub fn build(
    remote: &RemoteSpec,
    id: RepoId,
    now: UtcTimestamp,
) -> Result<Repository, RecordError> {
    let local_path =
        AppRelPath::parse(&format!("checkouts/{id}")).map_err(|_| RecordError::InvalidPath)?;
    Ok(Repository {
        repo_id: id,
        display_name: FixedText::RepositoryDisplay.redacted(),
        slug: remote.slug().clone(),
        remote_url: remote.remote_url().clone(),
        canonical_remote: remote.canonical_remote().clone(),
        local_path,
        default_ref_kind: None,
        default_ref_value: None,
        registered_at: now,
        last_update_at: None,
        last_commit_oid: None,
        last_git_version: None,
        checkout_state: CheckoutState::NotCloned,
        last_error_redacted: None,
        inventory_revision: 0,
        inventory_at: None,
        files: Vec::new(),
    })
}

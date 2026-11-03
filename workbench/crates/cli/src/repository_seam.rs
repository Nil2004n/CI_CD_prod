//! Typed seam for the later P04 repository CLI adapter. It is not wired to parsing or execution.

use application::repository_service::RemoveOptions;
use domain::contract::RawInput;
use domain::ids::RepoId;

/// Repository operations the CLI adapter will translate to the application service.
#[derive(Debug)]
pub enum RepositoryCommand {
    /// Register a remote supplied by the user.
    Add(RawInput),
    /// List registered repositories.
    List,
    /// Refresh a repository after authorization.
    Refresh(RepoId),
    /// Show one repository's status.
    Status(RepoId),
    /// Remove a repository with explicit retention options.
    Remove {
        repo_id: RepoId,
        options: RemoveOptions,
    },
}

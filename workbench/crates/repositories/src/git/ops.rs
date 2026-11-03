//! Verified Git operation contract; no implementation is provided here.
use crate::{git::runner::GitError, remote::RemoteSpec};
use domain::{
    store::values::{GitOid, GitRefName},
    time::UtcTimestamp,
};
use std::path::Path;

pub enum TargetRef {
    Branch(GitRefName),
    Tag(GitRefName),
    Commit(GitOid),
}
pub struct Provenance {
    pub oid: GitOid,
    pub target: TargetRef,
    pub git_version: String,
    pub observed_at: UtcTimestamp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitRefusal {
    Dirty,
    UntrackedWouldOverwrite,
    InProgress,
    Diverged,
    MissingRef,
    InvalidRef,
    UnsafeConfig,
    UnsafeAttributes,
    UnsafePath,
    Unsupported,
}
pub enum GitOutcome {
    Completed(Provenance),
    Skipped(GitRefusal),
}
pub trait HardenedGit {
    fn clone_verified(
        &mut self,
        remote: &RemoteSpec,
        target: &TargetRef,
        checkout: &Path,
    ) -> Result<GitOutcome, GitError>;
    fn update_verified(
        &mut self,
        remote: &RemoteSpec,
        target: &TargetRef,
        prior_oid: &GitOid,
        checkout: &Path,
    ) -> Result<GitOutcome, GitError>;
}

masked_debug!(TargetRef, Provenance, GitOutcome);

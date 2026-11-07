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

/// Check complete target-tree path and filter-attribute output as opaque bytes.
/// Callers must enforce runner limits before allocating either buffer. This
/// check alone does not authorize a checkout.
pub fn validate_target_filters(paths: &[u8], attributes: &[u8]) -> Result<(), GitRefusal> {
    use crate::git::runner::{MAX_PATH_BYTES, MAX_PATHS, STDIN_CAP, STDOUT_CAP};
    use std::collections::HashSet;

    if paths.is_empty() || paths.len() > STDIN_CAP || attributes.len() > STDOUT_CAP {
        return Err(GitRefusal::UnsafeAttributes);
    }
    fn fields(bytes: &[u8]) -> Option<impl Iterator<Item = &[u8]>> {
        bytes
            .strip_suffix(&[0])
            .map(|body| body.split(|byte| *byte == 0))
    }
    let path_fields = fields(paths).ok_or(GitRefusal::UnsafeAttributes)?;
    let mut attr_fields = fields(attributes).ok_or(GitRefusal::UnsafeAttributes)?;
    let mut seen = HashSet::new();
    for (index, path) in path_fields.enumerate() {
        if index >= MAX_PATHS
            || path.is_empty()
            || path.len() > MAX_PATH_BYTES
            || !seen.insert(path)
        {
            return Err(GitRefusal::UnsafeAttributes);
        }
        if attr_fields.next() != Some(path)
            || attr_fields.next() != Some(b"filter".as_slice())
            || attr_fields.next() != Some(b"unset".as_slice())
        {
            return Err(GitRefusal::UnsafeAttributes);
        }
    }
    if attr_fields.next().is_some() {
        return Err(GitRefusal::UnsafeAttributes);
    }
    Ok(())
}

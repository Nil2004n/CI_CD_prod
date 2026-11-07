//! Repository, RepositoryFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.5.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1b, T05b.
//!
//! The typed layer does not check `inventory_revision` bookkeeping, that `local_path` lies under the
//! checkout directory, or that `canonical_remote` matches `remote_url`: the semantic rules (T09, Data §11)
//! cover those.

use serde::{Deserialize, Serialize};

use super::plan::{FileRecord, RefKind};
use super::values::{
    AppRelPath, CanonicalRemote, GitOid, GitRefName, RemoteUrl, Slug, VersionLabel,
};
use super::{RepositoriesStoreType, SchemaVersion15 as SchemaVersion};
use crate::ids::RepoId;
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

pub use super::repository::CheckoutState;

/// Check the kind-specific 1.5 reference constraint without exposing its value.
///
/// Branch and tag values are short names, later qualified under `refs/heads/`
/// or `refs/tags/`. A symbolic HEAD is not a resolved branch. This check does
/// not resolve a reference or establish that its target exists.
pub fn valid_target_reference(kind: RefKind, value: &GitRefName) -> bool {
    let name = value.as_str();
    match kind {
        RefKind::Commit => GitOid::parse(name).is_ok(),
        RefKind::Branch | RefKind::Tag => {
            name != "HEAD" && !name.starts_with("refs/") && name.is_ascii()
        }
    }
}

/// A registered repository and its inventory (`$defs/Repository`).
///
/// `display_name` and `last_error_redacted` are FR-31 class R and are re-escaped on read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    /// Repository identifier.
    pub repo_id: RepoId,
    /// Display name.
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub display_name: RedactedText,
    /// Short lowercase name used in file names.
    pub slug: Slug,
    /// Remote URL; the type rejects embedded passwords, queries and fragments.
    pub remote_url: RemoteUrl,
    /// Normalised `host/path` form of the remote, used to detect duplicates.
    pub canonical_remote: CanonicalRemote,
    /// Checkout directory, relative to the application data root.
    pub local_path: AppRelPath,
    /// Kind of the default reference.
    pub default_ref_kind: Option<RefKind>,
    /// Name of the default reference.
    pub default_ref_value: Option<GitRefName>,
    /// Registration time.
    pub registered_at: UtcTimestamp,
    /// Time of the last successful acquire or update, `null` if none.
    pub last_update_at: Option<UtcTimestamp>,
    /// Commit at the last successful acquire or update, `null` if none.
    pub last_commit_oid: Option<GitOid>,
    /// Last successful allowlisted Git version; checked by versioned semantics.
    pub last_git_version: Option<VersionLabel>,
    /// Checkout state.
    pub checkout_state: CheckoutState,
    /// Redacted text of the last error, `null` if none.
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    pub last_error_redacted: Option<RedactedText>,
    /// Inventory revision counter, starting at 0.
    pub inventory_revision: u64,
    /// Time the inventory was taken, `null` if never.
    pub inventory_at: Option<UtcTimestamp>,
    /// Discovered files.
    pub files: Vec<FileRecord>,
}

/// The `repo.json` root (`$defs/RepositoryFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryFile {
    /// Always `"1.5"`.
    pub schema_version: SchemaVersion,
    /// Always `"repositories"`.
    pub store_type: RepositoriesStoreType,
    /// Time of the last write.
    pub updated_at: UtcTimestamp,
    /// The registered repositories.
    pub repositories: Vec<Repository>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_reference_is_kind_specific() -> Result<(), Box<dyn std::error::Error>> {
        for name in ["main", "release/v1"] {
            let value = GitRefName::parse(name)?;
            assert!(valid_target_reference(RefKind::Branch, &value));
            assert!(valid_target_reference(RefKind::Tag, &value));
            assert!(!valid_target_reference(RefKind::Commit, &value));
        }
        for name in ["HEAD", "refs/heads/main", "é"] {
            let value = GitRefName::parse(name)?;
            assert!(!valid_target_reference(RefKind::Branch, &value));
        }
        for name in ["a".repeat(40), "b".repeat(64)] {
            assert!(valid_target_reference(
                RefKind::Commit,
                &GitRefName::parse(&name)?
            ));
        }
        for name in ["a".repeat(39), "A".repeat(40), "g".repeat(40)] {
            assert!(!valid_target_reference(
                RefKind::Commit,
                &GitRefName::parse(&name)?
            ));
        }
        Ok(())
    }
}

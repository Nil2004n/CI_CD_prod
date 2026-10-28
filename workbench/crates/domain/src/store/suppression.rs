//! Suppression, SuppressionFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//! `suppressions.json` is a reserved, read-only root (RES-06 deferred): these types decode it, and
//! `StoreKind::writable` is `false` for it, so the writer refuses to write it.

use serde::{Deserialize, Serialize};

use super::values::Digest;
use super::{SchemaVersion, SuppressionsStoreType};
use crate::ids::{RepoId, SuppressionId};
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

/// A rule that hides one finding of one repository (`$defs/Suppression`).
///
/// Not enforced by the typed layer: cross-field and cross-record semantic rules (Data Schema §11, T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suppression {
    /// Identifier of the suppression.
    pub suppression_id: SuppressionId,
    /// Repository the suppression applies to.
    pub repo_id: RepoId,
    /// Fingerprint of the suppressed finding.
    pub finding_fingerprint: Digest,
    /// Tool rule identifier of the suppressed finding (class R).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub rule_id: RedactedText,
    /// Why the finding is suppressed (class R: user-written text).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub reason: RedactedText,
    /// When the suppression was created.
    pub created_at: UtcTimestamp,
    /// When the suppression lapses, if it does.
    pub expires_at: Option<UtcTimestamp>,
    /// Who created the suppression (class R: user-supplied).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub actor: RedactedText,
    /// Whether the suppression is in force.
    pub enabled: bool,
}

/// Root of `suppressions.json` (`$defs/SuppressionFile`); read-only in this version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuppressionFile {
    /// Schema contract version, always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Root kind, always `"suppressions"`.
    pub store_type: SuppressionsStoreType,
    /// When the file was last written.
    pub updated_at: UtcTimestamp,
    /// The suppressions.
    pub suppressions: Vec<Suppression>,
}

//! SourceRef, Engine, Artifact.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W2, wave 1b, T06.
//!
//! Its inline enums use `closed_enum!` (defined in [`super`]; P02-D05 moved it there from this module): a closed
//! set of wire strings that decodes through the no-echo string visitor of [`super::values`].
//!
//! Scope of the no-echo guarantee (FR-31; P02-D02, P02-D04): it holds exactly for the types defined by
//! `closed_enum!`, which reject an unknown value and a wrong-typed value with fixed text that never quotes the
//! input. It does not hold for the other field types of these records: the reused derive-based domain enums
//! (for example `RunState`, `CheckOutcome`, `Severity`, `AdapterId`), `SanitisedText`, the identifier types,
//! timestamps, integers and booleans, and the names of unknown fields can still be quoted by serde in their
//! error text. For those the guard is the read boundary (`persistence::read`, T08), which maps any decode
//! failure to `ProblemCode::TypedDecode` and never forwards the serde text.

use serde::{Deserialize, Serialize};

use super::values::{
    AppRelPath, Digest, GitOid, RedactionVersion, RelPath, ToolName, VersionLabel,
};
use crate::contract::AdapterId;
use crate::ids::{ApprovalId, ArtifactId, RepoId, RunId, SourceId};
use crate::text::RedactedText;
use crate::time::UtcTimestamp;

closed_enum!(
    /// What a [`SourceRef`] points at (`SourceRef.kind`).
    SourceKind, 8, {
        /// A file of a registered repository.
        RepositoryFile => "repository_file",
        /// A rendered template artifact.
        RenderedArtifact => "rendered_artifact",
        /// A scan report.
        ScanReport => "scan_report",
        /// The activity log.
        ActivityLog => "activity_log",
        /// A saved workflow.
        Workflow => "workflow",
        /// A rule document.
        RuleDocument => "rule_document",
        /// A schema document.
        SchemaDocument => "schema_document",
        /// An external reference.
        ExternalReference => "external_reference",
    }
);

closed_enum!(
    /// What an [`Artifact`] is (`Artifact.kind`).
    ArtifactKind, 4, {
        /// A rendered template.
        Rendered => "rendered",
        /// Content retrieved from an external reference.
        External => "external",
        /// A schema.
        Schema => "schema",
        /// A rule document.
        RuleDocument => "rule_document",
    }
);

/// A pointer to the evidence behind a check, finding, chunk or message (`$defs/SourceRef`).
///
/// Not enforced by the typed layer (schema validation T08 and semantic rules T09 cover them):
/// `document_index`, `line_start` and `line_end` are at least 1 (schema `minimum`), and the cross-field
/// rules of Data Schema §11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// Identifier of this reference.
    pub source_id: SourceId,
    /// What the reference points at.
    pub kind: SourceKind,
    /// Repository the evidence belongs to, if any.
    pub repo_id: Option<RepoId>,
    /// Commit the evidence was read at, if any.
    pub commit_oid: Option<GitOid>,
    /// Run the evidence belongs to, if any.
    pub run_id: Option<RunId>,
    /// Repository-relative path of the file, if the evidence is a file.
    pub file_path: Option<RelPath>,
    /// Artifact the evidence is held in, if any.
    pub artifact_id: Option<ArtifactId>,
    /// 1-based index of the YAML document within the file, if any.
    pub document_index: Option<u64>,
    /// 1-based first line of the evidence, if any.
    pub line_start: Option<u64>,
    /// 1-based last line of the evidence, if any.
    pub line_end: Option<u64>,
    /// Identifier of an external resource (class R: may carry repository- or tool-supplied text).
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    pub resource_identifier: Option<RedactedText>,
    /// Digest of the referenced content, if known.
    pub content_digest: Option<Digest>,
    /// When the evidence was retrieved.
    pub retrieved_at: UtcTimestamp,
    /// Approval that allowed retrieving the evidence, if one was needed.
    pub approval_id: Option<ApprovalId>,
    /// Redaction version applied to the retained content.
    pub redaction_version: RedactionVersion,
    /// Whether the referenced content has changed since it was retrieved.
    pub stale: bool,
}

/// The tool and rule data that produced a check or finding (`$defs/Engine`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Application adapter that ran the tool.
    pub adapter_id: AdapterId,
    /// Version of the adapter.
    pub adapter_version: VersionLabel,
    /// Name of the tool.
    pub tool_name: ToolName,
    /// Version of the tool.
    pub tool_version: VersionLabel,
    /// Version of the rule set, if the tool has one.
    pub ruleset_version: Option<VersionLabel>,
    /// When the tool's rule or advisory data was last updated, if known.
    pub data_updated_at: Option<UtcTimestamp>,
}

/// Content retained or referenced by a run (`$defs/Artifact`).
///
/// Not enforced by the typed layer: cross-field semantic rules of Data Schema §11 (T09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Identifier of the artifact.
    pub artifact_id: ArtifactId,
    /// What the artifact is.
    pub kind: ArtifactKind,
    /// Name shown for the artifact (class R: may derive from repository file names).
    #[serde(deserialize_with = "crate::text::store_read::redacted")]
    pub display_name: RedactedText,
    /// Digest of the artifact content.
    pub content_digest: Digest,
    /// Where the redacted content is retained, relative to the app-data root, if it is retained.
    pub retained_path: Option<AppRelPath>,
    /// Evidence the artifact was built from.
    pub source_refs: Vec<SourceRef>,
    /// Approval that allowed retrieving the artifact, if one was needed.
    pub approval_id: Option<ApprovalId>,
    /// Whether retained content, if any, is redacted content only.
    pub contains_only_redacted_retained_content: bool,
}

//! Selector, RepoTarget, SchemaBinding, RendererSetting, ScanSettings, FileRecord, TargetSnapshot, EngineSnapshot, PlanSnapshot.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1a, T05a.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::values::{
    Digest, FrameworkId, GitOid, GitRefName, Identifier, PathPattern, RelPath, RendererId,
    SchemaResourceId, ToolName, VersionLabel,
};
use crate::contract::{AdapterId, SelectorMode};
use crate::ids::{ApprovalId, FileId, RepoId, WorkflowId};
use crate::text::{RedactedText, SanitisedText};
use crate::time::UtcTimestamp;
use crate::vocab::Category;

/// Kind of a Git reference (`RepoTarget.ref_kind`, `Repository.default_ref_kind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    /// A branch.
    Branch,
    /// A tag.
    Tag,
    /// A commit ID.
    Commit,
}

impl RefKind {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Branch, Self::Tag, Self::Commit];
}

/// Which history is searched for secrets (`ScanSettings.secret_scope`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretScope {
    /// The selected checkout only.
    Checkout,
    /// Git history as well.
    History,
}

impl SecretScope {
    /// Every value, in schema order.
    pub const ALL: [Self; 2] = [Self::Checkout, Self::History];
}

/// Which form of template files is assessed (`ScanSettings.template_mode`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateMode {
    /// Source templates.
    Source,
    /// Rendered output.
    Rendered,
    /// Both.
    Both,
}

impl TemplateMode {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Source, Self::Rendered, Self::Both];
}

/// Where schemas come from (`ScanSettings.schema_mode`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaMode {
    /// Detected automatically.
    Automatic,
    /// Supplied through bindings.
    UserSupplied,
    /// Both.
    Both,
}

impl SchemaMode {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Automatic, Self::UserSupplied, Self::Both];
}

/// Result of file discovery (`FileRecord.discovery_state`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryState {
    /// Eligible for assessment.
    Eligible,
    /// Excluded by selection or policy.
    Excluded,
    /// Present but could not be read.
    Unreadable,
}

impl DiscoveryState {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Eligible, Self::Excluded, Self::Unreadable];
}

/// Availability of a tool in a plan snapshot (`EngineSnapshot.availability`).
///
/// A new enum rather than `contract::ToolAvailability`: the schema set here has no `unavailable` value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineAvailability {
    /// Installed, compatible and verified.
    Ready,
    /// Not installed.
    Missing,
    /// Installed version not compatible.
    Incompatible,
    /// Capability deferred.
    Deferred,
    /// Installed but requires configuration.
    Unconfigured,
}

impl EngineAvailability {
    /// Every value, in schema order.
    pub const ALL: [Self; 5] = [
        Self::Ready,
        Self::Missing,
        Self::Incompatible,
        Self::Deferred,
        Self::Unconfigured,
    ];
}

/// Which files of a repository a target covers (`$defs/Selector`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub mode: SelectorMode,
    pub paths: Vec<RelPath>,
    pub include_patterns: Vec<PathPattern>,
    pub exclude_patterns: Vec<PathPattern>,
}

/// One repository, reference and file selection of a workflow or plan (`$defs/RepoTarget`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoTarget {
    pub repo_id: RepoId,
    pub ref_kind: RefKind,
    pub ref_value: GitRefName,
    pub selector: Selector,
}

/// A user-supplied schema bound to a path pattern (`$defs/SchemaBinding`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaBinding {
    pub path_pattern: PathPattern,
    pub schema_resource: SchemaResourceId,
    pub dialect: Option<Identifier>,
    pub expected_digest: Option<Digest>,
}

/// A template renderer and its redacted parameters (`$defs/RendererSetting`).
///
/// `parameters_redacted` is an object of strings (FR-31 class R, P02-D01): keys are `Identifier`s and values
/// are `RedactedText`, re-escaped on read like every other `RedactedText`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSetting {
    pub framework: FrameworkId,
    pub renderer_id: RendererId,
    pub input_paths: Vec<RelPath>,
    #[serde(deserialize_with = "crate::text::store_read::redacted_map")]
    pub parameters_redacted: BTreeMap<Identifier, RedactedText>,
}

/// What an assessment covers and how (`$defs/ScanSettings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanSettings {
    pub categories: Vec<Category>,
    pub secret_scope: SecretScope,
    pub template_mode: TemplateMode,
    pub schema_mode: SchemaMode,
    pub schema_bindings: Vec<SchemaBinding>,
    pub renderers: Vec<RendererSetting>,
    pub allow_reference_context: bool,
    pub adapter_ids: Vec<AdapterId>,
}

/// A file found in a repository inventory (`$defs/FileRecord`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    pub file_id: FileId,
    pub path: RelPath,
    pub byte_size: u64,
    pub content_digest: Option<Digest>,
    pub commit_oid: GitOid,
    pub detected_framework: Option<FrameworkId>,
    pub discovery_state: DiscoveryState,
    pub reason: Option<SanitisedText>,
}

/// The state of one target when a plan was frozen (`$defs/TargetSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSnapshot {
    pub repo_id: RepoId,
    pub selected_ref: GitRefName,
    pub commit_oid: Option<GitOid>,
    pub inventory_revision: u64,
    pub selected_files: Vec<FileRecord>,
    pub missing_paths: Vec<RelPath>,
    pub snapshot_at: UtcTimestamp,
}

/// The state of one tool when a plan was frozen (`$defs/EngineSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSnapshot {
    pub adapter_id: AdapterId,
    pub adapter_version: VersionLabel,
    pub tool_name: ToolName,
    pub tool_version: Option<VersionLabel>,
    pub ruleset_version: Option<VersionLabel>,
    pub config_digest: Option<Digest>,
    pub manifest_digest: Digest,
    pub availability: EngineAvailability,
    pub data_updated_at: Option<UtcTimestamp>,
}

/// An immutable plan of a run (`$defs/PlanSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSnapshot {
    pub workflow_id: Option<WorkflowId>,
    pub workflow_revision: Option<u64>,
    pub targets: Vec<RepoTarget>,
    pub scan_settings: ScanSettings,
    pub approval_ids: Vec<ApprovalId>,
    pub created_at: UtcTimestamp,
    pub plan_digest: Digest,
    pub target_snapshots: Vec<TargetSnapshot>,
    pub engine_snapshots: Vec<EngineSnapshot>,
}

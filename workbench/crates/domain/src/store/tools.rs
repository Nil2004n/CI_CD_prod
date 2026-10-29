//! ExitMeaning, LicenseRecord, ToolManifest, ToolManifestFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1b, T05b.
//!
//! `ToolManifest.availability` reuses [`ToolAvailability`] (CA-03): its six values equal the schema enum.
//! The typed layer does not check `timeout_seconds >= 1`, `max_output_bytes >= 1`, that exit codes are
//! unique, that a `bundled` tool has a binary digest, or the licence review rules: the semantic rules
//! (T09, Data §11) cover those.

use serde::{Deserialize, Serialize};

use super::values::{
    Destination, Digest, FrameworkId, Identifier, PolicyId, RelPath, SpdxExpression, ToolName,
    VersionLabel,
};
use super::{SchemaVersion, ToolManifestStoreType};
use crate::contract::{AdapterId, ToolAvailability};
use crate::text::SanitisedText;
use crate::time::UtcTimestamp;
use crate::vocab::Category;

closed_enum!(
    /// What an exit code of a tool means (`ExitMeaning.meaning`).
    ExitMeaningKind, 4, {
        /// The tool needs a human to inspect its output.
        InspectionRequired => "inspection_required",
        /// The tool failed to run.
        ExecutionError => "execution_error",
        /// The tool may have reported findings.
        FindingsPossible => "findings_possible",
        /// No findings, provided the tool's output confirms it.
        NoFindingsWhenOutputConfirms => "no_findings_when_output_confirms",
    }
);

closed_enum!(
    /// Review state of a tool's licence (`LicenseRecord.review_status`).
    LicenseReviewStatus, 3, {
        /// Not yet reviewed.
        Pending => "pending",
        /// Reviewed and acceptable.
        Approved => "approved",
        /// Reviewed and not acceptable.
        Blocked => "blocked",
    }
);

closed_enum!(
    /// How a tool is used by the application (`ToolManifest.role`).
    ToolRole, 3, {
        /// Needed at run time.
        Runtime => "runtime",
        /// Needed only for development.
        Development => "development",
        /// Optional runtime helper.
        OptionalHelper => "optional_helper",
    }
);

closed_enum!(
    /// Release tier of a tool (`ToolManifest.release_tier`).
    ReleaseTier, 6, {
        /// Foundation.
        Foundation => "foundation",
        /// Pilot baseline.
        PilotBaseline => "pilot_baseline",
        /// Pilot framework support.
        PilotFramework => "pilot_framework",
        /// Recommended extension.
        RecommendedExtension => "recommended_extension",
        /// Conditional on a decision.
        Conditional => "conditional",
        /// Deferred.
        Deferred => "deferred",
    }
);

closed_enum!(
    /// How a tool is invoked (`ToolManifest.invocation_mode`).
    InvocationMode, 3, {
        /// Native binary run directly.
        Native => "native",
        /// Application-owned shell wrapper.
        Shell => "shell",
        /// Library call inside the application.
        InProcess => "in_process",
    }
);

closed_enum!(
    /// How files are handed to a tool (`ToolManifest.selection_strategy`).
    SelectionStrategy, 4, {
        /// The selected files are passed explicitly.
        ExplicitFiles => "explicit_files",
        /// The selected files are staged in a temporary tree.
        StagedSelection => "staged_selection",
        /// The tool sees wider context that was approved.
        ApprovedWiderContext => "approved_wider_context",
        /// Selection is handled in process.
        InProcess => "in_process",
    }
);

closed_enum!(
    /// Network use of a tool (`ToolManifest.network_policy`).
    NetworkPolicy, 3, {
        /// No network access.
        None => "none",
        /// Local caches only.
        CacheOnly => "cache_only",
        /// Network access only with an approval.
        ApprovalRequired => "approval_required",
    }
);

closed_enum!(
    /// How a tool reaches the user's machine (`ToolManifest.distribution`).
    ToolDistribution, 3, {
        /// Shipped with the application.
        Bundled => "bundled",
        /// Must be installed by the user.
        SystemPrerequisite => "system_prerequisite",
        /// Not shipped.
        NotShipped => "not_shipped",
    }
);

/// Meaning of one exit code of a tool (`$defs/ExitMeaning`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitMeaning {
    /// The exit code. The schema allows any integer; values outside the `i64` range do not decode.
    pub code: i64,
    /// What the code means.
    pub meaning: ExitMeaningKind,
}

/// Licence information for a tool (`$defs/LicenseRecord`).
///
/// `source_obligation_note` is text the application composes (FR-31 class S).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseRecord {
    /// SPDX expression, `null` if unknown.
    pub spdx: Option<SpdxExpression>,
    /// Version the licence was verified for, `null` if not verified.
    pub verified_version: Option<VersionLabel>,
    /// Review state.
    pub review_status: LicenseReviewStatus,
    /// Path of the licence file in the tool distribution, `null` if none.
    pub license_file: Option<RelPath>,
    /// Paths of notice files in the tool distribution.
    pub notice_files: Vec<RelPath>,
    /// Note on source-code obligations, `null` if none.
    pub source_obligation_note: Option<SanitisedText>,
    /// Time of the review, `null` if not reviewed.
    pub reviewed_at: Option<UtcTimestamp>,
}

/// An application-owned description of one tool adapter (`$defs/ToolManifest`).
///
/// `binary_origin` and `limitations` are text the application composes (FR-31 class S).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolManifest {
    /// Adapter identifier (closed set, Data §21).
    pub adapter_id: AdapterId,
    /// Adapter version.
    pub adapter_version: VersionLabel,
    /// Tool name.
    pub tool_name: ToolName,
    /// Installed tool version, `null` if unknown.
    pub tool_version: Option<VersionLabel>,
    /// How the tool is used.
    pub role: ToolRole,
    /// Release tier.
    pub release_tier: ReleaseTier,
    /// Availability (CA-03).
    pub availability: ToolAvailability,
    /// Assessment categories the tool covers.
    pub capabilities: Vec<Category>,
    /// Frameworks the tool understands.
    pub frameworks: Vec<FrameworkId>,
    /// Platforms the tool supports.
    pub platforms: Vec<Identifier>,
    /// Name of the application wrapper, `null` if the tool is run directly.
    pub wrapper_name: Option<Identifier>,
    /// How the tool is invoked.
    pub invocation_mode: InvocationMode,
    /// Identifier of the command policy applied to invocations.
    pub command_policy_id: PolicyId,
    /// Identifier of the output parser.
    pub parser_id: PolicyId,
    /// The tool's native output format.
    pub native_format: Identifier,
    /// Digest of the tool configuration in force, `null` if none.
    pub config_digest: Option<Digest>,
    /// How files are handed to the tool.
    pub selection_strategy: SelectionStrategy,
    /// Network use.
    pub network_policy: NetworkPolicy,
    /// Network destinations the tool may reach when approved.
    pub network_resources: Vec<Destination>,
    /// Meaning of the tool's exit codes.
    pub exit_mapping: Vec<ExitMeaning>,
    /// Time limit for one invocation; the schema minimum of 1 is not enforced here.
    pub timeout_seconds: u64,
    /// Output size limit for one invocation; the schema minimum of 1 is not enforced here.
    pub max_output_bytes: u64,
    /// How the tool reaches the machine.
    pub distribution: ToolDistribution,
    /// Where the binary comes from, `null` if not applicable.
    pub binary_origin: Option<SanitisedText>,
    /// SHA-256 of the binary, `null` if not applicable.
    pub binary_sha256: Option<Digest>,
    /// Licence information.
    pub license: LicenseRecord,
    /// Known limitations of the tool's coverage.
    pub limitations: Vec<SanitisedText>,
}

/// The `tools.json` root (`$defs/ToolManifestFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolManifestFile {
    /// Always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Always `"tool_manifest"`.
    pub store_type: ToolManifestStoreType,
    /// Time of the last write.
    pub updated_at: UtcTimestamp,
    /// The tool manifests.
    pub tools: Vec<ToolManifest>,
}

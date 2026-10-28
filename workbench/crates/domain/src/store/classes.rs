//! FR-31 classification table of the plain-string store properties (Appendix A) and its compile-time guards.
//!
//! Source: `planning/P02-T01-fr31-classification.md`, revision 2 (approved with P02-T01 by the owner on
//! 2026-10-27; revised in debug cycle P02-D01), generated from `schemas/workbench-store-v1.4.schema.json`.
//! [`TABLE`] holds one row per plain-string property of the 57 `$defs` reachable from the ten store roots, in
//! Appendix A order. The six wrapper-protocol `$defs` are not reachable from any store root and are classified
//! with the wrapper protocol in P06 (request T01-D2). "Plain string" means `type: string`, or a nullable
//! string, the items of a string array, or the values of a string map, with no `enum`, `const`, `format`,
//! `pattern` or `$ref`. This module is pure data and lookups; it performs no I/O and is not used by the
//! decoding path. `domain/tests/fr31.rs` proves the table complete against the embedded schema and checks
//! every row against the Rust field type of the matching store model.
//!
//! # Rule of thumb
//!
//! - Text that can originate from a user, a repository, a scanner or tool, or a model is [`Class::Redacted`].
//! - Text the application composes from its own constants and validated values is [`Class::Sanitised`].
//! - Machine identifiers with a closed syntax are [`Class::Newtype`] (validated newtypes of
//!   [`values`](super::values)).
//! - Identifiers with a closed value list in the domain crate are [`Class::ClosedEnum`].
//!
//! Any reclassification after approval is a lead change recorded in the T13 record.
//!
//! # Trust assumption (FR-31 (2))
//!
//! The store is application-owned (Architecture trust boundary 3). A `RedactedText` read back from it is
//! re-escaped (control, bidi, zero-width and separator code points become `<U+XXXX>`), not re-redacted: the
//! redaction happened before the text was written, and the reader does not repeat it.
//!
//! # Disclosed limitations (request §6.6)
//!
//! - Rust has no crate-restricted visibility for trait implementations. The read-back is sealed at the type
//!   level only inside `domain`: the helpers are `pub(crate)` and `RedactedText` has no `Deserialize`, so no
//!   other crate can decode a bare `RedactedText`. The guards below pin that.
//! - Store models are publicly `Deserialize`. The rule "store documents are decoded only in
//!   `persistence::read`" is enforced by the `static` check of `dev/p02_check.py`, not by the compiler.
//! - `text::redact` accepts any `Redactor`. A weak redactor produces weak redaction; the type system only
//!   proves that text passed through some redactor and was escaped.
//! - The table classifies properties, not values. A class-N or class-E field is as safe as its parser; the
//!   class-S fields rely on the application composing them from constants, which review and tests (not the
//!   types) enforce.
//!
//! # Compile-time guards
//!
//! Each `compile_fail` doctest below is paired with a compiling control that differs only in the forbidden
//! part. A `compile_fail` block alone passes for any compile error, so the control is what proves the
//! failure is caused by the forbidden construct. Overlap with `text.rs`: guard 1 repeats the existing
//! "`RedactedText` by deserialising" case of `RedactedText` (which has its own control) and adds the
//! derive-from-outside case; guards 2 and 3 are new.
//!
//! ## Guard 1: `RedactedText` cannot be deserialised from outside the store read-back
//!
//! Control (compiles): the sibling `SanitisedText` decodes with `serde_json`.
//!
//! ```
//! let decoded = serde_json::from_str::<domain::text::SanitisedText>("\"x\"");
//! assert!(decoded.is_ok());
//! ```
//!
//! Compile-fail (E0277, `RedactedText: Deserialize` is not implemented; overlaps `text.rs`):
//!
//! ```compile_fail,E0277
//! let _ = serde_json::from_str::<domain::text::RedactedText>("\"x\"");
//! ```
//!
//! Control (compiles): a downstream struct deriving `Deserialize` over a `SanitisedText` field.
//!
//! ```
//! use domain::text::SanitisedText;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Downstream {
//!     v: SanitisedText,
//! }
//! ```
//!
//! Compile-fail (E0277; new): the same struct over a `RedactedText` field.
//!
//! ```compile_fail,E0277
//! use domain::text::RedactedText;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Downstream {
//!     v: RedactedText,
//! }
//! ```
//!
//! ## Guard 2: a store model rejects a bare `String` where a typed field is required
//!
//! Control (compiles): `SchemaBinding` (class N field `path_pattern: PathPattern`) built from a parsed value.
//!
//! ```
//! use domain::store::plan::SchemaBinding;
//! use domain::store::values::{PathPattern, SchemaResourceId};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = SchemaBinding {
//!         path_pattern: PathPattern::parse("**/*.yaml")?,
//!         schema_resource: SchemaResourceId::parse("example-schema")?,
//!         dialect: None,
//!         expected_digest: None,
//!     };
//!     Ok(())
//! }
//! ```
//!
//! Compile-fail (E0308; new): the same value with a `String` for `path_pattern`.
//!
//! ```compile_fail,E0308
//! use domain::store::plan::SchemaBinding;
//! use domain::store::values::SchemaResourceId;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = SchemaBinding {
//!         path_pattern: String::from("**/*.yaml"),
//!         schema_resource: SchemaResourceId::parse("example-schema")?,
//!         dialect: None,
//!         expected_digest: None,
//!     };
//!     Ok(())
//! }
//! ```
//!
//! Control (compiles): `RendererSetting` (class R map `parameters_redacted`) built with a `RedactedText` value.
//!
//! ```
//! use domain::store::plan::RendererSetting;
//! use domain::store::values::{FrameworkId, Identifier, RendererId};
//! use domain::text::{RedactedText, Redactor, redact};
//! use std::borrow::Cow;
//! use std::collections::BTreeMap;
//!
//! struct Keep;
//! impl Redactor for Keep {
//!     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
//!         Cow::Borrowed(raw)
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut parameters: BTreeMap<Identifier, RedactedText> = BTreeMap::new();
//!     parameters.insert(Identifier::parse("release")?, redact("demo", &Keep));
//!     let _ = RendererSetting {
//!         framework: FrameworkId::parse("helm")?,
//!         renderer_id: RendererId::parse("helm")?,
//!         input_paths: Vec::new(),
//!         parameters_redacted: parameters,
//!     };
//!     Ok(())
//! }
//! ```
//!
//! Compile-fail (E0308; new): the same value with a `String` as the map value.
//!
//! ```compile_fail,E0308
//! use domain::store::plan::RendererSetting;
//! use domain::store::values::{FrameworkId, Identifier, RendererId};
//! use std::collections::BTreeMap;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut parameters = BTreeMap::new();
//!     parameters.insert(Identifier::parse("release")?, String::from("demo"));
//!     let _ = RendererSetting {
//!         framework: FrameworkId::parse("helm")?,
//!         renderer_id: RendererId::parse("helm")?,
//!         input_paths: Vec::new(),
//!         parameters_redacted: parameters,
//!     };
//!     Ok(())
//! }
//! ```
//!
//! Control (compiles): `FileRecord` (class S field `reason: Option<SanitisedText>`) built with `sanitise`.
//!
//! ```
//! use domain::ids::FileId;
//! use domain::store::plan::{DiscoveryState, FileRecord};
//! use domain::store::values::{GitOid, RelPath};
//! use domain::text::sanitise;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = FileRecord {
//!         file_id: FileId::from_random_bytes([1; 16]),
//!         path: RelPath::parse("ci.yml")?,
//!         byte_size: 0,
//!         content_digest: None,
//!         commit_oid: GitOid::parse("0123456789abcdef0123456789abcdef01234567")?,
//!         detected_framework: None,
//!         discovery_state: DiscoveryState::Eligible,
//!         reason: Some(sanitise("excluded by pattern")),
//!     };
//!     Ok(())
//! }
//! ```
//!
//! Compile-fail (E0308; new): the same value with a `String` inside the `Option`.
//!
//! ```compile_fail,E0308
//! use domain::ids::FileId;
//! use domain::store::plan::{DiscoveryState, FileRecord};
//! use domain::store::values::{GitOid, RelPath};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let _ = FileRecord {
//!         file_id: FileId::from_random_bytes([1; 16]),
//!         path: RelPath::parse("ci.yml")?,
//!         byte_size: 0,
//!         content_digest: None,
//!         commit_oid: GitOid::parse("0123456789abcdef0123456789abcdef01234567")?,
//!         detected_framework: None,
//!         discovery_state: DiscoveryState::Eligible,
//!         reason: Some(String::from("excluded by pattern")),
//!     };
//!     Ok(())
//! }
//! ```
//!
//! ## Guard 3: the store read-back helpers are `pub(crate)`
//!
//! Control (compiles): a public function of the same module, called the same way.
//!
//! ```
//! let text = domain::text::sanitise("x");
//! assert_eq!(text.as_str(), "x");
//! ```
//!
//! Compile-fail (E0603, module `store_read` is private; new): calling the read-back helper directly.
//!
//! ```compile_fail,E0603
//! let mut de = serde_json::Deserializer::from_str("\"x\"");
//! let _ = domain::text::store_read::redacted(&mut de);
//! ```
//!
//! Control (compiles): an external struct using a local `deserialize_with` function.
//!
//! ```
//! use serde::Deserialize;
//!
//! fn local<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
//!     String::deserialize(d)
//! }
//!
//! #[derive(Deserialize)]
//! struct Downstream {
//!     #[serde(deserialize_with = "local")]
//!     v: String,
//! }
//! ```
//!
//! Compile-fail (E0603; new): the same struct pointing `deserialize_with` at the read-back helper to obtain
//! a `RedactedText`.
//!
//! ```compile_fail,E0603
//! use domain::text::RedactedText;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Downstream {
//!     #[serde(deserialize_with = "domain::text::store_read::redacted")]
//!     v: RedactedText,
//! }
//! ```

/// The FR-31 class of a plain-string store property (Appendix A).
///
/// Letters used by Appendix A: `E` closed enum, `N` validated newtype, `R` redacted text, `S` sanitised text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    /// `E`: identifiers with a closed value list in the domain crate (`AdapterId`, `EventType`).
    ClosedEnum,
    /// `N`: machine identifiers with a closed syntax, a validated newtype of [`values`](super::values).
    Newtype,
    /// `R`: `RedactedText`, text that can originate from a user, repository, tool or model. Read back only
    /// through the `pub(crate)` `text::store_read` helpers, which re-escape.
    Redacted,
    /// `S`: `SanitisedText`, text the application composes from its own constants and validated values.
    Sanitised,
}

impl Class {
    /// Every class, in Appendix A letter order (E, N, R, S).
    pub const ALL: [Self; 4] = [
        Self::ClosedEnum,
        Self::Newtype,
        Self::Redacted,
        Self::Sanitised,
    ];

    /// The Appendix A letter of the class.
    pub fn letter(self) -> char {
        match self {
            Self::ClosedEnum => 'E',
            Self::Newtype => 'N',
            Self::Redacted => 'R',
            Self::Sanitised => 'S',
        }
    }
}

/// One classified property: `(def, property, class, type name)`.
///
/// `def` is the schema `$def` (equal to the model struct name), `property` the schema property (equal to the
/// field name), `class` its FR-31 class and `type name` the element type: `RedactedText` or `SanitisedText`
/// for classes R and S, the validated newtype for class N, the enum for class E (`AdapterId`, `EventType`;
/// Appendix A annotates them with their specifications). Arrays, nullable properties and string maps keep the
/// element type here; the container is a property of the schema and is checked against the model by
/// `domain/tests/fr31.rs`.
pub type Row = (&'static str, &'static str, Class, &'static str);

/// Number of rows of [`TABLE`] (Appendix A totals: E 6, N 66, R 27, S 6).
pub const ROW_COUNT: usize = 105;

/// The classification table, one row per plain-string property, in Appendix A order.
pub const TABLE: [Row; ROW_COUNT] = [
    (
        "Selector",
        "include_patterns",
        Class::Newtype,
        "PathPattern",
    ),
    (
        "Selector",
        "exclude_patterns",
        Class::Newtype,
        "PathPattern",
    ),
    ("RepoTarget", "ref_value", Class::Newtype, "GitRefName"),
    (
        "SchemaBinding",
        "path_pattern",
        Class::Newtype,
        "PathPattern",
    ),
    (
        "SchemaBinding",
        "schema_resource",
        Class::Newtype,
        "SchemaResourceId",
    ),
    ("SchemaBinding", "dialect", Class::Newtype, "Identifier"),
    (
        "RendererSetting",
        "framework",
        Class::Newtype,
        "FrameworkId",
    ),
    (
        "RendererSetting",
        "renderer_id",
        Class::Newtype,
        "RendererId",
    ),
    (
        "RendererSetting",
        "parameters_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "ScanSettings",
        "adapter_ids",
        Class::ClosedEnum,
        "AdapterId",
    ),
    (
        "FileRecord",
        "detected_framework",
        Class::Newtype,
        "FrameworkId",
    ),
    ("FileRecord", "reason", Class::Sanitised, "SanitisedText"),
    (
        "Repository",
        "display_name",
        Class::Redacted,
        "RedactedText",
    ),
    ("Repository", "slug", Class::Newtype, "Slug"),
    ("Repository", "remote_url", Class::Newtype, "RemoteUrl"),
    (
        "Repository",
        "canonical_remote",
        Class::Newtype,
        "CanonicalRemote",
    ),
    ("Repository", "local_path", Class::Newtype, "AppRelPath"),
    (
        "Repository",
        "default_ref_value",
        Class::Newtype,
        "GitRefName",
    ),
    (
        "Repository",
        "last_error_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    ("Schedule", "daily_local_time", Class::Newtype, "LocalTime"),
    ("Schedule", "timezone", Class::Newtype, "IanaTimezone"),
    ("Workflow", "name", Class::Redacted, "RedactedText"),
    ("Workflow", "description", Class::Redacted, "RedactedText"),
    ("Approval", "purpose", Class::Redacted, "RedactedText"),
    ("Approval", "destinations", Class::Newtype, "Destination"),
    (
        "Approval",
        "resource_constraints",
        Class::Sanitised,
        "SanitisedText",
    ),
    ("Approval", "actor", Class::Redacted, "RedactedText"),
    (
        "SourceRef",
        "resource_identifier",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "SourceRef",
        "redaction_version",
        Class::Newtype,
        "RedactionVersion",
    ),
    ("Engine", "adapter_id", Class::ClosedEnum, "AdapterId"),
    ("Engine", "adapter_version", Class::Newtype, "VersionLabel"),
    ("Engine", "tool_name", Class::Newtype, "ToolName"),
    ("Engine", "tool_version", Class::Newtype, "VersionLabel"),
    ("Engine", "ruleset_version", Class::Newtype, "VersionLabel"),
    ("Check", "framework", Class::Newtype, "FrameworkId"),
    ("Check", "reason_redacted", Class::Redacted, "RedactedText"),
    ("Check", "adapter_id", Class::ClosedEnum, "AdapterId"),
    ("Finding", "rule_id", Class::Redacted, "RedactedText"),
    ("Finding", "title", Class::Redacted, "RedactedText"),
    (
        "Finding",
        "description_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "Finding",
        "evidence_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "Finding",
        "remediation_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    ("Artifact", "display_name", Class::Redacted, "RedactedText"),
    ("Artifact", "retained_path", Class::Newtype, "AppRelPath"),
    ("Event", "event_type", Class::ClosedEnum, "EventType"),
    ("Event", "message_redacted", Class::Redacted, "RedactedText"),
    ("Event", "code", Class::Newtype, "EventCode"),
    ("ReportManifest", "json_path", Class::Newtype, "AppRelPath"),
    ("ReportManifest", "md_path", Class::Newtype, "AppRelPath"),
    ("ReportManifest", "pdf_path", Class::Newtype, "AppRelPath"),
    (
        "ReportManifest",
        "pdf_error_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "ReportManifest",
        "pdf_renderer_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("Suppression", "rule_id", Class::Redacted, "RedactedText"),
    ("Suppression", "reason", Class::Redacted, "RedactedText"),
    ("Suppression", "actor", Class::Redacted, "RedactedText"),
    ("Chunk", "text_redacted", Class::Redacted, "RedactedText"),
    (
        "Chunk",
        "redaction_version",
        Class::Newtype,
        "RedactionVersion",
    ),
    ("Chunk", "embedding_key", Class::Newtype, "Identifier"),
    ("Message", "text_redacted", Class::Redacted, "RedactedText"),
    ("Message", "provider_id", Class::Newtype, "ProviderId"),
    ("Message", "model_id", Class::Newtype, "ModelId"),
    ("Conversation", "title", Class::Redacted, "RedactedText"),
    ("Provider", "model_id", Class::Newtype, "ModelId"),
    (
        "Provider",
        "credential_reference",
        Class::Newtype,
        "CredentialRef",
    ),
    ("Config", "app_data_root", Class::Newtype, "LocalPath"),
    ("Config", "backup_directory", Class::Newtype, "LocalPath"),
    ("Config", "active_theme", Class::Newtype, "Identifier"),
    (
        "Config",
        "redaction_version",
        Class::Newtype,
        "RedactionVersion",
    ),
    ("ReportFile", "repo_name", Class::Redacted, "RedactedText"),
    (
        "ReportFile",
        "remote_identifier_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    ("ReportFile", "selected_ref", Class::Newtype, "GitRefName"),
    (
        "ReportFile",
        "limitations",
        Class::Sanitised,
        "SanitisedText",
    ),
    ("ProviderPolicy", "only", Class::Newtype, "ProviderId"),
    ("InferenceScope", "model_id", Class::Newtype, "ModelId"),
    (
        "InferenceScope",
        "redaction_version",
        Class::Newtype,
        "RedactionVersion",
    ),
    ("AIEnrichment", "requested_model", Class::Newtype, "ModelId"),
    ("AIEnrichment", "actual_model", Class::Newtype, "ModelId"),
    (
        "AIEnrichment",
        "actual_provider",
        Class::Newtype,
        "ProviderId",
    ),
    (
        "AIEnrichment",
        "redaction_version",
        Class::Newtype,
        "RedactionVersion",
    ),
    (
        "AIEnrichment",
        "narrative_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "AIEnrichment",
        "reason_redacted",
        Class::Redacted,
        "RedactedText",
    ),
    (
        "TargetSnapshot",
        "selected_ref",
        Class::Newtype,
        "GitRefName",
    ),
    (
        "EngineSnapshot",
        "adapter_id",
        Class::ClosedEnum,
        "AdapterId",
    ),
    (
        "EngineSnapshot",
        "adapter_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("EngineSnapshot", "tool_name", Class::Newtype, "ToolName"),
    (
        "EngineSnapshot",
        "tool_version",
        Class::Newtype,
        "VersionLabel",
    ),
    (
        "EngineSnapshot",
        "ruleset_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("LicenseRecord", "spdx", Class::Newtype, "SpdxExpression"),
    (
        "LicenseRecord",
        "verified_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("LicenseRecord", "license_file", Class::Newtype, "RelPath"),
    ("LicenseRecord", "notice_files", Class::Newtype, "RelPath"),
    (
        "LicenseRecord",
        "source_obligation_note",
        Class::Sanitised,
        "SanitisedText",
    ),
    ("ToolManifest", "adapter_id", Class::ClosedEnum, "AdapterId"),
    (
        "ToolManifest",
        "adapter_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("ToolManifest", "tool_name", Class::Newtype, "ToolName"),
    (
        "ToolManifest",
        "tool_version",
        Class::Newtype,
        "VersionLabel",
    ),
    ("ToolManifest", "frameworks", Class::Newtype, "FrameworkId"),
    ("ToolManifest", "platforms", Class::Newtype, "Identifier"),
    ("ToolManifest", "wrapper_name", Class::Newtype, "Identifier"),
    (
        "ToolManifest",
        "command_policy_id",
        Class::Newtype,
        "PolicyId",
    ),
    ("ToolManifest", "parser_id", Class::Newtype, "PolicyId"),
    (
        "ToolManifest",
        "native_format",
        Class::Newtype,
        "Identifier",
    ),
    (
        "ToolManifest",
        "network_resources",
        Class::Newtype,
        "Destination",
    ),
    (
        "ToolManifest",
        "binary_origin",
        Class::Sanitised,
        "SanitisedText",
    ),
    (
        "ToolManifest",
        "limitations",
        Class::Sanitised,
        "SanitisedText",
    ),
];

/// The row for `def.property`, if the property is classified.
pub fn lookup(def: &str, property: &str) -> Option<Row> {
    TABLE
        .iter()
        .copied()
        .find(|row| row.0 == def && row.1 == property)
}

/// How many rows have the given class.
pub fn count(class: Class) -> usize {
    TABLE.iter().filter(|row| row.2 == class).count()
}

/// The rows of one class, in table order.
pub fn rows_of(class: Class) -> impl Iterator<Item = Row> {
    TABLE.iter().copied().filter(move |row| row.2 == class)
}

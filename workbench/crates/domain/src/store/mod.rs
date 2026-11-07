//! Typed records of the JSON store, schema contract 1.4 (`schemas/workbench-store-v1.4.schema.json`;
//! frozen conventions `planning/P02-T01-request.md` §5.1–§5.3).
//!
//! Every object `$def` reachable from the ten store roots is a struct here with the schema's property names.
//! Free text uses the FR-31 classes (Appendix A): closed enums, validated newtypes ([`values`]),
//! `RedactedText` (read back only through `crate::text::store_read`) or `SanitisedText`. Decoding store files
//! is the job of `persistence::read` (schema validation first); this module holds no I/O.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Defines a closed string enum with explicit wire values (P02-D05, owner decision E-1: the one mechanism for
/// every enum defined in `domain::store`; reused contract/vocab/state enums keep their derives). Defined before
/// the `mod` declarations below, so it is in textual scope in every child module without an import.
///
/// The generated type derives `Clone, Copy, PartialEq, Eq, Hash, Debug`, has `ALL` (every value in schema
/// order), `as_str` (the wire string) and `from_wire`, and implements `Serialize` and `Deserialize`.
/// `Deserialize` accepts exactly the listed strings and fails with fixed text for anything else, including
/// a wrong-typed value, so the failed input is never part of the error.
macro_rules! closed_enum {
    (
        $(#[$doc:meta])*
        $name:ident, $count:literal, {
            $($(#[$vdoc:meta])* $variant:ident => $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum $name {
            $($(#[$vdoc])* $variant),+
        }

        impl $name {
            /// Every value, in schema order.
            pub const ALL: [Self; $count] = [$(Self::$variant),+];

            /// The wire string of this value.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            /// The value whose wire string is `s`, if any.
            pub fn from_wire(s: &str) -> Option<Self> {
                match s {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::store::values::deserialize_parsed(deserializer, |s| {
                    Self::from_wire(s).ok_or_else(|| String::from("unknown value"))
                })
            }
        }
    };
}

pub mod approval;
pub mod approval15;
pub mod classes;
pub mod config;
pub mod conversation;
pub mod evidence;
pub mod log;
pub mod plan;
pub mod provider;
pub mod rag;
pub mod report;
pub mod repository;
pub mod repository15;
pub mod roots15;
pub mod suppression;
pub mod tools;
pub mod values;
pub mod workflow;

/// The ten store root kinds (schema `oneOf`, Data Schema §2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StoreKind {
    Config,
    Repositories,
    Workflows,
    Approvals,
    Log,
    Suppressions,
    RagIndex,
    Conversations,
    Report,
    ToolManifest,
}

impl StoreKind {
    /// Every kind, in schema `oneOf` order.
    pub const ALL: [Self; 10] = [
        Self::Config,
        Self::Repositories,
        Self::Workflows,
        Self::Approvals,
        Self::Log,
        Self::Suppressions,
        Self::RagIndex,
        Self::Conversations,
        Self::Report,
        Self::ToolManifest,
    ];

    /// The schema `store_type` const.
    pub fn store_type(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Repositories => "repositories",
            Self::Workflows => "workflows",
            Self::Approvals => "approvals",
            Self::Log => "log",
            Self::Suppressions => "suppressions",
            Self::RagIndex => "rag_index",
            Self::Conversations => "conversations",
            Self::Report => "report",
            Self::ToolManifest => "tool_manifest",
        }
    }

    /// File name relative to the app-data root (Data Schema §2). `None` for reports, whose names are
    /// generated per run and repository (Data Schema §13).
    pub fn file_name(self) -> Option<&'static str> {
        match self {
            Self::Config => Some("config.json"),
            Self::Repositories => Some("repo.json"),
            Self::Workflows => Some("workflows.json"),
            Self::Approvals => Some("approvals.json"),
            Self::Log => Some("log.json"),
            Self::Suppressions => Some("suppressions.json"),
            Self::RagIndex => Some("rag/index.json"),
            Self::Conversations => Some("rag/conversations.json"),
            Self::Report => None,
            Self::ToolManifest => Some("tools.json"),
        }
    }

    /// Whether the application writes this root. `suppressions.json` is reserved (RES-06) and read-only.
    pub fn writable(self) -> bool {
        !matches!(self, Self::Suppressions)
    }

    /// The kind whose `store_type` const equals `s`.
    pub fn from_store_type(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.store_type() == s)
    }
}

/// Error text for marker types. Never contains the input.
fn marker_error<E: serde::de::Error>(what: &'static str) -> E {
    E::custom(what)
}

/// `schema_version` of a 1.4 root: serialises as `"1.4"` and accepts nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct SchemaVersion;

/// Version marker for CA-06 records. Kept distinct from the legacy 1.4 marker
/// so serializing a historical record cannot silently change its version.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct SchemaVersion15;

impl Serialize for SchemaVersion15 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("1.5")
    }
}

impl<'de> Deserialize<'de> for SchemaVersion15 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        values::deserialize_parsed(d, |s| {
            if s == "1.5" {
                Ok(Self)
            } else {
                Err(String::from("unsupported schema_version"))
            }
        })
    }
}

impl Serialize for SchemaVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(crate::versions::STORE_SCHEMA_VERSION)
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // P02-D02: through the no-echo visitor, so a wrong-typed value is not quoted in the error.
        values::deserialize_parsed(d, |s| {
            if s == crate::versions::STORE_SCHEMA_VERSION {
                Ok(Self)
            } else {
                Err(String::from("unsupported schema_version"))
            }
        })
    }
}

/// A schema `const: false` boolean (e.g. `telemetry_enabled`, `allow_fallbacks`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ConstFalse;

impl Serialize for ConstFalse {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(false)
    }
}

/// Accepts only `false`; every other value fails with fixed text (P02-D02: `bool::deserialize` makes
/// serde_json quote a wrong-typed string in its error).
struct ConstFalseVisitor;

impl serde::de::Visitor<'_> for ConstFalseVisitor {
    type Value = ConstFalse;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("false")
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<ConstFalse, E> {
        if v {
            Err(marker_error("value must be false"))
        } else {
            Ok(ConstFalse)
        }
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<ConstFalse, E> {
        Err(marker_error("value must be false"))
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<ConstFalse, E> {
        Err(marker_error("value must be false"))
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<ConstFalse, E> {
        Err(marker_error("value must be false"))
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<ConstFalse, E> {
        Err(marker_error("value must be false"))
    }
}

impl<'de> Deserialize<'de> for ConstFalse {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ConstFalseVisitor)
    }
}

/// Implemented by the ten `store_type` marker types only (private trait), so [`AnyStore::kind`] reads the
/// kind from the type of the document's `store_type` field.
trait StoreTypeMarker {
    /// The root kind whose `store_type` const this marker (de)serialises.
    const KIND: StoreKind;
}

macro_rules! store_type_marker {
    ($(#[$doc:meta])* $name:ident, $kind:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
        pub struct $name;

        impl $name {
            /// The root kind this marker belongs to.
            pub const KIND: StoreKind = StoreKind::$kind;
        }

        impl StoreTypeMarker for $name {
            const KIND: StoreKind = StoreKind::$kind;
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(StoreKind::$kind.store_type())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                values::deserialize_parsed(d, |s| {
                    if s == StoreKind::$kind.store_type() {
                        Ok(Self)
                    } else {
                        Err(String::from("store_type does not match the root"))
                    }
                })
            }
        }
    };
}

store_type_marker!(/// `store_type` of `config.json`.
    ConfigStoreType, Config);
store_type_marker!(/// `store_type` of `repo.json`.
    RepositoriesStoreType, Repositories);
store_type_marker!(/// `store_type` of `workflows.json`.
    WorkflowsStoreType, Workflows);
store_type_marker!(/// `store_type` of `approvals.json`.
    ApprovalsStoreType, Approvals);
store_type_marker!(/// `store_type` of `log.json`.
    LogStoreType, Log);
store_type_marker!(/// `store_type` of `suppressions.json`.
    SuppressionsStoreType, Suppressions);
store_type_marker!(/// `store_type` of `rag/index.json`.
    RagIndexStoreType, RagIndex);
store_type_marker!(/// `store_type` of `rag/conversations.json`.
    ConversationsStoreType, Conversations);
store_type_marker!(/// `store_type` of a `backup/` report.
    ReportStoreType, Report);
store_type_marker!(/// `store_type` of `tools.json`.
    ToolManifestStoreType, ToolManifest);

/// A decoded store document of any root kind (request §5.3; completed by the lead at B1).
///
/// Serialises as the root document itself (untagged: the root's own `store_type` member identifies it). It
/// has no `Deserialize`: decoding is `persistence::read`, keyed by the [`StoreKind`] of the file being read,
/// after the version gate and schema validation.
///
/// `Report` is boxed: a report is far larger than every other root (clippy `large_enum_variant`), and
/// boxing does not change the serialised form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum AnyStore {
    /// `config.json`.
    Config(config::ConfigFile),
    /// `repo.json`.
    Repositories(repository::RepositoryFile),
    /// `workflows.json`.
    Workflows(workflow::WorkflowFile),
    /// `approvals.json`.
    Approvals(approval::ApprovalFile),
    /// `log.json`.
    Log(log::LogFile),
    /// `suppressions.json` (reserved, read-only).
    Suppressions(suppression::SuppressionFile),
    /// `rag/index.json`.
    RagIndex(rag::RagIndexFile),
    /// `rag/conversations.json`.
    Conversations(conversation::ConversationFile),
    /// A `backup/` report.
    Report(Box<report::ReportFile>),
    /// `tools.json`.
    ToolManifest(tools::ToolManifestFile),
}

/// Typed 1.5 operational roots, distinct from historical 1.4 documents.
///
/// Construction does not imply validation or permission to write. Immutable
/// reports and reserved suppressions remain outside this operational set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum OperationalStore15 {
    Config(roots15::ConfigFile),
    Repositories(repository15::RepositoryFile),
    Workflows(roots15::WorkflowFile),
    Approvals(approval15::ApprovalFile),
    Log(roots15::LogFile),
    RagIndex(roots15::RagIndexFile),
    Conversations(roots15::ConversationFile),
    ToolManifest(roots15::ToolManifestFile),
}

impl OperationalStore15 {
    /// The root kind carried by this value.
    pub fn kind(&self) -> StoreKind {
        match self {
            Self::Config(_) => StoreKind::Config,
            Self::Repositories(_) => StoreKind::Repositories,
            Self::Workflows(_) => StoreKind::Workflows,
            Self::Approvals(_) => StoreKind::Approvals,
            Self::Log(_) => StoreKind::Log,
            Self::RagIndex(_) => StoreKind::RagIndex,
            Self::Conversations(_) => StoreKind::Conversations,
            Self::ToolManifest(_) => StoreKind::ToolManifest,
        }
    }
}

/// An explicitly versioned store document; 1.4 reports remain historical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionedStore {
    V14(AnyStore),
    V15(OperationalStore15),
}

impl VersionedStore {
    /// The root kind of the versioned document.
    pub fn kind(&self) -> StoreKind {
        match self {
            Self::V14(doc) => doc.kind(),
            Self::V15(doc) => doc.kind(),
        }
    }
}

impl AnyStore {
    /// The root kind, taken from the document's `store_type` marker type, so a variant cannot disagree
    /// with the `store_type` it serialises.
    pub fn kind(&self) -> StoreKind {
        fn of<M: StoreTypeMarker>(_: &M) -> StoreKind {
            M::KIND
        }
        match self {
            Self::Config(f) => of(&f.store_type),
            Self::Repositories(f) => of(&f.store_type),
            Self::Workflows(f) => of(&f.store_type),
            Self::Approvals(f) => of(&f.store_type),
            Self::Log(f) => of(&f.store_type),
            Self::Suppressions(f) => of(&f.store_type),
            Self::RagIndex(f) => of(&f.store_type),
            Self::Conversations(f) => of(&f.store_type),
            Self::Report(f) => of(&f.store_type),
            Self::ToolManifest(f) => of(&f.store_type),
        }
    }
}

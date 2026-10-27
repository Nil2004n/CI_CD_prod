//! Typed records of the JSON store, schema contract 1.4 (`schemas/workbench-store-v1.4.schema.json`;
//! frozen conventions `planning/P02-T01-request.md` §5.1–§5.3).
//!
//! Every object `$def` reachable from the ten store roots is a struct here with the schema's property names.
//! Free text uses the FR-31 classes (Appendix A): closed enums, validated newtypes ([`values`]),
//! `RedactedText` (read back only through `crate::text::store_read`) or `SanitisedText`. Decoding store files
//! is the job of `persistence::read` (schema validation first); this module holds no I/O.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod approval;
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

impl Serialize for SchemaVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(crate::versions::STORE_SCHEMA_VERSION)
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == crate::versions::STORE_SCHEMA_VERSION {
            Ok(Self)
        } else {
            Err(marker_error("unsupported schema_version"))
        }
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

impl<'de> Deserialize<'de> for ConstFalse {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? {
            Err(marker_error("value must be false"))
        } else {
            Ok(Self)
        }
    }
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

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(StoreKind::$kind.store_type())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                if s == StoreKind::$kind.store_type() {
                    Ok(Self)
                } else {
                    Err(marker_error("store_type does not match the root"))
                }
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

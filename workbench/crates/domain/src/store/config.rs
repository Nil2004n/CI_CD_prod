//! Config, ConfigFile.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1, field classes Appendix A). Owner: W1, wave 1b, T05b.
//!
//! `telemetry_enabled` is the constant `false` ([`ConstFalse`]): a document with `true` does not decode.
//! The typed layer does not check the schema minimum of 1 on the three counts or any relation between the
//! paths: the semantic rules (T09, Data §11) cover those.

use serde::{Deserialize, Serialize};

use super::provider::Provider;
use super::values::{Identifier, LocalPath, RedactionVersion};
use super::{ConfigStoreType, ConstFalse, SchemaVersion};
use crate::time::UtcTimestamp;

/// Application settings (`$defs/Config`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Application data directory.
    pub app_data_root: LocalPath,
    /// Directory for backups.
    pub backup_directory: LocalPath,
    /// Name of the active UI theme.
    pub active_theme: Identifier,
    /// Limit on repositories with a live checkout; the schema minimum of 1 is not enforced here.
    pub max_active_repositories: u64,
    /// Limit on concurrent scanner processes; the schema minimum of 1 is not enforced here.
    pub max_scanner_processes: u64,
    /// Days of history to keep, `null` for unlimited; the schema minimum of 1 is not enforced here.
    pub history_retention_days: Option<u64>,
    /// Whether chat history is stored.
    pub chat_persistence_enabled: bool,
    /// The configured model provider, `null` if none.
    pub provider: Option<Provider>,
    /// Always `false`: the application never sends telemetry.
    pub telemetry_enabled: ConstFalse,
    /// Version of the redaction rule set in force.
    pub redaction_version: RedactionVersion,
}

/// The `config.json` root (`$defs/ConfigFile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Always `"1.4"`.
    pub schema_version: SchemaVersion,
    /// Always `"config"`.
    pub store_type: ConfigStoreType,
    /// Time of the last write.
    pub updated_at: UtcTimestamp,
    /// The settings.
    pub config: Config,
}

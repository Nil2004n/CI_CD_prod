//! Independent contract version lines (Architecture A-11, Data Schema §17).

/// Store contract `schema_version` (CA-03, Data Schema §24).
pub const STORE_SCHEMA_VERSION: &str = "1.4";
/// Wrapper protocol `protocol_version` (CA-02).
pub const WRAPPER_PROTOCOL_VERSION: &str = "1.1";
/// Declarative UI schema `ui_schema_version` (CA-03).
pub const UI_SCHEMA_VERSION: &str = "1.1";
/// Application command/event contract version (Architecture §6).
pub use crate::contract::APP_CONTRACT_VERSION;

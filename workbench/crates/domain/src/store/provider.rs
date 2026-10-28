//! ProviderPolicy, Provider, InferenceScope.
//!
//! Typed records for the schema `$defs` of the same names (`schemas/workbench-store-v1.4.schema.json`;
//! conventions `planning/P02-T01-request.md` §5.1). The schema pins the provider to OpenRouter and the
//! routing policy to the privacy constants of Data §18.11; those constants are marker types here, so a
//! document with any other value fails to decode.

use std::fmt;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::ConstFalse;
use super::values::{CredentialRef, ModelId, ProviderId, RedactionVersion, deserialize_parsed};
use crate::ids::SourceId;

macro_rules! str_const {
    ($(#[$doc:meta])* $name:ident, $value:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
        pub struct $name;

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str($value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserialize_parsed(deserializer, |s| {
                    if s == $value {
                        Ok(Self)
                    } else {
                        Err(String::from("unexpected constant value"))
                    }
                })
            }
        }
    };
}

str_const!(
    /// Schema `const "openrouter"` (`provider_id`).
    OpenRouterId,
    "openrouter"
);

str_const!(
    /// Schema `const "api"` (`Provider.kind`).
    ApiKind,
    "api"
);

str_const!(
    /// Schema `const "https://openrouter.ai/api/v1"` (`Provider.endpoint`).
    OpenRouterEndpoint,
    "https://openrouter.ai/api/v1"
);

str_const!(
    /// Schema `const "deny"` (`ProviderPolicy.data_collection`).
    DenyDataCollection,
    "deny"
);

/// A schema `const: true` boolean (`ProviderPolicy.zdr`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ConstTrue;

impl Serialize for ConstTrue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for ConstTrue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TrueVisitor;

        impl Visitor<'_> for TrueVisitor {
            type Value = ConstTrue;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("the boolean true")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<ConstTrue, E> {
                if v {
                    Ok(ConstTrue)
                } else {
                    Err(E::custom("value must be true"))
                }
            }

            // The default `visit_*` methods quote the value; these overrides use a fixed message so a
            // failed decode never echoes the input (FR-31).
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<ConstTrue, E> {
                Err(E::custom("value must be true"))
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<ConstTrue, E> {
                Err(E::custom("value must be true"))
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<ConstTrue, E> {
                Err(E::custom("value must be true"))
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<ConstTrue, E> {
                Err(E::custom("value must be true"))
            }
        }

        // `deserialize_any`, not `deserialize_bool`: serde_json reports a wrongly typed value through
        // `invalid_type`, which quotes it; with `deserialize_any` the visitor methods above run instead.
        deserializer.deserialize_any(TrueVisitor)
    }
}

closed_enum!(
    /// Where the provider credential lives (`Provider.credential_mode`).
    ///
    /// A new enum rather than `contract::CredentialMode`: that one serialises the OS-store value as
    /// `os_store`, the schema value is `os_credential_store`.
    ProviderCredentialMode, 2, {
        /// Held in memory for this session only.
        SessionOnly => "session_only",
        /// OS credential store.
        OsCredentialStore => "os_credential_store",
    }
);

/// Fixed OpenRouter routing/privacy policy (`$defs/ProviderPolicy`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPolicy {
    pub data_collection: DenyDataCollection,
    pub zdr: ConstTrue,
    pub allow_fallbacks: ConstFalse,
    pub only: Vec<ProviderId>,
}

/// The configured model provider (`$defs/Provider`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub provider_id: OpenRouterId,
    pub kind: ApiKind,
    pub endpoint: OpenRouterEndpoint,
    pub model_id: ModelId,
    pub credential_mode: ProviderCredentialMode,
    pub credential_reference: Option<CredentialRef>,
    pub routing_policy: ProviderPolicy,
}

/// What an approval allows to be sent to the model provider (`$defs/InferenceScope`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceScope {
    pub provider_id: OpenRouterId,
    pub model_id: ModelId,
    pub routing_policy: ProviderPolicy,
    pub source_ids: Vec<SourceId>,
    pub redaction_version: RedactionVersion,
}

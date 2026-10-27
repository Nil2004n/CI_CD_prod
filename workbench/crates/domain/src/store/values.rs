//! Validated newtypes for store values (FR-31 class N; Data Schema §4; `planning/P02-T01-request.md` §5.2).
//!
//! The error types below are lead-authored and frozen at B0. The newtypes themselves (one per row of the
//! request §5.2 table, each with `parse`, `as_str`, `Display`, `Serialize`, `Deserialize` through `parse`)
//! are implemented by W1 in wave 1a (T05a).

use std::fmt;

/// Which newtype rejected a value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueKind {
    Digest,
    GitOid,
    RelPath,
    AppRelPath,
    LocalPath,
    PathPattern,
    GitRefName,
    RemoteUrl,
    CanonicalRemote,
    Slug,
    LocalTime,
    IanaTimezone,
    Destination,
    RedactionVersion,
    VersionLabel,
    ToolName,
    FrameworkId,
    RendererId,
    SchemaResourceId,
    ModelId,
    ProviderId,
    CredentialRef,
    PolicyId,
    SpdxExpression,
    EventCode,
    Identifier,
}

/// Why a value was rejected. Fieldless: errors never contain the input text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueReason {
    /// Empty where a value is required.
    Empty,
    /// Longer than the type's byte bound.
    TooLong,
    /// Contains a forbidden character (control, escape-set code point or outside the type's charset).
    Character,
    /// Characters are allowed but the structure is not (e.g. `..` segment, missing host).
    Syntax,
    /// Structurally valid but forbidden by policy (e.g. credentials in a URL, key-like credential reference).
    Forbidden,
}

/// A rejected value: the type and the reason, never the value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ValueError {
    pub kind: ValueKind,
    pub reason: ValueReason,
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {:?} value ({:?})", self.kind, self.reason)
    }
}

impl std::error::Error for ValueError {}

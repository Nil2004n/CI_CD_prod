//! Typed UUID identifiers (P01-T05; frozen surface `planning/P01-T01-request.md` §5.1).
//!
//! Canonical form only: lowercase hyphenated `8-4-4-4-12` hex. The nil UUID is rejected; version and
//! variant bits are not enforced. Errors never contain the input text.

use std::fmt;

/// Why a string is not a canonical identifier. Carries no input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// The input is not 36 characters long.
    Length {
        /// Length of the rejected input in bytes.
        found: usize,
    },
    /// Wrong hyphen positions, non-hex or uppercase characters.
    Format,
    /// The nil UUID (all zeros).
    Nil,
}

impl fmt::Display for IdError {
    #[allow(clippy::todo, unused_variables)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("P01-T05")
    }
}

impl std::error::Error for IdError {}

/// Parses the canonical 36-character form into its 16 bytes.
#[allow(clippy::todo, unused_variables)]
fn parse_canonical(s: &str) -> Result<[u8; 16], IdError> {
    todo!("P01-T05")
}

/// Writes the canonical lowercase hyphenated form.
#[allow(clippy::todo, unused_variables)]
fn write_canonical(bytes: &[u8; 16], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    todo!("P01-T05")
}

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Parses the canonical lowercase hyphenated UUID form.
            pub fn parse(s: &str) -> Result<Self, IdError> {
                parse_canonical(s).map(Self)
            }

            /// The 16 identifier bytes.
            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_canonical(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "("))?;
                write_canonical(&self.0, f)?;
                f.write_str(")")
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                Self::parse(&s).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_id!(/// Approval record identifier.
    ApprovalId);
define_id!(/// Artifact identifier.
    ArtifactId);
define_id!(/// Check record identifier.
    CheckId);
define_id!(/// Retrieval chunk identifier.
    ChunkId);
define_id!(/// Conversation identifier.
    ConversationId);
define_id!(/// Event identifier.
    EventId);
define_id!(/// File record identifier.
    FileId);
define_id!(/// Finding identifier.
    FindingId);
define_id!(/// Chat message identifier.
    MessageId);
define_id!(/// Repository identifier.
    RepoId);
define_id!(/// Report identifier.
    ReportId);
define_id!(/// Wrapper/adapter request identifier.
    RequestId);
define_id!(/// Run identifier.
    RunId);
define_id!(/// Source reference identifier.
    SourceId);
define_id!(/// Suppression identifier (reserved, RES-06 deferred).
    SuppressionId);
define_id!(/// Task identifier.
    TaskId);
define_id!(/// Workflow identifier.
    WorkflowId);

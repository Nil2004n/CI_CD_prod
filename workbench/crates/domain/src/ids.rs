//! Typed UUID identifiers (P01-T05; frozen surface `planning/P01-T01-request.md` §5.1).
//!
//! Canonical form only: lowercase hyphenated `8-4-4-4-12` hex. The nil UUID is rejected; version and
//! variant bits are not enforced. Errors never contain the input text.
//!
//! Identifier types are distinct: a `RunId` cannot be passed where a `RepoId` is required.
//! Stable rustdoc does not check the error code of a `compile_fail` doctest, so the positive
//! control below differs from the failing example only in the offending line.
//!
//! ```compile_fail
//! use domain::ids::{RepoId, RunId};
//! fn takes_repo(_: RepoId) {}
//! let run = RunId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b").unwrap();
//! let repo = RepoId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b").unwrap();
//! takes_repo(run);
//! ```
//!
//! Positive control (compiles and runs; only the last line differs):
//!
//! ```
//! use domain::ids::{RepoId, RunId};
//! fn takes_repo(_: RepoId) {}
//! let run = RunId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b").unwrap();
//! let repo = RepoId::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b").unwrap();
//! takes_repo(repo);
//! ```

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Fixed texts only: the rejected input may be hostile and is never echoed. The `Length`
        // arm prints the numeric byte length, which is a count and not input text.
        match self {
            Self::Length { found } => {
                write!(
                    f,
                    "identifier must be 36 characters long (found {found} bytes)"
                )
            }
            Self::Format => f.write_str("identifier is not lowercase hyphenated 8-4-4-4-12 hex"),
            Self::Nil => f.write_str("identifier must not be the nil UUID"),
        }
    }
}

impl std::error::Error for IdError {}

/// Byte offsets of the four hyphens in the canonical form.
const HYPHENS: [usize; 4] = [8, 13, 18, 23];

/// Value of a lowercase hexadecimal digit; uppercase and non-hex bytes give `None`.
fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Parses the canonical 36-character form into its 16 bytes.
///
/// Check order: byte length (`Length`), then hyphen positions and digits (`Format`), then the nil
/// value (`Nil`). Multi-byte input of exactly 36 bytes fails the digit scan with `Format`.
fn parse_canonical(s: &str) -> Result<[u8; 16], IdError> {
    let input = s.as_bytes();
    if input.len() != 36 {
        return Err(IdError::Length { found: input.len() });
    }
    let mut out = [0u8; 16];
    let mut nibbles = 0usize;
    for (pos, &b) in input.iter().enumerate() {
        if HYPHENS.contains(&pos) {
            if b != b'-' {
                return Err(IdError::Format);
            }
            continue;
        }
        let v = hex_value(b).ok_or(IdError::Format)?;
        if let Some(slot) = out.get_mut(nibbles / 2) {
            *slot = if nibbles.is_multiple_of(2) {
                v << 4
            } else {
                *slot | v
            };
        }
        nibbles += 1;
    }
    if out.iter().all(|&b| b == 0) {
        return Err(IdError::Nil);
    }
    Ok(out)
}

/// Writes the canonical lowercase hyphenated form.
fn write_canonical(bytes: &[u8; 16], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use fmt::Write as _;
    const DIGITS: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            f.write_char('-')?;
        }
        for nibble in [b >> 4, b & 0x0f] {
            if let Some(&c) = DIGITS.get(usize::from(nibble)) {
                f.write_char(c)?;
            }
        }
    }
    Ok(())
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

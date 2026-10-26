//! UTC timestamps (P01-T05; frozen surface `planning/P01-T01-request.md` §5.2).
//!
//! Accepted form: `YYYY-MM-DDTHH:MM:SS[.f{1,9}]Z`, years 0001–9999, no leap second, `Z` only.
//! Canonical output omits a zero fraction and trims trailing fractional zeros.

use std::fmt;

/// Why a timestamp was rejected. Carries no input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    /// Not the accepted RFC 3339 UTC form.
    Format,
    /// A field or the instant is outside the supported range.
    Range,
}

impl fmt::Display for TimeError {
    #[allow(clippy::todo, unused_variables)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("P01-T05")
    }
}

impl std::error::Error for TimeError {}

/// An instant in UTC, stored as seconds and nanoseconds since the Unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp {
    secs: i64,
    nanos: u32,
}

impl UtcTimestamp {
    /// Parses the accepted RFC 3339 UTC form.
    #[allow(clippy::todo, unused_variables)]
    pub fn parse_rfc3339(s: &str) -> Result<Self, TimeError> {
        todo!("P01-T05")
    }

    /// Formats the canonical RFC 3339 UTC form.
    #[allow(clippy::todo)]
    pub fn to_rfc3339(&self) -> String {
        todo!("P01-T05")
    }

    /// Builds a timestamp from Unix seconds and nanoseconds (`nanos < 1_000_000_000`,
    /// instant within years 0001–9999).
    #[allow(clippy::todo, unused_variables)]
    pub fn from_unix(secs: i64, nanos: u32) -> Result<Self, TimeError> {
        todo!("P01-T05")
    }

    /// Seconds since the Unix epoch.
    pub fn unix_seconds(&self) -> i64 {
        self.secs
    }

    /// Nanoseconds within the second.
    pub fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// The current system time.
    #[allow(clippy::todo)]
    pub fn now() -> Self {
        todo!("P01-T05")
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl fmt::Debug for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UtcTimestamp({})", self.to_rfc3339())
    }
}

impl serde::Serialize for UtcTimestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> serde::Deserialize<'de> for UtcTimestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse_rfc3339(&s).map_err(serde::de::Error::custom)
    }
}

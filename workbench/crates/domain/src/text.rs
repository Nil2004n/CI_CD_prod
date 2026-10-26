//! Text-safety newtypes (P01-T06; frozen surface `planning/P01-T01-request.md` §5.3).
//!
//! [`SanitisedText`] has control, bidi, zero-width and separator code points escaped as `<U+XXXX>`.
//! [`RedactedText`] has additionally passed through a [`Redactor`] before escaping. Neither can be built
//! from a raw string except through [`sanitise`] and [`redact`]; `RedactedText` cannot be deserialised.

use std::borrow::Cow;
use std::fmt;

/// Replaces secret material in raw text. Implementations are supplied by later phases (P06).
pub trait Redactor {
    /// Returns the text with secret material replaced.
    fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str>;
}

/// Text with unsafe code points escaped.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SanitisedText(String);

/// Text that passed through a [`Redactor`] and was then escaped like [`SanitisedText`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RedactedText(String);

/// Escapes the code points listed in the frozen escape set.
pub fn sanitise(raw: &str) -> SanitisedText {
    SanitisedText(escape(raw))
}

/// Runs `redactor` over the raw text, then escapes the result.
pub fn redact(raw: &str, redactor: &dyn Redactor) -> RedactedText {
    RedactedText(escape(&redactor.redact(raw)))
}

/// Escapes every code point of the frozen escape set as `<U+XXXX>`.
#[allow(clippy::todo, unused_variables)]
fn escape(raw: &str) -> String {
    todo!("P01-T06")
}

impl SanitisedText {
    /// The sanitised text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the text is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl RedactedText {
    /// The redacted text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the text is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The same text as a [`SanitisedText`] (redacted text is also sanitised).
    pub fn to_sanitised(&self) -> SanitisedText {
        SanitisedText(self.0.clone())
    }
}

impl fmt::Display for SanitisedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for RedactedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for SanitisedText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for SanitisedText {
    /// Re-sanitises on read; never yields a [`RedactedText`].
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(sanitise(&s))
    }
}

impl serde::Serialize for RedactedText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

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
///
/// # Construction is closed
///
/// `SanitisedText` from a `String` via `From` (positive control compiles):
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// `SanitisedText` from a `String` via `From` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = SanitisedText::from(String::from("x"));
/// ```
///
/// `SanitisedText` from a `String` via `Into` (positive control compiles):
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// `SanitisedText` from a `String` via `Into` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = String::from("x").into();
/// ```
///
/// A `String` passed where `SanitisedText` is required (positive control compiles):
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// fn f(_: SanitisedText) {}
/// f(sanitise("x"));
/// ```
///
/// A `String` passed where `SanitisedText` is required (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// fn f(_: SanitisedText) {}
/// f(String::from("x"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SanitisedText(String);

/// Text that passed through a [`Redactor`] and was then escaped like [`SanitisedText`].
///
/// # Construction is closed
///
/// `RedactedText` from a `String` via `From` (positive control compiles):
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// `RedactedText` from a `String` via `From` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = RedactedText::from(String::from("x"));
/// ```
///
/// `RedactedText` from a `String` via `Into` (positive control compiles):
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// `RedactedText` from a `String` via `Into` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = String::from("x").into();
/// ```
///
/// `RedactedText` from a `&str` via `From` (positive control compiles):
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// `RedactedText` from a `&str` via `From` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = RedactedText::from("x");
/// ```
///
/// `RedactedText` by tuple-struct construction (positive control compiles):
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// `RedactedText` by tuple-struct construction (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = RedactedText(String::new());
/// ```
///
/// `RedactedText` by deserialising (positive control compiles):
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// `RedactedText` by deserialising (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _ = serde_json::from_str::<RedactedText>("\"x\"");
/// ```
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

/// Whether `c` belongs to the frozen escape set (`planning/P01-T01-request.md` §5.3).
fn needs_escape(c: char) -> bool {
    matches!(
        c,
        '\u{0000}'..='\u{0008}'
            | '\u{000B}'..='\u{001F}'
            | '\u{007F}'
            | '\u{0080}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200B}'..='\u{200D}'
            | '\u{2060}'
            | '\u{FEFF}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Escapes every code point of the frozen escape set as `<U+XXXX>` (uppercase hex, at least four digits).
///
/// TAB (U+0009) and LF (U+000A) are kept. Disclosed ambiguity: literal input text such as `<U+001B>` is
/// indistinguishable from an escaped ESC, which is what makes escaping idempotent.
fn escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if needs_escape(c) {
            out.push_str(&format!("<U+{:04X}>", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
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

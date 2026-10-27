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
/// Positive control for the next case (compiles) - `SanitisedText` from a `String` via `From`:
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// Compile-fail case: `SanitisedText` from a `String` via `From` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = SanitisedText::from(String::from("x"));
/// ```
///
/// Positive control for the next case (compiles) - `SanitisedText` from a `String` via `Into`:
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// Compile-fail case: `SanitisedText` from a `String` via `Into` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = String::from("x").into();
/// ```
///
/// Positive control for the next case (compiles) - A `String` passed where `SanitisedText` is required:
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// fn f(_: SanitisedText) {}
/// f(sanitise("x"));
/// ```
///
/// Compile-fail case: A `String` passed where `SanitisedText` is required (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// fn f(_: SanitisedText) {}
/// f(String::from("x"));
/// ```
///
/// Positive control for the next case (compiles) - `SanitisedText` by `Default`:
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// Compile-fail case: `SanitisedText` by `Default` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _: SanitisedText = SanitisedText::default();
/// ```
///
/// Positive control for the next case (compiles) - `SanitisedText` by `FromStr`:
///
/// ```
/// use domain::text::{sanitise, SanitisedText};
/// let _ = "x".parse::<String>();
/// let _: SanitisedText = sanitise("x");
/// ```
///
/// Compile-fail case: `SanitisedText` by `FromStr` (does not compile):
///
/// ```compile_fail
/// use domain::text::{sanitise, SanitisedText};
/// let _ = "x".parse::<SanitisedText>();
/// let _: SanitisedText = sanitise("x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SanitisedText(String);

/// Text that passed through a [`Redactor`] and was then escaped like [`SanitisedText`].
///
/// # Construction is closed
///
/// Positive control for the next case (compiles) - `RedactedText` from a `String` via `From`:
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
/// Compile-fail case: `RedactedText` from a `String` via `From` (does not compile):
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
/// Positive control for the next case (compiles) - `RedactedText` from a `String` via `Into`:
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
/// Compile-fail case: `RedactedText` from a `String` via `Into` (does not compile):
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
/// Positive control for the next case (compiles) - `RedactedText` from a `&str` via `From`:
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
/// Compile-fail case: `RedactedText` from a `&str` via `From` (does not compile):
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
/// Positive control for the next case (compiles) - `RedactedText` by tuple-struct construction:
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
/// Compile-fail case: `RedactedText` by tuple-struct construction (does not compile):
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
/// Positive control for the next case (compiles) - `RedactedText` by deserialising:
///
/// ```
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// let _ = serde_json::from_str::<String>("\"x\"");
/// ```
///
/// Compile-fail case: `RedactedText` by deserialising (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// let _ = serde_json::from_str::<RedactedText>("\"x\"");
/// ```
///
/// Positive control for the next case (compiles) - `RedactedText` from a `SanitisedText` via `From`:
///
/// ```
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact(sanitise("x").as_str(), &Id);
/// ```
///
/// Compile-fail case: `RedactedText` from a `SanitisedText` via `From` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = RedactedText::from(sanitise("x"));
/// ```
///
/// Positive control for the next case (compiles) - `RedactedText` from a `SanitisedText` via `Into`:
///
/// ```
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact(sanitise("x").as_str(), &Id);
/// ```
///
/// Compile-fail case: `RedactedText` from a `SanitisedText` via `Into` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = sanitise("x").into();
/// ```
///
/// Positive control for the next case (compiles) - `RedactedText` by `Default`:
///
/// ```
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// Compile-fail case: `RedactedText` by `Default` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _: RedactedText = RedactedText::default();
/// ```
///
/// Positive control for the next case (compiles) - `RedactedText` by `FromStr`:
///
/// ```
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _ = "x".parse::<String>();
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// Compile-fail case: `RedactedText` by `FromStr` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _ = "x".parse::<RedactedText>();
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// Positive control for the next case (compiles) - `RedactedText` by `TryFrom`:
///
/// ```
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _ = String::try_from(String::from("x"));
/// let _: RedactedText = redact("x", &Id);
/// ```
///
/// Compile-fail case: `RedactedText` by `TryFrom` (does not compile):
///
/// ```compile_fail
/// use domain::text::{redact, sanitise, RedactedText, Redactor};
/// use std::borrow::Cow;
/// struct Id;
/// impl Redactor for Id {
///     fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> { Cow::Borrowed(raw) }
/// }
/// let _ = RedactedText::try_from(String::from("x"));
/// let _: RedactedText = redact("x", &Id);
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

/// Builds a [`RedactedText`] from a compile-time literal, applying the same escaping as [`sanitise`].
///
/// Only for compile-time literal, application-authored text containing no secrets or user, child or
/// provider content. Never pass `format!`-built or runtime data: a `&'static str` can still be leaked
/// from a `String` with `Box::leak`, so this is a discipline rule backed by review.
pub(crate) fn redact_static(text: &'static str) -> RedactedText {
    RedactedText(escape(text))
}

/// Read-back of [`RedactedText`] from the application-owned JSON store (FR-31 (2), P02-T01 §5.1, §6.6).
///
/// `pub(crate)`: only `domain::store` models use these as `#[serde(deserialize_with = ...)]` helpers, so no
/// other crate can deserialise a bare `RedactedText`. Text is re-escaped on read (not re-redacted): the store
/// is trusted as application-owned (Architecture trust boundary 3).
#[allow(
    dead_code,
    reason = "P02 B0: first used by the evidence-side store models in wave 1b; removed by the lead at B1"
)]
pub(crate) mod store_read {
    use super::{RedactedText, escape};
    use serde::de::{Deserialize, Deserializer, Visitor};
    use std::fmt;

    /// A JSON string read without echoing a wrong-typed value in the error (P02-D02: `String::deserialize`
    /// makes serde_json report e.g. ``invalid type: integer `123` ``). `deserialize_any` routes numbers and
    /// booleans to the fixed-message methods below.
    struct Plain(String);

    struct PlainVisitor;

    impl Visitor<'_> for PlainVisitor {
        type Value = Plain;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Plain, E> {
            Ok(Plain(v.to_owned()))
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Plain, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Plain, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Plain, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Plain, E> {
            Err(E::custom("expected a string"))
        }
    }

    impl<'de> Deserialize<'de> for Plain {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            d.deserialize_any(PlainVisitor)
        }
    }

    fn wrap(p: Plain) -> RedactedText {
        RedactedText(escape(&p.0))
    }

    /// One `RedactedText` field.
    pub(crate) fn redacted<'de, D: Deserializer<'de>>(d: D) -> Result<RedactedText, D::Error> {
        Plain::deserialize(d).map(wrap)
    }

    /// One nullable `RedactedText` field.
    pub(crate) fn redacted_opt<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<RedactedText>, D::Error> {
        Ok(Option::<Plain>::deserialize(d)?.map(wrap))
    }

    /// An array of `RedactedText`.
    pub(crate) fn redacted_vec<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<RedactedText>, D::Error> {
        Ok(Vec::<Plain>::deserialize(d)?
            .into_iter()
            .map(wrap)
            .collect())
    }

    /// A string-valued map with validated keys (e.g. `RendererSetting.parameters_redacted`, P02-D01).
    /// Keys deserialise through their own type; values are re-escaped like every `RedactedText`.
    pub(crate) fn redacted_map<'de, D, K>(
        d: D,
    ) -> Result<std::collections::BTreeMap<K, RedactedText>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
    {
        let m = std::collections::BTreeMap::<K, Plain>::deserialize(d)?;
        Ok(m.into_iter().map(|(k, p)| (k, wrap(p))).collect())
    }
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

#[cfg(test)]
mod tests {
    use super::{Redactor, redact, redact_static};
    use std::borrow::Cow;

    struct Identity;

    impl Redactor for Identity {
        fn redact<'a>(&self, raw: &'a str) -> Cow<'a, str> {
            Cow::Borrowed(raw)
        }
    }

    #[test]
    fn redact_static_passes_literal_through_unchanged() {
        assert_eq!(
            redact_static("storage operation failed").as_str(),
            "storage operation failed"
        );
    }

    #[test]
    fn redact_static_escapes_control_characters() {
        assert_eq!(redact_static("a\u{1b}b").as_str(), "a<U+001B>b");
    }

    #[test]
    fn redact_static_equals_redact_with_identity() {
        for lit in ["plain", "a\u{1b}[31mb", "x\u{202e}y\u{2028}", "한국어", ""] {
            assert_eq!(redact_static(lit), redact(lit, &Identity));
        }
    }

    /// P02-D02: the store read-back helpers re-escape and never echo a wrong-typed value in their error.
    mod store_read_no_echo {
        use super::super::RedactedText;
        use super::super::store_read::{redacted, redacted_map, redacted_opt, redacted_vec};
        use std::collections::BTreeMap;

        #[derive(serde::Deserialize, Debug)]
        struct One {
            #[serde(deserialize_with = "redacted")]
            v: RedactedText,
        }
        #[derive(serde::Deserialize, Debug)]
        struct Opt {
            #[serde(deserialize_with = "redacted_opt")]
            v: Option<RedactedText>,
        }
        #[derive(serde::Deserialize, Debug)]
        struct Many {
            #[serde(deserialize_with = "redacted_vec")]
            v: Vec<RedactedText>,
        }
        #[derive(serde::Deserialize, Debug)]
        struct Map {
            #[serde(deserialize_with = "redacted_map")]
            v: BTreeMap<String, RedactedText>,
        }

        const MARKERS: [&str; 4] = ["987654321", "true", "4.25e3", "-77"];

        fn err_text<T: std::fmt::Debug>(r: Result<T, serde_json::Error>) -> String {
            r.expect_err("wrong-typed value must be rejected")
                .to_string()
        }

        /// Element-level values: a wrong-typed text value is rejected with fixed text. A wrong-typed
        /// *container* (a number where the array/map is expected) is reported by serde_json itself with the
        /// value quoted, for every derived type in the crate; the guard for that is the read boundary
        /// (`persistence::read` never forwards serde messages, `ProblemCode::TypedDecode`), tested in T08.
        #[test]
        fn wrong_typed_values_are_rejected_without_echo() {
            for m in MARKERS {
                let errs = [
                    err_text(serde_json::from_str::<One>(&format!(r#"{{"v": {m}}}"#))),
                    err_text(serde_json::from_str::<Opt>(&format!(r#"{{"v": {m}}}"#))),
                    err_text(serde_json::from_str::<Many>(&format!(
                        r#"{{"v": ["ok", {m}]}}"#
                    ))),
                    err_text(serde_json::from_str::<Map>(&format!(
                        r#"{{"v": {{"k": {m}}}}}"#
                    ))),
                ];
                for e in errs {
                    assert!(!e.contains(m), "error echoes {m}: {e}");
                }
            }
        }

        /// Records the boundary of the guarantee above: serde_json quotes a scalar given where a
        /// container is expected. If this ever stops holding, the T08 read-boundary test still applies.
        #[test]
        fn container_type_mismatch_is_rejected() {
            assert!(serde_json::from_str::<Many>(r#"{"v": 5}"#).is_err());
            assert!(serde_json::from_str::<Map>(r#"{"v": 5}"#).is_err());
        }

        #[test]
        fn strings_are_re_escaped_and_null_is_none() {
            let one: One = serde_json::from_str(r#"{"v": "a\u001bb"}"#).unwrap();
            assert_eq!(one.v.as_str(), "a<U+001B>b");
            let opt: Opt = serde_json::from_str(r#"{"v": null}"#).unwrap();
            assert!(opt.v.is_none());
            let opt: Opt = serde_json::from_str(r#"{"v": "x\u202ey"}"#).unwrap();
            assert_eq!(
                opt.v.map(|t| t.as_str().to_owned()).as_deref(),
                Some("x<U+202E>y")
            );
            let many: Many = serde_json::from_str(r#"{"v": ["\u0007", "ok"]}"#).unwrap();
            let got: Vec<&str> = many.v.iter().map(RedactedText::as_str).collect();
            assert_eq!(got, ["<U+0007>", "ok"]);
            let map: Map = serde_json::from_str(r#"{"v": {"k": "\u2028"}}"#).unwrap();
            assert_eq!(map.v.get("k").map(RedactedText::as_str), Some("<U+2028>"));
        }
    }
}

//! Validation results and the validator seam (frozen, `planning/P02-T01-request.md` §5.4).

use domain::store::StoreKind;
use domain::text::SanitisedText;

/// Maximum number of problems kept per document; further problems set [`Problems::truncated`].
pub const MAX_PROBLEMS: usize = 100;
/// Maximum byte length of a problem pointer after sanitising.
pub const MAX_POINTER_BYTES: usize = 256;

/// Where in the read pipeline a problem was found (request §6.1 gate order).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layer {
    Json,
    Version,
    Schema,
    Typed,
    Semantic,
}

/// Classification of an unsupported `schema_version`. The value itself is never kept.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VersionClass {
    /// `"1.0"`–`"1.3"`.
    Older,
    /// Any other value except `"1.4"`.
    NewerOrUnknown,
    /// No `schema_version` member.
    Missing,
}

/// The JSON Schema keyword that failed, mapped from the validator's error kind. Validator messages are
/// never propagated because they quote instance values.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SchemaKeyword {
    Type,
    Required,
    AdditionalProperties,
    Const,
    Enum,
    Format,
    Pattern,
    Minimum,
    MinLength,
    OneOf,
    AnyOf,
    Other,
}

/// Semantic rule identifier, e.g. `"D11-04"` or `"D18-06a"` (request §7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RuleId(pub &'static str);

/// What went wrong.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProblemCode {
    Syntax,
    TooLarge,
    NotObject,
    UnknownStoreType,
    UnsupportedVersion(VersionClass),
    StoreTypeMismatch,
    Keyword(SchemaKeyword),
    TypedDecode,
    Rule,
}

/// One validation problem. `pointer` is the sanitised JSON pointer of the instance location (at most
/// [`MAX_POINTER_BYTES`]); it never includes an instance value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problem {
    pub layer: Layer,
    pub code: ProblemCode,
    pub pointer: SanitisedText,
    pub rule: Option<RuleId>,
}

/// Problems of one document, capped at [`MAX_PROBLEMS`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Problems {
    pub items: Vec<Problem>,
    pub truncated: bool,
}

/// Outcome of validating one document. `SchemaUnavailable` is never treated as valid (FR-15).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Verdict {
    Valid,
    Invalid(Problems),
    SchemaUnavailable,
}

/// The validation seam used by the writer (production: `read::SchemaValidator`; tests: fakes).
pub trait DocumentValidator: Send + Sync + std::fmt::Debug {
    /// Validates `doc` as a document of `kind` (version gate, schema, typed decode, semantic rules).
    fn validate(&self, kind: StoreKind, doc: &serde_json::Value) -> Verdict;
}

//! Embedded, hash-pinned 1.4 schema and per-root validators (FR-15, FR-16, request §6.2).
//!
//! [`SchemaValidator`] is the production [`DocumentValidator`]. For one document it runs the layers of
//! request §6.1 in order: [`read::precheck`] (object, `store_type`, `schema_version`), the JSON Schema of
//! the root, [`read::typed_decode`] and [`semantic::check`]. A document is [`Verdict::Valid`] only when every
//! layer reports nothing; the first failing layer's problems are returned, because the later layers need
//! the earlier ones to have passed (the typed decode needs a schema-valid document, the semantic rules
//! need a typed one).
//!
//! # Schema handling
//!
//! * The canonical file is embedded with `include_str!`. At first use its SHA-256 is compared with
//!   [`SCHEMA_SHA256`]. A hash mismatch, a parse failure, a non-local `$ref` or a compile failure makes
//!   every verdict [`Verdict::SchemaUnavailable`]. That verdict is never [`Verdict::Valid`], and a writer
//!   that receives it refuses to write.
//! * One validator is compiled per root and kept (`OnceLock`). The document handed to the compiler is the
//!   embedded schema with the root `oneOf` replaced by `{"$ref": "#/$defs/<Root>File"}`, so a document is
//!   only ever checked against the root it was read as. The draft is forced to 2020-12 (the `$schema`
//!   member is not trusted to select it) and `format` is asserted (`uuid`, `date-time`).
//! * **No retrieval.** The `jsonschema` crate is built with `default-features = false`, so its HTTP and file
//!   resolvers are not compiled in. Three further barriers apply: a pre-compile scan rejects any `$ref`
//!   that does not start with `#`; the compiler is given a retriever that refuses every URI and counts the
//!   calls ([`SchemaValidator::retriever_calls`]); and the compiled schema is local by construction.
//!
//! # Read boundary (E-3)
//!
//! Validator messages, instance values and keyword parameters are never read, stored or returned: they
//! quote the document. A [`Problem`] carries the layer, a fieldless [`SchemaKeyword`] and a pointer. The
//! pointer is rebuilt segment by segment from the document and the schema together: an array index is
//! kept, a property name is kept only when the schema declares it (and the schema's own spelling is
//! used), and every other segment, such as the key of a string map or an undeclared key, becomes `*`.

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use domain::store::{AnyStore, StoreKind};
use jsonschema::error::ValidationErrorKind;
use jsonschema::{Draft, Retrieve, Uri, Validator};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::read;
use crate::semantic;
use crate::validate::{
    DocumentValidator, Layer, Problem, ProblemCode, Problems, SchemaKeyword, Verdict,
};

/// SHA-256 (lowercase hex) of `schemas/workbench-store-v1.4.schema.json`, pinned at P02-T01. A mismatch
/// of the embedded copy makes every verdict [`Verdict::SchemaUnavailable`].
pub const SCHEMA_SHA256: &str = "909e2a55c315ca07096f9123e6986b637a83b1bcdfc4eb6ff88d9d06daa80e2e";

/// The canonical schema file, embedded at build time.
const EMBEDDED_SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");

/// Number of store roots (one validator slot each).
const ROOTS: usize = StoreKind::ALL.len();

/// The embedded engine, shared by every [`SchemaValidator::new`] and compiled at most once per process.
static EMBEDDED: OnceLock<Engine> = OnceLock::new();

/// Production validator: [`crate::read::precheck`] → JSON Schema (embedded, pinned, per-root, formats
/// asserted, no remote or file retrieval) → [`crate::read::typed_decode`] → [`crate::semantic::check`].
#[derive(Default)]
pub struct SchemaValidator {
    source: Source,
}

/// Which schema a [`SchemaValidator`] works on.
#[derive(Default)]
enum Source {
    /// The embedded, pinned canonical file (shared, process-wide).
    #[default]
    Embedded,
    /// A caller-supplied schema (tests and diagnostics, [`SchemaValidator::with_schema`]).
    Custom(Box<Engine>),
}

impl SchemaValidator {
    /// A validator over the embedded schema. Compilation happens once, on first use.
    pub fn new() -> Self {
        Self::default()
    }

    /// A validator over `schema_json` instead of the embedded file, pinned to `sha256_hex` (lowercase hex).
    ///
    /// This is the seam for the pin-mismatch and external-`$ref` cases: a schema whose SHA-256 differs
    /// from `sha256_hex`, that does not parse, or that has a `$ref` leaving the document, gives
    /// [`Verdict::SchemaUnavailable`] for every document. The embedded validator ([`Self::new`]) never
    /// calls this.
    pub fn with_schema(schema_json: &'static str, sha256_hex: &str) -> Self {
        Self {
            source: Source::Custom(Box::new(Engine::new(schema_json, sha256_hex))),
        }
    }

    /// How many times the schema compiler asked the (refusing) retriever for a resource. The pre-compile
    /// scan stops external references first, so this is 0 for every schema this crate accepts or rejects
    /// up front; a non-zero value would mean something tried to leave the document.
    pub fn retriever_calls(&self) -> u64 {
        self.engine().calls.load(Ordering::Relaxed)
    }

    fn engine(&self) -> &Engine {
        match &self.source {
            Source::Embedded => {
                EMBEDDED.get_or_init(|| Engine::new(EMBEDDED_SCHEMA, SCHEMA_SHA256))
            }
            Source::Custom(engine) => engine,
        }
    }
}

impl fmt::Debug for SchemaValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match self.source {
            Source::Embedded => "embedded",
            Source::Custom(_) => "custom",
        };
        f.debug_struct("SchemaValidator")
            .field("source", &source)
            .finish_non_exhaustive()
    }
}

impl DocumentValidator for SchemaValidator {
    /// Runs the four layers. [`Verdict::SchemaUnavailable`] comes first and covers every document: a
    /// validator without a usable schema must not report anything else, in particular not `Valid`.
    fn validate(&self, kind: StoreKind, doc: &Value) -> Verdict {
        let engine = self.engine();
        let (Some(validator), Some(schema)) = (engine.validator(kind), engine.schema()) else {
            return Verdict::SchemaUnavailable;
        };
        if let Err(problems) = read::precheck(kind, doc) {
            return Verdict::Invalid(problems);
        }
        let problems = schema_problems(validator, schema, kind, doc);
        if !problems.is_empty() {
            return Verdict::Invalid(problems);
        }
        let typed: AnyStore = match read::typed_decode(kind, doc.clone()) {
            Ok(typed) => typed,
            Err(problems) => return Verdict::Invalid(problems),
        };
        let problems = semantic::check(&typed);
        if problems.is_empty() {
            Verdict::Valid
        } else {
            Verdict::Invalid(problems)
        }
    }
}

/// The compiled state of one schema: pin, parsed copy and one validator slot per root.
struct Engine {
    schema_json: &'static str,
    expected_sha256: String,
    /// The parsed schema once the pin, the parse and the reference scan have passed.
    parsed: OnceLock<Option<Value>>,
    /// One compiled validator per [`StoreKind`], in [`StoreKind::ALL`] order.
    roots: [OnceLock<Option<Validator>>; ROOTS],
    /// Calls received by the refusing retriever.
    calls: Arc<AtomicU64>,
}

impl Engine {
    fn new(schema_json: &'static str, expected_sha256: &str) -> Self {
        Self {
            schema_json,
            expected_sha256: expected_sha256.to_owned(),
            parsed: OnceLock::new(),
            roots: std::array::from_fn(|_| OnceLock::new()),
            calls: Arc::new(AtomicU64::new(0)),
        }
    }

    /// The parsed schema, or `None` when the pin does not match, the text does not parse or a reference
    /// leaves the document.
    fn schema(&self) -> Option<&Value> {
        self.parsed
            .get_or_init(|| {
                if sha256_hex(self.schema_json) != self.expected_sha256 {
                    return None;
                }
                let schema: Value = serde_json::from_str(self.schema_json).ok()?;
                refs_are_local(&schema).then_some(schema)
            })
            .as_ref()
    }

    /// The validator of `kind`, compiled on first use, or `None` when the schema is unusable.
    fn validator(&self, kind: StoreKind) -> Option<&Validator> {
        let slot = StoreKind::ALL.iter().position(|k| *k == kind)?;
        self.roots
            .get(slot)?
            .get_or_init(|| self.compile(kind))
            .as_ref()
    }

    fn compile(&self, kind: StoreKind) -> Option<Validator> {
        let mut root = self.schema()?.clone();
        let entry = root.as_object_mut()?;
        entry.remove("oneOf");
        entry.insert(
            "$ref".to_owned(),
            Value::String(format!("#/$defs/{}", root_def(kind))),
        );
        build(&root, &self.calls)
    }
}

/// Compiles `schema` with the fixed options: Draft 2020-12, formats asserted, unknown formats refused,
/// and a retriever that refuses everything and counts the calls.
fn build(schema: &Value, calls: &Arc<AtomicU64>) -> Option<Validator> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .with_retriever(RefuseAll {
            calls: Arc::clone(calls),
        })
        .build(schema)
        .ok()
}

/// A retriever that fetches nothing: every request is refused and counted.
struct RefuseAll {
    calls: Arc<AtomicU64>,
}

impl Retrieve for RefuseAll {
    fn retrieve(&self, _uri: &Uri<String>) -> Result<Value, Box<dyn Error + Send + Sync>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err("external schema retrieval is disabled".into())
    }
}

/// Lowercase hex SHA-256 of `text`.
fn sha256_hex(text: &str) -> String {
    use fmt::Write as _;
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            // Writing to a String cannot fail.
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Whether every string-valued `$ref`, `$dynamicRef` and `$recursiveRef` in `node` stays inside the
/// document (starts with `#`). Anything else would need retrieval.
fn refs_are_local(node: &Value) -> bool {
    match node {
        Value::Object(map) => map.iter().all(|(key, value)| {
            let reference = matches!(key.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef");
            match value {
                Value::String(target) if reference => target.starts_with('#'),
                _ => refs_are_local(value),
            }
        }),
        Value::Array(items) => items.iter().all(refs_are_local),
        _ => true,
    }
}

/// The `$defs` name of the root document of `kind`.
fn root_def(kind: StoreKind) -> &'static str {
    match kind {
        StoreKind::Config => "ConfigFile",
        StoreKind::Repositories => "RepositoryFile",
        StoreKind::Workflows => "WorkflowFile",
        StoreKind::Approvals => "ApprovalFile",
        StoreKind::Log => "LogFile",
        StoreKind::Suppressions => "SuppressionFile",
        StoreKind::RagIndex => "RagIndexFile",
        StoreKind::Conversations => "ConversationFile",
        StoreKind::Report => "ReportFile",
        StoreKind::ToolManifest => "ToolManifestFile",
    }
}

/// Schema-layer problems of `doc`, at most [`crate::validate::MAX_PROBLEMS`] (then `truncated`).
fn schema_problems(
    validator: &Validator,
    schema: &Value,
    kind: StoreKind,
    doc: &Value,
) -> Problems {
    let entry = schema.pointer(&format!("/$defs/{}", root_def(kind)));
    let mut problems = Problems::default();
    for error in validator.iter_errors(doc) {
        let keyword = keyword_of(error.kind());
        let pointer = match entry {
            Some(entry) => safe_pointer(schema, entry, doc, error.instance_path().as_str()),
            None => String::new(),
        };
        problems.push(Problem::new(
            Layer::Schema,
            ProblemCode::Keyword(keyword),
            &pointer,
            None,
        ));
        if problems.truncated {
            break;
        }
    }
    problems
}

/// Maps the validator's error kind to the fieldless keyword. Nothing inside `kind` is read.
fn keyword_of(kind: &ValidationErrorKind) -> SchemaKeyword {
    match kind {
        ValidationErrorKind::Type { .. } => SchemaKeyword::Type,
        ValidationErrorKind::Required { .. } => SchemaKeyword::Required,
        ValidationErrorKind::AdditionalProperties { .. } => SchemaKeyword::AdditionalProperties,
        ValidationErrorKind::Constant { .. } => SchemaKeyword::Const,
        ValidationErrorKind::Enum { .. } => SchemaKeyword::Enum,
        ValidationErrorKind::Format { .. } => SchemaKeyword::Format,
        ValidationErrorKind::Pattern { .. }
        | ValidationErrorKind::BacktrackLimitExceeded { .. }
        | ValidationErrorKind::RegexEngineFailure { .. } => SchemaKeyword::Pattern,
        ValidationErrorKind::Minimum { .. } => SchemaKeyword::Minimum,
        ValidationErrorKind::MinLength { .. } => SchemaKeyword::MinLength,
        ValidationErrorKind::OneOfNotValid { .. }
        | ValidationErrorKind::OneOfMultipleValid { .. } => SchemaKeyword::OneOf,
        ValidationErrorKind::AnyOf { .. } => SchemaKeyword::AnyOf,
        _ => SchemaKeyword::Other,
    }
}

/// Schema nodes that can describe the same instance as `node`: the node itself, plus (transitively) the
/// targets of its local `$ref` and the branches of `anyOf`/`oneOf`/`allOf`.
fn expand<'a>(root: &'a Value, node: &'a Value, out: &mut Vec<&'a Value>, depth: u8) {
    const MAX_DEPTH: u8 = 12;
    let Some(map) = node.as_object() else {
        return;
    };
    if depth < MAX_DEPTH {
        if let Some(target) = map
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.strip_prefix('#'))
            .and_then(|p| root.pointer(p))
        {
            expand(root, target, out, depth + 1);
        }
        for key in ["anyOf", "oneOf", "allOf"] {
            if let Some(Value::Array(branches)) = map.get(key) {
                for branch in branches {
                    expand(root, branch, out, depth + 1);
                }
            }
        }
    }
    out.push(node);
}

/// Undoes the RFC 6901 escapes of one pointer segment.
fn unescape(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

/// Rebuilds the instance location `location` (RFC 6901, as reported by the validator) so that it can only
/// contain array indices and property names the schema declares; every other segment is `*`.
///
/// `entry` is the schema of the root document and `doc` the document. The two are walked together, so a
/// string-map key that happens to look like a declared name is still `*` (the schema node at that depth has
/// no such property), and a key can never reach the output through the instance side.
fn safe_pointer(schema: &Value, entry: &Value, doc: &Value, location: &str) -> String {
    let mut out = String::new();
    let mut instance = Some(doc);
    let mut candidates: Vec<&Value> = Vec::new();
    expand(schema, entry, &mut candidates, 0);
    for raw in location.split('/').skip(1) {
        let segment = unescape(raw);
        out.push('/');
        let mut next: Vec<&Value> = Vec::new();
        match instance {
            Some(Value::Array(items)) => {
                match segment
                    .parse::<usize>()
                    .ok()
                    .filter(|i| i.to_string() == segment && *i < items.len())
                {
                    Some(index) => {
                        out.push_str(&index.to_string());
                        instance = items.get(index);
                        for item_schema in candidates.iter().filter_map(|c| c.get("items")) {
                            expand(schema, item_schema, &mut next, 0);
                        }
                    }
                    None => {
                        out.push('*');
                        instance = None;
                    }
                }
            }
            Some(Value::Object(map)) => {
                let declared: Vec<(&String, &Value)> = candidates
                    .iter()
                    .filter_map(|c| c.get("properties")?.as_object())
                    .filter_map(|props| props.get_key_value(segment.as_str()))
                    .collect();
                instance = map.get(segment.as_str());
                if let Some((name, _)) = declared.first() {
                    // The schema's own spelling, never the document's.
                    out.push_str(name);
                    for (_, property_schema) in &declared {
                        expand(schema, property_schema, &mut next, 0);
                    }
                } else {
                    out.push('*');
                    for map_schema in candidates
                        .iter()
                        .filter_map(|c| c.get("additionalProperties"))
                        .filter(|s| s.is_object())
                    {
                        expand(schema, map_schema, &mut next, 0);
                    }
                }
            }
            _ => {
                out.push('*');
                instance = None;
            }
        }
        candidates = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_refusing_retriever_counts_and_fails() {
        let calls = Arc::new(AtomicU64::new(0));
        let retriever = RefuseAll {
            calls: Arc::clone(&calls),
        };
        let uri = jsonschema::uri::from_str("https://example.invalid/x.json").expect("uri");
        assert!(retriever.retrieve(&uri).is_err());
        assert!(retriever.retrieve(&uri).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn the_compiler_reaches_the_retriever_for_an_external_reference() {
        // The pre-compile scan normally stops this first. Calling the compiler directly shows that the
        // second barrier is wired in: the reference is requested, refused and counted, and the schema
        // does not compile. A mis-wired retriever would leave the count at 0 (or fetch).
        let calls = Arc::new(AtomicU64::new(0));
        let schema = json!({"$ref": "https://example.invalid/x.json"});
        assert!(build(&schema, &calls).is_none());
        assert!(calls.load(Ordering::Relaxed) >= 1);
        let local = json!({"$defs": {"A": {"type": "string"}}, "$ref": "#/$defs/A"});
        let calls = Arc::new(AtomicU64::new(0));
        assert!(build(&local, &calls).is_some());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn local_reference_scan_accepts_fragments_and_rejects_everything_else() {
        assert!(refs_are_local(
            &json!({"$ref": "#/$defs/A", "x": [{"$ref": "#"}]})
        ));
        for bad in [
            "https://example.invalid/a.json",
            "file:///etc/passwd",
            "other.json",
            "./other.json#/x",
            "",
        ] {
            assert!(
                !refs_are_local(&json!({"a": [{"b": {"$ref": bad}}]})),
                "{bad:?}"
            );
            assert!(!refs_are_local(&json!({"$dynamicRef": bad})), "{bad:?}");
        }
        // A property that is merely called "$ref" holds a schema object, not a reference string.
        assert!(refs_are_local(
            &json!({"properties": {"$ref": {"type": "string"}}})
        ));
    }

    #[test]
    fn sha256_hex_matches_the_known_empty_input_digest() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn the_embedded_copy_has_the_pinned_hash() {
        assert_eq!(sha256_hex(EMBEDDED_SCHEMA), SCHEMA_SHA256);
    }

    #[test]
    fn safe_pointer_keeps_indices_and_declared_names_and_masks_the_rest() {
        let schema = json!({
            "$defs": {
                "Root": {"type": "object", "properties": {
                    "items": {"type": "array", "items": {"$ref": "#/$defs/Item"}},
                    "maybe": {"anyOf": [{"$ref": "#/$defs/Item"}, {"type": "null"}]}
                }},
                "Item": {"type": "object", "properties": {
                    "map": {"type": "object", "additionalProperties": {"type": "string"}},
                    "name": {"type": "string"}
                }}
            }
        });
        let entry = schema.pointer("/$defs/Root").expect("root def");
        let doc = json!({
            "items": [{"name": 1, "map": {"name": 2, "SECRETKEY": 3}}],
            "maybe": {"name": 4},
            "SECRETTOP": 5
        });
        let at = |location: &str| safe_pointer(&schema, entry, &doc, location);
        assert_eq!(at(""), "");
        assert_eq!(at("/items"), "/items");
        assert_eq!(at("/items/0/name"), "/items/0/name");
        // A map key is masked even when it spells a declared name elsewhere.
        assert_eq!(at("/items/0/map/name"), "/items/0/map/*");
        assert_eq!(at("/items/0/map/SECRETKEY"), "/items/0/map/*");
        assert_eq!(at("/SECRETTOP"), "/*");
        assert_eq!(at("/maybe/name"), "/maybe/name");
        // Out-of-range or non-canonical indices, and anything below an unknown node, are masked.
        assert_eq!(at("/items/7"), "/items/*");
        assert_eq!(at("/items/00"), "/items/*");
        assert_eq!(at("/SECRETTOP/a/b"), "/*/*/*");
        // RFC 6901 escapes are undone before the lookup.
        assert_eq!(at("/it~1ems"), "/*");
    }
}

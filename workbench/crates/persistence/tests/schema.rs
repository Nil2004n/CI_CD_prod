//! P02-T08 tests: the JSON Schema layer of `persistence::schema::SchemaValidator` and its place in the read
//! pipeline (request §5.4, §5.6, §5.11 E-3, §6.1, §6.2, §9 row T08).
//!
//! Fixtures are in `tests/fixtures/store/` (synthetic, `.invalid` hosts, fake IDs) with the expected verdict of
//! each file in `manifest.json`. Three of them are the Data Schema §8–§10 examples, verbatim.
//!
//! Every test says what would make it fail. Where a test compares against `jsonschema` directly, the reference
//! validator is built here from the schema text with the plain `oneOf` root, independently of the code under
//! test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::path::PathBuf;
use std::sync::OnceLock;

use domain::store::StoreKind;
use jsonschema::Draft;
use persistence::error::StoreError;
use persistence::read::{decode, precheck, typed_decode};
use persistence::schema::{SCHEMA_SHA256, SchemaValidator};
use persistence::validate::{
    DocumentValidator, Layer, MAX_PROBLEMS, Problem, ProblemCode, Problems, SchemaKeyword, Verdict,
    VersionClass,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The canonical schema file, read independently of the code under test.
const SCHEMA_TEXT: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");

/// A recognisable string planted into documents; it must never come back out.
const MARKER: &str = "PLANTEDMARKERq7Zx";
/// An integer planted into documents (as `424242424242` and as its negation).
const MARKER_INT: i64 = 424_242_424_242;
/// A float planted into documents.
const MARKER_FLOAT: f64 = 4242.4242;

// --- fixtures and helpers -------------------------------------------------------------------------------

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/store")
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(fixture_dir().join(name))
        .unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

fn fixture(name: &str) -> Value {
    serde_json::from_str(&fixture_text(name))
        .unwrap_or_else(|e| panic!("parse fixture {name}: {e}"))
}

/// The fixture `<store_type>.<variant>.json` of `kind`.
fn root_fixture(kind: StoreKind, variant: &str) -> Value {
    fixture(&format!("{}.{variant}.json", kind.store_type()))
}

#[derive(Debug)]
struct Entry {
    file: String,
    kind: StoreKind,
    expect: String,
}

fn manifest() -> Vec<Entry> {
    let manifest = fixture("manifest.json");
    manifest["fixtures"]
        .as_array()
        .expect("fixtures array")
        .iter()
        .map(|e| Entry {
            file: e["file"].as_str().expect("file").to_owned(),
            kind: StoreKind::from_store_type(e["kind"].as_str().expect("kind"))
                .expect("a known store type"),
            expect: e["expect"].as_str().expect("expect").to_owned(),
        })
        .collect()
}

/// The reference: the whole schema, root `oneOf` included, Draft 2020-12, formats asserted.
fn full_oneof() -> &'static jsonschema::Validator {
    static REFERENCE: OnceLock<jsonschema::Validator> = OnceLock::new();
    REFERENCE.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA_TEXT).expect("the schema parses");
        jsonschema::options()
            .with_draft(Draft::Draft202012)
            .should_validate_formats(true)
            .build(&schema)
            .expect("the schema compiles")
    })
}

/// A validator that accepts everything, to reach the typed layer of `decode` directly.
#[derive(Debug)]
struct AlwaysValid;

impl DocumentValidator for AlwaysValid {
    fn validate(&self, _kind: StoreKind, _doc: &Value) -> Verdict {
        Verdict::Valid
    }
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// One edit of a JSON document, by RFC 6901 pointer.
#[derive(Clone, Debug)]
enum Edit {
    /// Replace the value at the pointer (the empty pointer replaces the root).
    Set(&'static str, Value),
    /// Remove the object member at the pointer.
    Remove(&'static str),
    /// Add member `key` with a value to the object at the pointer.
    Add(&'static str, &'static str, Value),
}

fn apply(mut doc: Value, edit: &Edit) -> Value {
    match edit {
        Edit::Set(ptr, value) => {
            *doc.pointer_mut(ptr)
                .unwrap_or_else(|| panic!("no location {ptr}")) = value.clone();
        }
        Edit::Remove(ptr) => {
            let (parent, key) = ptr.rsplit_once('/').expect("a member pointer");
            let removed = doc
                .pointer_mut(parent)
                .unwrap_or_else(|| panic!("no location {parent}"))
                .as_object_mut()
                .expect("the parent is an object")
                .remove(key);
            assert!(removed.is_some(), "no member at {ptr}");
        }
        Edit::Add(ptr, key, value) => {
            doc.pointer_mut(ptr)
                .unwrap_or_else(|| panic!("no location {ptr}"))
                .as_object_mut()
                .expect("an object")
                .insert((*key).to_owned(), value.clone());
        }
    }
    doc
}

fn set(doc: Value, ptr: &'static str, value: Value) -> Value {
    apply(doc, &Edit::Set(ptr, value))
}

fn only_problem(problems: &Problems) -> &Problem {
    assert_eq!(problems.items.len(), 1, "{problems:?}");
    assert!(!problems.truncated, "{problems:?}");
    &problems.items[0]
}

fn invalid(verdict: Verdict) -> Problems {
    match verdict {
        Verdict::Invalid(problems) => problems,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

/// Asserts `verdict` is exactly one problem and returns `(layer, code, pointer)`.
fn sole(verdict: Verdict) -> (Layer, ProblemCode, String) {
    let problems = invalid(verdict);
    let p = only_problem(&problems);
    (p.layer, p.code, p.pointer.as_str().to_owned())
}

fn keyword_name(keyword: SchemaKeyword) -> &'static str {
    match keyword {
        SchemaKeyword::Type => "type",
        SchemaKeyword::Required => "required",
        SchemaKeyword::AdditionalProperties => "additionalProperties",
        SchemaKeyword::Const => "const",
        SchemaKeyword::Enum => "enum",
        SchemaKeyword::Format => "format",
        SchemaKeyword::Pattern => "pattern",
        SchemaKeyword::Minimum => "minimum",
        SchemaKeyword::MinLength => "minLength",
        SchemaKeyword::OneOf => "oneOf",
        SchemaKeyword::AnyOf => "anyOf",
        SchemaKeyword::Other => "other",
    }
}

/// The manifest vocabulary for a verdict: `valid`, `version:<class>`, `store_type_mismatch`,
/// `unknown_store_type`, `schema:<keyword>` (first problem), `typed`, `semantic` or `unavailable`.
fn classify(verdict: &Verdict) -> String {
    let Verdict::Invalid(problems) = verdict else {
        return match verdict {
            Verdict::Valid => "valid",
            _ => "unavailable",
        }
        .to_owned();
    };
    let first = &problems.items[0];
    match (first.layer, first.code) {
        (Layer::Version, ProblemCode::UnsupportedVersion(VersionClass::Older)) => {
            "version:older".to_owned()
        }
        (Layer::Version, ProblemCode::UnsupportedVersion(VersionClass::NewerOrUnknown)) => {
            "version:newer_or_unknown".to_owned()
        }
        (Layer::Version, ProblemCode::UnsupportedVersion(VersionClass::Missing)) => {
            "version:missing".to_owned()
        }
        (Layer::Version, ProblemCode::StoreTypeMismatch) => "store_type_mismatch".to_owned(),
        (Layer::Version, ProblemCode::UnknownStoreType) => "unknown_store_type".to_owned(),
        (Layer::Schema, ProblemCode::Keyword(k)) => format!("schema:{}", keyword_name(k)),
        (Layer::Typed, ProblemCode::TypedDecode) => "typed".to_owned(),
        (Layer::Semantic, ProblemCode::Rule) => "semantic".to_owned(),
        other => format!("unexpected:{other:?}"),
    }
}

// --- fixture manifest -----------------------------------------------------------------------------------

/// Fails when a fixture file is added without a manifest line, a manifest line names a missing file, a
/// file is listed twice, or a root loses its minimal or maximal fixture.
#[test]
fn the_manifest_lists_every_fixture_file_and_nothing_else() {
    let entries = manifest();
    let mut listed: Vec<&str> = entries.iter().map(|e| e.file.as_str()).collect();
    listed.sort_unstable();
    let before = listed.len();
    listed.dedup();
    assert_eq!(before, listed.len(), "a file is listed twice");

    let mut on_disk: Vec<String> = std::fs::read_dir(fixture_dir())
        .expect("fixture directory")
        .map(|e| {
            e.expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 file name")
        })
        .filter(|name| name != "manifest.json")
        .collect();
    on_disk.sort_unstable();
    assert_eq!(on_disk, listed, "files on disk differ from the manifest");
    for file in &listed {
        assert!(fixture_dir().join(file).is_file(), "{file} is not a file");
    }

    // The Data Schema examples and a minimal and a maximal valid fixture per root.
    let valid = |name: &str, kind: StoreKind| {
        entries
            .iter()
            .any(|e| e.file == name && e.kind == kind && e.expect == "valid")
    };
    assert!(valid("data-s8-repo.json", StoreKind::Repositories));
    assert!(valid("data-s9-workflows.json", StoreKind::Workflows));
    assert!(valid("data-s10-log.json", StoreKind::Log));
    for kind in StoreKind::ALL {
        for variant in ["min", "max"] {
            let name = format!("{}.{variant}.json", kind.store_type());
            assert!(
                valid(&name, kind),
                "{name} is not a valid fixture of {kind:?}"
            );
        }
    }
}

/// Fails when any fixture's verdict (through `validate` and through `decode`) differs from its manifest line.
#[test]
fn every_fixture_has_the_verdict_the_manifest_expects() {
    let validator = SchemaValidator::new();
    for e in manifest() {
        let doc = fixture(&e.file);
        let verdict = validator.validate(e.kind, &doc);
        assert_eq!(classify(&verdict), e.expect, "{}: {verdict:?}", e.file);
        let decoded = decode(e.kind, fixture_text(&e.file).as_bytes(), &validator);
        match (&verdict, decoded) {
            (Verdict::Valid, Ok(any)) => assert_eq!(any.kind(), e.kind, "{}", e.file),
            (Verdict::Invalid(expected), Err(StoreError::Invalid(kind, got))) => {
                assert_eq!(kind, e.kind, "{}", e.file);
                assert_eq!(&got, expected, "{}", e.file);
                if !e.expect.starts_with("schema:") {
                    // Version, store-type and typed faults are one problem each; schema faults may be several.
                    only_problem(&got);
                }
            }
            (v, d) => panic!("{}: validate {v:?} but decode {d:?}", e.file),
        }
    }
}

// --- J11 ------------------------------------------------------------------------------------------------

/// J11. Fails when a valid example does not pass the reference schema, the typed decode, the version gate
/// or `SchemaValidator` itself, or when `decode` returns a different kind.
#[test]
fn j11_every_valid_example_passes_schema_and_typed_decode() {
    let validator = SchemaValidator::new();
    let mut kinds: Vec<StoreKind> = Vec::new();
    for e in manifest().iter().filter(|e| e.expect == "valid") {
        let doc = fixture(&e.file);
        precheck(e.kind, &doc).unwrap_or_else(|p| panic!("{}: gates {p:?}", e.file));
        assert!(full_oneof().is_valid(&doc), "{}: reference schema", e.file);
        let typed = typed_decode(e.kind, doc.clone())
            .unwrap_or_else(|p| panic!("{}: typed decode {p:?}", e.file));
        assert_eq!(typed.kind(), e.kind, "{}", e.file);
        assert_eq!(
            validator.validate(e.kind, &doc),
            Verdict::Valid,
            "{}",
            e.file
        );
        let any = decode(e.kind, fixture_text(&e.file).as_bytes(), &validator)
            .unwrap_or_else(|err| panic!("{}: decode {err:?}", e.file));
        assert_eq!(any.kind(), e.kind, "{}", e.file);
        if !kinds.contains(&e.kind) {
            kinds.push(e.kind);
        }
    }
    assert_eq!(
        kinds.len(),
        StoreKind::ALL.len(),
        "a root has no valid example"
    );
}

/// Every valid fixture passes all layers, the semantic rules (Data Schema §11 and §18) included, and the
/// semantic problem list is empty for each. Fails, naming the fixture, rule and pointer of every problem, when
/// a fixture breaks a rule; a future rule that a fixture violates shows up here first.
///
/// Where a `.max` fixture could not be maximal in the schema sense because a rule forbids the combination
/// (the rule is in brackets): `config` has chat persistence off and caps of 1 024 repositories and 64 scanner
/// processes (D11-24, D11-25); `workflows` workflow 1 is an `interval` schedule, so `daily_local_time` is
/// null, and its `patterns` selector has no `paths` (D11-09, D11-07), and a third workflow carries the
/// `daily` schedule and an `explicit` selector with paths; `approvals` record 1 is a consumed `once` grant
/// (D11-13); `rag_index` chunk 1 has no `embedding_key` (D11-24); `tool_manifest` tool 1 sits at the caps of
/// 86 400 s and 64 MiB (D18-01); `report` check 1 has a `term_signal` and no `exit_code`, and a `completed`
/// enrichment has no `reason_redacted` (D18-04, D18-12). The `report.min` fixture needs one plan target
/// (D11-08).
#[test]
fn every_valid_fixture_passes_the_semantic_rules_too() {
    let validator = SchemaValidator::new();
    let mut checked = 0_usize;
    let mut failures: Vec<String> = Vec::new();
    for e in manifest().iter().filter(|e| e.expect == "valid") {
        let doc = fixture(&e.file);
        let typed = typed_decode(e.kind, doc.clone())
            .unwrap_or_else(|p| panic!("{}: typed decode {p:?}", e.file));
        let semantic = persistence::semantic::check(&typed);
        for p in &semantic.items {
            failures.push(format!(
                "{}: rule {:?} at {:?} ({:?}/{:?})",
                e.file, p.rule, p.pointer, p.layer, p.code
            ));
        }
        if semantic.truncated {
            failures.push(format!("{}: semantic problem list truncated", e.file));
        }
        let verdict = validator.validate(e.kind, &doc);
        if verdict != Verdict::Valid {
            failures.push(format!("{}: SchemaValidator verdict {verdict:?}", e.file));
        }
        checked += 1;
    }
    assert!(
        failures.is_empty(),
        "valid fixtures break rules:\n{}",
        failures.join("\n")
    );
    assert!(
        checked >= 2 * StoreKind::ALL.len(),
        "min and max of every root are checked"
    );
}

// --- J12 / J13 ------------------------------------------------------------------------------------------

/// J12. Fails when `1.3`, `1.2`, `9.9`, a missing member or a non-string version is classified wrongly, or
/// when the problem is not exactly one `Version` problem at `/schema_version`, for any root.
#[test]
fn j12_schema_versions_are_classified_for_every_root() {
    let validator = SchemaValidator::new();
    for kind in StoreKind::ALL {
        let base = root_fixture(kind, "min");
        let cases: Vec<(Option<Value>, VersionClass)> = vec![
            (Some(json!("1.3")), VersionClass::Older),
            (Some(json!("1.2")), VersionClass::Older),
            (Some(json!("1.1")), VersionClass::Older),
            (Some(json!("1.0")), VersionClass::Older),
            (Some(json!("9.9")), VersionClass::NewerOrUnknown),
            (Some(json!("1.5")), VersionClass::NewerOrUnknown),
            (Some(json!("2.0")), VersionClass::NewerOrUnknown),
            (Some(json!(1.4)), VersionClass::NewerOrUnknown),
            (Some(Value::Null), VersionClass::NewerOrUnknown),
            (None, VersionClass::Missing),
        ];
        for (value, class) in cases {
            let doc = match &value {
                Some(v) => set(base.clone(), "/schema_version", v.clone()),
                None => apply(base.clone(), &Edit::Remove("/schema_version")),
            };
            let (layer, code, pointer) = sole(validator.validate(kind, &doc));
            assert_eq!(
                (layer, code, pointer.as_str()),
                (
                    Layer::Version,
                    ProblemCode::UnsupportedVersion(class),
                    "/schema_version"
                ),
                "{kind:?} {value:?}"
            );
            // The same through decode.
            let bytes = serde_json::to_vec(&doc).expect("serialise");
            match decode(kind, &bytes, &validator) {
                Err(StoreError::Invalid(k, p)) => {
                    assert_eq!(k, kind);
                    assert_eq!(
                        only_problem(&p).code,
                        ProblemCode::UnsupportedVersion(class)
                    );
                }
                other => panic!("{kind:?} {value:?}: {other:?}"),
            }
        }
    }
}

/// J13. Fails when a document of one root validated as another root is not exactly one
/// `StoreTypeMismatch` at `/store_type`, for any of the 90 ordered pairs; or when an unknown, mistyped or
/// missing `store_type` is not `UnknownStoreType`; or when `store_type` is not checked before the version.
#[test]
fn j13_store_type_mismatch_for_every_ordered_pair() {
    let validator = SchemaValidator::new();
    let mut pairs = 0;
    for kind in StoreKind::ALL {
        for other in StoreKind::ALL.into_iter().filter(|o| *o != kind) {
            let doc = root_fixture(other, "min");
            let got = sole(validator.validate(kind, &doc));
            assert_eq!(
                got,
                (
                    Layer::Version,
                    ProblemCode::StoreTypeMismatch,
                    "/store_type".to_owned()
                ),
                "{other:?} as {kind:?}"
            );
            pairs += 1;
        }
        let base = root_fixture(kind, "min");
        for bad in [json!("bogus"), json!(7), Value::Null, json!(["log"])] {
            let doc = set(base.clone(), "/store_type", bad.clone());
            let got = sole(validator.validate(kind, &doc));
            assert_eq!(
                got,
                (
                    Layer::Version,
                    ProblemCode::UnknownStoreType,
                    "/store_type".to_owned()
                ),
                "{kind:?} {bad}"
            );
        }
        let missing = apply(base.clone(), &Edit::Remove("/store_type"));
        assert_eq!(
            sole(validator.validate(kind, &missing)).1,
            ProblemCode::UnknownStoreType
        );
    }
    assert_eq!(pairs, 90);

    // store_type first: a log document that is also labelled 1.3 and read as a config is a mismatch.
    let doc = set(
        root_fixture(StoreKind::Log, "min"),
        "/schema_version",
        json!("1.3"),
    );
    assert_eq!(
        sole(validator.validate(StoreKind::Config, &doc)).1,
        ProblemCode::StoreTypeMismatch
    );
}

/// The per-root verdict differs from the plain `oneOf` root exactly for a document read as the wrong root.
/// Fails if the validator accepts a log document as a config because the `oneOf` would.
#[test]
fn a_per_root_validator_is_stricter_than_the_one_of_root() {
    let log = root_fixture(StoreKind::Log, "max");
    assert!(full_oneof().is_valid(&log), "the oneOf accepts any root");
    assert_ne!(
        SchemaValidator::new().validate(StoreKind::Config, &log),
        Verdict::Valid
    );
}

// --- format ---------------------------------------------------------------------------------------------

/// Fails when a non-UUID in a `uuid` field or an invalid `date-time` is not exactly one
/// `Layer::Schema` / `Keyword(Format)` problem at the right location, or when the unmodified fixture is not
/// valid (the positive control).
#[test]
fn format_negatives_fail_at_the_schema_layer() {
    let validator = SchemaValidator::new();
    let base = root_fixture(StoreKind::Repositories, "max");
    assert_eq!(
        validator.validate(StoreKind::Repositories, &base),
        Verdict::Valid
    );
    let cases: [(&'static str, Value); 8] = [
        ("/repositories/1/repo_id", json!("not-a-uuid")),
        ("/repositories/1/repo_id", json!("")),
        (
            "/repositories/1/repo_id",
            json!("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6"),
        ),
        ("/repositories/1/files/0/file_id", json!("1234")),
        ("/updated_at", json!("not-a-time")),
        ("/updated_at", json!("2026-10-16")),
        (
            "/repositories/0/registered_at",
            json!("2026-13-45T25:61:61Z"),
        ),
        ("/repositories/1/last_update_at", json!("yesterday")),
    ];
    for (ptr, bad) in cases {
        let doc = set(base.clone(), ptr, bad.clone());
        let (layer, code, pointer) = sole(validator.validate(StoreKind::Repositories, &doc));
        // The nullable timestamp sits under an `anyOf`, whose own keyword is reported there.
        let expected = if ptr.ends_with("last_update_at") {
            ProblemCode::Keyword(SchemaKeyword::AnyOf)
        } else {
            ProblemCode::Keyword(SchemaKeyword::Format)
        };
        assert_eq!(
            (layer, code, pointer.as_str()),
            (Layer::Schema, expected, ptr)
        );
        assert!(
            !full_oneof().is_valid(&doc),
            "{ptr} {bad}: reference agrees"
        );
    }
}

/// Fails when an uppercase UUID or a non-`Z` offset is rejected by `format` (they are valid RFC 4122 and
/// RFC 3339), or when the typed layer, which is stricter, accepts them.
#[test]
fn uppercase_uuid_and_non_z_offset_pass_format_and_fail_at_the_typed_layer() {
    let validator = SchemaValidator::new();
    let base = root_fixture(StoreKind::Repositories, "max");
    let cases = [
        (
            "/repositories/1/repo_id",
            json!("0190A3B4-5C6D-7E8F-9A0B-1C2D3E4F5A6B"),
        ),
        ("/updated_at", json!("2026-10-16T07:25:00+02:00")),
        ("/updated_at", json!("2026-10-16T05:25:00-00:00")),
    ];
    for (ptr, value) in cases {
        let doc = set(base.clone(), ptr, value.clone());
        assert!(
            full_oneof().is_valid(&doc),
            "{ptr} {value}: the schema (format) must accept it"
        );
        let got = sole(validator.validate(StoreKind::Repositories, &doc));
        assert_eq!(
            got,
            (Layer::Typed, ProblemCode::TypedDecode, String::new()),
            "{ptr} {value}"
        );
    }
}

// --- wave-1a/1b carry-over ------------------------------------------------------------------------------

/// D02-6. Fails when a required nullable member that is absent (not `null`) passes the schema layer.
#[test]
fn an_absent_required_nullable_key_fails_at_the_schema_layer() {
    let validator = SchemaValidator::new();
    let cases: [(&str, &'static str, &str); 4] = [
        (
            "repositories.max.json",
            "/repositories/0/last_update_at",
            "/repositories/0",
        ),
        (
            "approvals.max.json",
            "/approvals/0/workflow_id",
            "/approvals/0",
        ),
        (
            "config.max.json",
            "/config/history_retention_days",
            "/config",
        ),
        (
            "tool_manifest.max.json",
            "/tools/0/license/spdx",
            "/tools/0/license",
        ),
    ];
    for (file, ptr, location) in cases {
        let base = fixture(file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        assert_eq!(validator.validate(kind, &base), Verdict::Valid, "{file}");
        let doc = apply(base.clone(), &Edit::Remove(ptr));
        let got = sole(validator.validate(kind, &doc));
        assert_eq!(
            got,
            (
                Layer::Schema,
                ProblemCode::Keyword(SchemaKeyword::Required),
                location.to_owned()
            ),
            "{file} {ptr}"
        );
        // The same member present and null is fine.
        let (parent, key) = ptr.rsplit_once('/').expect("a member pointer");
        assert_eq!(
            validator.validate(kind, &apply(doc, &Edit::Add(parent, key, Value::Null))),
            Verdict::Valid,
            "{file}: null is accepted"
        );
    }
}

/// One numeric lower bound of the schema: the edit that violates it, the keyword that reports it and
/// whether the typed layer (`typed_decode` alone) also rejects the same document.
struct Bound {
    file: &'static str,
    ptr: &'static str,
    bad: Value,
    keyword: SchemaKeyword,
    typed_also_rejects: bool,
}

#[rustfmt::skip]
fn bounds() -> Vec<Bound> {
    let b = |file, ptr, bad, keyword, typed_also_rejects| Bound {
        file,
        ptr,
        bad,
        keyword,
        typed_also_rejects,
    };
    let min = SchemaKeyword::Minimum;
    let any = SchemaKeyword::AnyOf;
    let mut v = vec![
        b("workflows.max.json", "/workflows/1/revision", json!(0), min, false),
        b("workflows.max.json", "/workflows/1/schedule/interval_seconds", json!(0), any, false),
        b("approvals.max.json", "/approvals/1/workflow_revision", json!(0), any, false),
        b("repositories.max.json", "/repositories/1/inventory_revision", json!(-1), min, true),
        b("repositories.max.json", "/repositories/1/files/0/byte_size", json!(-1), min, true),
        b("config.max.json", "/config/max_active_repositories", json!(0), min, false),
        b("config.max.json", "/config/max_scanner_processes", json!(0), min, false),
        b("config.max.json", "/config/history_retention_days", json!(0), any, false),
        b("tool_manifest.max.json", "/tools/1/timeout_seconds", json!(0), min, false),
        b("tool_manifest.max.json", "/tools/1/max_output_bytes", json!(0), min, false),
        b("conversations.max.json", "/conversations/1/messages/1/tokens_in", json!(-1), any, true),
        b("conversations.max.json", "/conversations/1/messages/1/tokens_out", json!(-1), any, true),
        b("conversations.max.json", "/conversations/1/messages/1/source_refs/0/document_index", json!(0), any, false),
        b("conversations.max.json", "/conversations/1/messages/1/source_refs/0/line_start", json!(0), any, false),
        b("conversations.max.json", "/conversations/1/messages/1/source_refs/0/line_end", json!(0), any, false),
        b("rag_index.max.json", "/chunks/1/source/line_start", json!(0), any, false),
        b("report.max.json", "/checks/1/term_signal", json!(0), any, false),
        b("report.max.json", "/plan/workflow_revision", json!(0), any, false),
        b("report.max.json", "/plan/target_snapshots/0/inventory_revision", json!(-1), min, true),
        b("log.max.json", "/runs/1/plan/workflow_revision", json!(0), any, false),
        b("log.max.json", "/runs/1/plan/target_snapshots/0/inventory_revision", json!(-1), min, true),
    ];
    for counter in [
        "discovered_files",
        "selected_files",
        "assessed_files",
        "excluded_files",
        "missing_files",
        "checks_passed",
        "checks_with_findings",
        "checks_failed",
        "checks_unsupported",
        "checks_skipped",
        "checks_incomplete",
        "checks_cancelled",
    ] {
        v.push(b(
            "report.max.json",
            leak(format!("/coverage/{counter}")),
            json!(-1),
            min,
            true,
        ));
    }
    v
}

/// Schema `minimum`s (wave-1b report): each violation is a single `Layer::Schema` problem at the field.
/// Fails when the schema layer lets one through. `typed_also_rejects` records whether `typed_decode` alone
/// would have caught it: where it is `false`, this layer is the only one enforcing the bound (the typed
/// field is a plain integer type), and a failing assertion means the typed layer changed.
#[test]
fn schema_minimums_are_enforced_by_the_schema_layer() {
    let validator = SchemaValidator::new();
    for bound in bounds() {
        let base = fixture(bound.file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        assert_eq!(
            validator.validate(kind, &base),
            Verdict::Valid,
            "{}: positive control",
            bound.file
        );
        let doc = set(base, bound.ptr, bound.bad.clone());
        let got = sole(validator.validate(kind, &doc));
        assert_eq!(
            got,
            (
                Layer::Schema,
                ProblemCode::Keyword(bound.keyword),
                bound.ptr.to_owned()
            ),
            "{} {}",
            bound.file,
            bound.ptr
        );
        assert!(
            !full_oneof().is_valid(&doc),
            "{}: reference agrees",
            bound.ptr
        );
        assert_eq!(
            typed_decode(kind, doc).is_err(),
            bound.typed_also_rejects,
            "{} {}: typed layer behaviour changed",
            bound.file,
            bound.ptr
        );
    }
}

/// The boundary values themselves are accepted (a minimum of 1 admits 1, a minimum of 0 admits 0, and a
/// value the semantic rules cap is accepted at its cap), so the bounds above are not an over-tight check.
/// Fails when `1`/`0` or a cap value is rejected by any layer.
///
/// Two fixture values are bounded by semantic rules, not by the schema: the workflow `interval_seconds` is
/// only valid for an `interval` schedule (`workflows.max.json` workflow 1 is one; D11-09), and the tool
/// limits are capped at 86 400 s and 64 MiB (D18-01). The next test pins that the schema layer alone has no
/// such cap.
#[test]
fn the_boundary_values_are_accepted() {
    let validator = SchemaValidator::new();
    let cases: [(&str, &'static str, Value); 7] = [
        ("workflows.max.json", "/workflows/1/revision", json!(1)),
        (
            "workflows.max.json",
            "/workflows/1/schedule/interval_seconds",
            json!(1),
        ),
        ("config.max.json", "/config/max_scanner_processes", json!(1)),
        (
            "config.max.json",
            "/config/max_scanner_processes",
            json!(64),
        ),
        (
            "config.max.json",
            "/config/max_active_repositories",
            json!(1024),
        ),
        (
            "repositories.max.json",
            "/repositories/1/inventory_revision",
            json!(0),
        ),
        (
            "tool_manifest.max.json",
            "/tools/1/timeout_seconds",
            json!(86_400),
        ),
    ];
    for (file, ptr, ok) in cases {
        let base = fixture(file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        let doc = set(base, ptr, ok.clone());
        assert_eq!(
            validator.validate(kind, &doc),
            Verdict::Valid,
            "{file} {ptr} {ok}"
        );
    }
    // A passed-check counter of 0: D11-21 ties the counters to the checks, so the one passed check of the
    // fixture becomes a skipped one (with its counter) in the same document.
    let doc = [
        ("/coverage/checks_passed", json!(0)),
        ("/coverage/checks_skipped", json!(1)),
        ("/checks/0/outcome", json!("skipped")),
    ]
    .into_iter()
    .fold(fixture("report.max.json"), |doc, (ptr, v)| set(doc, ptr, v));
    assert_eq!(
        validator.validate(StoreKind::Report, &doc),
        Verdict::Valid,
        "report.max.json /coverage/checks_passed 0"
    );
}

/// The schema has no upper bound on these integers: the largest `u64` passes the reference schema and the
/// typed decode, and it is the semantic layer (D11-25 and D18-01 caps) that rejects it, once, at the field.
/// Fails when the schema or typed layer starts rejecting the value, or when the semantic layer stops capping
/// the field or reports anything else.
#[test]
fn values_the_schema_admits_and_a_semantic_cap_rejects() {
    let validator = SchemaValidator::new();
    let cases: [(&str, &'static str, &'static str); 4] = [
        (
            "tool_manifest.max.json",
            "/tools/1/timeout_seconds",
            "D18-01",
        ),
        (
            "tool_manifest.max.json",
            "/tools/1/max_output_bytes",
            "D18-01",
        ),
        (
            "config.max.json",
            "/config/max_active_repositories",
            "D11-25",
        ),
        ("config.max.json", "/config/max_scanner_processes", "D11-25"),
    ];
    for (file, ptr, rule) in cases {
        let base = fixture(file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        let doc = set(base, ptr, json!(u64::MAX));
        assert!(
            full_oneof().is_valid(&doc),
            "{file} {ptr}: reference schema"
        );
        typed_decode(kind, doc.clone()).unwrap_or_else(|p| panic!("{file} {ptr}: typed {p:?}"));
        let problems = invalid(validator.validate(kind, &doc));
        let problem = only_problem(&problems);
        assert_eq!(
            (problem.layer, problem.code, problem.pointer.as_str()),
            (Layer::Semantic, ProblemCode::Rule, ptr),
            "{file} {ptr}"
        );
        assert_eq!(
            problem.rule.as_ref().map(|r| r.0),
            Some(rule),
            "{file} {ptr}: rule"
        );
    }
}

/// One failing edit of a valid fixture, the keyword that must report it and where.
struct Case {
    file: &'static str,
    edit: Edit,
    keyword: SchemaKeyword,
    pointer: &'static str,
}

fn case(file: &'static str, edit: Edit, keyword: SchemaKeyword, pointer: &'static str) -> Case {
    Case {
        file,
        edit,
        keyword,
        pointer,
    }
}

#[rustfmt::skip]
fn keyword_cases() -> Vec<Case> {
    vec![
        case("repositories.max.json", Edit::Set("/repositories/1/display_name", json!(5)), SchemaKeyword::Type, "/repositories/1/display_name"),
        case("repositories.max.json", Edit::Set("/repositories/1/files", json!("x")), SchemaKeyword::Type, "/repositories/1/files"),
        case("repositories.max.json", Edit::Set("/repositories", json!({})), SchemaKeyword::Type, "/repositories"),
        case("repositories.max.json", Edit::Remove("/repositories/1/slug"), SchemaKeyword::Required, "/repositories/1"),
        case("repositories.max.json", Edit::Add("/repositories/1", "zz_unknown", json!(1)), SchemaKeyword::AdditionalProperties, "/repositories/1"),
        case("repositories.max.json", Edit::Add("", "zz_unknown", json!(1)), SchemaKeyword::AdditionalProperties, ""),
        case("config.max.json", Edit::Set("/config/telemetry_enabled", json!(true)), SchemaKeyword::Const, "/config/telemetry_enabled"),
        case("config.max.json", Edit::Set("/config/provider/endpoint", json!("https://example.invalid/v1")), SchemaKeyword::AnyOf, "/config/provider"),
        case("config.max.json", Edit::Set("/config/provider/routing_policy/zdr", json!(false)), SchemaKeyword::AnyOf, "/config/provider"),
        case("config.max.json", Edit::Set("/config/provider/routing_policy/allow_fallbacks", json!(true)), SchemaKeyword::AnyOf, "/config/provider"),
        case("repositories.max.json", Edit::Set("/repositories/1/checkout_state", json!("cloned")), SchemaKeyword::Enum, "/repositories/1/checkout_state"),
        case("approvals.max.json", Edit::Set("/approvals/1/data_categories/0", json!("raw_secrets")), SchemaKeyword::Enum, "/approvals/1/data_categories/0"),
        case("report.max.json", Edit::Set("/plan/plan_digest", json!("ABCDEF")), SchemaKeyword::Pattern, "/plan/plan_digest"),
        case("report.max.json", Edit::Set("/plan/plan_digest", json!("0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef")), SchemaKeyword::Pattern, "/plan/plan_digest"),
        case("repositories.max.json", Edit::Set("/repositories/1/files/0/commit_oid", json!("xyz")), SchemaKeyword::Pattern, "/repositories/1/files/0/commit_oid"),
        case("repositories.max.json", Edit::Set("/repositories/1/files/0/path", json!("")), SchemaKeyword::MinLength, "/repositories/1/files/0/path"),
        case("config.max.json", Edit::Set("/config/provider/model_id", json!("")), SchemaKeyword::AnyOf, "/config/provider"),
        case("workflows.max.json", Edit::Set("/workflows/1/output_formats", json!([])), SchemaKeyword::Other, "/workflows/1/output_formats"),
        case("workflows.max.json", Edit::Set("/workflows/1/output_formats", json!(["json", "json"])), SchemaKeyword::Other, "/workflows/1/output_formats"),
        case("repositories.max.json", Edit::Set("/repositories/1/last_update_at", json!(5)), SchemaKeyword::AnyOf, "/repositories/1/last_update_at"),
        case("repositories.max.json", Edit::Set("/repositories/1/files/0/content_digest", json!("nothex")), SchemaKeyword::AnyOf, "/repositories/1/files/0/content_digest"),
        case("repositories.max.json", Edit::Set("/repositories/1/repo_id", json!("not-a-uuid")), SchemaKeyword::Format, "/repositories/1/repo_id"),
    ]
}

/// One failing case per keyword the schema uses, with the exact location. Fails when a keyword is not
/// enforced (no problem), is reported as another keyword, or points elsewhere. `Other` stands for the
/// keywords without a dedicated variant (`minItems`, `uniqueItems`).
#[test]
fn every_keyword_the_schema_uses_is_enforced_with_its_location() {
    let validator = SchemaValidator::new();
    for c in keyword_cases() {
        let base = fixture(c.file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        assert_eq!(
            validator.validate(kind, &base),
            Verdict::Valid,
            "{}",
            c.file
        );
        let doc = apply(base, &c.edit);
        let got = sole(validator.validate(kind, &doc));
        assert_eq!(
            got,
            (
                Layer::Schema,
                ProblemCode::Keyword(c.keyword),
                c.pointer.to_owned()
            ),
            "{} {:?}",
            c.file,
            c.edit
        );
        assert!(
            !full_oneof().is_valid(&doc),
            "{:?}: reference agrees",
            c.edit
        );
    }
}

/// When the schema layer reports problems the typed layer is not consulted, and when only the typed layer
/// objects the answer is one `TypedDecode` at the root. Fails if a typed problem is mixed into a schema
/// verdict, or a typed-only fault reports a different layer or location.
#[test]
fn layers_report_in_order_and_do_not_mix() {
    let validator = SchemaValidator::new();
    let base = root_fixture(StoreKind::Repositories, "max");
    let typed_only = set(base.clone(), "/repositories/1/slug", json!("Not A Slug"));
    assert_eq!(
        sole(validator.validate(StoreKind::Repositories, &typed_only)),
        (Layer::Typed, ProblemCode::TypedDecode, String::new())
    );
    let both = apply(
        typed_only,
        &Edit::Add("/repositories/1", "zz_unknown", json!(1)),
    );
    let problems = invalid(validator.validate(StoreKind::Repositories, &both));
    assert!(
        problems.items.iter().all(|p| p.layer == Layer::Schema),
        "{problems:?}"
    );
    assert_eq!(problems.items.len(), 1);
}

// --- SchemaUnavailable ----------------------------------------------------------------------------------

/// Fails when the supplied-schema seam does not behave like the embedded schema for the same text and the
/// pinned hash. This is the positive control for the unavailability tests: without it they could pass because
/// `with_schema` never works.
#[test]
fn with_schema_over_the_pinned_text_validates_like_the_embedded_schema() {
    let custom = SchemaValidator::with_schema(SCHEMA_TEXT, SCHEMA_SHA256);
    for e in manifest() {
        let doc = fixture(&e.file);
        assert_eq!(
            custom.validate(e.kind, &doc),
            SchemaValidator::new().validate(e.kind, &doc),
            "{}",
            e.file
        );
    }
    assert_eq!(custom.retriever_calls(), 0);
}

/// The pinned constant is the SHA-256 of the canonical file. Fails if either changes without the other.
#[test]
fn the_pin_is_the_hash_of_the_canonical_schema_file() {
    assert_eq!(sha256_hex(SCHEMA_TEXT), SCHEMA_SHA256);
}

/// Fails when a schema whose hash differs from the pin (wrong pin, one changed character, unparsable text,
/// a pin of the wrong shape) ever yields a verdict other than `SchemaUnavailable`, for any root, for a valid
/// and an invalid document alike.
#[test]
fn a_pin_mismatch_makes_every_verdict_schema_unavailable() {
    let tampered = leak(SCHEMA_TEXT.replacen("\"const\": \"1.4\"", "\"const\": \"1.5\"", 1));
    assert_ne!(tampered, SCHEMA_TEXT, "the tamper must change the text");
    let broken = leak("{ \"$defs\": ".to_owned());
    let variants: Vec<(&str, SchemaValidator)> = vec![
        (
            "wrong pin",
            SchemaValidator::with_schema(SCHEMA_TEXT, &"0".repeat(64)),
        ),
        ("empty pin", SchemaValidator::with_schema(SCHEMA_TEXT, "")),
        (
            "uppercase pin",
            SchemaValidator::with_schema(SCHEMA_TEXT, &SCHEMA_SHA256.to_ascii_uppercase()),
        ),
        (
            "tampered text, pinned hash",
            SchemaValidator::with_schema(tampered, SCHEMA_SHA256),
        ),
        (
            "unparsable text with its own hash",
            SchemaValidator::with_schema(broken, &sha256_hex(broken)),
        ),
    ];
    for (label, validator) in &variants {
        for kind in StoreKind::ALL {
            for doc in [root_fixture(kind, "max"), json!({}), json!([1, 2])] {
                assert_eq!(
                    validator.validate(kind, &doc),
                    Verdict::SchemaUnavailable,
                    "{label} {kind:?}"
                );
            }
        }
        assert_eq!(validator.retriever_calls(), 0, "{label}");
    }
    // Through decode: SchemaUnavailable is an error, never a document.
    let bytes = fixture_text("repositories.min.json");
    assert_eq!(
        decode(StoreKind::Repositories, bytes.as_bytes(), &variants[0].1).map(|_| ()),
        Err(StoreError::SchemaUnavailable)
    );
}

/// Fails when a schema that is consistent with its own hash but cannot be compiled (missing root
/// definition, local `$ref` to nothing, invalid keyword value) is treated as usable.
#[test]
fn a_schema_that_does_not_compile_is_unavailable() {
    let texts: Vec<(&str, &'static str)> = vec![
        ("no $defs at all", leak("{}".to_owned())),
        (
            "a dangling local reference",
            leak(SCHEMA_TEXT.replace("#/$defs/Timestamp", "#/$defs/Nothing")),
        ),
        (
            "an invalid keyword value",
            leak(SCHEMA_TEXT.replacen("\"minimum\": 0", "\"minimum\": \"zero\"", 1)),
        ),
    ];
    for (label, text) in texts {
        let validator = SchemaValidator::with_schema(text, &sha256_hex(text));
        for kind in StoreKind::ALL {
            assert_eq!(
                validator.validate(kind, &root_fixture(kind, "min")),
                Verdict::SchemaUnavailable,
                "{label} {kind:?}"
            );
        }
        assert_eq!(validator.retriever_calls(), 0, "{label}");
    }
}

/// A schema with an external reference never validates anything and the retriever is never asked. Fails if
/// `https://`, `file://`, relative or fragment-qualified external references are followed, or counted.
#[test]
fn external_references_never_validate_and_never_reach_the_retriever() {
    let roots = [
        "ConfigFile",
        "RepositoryFile",
        "WorkflowFile",
        "ApprovalFile",
        "LogFile",
        "SuppressionFile",
        "RagIndexFile",
        "ConversationFile",
        "ReportFile",
        "ToolManifestFile",
    ];
    for reference in [
        "https://example.invalid/store.json",
        "http://example.invalid/store.json#/$defs/X",
        "file:///nonexistent/store.json",
        "other.json",
        "./other.json#/x",
    ] {
        let mut defs = Map::new();
        for name in roots {
            defs.insert(name.to_owned(), json!({"$ref": reference}));
        }
        let text = leak(
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "oneOf": roots.iter().map(|n| json!({"$ref": format!("#/$defs/{n}")})).collect::<Vec<_>>(),
                "$defs": defs,
            })
            .to_string(),
        );
        let validator = SchemaValidator::with_schema(text, &sha256_hex(text));
        for kind in StoreKind::ALL {
            assert_eq!(
                validator.validate(kind, &root_fixture(kind, "max")),
                Verdict::SchemaUnavailable,
                "{reference} {kind:?}"
            );
        }
        assert_eq!(validator.retriever_calls(), 0, "{reference}");
    }

    // One external reference anywhere in the otherwise genuine schema poisons all ten roots.
    for reference in [
        "https://example.invalid/ts.json",
        "file:///nonexistent/ts.json",
    ] {
        let text = leak(SCHEMA_TEXT.replacen(
            "\"$ref\": \"#/$defs/Timestamp\"",
            &format!("\"$ref\": \"{reference}\""),
            1,
        ));
        assert_ne!(text, SCHEMA_TEXT);
        let validator = SchemaValidator::with_schema(text, &sha256_hex(text));
        for kind in StoreKind::ALL {
            assert_eq!(
                validator.validate(kind, &root_fixture(kind, "min")),
                Verdict::SchemaUnavailable,
                "{reference} {kind:?}"
            );
        }
        assert_eq!(validator.retriever_calls(), 0, "{reference}");
    }
}

/// Fails when validating every fixture, valid and invalid, makes the production validator call the
/// retriever, or when the embedded schema is reported unavailable.
#[test]
fn the_production_validator_never_calls_the_retriever() {
    let validator = SchemaValidator::new();
    for e in manifest() {
        let verdict = validator.validate(e.kind, &fixture(&e.file));
        assert_ne!(verdict, Verdict::SchemaUnavailable, "{}", e.file);
    }
    assert_eq!(validator.retriever_calls(), 0);
    assert_eq!(SchemaValidator::new().retriever_calls(), 0);
}

/// Several threads using a validator for the first time compile each root once and agree. Fails on a
/// data race or divergent verdicts (a poisoned or half-built slot).
#[test]
fn concurrent_first_use_agrees() {
    let validator = SchemaValidator::with_schema(SCHEMA_TEXT, SCHEMA_SHA256);
    let doc = fixture("report.max.json");
    let bad = set(doc.clone(), "/report_id", json!("nope"));
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                s.spawn(|| {
                    (
                        validator.validate(StoreKind::Report, &doc),
                        classify(&validator.validate(StoreKind::Report, &bad)),
                    )
                })
            })
            .collect();
        for h in handles {
            let (good, wrong) = h.join().expect("thread");
            assert_eq!(good, Verdict::Valid);
            assert_eq!(wrong, "schema:format");
        }
    });
    assert_eq!(validator.retriever_calls(), 0);
}

// --- differential A -------------------------------------------------------------------------------------

/// Pointers to every node below the root of `v`.
fn node_pointers(v: &Value, at: &mut String, out: &mut Vec<String>) {
    let visit = |segment: String, child: &Value, at: &mut String, out: &mut Vec<String>| {
        let len = at.len();
        at.push('/');
        at.push_str(&segment);
        out.push(at.clone());
        node_pointers(child, at, out);
        at.truncate(len);
    };
    match v {
        Value::Object(map) => {
            for (k, c) in map {
                visit(k.replace('~', "~0").replace('/', "~1"), c, at, out);
            }
        }
        Value::Array(items) => {
            for (i, c) in items.iter().enumerate() {
                visit(i.to_string(), c, at, out);
            }
        }
        _ => {}
    }
}

/// Every single-edit mutant of `doc`: each node removed (objects' members), replaced by seven other JSON
/// values, and each object given an unknown member.
fn mutants(doc: &Value) -> Vec<(String, Value)> {
    let mut pointers = Vec::new();
    node_pointers(doc, &mut String::new(), &mut pointers);
    let replacements = [
        json!(null),
        json!("x"),
        json!(-1),
        json!(true),
        json!([]),
        json!({}),
        json!(1.5),
    ];
    let mut out = Vec::new();
    for ptr in &pointers {
        for r in &replacements {
            let mut m = doc.clone();
            *m.pointer_mut(ptr).expect("pointer") = r.clone();
            out.push((format!("{ptr} <- {r}"), m));
        }
        let (parent, key) = ptr.rsplit_once('/').expect("pointer has a parent");
        let parent_node = if parent.is_empty() {
            Some(doc)
        } else {
            doc.pointer(parent)
        };
        if parent_node.is_some_and(Value::is_object) {
            let mut m = doc.clone();
            let target = if parent.is_empty() {
                &mut m
            } else {
                m.pointer_mut(parent).expect("parent")
            };
            target
                .as_object_mut()
                .expect("object")
                .remove(&key.replace("~1", "/").replace("~0", "~"));
            out.push((format!("{ptr} removed"), m));
        }
        if doc.pointer(ptr).is_some_and(Value::is_object) {
            let mut m = doc.clone();
            m.pointer_mut(ptr)
                .expect("pointer")
                .as_object_mut()
                .expect("object")
                .insert("zz_unknown".to_owned(), json!(1));
            out.push((format!("{ptr} + unknown member"), m));
        }
    }
    let mut m = doc.clone();
    m.as_object_mut()
        .expect("root object")
        .insert("zz_unknown".to_owned(), json!(1));
    out.push(("root + unknown member".to_owned(), m));
    out
}

/// Differential A (request §6.2). For every valid fixture and every single-edit mutant of it, the
/// per-root verdict agrees with validation through the full `oneOf` root about whether the *schema* accepts
/// the document: a `Layer::Schema` problem exactly when the reference rejects it. (Mutants stopped earlier
/// by the version gate are checked to be rejected by the reference too.) Fails on any disagreement, and
/// on a run that would be vacuous (no schema rejection, no typed rejection after a schema pass, no
/// accepted mutant).
#[test]
fn differential_a_per_root_verdict_equals_the_full_one_of_root() {
    let validator = SchemaValidator::new();
    let (mut total, mut schema_rejected, mut typed_rejected, mut accepted, mut gated) =
        (0_usize, 0_usize, 0_usize, 0_usize, 0_usize);
    for e in manifest().iter().filter(|e| e.expect == "valid") {
        let base = fixture(&e.file);
        for (label, m) in mutants(&base) {
            total += 1;
            let reference_accepts = full_oneof().is_valid(&m);
            let verdict = validator.validate(e.kind, &m);
            let schema_accepts = match &verdict {
                Verdict::Valid => {
                    accepted += 1;
                    true
                }
                Verdict::Invalid(p) => match p.items[0].layer {
                    Layer::Schema => {
                        schema_rejected += 1;
                        false
                    }
                    Layer::Typed | Layer::Semantic => {
                        typed_rejected += 1;
                        true
                    }
                    Layer::Version | Layer::Json => {
                        gated += 1;
                        assert!(
                            !reference_accepts,
                            "{} {label}: gated but the reference accepts",
                            e.file
                        );
                        continue;
                    }
                },
                Verdict::SchemaUnavailable => panic!("{} {label}: schema unavailable", e.file),
            };
            assert_eq!(
                schema_accepts, reference_accepts,
                "{} {label}: per-root and oneOf disagree ({verdict:?})",
                e.file
            );
        }
    }
    assert!(total > 2000, "only {total} mutants");
    assert!(schema_rejected > 500, "{schema_rejected} schema rejections");
    assert!(
        typed_rejected > 20,
        "{typed_rejected} typed-only rejections"
    );
    assert!(accepted > 20, "{accepted} accepted mutants");
    assert!(gated > 0, "no mutant reached the version gate");
}

/// Differential A on every fixture file, valid and invalid. The reference (`oneOf` root) accepts exactly the
/// documents whose fault, if any, is not a schema fault: valid ones, a document of another root (the
/// per-root validator is stricter by design) and the typed-only faults. It rejects version faults and
/// schema faults. Fails when the manifest verdict and the reference disagree in that way.
#[test]
fn differential_a_agrees_on_every_fixture_file() {
    for e in manifest() {
        let doc = fixture(&e.file);
        let reference_accepts = full_oneof().is_valid(&doc);
        let expected = matches!(e.expect.as_str(), "valid" | "store_type_mismatch" | "typed");
        assert_eq!(reference_accepts, expected, "{} ({})", e.file, e.expect);
        let verdict = SchemaValidator::new().validate(e.kind, &doc);
        let schema_layer_rejects = matches!(
            &verdict,
            Verdict::Invalid(p) if p.items[0].layer == Layer::Schema
        );
        assert_eq!(
            schema_layer_rejects,
            !reference_accepts && e.expect.starts_with("schema:"),
            "{}",
            e.file
        );
    }
}

// --- E-3: no echo ---------------------------------------------------------------------------------------

fn assert_no_marker(text: &str, label: &str) {
    let lower = text.to_ascii_lowercase();
    for marker in [MARKER, "424242424242", "4242.4242"] {
        assert!(
            !text.contains(marker) && !lower.contains(&marker.to_ascii_lowercase()),
            "{label}: {marker} appears in {text}"
        );
    }
}

fn assert_problems_clean(problems: &Problems, label: &str) {
    assert_no_marker(&format!("{problems:?}"), label);
    for p in &problems.items {
        assert_no_marker(p.pointer.as_str(), label);
        assert_no_marker(&format!("{p:?}"), label);
    }
}

fn assert_error_clean(err: &StoreError, label: &str) {
    assert_no_marker(&format!("{err:?}"), label);
    assert_no_marker(&err.to_string(), label);
    if let StoreError::Invalid(_, problems) = err {
        assert_problems_clean(problems, label);
    }
}

/// Runs `doc` through `validate`, through `decode` with the schema validator, and through `decode` with a
/// validator that accepts everything (which exposes the typed layer's own errors). Returns the verdict.
/// Fails if a planted marker is found in any problem, pointer, `Debug` or `Display`.
fn run_clean(kind: StoreKind, doc: &Value, label: &str) -> Verdict {
    let validator = SchemaValidator::new();
    let verdict = validator.validate(kind, doc);
    assert_no_marker(&format!("{verdict:?}"), label);
    if let Verdict::Invalid(problems) = &verdict {
        assert_problems_clean(problems, label);
    }
    let bytes = serde_json::to_vec(doc).expect("serialise");
    match decode(kind, &bytes, &validator) {
        Ok(_) => assert_eq!(
            verdict,
            Verdict::Valid,
            "{label}: decode and validate disagree"
        ),
        Err(err) => {
            assert_error_clean(&err, label);
            assert!(matches!(err, StoreError::Invalid(..)), "{label}: {err:?}");
        }
    }
    if let Err(err) = decode(kind, &bytes, &AlwaysValid) {
        assert_error_clean(&err, label);
        assert!(matches!(err, StoreError::Invalid(..)), "{label}: {err:?}");
    }
    verdict
}

struct Plant {
    label: &'static str,
    file: &'static str,
    edit: Edit,
    /// Whether the planted document is invalid (a marker in free text is still a valid document).
    invalid: bool,
}

#[rustfmt::skip]
fn plants() -> Vec<Plant> {
    let m = || json!(MARKER);
    let p = |label, file, edit, invalid| Plant {
        label,
        file,
        edit,
        invalid,
    };
    vec![
        // validated newtypes
        p("newtype slug", "repositories.max.json", Edit::Set("/repositories/1/slug", m()), true),
        p("newtype remote url", "repositories.max.json", Edit::Set("/repositories/1/remote_url", m()), true),
        p("newtype relative path", "repositories.max.json", Edit::Set("/repositories/1/files/0/path", json!(format!("../{MARKER}"))), true),
        p("newtype timezone", "workflows.max.json", Edit::Set("/workflows/1/schedule/timezone", m()), true),
        p("newtype credential reference", "config.max.json", Edit::Set("/config/provider/credential_reference", m()), true),
        p("newtype adapter id", "tool_manifest.max.json", Edit::Set("/tools/1/adapter_id", json!(format!("{MARKER} x"))), true),
        // IDs
        p("id", "repositories.max.json", Edit::Set("/repositories/1/repo_id", m()), true),
        p("id in a list", "approvals.max.json", Edit::Set("/approvals/1/repo_ids/0", m()), true),
        p("id with marker number", "approvals.max.json", Edit::Set("/approvals/1/repo_ids/0", json!(MARKER_INT)), true),
        // timestamps
        p("timestamp", "repositories.max.json", Edit::Set("/repositories/1/registered_at", m()), true),
        p("timestamp root", "log.max.json", Edit::Set("/updated_at", m()), true),
        p("nullable timestamp", "repositories.max.json", Edit::Set("/repositories/1/last_update_at", m()), true),
        // enums: derived domain enums and closed_enum! enums
        p("derived enum capability", "approvals.max.json", Edit::Set("/approvals/1/capability", m()), true),
        p("derived enum data category", "approvals.max.json", Edit::Set("/approvals/1/data_categories/0", m()), true),
        p("derived enum availability", "tool_manifest.max.json", Edit::Set("/tools/0/availability", m()), true),
        p("derived enum execution", "log.max.json", Edit::Set("/runs/1/execution", m()), true),
        p("derived enum outcome", "report.max.json", Edit::Set("/checks/0/outcome", m()), true),
        p("closed enum checkout state", "repositories.max.json", Edit::Set("/repositories/1/checkout_state", m()), true),
        p("closed enum discovery state", "repositories.max.json", Edit::Set("/repositories/1/files/0/discovery_state", m()), true),
        p("closed enum output format", "workflows.max.json", Edit::Set("/workflows/1/output_formats/0", m()), true),
        p("closed enum message role", "conversations.max.json", Edit::Set("/conversations/1/messages/0/role", m()), true),
        p("closed enum with a number", "repositories.max.json", Edit::Set("/repositories/1/checkout_state", json!(MARKER_INT)), true),
        // integers
        p("integer as string", "repositories.max.json", Edit::Set("/repositories/1/inventory_revision", m()), true),
        p("integer as float", "repositories.max.json", Edit::Set("/repositories/1/inventory_revision", json!(MARKER_FLOAT)), true),
        p("integer negative marker", "repositories.max.json", Edit::Set("/repositories/1/inventory_revision", json!(-MARKER_INT)), true),
        p("nullable integer as string", "approvals.max.json", Edit::Set("/approvals/1/workflow_revision", m()), true),
        p("nullable integer negative marker", "conversations.max.json", Edit::Set("/conversations/1/messages/1/tokens_in", json!(-MARKER_INT)), true),
        // booleans
        p("boolean as string", "workflows.max.json", Edit::Set("/workflows/1/enabled", m()), true),
        p("boolean as number", "workflows.max.json", Edit::Set("/workflows/1/enabled", json!(MARKER_INT)), true),
        p("constant boolean", "config.max.json", Edit::Set("/config/telemetry_enabled", m()), true),
        // SanitisedText / RedactedText fields: a string is fine, a wrong type is not
        p("sanitised text as a string", "workflows.max.json", Edit::Set("/workflows/1/name", m()), false),
        p("sanitised text as an object", "workflows.max.json", Edit::Set("/workflows/1/name", json!({"k": MARKER})), true),
        p("sanitised text as a number", "workflows.max.json", Edit::Set("/workflows/1/name", json!(MARKER_INT)), true),
        p("redacted text as a string", "repositories.max.json", Edit::Set("/repositories/1/last_error_redacted", m()), false),
        p("redacted text as an array", "repositories.max.json", Edit::Set("/repositories/1/last_error_redacted", json!([MARKER])), true),
        p("redacted text as a number", "report.max.json", Edit::Set("/findings/1/evidence_redacted", json!(MARKER_FLOAT)), true),
        p("sanitised limitation as an object", "report.max.json", Edit::Set("/limitations/0", json!({"k": MARKER})), true),
        // unknown keys
        p("unknown key at the root", "repositories.max.json", Edit::Add("", MARKER, json!(true)), true),
        p("unknown key in a record", "repositories.max.json", Edit::Add("/repositories/1", MARKER, json!(1)), true),
        p("unknown key deep", "report.max.json", Edit::Add("/plan/target_snapshots/0", MARKER, json!(MARKER)), true),
        p("unknown key with a marker number value", "config.max.json", Edit::Add("/config", MARKER, json!(MARKER_INT)), true),
        // string-map keys
        p("map key, wrong value type", "workflows.max.json", Edit::Add("/workflows/1/scan_settings/renderers/0/parameters_redacted", MARKER, json!(5)), true),
        p("map key, valid value", "workflows.max.json", Edit::Add("/workflows/1/scan_settings/renderers/0/parameters_redacted", MARKER, json!("fine")), false),
        p("map value", "workflows.max.json", Edit::Add("/workflows/1/scan_settings/renderers/0/parameters_redacted", "k", json!([MARKER])), true),
        // wrong container types
        p("array replaced by a string", "repositories.max.json", Edit::Set("/repositories/1/files", m()), true),
        p("array replaced by an object", "repositories.max.json", Edit::Set("/repositories", json!({MARKER: MARKER})), true),
        p("record replaced by an array", "repositories.max.json", Edit::Set("/repositories/1", json!([MARKER])), true),
        p("object replaced by a string", "report.max.json", Edit::Set("/plan", m()), true),
        p("object replaced by a number", "report.max.json", Edit::Set("/coverage", json!(MARKER_INT)), true),
        p("root replaced by an array", "repositories.max.json", Edit::Set("", json!([MARKER])), true),
        p("root replaced by a string", "repositories.max.json", Edit::Set("", m()), true),
        // gates
        p("schema version", "repositories.max.json", Edit::Set("/schema_version", m()), true),
        p("store type", "repositories.max.json", Edit::Set("/store_type", m()), true),
        // constants, patterns
        p("digest pattern", "report.max.json", Edit::Set("/plan/plan_digest", m()), true),
        p("git oid pattern", "repositories.max.json", Edit::Set("/repositories/1/files/0/commit_oid", m()), true),
        p("provider endpoint constant", "config.max.json", Edit::Set("/config/provider/endpoint", m()), true),
    ]
}

/// E-3 (request §5.11). A marker string, number or float is planted into a field of every kind. Fails when
/// it appears in any `Problem` (pointer included), in `Debug` of `Verdict`, `Problems` or `StoreError`, or in
/// `Display` of `StoreError`; on the schema path, the `decode` path, and the typed layer alone; or when the
/// plant did not do what the table says (a planted free-text field stays valid, any other plant makes the
/// document invalid), so the check cannot pass vacuously.
#[test]
fn planted_markers_are_never_echoed() {
    for plant in plants() {
        let base = fixture(plant.file);
        let kind = StoreKind::from_store_type(base["store_type"].as_str().expect("store_type"))
            .expect("kind");
        // Positive control: the unplanted fixture is valid and clean.
        assert_eq!(
            run_clean(kind, &base, plant.label),
            Verdict::Valid,
            "{}: control",
            plant.label
        );
        let doc = apply(base, &plant.edit);
        let verdict = run_clean(kind, &doc, plant.label);
        assert_eq!(
            matches!(verdict, Verdict::Invalid(_)),
            plant.invalid,
            "{}: {verdict:?}",
            plant.label
        );
        // The plant is really in the document (so a miss would be a leak, not a no-op edit).
        assert!(
            serde_json::to_string(&doc)
                .expect("serialise")
                .contains(MARKER)
                || serde_json::to_string(&doc)
                    .expect("serialise")
                    .contains("424242424242")
                || serde_json::to_string(&doc)
                    .expect("serialise")
                    .contains("4242.4242"),
            "{}: the plant is missing from the document",
            plant.label
        );
    }
}

/// Text that is not JSON never echoes either (the lead's `read.rs` tests cover more shapes; this one goes
/// through the production validator's `decode`).
#[test]
fn malformed_json_text_is_not_echoed() {
    let validator = SchemaValidator::new();
    for raw in [
        format!("{{\"{MARKER}\": "),
        format!("{{\"a\": {MARKER}}}"),
        format!("[\"{MARKER}\""),
    ] {
        let err = decode(StoreKind::Config, raw.as_bytes(), &validator).expect_err("syntax");
        assert_error_clean(&err, "syntax");
        let StoreError::Invalid(_, problems) = err else {
            panic!("expected Invalid");
        };
        assert_eq!(
            (only_problem(&problems).layer, only_problem(&problems).code),
            (Layer::Json, ProblemCode::Syntax)
        );
    }
}

/// The key of a string map is replaced by `*` in the pointer, and a declared name is never replaced.
/// Fails if the planted key, or any other document key, reaches the pointer, or if declared names are masked.
#[test]
fn map_keys_and_unknown_keys_are_masked_in_pointers() {
    let validator = SchemaValidator::new();
    let map = "/workflows/1/scan_settings/renderers/0/parameters_redacted";
    let base = fixture("workflows.max.json");
    let doc = apply(base, &Edit::Add(map, MARKER, json!(5)));
    let problems = invalid(validator.validate(StoreKind::Workflows, &doc));
    let p = only_problem(&problems);
    assert_eq!(p.code, ProblemCode::Keyword(SchemaKeyword::Type));
    assert_eq!(p.pointer.as_str(), format!("{map}/*"));

    // A map key that spells a declared property elsewhere is still masked.
    let doc = apply(
        fixture("workflows.max.json"),
        &Edit::Add(map, "framework", json!(5)),
    );
    let problems = invalid(validator.validate(StoreKind::Workflows, &doc));
    assert_eq!(only_problem(&problems).pointer.as_str(), format!("{map}/*"));

    // An unknown member is reported on its object; the pointer has no `*` because the key is not in it.
    let doc = apply(
        fixture("workflows.max.json"),
        &Edit::Add("/workflows/1", MARKER, json!(1)),
    );
    let problems = invalid(validator.validate(StoreKind::Workflows, &doc));
    assert_eq!(only_problem(&problems).pointer.as_str(), "/workflows/1");
}

// --- problem cap ----------------------------------------------------------------------------------------

/// More than 100 problems keep exactly 100 and set `truncated`; exactly 100 do not. Fails on an off-by-one,
/// an unbounded list, or a missing flag.
#[test]
fn more_than_one_hundred_problems_are_capped_and_flagged() {
    let validator = SchemaValidator::new();
    let record = fixture("repositories.max.json")["repositories"][0].clone();
    let doc_with = |n: usize| {
        let mut records = Vec::new();
        for i in 0..n {
            let mut r = record.clone();
            r["zz_unknown"] = json!(i);
            records.push(r);
        }
        let mut doc = root_fixture(StoreKind::Repositories, "min");
        doc["repositories"] = Value::Array(records);
        doc
    };
    for (n, truncated) in [
        (1_usize, false),
        (99, false),
        (100, false),
        (101, true),
        (500, true),
    ] {
        let problems = invalid(validator.validate(StoreKind::Repositories, &doc_with(n)));
        assert_eq!(problems.items.len(), n.min(MAX_PROBLEMS), "n = {n}");
        assert_eq!(problems.truncated, truncated, "n = {n}");
        for item in &problems.items {
            assert_eq!(
                (item.layer, item.code),
                (
                    Layer::Schema,
                    ProblemCode::Keyword(SchemaKeyword::AdditionalProperties)
                )
            );
            assert!(
                item.pointer.as_str().starts_with("/repositories/"),
                "{item:?}"
            );
        }
        let mut pointers: Vec<&str> = problems.items.iter().map(|p| p.pointer.as_str()).collect();
        pointers.sort_unstable();
        pointers.dedup();
        assert_eq!(
            pointers.len(),
            problems.items.len(),
            "n = {n}: one problem per record"
        );
    }
    // The same through decode.
    let bytes = serde_json::to_vec(&doc_with(150)).expect("serialise");
    match decode(StoreKind::Repositories, &bytes, &validator) {
        Err(StoreError::Invalid(_, p)) => {
            assert_eq!(p.items.len(), MAX_PROBLEMS);
            assert!(p.truncated);
        }
        other => panic!("{other:?}"),
    }
}

/// Several different faults in one document are all reported, each at its own location. Fails if only the
/// first is kept or the locations are wrong.
#[test]
fn several_faults_are_reported_together_with_their_locations() {
    let validator = SchemaValidator::new();
    let doc = apply(
        apply(
            set(
                root_fixture(StoreKind::Repositories, "max"),
                "/repositories/1/repo_id",
                json!("nope"),
            ),
            &Edit::Remove("/repositories/0/slug"),
        ),
        &Edit::Add("/repositories/1/files/1", "zz_unknown", json!(1)),
    );
    let problems = invalid(validator.validate(StoreKind::Repositories, &doc));
    let mut got: Vec<(String, SchemaKeyword)> = problems
        .items
        .iter()
        .map(|p| match p.code {
            ProblemCode::Keyword(k) => (p.pointer.as_str().to_owned(), k),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    got.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        got,
        vec![
            ("/repositories/0".to_owned(), SchemaKeyword::Required),
            (
                "/repositories/1/files/1".to_owned(),
                SchemaKeyword::AdditionalProperties
            ),
            ("/repositories/1/repo_id".to_owned(), SchemaKeyword::Format),
        ]
    );
}

//! Lead tests for the decode pipeline glue (`persistence::read`, request §5.12, §6.1) and the problem
//! builders (`persistence::validate`). The schema layer is T08 (`tests/schema.rs`). All data is synthetic.

#![allow(
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::sync::atomic::{AtomicU32, Ordering};

use domain::store::StoreKind;
use persistence::error::StoreError;
use persistence::read::{MAX_DOCUMENT_BYTES, decode, precheck, typed_decode, version_class};
use persistence::validate::{
    DocumentValidator, Layer, MAX_POINTER_BYTES, MAX_PROBLEMS, Problem, ProblemCode, Problems,
    RuleId, Verdict, VersionClass,
};
use serde_json::{Value, json};

const MARKER: &str = "PLANTEDMARKERq7Zx";
const TS: &str = "2026-10-16T05:25:00Z";

/// Returns a fixed verdict and counts calls.
#[derive(Debug)]
struct Fixed {
    verdict: Verdict,
    calls: AtomicU32,
}

impl Fixed {
    fn new(verdict: Verdict) -> Self {
        Self {
            verdict,
            calls: AtomicU32::new(0),
        }
    }
    fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl DocumentValidator for Fixed {
    fn validate(&self, _kind: StoreKind, _doc: &Value) -> Verdict {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.verdict.clone()
    }
}

fn repo_doc() -> Value {
    json!({"schema_version": "1.4", "store_type": "repositories", "updated_at": TS, "repositories": []})
}

fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_else(|e| panic!("serialise fixture: {e}"))
}

fn problems(err: StoreError, kind: StoreKind) -> Problems {
    match err {
        StoreError::Invalid(k, p) => {
            assert_eq!(k, kind);
            p
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn only(p: &Problems) -> &Problem {
    assert_eq!(p.items.len(), 1, "{p:?}");
    assert!(!p.truncated);
    &p.items[0]
}

fn assert_no_marker(err: &StoreError) {
    let dbg = format!("{err:?}");
    let disp = err.to_string();
    assert!(!dbg.contains(MARKER), "Debug echoes input: {dbg}");
    assert!(!disp.contains(MARKER), "Display echoes input: {disp}");
}

#[test]
fn valid_document_decodes_to_its_kind_after_one_validator_call() {
    let v = Fixed::new(Verdict::Valid);
    let any = decode(StoreKind::Repositories, &bytes(&repo_doc()), &v).expect("decodes");
    assert_eq!(any.kind(), StoreKind::Repositories);
    assert_eq!(v.calls(), 1);
}

#[test]
fn oversized_input_is_too_large_before_parsing() {
    let v = Fixed::new(Verdict::Valid);
    let n = usize::try_from(MAX_DOCUMENT_BYTES + 1).expect("fits in usize on this host");
    let big = vec![b' '; n];
    let p = problems(
        decode(StoreKind::Log, &big, &v).expect_err("too large"),
        StoreKind::Log,
    );
    let pr = only(&p);
    assert_eq!((pr.layer, pr.code), (Layer::Json, ProblemCode::TooLarge));
    assert_eq!(v.calls(), 0);
}

#[test]
fn input_at_the_bound_is_not_too_large() {
    let v = Fixed::new(Verdict::Valid);
    let mut doc = bytes(&repo_doc());
    let pad = usize::try_from(MAX_DOCUMENT_BYTES).expect("fits") - doc.len();
    doc.extend(std::iter::repeat_n(b' ', pad));
    assert_eq!(u64::try_from(doc.len()).expect("fits"), MAX_DOCUMENT_BYTES);
    assert!(decode(StoreKind::Repositories, &doc, &v).is_ok());
}

#[test]
fn json_syntax_error_is_fixed_text_without_the_input() {
    let v = Fixed::new(Verdict::Valid);
    for raw in [
        format!("{{\"store_type\": \"{MARKER}"),
        format!("{{\"a\": {MARKER}}}"),
        format!("\u{feff}{{\"x\": \"{MARKER}\"}}"),
        String::new(),
    ] {
        let err = decode(StoreKind::Config, raw.as_bytes(), &v).expect_err("syntax");
        assert_no_marker(&err);
        let p = problems(err, StoreKind::Config);
        let pr = only(&p);
        assert_eq!(
            (pr.layer, pr.code),
            (Layer::Json, ProblemCode::Syntax),
            "{raw:?}"
        );
        assert_eq!(pr.pointer.as_str(), "");
    }
    assert_eq!(v.calls(), 0);
}

#[test]
fn non_object_documents_are_rejected() {
    let v = Fixed::new(Verdict::Valid);
    for doc in [json!([]), json!(MARKER), json!(1), Value::Null, json!(true)] {
        let err = decode(StoreKind::Log, &bytes(&doc), &v).expect_err("not object");
        assert_no_marker(&err);
        let p = problems(err, StoreKind::Log);
        assert_eq!(only(&p).code, ProblemCode::NotObject, "{doc}");
    }
    assert_eq!(v.calls(), 0);
}

#[test]
fn store_type_faults_are_classified_without_echo() {
    let v = Fixed::new(Verdict::Valid);
    let mut cases: Vec<(Value, ProblemCode)> = Vec::new();
    for bad in [
        json!(MARKER),
        json!(7),
        Value::Null,
        json!(["repositories"]),
        json!("Repositories"),
    ] {
        let mut d = repo_doc();
        d["store_type"] = bad;
        cases.push((d, ProblemCode::UnknownStoreType));
    }
    let mut missing = repo_doc();
    missing
        .as_object_mut()
        .expect("object")
        .remove("store_type");
    cases.push((missing, ProblemCode::UnknownStoreType));
    for other in StoreKind::ALL
        .into_iter()
        .filter(|k| *k != StoreKind::Repositories)
    {
        let mut d = repo_doc();
        d["store_type"] = json!(other.store_type());
        cases.push((d, ProblemCode::StoreTypeMismatch));
    }
    for (doc, code) in cases {
        let err = decode(StoreKind::Repositories, &bytes(&doc), &v).expect_err("store_type");
        assert_no_marker(&err);
        let p = problems(err, StoreKind::Repositories);
        let pr = only(&p);
        assert_eq!((pr.layer, pr.code), (Layer::Version, code), "{doc}");
        assert_eq!(pr.pointer.as_str(), "/store_type");
    }
    assert_eq!(v.calls(), 0);
}

#[test]
fn version_classes_follow_request_6_1() {
    let cases: Vec<(Option<Value>, Option<VersionClass>)> = vec![
        (Some(json!("1.4")), None),
        (Some(json!("1.0")), Some(VersionClass::Older)),
        (Some(json!("1.1")), Some(VersionClass::Older)),
        (Some(json!("1.2")), Some(VersionClass::Older)),
        (Some(json!("1.3")), Some(VersionClass::Older)),
        (Some(json!("1.5")), Some(VersionClass::NewerOrUnknown)),
        (Some(json!("9.9")), Some(VersionClass::NewerOrUnknown)),
        (Some(json!("1.4.0")), Some(VersionClass::NewerOrUnknown)),
        (Some(json!(" 1.4")), Some(VersionClass::NewerOrUnknown)),
        (Some(json!("")), Some(VersionClass::NewerOrUnknown)),
        (Some(json!(1.4)), Some(VersionClass::NewerOrUnknown)),
        (Some(Value::Null), Some(VersionClass::NewerOrUnknown)),
        (Some(json!(MARKER)), Some(VersionClass::NewerOrUnknown)),
        (None, Some(VersionClass::Missing)),
    ];
    let v = Fixed::new(Verdict::Valid);
    for (value, expected) in cases {
        assert_eq!(version_class(value.as_ref()), expected, "{value:?}");
        let mut d = repo_doc();
        match &value {
            Some(x) => d["schema_version"] = x.clone(),
            None => {
                d.as_object_mut().expect("object").remove("schema_version");
            }
        }
        let out = decode(StoreKind::Repositories, &bytes(&d), &v);
        match expected {
            None => assert!(out.is_ok()),
            Some(class) => {
                let err = out.expect_err("version");
                assert_no_marker(&err);
                let p = problems(err, StoreKind::Repositories);
                let pr = only(&p);
                assert_eq!(
                    (pr.layer, pr.code),
                    (Layer::Version, ProblemCode::UnsupportedVersion(class))
                );
                assert_eq!(pr.pointer.as_str(), "/schema_version");
            }
        }
    }
    // Only the documents with "1.4" reached the validator.
    assert_eq!(v.calls(), 1);
}

#[test]
fn store_type_is_checked_before_schema_version() {
    let mut d = repo_doc();
    d["store_type"] = json!("log");
    d["schema_version"] = json!("1.3");
    let p = precheck(StoreKind::Repositories, &d).expect_err("mismatch first");
    assert_eq!(only(&p).code, ProblemCode::StoreTypeMismatch);
}

#[test]
fn validator_verdicts_are_propagated() {
    let planted = Problems::one(Problem::new(
        Layer::Schema,
        ProblemCode::Rule,
        "/repositories/0",
        Some(RuleId("D11-01")),
    ));
    let v = Fixed::new(Verdict::Invalid(planted.clone()));
    let p = problems(
        decode(StoreKind::Repositories, &bytes(&repo_doc()), &v).expect_err("invalid"),
        StoreKind::Repositories,
    );
    assert_eq!(p, planted);

    let v = Fixed::new(Verdict::SchemaUnavailable);
    let err = decode(StoreKind::Repositories, &bytes(&repo_doc()), &v).expect_err("unavailable");
    assert_eq!(err, StoreError::SchemaUnavailable);
}

#[test]
fn typed_decode_failure_after_a_valid_verdict_is_a_fixed_typed_problem() {
    // A validator that wrongly says Valid must not let a bad document through, and the typed layer must not
    // echo what it rejected. One planted value per field kind of the repository root.
    let v = Fixed::new(Verdict::Valid);
    let repo = |field: &str, value: Value| {
        let mut r = json!({
            "repo_id": "00000000-0000-4000-8000-000000000010",
            "display_name": "Demo",
            "slug": "demo",
            "remote_url": "https://example.invalid/demo.git",
            "canonical_remote": "example.invalid/demo",
            "local_path": "checkouts/demo",
            "default_ref_kind": "branch",
            "default_ref_value": "main",
            "registered_at": TS,
            "last_update_at": null,
            "last_commit_oid": null,
            "checkout_state": "ready",
            "last_error_redacted": null,
            "inventory_revision": 0,
            "inventory_at": null,
            "files": []
        });
        r[field] = value;
        let mut d = repo_doc();
        d["repositories"] = json!([r]);
        d
    };
    // Positive control: the unmodified record decodes, so every failure below is caused by its planted value.
    let base = repo("inventory_revision", json!(0));
    assert!(
        decode(StoreKind::Repositories, &bytes(&base), &v).is_ok(),
        "base fixture must be valid"
    );
    let cases = vec![
        repo("repo_id", json!(MARKER)),
        repo("slug", json!(MARKER)),
        repo("registered_at", json!(MARKER)),
        repo("default_ref_kind", json!(MARKER)),
        repo("checkout_state", json!(MARKER)),
        repo("inventory_revision", json!(MARKER)),
        repo("display_name", json!(12345)),
        repo("files", json!(MARKER)),
        repo(MARKER, json!(1)),
        {
            let mut d = repo_doc();
            d[MARKER] = json!(true);
            d
        },
        {
            let mut d = repo_doc();
            d["updated_at"] = json!(MARKER);
            d
        },
    ];
    for doc in cases {
        let err =
            decode(StoreKind::Repositories, &bytes(&doc), &v).expect_err("typed decode must fail");
        assert_no_marker(&err);
        let p = problems(err, StoreKind::Repositories);
        let pr = only(&p);
        assert_eq!(
            (pr.layer, pr.code),
            (Layer::Typed, ProblemCode::TypedDecode)
        );
        assert_eq!(pr.pointer.as_str(), "");
        assert_eq!(pr.rule, None);
    }
}

#[test]
fn typed_decode_maps_every_kind_to_its_variant() {
    for kind in StoreKind::ALL {
        // An empty object passes no root, so every kind fails — through its own decoder, with one fixed
        // problem (a panic or a wrong-kind success would fail the test).
        let p = typed_decode(kind, json!({})).expect_err("empty object is not a root");
        assert_eq!(only(&p).code, ProblemCode::TypedDecode, "{kind:?}");
    }
    let any = typed_decode(StoreKind::Repositories, repo_doc()).expect("decodes");
    assert_eq!(any.kind(), StoreKind::Repositories);
}

#[test]
fn problems_are_capped_and_truncation_carries_over() {
    let mk = || {
        Problem::new(
            Layer::Semantic,
            ProblemCode::Rule,
            "/x",
            Some(RuleId("D11-01")),
        )
    };
    let mut p = Problems::default();
    assert!(p.is_empty());
    for _ in 0..MAX_PROBLEMS {
        p.push(mk());
    }
    assert_eq!(p.items.len(), MAX_PROBLEMS);
    assert!(!p.truncated);
    p.push(mk());
    assert_eq!(p.items.len(), MAX_PROBLEMS);
    assert!(p.truncated);
    assert!(!p.is_empty());

    let mut q = Problems::one(mk());
    q.extend(p);
    assert_eq!(q.items.len(), MAX_PROBLEMS);
    assert!(q.truncated);

    let mut r = Problems::default();
    r.extend(Problems {
        items: Vec::new(),
        truncated: true,
    });
    assert!(r.truncated && !r.is_empty());
}

#[test]
fn problem_pointer_is_sanitised_and_bounded_without_splitting_escapes() {
    let long = format!("/{}", "a".repeat(MAX_POINTER_BYTES * 2));
    assert_eq!(
        Problem::new(Layer::Schema, ProblemCode::Rule, &long, None)
            .pointer
            .as_str()
            .len(),
        MAX_POINTER_BYTES
    );

    let multibyte = format!("/{}", "é".repeat(MAX_POINTER_BYTES));
    let pr = Problem::new(Layer::Schema, ProblemCode::Rule, &multibyte, None);
    assert!(pr.pointer.as_str().len() <= MAX_POINTER_BYTES);
    assert!(pr.pointer.as_str().chars().all(|c| c == '/' || c == 'é'));

    let escaped = format!("/{}", "\u{202e}".repeat(MAX_POINTER_BYTES));
    let pr = Problem::new(Layer::Schema, ProblemCode::Rule, &escaped, None);
    let text = pr.pointer.as_str();
    assert!(text.len() <= MAX_POINTER_BYTES);
    assert!(!text.contains('\u{202e}'), "bidi control must be escaped");
    let body = text.strip_prefix('/').expect("starts with /");
    assert!(!body.is_empty());
    assert_eq!(
        body.len() % "<U+202E>".len(),
        0,
        "an escape was cut: {text}"
    );
    assert!(body.split_inclusive('>').all(|e| e == "<U+202E>"), "{text}");

    assert_eq!(
        Problem::new(Layer::Json, ProblemCode::Syntax, "", None)
            .pointer
            .as_str(),
        ""
    );
}

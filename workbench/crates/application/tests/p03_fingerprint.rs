//! P03-T05 tests: the canonical operation fingerprint, encoding v1 (`planning/P03-T01-request.md` §5.3).
//!
//! The golden digests were computed by an independent Python (`hashlib`) implementation of §5.3 written
//! from the specification, not from the Rust code (the script is reproduced in the T05-W1 report). A
//! second, smaller reference encoder below re-derives the bytes from the specification text with `sha2`
//! and the serde names of the enums. Every test says what would make it fail. All texts are synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use application::permission::{
    ArgError, ArgName, FINGERPRINT_PREFIX, MAX_ARG_VALUE_BYTES, NormalisedDestination,
    OperationArg, OperationName, PermissionRequest, RequestedData, WorkflowBinding, fingerprint,
};
use domain::contract::{ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{RepoId, RunId, WorkflowId};
use domain::store::provider::InferenceScope;
use domain::text::sanitise;
use domain::vocab::Capability;
use sha2::{Digest as _, Sha256};

/// A one-field change to a request, used by the one-field-at-a-time tables.
type Mutator = Box<dyn Fn(&mut PermissionRequest)>;

const R1: &str = "00000000-0000-4000-8000-000000000001";
const R2: &str = "00000000-0000-4000-8000-000000000002";
const R3: &str = "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
const WF: &str = "00000000-0000-4000-8000-0000000000a1";
const RUN: &str = "00000000-0000-4000-8000-0000000000b1";

const ALL_CATEGORIES: [DataCategory; 7] = [
    DataCategory::RepositoryMetadata,
    DataCategory::RedactedSource,
    DataCategory::RedactedFindings,
    DataCategory::SchemaDocuments,
    DataCategory::PublicArtifactIdentifiers,
    DataCategory::RedactedChatContext,
    DataCategory::AppMetadata,
];

fn repo(s: &str) -> RepoId {
    RepoId::parse(s).expect("repo id")
}

fn dest(s: &str) -> NormalisedDestination {
    NormalisedDestination::parse(s).expect("destination")
}

fn arg(name: &str, value: &str) -> OperationArg {
    OperationArg {
        name: ArgName::parse(name).expect("arg name"),
        value: value.to_owned(),
    }
}

fn cat(c: DataCategory) -> RequestedData {
    RequestedData::Category(c)
}

fn scope() -> InferenceScope {
    serde_json::from_value(serde_json::json!({
        "provider_id": "openrouter",
        "model_id": "openai/gpt-4o:free",
        "routing_policy": {"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": ["openrouter"]},
        "source_ids": ["01234567-89ab-4cde-8f01-23456789abcd"],
        "redaction_version": "2026.10-a"
    }))
    .expect("inference scope")
}

/// A request with every context field empty and a neutral purpose.
fn request(capability: Capability, operation: &str) -> PermissionRequest {
    PermissionRequest {
        capability,
        operation: OperationName::parse(operation).expect("operation"),
        arguments: Vec::new(),
        workflow: None,
        run_id: None,
        repo_ids: Vec::new(),
        destinations: Vec::new(),
        resource_constraints: Vec::new(),
        data: Vec::new(),
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: ApprovalLifetime::Once,
        validity_seconds: None,
        inference_scope: None,
    }
}

fn digest(r: &PermissionRequest) -> String {
    fingerprint(r).expect("fingerprint").as_str().to_owned()
}

// ---------------------------------------------------------------------------------------------------------
// Golden vectors (pinned hex; the same inputs are in the independent Python script).
// ---------------------------------------------------------------------------------------------------------

fn v1() -> PermissionRequest {
    let mut r = request(Capability::GitAcquire, "git.clone");
    r.arguments = vec![arg("branch", "main"), arg("depth", "1")];
    r.repo_ids = vec![repo(R1)];
    r.destinations = vec![dest("https://github.com/example/repo")];
    r.resource_constraints = vec![sanitise("max_size_mb=100")];
    r.data = vec![cat(DataCategory::RepositoryMetadata)];
    r
}

fn v2() -> PermissionRequest {
    request(Capability::LocalRender, "render.template")
}

fn v3() -> PermissionRequest {
    let mut r = request(Capability::ModelTransmission, "chat.send");
    r.arguments = vec![arg("model", "openai/gpt-4o:free")];
    r.repo_ids = vec![repo(R2), repo(R1)];
    r.destinations = vec![dest("https://openrouter.ai/api/v1")];
    r.resource_constraints = vec![sanitise("max_tokens=2048"), sanitise("max_requests=1")];
    r.data = vec![
        cat(DataCategory::RedactedSource),
        cat(DataCategory::RedactedFindings),
        cat(DataCategory::RedactedSource),
    ];
    r
}

fn v4() -> PermissionRequest {
    let mut r = request(Capability::GitUpdate, "git.fetch");
    r.arguments = vec![
        arg("remote", "origin"),
        arg("prune", "true"),
        arg("ref", "refs/heads/main"),
    ];
    r.repo_ids = vec![repo(R3), repo(R1), repo(R2)];
    r.destinations = vec![
        dest("ssh://git.example.org:2222/team/repo.git"),
        dest("https://example.com"),
    ];
    r.data = vec![
        cat(DataCategory::AppMetadata),
        cat(DataCategory::PublicArtifactIdentifiers),
    ];
    r
}

fn v5() -> PermissionRequest {
    let mut r = request(Capability::SchemaFetch, "schema.fetch");
    r.arguments = vec![
        arg(
            "title",
            "za\u{17c}\u{f3}\u{142}\u{107} g\u{119}\u{15b}l\u{105}",
        ),
        arg("empty", ""),
    ];
    r.destinations = vec![dest("https://schemas.example.com/v1/schema.json")];
    r.resource_constraints = vec![sanitise("caf\u{e9} limit")];
    r.data = vec![cat(DataCategory::SchemaDocuments)];
    r
}

fn v6() -> PermissionRequest {
    let mut r = request(Capability::ExternalReference, "ref.fetch");
    r.repo_ids = vec![repo(R1)];
    r.destinations = vec![dest("https://example.com:8443/docs")];
    r.data = vec![
        RequestedData::RawSecretValues,
        cat(DataCategory::RedactedChatContext),
    ];
    r
}

fn v7() -> PermissionRequest {
    let mut r = request(Capability::VulnerabilityLookup, "vuln.query");
    r.arguments = vec![arg("package", "left-pad"), arg("version", "1.3.0")];
    r.repo_ids = vec![repo(R1)];
    r.destinations = vec![dest("https://api.osv.dev/v1/query")];
    r.resource_constraints = vec![sanitise("rate=low")];
    r.data = ALL_CATEGORIES.iter().map(|c| cat(*c)).collect();
    r
}

/// Fails if the encoding changes in any way (prefix, integer width or endianness, sort, dedupe, field
/// order, string bytes). The expected values come from the independent Python script.
#[test]
fn golden_vectors_match_the_independent_implementation() {
    let vectors: [(&str, PermissionRequest, &str); 7] = [
        (
            "V1",
            v1(),
            "a602941b5c4d15d92b28ea5f4c305b526947fda4649176330546c69537ae9e5c",
        ),
        (
            "V2",
            v2(),
            "02000fa2adccc8bb04e1627fc18ef56d7eaf1b3621844b096549c893463e855b",
        ),
        (
            "V3",
            v3(),
            "4508c1fd50e8a08e3742499c424f6b40290dd4fd42e003e397e3a26f5d6cecea",
        ),
        (
            "V4",
            v4(),
            "8f9b9dfbcaffd3766f189dc70051779ae88c8c543eaa28ac16ed82b638ac2dd4",
        ),
        (
            "V5",
            v5(),
            "dd9f64515a83da293d96c34e893c3f88e2e6eb9e39493ae8ebee366af00dd2bd",
        ),
        (
            "V6",
            v6(),
            "cc9db5e3830962b2f0f2c4cc64bdff845208180d36ac40934f2ec902280e08d9",
        ),
        (
            "V7",
            v7(),
            "a3eec4c5cf42f3a890d03dc07f65971ca8275df2c2b57e50bc4b8c9e8166de8c",
        ),
    ];
    for (name, req, expected) in &vectors {
        assert_eq!(digest(req), *expected, "vector {name}");
    }
}

/// Fails if the digest is not 64 lowercase hex characters (the P02 `Digest` shape) or if two different
/// vectors collide.
#[test]
fn digests_are_lowercase_hex_and_distinct() {
    let all = [v1(), v2(), v3(), v4(), v5(), v6(), v7()].map(|r| digest(&r));
    for d in &all {
        assert_eq!(d.len(), 64);
        assert!(d.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    }
    let unique: std::collections::BTreeSet<_> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
}

/// Fails if the domain-separation prefix constant changes.
#[test]
fn prefix_constant_is_pinned() {
    assert_eq!(FINGERPRINT_PREFIX, b"pwt-operation-fingerprint\x00v1\x00");
    assert_eq!(FINGERPRINT_PREFIX.len(), 29);
}

// ---------------------------------------------------------------------------------------------------------
// Reference encoder: the specification text re-implemented in the test, with serde-derived names.
// ---------------------------------------------------------------------------------------------------------

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&u32::try_from(s.len()).expect("len").to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn serde_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .expect("serialise")
        .as_str()
        .expect("string")
        .to_owned()
}

/// `SHA-256(prefix || str(cap) || str(op) || 0 args || 0 repos || 0 dests || 0 constraints || list(data))`.
fn reference_digest(capability: &str, operation: &str, data: &[&str]) -> String {
    let mut bytes = b"pwt-operation-fingerprint\x00v1\x00".to_vec();
    put_str(&mut bytes, capability);
    put_str(&mut bytes, operation);
    for _ in 0..4 {
        bytes.extend_from_slice(&0u32.to_be_bytes());
    }
    let mut items: Vec<Vec<u8>> = data
        .iter()
        .map(|d| {
            let mut item = Vec::new();
            put_str(&mut item, d);
            item
        })
        .collect();
    items.sort();
    items.dedup();
    bytes.extend_from_slice(&u32::try_from(items.len()).expect("count").to_be_bytes());
    for item in &items {
        bytes.extend_from_slice(item);
    }
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Fails if the capability string used in F1 is not the serde `snake_case` name for any of the 11
/// capabilities (a hard-coded table that drifted from the enum).
#[test]
fn capability_names_equal_the_serde_names() {
    assert_eq!(Capability::ALL.len(), 11);
    for capability in Capability::ALL {
        let req = request(capability, "op");
        let expected = reference_digest(&serde_name(&capability), "op", &[]);
        assert_eq!(digest(&req), expected, "{capability:?}");
    }
}

/// Fails if the F7 string of any of the 7 data categories is not its serde `snake_case` name, or if
/// `RawSecretValues` is not `raw_secret_values`.
#[test]
fn data_category_names_equal_the_serde_names() {
    for category in ALL_CATEGORIES {
        let mut req = request(Capability::LocalRender, "op");
        req.data = vec![cat(category)];
        let expected = reference_digest("local_render", "op", &[&serde_name(&category)]);
        assert_eq!(digest(&req), expected, "{category:?}");
    }
    let mut req = request(Capability::LocalRender, "op");
    req.data = vec![RequestedData::RawSecretValues];
    assert_eq!(
        digest(&req),
        reference_digest("local_render", "op", &["raw_secret_values"])
    );
}

/// Fails if items are sorted by their text instead of their encoded bytes. The length prefix comes first,
/// so `redacted_source` (15 bytes) sorts before `redacted_findings` (17 bytes) although the text order is
/// the other way round; the reference encoder sorts the encoded bytes.
#[test]
fn sort_order_is_by_encoded_bytes_not_by_text() {
    assert!(
        "redacted_findings" < "redacted_source",
        "premise: text order"
    );
    let mut req = request(Capability::LocalRender, "op");
    req.data = vec![
        cat(DataCategory::RedactedFindings),
        cat(DataCategory::RedactedSource),
    ];
    let expected = reference_digest(
        "local_render",
        "op",
        &["redacted_source", "redacted_findings"],
    );
    assert_eq!(digest(&req), expected);
    // A text-ordered encoding would hash a different byte string.
    let mut text_ordered = b"pwt-operation-fingerprint\x00v1\x00".to_vec();
    put_str(&mut text_ordered, "local_render");
    put_str(&mut text_ordered, "op");
    for _ in 0..4 {
        text_ordered.extend_from_slice(&0u32.to_be_bytes());
    }
    text_ordered.extend_from_slice(&2u32.to_be_bytes());
    put_str(&mut text_ordered, "redacted_findings");
    put_str(&mut text_ordered, "redacted_source");
    let text_digest: String = Sha256::digest(&text_ordered)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_ne!(digest(&req), text_digest);
}

// ---------------------------------------------------------------------------------------------------------
// Permutation, duplicates, field sensitivity, exclusions.
// ---------------------------------------------------------------------------------------------------------

/// Fails if any list is order-sensitive.
#[test]
fn permuted_lists_give_the_same_digest() {
    let a = v4();
    let mut b = v4();
    b.arguments.reverse();
    b.repo_ids.reverse();
    b.destinations.reverse();
    b.data.reverse();
    assert_ne!(a.arguments, b.arguments, "premise: the order changed");
    assert_eq!(digest(&a), digest(&b));

    let c = v3();
    let mut d = v3();
    d.resource_constraints.reverse();
    d.repo_ids.reverse();
    assert_eq!(digest(&c), digest(&d));
}

/// Fails if a repeated list item changes the digest (items are de-duplicated before counting), including
/// two spellings of one destination.
#[test]
fn duplicates_in_lists_give_the_same_digest() {
    let base = digest(&v1());
    let mut r = v1();
    r.repo_ids.push(repo(R1));
    r.destinations
        .push(dest("HTTPS://GitHub.com./example/repo"));
    r.resource_constraints.push(sanitise("max_size_mb=100"));
    r.data.push(cat(DataCategory::RepositoryMetadata));
    assert_eq!(r.repo_ids.len(), 2);
    assert_eq!(digest(&r), base);
}

/// Fails if any one of F1-F7 is left out of the digest: each mutation below changes exactly one field.
#[test]
fn each_hashed_field_changes_the_digest() {
    let base = digest(&v1());
    let mutations: Vec<(&str, Mutator)> = vec![
        (
            "F1 capability",
            Box::new(|r: &mut PermissionRequest| r.capability = Capability::GitUpdate),
        ),
        (
            "F2 operation",
            Box::new(|r: &mut PermissionRequest| {
                r.operation = OperationName::parse("git.clone2").expect("op")
            }),
        ),
        (
            "F3 argument value",
            Box::new(|r: &mut PermissionRequest| r.arguments[0].value = "dev".to_owned()),
        ),
        (
            "F3 argument name",
            Box::new(|r: &mut PermissionRequest| {
                r.arguments[0].name = ArgName::parse("branch2").expect("n")
            }),
        ),
        (
            "F3 argument added",
            Box::new(|r: &mut PermissionRequest| r.arguments.push(arg("extra", "x"))),
        ),
        (
            "F3 argument removed",
            Box::new(|r: &mut PermissionRequest| {
                r.arguments.pop();
            }),
        ),
        (
            "F4 repo changed",
            Box::new(|r: &mut PermissionRequest| r.repo_ids = vec![repo(R2)]),
        ),
        (
            "F4 repo added",
            Box::new(|r: &mut PermissionRequest| r.repo_ids.push(repo(R2))),
        ),
        (
            "F4 repo removed",
            Box::new(|r: &mut PermissionRequest| r.repo_ids.clear()),
        ),
        (
            "F5 destination host",
            Box::new(|r: &mut PermissionRequest| {
                r.destinations = vec![dest("https://gitlab.com/example/repo")]
            }),
        ),
        (
            "F5 destination port",
            Box::new(|r: &mut PermissionRequest| {
                r.destinations = vec![dest("https://github.com:8443/example/repo")]
            }),
        ),
        (
            "F5 destination path",
            Box::new(|r: &mut PermissionRequest| {
                r.destinations = vec![dest("https://github.com/example/other")]
            }),
        ),
        (
            "F5 destination scheme",
            Box::new(|r: &mut PermissionRequest| {
                r.destinations = vec![dest("ssh://github.com:443/example/repo")]
            }),
        ),
        (
            "F5 destination added",
            Box::new(|r: &mut PermissionRequest| r.destinations.push(dest("https://example.com"))),
        ),
        (
            "F6 constraint changed",
            Box::new(|r: &mut PermissionRequest| {
                r.resource_constraints = vec![sanitise("max_size_mb=101")]
            }),
        ),
        (
            "F6 constraint added",
            Box::new(|r: &mut PermissionRequest| r.resource_constraints.push(sanitise("x"))),
        ),
        (
            "F7 category changed",
            Box::new(|r: &mut PermissionRequest| r.data = vec![cat(DataCategory::AppMetadata)]),
        ),
        (
            "F7 category added",
            Box::new(|r: &mut PermissionRequest| r.data.push(cat(DataCategory::AppMetadata))),
        ),
        (
            "F7 raw secret added",
            Box::new(|r: &mut PermissionRequest| r.data.push(RequestedData::RawSecretValues)),
        ),
        (
            "F7 emptied",
            Box::new(|r: &mut PermissionRequest| r.data.clear()),
        ),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for (name, mutate) in &mutations {
        let mut r = v1();
        mutate(&mut r);
        let changed = digest(&r);
        assert_ne!(changed, base, "{name} must change the digest");
        seen.insert(changed);
    }
    assert_eq!(
        seen.len(),
        mutations.len(),
        "each mutation gives its own digest"
    );
}

/// Fails if a field the spec excludes (workflow binding, run, lifetime, validity, purpose, inference
/// scope) leaks into the digest.
#[test]
fn excluded_fields_do_not_change_the_digest() {
    let base = digest(&v3());
    let mutations: Vec<(&str, Mutator)> = vec![
        (
            "workflow binding",
            Box::new(|r: &mut PermissionRequest| {
                r.workflow = Some(WorkflowBinding {
                    workflow_id: WorkflowId::parse(WF).expect("workflow"),
                    revision: 7,
                });
            }),
        ),
        (
            "workflow revision",
            Box::new(|r: &mut PermissionRequest| {
                r.workflow = Some(WorkflowBinding {
                    workflow_id: WorkflowId::parse(WF).expect("workflow"),
                    revision: 8,
                });
            }),
        ),
        (
            "run",
            Box::new(|r: &mut PermissionRequest| r.run_id = Some(RunId::parse(RUN).expect("run"))),
        ),
        (
            "lifetime run",
            Box::new(|r: &mut PermissionRequest| r.lifetime = ApprovalLifetime::Run),
        ),
        (
            "lifetime workflow_revision",
            Box::new(|r: &mut PermissionRequest| r.lifetime = ApprovalLifetime::WorkflowRevision),
        ),
        (
            "validity",
            Box::new(|r: &mut PermissionRequest| r.validity_seconds = Some(3600)),
        ),
        (
            "purpose",
            Box::new(|r: &mut PermissionRequest| r.purpose = FixedText::ActorRequester.redacted()),
        ),
        (
            "inference scope",
            Box::new(|r: &mut PermissionRequest| r.inference_scope = Some(scope())),
        ),
    ];
    for (name, mutate) in &mutations {
        let mut r = v3();
        mutate(&mut r);
        assert_eq!(digest(&r), base, "{name} must not change the digest");
    }
}

/// Fails if the encoding is ambiguous at a boundary: moving a byte between two adjacent strings, moving
/// an item between two lists, or merging two items must change the digest (length prefixes and counts).
#[test]
fn boundaries_are_unambiguous() {
    let with_args = |pairs: &[(&str, &str)]| {
        let mut r = request(Capability::LocalRender, "op");
        r.arguments = pairs.iter().map(|(n, v)| arg(n, v)).collect();
        digest(&r)
    };
    assert_ne!(with_args(&[("ab", "c")]), with_args(&[("a", "bc")]));
    assert_ne!(
        with_args(&[("a", "b"), ("c", "d")]),
        with_args(&[("a", "bcd")])
    );

    let with_constraints = |texts: &[&str]| {
        let mut r = request(Capability::LocalRender, "op");
        r.resource_constraints = texts.iter().map(|t| sanitise(t)).collect();
        digest(&r)
    };
    assert_ne!(with_constraints(&["ab", "c"]), with_constraints(&["abc"]));
    assert_ne!(with_constraints(&["a", "b"]), with_constraints(&["ab"]));
    assert_ne!(with_constraints(&[""]), with_constraints(&[]));

    // The same text in a different list (constraint "https://example.com:443" against a destination).
    let mut as_constraint = request(Capability::LocalRender, "op");
    as_constraint.resource_constraints = vec![sanitise("https://example.com:443")];
    let mut as_destination = request(Capability::LocalRender, "op");
    as_destination.destinations = vec![dest("https://example.com")];
    assert_ne!(digest(&as_constraint), digest(&as_destination));

    // Operation and capability are separate strings.
    let a = request(Capability::GitAcquire, "x");
    let b = request(Capability::GitUpdate, "x");
    assert_ne!(digest(&a), digest(&b));
}

/// Fails if multi-byte text is length-prefixed by characters instead of bytes, or if the empty argument
/// value is dropped (V5 pins both; this adds a direct comparison).
#[test]
fn lengths_are_bytes_and_empty_values_count() {
    let mut a = request(Capability::LocalRender, "op");
    a.arguments = vec![arg("k", "")];
    let b = request(Capability::LocalRender, "op");
    assert_ne!(digest(&a), digest(&b));

    let mut c = request(Capability::LocalRender, "op");
    c.arguments = vec![arg("k", "\u{e9}")];
    let mut d = request(Capability::LocalRender, "op");
    d.arguments = vec![arg("k", "e")];
    assert_ne!(digest(&c), digest(&d));
}

// ---------------------------------------------------------------------------------------------------------
// Errors.
// ---------------------------------------------------------------------------------------------------------

/// Fails if two arguments with one name are accepted, whether their values are equal or not and wherever
/// they stand in the list.
#[test]
fn duplicate_argument_names_are_an_error() {
    let cases: &[&[(&str, &str)]] = &[
        &[("a", "1"), ("a", "2")],
        &[("a", "1"), ("a", "1")],
        &[("a", "1"), ("b", "2"), ("c", "3"), ("a", "4")],
    ];
    for pairs in cases {
        let mut r = request(Capability::LocalRender, "op");
        r.arguments = pairs.iter().map(|(n, v)| arg(n, v)).collect();
        assert_eq!(fingerprint(&r), Err(ArgError::Duplicate), "{pairs:?}");
    }
    // Names that differ only in position or case of characters are not duplicates (names are lower-case
    // by construction; `a.b` and `a-b` are distinct).
    let mut ok = request(Capability::LocalRender, "op");
    ok.arguments = vec![arg("a.b", "1"), arg("a-b", "1"), arg("a_b", "1")];
    assert!(fingerprint(&ok).is_ok());
}

/// Fails if the argument value bound is not `MAX_ARG_VALUE_BYTES` (65 536) bytes, counted in bytes.
#[test]
fn argument_value_bound() {
    assert_eq!(MAX_ARG_VALUE_BYTES, 65_536);
    let mut at = request(Capability::LocalRender, "op");
    at.arguments = vec![arg("k", &"a".repeat(MAX_ARG_VALUE_BYTES))];
    assert!(fingerprint(&at).is_ok());
    let mut over = request(Capability::LocalRender, "op");
    over.arguments = vec![arg("k", &"a".repeat(MAX_ARG_VALUE_BYTES + 1))];
    assert_eq!(fingerprint(&over), Err(ArgError::TooLong));
    // 32 769 two-byte characters are 65 538 bytes.
    let mut bytes = request(Capability::LocalRender, "op");
    bytes.arguments = vec![arg("k", &"\u{e9}".repeat(MAX_ARG_VALUE_BYTES / 2 + 1))];
    assert_eq!(fingerprint(&bytes), Err(ArgError::TooLong));
}

/// Fails if an over-long value is masked by a duplicate-name error or the other way round: names and sizes
/// are checked first in argument order, duplicates after.
#[test]
fn error_precedence_is_pinned() {
    let mut both = request(Capability::LocalRender, "op");
    both.arguments = vec![
        arg("a", "1"),
        arg("a", "2"),
        arg("big", &"a".repeat(MAX_ARG_VALUE_BYTES + 1)),
    ];
    assert_eq!(fingerprint(&both), Err(ArgError::TooLong));
}

/// Fails if name validation (re-run by the fingerprint) accepts a name outside `[a-z0-9._-]{1,64}`.
#[test]
fn name_parsers_reject_bad_names() {
    assert_eq!(ArgName::parse(""), Err(ArgError::Empty));
    assert_eq!(ArgName::parse(&"a".repeat(65)), Err(ArgError::TooLong));
    assert!(ArgName::parse(&"a".repeat(64)).is_ok());
    for bad in ["A", "a b", "a/b", "a:b", "\u{e9}", "a\n", "a=b", "a,b"] {
        assert_eq!(ArgName::parse(bad), Err(ArgError::Character), "{bad:?}");
        assert_eq!(
            OperationName::parse(bad),
            Err(ArgError::Character),
            "{bad:?}"
        );
    }
    assert_eq!(OperationName::parse(""), Err(ArgError::Empty));
    assert_eq!(
        OperationName::parse(&"a".repeat(65)),
        Err(ArgError::TooLong)
    );
}

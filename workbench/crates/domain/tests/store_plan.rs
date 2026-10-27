//! P02-T05a plan and provider store records (`planning/P02-T01-request.md` §5.1, §9). All data is synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use domain::contract::{AdapterId, SelectorMode};
use domain::store::plan::{
    DiscoveryState, EngineAvailability, EngineSnapshot, FileRecord, PlanSnapshot, RefKind,
    RendererSetting, RepoTarget, ScanSettings, SchemaBinding, SchemaMode, SecretScope, Selector,
    TargetSnapshot, TemplateMode,
};
use domain::store::provider::{InferenceScope, Provider, ProviderCredentialMode, ProviderPolicy};
use domain::store::values::Identifier;
use domain::text::RedactedText;
use domain::vocab::Category;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");

const REPO: &str = "526a9739-68cb-5ef8-8c3a-0fda8a207475";
const REPO2: &str = "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
const WORKFLOW: &str = "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d";
const FILE: &str = "11111111-2222-4333-8444-555555555555";
const FILE2: &str = "66666666-7777-4888-9999-aaaaaaaaaaaa";
const APPROVAL: &str = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";
const SOURCE: &str = "01234567-89ab-4cde-8f01-23456789abcd";
const SOURCE2: &str = "76543210-ba98-4edc-8fed-cba987654321";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OID: &str = "0123456789abcdef0123456789abcdef01234567";
const TS: &str = "2026-10-16T05:25:00Z";
const TS_FRACTION: &str = "2026-10-16T05:25:00.123456789Z";

fn schema() -> Value {
    serde_json::from_str(SCHEMA).expect("schema file is JSON")
}

fn def_properties(def: &str) -> BTreeSet<String> {
    let schema = schema();
    let props = schema["$defs"][def]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{def} has no properties"));
    props.keys().cloned().collect()
}

fn def_required(def: &str) -> BTreeSet<String> {
    let schema = schema();
    let req = schema["$defs"][def]["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{def} has no required list"));
    req.iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect()
}

fn schema_enum(def: &str, prop: &str) -> Vec<String> {
    let schema = schema();
    let list = schema["$defs"][def]["properties"][prop]["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{def}.{prop} has no enum"));
    list.iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect()
}

fn serialised_all<T: Serialize>(all: &[T]) -> Vec<String> {
    all.iter()
        .map(|v| {
            serde_json::to_value(v)
                .expect("serialise")
                .as_str()
                .expect("string")
                .to_owned()
        })
        .collect()
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
}

fn with(v: &Value, key: &str, value: Value) -> Value {
    let mut out = v.clone();
    out.as_object_mut()
        .expect("object")
        .insert(key.to_owned(), value);
    out
}

fn without(v: &Value, key: &str) -> Value {
    let mut out = v.clone();
    out.as_object_mut().expect("object").remove(key);
    out
}

/// One `$def` under test.
struct Def {
    name: &'static str,
    minimal: Value,
    maximal: Value,
    /// Required keys whose value may be `null`; they are `null` in `minimal` and must stay present.
    nullable: &'static [&'static str],
    /// Top-level replacements that must fail to decode (wrong enum, constant, newtype or integer).
    bad: Vec<(&'static str, Value)>,
}

fn check<T>(d: &Def)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    // Minimal and maximal instances round-trip to an equal Value, also through JSON text.
    for (label, inst) in [("minimal", &d.minimal), ("maximal", &d.maximal)] {
        let typed: T = serde_json::from_value(inst.clone())
            .unwrap_or_else(|e| panic!("{} {label}: {e}", d.name));
        let back = serde_json::to_value(&typed).expect("serialise");
        assert_eq!(&back, inst, "{} {label}", d.name);
        let text = serde_json::to_string(&typed).expect("serialise");
        let again: T = serde_json::from_str(&text).expect("decode text");
        assert_eq!(again, typed, "{} {label} text", d.name);
    }

    // An unknown field is rejected, at the top level.
    for inst in [&d.minimal, &d.maximal] {
        let extra = with(inst, "unexpected_field", Value::Null);
        assert!(
            serde_json::from_value::<T>(extra).is_err(),
            "{}: unknown field accepted",
            d.name
        );
    }

    // A required nullable key serialises as null and is never skipped.
    let typed: T = serde_json::from_value(d.minimal.clone()).expect("minimal");
    let back = serde_json::to_value(&typed).expect("serialise");
    for key in d.nullable {
        assert_eq!(back.get(*key), Some(&Value::Null), "{}.{key}", d.name);
    }

    // A missing required non-nullable key is rejected.
    for key in keys(&d.minimal) {
        if d.nullable.contains(&key.as_str()) {
            continue;
        }
        assert!(
            serde_json::from_value::<T>(without(&d.minimal, &key)).is_err(),
            "{}: missing {key} accepted",
            d.name
        );
    }

    // Wrong enum, constant, newtype or integer values are rejected.
    assert!(!d.bad.is_empty(), "{}: no negative values", d.name);
    for (key, bad) in &d.bad {
        for inst in [&d.minimal, &d.maximal] {
            assert!(
                serde_json::from_value::<T>(with(inst, key, bad.clone())).is_err(),
                "{}.{key} accepted {bad}",
                d.name
            );
        }
    }

    // Schema cross-check: the serialised key sets equal the `$def` properties (all required).
    let max_typed: T = serde_json::from_value(d.maximal.clone()).expect("maximal");
    let max_keys = keys(&serde_json::to_value(&max_typed).expect("serialise"));
    assert_eq!(max_keys, def_properties(d.name), "{} properties", d.name);
    assert_eq!(keys(&back), def_properties(d.name), "{} minimal", d.name);
    assert_eq!(
        def_required(d.name),
        def_properties(d.name),
        "{} required",
        d.name
    );
}

// --- instances ------------------------------------------------------------------------------------------

/// `selector` of the Data Schema §9 example.
fn selector_example() -> Value {
    json!({"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []})
}

fn selector_max() -> Value {
    json!({
        "mode": "patterns",
        "paths": ["a/b.yaml", "c.yml"],
        "include_patterns": ["**/*.yaml", "charts/*/values.yaml", "[ab]*.yml"],
        "exclude_patterns": ["vendor/**"]
    })
}

fn target_min() -> Value {
    json!({"repo_id": REPO, "ref_kind": "branch", "ref_value": "main", "selector": selector_example()})
}

fn target_max() -> Value {
    json!({"repo_id": REPO2, "ref_kind": "commit", "ref_value": OID, "selector": selector_max()})
}

fn binding_min() -> Value {
    json!({"path_pattern": "**/*.yaml", "schema_resource": "github-workflow", "dialect": null, "expected_digest": null})
}

fn binding_max() -> Value {
    json!({
        "path_pattern": "deploy/*.yaml",
        "schema_resource": "bundled:schemas/k8s.json",
        "dialect": "draft-2020-12",
        "expected_digest": DIGEST
    })
}

fn renderer_min() -> Value {
    json!({"framework": "helm", "renderer_id": "helm", "input_paths": [], "parameters_redacted": {}})
}

fn renderer_max() -> Value {
    json!({
        "framework": "helm",
        "renderer_id": "helm:3.14",
        "input_paths": ["charts/app/values.yaml", "charts/app/Chart.yaml"],
        "parameters_redacted": {"namespace": "default", "release": "demo"}
    })
}

/// `scan_settings` of the Data Schema §9 example.
fn scan_example() -> Value {
    json!({
        "categories": ["syntax", "schema", "secrets", "misconfiguration", "vulnerability"],
        "secret_scope": "checkout",
        "template_mode": "both",
        "schema_mode": "both",
        "schema_bindings": [],
        "renderers": [],
        "allow_reference_context": true,
        "adapter_ids": []
    })
}

fn scan_max() -> Value {
    json!({
        "categories": [
            "syntax", "schema", "secrets", "misconfiguration", "vulnerability",
            "external_verification", "style", "workflow_validation", "rendering"
        ],
        "secret_scope": "history",
        "template_mode": "rendered",
        "schema_mode": "user_supplied",
        "schema_bindings": [binding_min(), binding_max()],
        "renderers": [renderer_min(), renderer_max()],
        "allow_reference_context": false,
        "adapter_ids": [
            "git", "yamllint", "gitleaks", "checkov", "actionlint", "kubeconform", "zizmor",
            "docker-compose", "helm", "trivy", "rust-schema"
        ]
    })
}

fn file_min() -> Value {
    json!({
        "file_id": FILE,
        "path": ".github/workflows/ci.yml",
        "byte_size": 0,
        "content_digest": null,
        "commit_oid": OID,
        "detected_framework": null,
        "discovery_state": "eligible",
        "reason": null
    })
}

fn file_max() -> Value {
    json!({
        "file_id": FILE2,
        "path": "charts/app/values.yaml",
        "byte_size": 18_446_744_073_709_551_615u64,
        "content_digest": DIGEST,
        "commit_oid": DIGEST2,
        "detected_framework": "github-actions",
        "discovery_state": "excluded",
        "reason": "Excluded by pattern vendor/**"
    })
}

fn target_snapshot_min() -> Value {
    json!({
        "repo_id": REPO,
        "selected_ref": "main",
        "commit_oid": null,
        "inventory_revision": 0,
        "selected_files": [],
        "missing_paths": [],
        "snapshot_at": TS
    })
}

fn target_snapshot_max() -> Value {
    json!({
        "repo_id": REPO2,
        "selected_ref": "release/2026.10",
        "commit_oid": OID,
        "inventory_revision": 7,
        "selected_files": [file_min(), file_max()],
        "missing_paths": ["gone.yaml", "dir/also-gone.yml"],
        "snapshot_at": TS_FRACTION
    })
}

fn engine_min() -> Value {
    json!({
        "adapter_id": "yamllint",
        "adapter_version": "1.0",
        "tool_name": "yamllint",
        "tool_version": null,
        "ruleset_version": null,
        "config_digest": null,
        "manifest_digest": DIGEST,
        "availability": "missing",
        "data_updated_at": null
    })
}

fn engine_max() -> Value {
    json!({
        "adapter_id": "rust-schema",
        "adapter_version": "0.1.0+local",
        "tool_name": "rust-schema",
        "tool_version": "1.35.1",
        "ruleset_version": "2026.10",
        "config_digest": DIGEST2,
        "manifest_digest": DIGEST,
        "availability": "unconfigured",
        "data_updated_at": TS_FRACTION
    })
}

fn plan_min() -> Value {
    json!({
        "workflow_id": null,
        "workflow_revision": null,
        "targets": [],
        "scan_settings": scan_example(),
        "approval_ids": [],
        "created_at": TS,
        "plan_digest": DIGEST,
        "target_snapshots": [],
        "engine_snapshots": []
    })
}

fn plan_max() -> Value {
    json!({
        "workflow_id": WORKFLOW,
        "workflow_revision": 3,
        "targets": [target_min(), target_max()],
        "scan_settings": scan_max(),
        "approval_ids": [APPROVAL],
        "created_at": TS_FRACTION,
        "plan_digest": DIGEST2,
        "target_snapshots": [target_snapshot_min(), target_snapshot_max()],
        "engine_snapshots": [engine_min(), engine_max()]
    })
}

fn policy_min() -> Value {
    json!({"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": []})
}

fn policy_max() -> Value {
    json!({
        "data_collection": "deny",
        "zdr": true,
        "allow_fallbacks": false,
        "only": ["openrouter", "anthropic"]
    })
}

fn provider_min() -> Value {
    json!({
        "provider_id": "openrouter",
        "kind": "api",
        "endpoint": "https://openrouter.ai/api/v1",
        "model_id": "anthropic/claude-sonnet-5",
        "credential_mode": "session_only",
        "credential_reference": null,
        "routing_policy": policy_min()
    })
}

fn provider_max() -> Value {
    json!({
        "provider_id": "openrouter",
        "kind": "api",
        "endpoint": "https://openrouter.ai/api/v1",
        "model_id": "openai/gpt-4o:free",
        "credential_mode": "os_credential_store",
        "credential_reference": "keychain:pwt.openrouter",
        "routing_policy": policy_max()
    })
}

fn scope_min() -> Value {
    json!({
        "provider_id": "openrouter",
        "model_id": "anthropic/claude-sonnet-5",
        "routing_policy": policy_min(),
        "source_ids": [],
        "redaction_version": "v1"
    })
}

fn scope_max() -> Value {
    json!({
        "provider_id": "openrouter",
        "model_id": "openai/gpt-4o:free",
        "routing_policy": policy_max(),
        "source_ids": [SOURCE, SOURCE2],
        "redaction_version": "2026.10-a"
    })
}

// --- per-def tests --------------------------------------------------------------------------------------

#[test]
fn selector() {
    check::<Selector>(&Def {
        name: "Selector",
        minimal: selector_example(),
        maximal: selector_max(),
        nullable: &[],
        bad: vec![
            ("mode", json!("bogus")),
            ("mode", json!("ALL_YAML")),
            ("paths", json!(["../escape.yaml"])),
            ("paths", json!(["/abs.yaml"])),
            ("include_patterns", json!(["{a,b}"])),
            ("exclude_patterns", json!(["$(cmd)"])),
            ("paths", json!("a.yaml")),
        ],
    });
}

#[test]
fn repo_target() {
    check::<RepoTarget>(&Def {
        name: "RepoTarget",
        minimal: target_min(),
        maximal: target_max(),
        nullable: &[],
        bad: vec![
            ("repo_id", json!("not-a-uuid")),
            ("repo_id", json!("526A9739-68CB-5EF8-8C3A-0FDA8A207475")),
            ("ref_kind", json!("bogus")),
            ("ref_value", json!("a..b")),
            ("ref_value", json!("")),
            ("selector", json!({"mode": "bogus"})),
        ],
    });
}

#[test]
fn schema_binding() {
    check::<SchemaBinding>(&Def {
        name: "SchemaBinding",
        minimal: binding_min(),
        maximal: binding_max(),
        nullable: &["dialect", "expected_digest"],
        bad: vec![
            ("path_pattern", json!("a;b")),
            ("schema_resource", json!("has space")),
            ("dialect", json!("a b")),
            ("expected_digest", json!("ABC")),
            ("expected_digest", json!(5)),
        ],
    });
}

#[test]
fn renderer_setting() {
    check::<RendererSetting>(&Def {
        name: "RendererSetting",
        minimal: renderer_min(),
        maximal: renderer_max(),
        nullable: &[],
        bad: vec![
            ("framework", json!("Helm")),
            ("renderer_id", json!("a b")),
            ("input_paths", json!(["../x"])),
            ("parameters_redacted", json!({"bad key": "v"})),
            ("parameters_redacted", json!({"k": 1})),
            ("parameters_redacted", json!(["k"])),
        ],
    });
}

#[test]
fn scan_settings() {
    check::<ScanSettings>(&Def {
        name: "ScanSettings",
        minimal: scan_example(),
        maximal: scan_max(),
        nullable: &[],
        bad: vec![
            ("categories", json!(["bogus"])),
            ("secret_scope", json!("everything")),
            ("template_mode", json!("neither")),
            ("schema_mode", json!("manual")),
            ("schema_bindings", json!([{"path_pattern": "x"}])),
            ("adapter_ids", json!(["nmap"])),
            ("adapter_ids", json!(["Git"])),
            ("allow_reference_context", json!("yes")),
        ],
    });
}

#[test]
fn file_record() {
    check::<FileRecord>(&Def {
        name: "FileRecord",
        minimal: file_min(),
        maximal: file_max(),
        nullable: &["content_digest", "detected_framework", "reason"],
        bad: vec![
            ("file_id", json!("nope")),
            ("path", json!("../x")),
            ("byte_size", json!(-1)),
            ("byte_size", json!(1.5)),
            ("content_digest", json!("xyz")),
            ("commit_oid", json!("abc")),
            ("commit_oid", Value::Null),
            ("detected_framework", json!("GitHub Actions")),
            ("discovery_state", json!("found")),
        ],
    });
}

#[test]
fn target_snapshot() {
    check::<TargetSnapshot>(&Def {
        name: "TargetSnapshot",
        minimal: target_snapshot_min(),
        maximal: target_snapshot_max(),
        nullable: &["commit_oid"],
        bad: vec![
            ("selected_ref", json!("a b")),
            ("commit_oid", json!("123")),
            ("inventory_revision", json!(-1)),
            ("selected_files", json!([{"file_id": FILE}])),
            ("missing_paths", json!(["/abs"])),
            ("snapshot_at", json!("2026-10-16 05:25:00")),
            ("snapshot_at", json!("2026-10-16T05:25:00+00:00")),
        ],
    });
}

#[test]
fn engine_snapshot() {
    check::<EngineSnapshot>(&Def {
        name: "EngineSnapshot",
        minimal: engine_min(),
        maximal: engine_max(),
        nullable: &[
            "tool_version",
            "ruleset_version",
            "config_digest",
            "data_updated_at",
        ],
        bad: vec![
            ("adapter_id", json!("nmap")),
            ("adapter_version", json!("1 0")),
            ("tool_name", json!("YamlLint")),
            ("manifest_digest", Value::Null),
            ("manifest_digest", json!("short")),
            ("availability", json!("unavailable")),
            ("availability", json!("Ready")),
            ("data_updated_at", json!("yesterday")),
        ],
    });
}

#[test]
fn plan_snapshot() {
    check::<PlanSnapshot>(&Def {
        name: "PlanSnapshot",
        minimal: plan_min(),
        maximal: plan_max(),
        nullable: &["workflow_id", "workflow_revision"],
        bad: vec![
            ("workflow_id", json!("nope")),
            ("workflow_revision", json!(-1)),
            ("targets", json!([{"repo_id": REPO}])),
            ("scan_settings", json!({})),
            ("approval_ids", json!(["nope"])),
            ("created_at", json!("now")),
            ("plan_digest", json!("00")),
            ("target_snapshots", json!([1])),
            ("engine_snapshots", json!([{"adapter_id": "git"}])),
        ],
    });
}

#[test]
fn provider_policy() {
    check::<ProviderPolicy>(&Def {
        name: "ProviderPolicy",
        minimal: policy_min(),
        maximal: policy_max(),
        nullable: &[],
        bad: vec![
            ("data_collection", json!("allow")),
            ("data_collection", json!("DENY")),
            ("zdr", json!(false)),
            ("zdr", json!("true")),
            ("allow_fallbacks", json!(true)),
            ("only", json!(["bad provider"])),
            ("only", json!("openrouter")),
        ],
    });
}

#[test]
fn provider() {
    check::<Provider>(&Def {
        name: "Provider",
        minimal: provider_min(),
        maximal: provider_max(),
        nullable: &["credential_reference"],
        bad: vec![
            ("provider_id", json!("anthropic")),
            ("kind", json!("local")),
            ("endpoint", json!("https://example.invalid/v1")),
            ("endpoint", json!("https://openrouter.ai/api/v1/")),
            ("model_id", json!("")),
            ("model_id", json!("bad model")),
            ("credential_mode", json!("os_store")),
            ("credential_mode", json!("plaintext")),
            ("credential_reference", json!("sk-or-v1-synthetic")),
            ("credential_reference", json!("a".repeat(40))),
            ("routing_policy", json!({"data_collection": "allow"})),
        ],
    });
}

#[test]
fn inference_scope() {
    check::<InferenceScope>(&Def {
        name: "InferenceScope",
        minimal: scope_min(),
        maximal: scope_max(),
        nullable: &[],
        bad: vec![
            ("provider_id", json!("someone-else")),
            ("model_id", json!("bad model")),
            ("routing_policy", json!({"zdr": false})),
            ("source_ids", json!(["nope"])),
            ("redaction_version", json!("")),
        ],
    });
}

// --- cross-checks ---------------------------------------------------------------------------------------

#[test]
fn twelve_defs_are_covered_and_exist_in_the_schema() {
    let names = [
        "Selector",
        "RepoTarget",
        "SchemaBinding",
        "RendererSetting",
        "ScanSettings",
        "FileRecord",
        "TargetSnapshot",
        "EngineSnapshot",
        "PlanSnapshot",
        "ProviderPolicy",
        "Provider",
        "InferenceScope",
    ];
    let schema = schema();
    for n in names {
        assert!(schema["$defs"][n].is_object(), "{n} missing from schema");
        assert_eq!(
            schema["$defs"][n]["additionalProperties"],
            json!(false),
            "{n}"
        );
    }
    // The same-named test functions above are the per-def checks.
    assert_eq!(names.len(), 12);
}

#[test]
fn new_enums_reproduce_the_schema_value_sets() {
    assert_eq!(
        serialised_all(&RefKind::ALL),
        schema_enum("RepoTarget", "ref_kind")
    );
    assert_eq!(
        serialised_all(&RefKind::ALL),
        schema_enum("Repository", "default_ref_kind")
    );
    assert_eq!(
        serialised_all(&SecretScope::ALL),
        schema_enum("ScanSettings", "secret_scope")
    );
    assert_eq!(
        serialised_all(&TemplateMode::ALL),
        schema_enum("ScanSettings", "template_mode")
    );
    assert_eq!(
        serialised_all(&SchemaMode::ALL),
        schema_enum("ScanSettings", "schema_mode")
    );
    assert_eq!(
        serialised_all(&DiscoveryState::ALL),
        schema_enum("FileRecord", "discovery_state")
    );
    assert_eq!(
        serialised_all(&EngineAvailability::ALL),
        schema_enum("EngineSnapshot", "availability")
    );
    assert_eq!(
        serialised_all(&ProviderCredentialMode::ALL),
        schema_enum("Provider", "credential_mode")
    );
}

#[test]
fn reused_domain_enums_have_identical_value_sets() {
    let schema = schema();
    // Selector.mode
    let modes = schema_enum("Selector", "mode");
    assert_eq!(modes.len(), 3);
    for m in &modes {
        let parsed: SelectorMode = serde_json::from_value(json!(m)).expect("selector mode");
        assert_eq!(serde_json::to_value(parsed).expect("serialise"), json!(m));
    }
    // Category (a shared `$def`).
    let categories: Vec<String> = schema["$defs"]["Category"]["enum"]
        .as_array()
        .expect("category enum")
        .iter()
        .map(|v| v.as_str().expect("string").to_owned())
        .collect();
    assert_eq!(serialised_all(&Category::ALL), categories);
    // ScanSettings.adapter_ids is a domain closed set (Data §21); every value the models write parses back.
    for id in [
        "git",
        "yamllint",
        "gitleaks",
        "checkov",
        "actionlint",
        "kubeconform",
        "zizmor",
        "docker-compose",
        "helm",
        "trivy",
        "rust-schema",
    ] {
        let parsed: AdapterId = serde_json::from_value(json!(id)).expect("adapter id");
        assert_eq!(serde_json::to_value(parsed).expect("serialise"), json!(id));
    }
}

#[test]
fn data_schema_section_nine_objects_decode_and_reserialise() {
    // Copied from the Data Schema §9 example `workflows.json`.
    let selector: Selector = serde_json::from_str(
        r#"{"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []}"#,
    )
    .expect("selector example");
    assert_eq!(selector.mode, SelectorMode::AllYaml);
    assert_eq!(
        serde_json::to_value(&selector).expect("serialise"),
        selector_example()
    );

    let scan: ScanSettings = serde_json::from_str(
        r#"{
            "categories": ["syntax", "schema", "secrets", "misconfiguration", "vulnerability"],
            "secret_scope": "checkout",
            "template_mode": "both",
            "schema_mode": "both",
            "schema_bindings": [],
            "renderers": [],
            "allow_reference_context": true,
            "adapter_ids": []
        }"#,
    )
    .expect("scan_settings example");
    assert_eq!(scan.secret_scope, SecretScope::Checkout);
    assert_eq!(scan.categories.len(), 5);
    assert_eq!(
        serde_json::to_value(&scan).expect("serialise"),
        scan_example()
    );

    let target: RepoTarget = serde_json::from_str(
        r#"{
            "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475",
            "ref_kind": "branch",
            "ref_value": "main",
            "selector": {"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []}
        }"#,
    )
    .expect("target example");
    assert_eq!(target.ref_kind, RefKind::Branch);
}

#[test]
fn model_decode_errors_do_not_echo_planted_text() {
    const MARKER: &str = "PLANTEDMARKER";
    let bad_paths = with(
        &selector_example(),
        "paths",
        json!([format!("../{MARKER}")]),
    );
    let err = serde_json::from_value::<Selector>(bad_paths)
        .expect_err("escaping path accepted")
        .to_string();
    assert!(!err.contains(MARKER), "{err}");

    let bad_endpoint = with(
        &provider_min(),
        "endpoint",
        json!(format!("https://{MARKER}/v1")),
    );
    let err = serde_json::from_value::<Provider>(bad_endpoint)
        .expect_err("endpoint accepted")
        .to_string();
    assert!(!err.contains(MARKER), "{err}");

    let bad_ref = with(
        &provider_min(),
        "credential_reference",
        json!(format!("sk-{MARKER}")),
    );
    let err = serde_json::from_value::<Provider>(bad_ref)
        .expect_err("key-like reference accepted")
        .to_string();
    assert!(!err.contains(MARKER), "{err}");
}

#[test]
fn nullable_keys_are_serialised_as_null_after_a_decode_encode_cycle() {
    // Required-but-nullable keys are never skipped (request §5.1).
    let typed: Provider = serde_json::from_value(provider_min()).expect("provider");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(text.contains("\"credential_reference\":null"), "{text}");
    let plan: PlanSnapshot = serde_json::from_value(plan_min()).expect("plan");
    let text = serde_json::to_string(&plan).expect("serialise");
    assert!(text.contains("\"workflow_id\":null"), "{text}");
    assert!(text.contains("\"workflow_revision\":null"), "{text}");
}

#[test]
fn renderer_parameter_text_is_sanitised_on_read() {
    // `parameters_redacted` values are `RedactedText`: control characters are escaped, not kept.
    let inst = with(
        &renderer_min(),
        "parameters_redacted",
        json!({"note": "a\u{1b}[31mb"}),
    );
    let typed: RendererSetting = serde_json::from_value(inst).expect("renderer");
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back["parameters_redacted"]["note"], json!("a<U+001B>[31mb"));
}

// --- P02-D01: RendererSetting.parameters_redacted is class R (`BTreeMap<Identifier, RedactedText>`) -----------

#[test]
fn renderer_parameter_value_with_esc_reads_back_escaped_and_reserialises_escaped() {
    let inst = with(
        &renderer_min(),
        "parameters_redacted",
        json!({"note": "a\u{1b}[31mb"}),
    );
    let typed: RendererSetting = serde_json::from_value(inst).expect("renderer");
    let value = typed
        .parameters_redacted
        .iter()
        .find(|(k, _)| k.as_str() == "note")
        .map(|(_, v)| v)
        .expect("note parameter");
    assert_eq!(value.as_str(), "a<U+001B>[31mb");
    assert!(!value.as_str().contains('\u{1b}'));
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back["parameters_redacted"]["note"], json!("a<U+001B>[31mb"));
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(!text.contains('\u{1b}'), "{text}");
    assert!(text.contains("a<U+001B>[31mb"), "{text}");
}

#[test]
fn renderer_parameter_key_with_control_character_or_invalid_identifier_is_rejected_without_echo() {
    const MARKER: &str = "PLANTEDKEYMARKER";
    for key in [
        format!("{MARKER}\u{1b}"),
        format!("{MARKER}\u{0}"),
        format!("{MARKER}\n"),
        format!("bad key {MARKER}"),
        format!("{MARKER}$"),
    ] {
        let inst = with(
            &renderer_min(),
            "parameters_redacted",
            json!({ key.clone(): "v" }),
        );
        let err = serde_json::from_value::<RendererSetting>(inst)
            .expect_err("invalid parameter key accepted")
            .to_string();
        assert!(!err.contains(MARKER), "key echoed: {err}");

        // The same through JSON text, where the error carries a position rather than the input.
        let text = serde_json::to_string(&with(
            &renderer_min(),
            "parameters_redacted",
            json!({ key: "v" }),
        ))
        .expect("text");
        let err = serde_json::from_str::<RendererSetting>(&text)
            .expect_err("invalid parameter key accepted from text")
            .to_string();
        assert!(!err.contains(MARKER), "key echoed from text: {err}");
    }
}

#[test]
fn renderer_parameters_clean_map_round_trips_to_an_equal_value() {
    for params in [
        json!({}),
        json!({"namespace": "default", "release": "demo"}),
        json!({"empty": "", "multi": "line one\nline two\tend", "unicode": "caf\u{e9}"}),
    ] {
        let inst = with(&renderer_min(), "parameters_redacted", params);
        let typed: RendererSetting = serde_json::from_value(inst.clone()).expect("renderer");
        assert_eq!(serde_json::to_value(&typed).expect("serialise"), inst);
        let text = serde_json::to_string(&typed).expect("serialise");
        let again: RendererSetting = serde_json::from_str(&text).expect("decode text");
        assert_eq!(again, typed);
    }
}

#[test]
fn renderer_setting_schema_cross_check_holds_after_the_class_change() {
    let typed: RendererSetting = serde_json::from_value(renderer_max()).expect("maximal");
    let ser = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(keys(&ser), def_properties("RendererSetting"));
    assert_eq!(
        def_required("RendererSetting"),
        def_properties("RendererSetting")
    );
    assert_eq!(ser, renderer_max());
}

// --- P02-D02 -----------------------------------------------------------------------------------------------

#[test]
fn renderer_parameters_field_type_is_exactly_a_map_of_identifier_to_redacted_text() {
    // D02-3: compile-time pin. A `SanitisedText` (or any other) value type would not compile here.
    let typed: RendererSetting = serde_json::from_value(renderer_min()).expect("renderer");
    let _: &BTreeMap<Identifier, RedactedText> = &typed.parameters_redacted;
}

#[test]
fn wrong_typed_zdr_value_is_rejected_without_echo() {
    // D02-4: `ConstTrue` must not quote the planted value (serde_json's `invalid_type` would).
    const MARKER: &str = "MARKERx";
    const NUMBER: &str = "987654321";
    let planted = [
        json!(MARKER),
        json!(987_654_321u64),
        json!(-987_654_321i64),
        json!(987_654_321.5f64),
        json!(false),
    ];
    for value in planted {
        let inst = with(&policy_min(), "zdr", value.clone());
        let err = serde_json::from_value::<ProviderPolicy>(inst.clone())
            .expect_err("wrong-typed zdr accepted")
            .to_string();
        assert!(!err.contains(MARKER), "{value}: {err}");
        assert!(!err.contains(NUMBER), "{value}: {err}");
        let text = serde_json::to_string(&inst).expect("serialise instance");
        let err = serde_json::from_str::<ProviderPolicy>(&text)
            .expect_err("wrong-typed zdr accepted from text")
            .to_string();
        assert!(!err.contains(MARKER), "{value}: {err}");
        assert!(!err.contains(NUMBER), "{value}: {err}");
    }
}

#[test]
fn absent_required_nullable_key_decodes_as_none_because_the_schema_layer_not_serde_enforces_presence()
 {
    // D02-6: pins current behaviour. Serde treats a missing `Option` field as `None`; the schema's
    // `required` list is the guard against a missing key (T08 carries the schema-layer negative). The
    // typed layer still always writes the key back.
    let inst = without(&file_min(), "detected_framework");
    assert!(inst.get("detected_framework").is_none());
    let typed: FileRecord = serde_json::from_value(inst).expect("decodes without the key");
    assert_eq!(typed.detected_framework, None);
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(text.contains("\"detected_framework\":null"), "{text}");
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back, file_min());
}

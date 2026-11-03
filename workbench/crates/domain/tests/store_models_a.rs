//! P02-T05b config-side store records: Schedule, Workflow, WorkflowFile, Repository, RepositoryFile,
//! ExitMeaning, LicenseRecord, ToolManifest, ToolManifestFile, Approval, ApprovalFile, Config, ConfigFile
//! (`planning/P02-T01-request.md` §5.1, §9). All data is synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use domain::contract::{ApprovalLifetime, DataCategory, ExportFormat, ToolAvailability};
use domain::states::ApprovalState;
use domain::store::StoreKind;
use domain::store::approval::{Approval, ApprovalFile};
use domain::store::config::{Config, ConfigFile};
use domain::store::repository::{CheckoutState, Repository, RepositoryFile};
use domain::store::tools::{
    ExitMeaning, ExitMeaningKind, InvocationMode, LicenseRecord, LicenseReviewStatus,
    NetworkPolicy, ReleaseTier, SelectionStrategy, ToolDistribution, ToolManifest,
    ToolManifestFile, ToolRole,
};
use domain::store::workflow::{
    MissedPolicy, OverlapPolicy, Schedule, ScheduleKind, Workflow, WorkflowFile,
};
use domain::vocab::{Capability, Category};
use serde::de::{DeserializeOwned, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");

const REPO: &str = "526a9739-68cb-5ef8-8c3a-0fda8a207475";
const REPO2: &str = "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
const WORKFLOW: &str = "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d";
const WORKFLOW2: &str = "3b241101-e2bb-4255-8caf-4136c566a962";
const FILE: &str = "11111111-2222-4333-8444-555555555555";
const FILE2: &str = "66666666-7777-4888-9999-aaaaaaaaaaaa";
const APPROVAL: &str = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";
const APPROVAL2: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
const RUN: &str = "12345678-1234-4234-8234-123456789abc";
const SOURCE: &str = "01234567-89ab-4cde-8f01-23456789abcd";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OID: &str = "0123456789abcdef0123456789abcdef01234567";
const TS: &str = "2026-10-16T05:25:00Z";
const TS_FRACTION: &str = "2026-10-16T05:25:00.123456789Z";
const U64_MAX: u64 = 18_446_744_073_709_551_615;

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

/// Properties of `def` whose schema admits `null`: `anyOf [.., {"type": "null"}]` or a `type` array that
/// contains `"null"`.
fn def_nullable(def: &str) -> BTreeSet<String> {
    let schema = schema();
    let props = schema["$defs"][def]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{def} has no properties"));
    props
        .iter()
        .filter(|(_, p)| {
            let any_of = p["anyOf"]
                .as_array()
                .is_some_and(|alts| alts.iter().any(|a| *a == json!({"type": "null"})));
            let type_array = p["type"]
                .as_array()
                .is_some_and(|ts| ts.iter().any(|t| t == "null"));
            any_of || type_array
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// The keys of one JSON object in document order. `serde_json` is built without `preserve_order`, so a
/// `Value` sorts its keys; a visitor sees them as they appear in the text.
struct OrderedKeys(Vec<String>);

impl<'de> Deserialize<'de> for OrderedKeys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;
        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = OrderedKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedKeys, A::Error> {
                let mut keys = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    map.next_value::<IgnoredAny>()?;
                    keys.push(key);
                }
                Ok(OrderedKeys(keys))
            }
        }
        deserializer.deserialize_map(KeyVisitor)
    }
}

/// Descends through the object members named by `path` and returns the document-order keys of the object
/// found there (`None` if the path does not exist).
struct KeysAt<'p>(&'p [&'p str]);

impl<'de> DeserializeSeed<'de> for KeysAt<'_> {
    type Value = Option<Vec<String>>;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        struct PathVisitor<'p>(&'p [&'p str]);
        impl<'de> Visitor<'de> for PathVisitor<'_> {
            type Value = Option<Vec<String>>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let (head, rest) = self.0.split_first().expect("non-empty path");
                let mut found = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key != *head {
                        map.next_value::<IgnoredAny>()?;
                    } else if rest.is_empty() {
                        found = Some(map.next_value::<OrderedKeys>()?.0);
                    } else {
                        found = map.next_value_seed(KeysAt(rest))?;
                    }
                }
                Ok(found)
            }
        }
        deserializer.deserialize_map(PathVisitor(self.0))
    }
}

/// Document-order keys of the object at `path` in JSON `text`.
fn keys_at(text: &str, path: &[&str]) -> Vec<String> {
    KeysAt(path)
        .deserialize(&mut serde_json::Deserializer::from_str(text))
        .expect("text is JSON")
        .unwrap_or_else(|| panic!("no object at {path:?}"))
}

/// Order of the `properties` of a `$def`, read from the schema text.
fn schema_property_order(def: &str) -> Vec<String> {
    keys_at(SCHEMA, &["$defs", def, "properties"])
}

/// Order of the top-level keys of serialised JSON text.
fn serialised_key_order(text: &str) -> Vec<String> {
    serde_json::from_str::<OrderedKeys>(text)
        .expect("object text")
        .0
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("enum list")
        .iter()
        .map(|s| s.as_str().expect("string").to_owned())
        .collect()
}

/// `enum` of a property (`$defs/<def>/properties/<prop>/enum`).
fn schema_enum(def: &str, prop: &str) -> Vec<String> {
    strings(&schema()["$defs"][def]["properties"][prop]["enum"])
}

/// `enum` of the items of an array property (`.../<prop>/items/enum`).
fn schema_items_enum(def: &str, prop: &str) -> Vec<String> {
    strings(&schema()["$defs"][def]["properties"][prop]["items"]["enum"])
}

/// `enum` of a shared `$def` such as `Capability`.
fn schema_def_enum(def: &str) -> Vec<String> {
    strings(&schema()["$defs"][def]["enum"])
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

    // The nullable list is exactly the set of properties whose schema admits null.
    let listed: BTreeSet<String> = d.nullable.iter().map(|k| (*k).to_owned()).collect();
    assert_eq!(
        listed.len(),
        d.nullable.len(),
        "{} nullable duplicates",
        d.name
    );
    assert_eq!(listed, def_nullable(d.name), "{} nullable list", d.name);

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

    // Field order: the serialised text lists the keys in the order of the schema's `properties`.
    let want = schema_property_order(d.name);
    for (label, inst) in [("minimal", &d.minimal), ("maximal", &d.maximal)] {
        let typed: T = serde_json::from_value(inst.clone()).expect(label);
        let text = serde_json::to_string(&typed).expect("serialise");
        assert_eq!(
            serialised_key_order(&text),
            want,
            "{} {label}: key order differs from the schema",
            d.name
        );
    }
}

// --- shared instances (wave 1a types) -------------------------------------------------------------------

fn selector_example() -> Value {
    json!({"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []})
}

fn selector_max() -> Value {
    json!({
        "mode": "patterns",
        "paths": ["a/b.yaml", "c.yml"],
        "include_patterns": ["**/*.yaml", "[ab]*.yml"],
        "exclude_patterns": ["vendor/**"]
    })
}

fn target_min() -> Value {
    json!({"repo_id": REPO, "ref_kind": "branch", "ref_value": "main", "selector": selector_example()})
}

fn target_max() -> Value {
    json!({"repo_id": REPO2, "ref_kind": "commit", "ref_value": OID, "selector": selector_max()})
}

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
        "categories": ["syntax", "rendering"],
        "secret_scope": "history",
        "template_mode": "rendered",
        "schema_mode": "user_supplied",
        "schema_bindings": [{
            "path_pattern": "deploy/*.yaml",
            "schema_resource": "bundled:schemas/k8s.json",
            "dialect": "draft-2020-12",
            "expected_digest": DIGEST
        }],
        "renderers": [{
            "framework": "helm",
            "renderer_id": "helm:3.14",
            "input_paths": ["charts/app/values.yaml"],
            "parameters_redacted": {"release": "demo"}
        }],
        "allow_reference_context": false,
        "adapter_ids": ["yamllint", "docker-compose"]
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
        "byte_size": U64_MAX,
        "content_digest": DIGEST,
        "commit_oid": DIGEST2,
        "detected_framework": "github-actions",
        "discovery_state": "excluded",
        "reason": "Excluded by pattern vendor/**"
    })
}

fn policy() -> Value {
    json!({"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": ["openrouter"]})
}

fn provider_max() -> Value {
    json!({
        "provider_id": "openrouter",
        "kind": "api",
        "endpoint": "https://openrouter.ai/api/v1",
        "model_id": "openai/gpt-4o:free",
        "credential_mode": "os_credential_store",
        "credential_reference": "keychain:pwt.openrouter",
        "routing_policy": policy()
    })
}

fn scope_max() -> Value {
    json!({
        "provider_id": "openrouter",
        "model_id": "openai/gpt-4o:free",
        "routing_policy": policy(),
        "source_ids": [SOURCE],
        "redaction_version": "2026.10-a"
    })
}

// --- Schedule -------------------------------------------------------------------------------------------

fn schedule_min() -> Value {
    json!({
        "kind": "manual",
        "interval_seconds": null,
        "daily_local_time": null,
        "timezone": "UTC",
        "next_due_at": null,
        "overlap_policy": "skip_if_running",
        "missed_policy": "record_no_replay"
    })
}

fn schedule_max() -> Value {
    json!({
        "kind": "daily",
        "interval_seconds": U64_MAX,
        "daily_local_time": "10:00",
        "timezone": "Asia/Kolkata",
        "next_due_at": TS_FRACTION,
        "overlap_policy": "skip_if_running",
        "missed_policy": "record_no_replay"
    })
}

#[test]
fn schedule() {
    check::<Schedule>(&Def {
        name: "Schedule",
        minimal: schedule_min(),
        maximal: schedule_max(),
        nullable: &["interval_seconds", "daily_local_time", "next_due_at"],
        bad: vec![
            ("kind", json!("weekly")),
            ("kind", json!("Daily")),
            ("interval_seconds", json!(-1)),
            ("interval_seconds", json!(1.5)),
            ("interval_seconds", json!("60")),
            ("daily_local_time", json!("25:00")),
            ("daily_local_time", json!("9:00")),
            ("timezone", json!("not a zone")),
            ("timezone", json!("")),
            ("timezone", Value::Null),
            ("next_due_at", json!("tomorrow")),
            ("next_due_at", json!("2026-10-16T05:25:00+00:00")),
            ("overlap_policy", json!("queue")),
            ("overlap_policy", json!("skip_if_running ")),
            ("missed_policy", json!("replay")),
            ("missed_policy", Value::Null),
        ],
    });
}

// --- Workflow / WorkflowFile ----------------------------------------------------------------------------

fn workflow_min() -> Value {
    json!({
        "workflow_id": WORKFLOW,
        "revision": 1,
        "name": "Daily configuration review",
        "description": "Example only; framework support depends on installed approved adapters.",
        "enabled": false,
        "created_at": TS,
        "updated_at": TS,
        "targets": [],
        "scan_settings": scan_example(),
        "schedule": schedule_min(),
        "approval_ids": [],
        "output_formats": ["json"]
    })
}

fn workflow_max() -> Value {
    json!({
        "workflow_id": WORKFLOW2,
        "revision": U64_MAX,
        "name": "Nightly review \u{2014} caf\u{e9}",
        "description": "",
        "enabled": true,
        "created_at": TS,
        "updated_at": TS_FRACTION,
        "targets": [target_min(), target_max()],
        "scan_settings": scan_max(),
        "schedule": schedule_max(),
        "approval_ids": [APPROVAL, APPROVAL2],
        "output_formats": ["json", "md", "pdf"]
    })
}

#[test]
fn workflow() {
    check::<Workflow>(&Def {
        name: "Workflow",
        minimal: workflow_min(),
        maximal: workflow_max(),
        nullable: &[],
        bad: vec![
            ("workflow_id", json!("nope")),
            ("workflow_id", json!("9D4AA1E3-E812-57CD-96AB-BF9C1AB1D62D")),
            ("revision", json!(-1)),
            ("revision", json!(1.5)),
            ("name", json!(5)),
            ("name", Value::Null),
            ("description", json!(true)),
            ("enabled", json!("yes")),
            ("created_at", json!("now")),
            ("updated_at", json!("2026-10-16 05:25:00")),
            ("targets", json!([{"repo_id": REPO}])),
            ("scan_settings", json!({})),
            ("schedule", json!({"kind": "manual"})),
            ("approval_ids", json!(["nope"])),
            ("output_formats", json!(["pdfx"])),
            ("output_formats", json!(["markdown"])),
            ("output_formats", json!(["JSON"])),
            ("output_formats", json!("json")),
        ],
    });
}

fn workflow_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "workflows",
        "updated_at": TS,
        "workflows": []
    })
}

fn workflow_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "workflows",
        "updated_at": TS_FRACTION,
        "workflows": [workflow_min(), workflow_max()]
    })
}

#[test]
fn workflow_file() {
    check::<WorkflowFile>(&Def {
        name: "WorkflowFile",
        minimal: workflow_file_min(),
        maximal: workflow_file_max(),
        nullable: &[],
        bad: vec![
            ("updated_at", json!("never")),
            ("workflows", json!([{"workflow_id": WORKFLOW}])),
            ("workflows", json!({})),
        ],
    });
}

// --- Repository / RepositoryFile ------------------------------------------------------------------------

fn repository_min() -> Value {
    json!({
        "repo_id": REPO,
        "display_name": "example-service",
        "slug": "example-service",
        "remote_url": "https://github.com/example/example-service.git",
        "canonical_remote": "github.com/example/example-service",
        "local_path": "checkouts/526a9739-68cb-5ef8-8c3a-0fda8a207475",
        "default_ref_kind": "branch",
        "default_ref_value": "main",
        "registered_at": TS,
        "last_update_at": null,
        "last_commit_oid": null,
        "checkout_state": "not_cloned",
        "last_error_redacted": null,
        "inventory_revision": 0,
        "inventory_at": null,
        "files": []
    })
}

fn repository_max() -> Value {
    json!({
        "repo_id": REPO2,
        "display_name": "Example service \u{2014} caf\u{e9}",
        "slug": "example-2",
        "remote_url": "ssh://git@git.example.invalid:2222/org/repo.git",
        "canonical_remote": "git.example.invalid/org/repo",
        "local_path": "checkouts/0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b",
        "default_ref_kind": "tag",
        "default_ref_value": "v1.2.3",
        "registered_at": TS,
        "last_update_at": TS_FRACTION,
        "last_commit_oid": DIGEST,
        "checkout_state": "error",
        "last_error_redacted": "update failed: exit status 128",
        "inventory_revision": U64_MAX,
        "inventory_at": TS,
        "files": [file_min(), file_max()]
    })
}

#[test]
fn repository() {
    check::<Repository>(&Def {
        name: "Repository",
        minimal: repository_min(),
        maximal: repository_max(),
        nullable: &[
            "last_update_at",
            "last_commit_oid",
            "last_error_redacted",
            "inventory_at",
        ],
        bad: vec![
            ("repo_id", json!("nope")),
            ("display_name", json!(5)),
            ("slug", json!("Bad Slug")),
            ("slug", json!("-lead")),
            ("slug", json!("")),
            ("remote_url", json!("https://user:pw@example.invalid/x.git")),
            ("remote_url", json!("https://example.invalid/x.git?token=1")),
            ("remote_url", json!("file:///etc/passwd")),
            ("canonical_remote", json!("https://github.com/x/y")),
            ("canonical_remote", json!("GitHub.com/x/y")),
            ("local_path", json!("/abs/checkout")),
            ("local_path", json!("../escape")),
            ("default_ref_kind", json!("remote")),
            ("default_ref_kind", json!("Branch")),
            ("default_ref_value", json!("a..b")),
            ("registered_at", json!("2026-10-16")),
            ("last_update_at", json!("yesterday")),
            ("last_commit_oid", json!("abc")),
            ("checkout_state", json!("cloned")),
            ("checkout_state", json!("READY")),
            ("last_error_redacted", json!(5)),
            ("inventory_revision", json!(-1)),
            ("inventory_revision", json!(0.5)),
            ("inventory_at", json!("later")),
            ("files", json!([{"file_id": FILE}])),
        ],
    });
}

fn repository_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "repositories",
        "updated_at": TS,
        "repositories": []
    })
}

fn repository_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "repositories",
        "updated_at": TS_FRACTION,
        "repositories": [repository_min(), repository_max()]
    })
}

#[test]
fn repository_file() {
    check::<RepositoryFile>(&Def {
        name: "RepositoryFile",
        minimal: repository_file_min(),
        maximal: repository_file_max(),
        nullable: &[],
        bad: vec![
            ("updated_at", json!("never")),
            ("repositories", json!([{"repo_id": REPO}])),
            ("repositories", json!("none")),
        ],
    });
}

// --- ExitMeaning / LicenseRecord / ToolManifest / ToolManifestFile --------------------------------------

fn exit_min() -> Value {
    json!({"code": 0, "meaning": "no_findings_when_output_confirms"})
}

fn exit_max() -> Value {
    json!({"code": i64::MIN, "meaning": "inspection_required"})
}

#[test]
fn exit_meaning() {
    check::<ExitMeaning>(&Def {
        name: "ExitMeaning",
        minimal: exit_min(),
        maximal: exit_max(),
        nullable: &[],
        bad: vec![
            ("code", json!("0")),
            ("code", json!(1.5)),
            ("code", Value::Null),
            ("meaning", json!("ok")),
            ("meaning", json!("Execution_Error")),
            ("meaning", json!("findings")),
        ],
    });
    // The remaining two schema values decode too.
    for m in ["execution_error", "findings_possible"] {
        let typed: ExitMeaning = serde_json::from_value(json!({"code": 1, "meaning": m})).expect(m);
        assert_eq!(
            serde_json::to_value(typed).expect("serialise"),
            json!({"code": 1, "meaning": m})
        );
    }
}

fn license_min() -> Value {
    json!({
        "spdx": null,
        "verified_version": null,
        "review_status": "pending",
        "license_file": null,
        "notice_files": [],
        "source_obligation_note": null,
        "reviewed_at": null
    })
}

fn license_max() -> Value {
    json!({
        "spdx": "Apache-2.0 OR (MIT AND BSD-3-Clause)",
        "verified_version": "1.35.1",
        "review_status": "approved",
        "license_file": "licenses/LICENSE",
        "notice_files": ["licenses/NOTICE", "licenses/NOTICE-extra"],
        "source_obligation_note": "No source offer required for unmodified dynamic use.",
        "reviewed_at": TS_FRACTION
    })
}

#[test]
fn license_record() {
    check::<LicenseRecord>(&Def {
        name: "LicenseRecord",
        minimal: license_min(),
        maximal: license_max(),
        nullable: &[
            "spdx",
            "verified_version",
            "license_file",
            "source_obligation_note",
            "reviewed_at",
        ],
        bad: vec![
            ("spdx", json!("")),
            ("spdx", json!("MIT;rm")),
            ("spdx", json!("MIT  OR Apache-2.0")),
            ("verified_version", json!("1 0")),
            ("review_status", json!("maybe")),
            ("review_status", json!("Approved")),
            ("license_file", json!("../LICENSE")),
            ("license_file", json!("/abs/LICENSE")),
            ("notice_files", json!(["/abs"])),
            ("notice_files", json!("NOTICE")),
            ("source_obligation_note", json!(5)),
            ("reviewed_at", json!("never")),
        ],
    });
}

fn tool_min() -> Value {
    json!({
        "adapter_id": "yamllint",
        "adapter_version": "1.0",
        "tool_name": "yamllint",
        "tool_version": null,
        "role": "runtime",
        "release_tier": "foundation",
        "availability": "missing",
        "capabilities": [],
        "frameworks": [],
        "platforms": [],
        "wrapper_name": null,
        "invocation_mode": "native",
        "command_policy_id": "policy.yamllint.v1",
        "parser_id": "parser.yamllint.v1",
        "native_format": "parsable",
        "config_digest": null,
        "selection_strategy": "explicit_files",
        "network_policy": "none",
        "network_resources": [],
        "exit_mapping": [],
        "timeout_seconds": 1,
        "max_output_bytes": 1,
        "distribution": "not_shipped",
        "binary_origin": null,
        "binary_sha256": null,
        "license": license_min(),
        "limitations": []
    })
}

fn tool_max() -> Value {
    json!({
        "adapter_id": "docker-compose",
        "adapter_version": "0.1.0+local",
        "tool_name": "docker-compose",
        "tool_version": "2.29.1",
        "role": "optional_helper",
        "release_tier": "recommended_extension",
        "availability": "unavailable",
        "capabilities": ["syntax", "schema", "secrets", "misconfiguration", "vulnerability",
            "external_verification", "style", "workflow_validation", "rendering"],
        "frameworks": ["docker-compose", "github-actions"],
        "platforms": ["linux-x86_64", "macos-aarch64"],
        "wrapper_name": "pwt-compose-wrapper",
        "invocation_mode": "shell",
        "command_policy_id": "policy.compose.v2",
        "parser_id": "parser.compose.v2",
        "native_format": "json",
        "config_digest": DIGEST,
        "selection_strategy": "approved_wider_context",
        "network_policy": "approval_required",
        "network_resources": ["registry.example.invalid:443", "https://api.example.invalid/v1"],
        "exit_mapping": [exit_min(), exit_max(),
            {"code": 2, "meaning": "execution_error"}, {"code": 3, "meaning": "findings_possible"}],
        "timeout_seconds": U64_MAX,
        "max_output_bytes": U64_MAX,
        "distribution": "system_prerequisite",
        "binary_origin": "Installed by the user from the vendor package.",
        "binary_sha256": DIGEST2,
        "license": license_max(),
        "limitations": ["Does not render templates.", "Needs a container runtime."]
    })
}

#[test]
fn tool_manifest() {
    check::<ToolManifest>(&Def {
        name: "ToolManifest",
        minimal: tool_min(),
        maximal: tool_max(),
        nullable: &[
            "tool_version",
            "wrapper_name",
            "config_digest",
            "binary_origin",
            "binary_sha256",
        ],
        bad: vec![
            ("adapter_id", json!("nmap")),
            ("adapter_id", json!("Git")),
            ("adapter_version", json!("1 0")),
            ("tool_name", json!("YamlLint")),
            ("tool_version", json!("a b")),
            ("role", json!("admin")),
            ("role", json!("Runtime")),
            ("release_tier", json!("ga")),
            ("release_tier", json!("pilot")),
            ("availability", json!("installed")),
            ("availability", json!("Ready")),
            ("capabilities", json!(["bogus"])),
            ("frameworks", json!(["Helm"])),
            ("platforms", json!(["a b"])),
            ("wrapper_name", json!("a b")),
            ("invocation_mode", json!("exec")),
            ("invocation_mode", json!("in-process")),
            ("command_policy_id", json!("")),
            ("parser_id", json!("a b")),
            ("native_format", json!("a b")),
            ("config_digest", json!("xyz")),
            ("selection_strategy", json!("all_files")),
            ("network_policy", json!("always")),
            ("network_policy", json!("None")),
            ("network_resources", json!(["bad host!"])),
            ("exit_mapping", json!([{"code": 0, "meaning": "ok"}])),
            ("exit_mapping", json!([{"code": 0}])),
            ("timeout_seconds", json!(-1)),
            ("timeout_seconds", json!(1.5)),
            ("max_output_bytes", json!(-1)),
            ("distribution", json!("cloud")),
            ("binary_origin", json!(5)),
            ("binary_sha256", json!("xyz")),
            ("license", json!({"spdx": null})),
            ("limitations", json!([1])),
        ],
    });
}

fn tool_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "tool_manifest",
        "updated_at": TS,
        "tools": []
    })
}

fn tool_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "tool_manifest",
        "updated_at": TS_FRACTION,
        "tools": [tool_min(), tool_max()]
    })
}

#[test]
fn tool_manifest_file() {
    check::<ToolManifestFile>(&Def {
        name: "ToolManifestFile",
        minimal: tool_file_min(),
        maximal: tool_file_max(),
        nullable: &[],
        bad: vec![
            ("updated_at", json!("never")),
            ("tools", json!([{"adapter_id": "git"}])),
            ("tools", json!("none")),
        ],
    });
}

// --- Approval / ApprovalFile ----------------------------------------------------------------------------

fn approval_min() -> Value {
    json!({
        "approval_id": APPROVAL,
        "status": "pending",
        "capability": "git_acquire",
        "workflow_id": null,
        "workflow_revision": null,
        "run_id": null,
        "repo_ids": [],
        "purpose": "Clone the registered repository",
        "operation_fingerprint": DIGEST,
        "destinations": [],
        "resource_constraints": [],
        "data_categories": [],
        "lifetime": "once",
        "requested_at": TS,
        "decided_at": null,
        "expires_at": null,
        "actor": "local user",
        "consumed": false,
        "inference_scope": null
    })
}

fn approval_max() -> Value {
    json!({
        "approval_id": APPROVAL2,
        "status": "approved",
        "capability": "model_transmission",
        "workflow_id": WORKFLOW,
        "workflow_revision": U64_MAX,
        "run_id": RUN,
        "repo_ids": [REPO, REPO2],
        "purpose": "Explain findings with the configured model",
        "operation_fingerprint": DIGEST2,
        "destinations": ["openrouter.ai:443", "https://openrouter.ai/api/v1"],
        "resource_constraints": ["At most 20 chunks.", "Single run only."],
        "data_categories": [
            "repository_metadata", "redacted_source", "redacted_findings", "schema_documents",
            "public_artifact_identifiers", "redacted_chat_context", "app_metadata"
        ],
        "lifetime": "workflow_revision",
        "requested_at": TS,
        "decided_at": TS_FRACTION,
        "expires_at": "2026-11-16T05:25:00Z",
        "actor": "local user",
        "consumed": true,
        "inference_scope": scope_max()
    })
}

#[test]
fn approval() {
    check::<Approval>(&Def {
        name: "Approval",
        minimal: approval_min(),
        maximal: approval_max(),
        nullable: &[
            "workflow_id",
            "workflow_revision",
            "run_id",
            "decided_at",
            "expires_at",
            "inference_scope",
        ],
        bad: vec![
            ("approval_id", json!("nope")),
            ("status", json!("granted")),
            ("status", json!("Approved")),
            ("capability", json!("git_clone")),
            ("capability", json!("GitAcquire")),
            ("workflow_id", json!("nope")),
            ("workflow_revision", json!(-1)),
            ("workflow_revision", json!("1")),
            ("run_id", json!("nope")),
            ("repo_ids", json!(["nope"])),
            ("purpose", json!(5)),
            ("operation_fingerprint", json!("abc")),
            ("operation_fingerprint", Value::Null),
            ("destinations", json!(["bad host!"])),
            ("resource_constraints", json!([1])),
            ("data_categories", json!(["raw_secrets"])),
            ("data_categories", json!(["Redacted_Source"])),
            ("lifetime", json!("forever")),
            ("lifetime", json!("Once")),
            ("requested_at", json!("sometime")),
            ("decided_at", json!("later")),
            ("expires_at", json!("2026-11-16T05:25:00+01:00")),
            ("actor", json!(false)),
            ("consumed", json!("no")),
            ("inference_scope", json!({"provider_id": "openrouter"})),
        ],
    });
}

fn approval_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "approvals",
        "updated_at": TS,
        "approvals": []
    })
}

fn approval_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "approvals",
        "updated_at": TS_FRACTION,
        "approvals": [approval_min(), approval_max()]
    })
}

#[test]
fn approval_file() {
    check::<ApprovalFile>(&Def {
        name: "ApprovalFile",
        minimal: approval_file_min(),
        maximal: approval_file_max(),
        nullable: &[],
        bad: vec![
            ("updated_at", json!("never")),
            ("approvals", json!([{"approval_id": APPROVAL}])),
            ("approvals", json!(null)),
        ],
    });
}

// --- Config / ConfigFile --------------------------------------------------------------------------------

fn config_min() -> Value {
    json!({
        "app_data_root": "/home/user/.local/share/pwt-example",
        "backup_directory": "backup",
        "active_theme": "default",
        "max_active_repositories": 1,
        "max_scanner_processes": 1,
        "history_retention_days": null,
        "chat_persistence_enabled": false,
        "provider": null,
        "telemetry_enabled": false,
        "redaction_version": "v1"
    })
}

fn config_max() -> Value {
    json!({
        "app_data_root": "/var/lib/pwt-example",
        "backup_directory": "/var/backups/pwt-example",
        "active_theme": "high-contrast",
        "max_active_repositories": U64_MAX,
        "max_scanner_processes": 8,
        "history_retention_days": 90,
        "chat_persistence_enabled": true,
        "provider": provider_max(),
        "telemetry_enabled": false,
        "redaction_version": "2026.10-a"
    })
}

#[test]
fn config() {
    check::<Config>(&Def {
        name: "Config",
        minimal: config_min(),
        maximal: config_max(),
        nullable: &["history_retention_days", "provider"],
        bad: vec![
            ("app_data_root", json!("a/../b")),
            ("app_data_root", json!("")),
            ("backup_directory", json!("/x/../y")),
            ("active_theme", json!("bad theme")),
            ("max_active_repositories", json!(-1)),
            ("max_active_repositories", json!(1.5)),
            ("max_scanner_processes", json!("2")),
            ("history_retention_days", json!(-1)),
            ("chat_persistence_enabled", json!("yes")),
            ("provider", json!({"provider_id": "openrouter"})),
            ("telemetry_enabled", json!(true)),
            ("telemetry_enabled", json!("false")),
            ("telemetry_enabled", json!(0)),
            ("telemetry_enabled", Value::Null),
            ("redaction_version", json!("")),
            ("redaction_version", json!("v 1")),
        ],
    });
}

fn config_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "config",
        "updated_at": TS,
        "config": config_min()
    })
}

fn config_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "config",
        "updated_at": TS_FRACTION,
        "config": config_max()
    })
}

#[test]
fn config_file() {
    check::<ConfigFile>(&Def {
        name: "ConfigFile",
        minimal: config_file_min(),
        maximal: config_file_max(),
        nullable: &[],
        bad: vec![
            ("updated_at", json!("never")),
            ("config", json!({})),
            ("config", Value::Null),
        ],
    });
}

// --- cross-checks ---------------------------------------------------------------------------------------

/// The thirteen `$defs` of this task; each has a `check::<Type>` call in this file.
const DEFS: [&str; 13] = [
    "Schedule",
    "Workflow",
    "WorkflowFile",
    "Repository",
    "RepositoryFile",
    "ExitMeaning",
    "LicenseRecord",
    "ToolManifest",
    "ToolManifestFile",
    "Approval",
    "ApprovalFile",
    "Config",
    "ConfigFile",
];

/// This file's own text, to prove that every listed def is driven through the shared `check` helper.
const THIS_FILE: &str = include_str!("store_models_a.rs");

#[test]
fn every_listed_def_exists_in_the_schema_and_is_driven_through_check() {
    let schema = schema();
    for n in DEFS {
        assert!(schema["$defs"][n].is_object(), "{n} missing from schema");
        assert_eq!(
            schema["$defs"][n]["additionalProperties"],
            json!(false),
            "{n}"
        );
        // Fails if a per-def test is deleted or its `check` call is dropped or duplicated.
        let call = format!("    check::<{n}>(&Def {{");
        assert_eq!(
            THIS_FILE.lines().filter(|l| *l == call).count(),
            1,
            "{n}: expected exactly one `{}`",
            call.trim()
        );
        assert!(
            THIS_FILE.contains(&format!("name: \"{n}\",")),
            "{n}: no Def named {n}"
        );
    }
    let distinct: BTreeSet<&str> = DEFS.into_iter().collect();
    assert_eq!(distinct.len(), DEFS.len(), "duplicate def names");
    // No further `check` call exists for a type outside the list.
    let calls = THIS_FILE
        .lines()
        .filter(|l| l.starts_with("    check::<") && l.ends_with(">(&Def {"))
        .count();
    assert_eq!(calls, DEFS.len(), "check calls versus listed defs");
}

#[test]
fn new_enums_reproduce_the_schema_value_sets() {
    assert_eq!(
        serialised_all(&ScheduleKind::ALL),
        schema_enum("Schedule", "kind")
    );
    assert_eq!(
        serialised_all(&OverlapPolicy::ALL),
        schema_enum("Schedule", "overlap_policy")
    );
    assert_eq!(
        serialised_all(&MissedPolicy::ALL),
        schema_enum("Schedule", "missed_policy")
    );
    assert_eq!(
        serialised_all(&CheckoutState::ALL),
        schema_enum("Repository", "checkout_state")
    );
    assert_eq!(
        serialised_all(&ExitMeaningKind::ALL),
        schema_enum("ExitMeaning", "meaning")
    );
    assert_eq!(
        serialised_all(&LicenseReviewStatus::ALL),
        schema_enum("LicenseRecord", "review_status")
    );
    assert_eq!(
        serialised_all(&ToolRole::ALL),
        schema_enum("ToolManifest", "role")
    );
    assert_eq!(
        serialised_all(&ReleaseTier::ALL),
        schema_enum("ToolManifest", "release_tier")
    );
    assert_eq!(
        serialised_all(&InvocationMode::ALL),
        schema_enum("ToolManifest", "invocation_mode")
    );
    assert_eq!(
        serialised_all(&SelectionStrategy::ALL),
        schema_enum("ToolManifest", "selection_strategy")
    );
    assert_eq!(
        serialised_all(&NetworkPolicy::ALL),
        schema_enum("ToolManifest", "network_policy")
    );
    assert_eq!(
        serialised_all(&ToolDistribution::ALL),
        schema_enum("ToolManifest", "distribution")
    );
}

/// Compile-time exhaustiveness: adding a variant to a reused enum fails to compile until the lists
/// below are extended.
fn all_export_formats() -> Vec<ExportFormat> {
    fn _exhaustive(f: ExportFormat) {
        match f {
            ExportFormat::Json | ExportFormat::Markdown | ExportFormat::Pdf => {}
        }
    }
    vec![
        ExportFormat::Json,
        ExportFormat::Markdown,
        ExportFormat::Pdf,
    ]
}

fn all_tool_availability() -> Vec<ToolAvailability> {
    fn _exhaustive(a: ToolAvailability) {
        match a {
            ToolAvailability::Ready
            | ToolAvailability::Missing
            | ToolAvailability::Incompatible
            | ToolAvailability::Unconfigured
            | ToolAvailability::Unavailable
            | ToolAvailability::Deferred => {}
        }
    }
    vec![
        ToolAvailability::Ready,
        ToolAvailability::Missing,
        ToolAvailability::Incompatible,
        ToolAvailability::Unconfigured,
        ToolAvailability::Unavailable,
        ToolAvailability::Deferred,
    ]
}

fn all_data_categories() -> Vec<DataCategory> {
    fn _exhaustive(c: DataCategory) {
        match c {
            DataCategory::RepositoryMetadata
            | DataCategory::RedactedSource
            | DataCategory::RedactedFindings
            | DataCategory::SchemaDocuments
            | DataCategory::PublicArtifactIdentifiers
            | DataCategory::RedactedChatContext
            | DataCategory::AppMetadata => {}
        }
    }
    vec![
        DataCategory::RepositoryMetadata,
        DataCategory::RedactedSource,
        DataCategory::RedactedFindings,
        DataCategory::SchemaDocuments,
        DataCategory::PublicArtifactIdentifiers,
        DataCategory::RedactedChatContext,
        DataCategory::AppMetadata,
    ]
}

fn all_approval_lifetimes() -> Vec<ApprovalLifetime> {
    fn _exhaustive(l: ApprovalLifetime) {
        match l {
            ApprovalLifetime::Once | ApprovalLifetime::Run | ApprovalLifetime::WorkflowRevision => {
            }
        }
    }
    vec![
        ApprovalLifetime::Once,
        ApprovalLifetime::Run,
        ApprovalLifetime::WorkflowRevision,
    ]
}

#[test]
fn reused_domain_enums_have_identical_value_sets_in_both_directions() {
    assert_eq!(
        serialised_all(&all_export_formats()),
        schema_items_enum("Workflow", "output_formats")
    );
    assert_eq!(
        serialised_all(&all_tool_availability()),
        schema_enum("ToolManifest", "availability")
    );
    assert_eq!(
        serialised_all(&ApprovalState::ALL),
        schema_enum("Approval", "status")
    );
    assert_eq!(
        serialised_all(&all_data_categories()),
        schema_items_enum("Approval", "data_categories")
    );
    assert_eq!(
        serialised_all(&all_approval_lifetimes()),
        schema_enum("Approval", "lifetime")
    );
    // Shared `$defs` referenced by the new records.
    assert_eq!(
        serialised_all(&Capability::ALL),
        schema_def_enum("Capability")
    );
    assert_eq!(serialised_all(&Category::ALL), schema_def_enum("Category"));
}

#[test]
fn every_wire_value_of_every_enum_field_decodes_in_place() {
    // Each schema value of each enum field is accepted at its field (and nothing else changes).
    for v in schema_enum("Schedule", "kind") {
        roundtrip::<Schedule>(&with(&schedule_min(), "kind", json!(v)));
    }
    for v in schema_enum("Schedule", "overlap_policy") {
        roundtrip::<Schedule>(&with(&schedule_min(), "overlap_policy", json!(v)));
    }
    for v in schema_enum("Schedule", "missed_policy") {
        roundtrip::<Schedule>(&with(&schedule_min(), "missed_policy", json!(v)));
    }
    for v in schema_enum("Repository", "checkout_state") {
        roundtrip::<Repository>(&with(&repository_min(), "checkout_state", json!(v)));
    }
    for v in schema_enum("LicenseRecord", "review_status") {
        roundtrip::<LicenseRecord>(&with(&license_min(), "review_status", json!(v)));
    }
    for v in schema_enum("ToolManifest", "role") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "role", json!(v)));
    }
    for v in schema_enum("ToolManifest", "release_tier") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "release_tier", json!(v)));
    }
    for v in schema_enum("ToolManifest", "availability") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "availability", json!(v)));
    }
    for v in schema_enum("Approval", "status") {
        roundtrip::<Approval>(&with(&approval_min(), "status", json!(v)));
    }
    for v in schema_enum("ExitMeaning", "meaning") {
        roundtrip::<ExitMeaning>(&with(&exit_min(), "meaning", json!(v)));
    }
    for v in schema_enum("Repository", "default_ref_kind") {
        roundtrip::<Repository>(&with(&repository_min(), "default_ref_kind", json!(v)));
    }
    for v in schema_enum("ToolManifest", "invocation_mode") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "invocation_mode", json!(v)));
    }
    for v in schema_enum("ToolManifest", "selection_strategy") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "selection_strategy", json!(v)));
    }
    for v in schema_enum("ToolManifest", "network_policy") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "network_policy", json!(v)));
    }
    for v in schema_enum("ToolManifest", "distribution") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "distribution", json!(v)));
    }
    for v in schema_enum("Approval", "lifetime") {
        roundtrip::<Approval>(&with(&approval_min(), "lifetime", json!(v)));
    }
    for v in schema_def_enum("Capability") {
        roundtrip::<Approval>(&with(&approval_min(), "capability", json!(v)));
    }
    for v in schema_def_enum("Category") {
        roundtrip::<ToolManifest>(&with(&tool_min(), "capabilities", json!([v])));
    }
    for v in schema_items_enum("Workflow", "output_formats") {
        roundtrip::<Workflow>(&with(&workflow_min(), "output_formats", json!([v])));
    }
    for v in schema_items_enum("Approval", "data_categories") {
        roundtrip::<Approval>(&with(&approval_min(), "data_categories", json!([v])));
    }
}

fn roundtrip<T: Serialize + DeserializeOwned>(inst: &Value) {
    let typed: T = serde_json::from_value(inst.clone()).unwrap_or_else(|e| panic!("{inst}: {e}"));
    assert_eq!(&serde_json::to_value(&typed).expect("serialise"), inst);
}

// --- Data Schema examples -------------------------------------------------------------------------------

/// Data Schema §8 example `repo.json`, verbatim.
const REPO_JSON: &str = r#"{
  "schema_version": "1.4",
  "store_type": "repositories",
  "updated_at": "2026-10-16T05:25:00Z",
  "repositories": [
    {
      "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475",
      "display_name": "example-service",
      "slug": "example-service",
      "remote_url": "https://github.com/example/example-service.git",
      "canonical_remote": "github.com/example/example-service",
      "local_path": "checkouts/526a9739-68cb-5ef8-8c3a-0fda8a207475",
      "default_ref_kind": "branch",
      "default_ref_value": "main",
      "registered_at": "2026-10-16T05:25:00Z",
      "last_update_at": null,
      "last_commit_oid": null,
      "checkout_state": "not_cloned",
      "last_error_redacted": null,
      "inventory_revision": 0,
      "inventory_at": null,
      "files": []
    }
  ]
}"#;

/// Data Schema §9 example `workflows.json`, verbatim.
const WORKFLOWS_JSON: &str = r#"{
  "schema_version": "1.4",
  "store_type": "workflows",
  "updated_at": "2026-10-16T05:25:00Z",
  "workflows": [
    {
      "workflow_id": "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d",
      "revision": 1,
      "name": "Daily configuration review",
      "description": "Example only; framework support depends on installed approved adapters.",
      "enabled": false,
      "created_at": "2026-10-16T05:25:00Z",
      "updated_at": "2026-10-16T05:25:00Z",
      "targets": [
        {
          "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475",
          "ref_kind": "branch",
          "ref_value": "main",
          "selector": {
            "mode": "all_yaml",
            "paths": [],
            "include_patterns": [],
            "exclude_patterns": []
          }
        }
      ],
      "scan_settings": {
        "categories": [
          "syntax",
          "schema",
          "secrets",
          "misconfiguration",
          "vulnerability"
        ],
        "secret_scope": "checkout",
        "template_mode": "both",
        "schema_mode": "both",
        "schema_bindings": [],
        "renderers": [],
        "allow_reference_context": true,
        "adapter_ids": []
      },
      "schedule": {
        "kind": "daily",
        "interval_seconds": null,
        "daily_local_time": "10:00",
        "timezone": "Asia/Kolkata",
        "next_due_at": null,
        "overlap_policy": "skip_if_running",
        "missed_policy": "record_no_replay"
      },
      "approval_ids": [],
      "output_formats": [
        "json",
        "md",
        "pdf"
      ]
    }
  ]
}"#;

/// Serialise, parse, serialise again: the two serialisations are equal byte for byte, and the typed value
/// serialises to the example's own JSON value.
fn assert_byte_stable<T>(example: &str)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let typed: T = serde_json::from_str(example).expect("example decodes");
    let first = serde_json::to_string(&typed).expect("serialise");
    let reparsed: T = serde_json::from_str(&first).expect("canonical form decodes");
    assert_eq!(reparsed, typed);
    let second = serde_json::to_string(&reparsed).expect("serialise");
    assert_eq!(first, second, "not byte-stable after one canonicalisation");
    let expected: Value = serde_json::from_str(example).expect("example is JSON");
    assert_eq!(serde_json::to_value(&typed).expect("serialise"), expected);
}

/// The example's own text (trailing newline aside) equals the pretty serialisation of the typed value.
/// This holds only because the example lists its keys in schema order, which makes it a field-order check
/// against an independent source (the Data Schema document) as well as a byte-stability check.
fn assert_pretty_text_equals_example<T>(example: &str)
where
    T: Serialize + DeserializeOwned,
{
    let typed: T = serde_json::from_str(example).expect("example decodes");
    let pretty = serde_json::to_string_pretty(&typed).expect("serialise");
    assert_eq!(pretty, example.trim_end_matches('\n'));
}

#[test]
fn data_schema_section_eight_repo_json_is_byte_stable() {
    assert_byte_stable::<RepositoryFile>(REPO_JSON);
    assert_pretty_text_equals_example::<RepositoryFile>(REPO_JSON);
    let typed: RepositoryFile = serde_json::from_str(REPO_JSON).expect("repo.json");
    assert_eq!(typed.repositories.len(), 1);
    let repo = &typed.repositories[0];
    assert_eq!(repo.checkout_state, CheckoutState::NotCloned);
    assert_eq!(repo.slug.as_str(), "example-service");
    assert_eq!(repo.inventory_revision, 0);
    assert!(repo.last_error_redacted.is_none());
}

#[test]
fn data_schema_section_nine_workflows_json_is_byte_stable() {
    assert_byte_stable::<WorkflowFile>(WORKFLOWS_JSON);
    assert_pretty_text_equals_example::<WorkflowFile>(WORKFLOWS_JSON);
    let typed: WorkflowFile = serde_json::from_str(WORKFLOWS_JSON).expect("workflows.json");
    assert_eq!(typed.workflows.len(), 1);
    let wf = &typed.workflows[0];
    assert_eq!(wf.schedule.kind, ScheduleKind::Daily);
    assert_eq!(wf.schedule.timezone.as_str(), "Asia/Kolkata");
    assert_eq!(
        wf.output_formats,
        [
            ExportFormat::Json,
            ExportFormat::Markdown,
            ExportFormat::Pdf
        ]
    );
    assert!(!wf.enabled);
}

// --- root markers ---------------------------------------------------------------------------------------

/// Minimal instance of every config-side root with its `StoreKind`.
fn root_cases() -> Vec<(StoreKind, Value)> {
    vec![
        (StoreKind::Config, config_file_min()),
        (StoreKind::Repositories, repository_file_min()),
        (StoreKind::Workflows, workflow_file_min()),
        (StoreKind::Approvals, approval_file_min()),
        (StoreKind::ToolManifest, tool_file_min()),
    ]
}

fn decode_root(kind: StoreKind, inst: &Value) -> Result<(), String> {
    let r = match kind {
        StoreKind::Config => serde_json::from_value::<ConfigFile>(inst.clone()).map(drop),
        StoreKind::Repositories => serde_json::from_value::<RepositoryFile>(inst.clone()).map(drop),
        StoreKind::Workflows => serde_json::from_value::<WorkflowFile>(inst.clone()).map(drop),
        StoreKind::Approvals => serde_json::from_value::<ApprovalFile>(inst.clone()).map(drop),
        StoreKind::ToolManifest => {
            serde_json::from_value::<ToolManifestFile>(inst.clone()).map(drop)
        }
        other => panic!("not a config-side root: {other:?}"),
    };
    r.map_err(|e| e.to_string())
}

#[test]
fn every_root_requires_schema_version_1_4_and_its_own_store_type() {
    let cases = root_cases();
    assert_eq!(cases.len(), 5);
    for (kind, inst) in &cases {
        decode_root(*kind, inst).unwrap_or_else(|e| panic!("{kind:?} valid root: {e}"));
        assert_eq!(inst["store_type"], json!(kind.store_type()));

        for wrong in [
            json!("1.3"),
            json!("1.2"),
            json!("1.4.0"),
            json!("9.9"),
            json!(""),
            json!(1.4),
            Value::Null,
        ] {
            let bad = with(inst, "schema_version", wrong.clone());
            assert!(
                decode_root(*kind, &bad).is_err(),
                "{kind:?} accepted schema_version {wrong}"
            );
        }
        assert!(
            decode_root(*kind, &without(inst, "schema_version")).is_err(),
            "{kind:?} accepted a missing schema_version"
        );

        // Every other store type, a wrong case and a non-string are rejected.
        for other in StoreKind::ALL {
            if other == *kind {
                continue;
            }
            let bad = with(inst, "store_type", json!(other.store_type()));
            assert!(
                decode_root(*kind, &bad).is_err(),
                "{kind:?} accepted store_type {}",
                other.store_type()
            );
        }
        for wrong in [
            json!(kind.store_type().to_uppercase()),
            json!(""),
            json!(1),
            Value::Null,
        ] {
            let bad = with(inst, "store_type", wrong.clone());
            assert!(
                decode_root(*kind, &bad).is_err(),
                "{kind:?} accepted store_type {wrong}"
            );
        }
        assert!(
            decode_root(*kind, &without(inst, "store_type")).is_err(),
            "{kind:?} accepted a missing store_type"
        );
    }
}

#[test]
fn root_keys_start_with_schema_version_and_store_type_in_schema_order() {
    // Field order is schema order (request §5.1): the serialised text starts with the two markers.
    let typed: ConfigFile = serde_json::from_value(config_file_min()).expect("config");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        text.starts_with(r#"{"schema_version":"1.4","store_type":"config","updated_at":"#),
        "{text}"
    );
    let typed: ToolManifestFile = serde_json::from_value(tool_file_min()).expect("tools");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        text.starts_with(r#"{"schema_version":"1.4","store_type":"tool_manifest","updated_at":"#),
        "{text}"
    );
}

// --- FR-31: RedactedText / SanitisedText re-escaped on read ---------------------------------------------

const ESC_IN: &str = "a\u{1b}[31m\u{202e}b";
const ESC_OUT: &str = "a<U+001B>[31m<U+202E>b";

#[test]
fn redacted_text_fields_re_escape_esc_and_bidi_override_on_read() {
    // Workflow.name and Workflow.description.
    let inst = with(
        &with(&workflow_min(), "name", json!(ESC_IN)),
        "description",
        json!(ESC_IN),
    );
    let typed: Workflow = serde_json::from_value(inst).expect("workflow");
    assert_eq!(typed.name.as_str(), ESC_OUT);
    assert_eq!(typed.description.as_str(), ESC_OUT);
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{202e}'),
        "{text}"
    );
    assert!(text.contains(ESC_OUT), "{text}");

    // Repository.display_name and Repository.last_error_redacted (the optional helper).
    let inst = with(
        &with(&repository_min(), "display_name", json!(ESC_IN)),
        "last_error_redacted",
        json!(ESC_IN),
    );
    let typed: Repository = serde_json::from_value(inst).expect("repository");
    assert_eq!(typed.display_name.as_str(), ESC_OUT);
    assert_eq!(
        typed.last_error_redacted.as_ref().map(|t| t.as_str()),
        Some(ESC_OUT)
    );
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back["last_error_redacted"], json!(ESC_OUT));

    // Approval.purpose and Approval.actor.
    let inst = with(
        &with(&approval_min(), "purpose", json!(ESC_IN)),
        "actor",
        json!(ESC_IN),
    );
    let typed: Approval = serde_json::from_value(inst).expect("approval");
    assert_eq!(typed.purpose.as_str(), ESC_OUT);
    assert_eq!(typed.actor.as_str(), ESC_OUT);
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{202e}'),
        "{text}"
    );
}

#[test]
fn redacted_text_is_idempotent_across_a_second_read() {
    let inst = with(&workflow_min(), "name", json!(ESC_IN));
    let once: Workflow = serde_json::from_value(inst).expect("workflow");
    let text = serde_json::to_string(&once).expect("serialise");
    let twice: Workflow = serde_json::from_str(&text).expect("second read");
    assert_eq!(twice, once);
    assert_eq!(twice.name.as_str(), ESC_OUT);
}

#[test]
fn sanitised_text_fields_re_escape_on_read() {
    let inst = with(&license_min(), "source_obligation_note", json!(ESC_IN));
    let typed: LicenseRecord = serde_json::from_value(inst).expect("licence");
    assert_eq!(
        typed.source_obligation_note.as_ref().map(|t| t.as_str()),
        Some(ESC_OUT)
    );

    let inst = with(
        &with(&tool_min(), "binary_origin", json!(ESC_IN)),
        "limitations",
        json!([ESC_IN]),
    );
    let typed: ToolManifest = serde_json::from_value(inst).expect("tool");
    assert_eq!(
        typed.binary_origin.as_ref().map(|t| t.as_str()),
        Some(ESC_OUT)
    );
    assert_eq!(typed.limitations.len(), 1);
    assert_eq!(typed.limitations[0].as_str(), ESC_OUT);

    let inst = with(&approval_min(), "resource_constraints", json!([ESC_IN]));
    let typed: Approval = serde_json::from_value(inst).expect("approval");
    assert_eq!(typed.resource_constraints[0].as_str(), ESC_OUT);
}

// --- no echo of planted text ----------------------------------------------------------------------------

#[test]
fn decode_errors_do_not_echo_planted_text() {
    const MARKER: &str = "PLANTEDMARKER";
    // (label, instance, root type tag)
    let planted: Vec<(&str, Value, &str)> = vec![
        (
            "workflow_id",
            with(&workflow_min(), "workflow_id", json!(format!("x-{MARKER}"))),
            "workflow",
        ),
        (
            "approval_ids",
            with(
                &workflow_min(),
                "approval_ids",
                json!([format!("x-{MARKER}")]),
            ),
            "workflow",
        ),
        (
            "timezone",
            with(
                &workflow_min(),
                "schedule",
                with(&schedule_min(), "timezone", json!(format!("bad {MARKER}"))),
            ),
            "workflow",
        ),
        (
            "daily_local_time",
            with(
                &workflow_min(),
                "schedule",
                with(&schedule_min(), "daily_local_time", json!(MARKER)),
            ),
            "workflow",
        ),
        (
            "created_at",
            with(&workflow_min(), "created_at", json!(MARKER)),
            "workflow",
        ),
        (
            "slug",
            with(&repository_min(), "slug", json!(format!("Bad {MARKER}"))),
            "repository",
        ),
        (
            "remote_url",
            with(
                &repository_min(),
                "remote_url",
                json!(format!("https://user:{MARKER}@example.invalid/x.git")),
            ),
            "repository",
        ),
        (
            "canonical_remote",
            with(
                &repository_min(),
                "canonical_remote",
                json!(format!("https://{MARKER}")),
            ),
            "repository",
        ),
        (
            "local_path",
            with(
                &repository_min(),
                "local_path",
                json!(format!("../{MARKER}")),
            ),
            "repository",
        ),
        (
            "default_ref_value",
            with(
                &repository_min(),
                "default_ref_value",
                json!(format!("a..{MARKER}")),
            ),
            "repository",
        ),
        (
            "last_commit_oid",
            with(&repository_min(), "last_commit_oid", json!(MARKER)),
            "repository",
        ),
        (
            "tool_name",
            with(&tool_min(), "tool_name", json!(format!("Bad {MARKER}"))),
            "tool",
        ),
        (
            "spdx",
            with(
                &tool_min(),
                "license",
                with(&license_min(), "spdx", json!(format!("MIT;{MARKER}"))),
            ),
            "tool",
        ),
        (
            "network_resources",
            with(
                &tool_min(),
                "network_resources",
                json!([format!("bad host {MARKER}")]),
            ),
            "tool",
        ),
        (
            "binary_sha256",
            with(&tool_min(), "binary_sha256", json!(MARKER)),
            "tool",
        ),
        (
            "operation_fingerprint",
            with(&approval_min(), "operation_fingerprint", json!(MARKER)),
            "approval",
        ),
        (
            "destinations",
            with(
                &approval_min(),
                "destinations",
                json!([format!("bad host {MARKER}")]),
            ),
            "approval",
        ),
        (
            "app_data_root",
            with(
                &config_min(),
                "app_data_root",
                json!(format!("a/../{MARKER}")),
            ),
            "config",
        ),
        (
            "active_theme",
            with(
                &config_min(),
                "active_theme",
                json!(format!("bad {MARKER}")),
            ),
            "config",
        ),
        (
            "redaction_version",
            with(
                &config_min(),
                "redaction_version",
                json!(format!("bad {MARKER}")),
            ),
            "config",
        ),
        (
            "telemetry_enabled",
            with(&config_min(), "telemetry_enabled", json!(MARKER)),
            "config",
        ),
        (
            "credential_reference",
            with(
                &config_min(),
                "provider",
                with(
                    &provider_max(),
                    "credential_reference",
                    json!(format!("sk-{MARKER}")),
                ),
            ),
            "config",
        ),
    ];
    for (label, inst, root) in planted {
        let from_value = decode_planted(root, &inst, false);
        assert!(
            !from_value.contains(MARKER),
            "{label} (value): {from_value}"
        );
        let from_text = decode_planted(root, &inst, true);
        assert!(!from_text.contains(MARKER), "{label} (text): {from_text}");
    }

    // Root markers.
    for (kind, inst) in root_cases() {
        for key in ["schema_version", "store_type"] {
            let bad = with(&inst, key, json!(MARKER));
            let err = decode_root(kind, &bad).expect_err("planted marker accepted");
            assert!(!err.contains(MARKER), "{kind:?}.{key}: {err}");
        }
    }
}

fn decode_planted(root: &str, inst: &Value, via_text: bool) -> String {
    fn go<T: DeserializeOwned>(inst: &Value, via_text: bool) -> String {
        let r = if via_text {
            let text = serde_json::to_string(inst).expect("text");
            serde_json::from_str::<T>(&text).map(drop)
        } else {
            serde_json::from_value::<T>(inst.clone()).map(drop)
        };
        r.expect_err("planted value accepted").to_string()
    }
    match root {
        "workflow" => go::<Workflow>(inst, via_text),
        "repository" => go::<Repository>(inst, via_text),
        "tool" => go::<ToolManifest>(inst, via_text),
        "approval" => go::<Approval>(inst, via_text),
        "config" => go::<Config>(inst, via_text),
        other => panic!("unknown root tag {other}"),
    }
}

#[test]
fn wrong_typed_values_in_redacted_fields_are_rejected_without_echo() {
    // The number must not be quoted by the decode error (P02-D02 helper behaviour for class R fields).
    const NUMBER: &str = "987654321";
    for inst in [
        with(&workflow_min(), "name", json!(987_654_321u64)),
        with(
            &repository_min(),
            "last_error_redacted",
            json!(987_654_321u64),
        ),
        with(&approval_min(), "actor", json!(987_654_321u64)),
    ] {
        let err = if inst.get("workflow_id").is_some() {
            serde_json::from_value::<Workflow>(inst).expect_err("number accepted")
        } else if inst.get("repo_id").is_some() {
            serde_json::from_value::<Repository>(inst).expect_err("number accepted")
        } else {
            serde_json::from_value::<Approval>(inst).expect_err("number accepted")
        }
        .to_string();
        assert!(!err.contains(NUMBER), "{err}");
    }
}

// --- required nullable keys, presence ---------------------------------------------------------------------

#[test]
fn required_nullable_keys_are_written_as_null_after_a_decode_encode_cycle() {
    let text = serde_json::to_string(
        &serde_json::from_value::<Schedule>(schedule_min()).expect("schedule"),
    )
    .expect("serialise");
    for k in ["interval_seconds", "daily_local_time", "next_due_at"] {
        assert!(text.contains(&format!("\"{k}\":null")), "{k}: {text}");
    }
    let text = serde_json::to_string(
        &serde_json::from_value::<Repository>(repository_min()).expect("repository"),
    )
    .expect("serialise");
    for k in [
        "last_update_at",
        "last_commit_oid",
        "last_error_redacted",
        "inventory_at",
    ] {
        assert!(text.contains(&format!("\"{k}\":null")), "{k}: {text}");
    }
    let text = serde_json::to_string(
        &serde_json::from_value::<Approval>(approval_min()).expect("approval"),
    )
    .expect("serialise");
    for k in [
        "workflow_id",
        "workflow_revision",
        "run_id",
        "decided_at",
        "expires_at",
        "inference_scope",
    ] {
        assert!(text.contains(&format!("\"{k}\":null")), "{k}: {text}");
    }
    let text = serde_json::to_string(&serde_json::from_value::<Config>(config_min()).expect("cfg"))
        .expect("serialise");
    assert!(text.contains("\"history_retention_days\":null"), "{text}");
    assert!(text.contains("\"provider\":null"), "{text}");
    assert!(text.contains("\"telemetry_enabled\":false"), "{text}");
}

#[test]
fn a_redacted_optional_key_must_be_present_while_plain_optional_keys_default_to_none() {
    // Pins current behaviour. `last_error_redacted` uses a `deserialize_with` helper, so serde requires the
    // key; the other nullable keys are plain `Option`s and serde reads a missing key as `None`. The schema's
    // `required` list is the guard for the latter (T08).
    assert!(
        serde_json::from_value::<Repository>(without(&repository_min(), "last_error_redacted"))
            .is_err()
    );
    let typed: Repository =
        serde_json::from_value(without(&repository_min(), "last_update_at")).expect("decodes");
    assert!(typed.last_update_at.is_none());
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back, repository_min());
}

// --- order helpers can fail ---------------------------------------------------------------------------

#[test]
fn key_order_helpers_keep_document_order_rather_than_sorting() {
    // `serde_json::Value` sorts keys (no `preserve_order`); the visitor based helpers must not.
    let text = r#"{"z":1,"$defs":{"D":{"properties":{"b":1,"a":{"c":0,"b":0},"c":2}}},"a":3}"#;
    assert_eq!(serialised_key_order(text), ["z", "$defs", "a"]);
    assert_eq!(
        keys_at(text, &["$defs", "D", "properties"]),
        ["b", "a", "c"]
    );
    assert_eq!(keys_at(text, &["$defs", "D", "properties"]).len(), 3);
    // The same text through `Value` is sorted, which is why it cannot be used to check order.
    let sorted: Vec<String> = serde_json::from_str::<Value>(text)
        .expect("json")
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect();
    assert_eq!(sorted, ["$defs", "a", "z"]);
}

#[test]
fn schema_property_order_is_not_alphabetical_for_at_least_one_def() {
    // Guards against reading the schema through a sorting parser: Workflow lists `workflow_id` first.
    let order = schema_property_order("Workflow");
    assert_eq!(order.first().map(String::as_str), Some("workflow_id"));
    let mut sorted = order.clone();
    sorted.sort();
    assert_ne!(order, sorted);
}

// --- FR-31 class R source scan -------------------------------------------------------------------------

/// Parses the `pub struct` blocks of a source file and returns `(struct, field, type, preceding
/// deserialize_with attribute)` for every field whose type mentions `RedactedText`.
fn redacted_fields(src: &str) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut attr = String::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pub struct ") {
            current = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_owned();
            attr.clear();
        } else if t.starts_with("#[serde(deserialize_with") {
            attr = t.to_owned();
        } else if t.starts_with("///") || t.starts_with("#[") {
            // docs and other attributes keep the pending attribute
        } else if let Some(field) = t.strip_prefix("pub ")
            && let Some((name, ty)) = field.split_once(':')
        {
            if ty.contains("RedactedText") {
                out.push((
                    current.clone(),
                    name.trim().to_owned(),
                    ty.trim().to_owned(),
                    attr.clone(),
                ));
            }
            attr.clear();
        } else {
            attr.clear();
        }
    }
    out
}

#[test]
fn every_redacted_text_field_uses_the_store_read_helper_and_matches_the_classification() {
    let sources = [
        include_str!("../src/store/workflow.rs"),
        include_str!("../src/store/repository.rs"),
        include_str!("../src/store/tools.rs"),
        include_str!("../src/store/approval.rs"),
        include_str!("../src/store/config.rs"),
    ];
    let mut found = BTreeSet::new();
    for src in sources {
        for (strukt, field, ty, attr) in redacted_fields(src) {
            let helper = match ty.trim_end_matches(',') {
                "RedactedText" => "store_read::redacted\")",
                "Option<RedactedText>" => "store_read::redacted_opt\")",
                "Vec<RedactedText>" => "store_read::redacted_vec\")",
                other => panic!("{strukt}.{field}: unexpected RedactedText shape {other}"),
            };
            assert!(
                attr.contains(helper),
                "{strukt}.{field}: missing `{helper}` in `{attr}`"
            );
            found.insert(format!("{strukt}.{field}"));
        }
    }
    // Appendix A class R properties of the thirteen defs of this task (6 of the 27 R properties;
    // `planning/P02-T01-fr31-classification.md` rows 13, 19, 22, 23, 24, 27). The other 21 belong to the
    // twenty defs of P02-T06, and `RendererSetting.parameters_redacted` to P02-T05a.
    let expected: BTreeSet<String> = [
        "Workflow.name",
        "Workflow.description",
        "Repository.display_name",
        "Repository.last_error_redacted",
        "Approval.purpose",
        "Approval.actor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn redacted_field_scanner_detects_a_missing_or_wrong_helper() {
    // The scanner itself can fail: a field without the attribute, or with the wrong one, is visible.
    let src = "pub struct S {\n    pub a: RedactedText,\n    #[serde(deserialize_with = \"crate::text::store_read::redacted\")]\n    pub b: Option<RedactedText>,\n    #[serde(deserialize_with = \"crate::text::store_read::redacted_opt\")]\n    pub c: Option<RedactedText>,\n}\n";
    let fields = redacted_fields(src);
    assert_eq!(fields.len(), 3);
    assert!(fields[0].3.is_empty(), "a has no helper");
    assert!(
        !fields[1].3.contains("store_read::redacted_opt\")"),
        "b has the wrong helper"
    );
    assert!(fields[2].3.contains("store_read::redacted_opt\")"));
}

// --- constraints the typed layer does not enforce (pinned, so a change is noticed) ----------------------

#[test]
fn schema_minimums_and_collection_rules_are_not_enforced_by_the_typed_layer() {
    // These are caught by schema validation (T08) and the semantic rules (T09); the typed layer accepts
    // them. If a later change starts enforcing one, this test names it.
    let wf = with(&workflow_min(), "revision", json!(0));
    assert!(serde_json::from_value::<Workflow>(wf).is_ok(), "revision 0");
    let wf = with(&workflow_min(), "output_formats", json!([]));
    assert!(serde_json::from_value::<Workflow>(wf).is_ok(), "minItems");
    let wf = with(&workflow_min(), "output_formats", json!(["json", "json"]));
    assert!(
        serde_json::from_value::<Workflow>(wf).is_ok(),
        "uniqueItems"
    );
    let sc = with(&schedule_min(), "interval_seconds", json!(0));
    assert!(serde_json::from_value::<Schedule>(sc).is_ok(), "interval 0");
    let t = with(&tool_min(), "timeout_seconds", json!(0));
    assert!(
        serde_json::from_value::<ToolManifest>(t).is_ok(),
        "timeout 0"
    );
    let t = with(&tool_min(), "max_output_bytes", json!(0));
    assert!(serde_json::from_value::<ToolManifest>(t).is_ok(), "max 0");
    let a = with(&approval_min(), "workflow_revision", json!(0));
    assert!(serde_json::from_value::<Approval>(a).is_ok(), "revision 0");
    let c = with(&config_min(), "max_active_repositories", json!(0));
    assert!(serde_json::from_value::<Config>(c).is_ok(), "repos 0");
    let c = with(&config_min(), "max_scanner_processes", json!(0));
    assert!(serde_json::from_value::<Config>(c).is_ok(), "processes 0");
    let c = with(&config_min(), "history_retention_days", json!(0));
    assert!(serde_json::from_value::<Config>(c).is_ok(), "retention 0");
}

/// Planted values for an enum field `base[field]`: a marker string, the valid wire value with the marker
/// appended, a marker number (unsigned, negative, float), and a marker inside an array and an object.
fn planted_values(valid: &str) -> Vec<Value> {
    vec![
        json!(ENUM_MARKER),
        json!(format!("{valid}{ENUM_MARKER}")),
        json!(format!("{ENUM_MARKER}{valid}")),
        json!(ENUM_MARKER_NUMBER),
        json!(-i64::try_from(ENUM_MARKER_NUMBER).expect("fits")),
        json!(6_543.210_987_6_f64),
        json!([ENUM_MARKER]),
        json!({ ENUM_MARKER: ENUM_MARKER }),
    ]
}

const ENUM_MARKER: &str = "PLANTEDENUMMARKER";
const ENUM_MARKER_NUMBER: u64 = 7_654_321_987;

/// Plants every marker value in `field` of `base`, decoding as `T` from a `Value` and from JSON text. The
/// decode must fail and the error text must contain neither the marker string nor any marker number. This
/// fails if the field's enum goes back to a derived `Deserialize` (`unknown variant `PLANTED...``,
/// `invalid type: integer `7654321987``).
fn assert_enum_field_does_not_echo<T: DeserializeOwned>(label: &str, base: &Value, field: &str) {
    let valid = base[field]
        .as_str()
        .expect("base holds a wire string")
        .to_owned();
    for planted in planted_values(&valid) {
        let inst = with(base, field, planted.clone());
        let text = serde_json::to_string(&inst).expect("serialise instance");
        for (how, err) in [
            (
                "value",
                serde_json::from_value::<T>(inst.clone())
                    .map(drop)
                    .expect_err("planted enum value accepted"),
            ),
            (
                "text",
                serde_json::from_str::<T>(&text)
                    .map(drop)
                    .expect_err("planted enum value accepted from text"),
            ),
        ] {
            let err = err.to_string();
            for needle in [ENUM_MARKER, "7654321987", "6543.2109876"] {
                assert!(
                    !err.contains(needle),
                    "{label}.{field} ({how}) echoed {needle} for {planted}: {err}"
                );
            }
        }
    }
}

/// `Struct.field` of every `pub field: <one of enums>` in `src` (the model sources are read as text so a new
/// enum-typed field cannot be added without extending the planted-marker test).
fn enum_typed_fields(src: &str, enums: &[&str]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut current = String::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pub struct ") {
            current = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .to_owned();
        } else if let Some(field) = t.strip_prefix("pub ")
            && let Some((name, ty)) = field.split_once(':')
            && enums.contains(&ty.trim().trim_end_matches(','))
        {
            out.insert(format!("{current}.{}", name.trim()));
        }
    }
    out
}

/// Every closed-enum type of the store models (P02-D05 E-1) except those defined in `log.rs`/`report.rs`.
const D05_ENUMS: &[&str] = &[
    "RefKind",
    "SecretScope",
    "TemplateMode",
    "SchemaMode",
    "DiscoveryState",
    "EngineAvailability",
    "ProviderCredentialMode",
    "ScheduleKind",
    "OverlapPolicy",
    "MissedPolicy",
    "CheckoutState",
    "ExitMeaningKind",
    "LicenseReviewStatus",
    "ToolRole",
    "ReleaseTier",
    "InvocationMode",
    "SelectionStrategy",
    "NetworkPolicy",
    "ToolDistribution",
];

/// For each listed `closed_enum!` type: `as_str` equals the serialised value, `from_wire` and `Deserialize`
/// invert it, wire strings are distinct, and near-misses are rejected. Fails if a variant's `as_str` and
/// serialisation diverge, `ALL` omits or repeats a variant, or `from_wire` accepts a non-wire string.
macro_rules! wire_agrees {
    ($($ty:ident),+ $(,)?) => {$(
        let mut seen = BTreeSet::new();
        for v in $ty::ALL {
            let s = v.as_str();
            assert!(seen.insert(s), "{}: {s} listed twice", stringify!($ty));
            assert_eq!(serde_json::to_value(v).expect("serialise"), json!(s), "{}", stringify!($ty));
            assert_eq!($ty::from_wire(s), Some(v), "{}: {s}", stringify!($ty));
            assert_eq!(
                serde_json::from_value::<$ty>(json!(s)).expect("decode wire value"),
                v,
                "{}: {s}",
                stringify!($ty)
            );
            for near in [s.to_uppercase(), format!("{s} "), format!(" {s}"), format!("{s}x"), String::new()] {
                assert_eq!($ty::from_wire(&near), None, "{}: {near:?}", stringify!($ty));
                assert!(
                    serde_json::from_value::<$ty>(json!(near)).is_err(),
                    "{}: {near:?}",
                    stringify!($ty)
                );
            }
        }
        assert!($ty::from_wire("not_a_value").is_none(), "{}", stringify!($ty));
    )+};
}

#[test]
fn d05_every_workflow_repository_and_tool_enum_field_rejects_planted_text_and_numbers_without_echo()
{
    for field in ["kind", "overlap_policy", "missed_policy"] {
        assert_enum_field_does_not_echo::<Schedule>("Schedule", &schedule_min(), field);
    }
    for field in ["default_ref_kind", "checkout_state"] {
        assert_enum_field_does_not_echo::<Repository>("Repository", &repository_min(), field);
    }
    assert_enum_field_does_not_echo::<ExitMeaning>("ExitMeaning", &exit_min(), "meaning");
    assert_enum_field_does_not_echo::<LicenseRecord>(
        "LicenseRecord",
        &license_min(),
        "review_status",
    );
    for field in [
        "role",
        "release_tier",
        "invocation_mode",
        "selection_strategy",
        "network_policy",
        "distribution",
    ] {
        assert_enum_field_does_not_echo::<ToolManifest>("ToolManifest", &tool_min(), field);
    }

    // The list above is complete: it equals the enum-typed fields found in the model sources.
    let mut found = BTreeSet::new();
    for src in [
        include_str!("../src/store/workflow.rs"),
        include_str!("../src/store/repository.rs"),
        include_str!("../src/store/tools.rs"),
    ] {
        found.extend(enum_typed_fields(src, D05_ENUMS));
    }
    let planted: BTreeSet<String> = [
        "Schedule.kind",
        "Schedule.overlap_policy",
        "Schedule.missed_policy",
        "Repository.default_ref_kind",
        "Repository.checkout_state",
        "ExitMeaning.meaning",
        "LicenseRecord.review_status",
        "ToolManifest.role",
        "ToolManifest.release_tier",
        "ToolManifest.invocation_mode",
        "ToolManifest.selection_strategy",
        "ToolManifest.network_policy",
        "ToolManifest.distribution",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(found, planted);
}

#[test]
fn d05_workflow_repository_and_tool_enums_agree_between_as_str_from_wire_and_serde() {
    wire_agrees!(
        ScheduleKind,
        OverlapPolicy,
        MissedPolicy,
        CheckoutState,
        ExitMeaningKind,
        LicenseReviewStatus,
        ToolRole,
        ReleaseTier,
        InvocationMode,
        SelectionStrategy,
        NetworkPolicy,
        ToolDistribution,
    );
}

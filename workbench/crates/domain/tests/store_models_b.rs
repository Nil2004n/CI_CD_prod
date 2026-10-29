//! P02-T06 evidence-side store records (`planning/P02-T01-request.md` §5.1, §9 row T06). All data is synthetic.
//!
//! Covers SourceRef, Engine, Artifact, Check, Finding, CoverageSummary, AIEnrichment, ReportFile, Event,
//! RunSummary, ReportManifest, LogFile, Suppression, SuppressionFile, Chunk, RagIndexFile, ChatContext,
//! Message, Conversation and ConversationFile.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use domain::contract::AdapterId;
use domain::states::{CheckOutcome, RunState};
use domain::store::StoreKind;
use domain::store::conversation::{
    ChatContext, Conversation, ConversationFile, Message, MessageRole, MessageStatus,
};
use domain::store::evidence::{Artifact, ArtifactKind, Engine, SourceKind, SourceRef};
use domain::store::log::{
    Event, EventLevel, EventType, LogFile, ReportManifest, RunSummary, RunTrigger,
};
use domain::store::rag::{Chunk, RagIndexFile};
use domain::store::report::{
    AIEnrichment, Check, Confidence, CoverageSummary, EnrichmentStatus, Finding, NativeStatus,
    ReportFile, ReviewStatus,
};
use domain::store::suppression::{Suppression, SuppressionFile};
use domain::vocab::{Assessment, Category, Coverage, Severity};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");
const CONTRACT_SRC: &str = include_str!("../src/contract.rs");

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OID: &str = "0123456789abcdef0123456789abcdef01234567";
const TS: &str = "2026-10-16T05:25:00Z";
const TS_FRACTION: &str = "2026-10-16T05:25:00.123456789Z";
const TS_LATER: &str = "2026-10-16T06:00:00Z";

/// A fake, non-nil identifier.
fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

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

/// Properties of `def` whose schema is `anyOf [.., {"type": "null"}]`.
fn def_nullable(def: &str) -> BTreeSet<String> {
    let schema = schema();
    let props = schema["$defs"][def]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{def} has no properties"));
    props
        .iter()
        .filter(|(_, p)| {
            p["anyOf"]
                .as_array()
                .is_some_and(|alts| alts.iter().any(|a| *a == json!({"type": "null"})))
        })
        .map(|(k, _)| k.clone())
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

/// The members of the JSON object whose `{` is at byte `start` of `text`, as `(key, value start offset)` in
/// text order. `serde_json` here has no `preserve_order`, so a `Value` cannot tell the order of a schema's
/// `properties` or of a serialisation; this scanner reads it from the text. Strings (with escapes) and
/// nested objects and arrays are skipped as units.
fn members(text: &str, start: usize) -> Vec<(String, usize)> {
    fn skip_ws(b: &[u8], mut i: usize) -> usize {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    }
    fn skip_string(b: &[u8], start: usize) -> usize {
        assert_eq!(b[start], b'"', "string expected at {start}");
        let mut i = start + 1;
        while b[i] != b'"' {
            i += if b[i] == b'\\' { 2 } else { 1 };
        }
        i + 1
    }
    fn skip_value(b: &[u8], start: usize) -> usize {
        match b[start] {
            b'"' => skip_string(b, start),
            b'{' | b'[' => {
                let mut depth = 0_usize;
                let mut i = start;
                loop {
                    match b[i] {
                        b'"' => {
                            i = skip_string(b, i);
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {
                let mut i = start;
                while !matches!(b[i], b',' | b'}' | b']') && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                i
            }
        }
    }

    let b = text.as_bytes();
    assert_eq!(b[start], b'{', "object expected at {start}");
    let mut out = Vec::new();
    let mut i = start + 1;
    loop {
        i = skip_ws(b, i);
        if b[i] == b',' {
            i = skip_ws(b, i + 1);
        }
        if b[i] == b'}' {
            return out;
        }
        let key_end = skip_string(b, i);
        let key = &text[i + 1..key_end - 1];
        assert!(!key.contains('\\'), "escaped key {key}");
        i = skip_ws(b, key_end);
        assert_eq!(b[i], b':', "colon expected after {key}");
        i = skip_ws(b, i + 1);
        out.push((key.to_owned(), i));
        i = skip_value(b, i);
    }
}

/// Start offset of the object that is the value of `key` in the object at `start`.
fn child(text: &str, start: usize, key: &str) -> usize {
    members(text, start)
        .into_iter()
        .find_map(|(k, at)| (k == key).then_some(at))
        .unwrap_or_else(|| panic!("no member {key}"))
}

/// The keys of `properties` of the schema `$def`, in the order of the schema text.
fn schema_property_order(def: &str) -> Vec<String> {
    let root = SCHEMA.find('{').expect("schema is an object");
    let defs = child(SCHEMA, root, "$defs");
    let def_at = child(SCHEMA, defs, def);
    let props = child(SCHEMA, def_at, "properties");
    members(SCHEMA, props).into_iter().map(|(k, _)| k).collect()
}

/// The top-level key order of the compact serialisation of `typed` (read from the text, not from a `Value`).
fn serialised_key_order<T: Serialize>(typed: &T) -> Vec<String> {
    let text = serde_json::to_string(typed).expect("serialise");
    members(&text, 0).into_iter().map(|(k, _)| k).collect()
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

fn decode_err<T: DeserializeOwned + Debug>(inst: Value) -> String {
    serde_json::from_value::<T>(inst)
        .expect_err("input must be rejected")
        .to_string()
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
    // The nullable list is exactly the schema's nullable properties.
    let listed: BTreeSet<String> = d.nullable.iter().map(|k| (*k).to_owned()).collect();
    assert_eq!(listed, def_nullable(d.name), "{} nullable list", d.name);

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

    // Field order: the serialised key order (compact text) is the order of the `$def`'s `properties` in the
    // schema text, for the maximal and the minimal instance. Fails if a struct field is moved.
    let schema_order = schema_property_order(d.name);
    assert_eq!(
        schema_order.iter().cloned().collect::<BTreeSet<_>>(),
        def_properties(d.name),
        "{}: order scan finds the same properties",
        d.name
    );
    assert_eq!(
        serialised_key_order(&max_typed),
        schema_order,
        "{} maximal key order",
        d.name
    );
    assert_eq!(
        serialised_key_order(&typed),
        schema_order,
        "{} minimal key order",
        d.name
    );
    assert_eq!(
        def_required(d.name),
        def_properties(d.name),
        "{} required",
        d.name
    );
}

// --- instances ------------------------------------------------------------------------------------------

fn source_min() -> Value {
    json!({
        "source_id": uid(1),
        "kind": "repository_file",
        "repo_id": null,
        "commit_oid": null,
        "run_id": null,
        "file_path": null,
        "artifact_id": null,
        "document_index": null,
        "line_start": null,
        "line_end": null,
        "resource_identifier": null,
        "content_digest": null,
        "retrieved_at": TS,
        "approval_id": null,
        "redaction_version": "redact-1",
        "stale": false
    })
}

fn source_max() -> Value {
    json!({
        "source_id": uid(2),
        "kind": "external_reference",
        "repo_id": uid(10),
        "commit_oid": OID,
        "run_id": uid(20),
        "file_path": ".github/workflows/ci.yml",
        "artifact_id": uid(30),
        "document_index": 2,
        "line_start": 3,
        "line_end": 18_446_744_073_709_551_615u64,
        "resource_identifier": "https://example.invalid/advisory/FAKE-0001",
        "content_digest": DIGEST,
        "retrieved_at": TS_FRACTION,
        "approval_id": uid(40),
        "redaction_version": "redact-1",
        "stale": true
    })
}

fn engine_min() -> Value {
    json!({
        "adapter_id": "yamllint",
        "adapter_version": "1.0",
        "tool_name": "yamllint",
        "tool_version": "1.35.1",
        "ruleset_version": null,
        "data_updated_at": null
    })
}

fn engine_max() -> Value {
    json!({
        "adapter_id": "docker-compose",
        "adapter_version": "0.1.0+local",
        "tool_name": "docker-compose",
        "tool_version": "2.29.0",
        "ruleset_version": "2026.10",
        "data_updated_at": TS_FRACTION
    })
}

fn artifact_min() -> Value {
    json!({
        "artifact_id": uid(30),
        "kind": "rendered",
        "display_name": "ci.yml",
        "content_digest": DIGEST,
        "retained_path": null,
        "source_refs": [],
        "approval_id": null,
        "contains_only_redacted_retained_content": false
    })
}

fn artifact_max() -> Value {
    json!({
        "artifact_id": uid(31),
        "kind": "rule_document",
        "display_name": "rule-doc.md",
        "content_digest": DIGEST2,
        "retained_path": "backup/artifacts/rule-doc.md",
        "source_refs": [source_min(), source_max()],
        "approval_id": uid(40),
        "contains_only_redacted_retained_content": true
    })
}

fn check_min() -> Value {
    json!({
        "check_id": uid(50),
        "task_id": uid(60),
        "file_id": null,
        "category": "syntax",
        "framework": null,
        "engine": engine_min(),
        "outcome": "pending",
        "reason_redacted": null,
        "started_at": null,
        "finished_at": null,
        "finding_ids": [],
        "source_refs": [],
        "adapter_id": "yamllint",
        "config_digest": null,
        "manifest_digest": null,
        "exit_code": null,
        "term_signal": null,
        "native_status": "not_started",
        "approval_ids": []
    })
}

fn check_max() -> Value {
    json!({
        "check_id": uid(51),
        "task_id": uid(61),
        "file_id": uid(70),
        "category": "secrets",
        "framework": "github-actions",
        "engine": engine_max(),
        "outcome": "findings",
        "reason_redacted": "Tool reported findings",
        "started_at": TS,
        "finished_at": TS_LATER,
        "finding_ids": [uid(80), uid(81)],
        "source_refs": [source_max()],
        "adapter_id": "gitleaks",
        "config_digest": DIGEST,
        "manifest_digest": DIGEST2,
        "exit_code": -2,
        "term_signal": 9,
        "native_status": "output_limit_exceeded",
        "approval_ids": [uid(40)]
    })
}

fn finding_min() -> Value {
    json!({
        "finding_id": uid(80),
        "fingerprint": DIGEST,
        "check_id": uid(50),
        "task_id": uid(60),
        "repo_id": uid(10),
        "run_id": uid(20),
        "commit_oid": OID,
        "category": "syntax",
        "severity": "info",
        "confidence": "unknown",
        "rule_id": "FAKE001",
        "title": "Synthetic finding",
        "description_redacted": "",
        "evidence_redacted": "",
        "remediation_redacted": "",
        "engine": engine_min(),
        "sources": [],
        "review_status": "new",
        "suppression_id": null,
        "observed_at": TS
    })
}

fn finding_max() -> Value {
    json!({
        "finding_id": uid(81),
        "fingerprint": DIGEST2,
        "check_id": uid(51),
        "task_id": uid(61),
        "repo_id": uid(11),
        "run_id": uid(21),
        "commit_oid": DIGEST,
        "category": "vulnerability",
        "severity": "critical",
        "confidence": "high",
        "rule_id": "FAKE-RULE-0002",
        "title": "Synthetic critical finding",
        "description_redacted": "A synthetic description.",
        "evidence_redacted": "key: [REDACTED]",
        "remediation_redacted": "Replace the synthetic value.",
        "engine": engine_max(),
        "sources": [source_min(), source_max()],
        "review_status": "suppressed",
        "suppression_id": uid(90),
        "observed_at": TS_FRACTION
    })
}

fn coverage_min() -> Value {
    json!({
        "discovered_files": 0, "selected_files": 0, "assessed_files": 0, "excluded_files": 0,
        "missing_files": 0, "checks_passed": 0, "checks_with_findings": 0, "checks_failed": 0,
        "checks_unsupported": 0, "checks_skipped": 0, "checks_incomplete": 0, "checks_cancelled": 0,
        "coverage": "none",
        "assessment": "unknown"
    })
}

fn coverage_max() -> Value {
    json!({
        "discovered_files": 18_446_744_073_709_551_615u64, "selected_files": 12, "assessed_files": 11,
        "excluded_files": 3, "missing_files": 1, "checks_passed": 7, "checks_with_findings": 2,
        "checks_failed": 1, "checks_unsupported": 1, "checks_skipped": 1, "checks_incomplete": 1,
        "checks_cancelled": 1,
        "coverage": "partial",
        "assessment": "findings"
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

fn ai_min() -> Value {
    json!({
        "status": "not_requested",
        "provider_id": null,
        "requested_model": null,
        "actual_model": null,
        "actual_provider": null,
        "approval_ids": [],
        "source_ids": [],
        "routing_policy": null,
        "redaction_version": "redact-1",
        "generated_at": null,
        "narrative_redacted": "",
        "reason_redacted": null
    })
}

fn ai_max() -> Value {
    json!({
        "status": "completed",
        "provider_id": "openrouter",
        "requested_model": "anthropic/fake-model",
        "actual_model": "anthropic/fake-model-2",
        "actual_provider": "anthropic",
        "approval_ids": [uid(40)],
        "source_ids": [uid(1), uid(2)],
        "routing_policy": policy_max(),
        "redaction_version": "redact-1",
        "generated_at": TS_FRACTION,
        "narrative_redacted": "A synthetic narrative.",
        "reason_redacted": "A synthetic reason."
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
        "workflow_id": uid(100),
        "workflow_revision": 3,
        "targets": [{
            "repo_id": uid(10),
            "ref_kind": "branch",
            "ref_value": "main",
            "selector": {"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []}
        }],
        "scan_settings": scan_example(),
        "approval_ids": [uid(40)],
        "created_at": TS_FRACTION,
        "plan_digest": DIGEST2,
        "target_snapshots": [{
            "repo_id": uid(10),
            "selected_ref": "main",
            "commit_oid": OID,
            "inventory_revision": 7,
            "selected_files": [],
            "missing_paths": [],
            "snapshot_at": TS
        }],
        "engine_snapshots": [{
            "adapter_id": "yamllint",
            "adapter_version": "1.0",
            "tool_name": "yamllint",
            "tool_version": "1.35.1",
            "ruleset_version": null,
            "config_digest": null,
            "manifest_digest": DIGEST,
            "availability": "ready",
            "data_updated_at": null
        }]
    })
}

fn report_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "report",
        "updated_at": TS,
        "report_id": uid(110),
        "run_id": uid(20),
        "repo_id": uid(10),
        "repo_name": "demo-repo",
        "remote_identifier_redacted": "example.invalid/demo",
        "selected_ref": "main",
        "commit_oid": null,
        "execution": "queued",
        "plan": plan_min(),
        "generated_at": TS,
        "coverage": coverage_min(),
        "checks": [],
        "findings": [],
        "artifacts": [],
        "limitations": [],
        "ai_enrichment": ai_min()
    })
}

fn report_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "report",
        "updated_at": TS_FRACTION,
        "report_id": uid(111),
        "run_id": uid(21),
        "repo_id": uid(11),
        "repo_name": "demo-repo-two",
        "remote_identifier_redacted": "example.invalid/demo-two",
        "selected_ref": "release/2026.10",
        "commit_oid": OID,
        "execution": "partial",
        "plan": plan_max(),
        "generated_at": TS_LATER,
        "coverage": coverage_max(),
        "checks": [check_min(), check_max()],
        "findings": [finding_min(), finding_max()],
        "artifacts": [artifact_min(), artifact_max()],
        "limitations": ["Synthetic limitation one.", "Synthetic limitation two."],
        "ai_enrichment": ai_max()
    })
}

fn event_min() -> Value {
    json!({
        "event_id": uid(120),
        "at": TS,
        "level": "info",
        "event_type": "RunStarted",
        "run_id": null,
        "repo_id": null,
        "workflow_id": null,
        "task_id": null,
        "approval_id": null,
        "message_redacted": "Run started",
        "code": null
    })
}

fn event_max() -> Value {
    json!({
        "event_id": uid(121),
        "at": TS_FRACTION,
        "level": "error",
        "event_type": "StorageFailure",
        "run_id": uid(20),
        "repo_id": uid(10),
        "workflow_id": uid(100),
        "task_id": uid(60),
        "approval_id": uid(40),
        "message_redacted": "Synthetic failure detail",
        "code": "storage_failed"
    })
}

fn run_min() -> Value {
    json!({
        "run_id": uid(20),
        "trigger": "manual",
        "execution": "queued",
        "created_at": TS,
        "started_at": null,
        "finished_at": null,
        "plan": plan_min(),
        "coverage": coverage_min(),
        "report_ids": [],
        "last_checkpoint_at": null
    })
}

fn run_max() -> Value {
    json!({
        "run_id": uid(21),
        "trigger": "daily",
        "execution": "completed",
        "created_at": TS,
        "started_at": TS_FRACTION,
        "finished_at": TS_LATER,
        "plan": plan_max(),
        "coverage": coverage_max(),
        "report_ids": [uid(110), uid(111)],
        "last_checkpoint_at": TS_LATER
    })
}

fn manifest_min() -> Value {
    json!({
        "report_id": uid(110),
        "run_id": uid(20),
        "repo_id": uid(10),
        "json_path": "backup/report-fake.json",
        "md_path": "backup/report-fake.md",
        "json_written": false,
        "md_written": false,
        "json_digest": null,
        "generated_at": TS,
        "pdf_path": null,
        "pdf_written": false,
        "pdf_digest": null,
        "pdf_error_redacted": null,
        "pdf_renderer_version": null
    })
}

fn manifest_max() -> Value {
    json!({
        "report_id": uid(111),
        "run_id": uid(21),
        "repo_id": uid(11),
        "json_path": "backup/report-fake-2.json",
        "md_path": "backup/report-fake-2.md",
        "json_written": true,
        "md_written": true,
        "json_digest": DIGEST,
        "generated_at": TS_FRACTION,
        "pdf_path": "backup/report-fake-2.pdf",
        "pdf_written": false,
        "pdf_digest": DIGEST2,
        "pdf_error_redacted": "Synthetic renderer error",
        "pdf_renderer_version": "renderer-0.1"
    })
}

fn log_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "log",
        "updated_at": TS,
        "runs": [],
        "events": [],
        "reports": []
    })
}

fn log_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "log",
        "updated_at": TS_FRACTION,
        "runs": [run_min(), run_max()],
        "events": [event_min(), event_max()],
        "reports": [manifest_min(), manifest_max()]
    })
}

fn suppression_min() -> Value {
    json!({
        "suppression_id": uid(90),
        "repo_id": uid(10),
        "finding_fingerprint": DIGEST,
        "rule_id": "FAKE001",
        "reason": "Synthetic reason",
        "created_at": TS,
        "expires_at": null,
        "actor": "fake-user",
        "enabled": true
    })
}

fn suppression_max() -> Value {
    json!({
        "suppression_id": uid(91),
        "repo_id": uid(11),
        "finding_fingerprint": DIGEST2,
        "rule_id": "FAKE-RULE-0002",
        "reason": "Another synthetic reason",
        "created_at": TS_FRACTION,
        "expires_at": TS_LATER,
        "actor": "fake-user-two",
        "enabled": false
    })
}

fn suppression_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "suppressions",
        "updated_at": TS,
        "suppressions": []
    })
}

fn suppression_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "suppressions",
        "updated_at": TS_FRACTION,
        "suppressions": [suppression_min(), suppression_max()]
    })
}

fn chunk_min() -> Value {
    json!({
        "chunk_id": uid(130),
        "source": source_min(),
        "text_redacted": "",
        "content_digest": DIGEST,
        "redaction_version": "redact-1",
        "embedding_key": null,
        "indexed_at": TS
    })
}

fn chunk_max() -> Value {
    json!({
        "chunk_id": uid(131),
        "source": source_max(),
        "text_redacted": "synthetic: value",
        "content_digest": DIGEST2,
        "redaction_version": "redact-1",
        "embedding_key": "emb:fake-0001",
        "indexed_at": TS_FRACTION
    })
}

fn rag_min() -> Value {
    json!({"schema_version": "1.4", "store_type": "rag_index", "updated_at": TS, "chunks": []})
}

fn rag_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "rag_index",
        "updated_at": TS_FRACTION,
        "chunks": [chunk_min(), chunk_max()]
    })
}

fn context_min() -> Value {
    json!({
        "repo_ids": [], "run_ids": [], "finding_ids": [], "file_ids": [],
        "include_history": false
    })
}

fn context_max() -> Value {
    json!({
        "repo_ids": [uid(10), uid(11)],
        "run_ids": [uid(20)],
        "finding_ids": [uid(80)],
        "file_ids": [uid(70)],
        "include_history": true
    })
}

fn message_min() -> Value {
    json!({
        "message_id": uid(140),
        "role": "user",
        "text_redacted": "Synthetic question",
        "source_refs": [],
        "provider_id": null,
        "model_id": null,
        "approval_ids": [],
        "created_at": TS,
        "status": "complete",
        "tokens_in": null,
        "tokens_out": null
    })
}

fn message_max() -> Value {
    json!({
        "message_id": uid(141),
        "role": "assistant",
        "text_redacted": "Synthetic answer",
        "source_refs": [source_min(), source_max()],
        "provider_id": "openrouter",
        "model_id": "anthropic/fake-model",
        "approval_ids": [uid(40)],
        "created_at": TS_FRACTION,
        "status": "cancelled",
        "tokens_in": 120,
        "tokens_out": 18_446_744_073_709_551_615u64
    })
}

fn conversation_min() -> Value {
    json!({
        "conversation_id": uid(150),
        "title": "Synthetic chat",
        "context": context_min(),
        "created_at": TS,
        "updated_at": TS,
        "messages": []
    })
}

fn conversation_max() -> Value {
    json!({
        "conversation_id": uid(151),
        "title": "Another synthetic chat",
        "context": context_max(),
        "created_at": TS,
        "updated_at": TS_LATER,
        "messages": [message_min(), message_max()]
    })
}

fn conversation_file_min() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "conversations",
        "updated_at": TS,
        "conversations": []
    })
}

fn conversation_file_max() -> Value {
    json!({
        "schema_version": "1.4",
        "store_type": "conversations",
        "updated_at": TS_FRACTION,
        "conversations": [conversation_min(), conversation_max()]
    })
}

/// Wrong marker values every root must reject.
fn bad_markers(own: StoreKind) -> Vec<(&'static str, Value)> {
    let mut bad = vec![
        ("schema_version", json!("1.3")),
        ("schema_version", json!("1.40")),
        ("schema_version", json!(1.4)),
        ("schema_version", Value::Null),
        ("store_type", json!("")),
        ("store_type", json!(5)),
        ("store_type", Value::Null),
    ];
    for other in StoreKind::ALL {
        if other != own {
            bad.push(("store_type", json!(other.store_type())));
        }
    }
    bad
}

// --- per-def tests --------------------------------------------------------------------------------------

#[test]
fn source_ref() {
    check::<SourceRef>(&Def {
        name: "SourceRef",
        minimal: source_min(),
        maximal: source_max(),
        nullable: &[
            "repo_id",
            "commit_oid",
            "run_id",
            "file_path",
            "artifact_id",
            "document_index",
            "line_start",
            "line_end",
            "resource_identifier",
            "content_digest",
            "approval_id",
        ],
        bad: vec![
            ("kind", json!("bogus")),
            ("kind", json!("REPOSITORY_FILE")),
            ("kind", json!("")),
            ("kind", json!(1)),
            ("source_id", json!("not-a-uuid")),
            ("repo_id", json!("not-a-uuid")),
            ("commit_oid", json!("xyz")),
            ("run_id", json!(7)),
            ("file_path", json!("../escape.yaml")),
            ("file_path", json!("/abs.yaml")),
            ("artifact_id", json!("")),
            ("document_index", json!(-1)),
            ("document_index", json!("1")),
            ("line_start", json!(1.5)),
            ("resource_identifier", json!(5)),
            ("content_digest", json!("ABC")),
            ("retrieved_at", json!("yesterday")),
            ("retrieved_at", Value::Null),
            ("approval_id", json!(false)),
            ("redaction_version", json!("has space")),
            ("redaction_version", Value::Null),
            ("stale", json!("yes")),
        ],
    });
}

#[test]
fn engine() {
    check::<Engine>(&Def {
        name: "Engine",
        minimal: engine_min(),
        maximal: engine_max(),
        nullable: &["ruleset_version", "data_updated_at"],
        bad: vec![
            ("adapter_id", json!("bogus")),
            ("adapter_id", json!("YAMLLINT")),
            ("adapter_version", json!("")),
            ("tool_name", json!("Bad Name")),
            ("tool_name", json!("UPPER")),
            ("tool_version", Value::Null),
            ("tool_version", json!("a b")),
            ("ruleset_version", json!("a b")),
            ("data_updated_at", json!("yesterday")),
        ],
    });
}

#[test]
fn artifact() {
    check::<Artifact>(&Def {
        name: "Artifact",
        minimal: artifact_min(),
        maximal: artifact_max(),
        nullable: &["retained_path", "approval_id"],
        bad: vec![
            ("kind", json!("bogus")),
            ("kind", json!("Rendered")),
            ("kind", json!(1)),
            ("artifact_id", json!("not-a-uuid")),
            ("display_name", json!(5)),
            ("display_name", Value::Null),
            ("content_digest", json!("abc")),
            ("content_digest", Value::Null),
            ("retained_path", json!("/abs.txt")),
            ("retained_path", json!("../escape.txt")),
            ("source_refs", json!([{}])),
            ("approval_id", json!("x")),
            ("contains_only_redacted_retained_content", json!("true")),
        ],
    });
}

#[test]
fn check_record() {
    check::<Check>(&Def {
        name: "Check",
        minimal: check_min(),
        maximal: check_max(),
        nullable: &[
            "file_id",
            "framework",
            "reason_redacted",
            "started_at",
            "finished_at",
            "config_digest",
            "manifest_digest",
            "exit_code",
            "term_signal",
        ],
        bad: vec![
            ("category", json!("bogus")),
            ("outcome", json!("bogus")),
            ("outcome", json!("PASSED")),
            ("native_status", json!("bogus")),
            ("native_status", json!("timeout")),
            ("native_status", Value::Null),
            ("adapter_id", json!("bogus")),
            ("check_id", json!("x")),
            ("task_id", Value::Null),
            ("file_id", json!("x")),
            ("framework", json!("Bad Framework")),
            ("engine", json!({})),
            ("reason_redacted", json!(5)),
            ("started_at", json!("never")),
            ("finding_ids", json!(["x"])),
            ("source_refs", json!([5])),
            ("config_digest", json!("ABC")),
            ("exit_code", json!("1")),
            ("exit_code", json!(1.5)),
            ("term_signal", json!(-1)),
            ("approval_ids", json!(["x"])),
        ],
    });
}

#[test]
fn finding() {
    check::<Finding>(&Def {
        name: "Finding",
        minimal: finding_min(),
        maximal: finding_max(),
        nullable: &["suppression_id"],
        bad: vec![
            ("category", json!("bogus")),
            ("severity", json!("bogus")),
            ("severity", json!("Critical")),
            ("confidence", json!("bogus")),
            ("confidence", json!("HIGH")),
            ("confidence", Value::Null),
            ("review_status", json!("bogus")),
            ("review_status", json!("closed")),
            ("fingerprint", json!("abc")),
            ("commit_oid", Value::Null),
            ("commit_oid", json!("xyz")),
            ("finding_id", json!("x")),
            ("check_id", json!(1)),
            ("task_id", json!("")),
            ("repo_id", json!("x")),
            ("run_id", Value::Null),
            ("rule_id", json!(5)),
            ("title", Value::Null),
            ("description_redacted", json!(true)),
            ("evidence_redacted", json!(1.5)),
            ("remediation_redacted", json!([])),
            ("engine", json!(null)),
            ("sources", json!([{}])),
            ("suppression_id", json!("x")),
            ("observed_at", json!("never")),
        ],
    });
}

#[test]
fn coverage_summary() {
    check::<CoverageSummary>(&Def {
        name: "CoverageSummary",
        minimal: coverage_min(),
        maximal: coverage_max(),
        nullable: &[],
        bad: vec![
            ("coverage", json!("bogus")),
            ("coverage", json!("Complete")),
            ("assessment", json!("bogus")),
            ("assessment", json!("secure")),
            ("discovered_files", json!(-1)),
            ("selected_files", json!("3")),
            ("assessed_files", json!(1.5)),
            ("checks_passed", Value::Null),
            ("checks_cancelled", json!(-5)),
        ],
    });
}

#[test]
fn ai_enrichment() {
    check::<AIEnrichment>(&Def {
        name: "AIEnrichment",
        minimal: ai_min(),
        maximal: ai_max(),
        nullable: &[
            "provider_id",
            "requested_model",
            "actual_model",
            "actual_provider",
            "routing_policy",
            "generated_at",
            "reason_redacted",
        ],
        bad: vec![
            ("status", json!("bogus")),
            ("status", json!("Completed")),
            ("status", json!(1)),
            ("provider_id", json!("anthropic")),
            ("provider_id", json!("OpenRouter")),
            ("requested_model", json!("bad model")),
            ("actual_model", json!("")),
            ("actual_provider", json!("bad provider")),
            ("approval_ids", json!(["x"])),
            ("source_ids", json!(["x"])),
            ("routing_policy", json!({})),
            ("routing_policy", json!("deny")),
            ("redaction_version", json!("")),
            ("redaction_version", Value::Null),
            ("generated_at", json!("never")),
            ("narrative_redacted", Value::Null),
            ("narrative_redacted", json!(5)),
            ("reason_redacted", json!(5)),
        ],
    });
}

#[test]
fn report_file() {
    let mut bad = bad_markers(StoreKind::Report);
    bad.extend([
        ("updated_at", json!("never")),
        ("report_id", json!("x")),
        ("run_id", Value::Null),
        ("repo_id", json!(1)),
        ("repo_name", json!(5)),
        ("remote_identifier_redacted", Value::Null),
        ("selected_ref", json!("bad ref")),
        ("selected_ref", json!("a..b")),
        ("commit_oid", json!("xyz")),
        ("execution", json!("bogus")),
        ("execution", json!("Completed")),
        ("plan", json!({})),
        ("generated_at", json!("never")),
        ("coverage", json!({})),
        ("checks", json!([{}])),
        ("findings", json!([5])),
        ("artifacts", json!([null])),
        ("limitations", json!("text")),
        ("ai_enrichment", json!({})),
    ]);
    check::<ReportFile>(&Def {
        name: "ReportFile",
        minimal: report_min(),
        maximal: report_max(),
        nullable: &["commit_oid"],
        bad,
    });
}

#[test]
fn event() {
    check::<Event>(&Def {
        name: "Event",
        minimal: event_min(),
        maximal: event_max(),
        nullable: &[
            "run_id",
            "repo_id",
            "workflow_id",
            "task_id",
            "approval_id",
            "code",
        ],
        bad: vec![
            ("level", json!("bogus")),
            ("level", json!("INFO")),
            ("level", json!("warn")),
            ("level", json!(1)),
            ("event_type", json!("NotAnEvent")),
            ("event_type", json!("runstarted")),
            ("event_type", json!("")),
            ("event_type", Value::Null),
            ("event_id", json!("x")),
            ("at", json!("never")),
            ("run_id", json!("x")),
            ("repo_id", json!(5)),
            ("workflow_id", json!("")),
            ("task_id", json!(true)),
            ("approval_id", json!("x")),
            ("message_redacted", json!(5)),
            ("message_redacted", Value::Null),
            ("code", json!("Bad Code")),
            ("code", json!("UPPER")),
            ("code", json!(5)),
        ],
    });
}

#[test]
fn run_summary() {
    check::<RunSummary>(&Def {
        name: "RunSummary",
        minimal: run_min(),
        maximal: run_max(),
        nullable: &["started_at", "finished_at", "last_checkpoint_at"],
        bad: vec![
            ("trigger", json!("bogus")),
            ("trigger", json!("Manual")),
            ("trigger", json!("weekly")),
            ("execution", json!("bogus")),
            ("execution", json!("RUNNING")),
            ("run_id", json!("x")),
            ("created_at", json!("never")),
            ("started_at", json!("never")),
            ("finished_at", json!(5)),
            ("plan", json!({})),
            ("coverage", json!({})),
            ("report_ids", json!(["x"])),
            ("last_checkpoint_at", json!("never")),
        ],
    });
}

#[test]
fn report_manifest() {
    check::<ReportManifest>(&Def {
        name: "ReportManifest",
        minimal: manifest_min(),
        maximal: manifest_max(),
        nullable: &[
            "json_digest",
            "pdf_path",
            "pdf_digest",
            "pdf_error_redacted",
            "pdf_renderer_version",
        ],
        bad: vec![
            ("report_id", json!("x")),
            ("run_id", json!(1)),
            ("repo_id", Value::Null),
            ("json_path", json!("/abs.json")),
            ("json_path", json!("../escape.json")),
            ("md_path", json!("a\\b.md")),
            ("json_written", json!("true")),
            ("md_written", json!(1)),
            ("json_digest", json!("ABC")),
            ("generated_at", json!("never")),
            ("pdf_path", json!("/abs.pdf")),
            ("pdf_written", Value::Null),
            ("pdf_digest", json!("abc")),
            ("pdf_error_redacted", json!(5)),
            ("pdf_renderer_version", json!("a b")),
        ],
    });
}

#[test]
fn log_file() {
    let mut bad = bad_markers(StoreKind::Log);
    bad.extend([
        ("updated_at", json!("never")),
        ("runs", json!([{}])),
        ("events", json!([5])),
        ("reports", json!([null])),
        ("events", Value::Null),
    ]);
    check::<LogFile>(&Def {
        name: "LogFile",
        minimal: log_min(),
        maximal: log_max(),
        nullable: &[],
        bad,
    });
}

#[test]
fn suppression() {
    check::<Suppression>(&Def {
        name: "Suppression",
        minimal: suppression_min(),
        maximal: suppression_max(),
        nullable: &["expires_at"],
        bad: vec![
            ("suppression_id", json!("x")),
            ("repo_id", json!(1)),
            ("finding_fingerprint", json!("abc")),
            ("rule_id", json!(5)),
            ("reason", Value::Null),
            ("created_at", json!("never")),
            ("expires_at", json!("never")),
            ("actor", json!(true)),
            ("enabled", json!("yes")),
            ("enabled", Value::Null),
        ],
    });
}

#[test]
fn suppression_file() {
    let mut bad = bad_markers(StoreKind::Suppressions);
    bad.extend([
        ("updated_at", json!("never")),
        ("suppressions", json!([{}])),
        ("suppressions", Value::Null),
    ]);
    check::<SuppressionFile>(&Def {
        name: "SuppressionFile",
        minimal: suppression_file_min(),
        maximal: suppression_file_max(),
        nullable: &[],
        bad,
    });
}

#[test]
fn chunk() {
    check::<Chunk>(&Def {
        name: "Chunk",
        minimal: chunk_min(),
        maximal: chunk_max(),
        nullable: &["embedding_key"],
        bad: vec![
            ("chunk_id", json!("x")),
            ("source", json!({})),
            ("source", Value::Null),
            ("text_redacted", json!(5)),
            ("text_redacted", Value::Null),
            ("content_digest", json!("abc")),
            ("redaction_version", json!("has space")),
            ("embedding_key", json!("has space")),
            ("embedding_key", json!(5)),
            ("indexed_at", json!("never")),
        ],
    });
}

#[test]
fn rag_index_file() {
    let mut bad = bad_markers(StoreKind::RagIndex);
    bad.extend([
        ("updated_at", json!("never")),
        ("chunks", json!([{}])),
        ("chunks", Value::Null),
    ]);
    check::<RagIndexFile>(&Def {
        name: "RagIndexFile",
        minimal: rag_min(),
        maximal: rag_max(),
        nullable: &[],
        bad,
    });
}

#[test]
fn chat_context() {
    check::<ChatContext>(&Def {
        name: "ChatContext",
        minimal: context_min(),
        maximal: context_max(),
        nullable: &[],
        bad: vec![
            ("repo_ids", json!(["x"])),
            ("run_ids", json!([5])),
            ("finding_ids", json!([""])),
            ("file_ids", json!("x")),
            ("include_history", json!("true")),
            ("include_history", Value::Null),
        ],
    });
}

#[test]
fn message() {
    check::<Message>(&Def {
        name: "Message",
        minimal: message_min(),
        maximal: message_max(),
        nullable: &["provider_id", "model_id", "tokens_in", "tokens_out"],
        bad: vec![
            ("role", json!("bogus")),
            ("role", json!("System_Notice")),
            ("role", json!("system")),
            ("role", Value::Null),
            ("status", json!("bogus")),
            ("status", json!("Complete")),
            ("status", json!("incomplete")),
            ("message_id", json!("x")),
            ("text_redacted", json!(5)),
            ("source_refs", json!([{}])),
            ("provider_id", json!("bad provider")),
            ("model_id", json!("bad model")),
            ("approval_ids", json!(["x"])),
            ("created_at", json!("never")),
            ("tokens_in", json!(-1)),
            ("tokens_out", json!("5")),
        ],
    });
}

#[test]
fn conversation() {
    check::<Conversation>(&Def {
        name: "Conversation",
        minimal: conversation_min(),
        maximal: conversation_max(),
        nullable: &[],
        bad: vec![
            ("conversation_id", json!("x")),
            ("title", json!(5)),
            ("title", Value::Null),
            ("context", json!({})),
            ("created_at", json!("never")),
            ("updated_at", Value::Null),
            ("messages", json!([{}])),
            ("messages", json!([5])),
        ],
    });
}

#[test]
fn conversation_file() {
    let mut bad = bad_markers(StoreKind::Conversations);
    bad.extend([
        ("updated_at", json!("never")),
        ("conversations", json!([{}])),
        ("conversations", Value::Null),
    ]);
    check::<ConversationFile>(&Def {
        name: "ConversationFile",
        minimal: conversation_file_min(),
        maximal: conversation_file_max(),
        nullable: &[],
        bad,
    });
}

#[test]
fn twenty_defs_are_covered_and_exist_in_the_schema() {
    let names = [
        "SourceRef",
        "Engine",
        "Artifact",
        "Check",
        "Finding",
        "CoverageSummary",
        "AIEnrichment",
        "ReportFile",
        "Event",
        "RunSummary",
        "ReportManifest",
        "LogFile",
        "Suppression",
        "SuppressionFile",
        "Chunk",
        "RagIndexFile",
        "ChatContext",
        "Message",
        "Conversation",
        "ConversationFile",
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
    // The per-def checks are the same-named test functions above (`check` runs each against the schema).
}

// --- enums ----------------------------------------------------------------------------------------------

/// Every closed enum of this task: wire strings equal the schema enum, both directions, in order; an
/// unknown value is rejected.
fn closed_enum_matches_schema<T>(
    all: &[T],
    def: &str,
    prop: &str,
    wire: impl Fn(&T) -> &'static str,
) where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let expected = schema_enum(def, prop);
    assert_eq!(serialised_all(all), expected, "{def}.{prop} serialised");
    assert_eq!(
        all.iter().map(|v| wire(v).to_owned()).collect::<Vec<_>>(),
        expected,
        "{def}.{prop} as_str"
    );
    for (v, w) in all.iter().zip(&expected) {
        let parsed: T = serde_json::from_value(json!(w)).expect("schema value decodes");
        assert_eq!(&parsed, v, "{def}.{prop} {w}");
    }
    for bad in [
        json!("bogus"),
        json!(""),
        json!(expected[0].to_uppercase()),
        json!(format!(" {}", expected[0])),
        json!(1),
        Value::Null,
    ] {
        assert!(
            serde_json::from_value::<T>(bad.clone()).is_err(),
            "{def}.{prop} accepted {bad}"
        );
    }
}

#[test]
fn new_enums_reproduce_the_schema_value_sets() {
    closed_enum_matches_schema(&SourceKind::ALL, "SourceRef", "kind", |v| v.as_str());
    closed_enum_matches_schema(&ArtifactKind::ALL, "Artifact", "kind", |v| v.as_str());
    closed_enum_matches_schema(&NativeStatus::ALL, "Check", "native_status", |v| v.as_str());
    closed_enum_matches_schema(&Confidence::ALL, "Finding", "confidence", |v| v.as_str());
    closed_enum_matches_schema(&ReviewStatus::ALL, "Finding", "review_status", |v| {
        v.as_str()
    });
    closed_enum_matches_schema(&EnrichmentStatus::ALL, "AIEnrichment", "status", |v| {
        v.as_str()
    });
    closed_enum_matches_schema(&EventLevel::ALL, "Event", "level", |v| v.as_str());
    closed_enum_matches_schema(&RunTrigger::ALL, "RunSummary", "trigger", |v| v.as_str());
    closed_enum_matches_schema(&MessageRole::ALL, "Message", "role", |v| v.as_str());
    closed_enum_matches_schema(&MessageStatus::ALL, "Message", "status", |v| v.as_str());
}

#[test]
fn event_level_values_equal_the_schema_enum() {
    assert_eq!(
        serialised_all(&EventLevel::ALL),
        vec!["debug", "info", "warning", "error"]
    );
    assert_eq!(
        serialised_all(&EventLevel::ALL),
        schema_enum("Event", "level")
    );
    assert_eq!(
        serialised_all(&EventLevel::ALL)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        schema_enum("Event", "level")
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
}

fn run_state_wire(s: RunState) -> &'static str {
    match s {
        RunState::Queued => "queued",
        RunState::AwaitingApproval => "awaiting_approval",
        RunState::Running => "running",
        RunState::Cancelling => "cancelling",
        RunState::Completed => "completed",
        RunState::Partial => "partial",
        RunState::Failed => "failed",
        RunState::Cancelled => "cancelled",
        RunState::Interrupted => "interrupted",
    }
}

fn outcome_wire(s: CheckOutcome) -> &'static str {
    match s {
        CheckOutcome::Pending => "pending",
        CheckOutcome::Running => "running",
        CheckOutcome::Passed => "passed",
        CheckOutcome::Findings => "findings",
        CheckOutcome::Failed => "failed",
        CheckOutcome::Unsupported => "unsupported",
        CheckOutcome::Skipped => "skipped",
        CheckOutcome::Incomplete => "incomplete",
        CheckOutcome::Cancelled => "cancelled",
    }
}

#[test]
fn reused_domain_enums_have_identical_value_sets() {
    let schema = schema();
    let def_enum = |name: &str| -> Vec<String> {
        schema["$defs"][name]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} has no enum"))
            .iter()
            .map(|v| v.as_str().expect("string").to_owned())
            .collect()
    };

    // Execution = RunState: the match above is exhaustive, so a new variant fails to compile here.
    let states = [
        RunState::Queued,
        RunState::AwaitingApproval,
        RunState::Running,
        RunState::Cancelling,
        RunState::Completed,
        RunState::Partial,
        RunState::Failed,
        RunState::Cancelled,
        RunState::Interrupted,
    ];
    let wire: Vec<String> = states
        .iter()
        .map(|s| run_state_wire(*s).to_owned())
        .collect();
    assert_eq!(wire, def_enum("Execution"));
    assert_eq!(serialised_all(&states), wire);

    // Outcome = CheckOutcome.
    let outcomes = [
        CheckOutcome::Pending,
        CheckOutcome::Running,
        CheckOutcome::Passed,
        CheckOutcome::Findings,
        CheckOutcome::Failed,
        CheckOutcome::Unsupported,
        CheckOutcome::Skipped,
        CheckOutcome::Incomplete,
        CheckOutcome::Cancelled,
    ];
    let wire: Vec<String> = outcomes
        .iter()
        .map(|s| outcome_wire(*s).to_owned())
        .collect();
    assert_eq!(wire, def_enum("Outcome"));
    assert_eq!(serialised_all(&outcomes), wire);

    // Shared vocabulary enums.
    assert_eq!(serialised_all(&Category::ALL), def_enum("Category"));
    assert_eq!(serialised_all(&Severity::ALL), def_enum("Severity"));
    assert_eq!(serialised_all(&Assessment::ALL), def_enum("Assessment"));
    assert_eq!(serialised_all(&Coverage::ALL), def_enum("Coverage"));

    // Check.adapter_id / Engine.adapter_id are the domain closed set (Data §21).
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

/// The names returned by `AppEvent::event_type`, read from the source of `contract.rs` (the `=> "Name"` arms
/// of `fn event_type`).
fn app_event_names() -> Vec<String> {
    let start = CONTRACT_SRC
        .find("pub fn event_type(&self) -> &'static str")
        .expect("fn event_type present in contract.rs");
    let body = &CONTRACT_SRC[start..];
    let end = body.find("\n    }\n").expect("end of fn event_type");
    body[..end]
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("=> \"")?;
            let (name, _) = rest.split_once('"')?;
            Some(name.to_owned())
        })
        .collect()
}

/// The body of `fn event_type` of `contract.rs`.
fn event_type_body() -> &'static str {
    let start = CONTRACT_SRC
        .find("pub fn event_type(&self) -> &'static str")
        .expect("fn event_type present in contract.rs");
    let body = &CONTRACT_SRC[start..];
    let end = body.find("\n    }\n").expect("end of fn event_type");
    &body[..end]
}

/// The number of variants of `pub enum AppEvent` in `contract.rs`: lines at variant indentation (four spaces)
/// that start with an upper-case identifier, between the enum header and its closing brace.
fn app_event_variant_count() -> usize {
    let start = CONTRACT_SRC
        .find("pub enum AppEvent {\n")
        .expect("pub enum AppEvent present in contract.rs");
    let body = &CONTRACT_SRC[start..];
    let end = body.find("\n}\n").expect("end of enum AppEvent");
    body[..end]
        .lines()
        .skip(1)
        .filter(|line| {
            line.strip_prefix("    ")
                .and_then(|rest| rest.chars().next())
                .is_some_and(|c| c.is_ascii_uppercase())
        })
        .count()
}

#[test]
fn event_type_equals_the_app_event_names() {
    let from_contract = app_event_names();

    // Three independent counts must agree: the `=> "Name"` literals extracted from `fn event_type`, the
    // `Self::` arm patterns of that body (an arm whose result is not a plain literal, or an or-pattern,
    // makes these differ) and the variants of `pub enum AppEvent`. Fails if any arm is not a literal arm
    // or the extraction misses a variant.
    let arms = event_type_body().matches("Self::").count();
    let variants = app_event_variant_count();
    assert_eq!(
        from_contract.len(),
        arms,
        "literal arms vs `Self::` arm patterns of fn event_type"
    );
    assert_eq!(
        from_contract.len(),
        variants,
        "literal arms vs variants of enum AppEvent"
    );
    assert_eq!(
        from_contract.len(),
        EventType::ALL.len(),
        "AppEvent names vs EventType variants"
    );
    assert!(!from_contract.is_empty(), "extraction found nothing");

    let unique: BTreeSet<&String> = from_contract.iter().collect();
    assert_eq!(unique.len(), from_contract.len(), "duplicate names");

    let ours: Vec<String> = EventType::ALL
        .iter()
        .map(|t| t.as_str().to_owned())
        .collect();
    assert_eq!(ours.len(), EventType::ALL.len());
    let ours_set: BTreeSet<&String> = ours.iter().collect();
    assert_eq!(ours_set.len(), ours.len(), "duplicate EventType values");

    let missing: Vec<&&String> = unique.difference(&ours_set).collect();
    let extra: Vec<&&String> = ours_set.difference(&unique).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "EventType out of sync with AppEvent::event_type; missing from EventType: {missing:?}, not in AppEvent: {extra:?}"
    );

    // Serialised verbatim (PascalCase) and every name decodes back.
    assert_eq!(serialised_all(&EventType::ALL), ours);
    for name in &ours {
        let parsed: EventType = serde_json::from_value(json!(name)).expect("event type");
        assert_eq!(parsed.as_str(), name);
        assert_eq!(EventType::from_wire(name), Some(parsed));
    }
    // The contract's order is kept too.
    assert_eq!(ours, from_contract);
    // The schema leaves the property an open string; the typed layer closes it.
    let schema = schema();
    assert_eq!(
        schema["$defs"]["Event"]["properties"]["event_type"],
        json!({"type": "string"})
    );
}

#[test]
fn every_event_type_round_trips_inside_an_event() {
    for t in EventType::ALL {
        let inst = with(&event_min(), "event_type", json!(t.as_str()));
        let typed: Event = serde_json::from_value(inst.clone()).expect("event");
        assert_eq!(typed.event_type, t);
        assert_eq!(serde_json::to_value(&typed).expect("serialise"), inst);
    }
}

// --- markers and constants ------------------------------------------------------------------------------

fn root_markers<T>(min: &Value, own: StoreKind)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let typed: T = serde_json::from_value(min.clone()).expect("minimal root");
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back["schema_version"], json!("1.4"));
    assert_eq!(back["store_type"], json!(own.store_type()));
    for (key, bad) in bad_markers(own) {
        assert!(
            serde_json::from_value::<T>(with(min, key, bad.clone())).is_err(),
            "{own:?}: {key} accepted {bad}"
        );
    }
    assert!(serde_json::from_value::<T>(without(min, "schema_version")).is_err());
    assert!(serde_json::from_value::<T>(without(min, "store_type")).is_err());
}

#[test]
fn every_root_rejects_a_wrong_schema_version_and_store_type() {
    root_markers::<ReportFile>(&report_min(), StoreKind::Report);
    root_markers::<LogFile>(&log_min(), StoreKind::Log);
    root_markers::<SuppressionFile>(&suppression_file_min(), StoreKind::Suppressions);
    root_markers::<RagIndexFile>(&rag_min(), StoreKind::RagIndex);
    root_markers::<ConversationFile>(&conversation_file_min(), StoreKind::Conversations);
}

#[test]
fn suppressions_root_is_reserved_and_read_only() {
    assert!(!StoreKind::Suppressions.writable());
    // It still decodes and re-serialises like any other root.
    let typed: SuppressionFile = serde_json::from_value(suppression_file_max()).expect("decode");
    assert_eq!(typed.suppressions.len(), 2);
    assert_eq!(
        serde_json::to_value(&typed).expect("serialise"),
        suppression_file_max()
    );
}

#[test]
fn routing_policy_with_an_empty_provider_list_round_trips_inside_an_enrichment() {
    let inst = with(&ai_max(), "routing_policy", policy_min());
    let typed: AIEnrichment = serde_json::from_value(inst.clone()).expect("enrichment");
    assert_eq!(serde_json::to_value(&typed).expect("serialise"), inst);
}

#[test]
fn constant_booleans_of_the_routing_policy_reject_wrong_values() {
    // AIEnrichment.routing_policy carries `allow_fallbacks: const false` (ConstFalse) and `zdr: const true`
    // (ConstTrue); both go through the typed policy of the report.
    let wrong_zdr = [
        json!(false),
        json!("true"),
        json!(1),
        json!(1.0),
        Value::Null,
    ];
    let wrong_fallbacks = [
        json!(true),
        json!("false"),
        json!(0),
        json!(0.0),
        Value::Null,
    ];
    for inst in [ai_max(), report_max()["ai_enrichment"].clone()] {
        for bad in &wrong_zdr {
            let policy = with(&policy_max(), "zdr", bad.clone());
            let doc = with(&inst, "routing_policy", policy);
            assert!(
                serde_json::from_value::<AIEnrichment>(doc).is_err(),
                "zdr accepted {bad}"
            );
        }
        for bad in &wrong_fallbacks {
            let policy = with(&policy_max(), "allow_fallbacks", bad.clone());
            let doc = with(&inst, "routing_policy", policy);
            assert!(
                serde_json::from_value::<AIEnrichment>(doc).is_err(),
                "allow_fallbacks accepted {bad}"
            );
        }
        let policy = with(&policy_max(), "data_collection", json!("allow"));
        assert!(
            serde_json::from_value::<AIEnrichment>(with(&inst, "routing_policy", policy)).is_err()
        );
    }
    // The same through a whole report.
    for bad in &wrong_zdr {
        let policy = with(&policy_max(), "zdr", bad.clone());
        let ai = with(&ai_max(), "routing_policy", policy);
        assert!(
            serde_json::from_value::<ReportFile>(with(&report_max(), "ai_enrichment", ai)).is_err()
        );
    }
    for bad in &wrong_fallbacks {
        let policy = with(&policy_max(), "allow_fallbacks", bad.clone());
        let ai = with(&ai_max(), "routing_policy", policy);
        assert!(
            serde_json::from_value::<ReportFile>(with(&report_max(), "ai_enrichment", ai)).is_err()
        );
    }
}

// --- FR-31 ----------------------------------------------------------------------------------------------

const HOSTILE: &str = "a\u{1b}b\u{202e}c";
const HOSTILE_ESCAPED: &str = "a<U+001B>b<U+202E>c";

/// Sets each of `keys` of `inst` to hostile text and checks that it is re-escaped on read and written back
/// escaped, with no raw control or bidi character left in the serialised JSON text.
fn reescapes<T>(label: &str, inst: &Value, keys: &[&str])
where
    T: Serialize + DeserializeOwned,
{
    for key in keys {
        let planted = with(inst, key, json!(HOSTILE));
        let typed: T = serde_json::from_value(planted)
            .unwrap_or_else(|e| panic!("{label}.{key} hostile text rejected: {e}"));
        let back = serde_json::to_value(&typed).expect("serialise");
        assert_eq!(back[*key], json!(HOSTILE_ESCAPED), "{label}.{key}");
        let text = serde_json::to_string(&typed).expect("serialise");
        assert!(!text.contains('\u{1b}'), "{label}.{key}: raw ESC in {text}");
        assert!(
            !text.contains('\u{202e}'),
            "{label}.{key}: raw bidi override in {text}"
        );
        assert!(text.contains(HOSTILE_ESCAPED), "{label}.{key}");
        // Reading the escaped form back again changes nothing (idempotent).
        let again: Value =
            serde_json::to_value(serde_json::from_value::<T>(back.clone()).expect("decode again"))
                .expect("serialise");
        assert_eq!(again, back, "{label}.{key} idempotent");
    }
}

#[test]
fn redacted_text_is_re_escaped_on_read_in_every_r_bearing_def() {
    reescapes::<SourceRef>("SourceRef", &source_max(), &["resource_identifier"]);
    reescapes::<Artifact>("Artifact", &artifact_max(), &["display_name"]);
    reescapes::<Check>("Check", &check_max(), &["reason_redacted"]);
    reescapes::<Finding>(
        "Finding",
        &finding_max(),
        &[
            "rule_id",
            "title",
            "description_redacted",
            "evidence_redacted",
            "remediation_redacted",
        ],
    );
    reescapes::<AIEnrichment>(
        "AIEnrichment",
        &ai_max(),
        &["narrative_redacted", "reason_redacted"],
    );
    reescapes::<ReportFile>(
        "ReportFile",
        &report_max(),
        &["repo_name", "remote_identifier_redacted"],
    );
    reescapes::<Event>("Event", &event_max(), &["message_redacted"]);
    reescapes::<ReportManifest>("ReportManifest", &manifest_max(), &["pdf_error_redacted"]);
    reescapes::<Suppression>(
        "Suppression",
        &suppression_max(),
        &["rule_id", "reason", "actor"],
    );
    reescapes::<Chunk>("Chunk", &chunk_max(), &["text_redacted"]);
    reescapes::<Message>("Message", &message_max(), &["text_redacted"]);
    reescapes::<Conversation>("Conversation", &conversation_max(), &["title"]);
}

#[test]
fn redacted_text_is_re_escaped_when_nested_in_a_root() {
    // Through the roots: a hostile value deep in the tree is escaped on read.
    let mut report = report_max();
    report["findings"][1]["title"] = json!(HOSTILE);
    report["checks"][1]["reason_redacted"] = json!(HOSTILE);
    report["ai_enrichment"]["narrative_redacted"] = json!(HOSTILE);
    let typed: ReportFile = serde_json::from_value(report).expect("report");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{202e}'),
        "{text}"
    );
    assert_eq!(text.matches(HOSTILE_ESCAPED).count(), 3);

    let mut log = log_max();
    log["events"][1]["message_redacted"] = json!(HOSTILE);
    log["reports"][1]["pdf_error_redacted"] = json!(HOSTILE);
    let typed: LogFile = serde_json::from_value(log).expect("log");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{202e}'),
        "{text}"
    );
    assert_eq!(text.matches(HOSTILE_ESCAPED).count(), 2);

    let mut convo = conversation_file_max();
    convo["conversations"][1]["messages"][1]["text_redacted"] = json!(HOSTILE);
    let typed: ConversationFile = serde_json::from_value(convo).expect("conversations");
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{202e}'),
        "{text}"
    );
    assert_eq!(text.matches(HOSTILE_ESCAPED).count(), 1);
}

#[test]
fn sanitised_limitations_are_escaped_on_read() {
    let doc = with(&report_min(), "limitations", json!([HOSTILE]));
    let typed: ReportFile = serde_json::from_value(doc).expect("report");
    let back = serde_json::to_value(&typed).expect("serialise");
    assert_eq!(back["limitations"], json!([HOSTILE_ESCAPED]));
}

/// Parses `pub struct` blocks of a source file and returns `(struct, field, preceding serde attribute)` for
/// every field whose type mentions `RedactedText`.
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
        include_str!("../src/store/evidence.rs"),
        include_str!("../src/store/report.rs"),
        include_str!("../src/store/log.rs"),
        include_str!("../src/store/suppression.rs"),
        include_str!("../src/store/rag.rs"),
        include_str!("../src/store/conversation.rs"),
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
    // Appendix A class R properties of the twenty defs of this task (20 of the 27 R properties).
    let expected: BTreeSet<String> = [
        "SourceRef.resource_identifier",
        "Check.reason_redacted",
        "Finding.rule_id",
        "Finding.title",
        "Finding.description_redacted",
        "Finding.evidence_redacted",
        "Finding.remediation_redacted",
        "Artifact.display_name",
        "Event.message_redacted",
        "ReportManifest.pdf_error_redacted",
        "Suppression.rule_id",
        "Suppression.reason",
        "Suppression.actor",
        "Chunk.text_redacted",
        "Message.text_redacted",
        "Conversation.title",
        "ReportFile.repo_name",
        "ReportFile.remote_identifier_redacted",
        "AIEnrichment.narrative_redacted",
        "AIEnrichment.reason_redacted",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn decode_errors_do_not_echo_planted_text() {
    const MARKER: &str = "PLANTEDMARKER";
    const NUMBER: &str = "987654321";
    let cases: Vec<(&str, String)> = vec![
        // New closed enums.
        (
            "Event.level",
            decode_err::<Event>(with(&event_min(), "level", json!(MARKER))),
        ),
        (
            "Event.event_type",
            decode_err::<Event>(with(&event_min(), "event_type", json!(MARKER))),
        ),
        (
            "SourceRef.kind",
            decode_err::<SourceRef>(with(&source_min(), "kind", json!(MARKER))),
        ),
        (
            "Artifact.kind",
            decode_err::<Artifact>(with(&artifact_min(), "kind", json!(MARKER))),
        ),
        (
            "Check.native_status",
            decode_err::<Check>(with(&check_min(), "native_status", json!(MARKER))),
        ),
        (
            "Finding.confidence",
            decode_err::<Finding>(with(&finding_min(), "confidence", json!(MARKER))),
        ),
        (
            "Finding.review_status",
            decode_err::<Finding>(with(&finding_min(), "review_status", json!(MARKER))),
        ),
        (
            "AIEnrichment.status",
            decode_err::<AIEnrichment>(with(&ai_min(), "status", json!(MARKER))),
        ),
        (
            "RunSummary.trigger",
            decode_err::<RunSummary>(with(&run_min(), "trigger", json!(MARKER))),
        ),
        (
            "Message.role",
            decode_err::<Message>(with(&message_min(), "role", json!(MARKER))),
        ),
        (
            "Message.status",
            decode_err::<Message>(with(&message_min(), "status", json!(MARKER))),
        ),
        // Wrong-typed values in enum fields.
        (
            "Event.level number",
            decode_err::<Event>(with(&event_min(), "level", json!(987_654_321u64))),
        ),
        // Validated newtypes.
        (
            "SourceRef.file_path",
            decode_err::<SourceRef>(with(
                &source_min(),
                "file_path",
                json!(format!("../{MARKER}")),
            )),
        ),
        (
            "SourceRef.redaction_version",
            decode_err::<SourceRef>(with(
                &source_min(),
                "redaction_version",
                json!(format!("bad {MARKER}")),
            )),
        ),
        (
            "SourceRef.commit_oid",
            decode_err::<SourceRef>(with(&source_min(), "commit_oid", json!(MARKER))),
        ),
        (
            "Artifact.retained_path",
            decode_err::<Artifact>(with(
                &artifact_min(),
                "retained_path",
                json!(format!("/{MARKER}")),
            )),
        ),
        (
            "Event.code",
            decode_err::<Event>(with(&event_min(), "code", json!(format!("Bad {MARKER}")))),
        ),
        (
            "ReportManifest.json_path",
            decode_err::<ReportManifest>(with(
                &manifest_min(),
                "json_path",
                json!(format!("../{MARKER}")),
            )),
        ),
        (
            "Engine.tool_name",
            decode_err::<Engine>(with(&engine_min(), "tool_name", json!(MARKER))),
        ),
        (
            "Chunk.embedding_key",
            decode_err::<Chunk>(with(
                &chunk_min(),
                "embedding_key",
                json!(format!("bad {MARKER}")),
            )),
        ),
        (
            "Message.model_id",
            decode_err::<Message>(with(
                &message_min(),
                "model_id",
                json!(format!("bad {MARKER}")),
            )),
        ),
        (
            "ReportFile.selected_ref",
            decode_err::<ReportFile>(with(
                &report_min(),
                "selected_ref",
                json!(format!("bad {MARKER}")),
            )),
        ),
        // Identifiers and timestamps.
        (
            "Event.event_id",
            decode_err::<Event>(with(&event_min(), "event_id", json!(MARKER))),
        ),
        (
            "Event.at",
            decode_err::<Event>(with(&event_min(), "at", json!(MARKER))),
        ),
        // Root markers.
        (
            "LogFile.schema_version",
            decode_err::<LogFile>(with(&log_min(), "schema_version", json!(MARKER))),
        ),
        (
            "LogFile.store_type",
            decode_err::<LogFile>(with(&log_min(), "store_type", json!(MARKER))),
        ),
        (
            "LogFile.store_type number",
            decode_err::<LogFile>(with(&log_min(), "store_type", json!(987_654_321u64))),
        ),
        // Constants.
        (
            "ProviderPolicy.zdr",
            decode_err::<AIEnrichment>(with(
                &ai_max(),
                "routing_policy",
                with(&policy_max(), "zdr", json!(MARKER)),
            )),
        ),
        (
            "ProviderPolicy.allow_fallbacks",
            decode_err::<AIEnrichment>(with(
                &ai_max(),
                "routing_policy",
                with(&policy_max(), "allow_fallbacks", json!(MARKER)),
            )),
        ),
        (
            "AIEnrichment.provider_id",
            decode_err::<AIEnrichment>(with(&ai_max(), "provider_id", json!(MARKER))),
        ),
        // Wrong-typed text values in R fields.
        (
            "Finding.title number",
            decode_err::<Finding>(with(&finding_min(), "title", json!(987_654_321u64))),
        ),
        (
            "Event.message_redacted bool",
            decode_err::<Event>(with(&event_min(), "message_redacted", json!(true))),
        ),
        (
            "Check.reason_redacted number",
            decode_err::<Check>(with(&check_min(), "reason_redacted", json!(987_654_321u64))),
        ),
        (
            "Suppression.actor float",
            decode_err::<Suppression>(with(&suppression_min(), "actor", json!(987_654.321f64))),
        ),
    ];
    for (label, err) in &cases {
        assert!(!err.contains(MARKER), "{label} echoes the marker: {err}");
        assert!(!err.contains(NUMBER), "{label} echoes the number: {err}");
        assert!(
            !err.contains("987654.321"),
            "{label} echoes the float: {err}"
        );
    }
}

#[test]
fn decode_errors_from_text_do_not_echo_planted_text() {
    // The same through JSON text, where serde_json formats the error itself.
    const MARKER: &str = "PLANTEDMARKER";
    for (label, doc) in [
        ("level", with(&event_min(), "level", json!(MARKER))),
        (
            "event_type",
            with(&event_min(), "event_type", json!(MARKER)),
        ),
        (
            "code",
            with(&event_min(), "code", json!(format!("Bad {MARKER}"))),
        ),
    ] {
        let text = serde_json::to_string(&doc).expect("serialise");
        let err = serde_json::from_str::<Event>(&text)
            .expect_err("rejected")
            .to_string();
        assert!(!err.contains(MARKER), "{label}: {err}");
    }
}

// --- misc -----------------------------------------------------------------------------------------------

/// Data Schema §10, copied verbatim.
const LOG_EXAMPLE: &str = r#"{
  "schema_version": "1.4",
  "store_type": "log",
  "updated_at": "2026-10-16T05:25:00Z",
  "runs": [],
  "events": [],
  "reports": []
}"#;

#[test]
fn data_schema_section_ten_initial_log_decodes_and_is_byte_stable() {
    let typed: LogFile = serde_json::from_str(LOG_EXAMPLE).expect("section 10 example");
    assert!(typed.runs.is_empty() && typed.events.is_empty() && typed.reports.is_empty());

    // One canonicalisation, then the bytes no longer change.
    let canonical = serde_json::to_vec_pretty(&typed).expect("serialise");
    let again: LogFile = serde_json::from_slice(&canonical).expect("canonical form decodes");
    assert_eq!(again, typed);
    let second = serde_json::to_vec_pretty(&again).expect("serialise");
    assert_eq!(second, canonical, "re-serialisation is not byte-stable");

    // The compact form is stable as well, and the example's value is preserved.
    let compact = serde_json::to_vec(&typed).expect("serialise");
    let compact_again =
        serde_json::to_vec(&serde_json::from_slice::<LogFile>(&compact).expect("decode"))
            .expect("serialise");
    assert_eq!(compact, compact_again);
    let original: Value = serde_json::from_str(LOG_EXAMPLE).expect("json");
    assert_eq!(serde_json::to_value(&typed).expect("serialise"), original);
    assert_eq!(original, log_min_at(TS));

    // The example's own key order is the schema order of `LogFile`, so the typed value reproduces the
    // example text exactly (the pretty form has no trailing newline). Fails if a `LogFile` field moves or
    // the example's key order or layout differs from the serialisation.
    let example_order: Vec<String> = members(LOG_EXAMPLE, 0)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(example_order, schema_property_order("LogFile"));
    let pretty = serde_json::to_string_pretty(&typed).expect("serialise");
    assert_eq!(pretty, LOG_EXAMPLE.trim_end_matches('\n'));
}

#[test]
fn the_key_order_scanner_reads_text_order_and_skips_nested_values() {
    let text =
        r#" {"b" : {"z": 1, "a": "}{,\"q\":"}, "a": [1, {"k": 2}, "]"], "c": null,"d":true }"#;
    let at = text.find('{').expect("object");
    let top: Vec<String> = members(text, at).into_iter().map(|(k, _)| k).collect();
    assert_eq!(top, ["b", "a", "c", "d"]);
    let nested = child(text, at, "b");
    let inner: Vec<String> = members(text, nested).into_iter().map(|(k, _)| k).collect();
    assert_eq!(inner, ["z", "a"]);
    // A text-order result that is not sorted: this is what a `Value`/`BTreeMap` could not report.
    assert_ne!(top, {
        let mut sorted = top.clone();
        sorted.sort();
        sorted
    });
    assert!(members("{}", 0).is_empty());
}

fn log_min_at(updated_at: &str) -> Value {
    with(&log_min(), "updated_at", json!(updated_at))
}

#[test]
fn synthetic_roots_round_trip_and_are_byte_stable() {
    fn stable<T: Serialize + DeserializeOwned + PartialEq + Debug>(label: &str, inst: &Value) {
        let typed: T =
            serde_json::from_value(inst.clone()).unwrap_or_else(|e| panic!("{label}: {e}"));
        let first = serde_json::to_vec_pretty(&typed).expect("serialise");
        let second = serde_json::to_vec_pretty(
            &serde_json::from_slice::<T>(&first).unwrap_or_else(|e| panic!("{label}: {e}")),
        )
        .expect("serialise");
        assert_eq!(first, second, "{label}");
        assert_eq!(
            &serde_json::to_value(&typed).expect("serialise"),
            inst,
            "{label}"
        );
    }
    stable::<ReportFile>("report min", &report_min());
    stable::<ReportFile>("report max", &report_max());
    stable::<LogFile>("log min", &log_min());
    stable::<LogFile>("log max", &log_max());
    stable::<SuppressionFile>("suppressions min", &suppression_file_min());
    stable::<SuppressionFile>("suppressions max", &suppression_file_max());
    stable::<RagIndexFile>("rag min", &rag_min());
    stable::<RagIndexFile>("rag max", &rag_max());
    stable::<ConversationFile>("conversations min", &conversation_file_min());
    stable::<ConversationFile>("conversations max", &conversation_file_max());
}

#[test]
fn log_maximal_contains_runs_events_and_manifests() {
    let typed: LogFile = serde_json::from_value(log_max()).expect("log");
    assert_eq!(typed.runs.len(), 2);
    assert_eq!(typed.events.len(), 2);
    assert_eq!(typed.reports.len(), 2);
    assert_eq!(typed.runs[1].trigger, RunTrigger::Daily);
    assert_eq!(typed.runs[1].execution, RunState::Completed);
    assert_eq!(typed.events[1].event_type, EventType::StorageFailure);
    assert_eq!(typed.events[1].level, EventLevel::Error);
    assert_eq!(
        typed.reports[1].pdf_path.as_ref().map(|p| p.as_str()),
        Some("backup/report-fake-2.pdf")
    );
}

#[test]
fn nullable_keys_are_serialised_as_null_after_a_decode_encode_cycle() {
    // Required-but-nullable keys are never skipped (request §5.1).
    let typed: Event = serde_json::from_value(event_min()).expect("event");
    let text = serde_json::to_string(&typed).expect("serialise");
    for key in [
        "run_id",
        "repo_id",
        "workflow_id",
        "task_id",
        "approval_id",
        "code",
    ] {
        assert!(text.contains(&format!("\"{key}\":null")), "{key}: {text}");
    }
    let typed: AIEnrichment = serde_json::from_value(ai_min()).expect("ai");
    let text = serde_json::to_string(&typed).expect("serialise");
    for key in [
        "provider_id",
        "routing_policy",
        "generated_at",
        "reason_redacted",
    ] {
        assert!(text.contains(&format!("\"{key}\":null")), "{key}: {text}");
    }
}

#[test]
fn absent_required_nullable_key_follows_the_documented_serde_behaviour() {
    // Pins current behaviour (as P02-D02-6 for the plan records). Serde treats a missing plain `Option`
    // field as `None`; the schema's `required` list is the guard (T08). A nullable *class R* field uses
    // `deserialize_with`, for which serde has no implicit default, so a missing key is an error there.
    // Either way the typed layer always writes the key back.
    let typed: SourceRef =
        serde_json::from_value(without(&source_min(), "repo_id")).expect("decodes without repo_id");
    assert_eq!(typed.repo_id, None);
    let text = serde_json::to_string(&typed).expect("serialise");
    assert!(text.contains("\"repo_id\":null"), "{text}");
    assert_eq!(
        serde_json::to_value(&typed).expect("serialise"),
        source_min()
    );

    assert!(
        serde_json::from_value::<SourceRef>(without(&source_min(), "resource_identifier")).is_err()
    );
}

#[test]
fn integer_bounds_follow_the_schema_minimums_that_the_typed_layer_does_not_enforce() {
    // Pins current behaviour: schema `minimum: 1` on `document_index`, `line_start`, `line_end` and
    // `term_signal` is not enforced by the types (T08 does), while a negative number is rejected by the
    // unsigned field type. Reported as an unenforced constraint.
    let src = with(&source_min(), "document_index", json!(0));
    let typed: SourceRef = serde_json::from_value(src).expect("0 decodes at the typed layer");
    assert_eq!(typed.document_index, Some(0));
    let chk = with(&check_min(), "term_signal", json!(0));
    let typed: Check = serde_json::from_value(chk).expect("0 decodes at the typed layer");
    assert_eq!(typed.term_signal, Some(0));
    let schema = schema();
    assert_eq!(
        schema["$defs"]["SourceRef"]["properties"]["document_index"]["anyOf"][0]["minimum"],
        json!(1)
    );
    assert_eq!(
        schema["$defs"]["Check"]["properties"]["term_signal"]["anyOf"][0]["minimum"],
        json!(1)
    );
}

#[test]
fn reused_and_new_types_have_the_documented_rust_shapes() {
    // Compile-time pins of the field types mapped in the request (D03 newtype rules).
    let event: Event = serde_json::from_value(event_max()).expect("event");
    let _: &domain::ids::EventId = &event.event_id;
    let _: &domain::time::UtcTimestamp = &event.at;
    let _: &EventLevel = &event.level;
    let _: &EventType = &event.event_type;
    let _: &Option<domain::store::values::EventCode> = &event.code;
    let _: &domain::text::RedactedText = &event.message_redacted;

    let report: ReportFile = serde_json::from_value(report_max()).expect("report");
    let _: &domain::store::ReportStoreType = &report.store_type;
    let _: &RunState = &report.execution;
    let _: &CheckOutcome = &report.checks[0].outcome;
    let _: &Option<domain::store::provider::ProviderPolicy> = &report.ai_enrichment.routing_policy;
    let _: &Option<domain::store::provider::OpenRouterId> = &report.ai_enrichment.provider_id;
    let _: &Vec<domain::text::SanitisedText> = &report.limitations;
}

//! Tests of the semantic rules (`persistence::semantic`, request §7, T09). All data is synthetic.
//!
//! Documents are built as JSON, edited per case, and decoded through the `domain` store types, so a case can
//! only exercise values the typed layer accepts. Each negative case states the exact set of
//! `(rule, pointer)` pairs `check` must report: a rule that stops detecting its violation, flags a different
//! pointer or flags an extra place makes the case fail. Each positive case must produce no problem at all.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeSet;

use domain::store::approval::ApprovalFile;
use domain::store::config::ConfigFile;
use domain::store::conversation::ConversationFile;
use domain::store::log::LogFile;
use domain::store::rag::RagIndexFile;
use domain::store::report::ReportFile;
use domain::store::repository::RepositoryFile;
use domain::store::suppression::SuppressionFile;
use domain::store::tools::ToolManifestFile;
use domain::store::workflow::WorkflowFile;
use domain::store::{AnyStore, StoreKind};
use persistence::semantic::{RuleStatus, check, rule_table};
use persistence::validate::{Layer, ProblemCode, Problems};
use serde_json::{Value, json};

const T0: &str = "2026-10-16T05:25:00Z";
const T1: &str = "2026-10-16T05:26:00Z";
const T2: &str = "2026-10-16T05:27:00Z";
const T3: &str = "2026-10-16T05:30:00Z";
const EARLY: &str = "2026-10-16T05:00:00Z";
const LATE: &str = "2026-10-16T05:50:00Z";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const OID: &str = "0123456789abcdef0123456789abcdef01234567";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// SHA-256 of the empty byte string.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// SHA-256 of `abc` (FIPS 180-4 test vector).
const SHA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
/// SHA-256 of the escaped text `a<U+001B>b` (the form persisted for an embedded ESC).
const SHA_ESCAPED: &str = "83d01bd21ff52c970ae355d02baf2cb8712ebdea9409c7ae49b0589a6c2e5950";
/// SHA-256 of the raw text `a`, ESC, `b` (never persisted: sanitising escapes the ESC first).
const SHA_RAW_ESC: &str = "1b036bb7c56caec6e11a3891cea30f910b89b34d6d27be39202efd3b38a29bad";
const MARKER: &str = "PLANTEDMARKERq7Zx";

fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

/// `base` with the members of `over` set (objects only).
fn with(mut base: Value, over: Value) -> Value {
    if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

fn header(store_type: &str) -> Value {
    json!({"schema_version": "1.4", "store_type": store_type, "updated_at": T0})
}

// --- building blocks ------------------------------------------------------------------------------------

fn src(id: u32, kind: &str, over: Value) -> Value {
    with(
        json!({
            "source_id": uid(id), "kind": kind, "repo_id": null, "commit_oid": null, "run_id": null,
            "file_path": null, "artifact_id": null, "document_index": null, "line_start": null,
            "line_end": null, "resource_identifier": null, "content_digest": null, "retrieved_at": T1,
            "approval_id": null, "redaction_version": "redact-1", "stale": false
        }),
        over,
    )
}

/// A repository-file source of the sample repository, cited by a check, a finding and an artifact.
fn src1() -> Value {
    src(
        80,
        "repository_file",
        json!({"repo_id": uid(10), "commit_oid": OID, "run_id": uid(20), "file_path": "deploy/app.yaml",
               "document_index": 1, "line_start": 3, "line_end": 4, "content_digest": DIGEST}),
    )
}

/// A rendered-artifact source that points at artifact 90 of the sample report.
fn src2() -> Value {
    src(
        81,
        "rendered_artifact",
        json!({"repo_id": uid(10), "run_id": uid(20), "artifact_id": uid(90)}),
    )
}

fn engine(adapter: &str) -> Value {
    json!({"adapter_id": adapter, "adapter_version": "1.0", "tool_name": adapter,
           "tool_version": "1.35.1", "ruleset_version": null, "data_updated_at": null})
}

fn check_json(n: u32, adapter: &str, over: Value) -> Value {
    with(
        json!({
            "check_id": uid(n), "task_id": uid(n + 10), "file_id": null, "category": "syntax",
            "framework": null, "engine": engine(adapter), "outcome": "passed", "reason_redacted": null,
            "started_at": T1, "finished_at": T2, "finding_ids": [], "source_refs": [],
            "adapter_id": adapter, "config_digest": null, "manifest_digest": DIGEST, "exit_code": 0,
            "term_signal": null, "native_status": "exited", "approval_ids": []
        }),
        over,
    )
}

fn finding_json() -> Value {
    json!({
        "finding_id": uid(70), "fingerprint": DIGEST, "check_id": uid(50), "task_id": uid(60),
        "repo_id": uid(10), "run_id": uid(20), "commit_oid": OID, "category": "syntax",
        "severity": "low", "confidence": "high", "rule_id": "rule-1", "title": "Synthetic finding",
        "description_redacted": "A synthetic description.", "evidence_redacted": "key: value",
        "remediation_redacted": "Fix the synthetic value.", "engine": engine("yamllint"),
        "sources": [src1()], "review_status": "new", "suppression_id": null, "observed_at": T1
    })
}

fn file_json(n: u32, path: &str, state: &str, digest: Value) -> Value {
    json!({"file_id": uid(n), "path": path, "byte_size": 120, "content_digest": digest,
           "commit_oid": OID, "detected_framework": null, "discovery_state": state, "reason": null})
}

fn target_json(repo: u32, selector: Value) -> Value {
    json!({"repo_id": uid(repo), "ref_kind": "branch", "ref_value": "main", "selector": selector})
}

fn all_yaml() -> Value {
    json!({"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []})
}

fn explicit(paths: &[&str]) -> Value {
    json!({"mode": "explicit", "paths": paths, "include_patterns": [], "exclude_patterns": []})
}

fn patterns(include: &[&str]) -> Value {
    json!({"mode": "patterns", "paths": [], "include_patterns": include, "exclude_patterns": []})
}

fn renderer(params: Value) -> Value {
    json!({"framework": "helm", "renderer_id": "helm-template", "input_paths": ["chart/values.yaml"],
           "parameters_redacted": params})
}

fn scan_settings(renderers: Value) -> Value {
    json!({
        "categories": ["syntax", "schema", "secrets"], "secret_scope": "checkout",
        "template_mode": "both", "schema_mode": "both", "schema_bindings": [], "renderers": renderers,
        "allow_reference_context": true, "adapter_ids": []
    })
}

fn policy(only: &[&str]) -> Value {
    json!({"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": only})
}

fn plan_json() -> Value {
    json!({
        "workflow_id": uid(40), "workflow_revision": 1,
        "targets": [target_json(10, all_yaml())],
        "scan_settings": scan_settings(json!([])),
        "approval_ids": [], "created_at": T0, "plan_digest": DIGEST,
        "target_snapshots": [{
            "repo_id": uid(10), "selected_ref": "main", "commit_oid": OID, "inventory_revision": 1,
            "selected_files": [file_json(30, "deploy/app.yaml", "eligible", json!(DIGEST))],
            "missing_paths": [], "snapshot_at": T0
        }],
        "engine_snapshots": [{
            "adapter_id": "yamllint", "adapter_version": "1.0", "tool_name": "yamllint",
            "tool_version": "1.35.1", "ruleset_version": null, "config_digest": null,
            "manifest_digest": DIGEST, "availability": "ready", "data_updated_at": null
        }]
    })
}

fn coverage_json() -> Value {
    json!({
        "discovered_files": 2, "selected_files": 1, "assessed_files": 1, "excluded_files": 0,
        "missing_files": 0, "checks_passed": 1, "checks_with_findings": 1, "checks_failed": 0,
        "checks_unsupported": 0, "checks_skipped": 0, "checks_incomplete": 1, "checks_cancelled": 0,
        "coverage": "partial", "assessment": "findings"
    })
}

fn zero_coverage() -> Value {
    json!({
        "discovered_files": 0, "selected_files": 0, "assessed_files": 0, "excluded_files": 0,
        "missing_files": 0, "checks_passed": 0, "checks_with_findings": 0, "checks_failed": 0,
        "checks_unsupported": 0, "checks_skipped": 0, "checks_incomplete": 0, "checks_cancelled": 0,
        "coverage": "none", "assessment": "unknown"
    })
}

// --- one rich, valid document per root kind -------------------------------------------------------------

fn config_doc() -> Value {
    with(
        header("config"),
        json!({"config": {
            "app_data_root": "/home/user/.local/share/pwt-example", "backup_directory": "backup",
            "active_theme": "default", "max_active_repositories": 8, "max_scanner_processes": 4,
            "history_retention_days": 90, "chat_persistence_enabled": false,
            "provider": {
                "provider_id": "openrouter", "kind": "api", "endpoint": "https://openrouter.ai/api/v1",
                "model_id": "example/model-1", "credential_mode": "session_only",
                "credential_reference": null, "routing_policy": policy(&["example-provider"])
            },
            "telemetry_enabled": false, "redaction_version": "redact-1"
        }}),
    )
}

fn repo_json(n: u32, remote: &str, canonical: &str, files: Value) -> Value {
    json!({
        "repo_id": uid(n), "display_name": "example-service", "slug": "example-service",
        "remote_url": remote, "canonical_remote": canonical, "local_path": format!("checkouts/{}", uid(n)),
        "default_ref_kind": "branch", "default_ref_value": "main", "registered_at": T0,
        "last_update_at": T1, "last_commit_oid": OID, "checkout_state": "ready",
        "last_error_redacted": null, "inventory_revision": 1, "inventory_at": T1, "files": files
    })
}

fn repositories_doc() -> Value {
    with(
        header("repositories"),
        json!({"repositories": [repo_json(
            10,
            "https://github.com/example/example-service.git",
            "github.com/example/example-service",
            json!([
                file_json(30, "deploy/app.yaml", "eligible", json!(DIGEST)),
                file_json(31, "deploy/broken.yaml", "unreadable", Value::Null)
            ]),
        )]}),
    )
}

fn workflow_json(
    n: u32,
    enabled: bool,
    targets: Value,
    schedule: Value,
    renderers: Value,
) -> Value {
    json!({
        "workflow_id": uid(n), "revision": 1, "name": "Review", "description": "Example only.",
        "enabled": enabled, "created_at": T0, "updated_at": T1, "targets": targets,
        "scan_settings": scan_settings(renderers), "schedule": schedule, "approval_ids": [],
        "output_formats": ["json", "md"]
    })
}

fn schedule(kind: &str, interval: Value, daily: Value, tz: &str) -> Value {
    json!({"kind": kind, "interval_seconds": interval, "daily_local_time": daily, "timezone": tz,
           "next_due_at": null, "overlap_policy": "skip_if_running", "missed_policy": "record_no_replay"})
}

fn workflows_doc() -> Value {
    with(
        header("workflows"),
        json!({"workflows": [
            workflow_json(
                40, true,
                json!([target_json(10, all_yaml()), target_json(10, explicit(&["deploy/app.yaml"]))]),
                schedule("daily", Value::Null, json!("10:00"), "Asia/Kolkata"),
                json!([renderer(json!({"release": "demo"}))]),
            ),
            workflow_json(
                41, false, json!([target_json(10, patterns(&["deploy/*.yaml"]))]),
                schedule("manual", Value::Null, Value::Null, "UTC"), json!([]),
            ),
            workflow_json(
                42, true, json!([target_json(10, all_yaml())]),
                schedule("interval", json!(3600), Value::Null, "UTC"), json!([]),
            )
        ]}),
    )
}

fn approval_json(n: u32, status: &str, capability: &str, lifetime: &str, over: Value) -> Value {
    with(
        json!({
            "approval_id": uid(n), "status": status, "capability": capability, "workflow_id": null,
            "workflow_revision": null, "run_id": null, "repo_ids": [uid(10)], "purpose": "Example purpose.",
            "operation_fingerprint": DIGEST, "destinations": ["example.invalid"],
            "resource_constraints": [], "data_categories": ["repository_metadata"], "lifetime": lifetime,
            "requested_at": T0, "decided_at": T1, "expires_at": T3, "actor": "reviewer", "consumed": false,
            "inference_scope": null
        }),
        over,
    )
}

fn scope_json() -> Value {
    json!({"provider_id": "openrouter", "model_id": "example/model-1",
           "routing_policy": policy(&["example-provider"]), "source_ids": [uid(80)],
           "redaction_version": "redact-1"})
}

fn approvals_doc() -> Value {
    with(
        header("approvals"),
        json!({"approvals": [
            approval_json(100, "approved", "model_transmission", "run", json!({
                "run_id": uid(20), "destinations": ["openrouter.ai"], "data_categories": ["redacted_source"],
                "inference_scope": scope_json()
            })),
            approval_json(101, "approved", "git_acquire", "once", json!({"destinations": ["github.com"]})),
            approval_json(102, "approved", "external_reference", "once", json!({"consumed": true})),
            approval_json(103, "pending", "local_render", "once", json!({
                "destinations": [], "decided_at": null, "expires_at": null, "actor": ""
            })),
            approval_json(104, "approved", "schema_fetch", "workflow_revision", json!({
                "workflow_id": uid(40), "workflow_revision": 1
            }))
        ]}),
    )
}

fn run_json(
    n: u32,
    execution: &str,
    started: Value,
    finished: Value,
    reports: Value,
    cov: Value,
) -> Value {
    json!({
        "run_id": uid(n), "trigger": "manual", "execution": execution, "created_at": T0,
        "started_at": started, "finished_at": finished, "plan": plan_json(), "coverage": cov,
        "report_ids": reports, "last_checkpoint_at": null
    })
}

fn event_json(n: u32, level: &str, event_type: &str, code: Value) -> Value {
    json!({"event_id": uid(n), "at": T1, "level": level, "event_type": event_type, "run_id": uid(20),
           "repo_id": null, "workflow_id": null, "task_id": null, "approval_id": null,
           "message_redacted": "Example event", "code": code})
}

fn manifest_json() -> Value {
    json!({
        "report_id": uid(110), "run_id": uid(20), "repo_id": uid(10), "json_path": "backup/report-a.json",
        "md_path": "backup/report-a.md", "json_written": true, "md_written": true, "json_digest": DIGEST,
        "generated_at": T2, "pdf_path": "backup/report-a.pdf", "pdf_written": true, "pdf_digest": DIGEST2,
        "pdf_error_redacted": null, "pdf_renderer_version": "renderer-0.1"
    })
}

fn log_doc() -> Value {
    with(
        header("log"),
        json!({
            "runs": [
                run_json(20, "completed", json!(T1), json!(T2), json!([uid(110)]), coverage_json()),
                run_json(21, "queued", Value::Null, Value::Null, json!([]), zero_coverage())
            ],
            "events": [
                event_json(120, "info", "RunStarted", Value::Null),
                event_json(121, "error", "StorageFailure", json!("storage.failure"))
            ],
            "reports": [manifest_json()]
        }),
    )
}

fn suppressions_doc() -> Value {
    with(
        header("suppressions"),
        json!({"suppressions": [{
            "suppression_id": uid(160), "repo_id": uid(10), "finding_fingerprint": DIGEST,
            "rule_id": "rule-1", "reason": "Accepted risk.", "created_at": T0, "expires_at": T3,
            "actor": "reviewer", "enabled": true
        }]}),
    )
}

fn chunk_json(n: u32, text: &str, digest: &str) -> Value {
    json!({
        "chunk_id": uid(n), "source": src(82, "repository_file", json!({
            "repo_id": uid(10), "file_path": "deploy/app.yaml", "line_start": 1, "line_end": 2
        })),
        "text_redacted": text, "content_digest": digest, "redaction_version": "redact-1",
        "embedding_key": null, "indexed_at": T2
    })
}

fn rag_doc() -> Value {
    with(
        header("rag_index"),
        json!({"chunks": [chunk_json(130, "abc", SHA_ABC)]}),
    )
}

fn message_json(n: u32, role: &str, over: Value) -> Value {
    with(
        json!({
            "message_id": uid(n), "role": role, "text_redacted": "Example text.", "source_refs": [],
            "provider_id": null, "model_id": null, "approval_ids": [], "created_at": T1,
            "status": "complete", "tokens_in": null, "tokens_out": null
        }),
        over,
    )
}

fn conversations_doc() -> Value {
    with(
        header("conversations"),
        json!({"conversations": [{
            "conversation_id": uid(140), "title": "Example chat",
            "context": {"repo_ids": [uid(10)], "run_ids": [], "finding_ids": [], "file_ids": [],
                        "include_history": false},
            "created_at": T0, "updated_at": T2,
            "messages": [
                message_json(150, "user", json!({})),
                message_json(151, "assistant", json!({
                    "provider_id": "openrouter", "model_id": "example/model-1",
                    "approval_ids": [uid(100)], "created_at": T2, "tokens_in": 10, "tokens_out": 20,
                    "source_refs": [src(83, "repository_file", json!({
                        "repo_id": uid(10), "file_path": "deploy/app.yaml"
                    }))]
                }))
            ]
        }]}),
    )
}

fn tool_json(adapter: &str, over: Value) -> Value {
    with(
        json!({
            "adapter_id": adapter, "adapter_version": "1.0", "tool_name": adapter, "tool_version": "1.35.1",
            "role": "runtime", "release_tier": "pilot_baseline", "availability": "ready",
            "capabilities": ["syntax", "style"], "frameworks": [], "platforms": ["linux-x86_64"],
            "wrapper_name": null, "invocation_mode": "native", "command_policy_id": "policy-1",
            "parser_id": "parser-1", "native_format": "json", "config_digest": null,
            "selection_strategy": "explicit_files", "network_policy": "none", "network_resources": [],
            "exit_mapping": [
                {"code": 0, "meaning": "no_findings_when_output_confirms"},
                {"code": 1, "meaning": "findings_possible"},
                {"code": 2, "meaning": "execution_error"}
            ],
            "timeout_seconds": 300, "max_output_bytes": 1_048_576, "distribution": "bundled",
            "binary_origin": "vendored", "binary_sha256": DIGEST,
            "license": {"spdx": "MIT", "verified_version": "1.35.1", "review_status": "approved",
                        "license_file": null, "notice_files": [], "source_obligation_note": null,
                        "reviewed_at": T0},
            "limitations": []
        }),
        over,
    )
}

fn tools_doc() -> Value {
    with(
        header("tool_manifest"),
        json!({"tools": [
            tool_json("yamllint", json!({})),
            tool_json("gitleaks", json!({
                "availability": "missing", "tool_version": null, "distribution": "system_prerequisite",
                "binary_origin": null, "binary_sha256": null, "network_policy": "approval_required",
                "network_resources": ["example.invalid"], "exit_mapping": [],
                "license": {"spdx": null, "verified_version": null, "review_status": "pending",
                            "license_file": null, "notice_files": [], "source_obligation_note": null,
                            "reviewed_at": null}
            }))
        ]}),
    )
}

fn ai_not_requested() -> Value {
    json!({
        "status": "not_requested", "provider_id": null, "requested_model": null, "actual_model": null,
        "actual_provider": null, "approval_ids": [], "source_ids": [], "routing_policy": null,
        "redaction_version": "redact-1", "generated_at": null, "narrative_redacted": "",
        "reason_redacted": null
    })
}

fn ai_completed() -> Value {
    json!({
        "status": "completed", "provider_id": "openrouter", "requested_model": "example/model-1",
        "actual_model": null, "actual_provider": null, "approval_ids": [uid(100)],
        "source_ids": [uid(80)], "routing_policy": policy(&["example-provider"]),
        "redaction_version": "redact-1", "generated_at": T2, "narrative_redacted": "A summary.",
        "reason_redacted": null
    })
}

fn artifact_json(n: u32, kind: &str, over: Value) -> Value {
    with(
        json!({
            "artifact_id": uid(n), "kind": kind, "display_name": "app.yaml", "content_digest": DIGEST,
            "retained_path": "tmp/rendered/app.yaml", "source_refs": [src1()], "approval_id": null,
            "contains_only_redacted_retained_content": true
        }),
        over,
    )
}

/// A completed report with a finding, three checks (findings, passed, incomplete), two artifacts and a
/// completed AI enrichment: every rule has material to look at.
fn report_doc() -> Value {
    with(
        header("report"),
        json!({
            "report_id": uid(110), "run_id": uid(20), "repo_id": uid(10), "repo_name": "example-service",
            "remote_identifier_redacted": "github.com/example/example-service", "selected_ref": "main",
            "commit_oid": OID, "execution": "completed", "plan": plan_json(), "generated_at": T2,
            "coverage": coverage_json(),
            "checks": [
                check_json(50, "yamllint", json!({
                    "file_id": uid(30), "outcome": "findings", "finding_ids": [uid(70)],
                    "source_refs": [src1()], "exit_code": 1
                })),
                check_json(51, "gitleaks", json!({"source_refs": [src2()]})),
                check_json(52, "checkov", json!({
                    "outcome": "incomplete", "reason_redacted": "Stopped at the time limit.",
                    "native_status": "timed_out", "exit_code": null, "term_signal": 9
                }))
            ],
            "findings": [finding_json()],
            "artifacts": [
                artifact_json(90, "rendered", json!({})),
                artifact_json(91, "external", json!({
                    "approval_id": uid(102), "retained_path": null, "source_refs": [],
                    "contains_only_redacted_retained_content": false
                }))
            ],
            "limitations": ["Schema fetch was not approved."],
            "ai_enrichment": ai_completed()
        }),
    )
}

/// A queued report with nothing in it but a plan: the smallest document the semantic rules accept.
fn minimal_report() -> Value {
    with(
        header("report"),
        json!({
            "report_id": uid(110), "run_id": uid(20), "repo_id": uid(10), "repo_name": "example-service",
            "remote_identifier_redacted": "github.com/example/example-service", "selected_ref": "main",
            "commit_oid": null, "execution": "queued", "plan": plan_json(), "generated_at": T2,
            "coverage": with(zero_coverage(), json!({"discovered_files": 1, "selected_files": 1})),
            "checks": [], "findings": [], "artifacts": [], "limitations": [],
            "ai_enrichment": ai_not_requested()
        }),
    )
}

fn minimal_docs() -> Vec<(StoreKind, Value)> {
    vec![
        (
            StoreKind::Config,
            with(
                config_doc(),
                json!({"config": {
                    "app_data_root": "/data", "backup_directory": "backup", "active_theme": "default",
                    "max_active_repositories": 1, "max_scanner_processes": 1, "history_retention_days": null,
                    "chat_persistence_enabled": false, "provider": null, "telemetry_enabled": false,
                    "redaction_version": "redact-1"
                }}),
            ),
        ),
        (
            StoreKind::Repositories,
            with(repositories_doc(), json!({"repositories": []})),
        ),
        (
            StoreKind::Workflows,
            with(workflows_doc(), json!({"workflows": []})),
        ),
        (
            StoreKind::Approvals,
            with(approvals_doc(), json!({"approvals": []})),
        ),
        (
            StoreKind::Log,
            with(log_doc(), json!({"runs": [], "events": [], "reports": []})),
        ),
        (
            StoreKind::Suppressions,
            with(suppressions_doc(), json!({"suppressions": []})),
        ),
        (StoreKind::RagIndex, with(rag_doc(), json!({"chunks": []}))),
        (
            StoreKind::Conversations,
            with(conversations_doc(), json!({"conversations": []})),
        ),
        (StoreKind::Report, minimal_report()),
        (
            StoreKind::ToolManifest,
            with(tools_doc(), json!({"tools": []})),
        ),
    ]
}

fn rich_docs() -> Vec<(StoreKind, Value)> {
    vec![
        (StoreKind::Config, config_doc()),
        (StoreKind::Repositories, repositories_doc()),
        (StoreKind::Workflows, workflows_doc()),
        (StoreKind::Approvals, approvals_doc()),
        (StoreKind::Log, log_doc()),
        (StoreKind::Suppressions, suppressions_doc()),
        (StoreKind::RagIndex, rag_doc()),
        (StoreKind::Conversations, conversations_doc()),
        (StoreKind::Report, report_doc()),
        (StoreKind::ToolManifest, tools_doc()),
    ]
}

// --- decoding, editing, running -------------------------------------------------------------------------

fn decode(kind: StoreKind, v: Value) -> AnyStore {
    fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap_or_else(|e| panic!("fixture does not decode: {e}"))
    }
    match kind {
        StoreKind::Config => AnyStore::Config(from::<ConfigFile>(v)),
        StoreKind::Repositories => AnyStore::Repositories(from::<RepositoryFile>(v)),
        StoreKind::Workflows => AnyStore::Workflows(from::<WorkflowFile>(v)),
        StoreKind::Approvals => AnyStore::Approvals(from::<ApprovalFile>(v)),
        StoreKind::Log => AnyStore::Log(from::<LogFile>(v)),
        StoreKind::Suppressions => AnyStore::Suppressions(from::<SuppressionFile>(v)),
        StoreKind::RagIndex => AnyStore::RagIndex(from::<RagIndexFile>(v)),
        StoreKind::Conversations => AnyStore::Conversations(from::<ConversationFile>(v)),
        StoreKind::Report => AnyStore::Report(Box::new(from::<ReportFile>(v))),
        StoreKind::ToolManifest => AnyStore::ToolManifest(from::<ToolManifestFile>(v)),
    }
}

/// One edit of a base document.
enum Edit {
    /// Replaces the value at the pointer.
    Set(&'static str, Value),
    /// Appends a value to the array at the pointer.
    Push(&'static str, Value),
    /// Appends a copy of the value at the first pointer to the array at the second.
    Dup(&'static str, &'static str),
}

fn set(p: &'static str, v: Value) -> Edit {
    Edit::Set(p, v)
}

fn push(p: &'static str, v: Value) -> Edit {
    Edit::Push(p, v)
}

fn apply(doc: &mut Value, edits: &[Edit]) {
    for e in edits {
        match e {
            Edit::Set(p, v) => {
                *doc.pointer_mut(p)
                    .unwrap_or_else(|| panic!("no such pointer {p}")) = v.clone();
            }
            Edit::Push(p, v) => doc
                .pointer_mut(p)
                .and_then(Value::as_array_mut)
                .unwrap_or_else(|| panic!("no array at {p}"))
                .push(v.clone()),
            Edit::Dup(from, to) => {
                let copy = doc
                    .pointer(from)
                    .cloned()
                    .unwrap_or_else(|| panic!("no such pointer {from}"));
                doc.pointer_mut(to)
                    .and_then(Value::as_array_mut)
                    .unwrap_or_else(|| panic!("no array at {to}"))
                    .push(copy);
            }
        }
    }
}

fn found(problems: &Problems) -> BTreeSet<(String, String)> {
    let mut set = BTreeSet::new();
    for p in &problems.items {
        assert_eq!(p.layer, Layer::Semantic);
        assert_eq!(p.code, ProblemCode::Rule);
        let rule = p.rule.expect("a semantic problem names its rule").0;
        set.insert((rule.to_owned(), p.pointer.as_str().to_owned()));
    }
    set
}

fn pairs(list: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    list.iter()
        .map(|(r, p)| ((*r).to_owned(), (*p).to_owned()))
        .collect()
}

struct Case {
    rule: &'static str,
    name: &'static str,
    kind: StoreKind,
    base: fn() -> Value,
    edits: Vec<Edit>,
    /// Empty for a positive case.
    expect: Vec<(&'static str, &'static str)>,
}

fn rich(kind: StoreKind) -> fn() -> Value {
    match kind {
        StoreKind::Config => config_doc,
        StoreKind::Repositories => repositories_doc,
        StoreKind::Workflows => workflows_doc,
        StoreKind::Approvals => approvals_doc,
        StoreKind::Log => log_doc,
        StoreKind::Suppressions => suppressions_doc,
        StoreKind::RagIndex => rag_doc,
        StoreKind::Conversations => conversations_doc,
        StoreKind::Report => report_doc,
        StoreKind::ToolManifest => tools_doc,
    }
}

/// A negative case on the rich document of `kind`.
fn neg(
    rule: &'static str,
    name: &'static str,
    kind: StoreKind,
    edits: Vec<Edit>,
    expect: &[(&'static str, &'static str)],
) -> Case {
    Case {
        rule,
        name,
        kind,
        base: rich(kind),
        edits,
        expect: expect.to_vec(),
    }
}

/// A positive case on the rich document of `kind`: valid, so no rule reports anything.
fn pos(rule: &'static str, name: &'static str, kind: StoreKind, edits: Vec<Edit>) -> Case {
    Case {
        rule,
        name,
        kind,
        base: rich(kind),
        edits,
        expect: Vec::new(),
    }
}

/// A negative case on the minimal report.
fn neg_min(
    rule: &'static str,
    name: &'static str,
    edits: Vec<Edit>,
    expect: &[(&'static str, &'static str)],
) -> Case {
    Case {
        rule,
        name,
        kind: StoreKind::Report,
        base: minimal_report,
        edits,
        expect: expect.to_vec(),
    }
}

/// A positive case on the minimal report.
fn pos_min(rule: &'static str, name: &'static str, edits: Vec<Edit>) -> Case {
    Case {
        rule,
        name,
        kind: StoreKind::Report,
        base: minimal_report,
        edits,
        expect: Vec::new(),
    }
}

fn run_case(c: &Case) {
    let mut doc = (c.base)();
    apply(&mut doc, &c.edits);
    let problems = check(&decode(c.kind, doc));
    assert!(!problems.truncated, "{} / {}: truncated", c.rule, c.name);
    assert_eq!(
        found(&problems),
        pairs(&c.expect),
        "{} / {}: reported problems differ",
        c.rule,
        c.name
    );
    if !c.expect.is_empty() {
        assert!(
            c.expect.iter().any(|(r, _)| *r == c.rule),
            "{} / {}: the case does not expect its own rule",
            c.rule,
            c.name
        );
    }
}

// --- registry -------------------------------------------------------------------------------------------

fn expected_ids() -> Vec<String> {
    let mut ids: Vec<String> = (1..=25).map(|n| format!("D11-{n:02}")).collect();
    ids.extend((1..=18).map(|n| format!("D18-{n:02}")));
    ids.push(String::from("D18-06a"));
    ids
}

#[test]
fn registry_lists_every_rule_id_once_in_order_and_nothing_else() {
    let got: Vec<&str> = rule_table().map(|r| r.id.0).collect();
    assert_eq!(got.len(), 44);
    let want = expected_ids();
    assert_eq!(got, want.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn every_unchecked_or_partial_rule_names_where_the_rest_is_enforced() {
    for r in rule_table() {
        match r.status {
            RuleStatus::Implemented => {}
            RuleStatus::Partial { rest_in } => {
                assert!(!rest_in.trim().is_empty(), "{}", r.id.0);
            }
            RuleStatus::Producer { phase } => assert!(!phase.trim().is_empty(), "{}", r.id.0),
            RuleStatus::Policy { phase } => {
                assert!(phase.is_some_and(|p| !p.trim().is_empty()), "{}", r.id.0);
            }
        }
        assert!(!r.title.trim().is_empty(), "{}", r.id.0);
    }
}

#[test]
fn registry_status_counts_are_as_reviewed() {
    let count = |f: fn(&RuleStatus) -> bool| rule_table().filter(|r| f(&r.status)).count();
    assert_eq!(count(|s| matches!(s, RuleStatus::Implemented)), 17);
    assert_eq!(count(|s| matches!(s, RuleStatus::Partial { .. })), 16);
    assert_eq!(count(|s| matches!(s, RuleStatus::Producer { .. })), 9);
    assert_eq!(count(|s| matches!(s, RuleStatus::Policy { .. })), 2);
}

// --- valid documents ------------------------------------------------------------------------------------

#[test]
fn rich_and_minimal_documents_of_every_root_kind_report_nothing() {
    for (kind, doc) in rich_docs().into_iter().chain(minimal_docs()) {
        let problems = check(&decode(kind, doc));
        assert!(
            problems.is_empty(),
            "{kind:?} is valid but reported {:?}",
            found(&problems)
        );
    }
}

#[test]
fn every_root_kind_has_a_rich_and_a_minimal_document() {
    let kinds =
        |docs: Vec<(StoreKind, Value)>| docs.into_iter().map(|(k, _)| k).collect::<Vec<_>>();
    assert_eq!(kinds(rich_docs()), StoreKind::ALL.to_vec());
    assert_eq!(kinds(minimal_docs()), StoreKind::ALL.to_vec());
}

#[test]
fn check_is_deterministic_and_does_not_change_the_document() {
    for (kind, mut doc) in rich_docs() {
        if kind == StoreKind::Approvals {
            apply(&mut doc, &[set("/approvals/3/decided_at", json!(T1))]);
        }
        let store = decode(kind, doc);
        let before = store.clone();
        assert_eq!(check(&store), check(&store), "{kind:?}");
        assert_eq!(store, before, "{kind:?}");
    }
}

// --- cases ----------------------------------------------------------------------------------------------

fn cases_ids() -> Vec<Case> {
    use StoreKind::{
        Approvals, Conversations, Log, RagIndex, Report, Repositories, Suppressions, ToolManifest,
        Workflows,
    };
    let repo2 = repo_json(
        11,
        "https://github.com/example/other.git",
        "github.com/example/other",
        json!([]),
    );
    let flow2 = workflow_json(
        40,
        false,
        json!([]),
        schedule("manual", Value::Null, Value::Null, "UTC"),
        json!([]),
    );
    vec![
        neg(
            "D11-01",
            "file id repeated in one repository",
            Repositories,
            vec![push(
                "/repositories/0/files",
                file_json(30, "deploy/other.yaml", "eligible", json!(DIGEST)),
            )],
            &[("D11-01", "/repositories/0/files/2/file_id")],
        ),
        neg(
            "D11-01",
            "file path repeated in one repository",
            Repositories,
            vec![push(
                "/repositories/0/files",
                file_json(32, "deploy/app.yaml", "eligible", json!(DIGEST)),
            )],
            &[("D11-01", "/repositories/0/files/2/path")],
        ),
        neg(
            "D11-01",
            "repository id repeated",
            Repositories,
            vec![push(
                "/repositories",
                with(
                    repo2.clone(),
                    json!({"repo_id": uid(10), "local_path": format!("checkouts/{}", uid(10))}),
                ),
            )],
            &[("D11-01", "/repositories/1/repo_id")],
        ),
        neg(
            "D11-01",
            "file id repeated across repositories",
            Repositories,
            vec![push(
                "/repositories",
                with(
                    repo2.clone(),
                    json!({"files": [file_json(30, "x.yaml", "eligible", json!(DIGEST))]}),
                ),
            )],
            &[("D11-01", "/repositories/1/files/0/file_id")],
        ),
        pos(
            "D11-01",
            "a second repository with its own ids",
            Repositories,
            vec![push("/repositories", repo2)],
        ),
        neg(
            "D11-01",
            "workflow id repeated",
            Workflows,
            vec![push("/workflows", flow2.clone())],
            &[("D11-01", "/workflows/3/workflow_id")],
        ),
        neg(
            "D11-01",
            "workflow approval id repeated",
            Workflows,
            vec![set(
                "/workflows/0/approval_ids",
                json!([uid(104), uid(104)]),
            )],
            &[("D11-01", "/workflows/0/approval_ids/1")],
        ),
        pos(
            "D11-01",
            "a workflow with its own id",
            Workflows,
            vec![push(
                "/workflows",
                with(flow2, json!({"workflow_id": uid(43)})),
            )],
        ),
        neg(
            "D11-01",
            "approval id repeated",
            Approvals,
            vec![Edit::Dup("/approvals/0", "/approvals")],
            &[("D11-01", "/approvals/5/approval_id")],
        ),
        neg(
            "D11-01",
            "run id repeated",
            Log,
            vec![set("/runs/1/run_id", json!(uid(20)))],
            &[("D11-01", "/runs/1/run_id")],
        ),
        neg(
            "D11-01",
            "event id repeated",
            Log,
            vec![set("/events/1/event_id", json!(uid(120)))],
            &[("D11-01", "/events/1/event_id")],
        ),
        neg(
            "D11-01",
            "report manifest duplicated",
            Log,
            vec![Edit::Dup("/reports/0", "/reports")],
            &[
                ("D11-01", "/reports/1/report_id"),
                ("D11-01", "/reports/1/repo_id"),
            ],
        ),
        neg(
            "D11-01",
            "manifest names another run than the run that lists it",
            Log,
            vec![set("/reports/0/run_id", json!(uid(21)))],
            &[("D11-01", "/reports/0/run_id")],
        ),
        neg(
            "D11-01",
            "run lists a report twice",
            Log,
            vec![set("/runs/0/report_ids", json!([uid(110), uid(110)]))],
            &[("D11-01", "/runs/0/report_ids/1")],
        ),
        pos(
            "D11-01",
            "a manifest of a run that does not list it (compacted history)",
            Log,
            vec![set("/runs/0/report_ids", json!([]))],
        ),
        neg(
            "D11-01",
            "check id repeated",
            Report,
            vec![set("/checks/1/check_id", json!(uid(50)))],
            &[("D11-01", "/checks/1/check_id")],
        ),
        neg(
            "D11-01",
            "artifact id repeated",
            Report,
            vec![set("/artifacts/1/artifact_id", json!(uid(90)))],
            &[("D11-01", "/artifacts/1/artifact_id")],
        ),
        neg(
            "D11-01",
            "finding id listed twice by a check",
            Report,
            vec![set("/checks/0/finding_ids", json!([uid(70), uid(70)]))],
            &[("D11-01", "/checks/0/finding_ids/1")],
        ),
        neg(
            "D11-01",
            "finding of another run",
            Report,
            vec![set("/findings/0/run_id", json!(uid(21)))],
            &[("D11-01", "/findings/0/run_id")],
        ),
        neg(
            "D11-01",
            "source reference of another run",
            Report,
            vec![set("/checks/1/source_refs/0/run_id", json!(uid(21)))],
            &[("D11-01", "/checks/1/source_refs/0/run_id")],
        ),
        neg(
            "D11-01",
            "one source id, two different source references",
            Report,
            vec![set("/findings/0/sources/0/stale", json!(true))],
            &[("D11-01", "/findings/0/sources/0/source_id")],
        ),
        pos(
            "D11-01",
            "the same source reference cited in three places (base)",
            Report,
            vec![],
        ),
        neg(
            "D11-01",
            "plan with a revision but no workflow",
            Report,
            vec![set("/plan/workflow_id", Value::Null)],
            &[("D11-01", "/plan/workflow_revision")],
        ),
        neg(
            "D11-01",
            "plan with revision zero",
            Report,
            vec![set("/plan/workflow_revision", json!(0))],
            &[("D11-01", "/plan/workflow_revision")],
        ),
        pos(
            "D11-01",
            "an ad-hoc plan has neither workflow nor revision",
            Report,
            vec![
                set("/plan/workflow_id", Value::Null),
                set("/plan/workflow_revision", Value::Null),
            ],
        ),
        neg(
            "D11-01",
            "plan engine repeated",
            Report,
            vec![Edit::Dup(
                "/plan/engine_snapshots/0",
                "/plan/engine_snapshots",
            )],
            &[("D11-01", "/plan/engine_snapshots/1/adapter_id")],
        ),
        neg(
            "D11-01",
            "plan approval id repeated (log run)",
            Log,
            vec![set(
                "/runs/0/plan/approval_ids",
                json!([uid(100), uid(100)]),
            )],
            &[("D11-01", "/runs/0/plan/approval_ids/1")],
        ),
        neg(
            "D11-01",
            "chunk id repeated",
            RagIndex,
            vec![Edit::Dup("/chunks/0", "/chunks")],
            &[("D11-01", "/chunks/1/chunk_id")],
        ),
        neg(
            "D11-01",
            "message id repeated",
            Conversations,
            vec![set(
                "/conversations/0/messages/1/message_id",
                json!(uid(150)),
            )],
            &[("D11-01", "/conversations/0/messages/1/message_id")],
        ),
        neg(
            "D11-01",
            "conversation id repeated",
            Conversations,
            vec![Edit::Dup("/conversations/0", "/conversations")],
            &[
                ("D11-01", "/conversations/1/conversation_id"),
                ("D11-01", "/conversations/1/messages/0/message_id"),
                ("D11-01", "/conversations/1/messages/1/message_id"),
            ],
        ),
        neg(
            "D11-01",
            "suppression id repeated",
            Suppressions,
            vec![Edit::Dup("/suppressions/0", "/suppressions")],
            &[("D11-01", "/suppressions/1/suppression_id")],
        ),
        neg(
            "D11-01",
            "tool adapter repeated",
            ToolManifest,
            vec![Edit::Dup("/tools/0", "/tools")],
            &[("D11-01", "/tools/2/adapter_id")],
        ),
        neg_min(
            "D11-01",
            "plan snapshot file id repeated inside one target",
            vec![
                push(
                    "/plan/target_snapshots/0/selected_files",
                    file_json(30, "deploy/other.yaml", "eligible", json!(DIGEST)),
                ),
                set("/coverage/selected_files", json!(2)),
            ],
            &[(
                "D11-01",
                "/plan/target_snapshots/0/selected_files/1/file_id",
            )],
        ),
    ]
}

fn cases_scope() -> Vec<Case> {
    use StoreKind::{Log, Report};
    vec![
        neg(
            "D11-03",
            "finding of another repository",
            Report,
            vec![set("/findings/0/repo_id", json!(uid(11)))],
            &[("D11-03", "/findings/0/repo_id")],
        ),
        neg(
            "D11-03",
            "source reference of another repository",
            Report,
            vec![set("/checks/1/source_refs/0/repo_id", json!(uid(11)))],
            &[("D11-03", "/checks/1/source_refs/0/repo_id")],
        ),
        neg(
            "D11-03",
            "check on a file the plan did not select",
            Report,
            vec![set("/checks/0/file_id", json!(uid(31)))],
            &[("D11-03", "/checks/0/file_id")],
        ),
        neg(
            "D11-03",
            "report repository is not a plan target",
            Report,
            vec![set("/plan/targets/0/repo_id", json!(uid(11)))],
            &[
                ("D11-03", "/repo_id"),
                ("D11-03", "/plan/target_snapshots/0/repo_id"),
            ],
        ),
        neg(
            "D11-03",
            "log run snapshot of a repository that is not a target",
            Log,
            vec![set("/runs/0/plan/targets/0/repo_id", json!(uid(11)))],
            &[("D11-03", "/runs/0/plan/target_snapshots/0/repo_id")],
        ),
        pos(
            "D11-03",
            "a second plan target without a snapshot",
            Report,
            vec![push("/plan/targets", target_json(11, all_yaml()))],
        ),
        pos(
            "D11-03",
            "a check without a file under a plan with snapshots (base check 51)",
            Report,
            vec![],
        ),
    ]
}

fn cases_findings() -> Vec<Case> {
    use StoreKind::Report;
    vec![
        neg(
            "D11-22",
            "finding without a source and without a limitation",
            Report,
            vec![
                set("/findings/0/sources", json!([])),
                set("/limitations", json!([])),
            ],
            &[("D11-22", "/findings/0/sources")],
        ),
        pos(
            "D11-22",
            "finding without a source but with a stated limitation",
            Report,
            vec![set("/findings/0/sources", json!([]))],
        ),
        pos(
            "D11-22",
            "finding with a source and no limitation",
            Report,
            vec![set("/limitations", json!([]))],
        ),
        neg(
            "D11-04",
            "check lists a finding that does not exist",
            Report,
            vec![set("/checks/0/finding_ids", json!([uid(71)]))],
            &[
                ("D11-04", "/checks/0/finding_ids/0"),
                ("D11-04", "/findings/0/check_id"),
            ],
        ),
        neg(
            "D11-04",
            "finding task differs from its check",
            Report,
            vec![set("/findings/0/task_id", json!(uid(61)))],
            &[("D11-04", "/findings/0/task_id")],
        ),
        neg(
            "D11-04",
            "finding names a check that does not exist",
            Report,
            vec![set("/findings/0/check_id", json!(uid(59)))],
            &[
                ("D11-04", "/findings/0/check_id"),
                ("D11-04", "/checks/0/finding_ids/0"),
            ],
        ),
        neg(
            "D11-04",
            "a passed check lists a finding",
            Report,
            vec![set("/checks/1/finding_ids", json!([uid(70)]))],
            &[
                ("D11-04", "/checks/1/finding_ids/0"),
                ("D11-04", "/checks/1/finding_ids"),
            ],
        ),
        neg(
            "D11-04",
            "a check with outcome findings lists none",
            Report,
            vec![set("/checks/0/finding_ids", json!([]))],
            &[
                ("D11-04", "/checks/0/finding_ids"),
                ("D11-04", "/findings/0/check_id"),
            ],
        ),
        pos(
            "D11-04",
            "an incomplete check keeps its findings",
            Report,
            vec![
                set("/checks/0/outcome", json!("incomplete")),
                set(
                    "/checks/0/reason_redacted",
                    json!("Stopped at the time limit."),
                ),
                set("/coverage/checks_with_findings", json!(0)),
                set("/coverage/checks_incomplete", json!(2)),
            ],
        ),
    ]
}

fn cases_sources() -> Vec<Case> {
    use StoreKind::{RagIndex, Report};
    vec![
        neg(
            "D11-05",
            "rendered artifact source without an artifact",
            Report,
            vec![set("/checks/1/source_refs/0/artifact_id", Value::Null)],
            &[("D11-05", "/checks/1/source_refs/0/artifact_id")],
        ),
        neg(
            "D11-05",
            "artifact that is not in the report",
            Report,
            vec![set("/checks/1/source_refs/0/artifact_id", json!(uid(99)))],
            &[("D11-05", "/checks/1/source_refs/0/artifact_id")],
        ),
        neg(
            "D11-05",
            "scan report source without a run",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("scan_report")),
                set("/checks/1/source_refs/0/run_id", Value::Null),
            ],
            &[("D11-05", "/checks/1/source_refs/0/run_id")],
        ),
        neg(
            "D11-05",
            "repository file source without repository and path",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("repository_file")),
                set("/checks/1/source_refs/0/repo_id", Value::Null),
            ],
            &[
                ("D11-05", "/checks/1/source_refs/0/repo_id"),
                ("D11-05", "/checks/1/source_refs/0/file_path"),
            ],
        ),
        neg(
            "D11-05",
            "external reference without identifier",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("external_reference")),
                set("/checks/1/source_refs/0/artifact_id", Value::Null),
            ],
            &[("D11-05", "/checks/1/source_refs/0/resource_identifier")],
        ),
        neg(
            "D11-05",
            "retrieved external content without an approval",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("external_reference")),
                set("/checks/1/source_refs/0/artifact_id", Value::Null),
                set(
                    "/checks/1/source_refs/0/resource_identifier",
                    json!("https://example.invalid/reference"),
                ),
                set("/checks/1/source_refs/0/content_digest", json!(DIGEST)),
            ],
            &[("D11-05", "/checks/1/source_refs/0/approval_id")],
        ),
        pos(
            "D11-05",
            "retrieved external content with its approval",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("external_reference")),
                set("/checks/1/source_refs/0/artifact_id", Value::Null),
                set(
                    "/checks/1/source_refs/0/resource_identifier",
                    json!("https://example.invalid/reference"),
                ),
                set("/checks/1/source_refs/0/content_digest", json!(DIGEST)),
                set("/checks/1/source_refs/0/approval_id", json!(uid(102))),
            ],
        ),
        neg(
            "D11-05",
            "rule document without any locator",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("rule_document")),
                set("/checks/1/source_refs/0/artifact_id", Value::Null),
            ],
            &[("D11-05", "/checks/1/source_refs/0/resource_identifier")],
        ),
        pos(
            "D11-05",
            "activity log and workflow sources need nothing more",
            Report,
            vec![
                set("/checks/1/source_refs/0/kind", json!("activity_log")),
                set("/checks/1/source_refs/0/artifact_id", Value::Null),
                set("/checks/1/source_refs/0/repo_id", Value::Null),
                set("/checks/1/source_refs/0/run_id", Value::Null),
            ],
        ),
        neg(
            "D11-05",
            "external artifact without an approval",
            Report,
            vec![set("/artifacts/1/approval_id", Value::Null)],
            &[("D11-05", "/artifacts/1/approval_id")],
        ),
        neg(
            "D11-05",
            "chunk source of a repository file without a path",
            RagIndex,
            vec![set("/chunks/0/source/file_path", Value::Null)],
            &[("D11-05", "/chunks/0/source/file_path")],
        ),
        pos(
            "D11-05",
            "a chunk may name an artifact of another document",
            RagIndex,
            vec![
                set("/chunks/0/source/kind", json!("rendered_artifact")),
                set("/chunks/0/source/artifact_id", json!(uid(99))),
            ],
        ),
    ]
}

fn cases_paths() -> Vec<Case> {
    use StoreKind::{Log, Report, Repositories};
    let dup_remote = repo_json(
        11,
        "https://github.com/example/example-service",
        "github.com/example/example-service",
        json!([]),
    );
    vec![
        neg(
            "D11-06",
            "checkout path is not the repository's",
            Repositories,
            vec![set(
                "/repositories/0/local_path",
                json!("checkouts/elsewhere"),
            )],
            &[("D11-06", "/repositories/0/local_path")],
        ),
        neg(
            "D11-06",
            "canonical remote differs from the URL",
            Repositories,
            vec![set(
                "/repositories/0/canonical_remote",
                json!("github.com/example/other"),
            )],
            &[("D11-06", "/repositories/0/canonical_remote")],
        ),
        neg(
            "D11-06",
            "canonical remote registered twice",
            Repositories,
            vec![push("/repositories", dup_remote)],
            &[("D11-06", "/repositories/1/canonical_remote")],
        ),
        pos(
            "D11-06",
            "scp-like remote",
            Repositories,
            vec![set(
                "/repositories/0/remote_url",
                json!("git@github.com:example/example-service.git"),
            )],
        ),
        pos(
            "D11-06",
            "ssh remote with a port and user",
            Repositories,
            vec![
                set(
                    "/repositories/0/remote_url",
                    json!("ssh://git@example.com:2222/example/example-service.git"),
                ),
                set(
                    "/repositories/0/canonical_remote",
                    json!("example.com/example/example-service"),
                ),
            ],
        ),
        neg(
            "D11-06",
            "markdown path has another extension",
            Log,
            vec![set("/reports/0/md_path", json!("backup/report-a.txt"))],
            &[("D11-06", "/reports/0/md_path")],
        ),
        neg(
            "D11-06",
            "markdown stem differs",
            Log,
            vec![set("/reports/0/md_path", json!("backup/other.md"))],
            &[("D11-06", "/reports/0/md_path")],
        ),
        neg(
            "D11-06",
            "pdf in another directory",
            Log,
            vec![set("/reports/0/pdf_path", json!("other/report-a.pdf"))],
            &[("D11-06", "/reports/0/pdf_path")],
        ),
        neg(
            "D11-06",
            "json path has the wrong extension",
            Log,
            vec![set("/reports/0/json_path", json!("backup/report-a.js"))],
            &[("D11-06", "/reports/0/json_path")],
        ),
        neg(
            "D11-06",
            "manifest points into checkouts",
            Log,
            vec![
                set("/reports/0/json_path", json!("checkouts/report-a.json")),
                set("/reports/0/md_path", json!("checkouts/report-a.md")),
                set("/reports/0/pdf_path", json!("checkouts/report-a.pdf")),
            ],
            &[
                ("D11-06", "/reports/0/json_path"),
                ("D11-06", "/reports/0/md_path"),
                ("D11-06", "/reports/0/pdf_path"),
            ],
        ),
        pos(
            "D11-06",
            "a manifest without a pdf",
            Log,
            vec![
                set("/reports/0/pdf_path", Value::Null),
                set("/reports/0/pdf_written", json!(false)),
                set("/reports/0/pdf_digest", Value::Null),
                set("/reports/0/pdf_renderer_version", Value::Null),
            ],
        ),
        neg(
            "D11-06",
            "remote identifier with a password",
            Report,
            vec![set(
                "/remote_identifier_redacted",
                json!("https://user:secretexample@github.com/example/example-service"),
            )],
            &[("D11-06", "/remote_identifier_redacted")],
        ),
        neg(
            "D11-06",
            "remote identifier with a token user over https",
            Report,
            vec![set(
                "/remote_identifier_redacted",
                json!("https://tokenexample@github.com/example/example-service"),
            )],
            &[("D11-06", "/remote_identifier_redacted")],
        ),
        neg(
            "D11-06",
            "remote identifier with a query",
            Report,
            vec![set(
                "/remote_identifier_redacted",
                json!("https://github.com/example/example-service?token=example"),
            )],
            &[("D11-06", "/remote_identifier_redacted")],
        ),
        pos(
            "D11-06",
            "ssh user without a password and scp-like identifiers",
            Report,
            vec![set(
                "/remote_identifier_redacted",
                json!("ssh://git@github.com/example/example-service"),
            )],
        ),
        pos(
            "D11-06",
            "scp-like identifier",
            Report,
            vec![set(
                "/remote_identifier_redacted",
                json!("git@github.com:example/example-service"),
            )],
        ),
        neg(
            "D11-06",
            "resource identifier with a query",
            Report,
            vec![set(
                "/checks/1/source_refs/0/resource_identifier",
                json!("https://example.invalid/x?token=example"),
            )],
            &[("D11-06", "/checks/1/source_refs/0/resource_identifier")],
        ),
        pos(
            "D11-06",
            "resource identifier with a fragment",
            Report,
            vec![set(
                "/checks/1/source_refs/0/resource_identifier",
                json!("https://example.invalid/schema.json#/defs/a"),
            )],
        ),
        neg(
            "D11-06",
            "retained artifact path inside checkouts",
            Report,
            vec![set("/artifacts/0/retained_path", json!("checkouts/x"))],
            &[("D11-06", "/artifacts/0/retained_path")],
        ),
    ]
}

fn cases_selectors() -> Vec<Case> {
    use StoreKind::{Log, Report, Workflows};
    vec![
        neg(
            "D11-07",
            "all_yaml with explicit paths",
            Workflows,
            vec![set(
                "/workflows/0/targets/0/selector/paths",
                json!(["deploy/app.yaml"]),
            )],
            &[("D11-07", "/workflows/0/targets/0/selector/paths")],
        ),
        neg(
            "D11-07",
            "all_yaml with include patterns",
            Workflows,
            vec![set(
                "/workflows/0/targets/0/selector/include_patterns",
                json!(["deploy/*.yaml"]),
            )],
            &[("D11-07", "/workflows/0/targets/0/selector/include_patterns")],
        ),
        pos(
            "D11-07",
            "all_yaml may exclude",
            Workflows,
            vec![set(
                "/workflows/0/targets/0/selector/exclude_patterns",
                json!(["vendor/**"]),
            )],
        ),
        neg(
            "D11-07",
            "explicit with include patterns",
            Workflows,
            vec![set(
                "/workflows/0/targets/1/selector/include_patterns",
                json!(["*.yaml"]),
            )],
            &[("D11-07", "/workflows/0/targets/1/selector/include_patterns")],
        ),
        neg(
            "D11-07",
            "explicit with exclude patterns",
            Workflows,
            vec![set(
                "/workflows/0/targets/1/selector/exclude_patterns",
                json!(["*.yaml"]),
            )],
            &[("D11-07", "/workflows/0/targets/1/selector/exclude_patterns")],
        ),
        neg(
            "D11-07",
            "explicit path repeated",
            Workflows,
            vec![set(
                "/workflows/0/targets/1/selector/paths",
                json!(["a.yaml", "a.yaml"]),
            )],
            &[("D11-07", "/workflows/0/targets/1/selector/paths/1")],
        ),
        neg(
            "D11-07",
            "patterns with explicit paths",
            Workflows,
            vec![set(
                "/workflows/1/targets/0/selector/paths",
                json!(["a.yaml"]),
            )],
            &[("D11-07", "/workflows/1/targets/0/selector/paths")],
        ),
        neg(
            "D11-07",
            "report plan target",
            Report,
            vec![set("/plan/targets/0/selector/paths", json!(["a.yaml"]))],
            &[("D11-07", "/plan/targets/0/selector/paths")],
        ),
        neg(
            "D11-07",
            "log run plan target",
            Log,
            vec![set(
                "/runs/1/plan/targets/0/selector/paths",
                json!(["a.yaml"]),
            )],
            &[("D11-07", "/runs/1/plan/targets/0/selector/paths")],
        ),
    ]
}

fn cases_runnable() -> Vec<Case> {
    use StoreKind::{Log, Report, Workflows};
    vec![
        neg(
            "D11-08",
            "revision zero",
            Workflows,
            vec![set("/workflows/0/revision", json!(0))],
            &[("D11-08", "/workflows/0/revision")],
        ),
        neg(
            "D11-08",
            "enabled without targets",
            Workflows,
            vec![set("/workflows/0/targets", json!([]))],
            &[("D11-08", "/workflows/0/targets")],
        ),
        neg(
            "D11-08",
            "enabled without categories",
            Workflows,
            vec![set("/workflows/0/scan_settings/categories", json!([]))],
            &[("D11-08", "/workflows/0/scan_settings/categories")],
        ),
        neg(
            "D11-08",
            "enabled without json output",
            Workflows,
            vec![set("/workflows/0/output_formats", json!(["md", "pdf"]))],
            &[("D11-08", "/workflows/0/output_formats")],
        ),
        neg(
            "D11-08",
            "enabled without any output",
            Workflows,
            vec![set("/workflows/0/output_formats", json!([]))],
            &[("D11-08", "/workflows/0/output_formats")],
        ),
        neg(
            "D11-08",
            "output format repeated",
            Workflows,
            vec![set("/workflows/0/output_formats", json!(["json", "json"]))],
            &[("D11-08", "/workflows/0/output_formats/1")],
        ),
        neg(
            "D11-08",
            "enabled explicit target without paths",
            Workflows,
            vec![set("/workflows/0/targets/1/selector/paths", json!([]))],
            &[("D11-08", "/workflows/0/targets/1/selector/paths")],
        ),
        neg(
            "D11-08",
            "enabled patterns target without include patterns",
            Workflows,
            vec![
                set("/workflows/1/enabled", json!(true)),
                set(
                    "/workflows/1/targets/0/selector/include_patterns",
                    json!([]),
                ),
            ],
            &[("D11-08", "/workflows/1/targets/0/selector/include_patterns")],
        ),
        pos(
            "D11-08",
            "a disabled draft may be empty",
            Workflows,
            vec![
                set("/workflows/1/targets", json!([])),
                set("/workflows/1/output_formats", json!([])),
                set("/workflows/1/scan_settings/categories", json!([])),
            ],
        ),
        pos(
            "D11-08",
            "a disabled draft may hold an incomplete selection",
            Workflows,
            vec![set(
                "/workflows/1/targets/0/selector/include_patterns",
                json!([]),
            )],
        ),
        neg(
            "D11-08",
            "plan without targets",
            Report,
            vec![set("/plan/targets", json!([]))],
            &[
                ("D11-08", "/plan/targets"),
                ("D11-03", "/plan/target_snapshots/0/repo_id"),
            ],
        ),
        neg(
            "D11-08",
            "plan without categories",
            Report,
            vec![set("/plan/scan_settings/categories", json!([]))],
            &[("D11-08", "/plan/scan_settings/categories")],
        ),
        neg(
            "D11-08",
            "log plan with an explicit target but no paths",
            Log,
            vec![set(
                "/runs/0/plan/targets/0/selector",
                json!({"mode": "explicit", "paths": [], "include_patterns": [], "exclude_patterns": []}),
            )],
            &[("D11-08", "/runs/0/plan/targets/0/selector/paths")],
        ),
    ]
}

fn cases_schedule() -> Vec<Case> {
    use StoreKind::Workflows;
    vec![
        neg(
            "D11-09",
            "manual with an interval",
            Workflows,
            vec![set("/workflows/1/schedule/interval_seconds", json!(60))],
            &[("D11-09", "/workflows/1/schedule/interval_seconds")],
        ),
        neg(
            "D11-09",
            "manual with a daily time",
            Workflows,
            vec![set(
                "/workflows/1/schedule/daily_local_time",
                json!("10:00"),
            )],
            &[("D11-09", "/workflows/1/schedule/daily_local_time")],
        ),
        neg(
            "D11-09",
            "manual with a next due time",
            Workflows,
            vec![set("/workflows/1/schedule/next_due_at", json!(T3))],
            &[("D11-09", "/workflows/1/schedule/next_due_at")],
        ),
        neg(
            "D11-09",
            "interval of zero",
            Workflows,
            vec![set("/workflows/2/schedule/interval_seconds", json!(0))],
            &[("D11-09", "/workflows/2/schedule/interval_seconds")],
        ),
        neg(
            "D11-09",
            "interval without a period",
            Workflows,
            vec![set("/workflows/2/schedule/interval_seconds", Value::Null)],
            &[("D11-09", "/workflows/2/schedule/interval_seconds")],
        ),
        neg(
            "D11-09",
            "interval with a daily time",
            Workflows,
            vec![set(
                "/workflows/2/schedule/daily_local_time",
                json!("10:00"),
            )],
            &[("D11-09", "/workflows/2/schedule/daily_local_time")],
        ),
        pos(
            "D11-09",
            "interval with a next due time",
            Workflows,
            vec![set("/workflows/2/schedule/next_due_at", json!(T3))],
        ),
        neg(
            "D11-09",
            "daily without a time",
            Workflows,
            vec![set("/workflows/0/schedule/daily_local_time", Value::Null)],
            &[("D11-09", "/workflows/0/schedule/daily_local_time")],
        ),
        neg(
            "D11-09",
            "daily with an interval",
            Workflows,
            vec![set("/workflows/0/schedule/interval_seconds", json!(60))],
            &[("D11-09", "/workflows/0/schedule/interval_seconds")],
        ),
    ]
}

fn cases_renderers() -> Vec<Case> {
    use StoreKind::{Log, Report, Workflows};
    let bad = |v: &'static str| -> Case {
        neg(
            "D11-11",
            v,
            Workflows,
            vec![set(
                "/workflows/0/scan_settings/renderers/0/parameters_redacted",
                json!({"release": v}),
            )],
            &[(
                "D11-11",
                "/workflows/0/scan_settings/renderers/0/parameters_redacted",
            )],
        )
    };
    vec![
        bad("a;b"),
        bad("$(id)"),
        bad("`id`"),
        bad("a|b"),
        bad("a&b"),
        bad("${HOME}"),
        pos(
            "D11-11",
            "plain parameters",
            Workflows,
            vec![set(
                "/workflows/0/scan_settings/renderers/0/parameters_redacted",
                json!({"release": "demo", "values": "a=b,c=d", "ns": "<U+0020>"}),
            )],
        ),
        neg(
            "D11-11",
            "report plan renderer",
            Report,
            vec![set(
                "/plan/scan_settings/renderers",
                json!([renderer(json!({"k": "a;b"}))]),
            )],
            &[(
                "D11-11",
                "/plan/scan_settings/renderers/0/parameters_redacted",
            )],
        ),
        neg(
            "D11-11",
            "log plan renderer",
            Log,
            vec![set(
                "/runs/0/plan/scan_settings/renderers",
                json!([renderer(json!({"k": "a;b"}))]),
            )],
            &[(
                "D11-11",
                "/runs/0/plan/scan_settings/renderers/0/parameters_redacted",
            )],
        ),
    ]
}

fn cases_approvals() -> Vec<Case> {
    use StoreKind::Approvals;
    vec![
        neg(
            "D11-12",
            "workflow-lifetime grant without a workflow",
            Approvals,
            vec![set("/approvals/4/workflow_id", Value::Null)],
            &[("D11-12", "/approvals/4/workflow_id")],
        ),
        neg(
            "D11-12",
            "workflow-lifetime grant without a revision",
            Approvals,
            vec![set("/approvals/4/workflow_revision", Value::Null)],
            &[("D11-12", "/approvals/4/workflow_revision")],
        ),
        neg(
            "D11-12",
            "workflow-lifetime grant with revision zero",
            Approvals,
            vec![set("/approvals/4/workflow_revision", json!(0))],
            &[("D11-12", "/approvals/4/workflow_revision")],
        ),
        neg(
            "D11-12",
            "run-lifetime grant without a run",
            Approvals,
            vec![set("/approvals/0/run_id", Value::Null)],
            &[("D11-12", "/approvals/0/run_id")],
        ),
        neg(
            "D11-12",
            "a revision without its workflow",
            Approvals,
            vec![set("/approvals/1/workflow_revision", json!(1))],
            &[("D11-12", "/approvals/1/workflow_id")],
        ),
        pos(
            "D11-12",
            "a once grant bound to a workflow and a run",
            Approvals,
            vec![
                set("/approvals/1/workflow_id", json!(uid(40))),
                set("/approvals/1/workflow_revision", json!(2)),
                set("/approvals/1/run_id", json!(uid(20))),
            ],
        ),
        neg(
            "D11-13",
            "pending request with a decision time",
            Approvals,
            vec![set("/approvals/3/decided_at", json!(T1))],
            &[("D11-13", "/approvals/3/decided_at")],
        ),
        neg(
            "D11-13",
            "pending request already consumed",
            Approvals,
            vec![set("/approvals/3/consumed", json!(true))],
            &[("D11-13", "/approvals/3/consumed")],
        ),
        neg(
            "D11-13",
            "approved without a decision time",
            Approvals,
            vec![set("/approvals/1/decided_at", Value::Null)],
            &[("D11-13", "/approvals/1/decided_at")],
        ),
        neg(
            "D11-13",
            "revoked without a decision time",
            Approvals,
            vec![
                set("/approvals/1/status", json!("revoked")),
                set("/approvals/1/decided_at", Value::Null),
            ],
            &[("D11-13", "/approvals/1/decided_at")],
        ),
        neg(
            "D11-13",
            "denied but consumed",
            Approvals,
            vec![set("/approvals/2/status", json!("denied"))],
            &[("D11-13", "/approvals/2/consumed")],
        ),
        neg(
            "D11-13",
            "cancelled but consumed",
            Approvals,
            vec![set("/approvals/2/status", json!("cancelled"))],
            &[("D11-13", "/approvals/2/consumed")],
        ),
        neg(
            "D11-13",
            "run-lifetime grant marked consumed",
            Approvals,
            vec![set("/approvals/0/consumed", json!(true))],
            &[("D11-13", "/approvals/0/consumed")],
        ),
        pos(
            "D11-13",
            "an expired once grant stays consumed",
            Approvals,
            vec![set("/approvals/2/status", json!("expired"))],
        ),
        pos(
            "D11-13",
            "a revoked grant, a denied request, a cancelled request",
            Approvals,
            vec![
                set("/approvals/1/status", json!("revoked")),
                set("/approvals/3/status", json!("cancelled")),
                set("/approvals/4/status", json!("denied")),
            ],
        ),
        neg(
            "D11-16",
            "model transmission without data categories",
            Approvals,
            vec![set("/approvals/0/data_categories", json!([]))],
            &[("D11-16", "/approvals/0/data_categories")],
        ),
        pos(
            "D11-16",
            "another capability may disclose nothing",
            Approvals,
            vec![set("/approvals/1/data_categories", json!([]))],
        ),
        neg(
            "D11-17",
            "network grant without a destination",
            Approvals,
            vec![set("/approvals/1/destinations", json!([]))],
            &[("D11-17", "/approvals/1/destinations")],
        ),
        neg(
            "D11-17",
            "local grant with a destination",
            Approvals,
            vec![set("/approvals/3/destinations", json!(["example.invalid"]))],
            &[("D11-17", "/approvals/3/destinations")],
        ),
        neg(
            "D11-17",
            "destination repeated",
            Approvals,
            vec![set(
                "/approvals/1/destinations",
                json!(["github.com", "github.com"]),
            )],
            &[("D11-17", "/approvals/1/destinations/1")],
        ),
        pos(
            "D11-17",
            "a local render with no destination (base)",
            Approvals,
            vec![],
        ),
        neg(
            "D18-10",
            "model transmission without a scope",
            Approvals,
            vec![set("/approvals/0/inference_scope", Value::Null)],
            &[("D18-10", "/approvals/0/inference_scope")],
        ),
        neg(
            "D18-10",
            "another capability with a scope",
            Approvals,
            vec![set("/approvals/1/inference_scope", scope_json())],
            &[("D18-10", "/approvals/1/inference_scope")],
        ),
        neg(
            "D18-10",
            "scope lists a source twice",
            Approvals,
            vec![set(
                "/approvals/0/inference_scope/source_ids",
                json!([uid(80), uid(80)]),
            )],
            &[("D18-10", "/approvals/0/inference_scope/source_ids/1")],
        ),
        pos(
            "D18-10",
            "a scope without sources",
            Approvals,
            vec![set("/approvals/0/inference_scope/source_ids", json!([]))],
        ),
    ]
}

fn cases_deferred() -> Vec<Case> {
    use StoreKind::{Approvals, Log, Report, Workflows};
    let deferred = |cap: &'static str| -> Case {
        neg(
            "D18-06a",
            cap,
            Approvals,
            vec![set("/approvals/1/capability", json!(cap))],
            &[("D18-06a", "/approvals/1/capability")],
        )
    };
    vec![
        neg(
            "D18-06a",
            "history secret scope in a workflow",
            Workflows,
            vec![set(
                "/workflows/0/scan_settings/secret_scope",
                json!("history"),
            )],
            &[("D18-06a", "/workflows/0/scan_settings/secret_scope")],
        ),
        neg(
            "D18-06a",
            "history secret scope in a report plan",
            Report,
            vec![set("/plan/scan_settings/secret_scope", json!("history"))],
            &[("D18-06a", "/plan/scan_settings/secret_scope")],
        ),
        neg(
            "D18-06a",
            "history secret scope in a log plan",
            Log,
            vec![set(
                "/runs/1/plan/scan_settings/secret_scope",
                json!("history"),
            )],
            &[("D18-06a", "/runs/1/plan/scan_settings/secret_scope")],
        ),
        deferred("documentation_fetch"),
        deferred("embedding_transmission"),
        deferred("non_mutating_verification"),
        neg(
            "D18-06a",
            "a denied request for a deferred capability is still rejected",
            Approvals,
            vec![
                set("/approvals/1/capability", json!("documentation_fetch")),
                set("/approvals/1/status", json!("denied")),
            ],
            &[("D18-06a", "/approvals/1/capability")],
        ),
        pos(
            "D18-06a",
            "checkout scope and a supported capability (base)",
            Workflows,
            vec![],
        ),
    ]
}

fn cases_time() -> Vec<Case> {
    use StoreKind::{
        Approvals, Conversations, Log, RagIndex, Report, Repositories, Suppressions, Workflows,
    };
    vec![
        neg(
            "D11-18",
            "repository updated before registration",
            Repositories,
            vec![set("/repositories/0/last_update_at", json!(EARLY))],
            &[("D11-18", "/repositories/0/last_update_at")],
        ),
        neg(
            "D11-18",
            "inventory before registration",
            Repositories,
            vec![set("/repositories/0/inventory_at", json!(EARLY))],
            &[("D11-18", "/repositories/0/inventory_at")],
        ),
        neg(
            "D11-18",
            "workflow updated before creation",
            Workflows,
            vec![set("/workflows/0/updated_at", json!(EARLY))],
            &[("D11-18", "/workflows/0/updated_at")],
        ),
        neg(
            "D11-18",
            "decision before request",
            Approvals,
            vec![set("/approvals/1/decided_at", json!(EARLY))],
            &[("D11-18", "/approvals/1/decided_at")],
        ),
        neg(
            "D11-18",
            "expiry before request",
            Approvals,
            vec![set("/approvals/1/expires_at", json!(EARLY))],
            &[("D11-18", "/approvals/1/expires_at")],
        ),
        neg(
            "D11-18",
            "expiry between request and decision",
            Approvals,
            vec![set(
                "/approvals/1/expires_at",
                json!("2026-10-16T05:25:30Z"),
            )],
            &[("D11-18", "/approvals/1/expires_at")],
        ),
        pos(
            "D11-18",
            "decision and expiry at the same instant",
            Approvals,
            vec![set("/approvals/1/expires_at", json!(T1))],
        ),
        neg(
            "D11-18",
            "run started before it was created",
            Log,
            vec![set("/runs/0/started_at", json!(EARLY))],
            &[("D11-18", "/runs/0/started_at")],
        ),
        neg(
            "D11-18",
            "run finished before it started",
            Log,
            vec![set("/runs/0/finished_at", json!(T0))],
            &[("D11-18", "/runs/0/finished_at")],
        ),
        neg(
            "D11-18",
            "checkpoint before creation",
            Log,
            vec![set("/runs/0/last_checkpoint_at", json!(EARLY))],
            &[("D11-18", "/runs/0/last_checkpoint_at")],
        ),
        neg(
            "D11-18",
            "queued run with a start time",
            Log,
            vec![set("/runs/1/started_at", json!(T1))],
            &[("D11-18", "/runs/1/started_at")],
        ),
        neg(
            "D11-18",
            "completed run without a finish time",
            Log,
            vec![set("/runs/0/finished_at", Value::Null)],
            &[("D11-18", "/runs/0/finished_at")],
        ),
        neg(
            "D11-18",
            "completed run without a start time",
            Log,
            vec![set("/runs/0/started_at", Value::Null)],
            &[("D11-18", "/runs/0/started_at")],
        ),
        neg(
            "D11-18",
            "running run that has finished",
            Log,
            vec![set("/runs/0/execution", json!("running"))],
            &[("D11-18", "/runs/0/finished_at")],
        ),
        neg(
            "D11-18",
            "completed run that never started",
            Log,
            vec![set("/runs/1/execution", json!("completed"))],
            &[
                ("D11-18", "/runs/1/started_at"),
                ("D11-18", "/runs/1/finished_at"),
            ],
        ),
        pos(
            "D11-18",
            "an interrupted run may have no finish time",
            Log,
            vec![
                set("/runs/0/execution", json!("interrupted")),
                set("/runs/0/finished_at", Value::Null),
            ],
        ),
        pos(
            "D11-18",
            "a failed run that never started",
            Log,
            vec![
                set("/runs/1/execution", json!("failed")),
                set("/runs/1/finished_at", json!(T1)),
            ],
        ),
        pos(
            "D11-18",
            "a running run and an awaiting run",
            Log,
            vec![
                set("/runs/0/execution", json!("running")),
                set("/runs/0/finished_at", Value::Null),
                set("/runs/1/execution", json!("awaiting_approval")),
            ],
        ),
        neg(
            "D11-18",
            "report generated before its plan",
            Report,
            vec![set("/plan/created_at", json!("2026-10-16T05:40:00Z"))],
            &[("D11-18", "/generated_at")],
        ),
        neg(
            "D11-18",
            "check finished before it started",
            Report,
            vec![set("/checks/0/finished_at", json!(EARLY))],
            &[("D11-18", "/checks/0/finished_at")],
        ),
        neg(
            "D11-18",
            "finding observed after the report",
            Report,
            vec![set("/findings/0/observed_at", json!(LATE))],
            &[("D11-18", "/findings/0/observed_at")],
        ),
        pos(
            "D11-18",
            "a skipped check with neither start nor finish",
            Report,
            vec![
                set("/checks/1/started_at", Value::Null),
                set("/checks/1/finished_at", Value::Null),
            ],
        ),
        neg(
            "D11-18",
            "chunk indexed before retrieval",
            RagIndex,
            vec![set("/chunks/0/indexed_at", json!(EARLY))],
            &[("D11-18", "/chunks/0/indexed_at")],
        ),
        neg(
            "D11-18",
            "conversation updated before creation",
            Conversations,
            vec![set("/conversations/0/updated_at", json!(EARLY))],
            &[
                ("D11-18", "/conversations/0/updated_at"),
                ("D11-18", "/conversations/0/messages/0/created_at"),
                ("D11-18", "/conversations/0/messages/1/created_at"),
            ],
        ),
        neg(
            "D11-18",
            "messages out of order",
            Conversations,
            vec![set("/conversations/0/messages/1/created_at", json!(T0))],
            &[("D11-18", "/conversations/0/messages/1/created_at")],
        ),
        pos(
            "D11-18",
            "messages at the same instant",
            Conversations,
            vec![set("/conversations/0/messages/1/created_at", json!(T1))],
        ),
        neg(
            "D11-18",
            "suppression that expires before it was created",
            Suppressions,
            vec![set("/suppressions/0/expires_at", json!(EARLY))],
            &[("D11-18", "/suppressions/0/expires_at")],
        ),
    ]
}

fn cases_digest() -> Vec<Case> {
    use StoreKind::RagIndex;
    vec![
        neg(
            "D11-19",
            "digest of other content",
            RagIndex,
            vec![set("/chunks/0/content_digest", json!(DIGEST))],
            &[("D11-19", "/chunks/0/content_digest")],
        ),
        neg(
            "D11-19",
            "digest of the empty text",
            RagIndex,
            vec![set("/chunks/0/content_digest", json!(EMPTY_SHA256))],
            &[("D11-19", "/chunks/0/content_digest")],
        ),
        neg(
            "D11-19",
            "digest of the raw text, not the persisted escaped text",
            RagIndex,
            vec![
                set("/chunks/0/text_redacted", json!("a\u{1b}b")),
                set("/chunks/0/content_digest", json!(SHA_RAW_ESC)),
            ],
            &[("D11-19", "/chunks/0/content_digest")],
        ),
        pos(
            "D11-19",
            "digest of the persisted escaped text",
            RagIndex,
            vec![
                set("/chunks/0/text_redacted", json!("a\u{1b}b")),
                set("/chunks/0/content_digest", json!(SHA_ESCAPED)),
            ],
        ),
        pos(
            "D11-19",
            "an empty chunk text",
            RagIndex,
            vec![
                set("/chunks/0/text_redacted", json!("")),
                set("/chunks/0/content_digest", json!(EMPTY_SHA256)),
            ],
        ),
    ]
}

fn cases_lines() -> Vec<Case> {
    use StoreKind::{Report, Repositories};
    let line = |name: &'static str, edits: Vec<Edit>, ptr: &'static str| {
        neg("D11-20", name, Report, edits, &[("D11-20", ptr)])
    };
    vec![
        line(
            "line start zero",
            vec![set("/checks/1/source_refs/0/line_start", json!(0))],
            "/checks/1/source_refs/0/line_start",
        ),
        line(
            "end before start",
            vec![
                set("/checks/1/source_refs/0/line_start", json!(5)),
                set("/checks/1/source_refs/0/line_end", json!(4)),
            ],
            "/checks/1/source_refs/0/line_end",
        ),
        line(
            "end without start",
            vec![set("/checks/1/source_refs/0/line_end", json!(3))],
            "/checks/1/source_refs/0/line_end",
        ),
        line(
            "document index zero",
            vec![set("/checks/1/source_refs/0/document_index", json!(0))],
            "/checks/1/source_refs/0/document_index",
        ),
        pos(
            "D11-20",
            "a single line, start only, and no mapping",
            Report,
            vec![
                set("/checks/1/source_refs/0/line_start", json!(7)),
                set("/checks/1/source_refs/0/line_end", json!(7)),
                set("/checks/1/source_refs/0/document_index", json!(2)),
            ],
        ),
        pos(
            "D11-20",
            "a start line without an end",
            Report,
            vec![set("/checks/1/source_refs/0/line_start", json!(7))],
        ),
        neg(
            "D11-20",
            "completed report without a commit",
            Report,
            vec![set("/commit_oid", Value::Null)],
            &[("D11-20", "/commit_oid")],
        ),
        neg_min(
            "D11-20",
            "queued report with findings but no commit",
            vec![
                set("/findings", json!([finding_json()])),
                set("/coverage/assessment", json!("findings")),
            ],
            &[
                ("D11-20", "/commit_oid"),
                ("D11-04", "/findings/0/check_id"),
            ],
        ),
        pos_min("D11-20", "a queued report may have no commit", vec![]),
        pos(
            "D11-20",
            "a failed report without findings may have no commit",
            Report,
            vec![
                set("/commit_oid", Value::Null),
                set("/execution", json!("failed")),
                set("/findings", json!([])),
                set("/checks/0/finding_ids", json!([])),
                set("/checks/0/outcome", json!("failed")),
                set("/coverage/checks_with_findings", json!(0)),
                set("/coverage/checks_failed", json!(1)),
                set("/coverage/assessment", json!("unknown")),
                set("/ai_enrichment", ai_not_requested()),
            ],
        ),
        neg(
            "D11-20",
            "update time without a commit",
            Repositories,
            vec![set("/repositories/0/last_commit_oid", Value::Null)],
            &[("D11-20", "/repositories/0/last_commit_oid")],
        ),
        neg(
            "D11-20",
            "commit without an update time",
            Repositories,
            vec![set("/repositories/0/last_update_at", Value::Null)],
            &[("D11-20", "/repositories/0/last_update_at")],
        ),
        pos(
            "D11-20",
            "a repository that was never acquired",
            Repositories,
            vec![
                set("/repositories/0/last_update_at", Value::Null),
                set("/repositories/0/last_commit_oid", Value::Null),
                set("/repositories/0/checkout_state", json!("not_cloned")),
            ],
        ),
    ]
}

fn cases_coverage() -> Vec<Case> {
    use StoreKind::{Log, Report};
    let counter = |name: &'static str, field: &'static str, ptr: &'static str, v: u64| {
        neg(
            "D11-21",
            name,
            Report,
            vec![set(field, json!(v))],
            &[("D11-21", ptr)],
        )
    };
    vec![
        counter(
            "passed counter",
            "/coverage/checks_passed",
            "/coverage/checks_passed",
            2,
        ),
        counter(
            "findings counter",
            "/coverage/checks_with_findings",
            "/coverage/checks_with_findings",
            0,
        ),
        counter(
            "failed counter",
            "/coverage/checks_failed",
            "/coverage/checks_failed",
            1,
        ),
        counter(
            "unsupported counter",
            "/coverage/checks_unsupported",
            "/coverage/checks_unsupported",
            1,
        ),
        counter(
            "skipped counter",
            "/coverage/checks_skipped",
            "/coverage/checks_skipped",
            1,
        ),
        counter(
            "incomplete counter",
            "/coverage/checks_incomplete",
            "/coverage/checks_incomplete",
            5,
        ),
        counter(
            "cancelled counter",
            "/coverage/checks_cancelled",
            "/coverage/checks_cancelled",
            1,
        ),
        counter(
            "selected files against the snapshot",
            "/coverage/selected_files",
            "/coverage/selected_files",
            2,
        ),
        counter(
            "missing files against the snapshot",
            "/coverage/missing_files",
            "/coverage/missing_files",
            1,
        ),
        counter(
            "assessed more than selected",
            "/coverage/assessed_files",
            "/coverage/assessed_files",
            2,
        ),
        neg(
            "D11-21",
            "no_findings with a finding and a check with findings",
            Report,
            vec![set("/coverage/assessment", json!("no_findings"))],
            &[("D11-21", "/coverage/assessment")],
        ),
        neg_min(
            "D11-21",
            "findings assessment without any finding",
            vec![set("/coverage/assessment", json!("findings"))],
            &[("D11-21", "/coverage/assessment")],
        ),
        pos_min(
            "D11-21",
            "a pending check is counted nowhere",
            vec![push(
                "/checks",
                check_json(
                    50,
                    "yamllint",
                    json!({
                        "outcome": "pending", "started_at": Value::Null, "finished_at": Value::Null, "exit_code": Value::Null,
                        "native_status": "not_started", "manifest_digest": Value::Null
                    }),
                ),
            )],
        ),
        pos(
            "D11-21",
            "no snapshot for the reference means no snapshot comparison",
            Report,
            vec![
                set("/plan/target_snapshots/0/selected_ref", json!("other")),
                set("/coverage/selected_files", json!(9)),
                set("/coverage/missing_files", json!(4)),
            ],
        ),
        neg(
            "D11-21",
            "log run assessed more than selected",
            Log,
            vec![set("/runs/0/coverage/assessed_files", json!(5))],
            &[("D11-21", "/runs/0/coverage/assessed_files")],
        ),
        neg(
            "D11-21",
            "log run no_findings with a check with findings",
            Log,
            vec![set("/runs/0/coverage/assessment", json!("no_findings"))],
            &[("D11-21", "/runs/0/coverage/assessment")],
        ),
    ]
}

fn cases_limits() -> Vec<Case> {
    use StoreKind::{Config, Log};
    let bounded = |name: &'static str, field: &'static str, v: u64, bad: bool| {
        let expect = if bad {
            vec![("D11-25", field)]
        } else {
            Vec::new()
        };
        Case {
            rule: "D11-25",
            name,
            kind: Config,
            base: config_doc,
            edits: vec![set(field, json!(v))],
            expect,
        }
    };
    let proc = |name: &'static str, v: u64, bad: bool| {
        bounded(name, "/config/max_scanner_processes", v, bad)
    };
    let repos = |name: &'static str, v: u64, bad: bool| {
        bounded(name, "/config/max_active_repositories", v, bad)
    };
    let tool = |name: &'static str, field: &'static str, v: u64, bad: bool| {
        let expect = if bad {
            vec![("D18-01", field)]
        } else {
            Vec::new()
        };
        Case {
            rule: "D18-01",
            name,
            kind: StoreKind::ToolManifest,
            base: tools_doc,
            edits: vec![set(field, json!(v))],
            expect,
        }
    };
    vec![
        proc("scanner processes 1", 1, false),
        proc("scanner processes 64", 64, false),
        proc("scanner processes 0", 0, true),
        proc("scanner processes 65", 65, true),
        repos("active repositories 1", 1, false),
        repos("active repositories 1024", 1024, false),
        repos("active repositories 0", 0, true),
        repos("active repositories 1025", 1025, true),
        neg(
            "D11-25",
            "retention of zero days",
            Config,
            vec![set("/config/history_retention_days", json!(0))],
            &[("D11-25", "/config/history_retention_days")],
        ),
        pos(
            "D11-25",
            "one day and unlimited retention",
            Config,
            vec![set("/config/history_retention_days", json!(1))],
        ),
        pos(
            "D11-25",
            "unlimited retention",
            Config,
            vec![set("/config/history_retention_days", Value::Null)],
        ),
        tool("tool timeout 1 s", "/tools/0/timeout_seconds", 1, false),
        tool(
            "tool timeout 86400 s",
            "/tools/0/timeout_seconds",
            86_400,
            false,
        ),
        tool("tool timeout 0 s", "/tools/0/timeout_seconds", 0, true),
        tool(
            "tool timeout 86401 s",
            "/tools/0/timeout_seconds",
            86_401,
            true,
        ),
        tool("tool output 1 byte", "/tools/0/max_output_bytes", 1, false),
        tool(
            "tool output 64 MiB",
            "/tools/0/max_output_bytes",
            67_108_864,
            false,
        ),
        tool("tool output 0 bytes", "/tools/0/max_output_bytes", 0, true),
        tool(
            "tool output 64 MiB + 1",
            "/tools/0/max_output_bytes",
            67_108_865,
            true,
        ),
        neg(
            "D11-25",
            "storage failure logged as information",
            Log,
            vec![set("/events/1/level", json!("info"))],
            &[("D11-25", "/events/1/level")],
        ),
        neg(
            "D11-25",
            "operation failure logged as a warning",
            Log,
            vec![
                set("/events/0/event_type", json!("OperationFailed")),
                set("/events/0/level", json!("warning")),
            ],
            &[("D11-25", "/events/0/level")],
        ),
        pos(
            "D11-25",
            "an informational event and an error failure (base)",
            Log,
            vec![],
        ),
    ]
}

fn cases_tools() -> Vec<Case> {
    use StoreKind::{Log, ToolManifest};
    let zeros = ZERO_DIGEST;
    vec![
        neg(
            "D18-01",
            "bundled tool without a binary digest",
            ToolManifest,
            vec![set("/tools/0/binary_sha256", Value::Null)],
            &[("D18-01", "/tools/0/binary_sha256")],
        ),
        neg(
            "D18-01",
            "ready tool without a version",
            ToolManifest,
            vec![set("/tools/0/tool_version", Value::Null)],
            &[
                ("D18-01", "/tools/0/tool_version"),
                ("D18-01", "/tools/0/license/verified_version"),
            ],
        ),
        neg(
            "D18-01",
            "ready tool with a placeholder digest",
            ToolManifest,
            vec![set("/tools/0/binary_sha256", json!(zeros))],
            &[("D18-01", "/tools/0/binary_sha256")],
        ),
        neg(
            "D18-01",
            "ready tool that is not shipped",
            ToolManifest,
            vec![set("/tools/0/distribution", json!("not_shipped"))],
            &[("D18-01", "/tools/0/distribution")],
        ),
        neg(
            "D18-01",
            "ready tool without an exit mapping",
            ToolManifest,
            vec![set("/tools/0/exit_mapping", json!([]))],
            &[("D18-01", "/tools/0/exit_mapping")],
        ),
        neg(
            "D18-01",
            "ready tool with a pending licence review",
            ToolManifest,
            vec![set("/tools/0/license/review_status", json!("pending"))],
            &[("D18-01", "/tools/0/license/review_status")],
        ),
        neg(
            "D18-01",
            "licence verified for another version",
            ToolManifest,
            vec![set("/tools/0/license/verified_version", json!("1.0.0"))],
            &[("D18-01", "/tools/0/license/verified_version")],
        ),
        neg(
            "D18-01",
            "missing tool with an invented version",
            ToolManifest,
            vec![set("/tools/1/tool_version", json!("9.9.9"))],
            &[("D18-01", "/tools/1/tool_version")],
        ),
        neg(
            "D18-01",
            "deferred tool with an invented version",
            ToolManifest,
            vec![
                set("/tools/1/availability", json!("deferred")),
                set("/tools/1/tool_version", json!("9.9.9")),
            ],
            &[("D18-01", "/tools/1/tool_version")],
        ),
        neg(
            "D18-01",
            "approved licence without an SPDX expression",
            ToolManifest,
            vec![set("/tools/0/license/spdx", Value::Null)],
            &[("D18-01", "/tools/0/license/spdx")],
        ),
        neg(
            "D18-01",
            "approved licence without a review time",
            ToolManifest,
            vec![set("/tools/0/license/reviewed_at", Value::Null)],
            &[("D18-01", "/tools/0/license/reviewed_at")],
        ),
        neg(
            "D18-01",
            "engine snapshot that is missing but has a version",
            Log,
            vec![set(
                "/runs/0/plan/engine_snapshots/0/availability",
                json!("missing"),
            )],
            &[("D18-01", "/runs/0/plan/engine_snapshots/0/tool_version")],
        ),
        neg(
            "D18-01",
            "engine snapshot that is ready without a version",
            Log,
            vec![set(
                "/runs/0/plan/engine_snapshots/0/tool_version",
                Value::Null,
            )],
            &[("D18-01", "/runs/0/plan/engine_snapshots/0/tool_version")],
        ),
        pos(
            "D18-01",
            "a ready system prerequisite with a recorded identity",
            ToolManifest,
            vec![set("/tools/0/distribution", json!("system_prerequisite"))],
        ),
        pos(
            "D18-01",
            "an incompatible tool keeps its installed version",
            ToolManifest,
            vec![
                set("/tools/0/availability", json!("incompatible")),
                set("/tools/0/license/review_status", json!("pending")),
            ],
        ),
        neg(
            "D18-03",
            "network policy none with resources",
            ToolManifest,
            vec![set(
                "/tools/0/network_resources",
                json!(["example.invalid"]),
            )],
            &[("D18-03", "/tools/0/network_resources")],
        ),
        neg(
            "D18-03",
            "resource listed twice",
            ToolManifest,
            vec![set(
                "/tools/1/network_resources",
                json!(["a.example", "a.example"]),
            )],
            &[("D18-03", "/tools/1/network_resources/1")],
        ),
        pos(
            "D18-03",
            "approval-required policy with resources (base tool 1)",
            ToolManifest,
            vec![],
        ),
        neg(
            "D18-04",
            "exit code mapped twice",
            ToolManifest,
            vec![push(
                "/tools/0/exit_mapping",
                json!({"code": 1, "meaning": "execution_error"}),
            )],
            &[("D18-04", "/tools/0/exit_mapping/3/code")],
        ),
    ]
}

fn cases_checks() -> Vec<Case> {
    use StoreKind::Report;
    vec![
        neg(
            "D18-04",
            "check adapter differs from its engine",
            Report,
            vec![set("/checks/0/adapter_id", json!("gitleaks"))],
            &[("D18-04", "/checks/0/adapter_id")],
        ),
        neg(
            "D18-04",
            "signal zero",
            Report,
            vec![set("/checks/2/term_signal", json!(0))],
            &[("D18-04", "/checks/2/term_signal")],
        ),
        neg(
            "D18-04",
            "exit code and signal together",
            Report,
            vec![set("/checks/0/term_signal", json!(9))],
            &[("D18-04", "/checks/0/term_signal")],
        ),
        neg(
            "D18-04",
            "not started tool with an exit code",
            Report,
            vec![set("/checks/0/native_status", json!("not_started"))],
            &[("D18-04", "/checks/0/exit_code")],
        ),
        neg(
            "D18-04",
            "launch failure with a signal",
            Report,
            vec![set("/checks/2/native_status", json!("launch_failed"))],
            &[("D18-04", "/checks/2/term_signal")],
        ),
        neg(
            "D18-04",
            "exited without an exit code",
            Report,
            vec![set("/checks/0/exit_code", Value::Null)],
            &[("D18-04", "/checks/0/exit_code")],
        ),
        neg(
            "D18-04",
            "passed check that timed out",
            Report,
            vec![set("/checks/1/native_status", json!("timed_out"))],
            &[("D18-04", "/checks/1/native_status")],
        ),
        pos(
            "D18-04",
            "an in-process adapter has no exit code",
            Report,
            vec![
                set("/checks/1/adapter_id", json!("rust-schema")),
                set("/checks/1/engine/adapter_id", json!("rust-schema")),
                set("/checks/1/engine/tool_name", json!("rust-schema")),
                set("/checks/1/exit_code", Value::Null),
            ],
        ),
        pos(
            "D18-04",
            "a timed-out tool killed by a signal (base check 52)",
            Report,
            vec![],
        ),
    ]
}

fn cases_selection() -> Vec<Case> {
    use StoreKind::{Report, Repositories};
    vec![
        neg(
            "D18-07",
            "eligible file without a digest",
            Repositories,
            vec![set("/repositories/0/files/0/content_digest", Value::Null)],
            &[("D18-07", "/repositories/0/files/0/content_digest")],
        ),
        neg(
            "D18-07",
            "unreadable file with a digest",
            Repositories,
            vec![set("/repositories/0/files/1/content_digest", json!(DIGEST))],
            &[("D18-07", "/repositories/0/files/1/content_digest")],
        ),
        pos(
            "D18-07",
            "an excluded file with or without a digest",
            Repositories,
            vec![set(
                "/repositories/0/files/1/discovery_state",
                json!("excluded"),
            )],
        ),
        neg(
            "D18-07",
            "selected file that is not eligible",
            Report,
            vec![set(
                "/plan/target_snapshots/0/selected_files/0/discovery_state",
                json!("excluded"),
            )],
            &[(
                "D18-07",
                "/plan/target_snapshots/0/selected_files/0/discovery_state",
            )],
        ),
        neg(
            "D18-07",
            "selected path also listed as missing",
            Report,
            vec![set(
                "/plan/target_snapshots/0/missing_paths",
                json!(["deploy/app.yaml"]),
            )],
            &[
                ("D18-07", "/plan/target_snapshots/0/selected_files/0/path"),
                ("D11-21", "/coverage/missing_files"),
            ],
        ),
        neg(
            "D18-07",
            "selected path listed twice",
            Report,
            vec![push(
                "/plan/target_snapshots/0/selected_files",
                file_json(32, "deploy/app.yaml", "eligible", json!(DIGEST)),
            )],
            &[
                ("D18-07", "/plan/target_snapshots/0/selected_files/1/path"),
                ("D11-21", "/coverage/selected_files"),
            ],
        ),
        neg(
            "D18-07",
            "rendered artifact without its sources",
            Report,
            vec![set("/artifacts/0/source_refs", json!([]))],
            &[("D18-07", "/artifacts/0/source_refs")],
        ),
        pos(
            "D18-07",
            "an external artifact needs no source (base artifact 91)",
            Report,
            vec![],
        ),
        neg(
            "D18-08",
            "retained content not marked redacted",
            Report,
            vec![set(
                "/artifacts/0/contains_only_redacted_retained_content",
                json!(false),
            )],
            &[(
                "D18-08",
                "/artifacts/0/contains_only_redacted_retained_content",
            )],
        ),
        pos(
            "D18-08",
            "nothing retained, flag false",
            Report,
            vec![
                set("/artifacts/0/retained_path", Value::Null),
                set(
                    "/artifacts/0/contains_only_redacted_retained_content",
                    json!(false),
                ),
            ],
        ),
    ]
}

fn cases_ai() -> Vec<Case> {
    use StoreKind::Config;
    let os_mode = |reference: Value| -> Vec<Edit> {
        vec![
            set(
                "/config/provider/credential_mode",
                json!("os_credential_store"),
            ),
            set("/config/provider/credential_reference", reference),
        ]
    };
    vec![
        neg(
            "D11-23",
            "OS credential store mode without a reference",
            Config,
            os_mode(Value::Null),
            &[("D11-23", "/config/provider/credential_reference")],
        ),
        pos(
            "D11-23",
            "OS credential store mode with a locator",
            Config,
            os_mode(json!("os-keyring:pwt-openrouter")),
        ),
        pos(
            "D11-23",
            "no provider configured",
            Config,
            vec![set("/config/provider", Value::Null)],
        ),
        neg(
            "D18-09",
            "session-only mode with a stored reference",
            Config,
            vec![set(
                "/config/provider/credential_reference",
                json!("os-keyring:pwt-openrouter"),
            )],
            &[("D18-09", "/config/provider/credential_reference")],
        ),
        pos(
            "D18-09",
            "session-only mode with no reference (base)",
            Config,
            vec![],
        ),
        neg(
            "D11-24",
            "chat persistence enabled in the pilot",
            Config,
            vec![set("/config/chat_persistence_enabled", json!(true))],
            &[("D11-24", "/config/chat_persistence_enabled")],
        ),
        neg(
            "D18-11",
            "provider allowlist repeated in the configuration",
            Config,
            vec![set(
                "/config/provider/routing_policy/only",
                json!(["a-provider", "a-provider"]),
            )],
            &[("D18-11", "/config/provider/routing_policy/only/1")],
        ),
        pos(
            "D18-11",
            "an empty provider allowlist",
            Config,
            vec![set("/config/provider/routing_policy/only", json!([]))],
        ),
    ]
}

/// The pointer of a field of message `$n` of the first conversation.
macro_rules! msg {
    ($n:literal, $f:literal) => {
        concat!("/conversations/0/messages/", $n, "/", $f)
    };
}

fn cases_chat() -> Vec<Case> {
    use StoreKind::{Approvals, Conversations, Log, RagIndex, Report};
    vec![
        neg(
            "D11-24",
            "chunk with an embedding key",
            RagIndex,
            vec![set("/chunks/0/embedding_key", json!("emb-1"))],
            &[("D11-24", "/chunks/0/embedding_key")],
        ),
        neg(
            "D11-24",
            "chat event in the run log",
            Log,
            vec![set("/events/0/event_type", json!("ChatFailed"))],
            &[("D11-24", "/events/0/event_type")],
        ),
        pos("D11-24", "a run event in the run log (base)", Log, vec![]),
        neg(
            "D11-24",
            "user message with a provider",
            Conversations,
            vec![set(msg!(0, "provider_id"), json!("openrouter"))],
            &[("D11-24", msg!(0, "provider_id"))],
        ),
        neg(
            "D11-24",
            "user message with a model",
            Conversations,
            vec![set(msg!(0, "model_id"), json!("example/model-1"))],
            &[("D11-24", msg!(0, "model_id"))],
        ),
        neg(
            "D11-24",
            "user message with token counts",
            Conversations,
            vec![
                set(msg!(0, "tokens_in"), json!(1)),
                set(msg!(0, "tokens_out"), json!(2)),
            ],
            &[
                ("D11-24", msg!(0, "tokens_in")),
                ("D11-24", msg!(0, "tokens_out")),
            ],
        ),
        neg(
            "D11-24",
            "user message with approvals",
            Conversations,
            vec![set(msg!(0, "approval_ids"), json!([uid(100)]))],
            &[("D11-24", msg!(0, "approval_ids"))],
        ),
        neg(
            "D11-24",
            "assistant message with a provider but no approvals",
            Conversations,
            vec![set(msg!(1, "approval_ids"), json!([]))],
            &[("D11-24", msg!(1, "approval_ids"))],
        ),
        neg(
            "D11-24",
            "message citing a repository outside its context",
            Conversations,
            vec![set(
                "/conversations/0/messages/1/source_refs/0/repo_id",
                json!(uid(11)),
            )],
            &[(
                "D11-24",
                "/conversations/0/messages/1/source_refs/0/repo_id",
            )],
        ),
        pos(
            "D11-24",
            "a system notice and an assistant message without a provider",
            Conversations,
            vec![
                push(
                    "/conversations/0/messages",
                    message_json(152, "system_notice", json!({"created_at": T2})),
                ),
                set(msg!(1, "provider_id"), Value::Null),
                set(msg!(1, "model_id"), Value::Null),
                set(msg!(1, "approval_ids"), json!([])),
            ],
        ),
        neg(
            "D18-11",
            "approval scope allowlist repeated",
            Approvals,
            vec![set(
                "/approvals/0/inference_scope/routing_policy/only",
                json!(["a-provider", "a-provider"]),
            )],
            &[(
                "D18-11",
                "/approvals/0/inference_scope/routing_policy/only/1",
            )],
        ),
        neg(
            "D18-11",
            "enrichment allowlist repeated",
            Report,
            vec![set(
                "/ai_enrichment/routing_policy/only",
                json!(["a-provider", "a-provider"]),
            )],
            &[("D18-11", "/ai_enrichment/routing_policy/only/1")],
        ),
        neg(
            "D18-11",
            "enrichment names a provider but no policy",
            Report,
            vec![set("/ai_enrichment/routing_policy", Value::Null)],
            &[
                ("D18-11", "/ai_enrichment/routing_policy"),
                ("D18-12", "/ai_enrichment/routing_policy"),
            ],
        ),
        neg(
            "D18-11",
            "enrichment has a policy but no provider",
            Report,
            vec![set("/ai_enrichment/provider_id", Value::Null)],
            &[
                ("D18-11", "/ai_enrichment/routing_policy"),
                ("D18-11", "/ai_enrichment/provider_id"),
                ("D18-12", "/ai_enrichment/provider_id"),
            ],
        ),
    ]
}

fn cases_enrichment() -> Vec<Case> {
    let f = |name: &'static str, edits: Vec<Edit>, ptr: &'static str| {
        neg("D18-12", name, StoreKind::Report, edits, &[("D18-12", ptr)])
    };
    vec![
        f(
            "completed without a narrative",
            vec![set("/ai_enrichment/narrative_redacted", json!(""))],
            "/ai_enrichment/narrative_redacted",
        ),
        f(
            "completed without a generation time",
            vec![set("/ai_enrichment/generated_at", Value::Null)],
            "/ai_enrichment/generated_at",
        ),
        f(
            "completed without approvals",
            vec![set("/ai_enrichment/approval_ids", json!([]))],
            "/ai_enrichment/approval_ids",
        ),
        f(
            "completed without cited sources",
            vec![set("/ai_enrichment/source_ids", json!([]))],
            "/ai_enrichment/source_ids",
        ),
        f(
            "completed with a failure reason",
            vec![set("/ai_enrichment/reason_redacted", json!("Odd."))],
            "/ai_enrichment/reason_redacted",
        ),
        f(
            "completed without a requested model",
            vec![set("/ai_enrichment/requested_model", Value::Null)],
            "/ai_enrichment/requested_model",
        ),
        f(
            "cited source that is not in the report",
            vec![set("/ai_enrichment/source_ids", json!([uid(99)]))],
            "/ai_enrichment/source_ids/0",
        ),
        pos(
            "D18-12",
            "completed without actual provider and model",
            StoreKind::Report,
            vec![],
        ),
        pos(
            "D18-12",
            "completed with the actual provider and model",
            StoreKind::Report,
            vec![
                set("/ai_enrichment/actual_model", json!("example/model-1")),
                set("/ai_enrichment/actual_provider", json!("example-provider")),
            ],
        ),
        neg_min(
            "D18-12",
            "not requested with a narrative",
            vec![set("/ai_enrichment/narrative_redacted", json!("Text."))],
            &[("D18-12", "/ai_enrichment/narrative_redacted")],
        ),
        neg_min(
            "D18-12",
            "not requested with a generation time",
            vec![set("/ai_enrichment/generated_at", json!(T2))],
            &[("D18-12", "/ai_enrichment/generated_at")],
        ),
        neg_min(
            "D18-12",
            "not requested with an approval",
            vec![set("/ai_enrichment/approval_ids", json!([uid(100)]))],
            &[("D18-12", "/ai_enrichment/approval_ids")],
        ),
        neg_min(
            "D18-12",
            "not requested with an answering model",
            vec![set("/ai_enrichment/actual_model", json!("example/model-1"))],
            &[("D18-12", "/ai_enrichment/actual_model")],
        ),
        neg_min(
            "D18-12",
            "not requested with an answering provider",
            vec![set(
                "/ai_enrichment/actual_provider",
                json!("example-provider"),
            )],
            &[("D18-12", "/ai_enrichment/actual_provider")],
        ),
        neg_min(
            "D18-12",
            "failed without a reason",
            vec![set("/ai_enrichment/status", json!("failed"))],
            &[("D18-12", "/ai_enrichment/reason_redacted")],
        ),
        neg_min(
            "D18-12",
            "unavailable without a reason",
            vec![set("/ai_enrichment/status", json!("unavailable"))],
            &[("D18-12", "/ai_enrichment/reason_redacted")],
        ),
        neg_min(
            "D18-12",
            "denied without a reason",
            vec![set("/ai_enrichment/status", json!("denied"))],
            &[("D18-12", "/ai_enrichment/reason_redacted")],
        ),
        pos_min(
            "D18-12",
            "unavailable with a reason",
            vec![
                set("/ai_enrichment/status", json!("unavailable")),
                set("/ai_enrichment/reason_redacted", json!("No route.")),
            ],
        ),
        pos_min(
            "D18-12",
            "failed after a request, with the answering model",
            vec![
                set("/ai_enrichment/status", json!("failed")),
                set("/ai_enrichment/reason_redacted", json!("Provider outage.")),
                set("/ai_enrichment/provider_id", json!("openrouter")),
                set("/ai_enrichment/requested_model", json!("example/model-1")),
                set("/ai_enrichment/actual_model", json!("example/model-1")),
                set("/ai_enrichment/routing_policy", policy(&[])),
            ],
        ),
    ]
}

fn cases_export() -> Vec<Case> {
    use StoreKind::{Log, Report};
    let pdf = |name: &'static str, edits: Vec<Edit>, ptr: &'static str| {
        neg("D18-14", name, Log, edits, &[("D18-14", ptr)])
    };
    vec![
        pdf(
            "written json without a digest",
            vec![set("/reports/0/json_digest", Value::Null)],
            "/reports/0/json_digest",
        ),
        pdf(
            "unwritten json with a digest",
            vec![
                set("/reports/0/json_written", json!(false)),
                set("/reports/0/md_written", json!(false)),
                set("/reports/0/pdf_written", json!(false)),
                set("/reports/0/pdf_digest", Value::Null),
            ],
            "/reports/0/json_digest",
        ),
        pdf(
            "json digest of an empty file",
            vec![set("/reports/0/json_digest", json!(EMPTY_SHA256))],
            "/reports/0/json_digest",
        ),
        neg(
            "D18-14",
            "markdown and pdf written without the json",
            Log,
            vec![
                set("/reports/0/json_written", json!(false)),
                set("/reports/0/json_digest", Value::Null),
            ],
            &[
                ("D18-14", "/reports/0/md_written"),
                ("D18-14", "/reports/0/pdf_written"),
            ],
        ),
        pdf(
            "written pdf without a path",
            vec![set("/reports/0/pdf_path", Value::Null)],
            "/reports/0/pdf_path",
        ),
        pdf(
            "written pdf without a digest",
            vec![set("/reports/0/pdf_digest", Value::Null)],
            "/reports/0/pdf_digest",
        ),
        pdf(
            "written pdf of an empty file",
            vec![set("/reports/0/pdf_digest", json!(EMPTY_SHA256))],
            "/reports/0/pdf_digest",
        ),
        pdf(
            "written pdf with an unresolved error",
            vec![set(
                "/reports/0/pdf_error_redacted",
                json!("Conversion failed."),
            )],
            "/reports/0/pdf_error_redacted",
        ),
        pdf(
            "written pdf without a renderer version",
            vec![set("/reports/0/pdf_renderer_version", Value::Null)],
            "/reports/0/pdf_renderer_version",
        ),
        pdf(
            "unwritten pdf with a digest",
            vec![set("/reports/0/pdf_written", json!(false))],
            "/reports/0/pdf_digest",
        ),
        pos(
            "D18-14",
            "a failed pdf export keeps its error and no digest",
            Log,
            vec![
                set("/reports/0/pdf_written", json!(false)),
                set("/reports/0/pdf_digest", Value::Null),
                set("/reports/0/pdf_error_redacted", json!("Conversion failed.")),
            ],
        ),
        pos(
            "D18-14",
            "a json-only manifest",
            Log,
            vec![
                set("/reports/0/md_written", json!(false)),
                set("/reports/0/md_path", json!("backup/report-a.md")),
                set("/reports/0/pdf_path", Value::Null),
                set("/reports/0/pdf_written", json!(false)),
                set("/reports/0/pdf_digest", Value::Null),
                set("/reports/0/pdf_renderer_version", Value::Null),
            ],
        ),
        neg(
            "D18-18",
            "pending check in a completed report",
            Report,
            vec![set("/checks/1/outcome", json!("pending"))],
            &[
                ("D18-18", "/checks/1/outcome"),
                ("D11-21", "/coverage/checks_passed"),
            ],
        ),
        neg(
            "D18-18",
            "running check in a failed report",
            Report,
            vec![
                set("/execution", json!("failed")),
                set("/checks/1/outcome", json!("running")),
            ],
            &[
                ("D18-18", "/checks/1/outcome"),
                ("D11-21", "/coverage/checks_passed"),
            ],
        ),
        neg(
            "D18-18",
            "incomplete check without an explanation",
            Report,
            vec![set("/checks/2/reason_redacted", Value::Null)],
            &[("D18-18", "/checks/2/reason_redacted")],
        ),
        neg(
            "D18-18",
            "cancelled check with an empty explanation",
            Report,
            vec![
                set("/checks/2/outcome", json!("cancelled")),
                set("/checks/2/reason_redacted", json!("")),
                set("/coverage/checks_incomplete", json!(0)),
                set("/coverage/checks_cancelled", json!(1)),
            ],
            &[("D18-18", "/checks/2/reason_redacted")],
        ),
        pos_min(
            "D18-18",
            "a running check in a running report",
            vec![
                set("/execution", json!("running")),
                push(
                    "/checks",
                    check_json(
                        50,
                        "yamllint",
                        json!({
                            "outcome": "running", "finished_at": Value::Null, "exit_code": Value::Null,
                            "native_status": "not_started", "manifest_digest": Value::Null
                        }),
                    ),
                ),
            ],
        ),
    ]
}

fn all_cases() -> Vec<Case> {
    let mut all = Vec::new();
    all.extend(cases_ids());
    all.extend(cases_scope());
    all.extend(cases_findings());
    all.extend(cases_sources());
    all.extend(cases_paths());
    all.extend(cases_selectors());
    all.extend(cases_runnable());
    all.extend(cases_schedule());
    all.extend(cases_renderers());
    all.extend(cases_approvals());
    all.extend(cases_deferred());
    all.extend(cases_time());
    all.extend(cases_digest());
    all.extend(cases_lines());
    all.extend(cases_coverage());
    all.extend(cases_limits());
    all.extend(cases_tools());
    all.extend(cases_checks());
    all.extend(cases_selection());
    all.extend(cases_ai());
    all.extend(cases_chat());
    all.extend(cases_enrichment());
    all.extend(cases_export());
    all
}

#[test]
fn every_case_reports_exactly_what_it_states() {
    for c in all_cases() {
        run_case(&c);
    }
}

#[test]
fn every_checked_rule_has_a_positive_and_a_negative_case_and_no_case_names_an_unknown_rule() {
    let cases = all_cases();
    let known: BTreeSet<&str> = rule_table().map(|r| r.id.0).collect();
    for c in &cases {
        assert!(known.contains(c.rule), "{}: unknown rule", c.name);
        for (r, _) in &c.expect {
            assert!(known.contains(r), "{}: expects unknown rule {r}", c.name);
        }
    }
    for r in rule_table() {
        let checked = matches!(
            r.status,
            RuleStatus::Implemented | RuleStatus::Partial { .. }
        );
        let negative = cases
            .iter()
            .filter(|c| c.rule == r.id.0 && !c.expect.is_empty())
            .count();
        let positive = cases
            .iter()
            .filter(|c| c.rule == r.id.0 && c.expect.is_empty())
            .count();
        if checked {
            assert!(negative >= 1, "{} has no negative case", r.id.0);
            assert!(positive >= 1, "{} has no positive case", r.id.0);
        } else {
            assert_eq!(
                negative + positive,
                0,
                "{} has no check but has cases",
                r.id.0
            );
        }
    }
}

#[test]
fn rules_without_a_check_report_nothing_on_any_document() {
    // A producer or policy rule never appears in a result, whatever the document holds.
    let unchecked: BTreeSet<&str> = rule_table()
        .filter(|r| {
            matches!(
                r.status,
                RuleStatus::Producer { .. } | RuleStatus::Policy { .. }
            )
        })
        .map(|r| r.id.0)
        .collect();
    assert_eq!(unchecked.len(), 11);
    for c in all_cases() {
        for (rule, _) in &c.expect {
            assert!(!unchecked.contains(rule), "{}", c.name);
        }
    }
}

// --- structure ------------------------------------------------------------------------------------------

#[test]
fn many_problems_are_capped_and_flagged_as_truncated() {
    let mut doc = tools_doc();
    for _ in 0..150 {
        apply(&mut doc, &[Edit::Dup("/tools/0", "/tools")]);
    }
    let problems = check(&decode(StoreKind::ToolManifest, doc));
    assert_eq!(problems.items.len(), 100);
    assert!(problems.truncated);
    assert!(
        problems
            .items
            .iter()
            .all(|p| p.rule.is_some_and(|r| r.0 == "D11-01"))
    );
}

#[test]
fn a_problem_pointer_is_built_from_names_and_indices_only() {
    let mut doc = report_doc();
    apply(
        &mut doc,
        &[
            set(
                "/plan/scan_settings/renderers",
                json!([renderer(json!({MARKER: "a;b"}))]),
            ),
            set(
                "/checks/1/source_refs/0/resource_identifier",
                json!(format!("https://example.invalid/x?token={MARKER}")),
            ),
            set(
                "/remote_identifier_redacted",
                json!(format!(
                    "https://user:{MARKER}@github.com/example/example-service"
                )),
            ),
        ],
    );
    let problems = check(&decode(StoreKind::Report, doc));
    let hits = found(&problems);
    assert!(hits.contains(&(
        String::from("D11-11"),
        String::from("/plan/scan_settings/renderers/0/parameters_redacted")
    )));
    assert!(hits.contains(&(
        String::from("D11-06"),
        String::from("/remote_identifier_redacted")
    )));
    assert!(hits.contains(&(
        String::from("D11-06"),
        String::from("/checks/1/source_refs/0/resource_identifier")
    )));
    for p in &problems.items {
        assert!(p.pointer.as_str().starts_with('/'));
        assert!(
            p.pointer
                .as_str()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '/' || c == '_'),
            "{}",
            p.pointer.as_str()
        );
    }
}

#[test]
fn a_marker_in_free_text_never_reaches_the_problems() {
    let mut doc = report_doc();
    apply(
        &mut doc,
        &[
            set("/repo_name", json!(MARKER)),
            set("/limitations", json!([MARKER])),
            set("/findings/0/title", json!(MARKER)),
            set("/findings/0/evidence_redacted", json!(MARKER)),
            set("/artifacts/0/display_name", json!(MARKER)),
            set("/ai_enrichment/narrative_redacted", json!("")),
            set("/ai_enrichment/reason_redacted", json!(MARKER)),
            set(
                "/plan/scan_settings/renderers",
                json!([renderer(json!({MARKER: format!("{MARKER};x")}))]),
            ),
            set(
                "/checks/1/source_refs/0/resource_identifier",
                json!(format!("https://example.invalid/x?token={MARKER}")),
            ),
            set(
                "/remote_identifier_redacted",
                json!(format!(
                    "https://user:{MARKER}@github.com/example/example-service"
                )),
            ),
            set("/checks/2/reason_redacted", json!(MARKER)),
            set("/checks/2/term_signal", json!(0)),
        ],
    );
    let problems = check(&decode(StoreKind::Report, doc));
    assert!(!problems.items.is_empty());
    assert!(!format!("{problems:?}").contains(MARKER));

    let mut log = log_doc();
    apply(
        &mut log,
        &[
            set("/events/1/message_redacted", json!(MARKER)),
            set("/events/1/level", json!("info")),
            set("/reports/0/pdf_error_redacted", json!(MARKER)),
        ],
    );
    let problems = check(&decode(StoreKind::Log, log));
    assert!(!problems.items.is_empty());
    assert!(!format!("{problems:?}").contains(MARKER));

    let mut approvals = approvals_doc();
    apply(
        &mut approvals,
        &[
            set("/approvals/3/purpose", json!(MARKER)),
            set("/approvals/3/actor", json!(MARKER)),
            set("/approvals/3/decided_at", json!(T1)),
            set("/approvals/3/resource_constraints", json!([MARKER])),
        ],
    );
    let problems = check(&decode(StoreKind::Approvals, approvals));
    assert!(!problems.items.is_empty());
    assert!(!format!("{problems:?}").contains(MARKER));
}

#[test]
fn a_chunk_digest_covers_the_sanitised_text_bytes() {
    // The persisted text of an embedded ESC is `a<U+001B>b`; that, not the raw control character, is hashed.
    let mut doc = rag_doc();
    apply(
        &mut doc,
        &[
            set("/chunks/0/text_redacted", json!("a\u{1b}b")),
            set("/chunks/0/content_digest", json!(SHA_ESCAPED)),
        ],
    );
    match decode(StoreKind::RagIndex, doc) {
        AnyStore::RagIndex(f) => {
            assert_eq!(f.chunks[0].text_redacted.as_str(), "a<U+001B>b");
            assert!(check(&AnyStore::RagIndex(f)).is_empty());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_data_schema_examples_report_nothing() {
    // Data Schema §8 (`repo.json`), §9 (`workflows.json`) and §10 (`log.json`), synthetic.
    let repo = json!({
        "schema_version": "1.4", "store_type": "repositories", "updated_at": T0,
        "repositories": [{
            "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475", "display_name": "example-service",
            "slug": "example-service", "remote_url": "https://github.com/example/example-service.git",
            "canonical_remote": "github.com/example/example-service",
            "local_path": "checkouts/526a9739-68cb-5ef8-8c3a-0fda8a207475", "default_ref_kind": "branch",
            "default_ref_value": "main", "registered_at": T0, "last_update_at": null,
            "last_commit_oid": null, "checkout_state": "not_cloned", "last_error_redacted": null,
            "inventory_revision": 0, "inventory_at": null, "files": []
        }]
    });
    let workflows = json!({
        "schema_version": "1.4", "store_type": "workflows", "updated_at": T0,
        "workflows": [{
            "workflow_id": "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d", "revision": 1,
            "name": "Daily configuration review",
            "description": "Example only; framework support depends on installed approved adapters.",
            "enabled": false, "created_at": T0, "updated_at": T0,
            "targets": [{
                "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475", "ref_kind": "branch", "ref_value": "main",
                "selector": {"mode": "all_yaml", "paths": [], "include_patterns": [], "exclude_patterns": []}
            }],
            "scan_settings": {
                "categories": ["syntax", "schema", "secrets", "misconfiguration", "vulnerability"],
                "secret_scope": "checkout", "template_mode": "both", "schema_mode": "both",
                "schema_bindings": [], "renderers": [], "allow_reference_context": true, "adapter_ids": []
            },
            "schedule": {"kind": "daily", "interval_seconds": null, "daily_local_time": "10:00",
                         "timezone": "Asia/Kolkata", "next_due_at": null,
                         "overlap_policy": "skip_if_running", "missed_policy": "record_no_replay"},
            "approval_ids": [], "output_formats": ["json", "md", "pdf"]
        }]
    });
    let log = json!({
        "schema_version": "1.4", "store_type": "log", "updated_at": T0,
        "runs": [], "events": [], "reports": []
    });
    for (kind, doc) in [
        (StoreKind::Repositories, repo),
        (StoreKind::Workflows, workflows),
        (StoreKind::Log, log),
    ] {
        let problems = check(&decode(kind, doc));
        assert!(problems.is_empty(), "{kind:?}: {:?}", found(&problems));
    }
}

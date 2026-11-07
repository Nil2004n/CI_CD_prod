//! `AnyStore` (request §5.3, completed by the lead at B1): one variant per root kind, `kind()` agrees with
//! the serialised `store_type`, and serialisation is exactly the root document (untagged). All data is synthetic.

#![allow(
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

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
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const TS: &str = "2026-10-16T05:25:00Z";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn header(store_type: &str) -> Value {
    json!({"schema_version": "1.4", "store_type": store_type, "updated_at": TS})
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Some(e)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    base
}

fn config_doc() -> Value {
    with(
        header("config"),
        json!({"config": {
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
        }}),
    )
}

fn report_doc() -> Value {
    with(
        header("report"),
        json!({
            "report_id": uid(110),
            "run_id": uid(20),
            "repo_id": uid(10),
            "repo_name": "demo-repo",
            "remote_identifier_redacted": "example.invalid/demo",
            "selected_ref": "main",
            "commit_oid": null,
            "execution": "queued",
            "plan": {
                "workflow_id": null,
                "workflow_revision": null,
                "targets": [],
                "scan_settings": {
                    "categories": ["syntax", "schema", "secrets", "misconfiguration", "vulnerability"],
                    "secret_scope": "checkout",
                    "template_mode": "both",
                    "schema_mode": "both",
                    "schema_bindings": [],
                    "renderers": [],
                    "allow_reference_context": true,
                    "adapter_ids": []
                },
                "approval_ids": [],
                "created_at": TS,
                "plan_digest": DIGEST,
                "target_snapshots": [],
                "engine_snapshots": []
            },
            "generated_at": TS,
            "coverage": {
                "discovered_files": 0, "selected_files": 0, "assessed_files": 0, "excluded_files": 0,
                "missing_files": 0, "checks_passed": 0, "checks_with_findings": 0, "checks_failed": 0,
                "checks_unsupported": 0, "checks_skipped": 0, "checks_incomplete": 0, "checks_cancelled": 0,
                "coverage": "none",
                "assessment": "unknown"
            },
            "checks": [],
            "findings": [],
            "artifacts": [],
            "limitations": [],
            "ai_enrichment": {
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
            }
        }),
    )
}

fn decode<T: DeserializeOwned>(doc: &Value) -> T {
    match serde_json::from_value(doc.clone()) {
        Ok(v) => v,
        Err(e) => panic!("fixture must decode: {e}"),
    }
}

/// One minimal valid document per root kind, wrapped in its `AnyStore` variant.
fn cases() -> Vec<(StoreKind, Value, AnyStore)> {
    let collections = |t: &str, keys: &[&str]| {
        let mut extra = serde_json::Map::new();
        for k in keys {
            extra.insert((*k).to_owned(), json!([]));
        }
        with(header(t), Value::Object(extra))
    };
    let repo = collections("repositories", &["repositories"]);
    let workflows = collections("workflows", &["workflows"]);
    let approvals = collections("approvals", &["approvals"]);
    let log = collections("log", &["runs", "events", "reports"]);
    let suppressions = collections("suppressions", &["suppressions"]);
    let rag = collections("rag_index", &["chunks"]);
    let conversations = collections("conversations", &["conversations"]);
    let tools = collections("tool_manifest", &["tools"]);
    let config = config_doc();
    let report = report_doc();
    vec![
        (
            StoreKind::Config,
            config.clone(),
            AnyStore::Config(decode::<ConfigFile>(&config)),
        ),
        (
            StoreKind::Repositories,
            repo.clone(),
            AnyStore::Repositories(decode::<RepositoryFile>(&repo)),
        ),
        (
            StoreKind::Workflows,
            workflows.clone(),
            AnyStore::Workflows(decode::<WorkflowFile>(&workflows)),
        ),
        (
            StoreKind::Approvals,
            approvals.clone(),
            AnyStore::Approvals(decode::<ApprovalFile>(&approvals)),
        ),
        (
            StoreKind::Log,
            log.clone(),
            AnyStore::Log(decode::<LogFile>(&log)),
        ),
        (
            StoreKind::Suppressions,
            suppressions.clone(),
            AnyStore::Suppressions(decode::<SuppressionFile>(&suppressions)),
        ),
        (
            StoreKind::RagIndex,
            rag.clone(),
            AnyStore::RagIndex(decode::<RagIndexFile>(&rag)),
        ),
        (
            StoreKind::Conversations,
            conversations.clone(),
            AnyStore::Conversations(decode::<ConversationFile>(&conversations)),
        ),
        (
            StoreKind::Report,
            report.clone(),
            AnyStore::Report(Box::new(decode::<ReportFile>(&report))),
        ),
        (
            StoreKind::ToolManifest,
            tools.clone(),
            AnyStore::ToolManifest(decode::<ToolManifestFile>(&tools)),
        ),
    ]
}

#[test]
fn every_store_kind_has_exactly_one_case() {
    let kinds: Vec<StoreKind> = cases().into_iter().map(|(k, _, _)| k).collect();
    assert_eq!(kinds, StoreKind::ALL.to_vec());
}

#[test]
fn kind_matches_variant_and_serialised_store_type() {
    for (kind, _, any) in cases() {
        assert_eq!(any.kind(), kind);
        let out = serde_json::to_value(&any).expect("serialises");
        assert_eq!(out["store_type"], json!(kind.store_type()), "{kind:?}");
        assert_eq!(out["schema_version"], json!("1.4"), "{kind:?}");
    }
}

#[test]
fn serialises_as_the_root_document_untagged() {
    for (kind, doc, any) in cases() {
        let out = serde_json::to_value(&any).expect("serialises");
        assert_eq!(out, doc, "{kind:?}");
        let text = serde_json::to_string(&any).expect("serialises");
        assert!(
            text.starts_with(r#"{"schema_version":"1.4","store_type":""#),
            "{kind:?}: {text}"
        );
    }
}

/// Every `domain::store` module source (E-1, request §5.11).
const STORE_SOURCES: [(&str, &str); 19] = [
    ("mod.rs", include_str!("../src/store/mod.rs")),
    ("approval.rs", include_str!("../src/store/approval.rs")),
    ("approval15.rs", include_str!("../src/store/approval15.rs")),
    ("classes.rs", include_str!("../src/store/classes.rs")),
    ("config.rs", include_str!("../src/store/config.rs")),
    (
        "conversation.rs",
        include_str!("../src/store/conversation.rs"),
    ),
    ("evidence.rs", include_str!("../src/store/evidence.rs")),
    ("log.rs", include_str!("../src/store/log.rs")),
    ("plan.rs", include_str!("../src/store/plan.rs")),
    ("provider.rs", include_str!("../src/store/provider.rs")),
    ("rag.rs", include_str!("../src/store/rag.rs")),
    ("report.rs", include_str!("../src/store/report.rs")),
    ("repository.rs", include_str!("../src/store/repository.rs")),
    (
        "repository15.rs",
        include_str!("../src/store/repository15.rs"),
    ),
    ("roots15.rs", include_str!("../src/store/roots15.rs")),
    (
        "suppression.rs",
        include_str!("../src/store/suppression.rs"),
    ),
    ("tools.rs", include_str!("../src/store/tools.rs")),
    ("values.rs", include_str!("../src/store/values.rs")),
    ("workflow.rs", include_str!("../src/store/workflow.rs")),
];

/// Names of enums whose attribute block (the `#[...]` and `///` lines directly above the item) derives
/// `Deserialize`. Multi-line attributes are joined until the closing `]`.
fn enums_deriving_deserialize(src: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut attrs = String::new();
    let mut in_attr = false;
    for line in src.lines() {
        let t = line.trim();
        if in_attr {
            attrs.push_str(t);
            in_attr = !t.ends_with(']');
            continue;
        }
        if t.starts_with("#[") {
            attrs.push_str(t);
            in_attr = !t.ends_with(']');
            continue;
        }
        if t.starts_with("///") || t.is_empty() && attrs.is_empty() {
            continue;
        }
        let item = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t);
        if let Some(rest) = item.strip_prefix("enum ")
            && attrs.contains("derive(")
            && attrs.contains("Deserialize")
        {
            found.push(
                rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("")
                    .to_owned(),
            );
        }
        attrs.clear();
    }
    found
}

#[test]
fn the_enum_scanner_detects_a_derived_deserialize() {
    let positive = "/// Doc.\n#[derive(Clone, Copy,\n    Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum Bad {\n    A,\n}\n#[derive(Debug, Serialize, Deserialize)]\npub struct Fine {}\n#[derive(Debug)]\npub enum AlsoFine { B }\n";
    assert_eq!(
        enums_deriving_deserialize(positive),
        vec![String::from("Bad")]
    );
}

#[test]
fn no_store_enum_derives_deserialize() {
    // E-1: every enum defined in `domain::store` is generated by `closed_enum!` (no-echo decoding).
    for (name, src) in STORE_SOURCES {
        assert_eq!(
            enums_deriving_deserialize(src),
            Vec::<String>::new(),
            "{name}"
        );
    }
}

#[test]
fn every_store_module_is_scanned() {
    let declared: Vec<&str> = include_str!("../src/store/mod.rs")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub mod ")?.strip_suffix(';'))
        .collect();
    let mut scanned: Vec<&str> = STORE_SOURCES
        .iter()
        .filter_map(|(f, _)| f.strip_suffix(".rs"))
        .filter(|f| *f != "mod")
        .collect();
    scanned.sort_unstable();
    let mut declared_sorted = declared.clone();
    declared_sorted.sort_unstable();
    assert_eq!(scanned, declared_sorted);
}

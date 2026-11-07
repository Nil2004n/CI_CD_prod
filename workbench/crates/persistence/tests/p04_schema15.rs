//! Shape checks only: production readers and semantic validation remain separate.

use domain::store::StoreKind;
use persistence::schema::{
    Repository15ReferenceError, check_repository15_references, validate_unchanged15,
};
use persistence::validate::Verdict;
use serde_json::{Value, json};

#[test]
fn upgrade_preflight_preserves_synthetic_roots_and_refuses_incompatible_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let validator = persistence::read::SchemaValidator::new();
    for (kind, source) in [
        (
            StoreKind::Config,
            include_str!("fixtures/store/config.min.json"),
        ),
        (
            StoreKind::Repositories,
            include_str!("fixtures/store/repositories.min.json"),
        ),
        (
            StoreKind::Workflows,
            include_str!("fixtures/store/workflows.min.json"),
        ),
        (
            StoreKind::Approvals,
            include_str!("fixtures/store/approvals.min.json"),
        ),
        (StoreKind::Log, include_str!("fixtures/store/log.min.json")),
        (
            StoreKind::RagIndex,
            include_str!("fixtures/store/rag_index.min.json"),
        ),
        (
            StoreKind::Conversations,
            include_str!("fixtures/store/conversations.min.json"),
        ),
        (
            StoreKind::ToolManifest,
            include_str!("fixtures/store/tool_manifest.min.json"),
        ),
    ] {
        let old: Value = serde_json::from_str(source)?;
        let bytes = persistence::read::prepare_operational15(kind, source.as_bytes(), &validator)?;
        let mut new: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(new["schema_version"], "1.5");
        new["schema_version"] = json!("1.4");
        assert_eq!(new, old);
    }
    assert!(
        persistence::read::prepare_operational15(
            StoreKind::Repositories,
            include_bytes!("fixtures/store/repositories.max.json"),
            &validator,
        )
        .is_err()
    );
    let old: Value = serde_json::from_str(include_str!("fixtures/store/approvals.max.json"))?;
    let bytes = persistence::read::prepare_operational15(
        StoreKind::Approvals,
        &serde_json::to_vec(&old)?,
        &validator,
    )?;
    let mut new: Value = serde_json::from_slice(&bytes)?;
    new["schema_version"] = json!("1.4");
    for approval in new["approvals"].as_array_mut().ok_or("approvals")? {
        let fields = approval.as_object_mut().ok_or("approval")?;
        assert_eq!(fields.remove("git_approval_context"), Some(Value::Null));
    }
    assert_eq!(new, old);
    let mut old: Value =
        serde_json::from_str(include_str!("fixtures/store/repositories.max.json"))?;
    old["repositories"]
        .as_array_mut()
        .ok_or("repositories")?
        .truncate(1);
    old["repositories"][0]["remote_url"] = json!("https://github.com/example/example-service");
    let bytes = persistence::read::prepare_operational15(
        StoreKind::Repositories,
        &serde_json::to_vec(&old)?,
        &validator,
    )?;
    let mut new: Value = serde_json::from_slice(&bytes)?;
    new["schema_version"] = json!("1.4");
    for repository in new["repositories"].as_array_mut().ok_or("repositories")? {
        assert_eq!(
            repository
                .as_object_mut()
                .ok_or("repository")?
                .remove("last_git_version"),
            Some(Value::Null)
        );
    }
    assert_eq!(new, old);
    Ok(())
}

#[test]
fn version_dispatch_rejects_future_roots_and_versioned_historical_reports()
-> Result<(), Box<dyn std::error::Error>> {
    let validator = persistence::read::SchemaValidator::new();
    let mut doc: Value = serde_json::from_str(include_str!("fixtures/store/report.min.json"))?;
    assert!(
        persistence::read::decode_versioned(
            StoreKind::Report,
            &serde_json::to_vec(&doc)?,
            &validator
        )
        .is_ok()
    );
    doc["schema_version"] = json!("1.5");
    assert!(
        persistence::read::decode_versioned(
            StoreKind::Report,
            &serde_json::to_vec(&doc)?,
            &validator
        )
        .is_err()
    );
    doc["schema_version"] = json!("1.6");
    assert!(
        persistence::read::decode_versioned(
            StoreKind::Report,
            &serde_json::to_vec(&doc)?,
            &validator
        )
        .is_err()
    );
    Ok(())
}

fn schema() -> Result<jsonschema::Validator, Box<dyn std::error::Error>> {
    let source = include_str!("../../../../schemas/workbench-store-v1.5.schema.json");
    let doc: Value = serde_json::from_str(source)?;
    Ok(jsonschema::validator_for(&doc)?)
}

#[test]
fn unchanged_root_projection_only_changes_version_schema() -> Result<(), Box<dyn std::error::Error>>
{
    let old: Value = serde_json::from_str(include_str!(
        "../../../../schemas/workbench-store-v1.4.schema.json"
    ))?;
    let mut new: Value = serde_json::from_str(include_str!(
        "../../../../schemas/workbench-store-v1.5.schema.json"
    ))?;
    let old_defs = old["$defs"].as_object().ok_or("old definitions")?;
    let new_defs = new["$defs"].as_object_mut().ok_or("new definitions")?;
    assert_eq!(old_defs.len(), new_defs.len());
    for (name, old_def) in old_defs {
        if matches!(name.as_str(), "Repository" | "Approval") {
            continue;
        }
        let new_def = new_defs.get_mut(name).ok_or("missing definition")?;
        if name.ends_with("File") {
            new_def["properties"]["schema_version"] =
                old_def["properties"]["schema_version"].clone();
        }
        assert_eq!(old_def, new_def, "schema definition changed: {name}");
    }
    Ok(())
}

#[test]
fn unchanged_roots_validate_under_both_schema_and_existing_rules()
-> Result<(), Box<dyn std::error::Error>> {
    for (kind, source) in [
        (
            StoreKind::Config,
            include_str!("fixtures/store/config.min.json"),
        ),
        (
            StoreKind::Workflows,
            include_str!("fixtures/store/workflows.min.json"),
        ),
        (StoreKind::Log, include_str!("fixtures/store/log.min.json")),
        (
            StoreKind::RagIndex,
            include_str!("fixtures/store/rag_index.min.json"),
        ),
        (
            StoreKind::Conversations,
            include_str!("fixtures/store/conversations.min.json"),
        ),
        (
            StoreKind::ToolManifest,
            include_str!("fixtures/store/tool_manifest.min.json"),
        ),
    ] {
        let mut doc: Value = serde_json::from_str(source)?;
        let legacy = persistence::read::decode_versioned(
            kind,
            &serde_json::to_vec(&doc)?,
            &persistence::read::SchemaValidator::new(),
        )?;
        assert!(matches!(legacy, domain::store::VersionedStore::V14(_)));
        assert_ne!(validate_unchanged15(kind, &doc), Verdict::Valid);
        doc["schema_version"] = json!("1.5");
        assert_eq!(validate_unchanged15(kind, &doc), Verdict::Valid);
        let versioned = persistence::read::decode_versioned(
            kind,
            &serde_json::to_vec(&doc)?,
            &persistence::read::SchemaValidator::new(),
        )?;
        assert!(matches!(versioned, domain::store::VersionedStore::V15(_)));
        let typed = persistence::read::decode_unchanged15(kind, &serde_json::to_vec(&doc)?)?;
        assert_eq!(typed.kind(), kind);
        assert_eq!(serde_json::to_value(typed)?["schema_version"], "1.5");
        doc["unexpected"] = json!("canary");
        assert_ne!(validate_unchanged15(kind, &doc), Verdict::Valid);
        assert!(persistence::read::decode_unchanged15(kind, &serde_json::to_vec(&doc)?).is_err());
    }
    let doc: Value = serde_json::from_str(include_str!("fixtures/store/repositories.min.json"))?;
    assert_ne!(
        validate_unchanged15(StoreKind::Repositories, &doc),
        Verdict::Valid
    );
    Ok(())
}

#[test]
fn repository_reference_pairs_and_git_version_are_checked() -> Result<(), Box<dyn std::error::Error>>
{
    let validator = schema()?;
    let mut doc: Value =
        serde_json::from_str(include_str!("fixtures/store/repositories.max.json"))?;
    doc["schema_version"] = json!("1.5");
    for repo in doc["repositories"].as_array_mut().unwrap() {
        repo["last_git_version"] = Value::Null;
    }
    assert!(validator.is_valid(&doc));
    doc["repositories"][0]["default_ref_kind"] = Value::Null;
    assert!(!validator.is_valid(&doc));
    doc["repositories"][0]["default_ref_value"] = Value::Null;
    assert!(validator.is_valid(&doc));
    for version in ["2.53.0", "2.53.999999"] {
        doc["repositories"][0]["last_git_version"] = json!(version);
        assert!(validator.is_valid(&doc));
    }
    for version in ["2.53.01", "2.54.0", "2.53.1-extra"] {
        doc["repositories"][0]["last_git_version"] = json!(version);
        assert!(!validator.is_valid(&doc));
    }
    Ok(())
}

#[test]
fn approval_context_is_required_and_stage_shapes_are_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let validator = schema()?;
    let mut doc: Value = serde_json::from_str(include_str!("fixtures/store/approvals.max.json"))?;
    doc["schema_version"] = json!("1.5");
    for approval in doc["approvals"].as_array_mut().unwrap() {
        approval["git_approval_context"] = Value::Null;
    }
    assert!(validator.is_valid(&doc));
    let id = "00000000-0000-4000-8000-000000000001";
    doc["approvals"][0]["git_approval_context"] =
        json!({"stage":"discover","repo_id":id,"ref_kind":null,"ref_value":null});
    assert!(validator.is_valid(&doc));
    doc["approvals"][0]["git_approval_context"]["ref_kind"] = json!("branch");
    assert!(!validator.is_valid(&doc));
    doc["approvals"][0]["git_approval_context"] =
        json!({"stage":"clone","repo_id":id,"ref_kind":"branch","ref_value":"main"});
    assert!(validator.is_valid(&doc));
    doc["approvals"][0]["git_approval_context"]["extra"] = json!(true);
    assert!(!validator.is_valid(&doc));
    Ok(())
}

#[test]
fn approval15_applies_legacy_semantics_and_git_context_rules()
-> Result<(), Box<dyn std::error::Error>> {
    use persistence::schema::validate_approval15;

    let mut doc: Value = serde_json::from_str(include_str!("fixtures/store/approvals.max.json"))?;
    doc["schema_version"] = json!("1.5");
    for approval in doc["approvals"].as_array_mut().ok_or("approvals")? {
        approval["git_approval_context"] = Value::Null;
    }
    assert_eq!(validate_approval15(&doc), Verdict::Valid);
    let read = persistence::read::decode_versioned(
        StoreKind::Approvals,
        &serde_json::to_vec(&doc)?,
        &persistence::read::SchemaValidator::new(),
    )?;
    assert!(matches!(
        read,
        domain::store::VersionedStore::V15(domain::store::OperationalStore15::Approvals(_))
    ));
    doc["approvals"][0]["git_approval_context"] = json!({
        "stage":"discover", "repo_id":"00000000-0000-4000-8000-000000000001",
        "ref_kind":null, "ref_value":null
    });
    assert_ne!(validate_approval15(&doc), Verdict::Valid);
    doc["approvals"][0]["git_approval_context"] = Value::Null;
    doc["approvals"][0]["workflow_revision"] = json!(0);
    assert_ne!(validate_approval15(&doc), Verdict::Valid);
    Ok(())
}

#[test]
fn repository_reference_check_runs_schema_before_kind_rules()
-> Result<(), Box<dyn std::error::Error>> {
    let mut doc: Value =
        serde_json::from_str(include_str!("fixtures/store/repositories.max.json"))?;
    doc["schema_version"] = json!("1.5");
    for repo in doc["repositories"].as_array_mut().ok_or("repositories")? {
        repo["last_git_version"] = Value::Null;
        repo["default_ref_kind"] = Value::Null;
        repo["default_ref_value"] = Value::Null;
    }
    let check = |value: &Value| -> Result<_, serde_json::Error> {
        Ok(check_repository15_references(&serde_json::to_vec(value)?))
    };
    assert_eq!(check(&doc)?, Ok(()));
    doc["repositories"][0]["default_ref_kind"] = json!("branch");
    assert_eq!(
        check(&doc)?,
        Err(Repository15ReferenceError::InvalidDocument)
    );
    doc["repositories"][0]["default_ref_value"] = json!("main");
    assert_eq!(check(&doc)?, Ok(()));
    doc["repositories"][0]["default_ref_value"] = json!("HEAD");
    assert_eq!(
        check(&doc)?,
        Err(Repository15ReferenceError::InvalidReference)
    );
    doc["repositories"][0]["default_ref_kind"] = json!("commit");
    doc["repositories"][0]["default_ref_value"] = json!("a".repeat(40));
    assert_eq!(check(&doc)?, Ok(()));
    doc["repositories"][0]["default_ref_value"] = json!("a".repeat(39));
    assert_eq!(
        check(&doc)?,
        Err(Repository15ReferenceError::InvalidReference)
    );
    doc["schema_version"] = json!("1.4");
    assert_eq!(
        check(&doc)?,
        Err(Repository15ReferenceError::InvalidDocument)
    );
    Ok(())
}

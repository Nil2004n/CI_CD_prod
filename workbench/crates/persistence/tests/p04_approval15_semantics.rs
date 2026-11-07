//! Synthetic checks for the isolated 1.5 Git-context rules.
use domain::store::approval15::Approval;
use persistence::semantic::approval15::{GitContextError, validate_git_context};
use serde_json::{Value, json};

const REPO: &str = "526a9739-68cb-5ef8-8c3a-0fda8a207475";

#[test]
fn invalid_stage_error_does_not_echo_input() {
    let marker = "sensitive-stage-marker";
    let input = format!("\"{marker}\"");
    let result = serde_json::from_str::<domain::store::approval15::GitApprovalStage>(&input);
    assert!(result.is_err());
    if let Err(error) = result {
        assert!(!error.to_string().contains(marker));
    }
}

fn candidate(stage: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/store/approvals.max.json"))?;
    let mut approval = fixture["approvals"][0].clone();
    approval["repo_ids"] = json!([REPO]);
    approval["data_categories"] = json!(["repository_metadata"]);
    approval["capability"] = json!(if stage == "update" {
        "git_update"
    } else {
        "git_acquire"
    });
    approval["git_approval_context"] = if stage == "discover" {
        json!({"stage": stage, "repo_id": REPO, "ref_kind": null, "ref_value": null})
    } else {
        json!({"stage": stage, "repo_id": REPO, "ref_kind": "branch", "ref_value": "main"})
    };
    Ok(approval)
}

fn check(value: Value) -> Result<Result<(), GitContextError>, serde_json::Error> {
    Ok(validate_git_context(&serde_json::from_value::<Approval>(
        value,
    )?))
}

#[test]
fn valid_stages_references_and_historical_null() -> Result<(), Box<dyn std::error::Error>> {
    for stage in ["discover", "clone", "update"] {
        assert_eq!(check(candidate(stage)?)?, Ok(()));
    }
    for (kind, value) in [
        ("branch", "main"),
        ("tag", "v1"),
        ("commit", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    ] {
        let mut a = candidate("clone")?;
        a["git_approval_context"]["ref_kind"] = json!(kind);
        a["git_approval_context"]["ref_value"] = json!(value);
        assert_eq!(check(a)?, Ok(()));
    }
    let mut historical = candidate("clone")?;
    historical["git_approval_context"] = Value::Null;
    assert_eq!(check(historical)?, Ok(()));
    Ok(())
}

#[test]
fn stage_capability_and_reference_mismatches_are_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    for stage in ["discover", "clone", "update"] {
        let mut a = candidate(stage)?;
        a["capability"] = json!(if stage == "update" {
            "git_acquire"
        } else {
            "git_update"
        });
        assert_eq!(check(a)?, Err(GitContextError::Capability));
    }
    for (stage, kind, value) in [
        ("discover", json!("branch"), json!("main")),
        ("discover", Value::Null, json!("main")),
        ("clone", Value::Null, Value::Null),
        ("clone", json!("branch"), Value::Null),
        ("update", Value::Null, json!("main")),
        ("clone", json!("branch"), json!("HEAD")),
        ("clone", json!("tag"), json!("refs/tags/v1")),
        ("update", json!("commit"), json!("main")),
    ] {
        let mut a = candidate(stage)?;
        a["git_approval_context"]["ref_kind"] = kind;
        a["git_approval_context"]["ref_value"] = value;
        assert_eq!(check(a)?, Err(GitContextError::Reference));
    }
    Ok(())
}

#[test]
fn scope_mismatches_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("repo_ids", json!([]), GitContextError::RepositoryScope),
        (
            "repo_ids",
            json!(["0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b"]),
            GitContextError::RepositoryScope,
        ),
        (
            "repo_ids",
            json!([REPO, REPO]),
            GitContextError::RepositoryScope,
        ),
        (
            "workflow_id",
            json!("9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d"),
            GitContextError::Binding,
        ),
        ("workflow_revision", json!(1), GitContextError::Binding),
        (
            "run_id",
            json!("12345678-1234-4234-8234-123456789abc"),
            GitContextError::Binding,
        ),
        ("destinations", json!([]), GitContextError::Destination),
        (
            "destinations",
            json!(["github.com:443", "example.com:443"]),
            GitContextError::Destination,
        ),
        ("lifetime", json!("run"), GitContextError::Lifetime),
        (
            "resource_constraints",
            json!(["limited"]),
            GitContextError::Constraints,
        ),
        (
            "data_categories",
            json!([]),
            GitContextError::DataCategories,
        ),
        (
            "data_categories",
            json!(["repository_metadata", "redacted_source"]),
            GitContextError::DataCategories,
        ),
        (
            "expires_at",
            json!("2026-11-16T05:25:00Z"),
            GitContextError::Expiry,
        ),
    ];
    for (field, value, expected) in cases {
        let mut a = candidate("clone")?;
        a[field] = value;
        assert_eq!(check(a)?, Err(expected), "{field}");
    }
    let fixture: Value = serde_json::from_str(include_str!("fixtures/store/approvals.max.json"))?;
    let mut a = candidate("clone")?;
    a["inference_scope"] = fixture["approvals"][1]["inference_scope"].clone();
    assert_eq!(check(a)?, Err(GitContextError::Binding));
    Ok(())
}

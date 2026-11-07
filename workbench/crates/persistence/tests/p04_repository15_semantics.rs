//! Requires the primary-owned semantic module export and repositories dependency.
use domain::store::repository15::RepositoryFile;
use persistence::semantic::repository15::check;
use serde_json::{Value, json};

fn doc() -> Value {
    json!({"schema_version":"1.5", "store_type":"repositories", "updated_at":"2026-01-01T00:00:00Z", "repositories":[{
        "repo_id":"00000000-0000-4000-8000-000000000001", "display_name":"Example", "slug":"example",
        "remote_url":"https://example.com/Org/Repo.git", "canonical_remote":"example.com/Org/Repo.git",
        "local_path":"checkouts/00000000-0000-4000-8000-000000000001", "default_ref_kind":null,
        "default_ref_value":null, "registered_at":"2026-01-01T00:00:00Z", "last_update_at":null,
        "last_commit_oid":null, "last_git_version":null, "checkout_state":"not_cloned",
        "last_error_redacted":null, "inventory_revision":0, "inventory_at":null, "files":[]
    }]})
}
fn valid(v: Value) -> bool {
    match serde_json::from_value::<RepositoryFile>(v) {
        Ok(file) => check(&file).is_empty(),
        Err(_) => false,
    }
}

#[test]
fn schema_and_semantics_are_both_enforced() -> Result<(), Box<dyn std::error::Error>> {
    use persistence::schema::validate_repository15;
    use persistence::validate::Verdict;

    let mut d = doc();
    assert_eq!(validate_repository15(&d), Verdict::Valid);
    let decoded = persistence::read::decode_repository15(&serde_json::to_vec(&d)?)?;
    assert_eq!(decoded.repositories.len(), 1);
    let versioned = persistence::read::decode_versioned(
        domain::store::StoreKind::Repositories,
        &serde_json::to_vec(&d)?,
        &persistence::read::SchemaValidator::new(),
    )?;
    assert!(matches!(
        versioned,
        domain::store::VersionedStore::V15(domain::store::OperationalStore15::Repositories(_))
    ));
    d["repositories"][0]["canonical_remote"] = json!("example.com/org/Repo.git");
    assert_ne!(validate_repository15(&d), Verdict::Valid);
    assert!(persistence::read::decode_repository15(&serde_json::to_vec(&d)?).is_err());
    d["repositories"][0]["canonical_remote"] = json!("example.com/Org/Repo.git");
    d["repositories"][0]["default_ref_kind"] = json!("branch");
    assert_ne!(validate_repository15(&d), Verdict::Valid);
    d["schema_version"] = json!("1.4");
    assert_ne!(validate_repository15(&d), Verdict::Valid);
    Ok(())
}
#[test]
fn reference_pairs_and_kinds() {
    assert!(valid(doc()));
    for (kind, value, expected) in [
        ("branch", "main", true),
        ("tag", "v1", true),
        ("branch", "HEAD", false),
        ("branch", "refs/heads/main", false),
        ("commit", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", true),
        ("commit", "main", false),
    ] {
        let mut d = doc();
        d["repositories"][0]["default_ref_kind"] = json!(kind);
        d["repositories"][0]["default_ref_value"] = json!(value);
        assert_eq!(valid(d), expected, "{kind} {value}");
    }
    let mut d = doc();
    d["repositories"][0]["default_ref_kind"] = json!("branch");
    assert!(!valid(d));
    let mut d = doc();
    d["repositories"][0]["default_ref_value"] = json!("main");
    assert!(!valid(d));
}
#[test]
fn canonical_identity_path_and_duplicates() -> Result<(), Box<dyn std::error::Error>> {
    assert!(valid(doc()));
    let mut d = doc();
    d["repositories"][0]["canonical_remote"] = json!("example.com/org/repo.git");
    assert!(!valid(d));
    let mut d = doc();
    d["repositories"][0]["canonical_remote"] = json!("example.com/Org/Repo");
    assert!(!valid(d));
    let mut d = doc();
    d["repositories"][0]["local_path"] = json!("checkouts/elsewhere");
    assert!(!valid(d));
    let mut d = doc();
    let second = d["repositories"][0].clone();
    d["repositories"]
        .as_array_mut()
        .ok_or("repositories")?
        .push(second);
    assert!(!valid(d));
    let mut d = doc();
    let mut second = d["repositories"][0].clone();
    second["repo_id"] = json!("00000000-0000-4000-8000-000000000002");
    second["local_path"] = json!("checkouts/00000000-0000-4000-8000-000000000002");
    second["remote_url"] = json!("https://example.com/org/Repo.git");
    second["canonical_remote"] = json!("example.com/org/Repo.git");
    d["repositories"]
        .as_array_mut()
        .ok_or("repositories")?
        .push(second);
    assert!(valid(d));
    Ok(())
}
#[test]
fn version_and_legacy_timestamp_invariants() {
    for (version, expected) in [
        ("2.53.0", true),
        ("2.53.999999", true),
        ("2.53.01", false),
        ("2.53.1000000", false),
        ("2.52.1", false),
        ("2.53.1-foo", false),
    ] {
        let mut d = doc();
        d["repositories"][0]["last_git_version"] = json!(version);
        d["repositories"][0]["last_update_at"] = json!("2026-01-02T00:00:00Z");
        d["repositories"][0]["last_commit_oid"] = json!("a".repeat(40));
        assert_eq!(valid(d), expected, "{version}");
    }
    let mut d = doc();
    d["repositories"][0]["last_git_version"] = json!("2.53.0");
    assert!(!valid(d));
    let mut d = doc();
    d["repositories"][0]["last_update_at"] = json!("2026-01-02T00:00:00Z");
    assert!(!valid(d));
    let mut d = doc();
    d["repositories"][0]["inventory_at"] = json!("2025-01-01T00:00:00Z");
    assert!(!valid(d));
}

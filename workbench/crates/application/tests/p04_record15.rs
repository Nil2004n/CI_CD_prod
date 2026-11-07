//! Synthetic record construction without storage or network access.
use application::repository_record15::{RecordError, build};
use domain::contract::FixedText;
use domain::ids::RepoId;
use domain::store::repository15::CheckoutState;
use domain::time::UtcTimestamp;
use repositories::remote::RemoteSpec;

#[test]
fn constructs_unresolved_record_with_fixed_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let remote = RemoteSpec::parse("https://Example.com:443/Team/Project.git")
        .map_err(|_| "remote parse")?;
    let id = RepoId::parse("526a9739-68cb-5ef8-8c3a-0fda8a207475")?;
    let now = UtcTimestamp::parse_rfc3339("2026-10-16T05:25:00Z")?;
    let record = build(&remote, id, now).map_err(|_| "record build")?;
    assert_eq!(record.repo_id, id);
    assert_eq!(record.display_name, FixedText::RepositoryDisplay.redacted());
    assert_eq!(record.slug.as_str(), "project-git");
    assert_eq!(
        record.remote_url.as_str(),
        "https://example.com/Team/Project.git"
    );
    assert_eq!(
        record.canonical_remote.as_str(),
        "example.com/Team/Project.git"
    );
    assert_eq!(record.local_path.as_str(), format!("checkouts/{id}"));
    assert_eq!(record.default_ref_kind, None);
    assert_eq!(record.default_ref_value, None);
    assert_eq!(record.registered_at, now);
    assert_eq!(record.last_update_at, None);
    assert_eq!(record.last_commit_oid, None);
    assert_eq!(record.last_git_version, None);
    assert_eq!(record.checkout_state, CheckoutState::NotCloned);
    assert_eq!(record.last_error_redacted, None);
    assert_eq!(record.inventory_revision, 0);
    assert_eq!(record.inventory_at, None);
    assert!(record.files.is_empty());
    let file = domain::store::repository15::RepositoryFile {
        schema_version: domain::store::SchemaVersion15,
        store_type: domain::store::RepositoriesStoreType,
        updated_at: now,
        repositories: vec![record],
    };
    assert_eq!(
        persistence::schema::validate_repository15(&serde_json::to_value(file)?),
        persistence::validate::Verdict::Valid
    );
    Ok(())
}

#[test]
fn preserves_validated_transport_and_normalizes_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let id = RepoId::parse("526a9739-68cb-5ef8-8c3a-0fda8a207475")?;
    let now = UtcTimestamp::parse_rfc3339("2026-10-16T05:25:00Z")?;
    for (input, normalized) in [
        (
            "https://Example.com:443/Team/Repo",
            "https://example.com/Team/Repo",
        ),
        ("git@Example.com:Team/Repo", "git@example.com:Team/Repo"),
        (
            "ssh://git@Example.com:2222/Team/Repo",
            "ssh://git@example.com:2222/Team/Repo",
        ),
    ] {
        let remote = RemoteSpec::parse(input).map_err(|_| "remote parse")?;
        let record = build(&remote, id, now).map_err(|_| "record build")?;
        assert_eq!(record.remote_url.as_str(), normalized);
        assert_eq!(record.canonical_remote.as_str(), "example.com/Team/Repo");
        assert_eq!(record.slug.as_str(), "repo");
    }
    Ok(())
}

#[test]
fn error_debug_is_fixed_and_contains_no_input() {
    assert_eq!(format!("{:?}", RecordError::InvalidPath), "InvalidPath");
}

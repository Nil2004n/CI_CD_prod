//! Convert validated Git remotes into approval destinations.

use domain::{
    contract::{FixedText, RawInput},
    store::values::RemoteUrl,
    text::RedactedText,
};
use repositories::remote::{RemoteScheme, RemoteSpec};

use crate::permission::{DestinationError, NormalisedDestination};

/// Fixed-text preflight failure; no source values are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repository15PreflightError {
    InvalidLegacyDocument,
    ManualReferenceDecision,
    ManualRemoteDecision,
    DuplicateIdentity,
    InvalidCandidate,
}

impl std::fmt::Display for Repository15PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "repository 1.5 preflight: {self:?}")
    }
}

impl std::error::Error for Repository15PreflightError {}

/// Construct a repository-only 1.5 candidate from validated 1.4 bytes.
///
/// This performs no I/O or migration. It preserves existing values, refusing
/// records requiring a reference or remote identity decision. Other roots,
/// complete semantic validation, and journal recovery remain required before
/// any candidate can be published.
pub fn preflight_repository15(
    bytes: &[u8],
) -> Result<domain::store::repository15::RepositoryFile, Repository15PreflightError> {
    use domain::store::{AnyStore, SchemaVersion15, StoreKind, repository15};
    use persistence::read::{SchemaValidator, decode};
    use std::collections::HashSet;

    let AnyStore::Repositories(source) =
        decode(StoreKind::Repositories, bytes, &SchemaValidator::new())
            .map_err(|_| Repository15PreflightError::InvalidLegacyDocument)?
    else {
        return Err(Repository15PreflightError::InvalidLegacyDocument);
    };
    let mut identities = HashSet::new();
    let mut repositories = Vec::with_capacity(source.repositories.len());
    for repo in source.repositories {
        if !repository15::valid_target_reference(repo.default_ref_kind, &repo.default_ref_value) {
            return Err(Repository15PreflightError::ManualReferenceDecision);
        }
        let parsed = RemoteSpec::parse(repo.remote_url.as_str())
            .map_err(|_| Repository15PreflightError::ManualRemoteDecision)?;
        if parsed.canonical_remote() != &repo.canonical_remote {
            return Err(Repository15PreflightError::ManualRemoteDecision);
        }
        if !identities.insert(repo.canonical_remote.as_str().to_owned()) {
            return Err(Repository15PreflightError::DuplicateIdentity);
        }
        repositories.push(repository15::Repository {
            repo_id: repo.repo_id,
            display_name: repo.display_name,
            slug: repo.slug,
            remote_url: repo.remote_url,
            canonical_remote: repo.canonical_remote,
            local_path: repo.local_path,
            default_ref_kind: Some(repo.default_ref_kind),
            default_ref_value: Some(repo.default_ref_value),
            registered_at: repo.registered_at,
            last_update_at: repo.last_update_at,
            last_commit_oid: repo.last_commit_oid,
            last_git_version: None,
            checkout_state: repo.checkout_state,
            last_error_redacted: repo.last_error_redacted,
            inventory_revision: repo.inventory_revision,
            inventory_at: repo.inventory_at,
            files: repo.files,
        });
    }
    let candidate = repository15::RepositoryFile {
        schema_version: SchemaVersion15,
        store_type: source.store_type,
        updated_at: source.updated_at,
        repositories,
    };
    let candidate_bytes =
        serde_json::to_vec(&candidate).map_err(|_| Repository15PreflightError::InvalidCandidate)?;
    persistence::schema::check_repository15_references(&candidate_bytes)
        .map_err(|_| Repository15PreflightError::InvalidCandidate)?;
    Ok(candidate)
}

/// Input-free intake failure. Existing remotes that cannot be interpreted under
/// the P04 grammar must be resolved rather than silently treated as distinct.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IntakeError {
    Invalid,
    Duplicate,
    ExistingUnverifiable,
    Destination,
}

/// Prepared metadata; the caller must retain its store lock until registration.
pub struct PreparedRemote {
    pub remote: RemoteSpec,
    pub destination: NormalisedDestination,
    pub display_name: RedactedText,
}

impl std::fmt::Debug for PreparedRemote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedRemote(redacted)")
    }
}

/// Validate intake and reject duplicate identities before any store mutation.
///
/// Existing identities are derived from their validated remote URLs rather than
/// historical stored keys. This does not change the 1.4 store validator or make
/// an unresolved-reference repository record writable under that version.
pub fn prepare<'a>(
    input: &RawInput,
    existing: impl IntoIterator<Item = &'a RemoteUrl>,
) -> Result<PreparedRemote, IntakeError> {
    let remote = RemoteSpec::parse(input.as_str()).map_err(|_| IntakeError::Invalid)?;
    let destination = approval_destination(&remote).map_err(|_| IntakeError::Destination)?;
    for stored in existing {
        let prior =
            RemoteSpec::parse(stored.as_str()).map_err(|_| IntakeError::ExistingUnverifiable)?;
        if prior.canonical_remote() == remote.canonical_remote() {
            return Err(IntakeError::Duplicate);
        }
    }
    Ok(PreparedRemote {
        remote,
        destination,
        display_name: FixedText::RepositoryDisplay.redacted(),
    })
}

/// Build the P04 approval destination without passing SSH userinfo to P03.
///
/// HTTPS approvals retain the remote path. SSH approvals are scoped to host and
/// port; the full remote, including SSH user and path, must be bound separately
/// in the operation fingerprint. This conversion does not authorize a request.
pub fn approval_destination(
    remote: &RemoteSpec,
) -> Result<NormalisedDestination, DestinationError> {
    let parts = remote.destination_parts();
    let input = match parts.scheme {
        RemoteScheme::Https => format!("https://{}:{}/{}", parts.host, parts.port, parts.path),
        RemoteScheme::Ssh => format!("ssh://{}:{}", parts.host, parts.port),
    };
    NormalisedDestination::parse(&input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_preserves_legacy_values_and_refuses_ambiguous_records()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc: serde_json::Value = serde_json::from_str(include_str!(
            "../../persistence/tests/fixtures/store/repositories.max.json"
        ))?;
        let repositories = doc["repositories"].as_array_mut().ok_or("repositories")?;
        repositories.truncate(1);
        repositories[0]["remote_url"] =
            serde_json::json!("https://github.com/example/example-service");
        let source = serde_json::to_vec(&doc)?;
        let candidate = preflight_repository15(&source)?;
        assert_eq!(candidate.repositories.len(), 1);
        assert_eq!(
            candidate.repositories[0].remote_url.as_str(),
            "https://github.com/example/example-service"
        );
        assert_eq!(
            candidate.repositories[0]
                .default_ref_value
                .as_ref()
                .map(|v| v.as_str()),
            Some("main")
        );
        assert_eq!(candidate.repositories[0].last_git_version, None);
        doc["repositories"][0]["default_ref_value"] = serde_json::json!("HEAD");
        assert_eq!(
            preflight_repository15(&serde_json::to_vec(&doc)?),
            Err(Repository15PreflightError::ManualReferenceDecision)
        );
        doc["repositories"][0]["default_ref_value"] = serde_json::json!("main");
        doc["repositories"][0]["remote_url"] =
            serde_json::json!("https://github.com/example/example-service.git");
        assert_eq!(
            preflight_repository15(&serde_json::to_vec(&doc)?),
            Err(Repository15PreflightError::ManualRemoteDecision)
        );
        assert_eq!(
            preflight_repository15(b"not json"),
            Err(Repository15PreflightError::InvalidLegacyDocument)
        );
        Ok(())
    }

    #[test]
    fn approval_destinations_preserve_https_path_and_use_ssh_host_port() {
        for (input, expected) in [
            (
                "https://Example.com/Team/repo.git",
                "https://example.com:443/Team/repo.git",
            ),
            (
                "https://example.com:8443/Team/repo",
                "https://example.com:8443/Team/repo",
            ),
            ("git@example.com:Team/repo", "ssh://example.com:22"),
            (
                "ssh://alice@example.com:2222/Team/repo",
                "ssh://example.com:2222",
            ),
        ] {
            let remote = RemoteSpec::parse(input).unwrap();
            assert_eq!(approval_destination(&remote).unwrap().canonical(), expected);
        }
    }

    #[test]
    fn prepare_rejects_duplicates_and_preserves_exact_path_identity() {
        let existing = RemoteUrl::parse("https://example.com/Team/repo.git").unwrap();
        let input = |s: &str| RawInput::new(s.to_owned());
        assert!(matches!(
            prepare(&input("git@example.com:Team/repo.git"), [&existing]),
            Err(IntakeError::Duplicate)
        ));
        for distinct in [
            "https://example.com/team/repo.git",
            "https://example.com/Team/repo",
        ] {
            let result = prepare(&input(distinct), [&existing]).unwrap();
            assert_eq!(result.display_name, FixedText::RepositoryDisplay.redacted());
            assert_eq!(format!("{result:?}"), "PreparedRemote(redacted)");
        }
        assert!(matches!(
            prepare(
                &input("https://example.com/Team/repo"),
                [&RemoteUrl::parse("https://localhost/team/repo").unwrap()]
            ),
            Err(IntakeError::ExistingUnverifiable)
        ));
    }
}

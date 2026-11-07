//! Pure store 1.5 repository checks. Remote parsing is delegated to the acquisition parser.
use std::collections::HashSet;

use domain::store::repository15::{RepositoryFile, valid_target_reference};
use repositories::remote::RemoteSpec;

use crate::validate::{Layer, Problem, ProblemCode, Problems, RuleId};

fn flag(out: &mut Problems, rule: &'static str, pointer: String) {
    out.push(Problem::new(
        Layer::Semantic,
        ProblemCode::Rule,
        &pointer,
        Some(RuleId(rule)),
    ));
}

/// Validate a typed 1.5 repository root without I/O. This does not check checkout contents.
pub fn check(file: &RepositoryFile) -> Problems {
    let mut out = Problems::default();
    let mut ids = HashSet::new();
    let mut file_ids = HashSet::new();
    let mut remotes = HashSet::new();
    for (i, repo) in file.repositories.iter().enumerate() {
        let base = format!("/repositories/{i}");
        if !ids.insert(repo.repo_id) {
            flag(&mut out, "D11-01", format!("{base}/repo_id"));
        }
        let mut paths = HashSet::new();
        for (j, item) in repo.files.iter().enumerate() {
            if !file_ids.insert(item.file_id) {
                flag(&mut out, "D11-01", format!("{base}/files/{j}/file_id"));
            }
            if !paths.insert(item.path.as_str()) {
                flag(&mut out, "D11-01", format!("{base}/files/{j}/path"));
            }
            super::workflow::file_record(&format!("{base}/files/{j}"), item, &mut out);
        }
        if repo.local_path.as_str() != format!("checkouts/{}", repo.repo_id) {
            flag(&mut out, "D11-06", format!("{base}/local_path"));
        }
        match RemoteSpec::parse(repo.remote_url.as_str()) {
            Ok(remote) if remote.canonical_remote().as_str() == repo.canonical_remote.as_str() => {}
            _ => flag(&mut out, "D11-06", format!("{base}/canonical_remote")),
        }
        if !remotes.insert(repo.canonical_remote.as_str()) {
            flag(&mut out, "D11-06", format!("{base}/canonical_remote"));
        }
        match (repo.default_ref_kind, repo.default_ref_value.as_ref()) {
            (None, None) => {}
            (Some(kind), Some(value)) if valid_target_reference(kind, value) => {}
            _ => flag(&mut out, "P04-REF", format!("{base}/default_ref_value")),
        }
        if let Some(version) = &repo.last_git_version {
            let valid = version.as_str().strip_prefix("2.53.").is_some_and(|patch| {
                !patch.is_empty()
                    && patch.len() <= 6
                    && patch.bytes().all(|b| b.is_ascii_digit())
                    && (patch == "0" || !patch.starts_with('0'))
            });
            if !valid {
                flag(
                    &mut out,
                    "P04-GIT-VERSION",
                    format!("{base}/last_git_version"),
                );
            }
            if repo.last_commit_oid.is_none() || repo.last_update_at.is_none() {
                flag(
                    &mut out,
                    "P04-GIT-VERSION",
                    format!("{base}/last_git_version"),
                );
            }
        }
        if repo.last_update_at.is_some() && repo.last_commit_oid.is_none() {
            flag(&mut out, "D11-20", format!("{base}/last_commit_oid"));
        }
        if repo.last_update_at.is_none() && repo.last_commit_oid.is_some() {
            flag(&mut out, "D11-20", format!("{base}/last_update_at"));
        }
        if repo
            .last_update_at
            .as_ref()
            .is_some_and(|t| t < &repo.registered_at)
        {
            flag(&mut out, "D11-18", format!("{base}/last_update_at"));
        }
        if repo
            .inventory_at
            .as_ref()
            .is_some_and(|t| t < &repo.registered_at)
        {
            flag(&mut out, "D11-18", format!("{base}/inventory_at"));
        }
    }
    out
}

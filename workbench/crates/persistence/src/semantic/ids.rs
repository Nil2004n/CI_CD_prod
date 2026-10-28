//! Semantic rule bodies for identity, reference and provenance (Data Schema §11 rules 1, 3, 4, 5, 6, 20, 22).
//!
//! * `D11-01` identifiers are unique and embedded identifiers agree.
//! * `D11-03` file IDs and source references stay within the repository scope of their report.
//! * `D11-04` `check.finding_ids` resolve, and a finding's check and task IDs match its check.
//! * `D11-05` source references have the fields their kind needs and resolve inside the document.
//! * `D11-06` internal paths are confined; remote identifiers carry no credentials.
//! * `D11-20` line ranges are ordered; a commit ID is null only where no usable snapshot exists.
//! * `D11-22` a finding cites a source or the report states a limitation.
//!
//! Every function is pure over one [`AnyStore`] and ignores the roots its rule does not concern.

use std::collections::{HashMap, HashSet};

use domain::ids::{CheckId, FileId, FindingId, RepoId, ReportId, RunId, SourceId};
use domain::states::{CheckOutcome, RunState};
use domain::store::AnyStore;
use domain::store::evidence::{ArtifactKind, SourceKind, SourceRef};
use domain::store::log::LogFile;
use domain::store::plan::PlanSnapshot;
use domain::store::report::ReportFile;
use domain::store::repository::RepositoryFile;

use super::rules::{duplicates, flag, plans, source_refs};
use crate::validate::Problems;

/// Rule `D11-01`: identifiers are unique within their entity domain and embedded identifiers agree with
/// the run and report that contain them.
///
/// Checked per root: repository, file and path identity; workflow, approval, suppression, chunk,
/// conversation, message and tool-adapter identity; run, event and report-manifest identity and the
/// agreement of manifests with the runs that list them; check, finding and artifact identity inside a
/// report, `finding.run_id` and `source.run_id` equal to the report's run, and one `SourceRef` per
/// `source_id`; and, for every plan snapshot, workflow ID and revision set together, unique adapters and
/// unique file IDs inside one target snapshot.
pub(crate) fn d11_01(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-01";
    match doc {
        AnyStore::Repositories(f) => repositories_unique(f, out),
        AnyStore::Workflows(f) => {
            duplicates(out, R, f.workflows.iter().map(|w| w.workflow_id), |i| {
                format!("/workflows/{i}/workflow_id")
            });
            for (i, w) in f.workflows.iter().enumerate() {
                duplicates(out, R, w.approval_ids.iter().copied(), |j| {
                    format!("/workflows/{i}/approval_ids/{j}")
                });
            }
        }
        AnyStore::Approvals(f) => {
            duplicates(out, R, f.approvals.iter().map(|a| a.approval_id), |i| {
                format!("/approvals/{i}/approval_id")
            });
        }
        AnyStore::Log(f) => log_unique(f, out),
        AnyStore::Suppressions(f) => {
            duplicates(
                out,
                R,
                f.suppressions.iter().map(|s| s.suppression_id),
                |i| format!("/suppressions/{i}/suppression_id"),
            );
        }
        AnyStore::RagIndex(f) => {
            duplicates(out, R, f.chunks.iter().map(|c| c.chunk_id), |i| {
                format!("/chunks/{i}/chunk_id")
            });
        }
        AnyStore::Conversations(f) => {
            duplicates(
                out,
                R,
                f.conversations.iter().map(|c| c.conversation_id),
                |i| format!("/conversations/{i}/conversation_id"),
            );
            let mut seen = HashSet::new();
            for (i, c) in f.conversations.iter().enumerate() {
                for (j, m) in c.messages.iter().enumerate() {
                    if !seen.insert(m.message_id) {
                        flag(
                            out,
                            R,
                            &format!("/conversations/{i}/messages/{j}/message_id"),
                        );
                    }
                }
            }
        }
        AnyStore::ToolManifest(f) => {
            duplicates(out, R, f.tools.iter().map(|t| t.adapter_id), |i| {
                format!("/tools/{i}/adapter_id")
            });
        }
        AnyStore::Report(r) => report_unique(doc, r, out),
        AnyStore::Config(_) => {}
    }
    for (prefix, plan) in plans(doc) {
        plan_unique(&prefix, plan, out);
    }
}

/// `D11-01` for `repo.json`: repository IDs, file IDs (store-wide) and file paths (per repository).
fn repositories_unique(f: &RepositoryFile, out: &mut Problems) {
    const R: &str = "D11-01";
    duplicates(out, R, f.repositories.iter().map(|r| r.repo_id), |i| {
        format!("/repositories/{i}/repo_id")
    });
    let mut file_ids = HashSet::new();
    for (i, r) in f.repositories.iter().enumerate() {
        let mut paths = HashSet::new();
        for (j, file) in r.files.iter().enumerate() {
            if !file_ids.insert(file.file_id) {
                flag(out, R, &format!("/repositories/{i}/files/{j}/file_id"));
            }
            if !paths.insert(file.path.as_str()) {
                flag(out, R, &format!("/repositories/{i}/files/{j}/path"));
            }
        }
    }
}

/// `D11-01` for `log.json`: run, event and report IDs; one manifest per run and repository; a manifest
/// whose report a run lists names that run.
fn log_unique(f: &LogFile, out: &mut Problems) {
    const R: &str = "D11-01";
    duplicates(out, R, f.runs.iter().map(|r| r.run_id), |i| {
        format!("/runs/{i}/run_id")
    });
    duplicates(out, R, f.events.iter().map(|e| e.event_id), |i| {
        format!("/events/{i}/event_id")
    });
    duplicates(out, R, f.reports.iter().map(|m| m.report_id), |i| {
        format!("/reports/{i}/report_id")
    });
    duplicates(
        out,
        R,
        f.reports.iter().map(|m| (m.run_id, m.repo_id)),
        |i| format!("/reports/{i}/repo_id"),
    );
    let mut owner: HashMap<ReportId, RunId> = HashMap::new();
    for (i, run) in f.runs.iter().enumerate() {
        duplicates(out, R, run.report_ids.iter().copied(), |j| {
            format!("/runs/{i}/report_ids/{j}")
        });
        for report_id in &run.report_ids {
            owner.entry(*report_id).or_insert(run.run_id);
        }
    }
    for (i, m) in f.reports.iter().enumerate() {
        if owner.get(&m.report_id).is_some_and(|run| *run != m.run_id) {
            flag(out, R, &format!("/reports/{i}/run_id"));
        }
    }
}

/// `D11-01` for a report: check, finding and artifact IDs; findings of this run; one `SourceRef` per
/// `source_id`; source references of this run.
fn report_unique(doc: &AnyStore, r: &ReportFile, out: &mut Problems) {
    const R: &str = "D11-01";
    duplicates(out, R, r.checks.iter().map(|c| c.check_id), |i| {
        format!("/checks/{i}/check_id")
    });
    duplicates(out, R, r.findings.iter().map(|f| f.finding_id), |i| {
        format!("/findings/{i}/finding_id")
    });
    duplicates(out, R, r.artifacts.iter().map(|a| a.artifact_id), |i| {
        format!("/artifacts/{i}/artifact_id")
    });
    for (i, c) in r.checks.iter().enumerate() {
        duplicates(out, R, c.finding_ids.iter().copied(), |j| {
            format!("/checks/{i}/finding_ids/{j}")
        });
    }
    for (i, f) in r.findings.iter().enumerate() {
        if f.run_id != r.run_id {
            flag(out, R, &format!("/findings/{i}/run_id"));
        }
    }
    let mut first: HashMap<SourceId, &SourceRef> = HashMap::new();
    for (loc, s) in source_refs(doc) {
        if s.run_id.is_some_and(|run| run != r.run_id) {
            flag(out, R, &format!("{}/run_id", loc.pointer()));
        }
        if *first.entry(s.source_id).or_insert(s) != s {
            flag(out, R, &format!("{}/source_id", loc.pointer()));
        }
    }
}

/// `D11-01` for one plan snapshot at `prefix`.
fn plan_unique(prefix: &str, plan: &PlanSnapshot, out: &mut Problems) {
    const R: &str = "D11-01";
    if plan.workflow_id.is_some() != plan.workflow_revision.is_some()
        || plan.workflow_revision == Some(0)
    {
        flag(out, R, &format!("{prefix}/workflow_revision"));
    }
    duplicates(out, R, plan.approval_ids.iter().copied(), |i| {
        format!("{prefix}/approval_ids/{i}")
    });
    duplicates(
        out,
        R,
        plan.engine_snapshots.iter().map(|e| e.adapter_id),
        |i| format!("{prefix}/engine_snapshots/{i}/adapter_id"),
    );
    for (k, t) in plan.target_snapshots.iter().enumerate() {
        duplicates(out, R, t.selected_files.iter().map(|f| f.file_id), |m| {
            format!("{prefix}/target_snapshots/{k}/selected_files/{m}/file_id")
        });
    }
}

/// Rule `D11-03`: file IDs and source references cannot silently cross repository scope.
///
/// In a report: the report's repository is one of its plan's targets; every finding and every source
/// reference that names a repository names the report's; a check's `file_id` is a file selected in the
/// plan's target snapshot of the report's repository (when the plan holds one). In every plan snapshot:
/// each target snapshot belongs to one of the plan's targets.
pub(crate) fn d11_03(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-03";
    if let AnyStore::Report(r) = doc {
        if !r.plan.targets.is_empty() && !r.plan.targets.iter().any(|t| t.repo_id == r.repo_id) {
            flag(out, R, "/repo_id");
        }
        for (i, f) in r.findings.iter().enumerate() {
            if f.repo_id != r.repo_id {
                flag(out, R, &format!("/findings/{i}/repo_id"));
            }
        }
        for (loc, s) in source_refs(doc) {
            if s.repo_id.is_some_and(|repo| repo != r.repo_id) {
                flag(out, R, &format!("{}/repo_id", loc.pointer()));
            }
        }
        let own: Vec<_> = r
            .plan
            .target_snapshots
            .iter()
            .filter(|t| t.repo_id == r.repo_id)
            .collect();
        if !own.is_empty() {
            let selected: HashSet<FileId> = own
                .iter()
                .flat_map(|t| t.selected_files.iter().map(|f| f.file_id))
                .collect();
            for (i, c) in r.checks.iter().enumerate() {
                if c.file_id.is_some_and(|id| !selected.contains(&id)) {
                    flag(out, R, &format!("/checks/{i}/file_id"));
                }
            }
        }
    }
    for (prefix, plan) in plans(doc) {
        let targeted: HashSet<RepoId> = plan.targets.iter().map(|t| t.repo_id).collect();
        for (k, t) in plan.target_snapshots.iter().enumerate() {
            if !targeted.contains(&t.repo_id) {
                flag(out, R, &format!("{prefix}/target_snapshots/{k}/repo_id"));
            }
        }
    }
}

/// Rule `D11-04`: inside a report, `check.finding_ids` resolve to findings of that check, each finding's
/// `check_id` resolves to a check that lists it, and its `task_id` equals the check's. A passed check lists
/// no finding and a check with outcome `findings` lists at least one.
pub(crate) fn d11_04(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-04";
    let AnyStore::Report(r) = doc else { return };
    let mut finding_at: HashMap<FindingId, usize> = HashMap::new();
    for (i, f) in r.findings.iter().enumerate() {
        finding_at.entry(f.finding_id).or_insert(i);
    }
    let mut check_at: HashMap<CheckId, usize> = HashMap::new();
    let mut listed: HashSet<(CheckId, FindingId)> = HashSet::new();
    for (i, c) in r.checks.iter().enumerate() {
        check_at.entry(c.check_id).or_insert(i);
        listed.extend(c.finding_ids.iter().map(|id| (c.check_id, *id)));
    }
    for (i, c) in r.checks.iter().enumerate() {
        for (j, id) in c.finding_ids.iter().enumerate() {
            let resolves = finding_at
                .get(id)
                .and_then(|k| r.findings.get(*k))
                .is_some_and(|f| f.check_id == c.check_id);
            if !resolves {
                flag(out, R, &format!("/checks/{i}/finding_ids/{j}"));
            }
        }
        let has_findings = !c.finding_ids.is_empty();
        if (c.outcome == CheckOutcome::Passed && has_findings)
            || (c.outcome == CheckOutcome::Findings && !has_findings)
        {
            flag(out, R, &format!("/checks/{i}/finding_ids"));
        }
    }
    for (i, f) in r.findings.iter().enumerate() {
        match check_at.get(&f.check_id).and_then(|k| r.checks.get(*k)) {
            None => flag(out, R, &format!("/findings/{i}/check_id")),
            Some(c) => {
                if c.task_id != f.task_id {
                    flag(out, R, &format!("/findings/{i}/task_id"));
                }
                if !listed.contains(&(c.check_id, f.finding_id)) {
                    flag(out, R, &format!("/findings/{i}/check_id"));
                }
            }
        }
    }
}

/// Rule `D11-05` (store part): a source reference has the fields its kind needs, evidence retrieved from an
/// external reference names the approval that allowed it, and in a report an `artifact_id` resolves to an
/// artifact of that report. An external artifact names its approval.
///
/// Kinds: `repository_file` needs `repo_id` and `file_path`; `rendered_artifact` an `artifact_id`;
/// `scan_report` a `run_id`; `rule_document` and `schema_document` one of `file_path`, `artifact_id` or
/// `resource_identifier`; `external_reference` a `resource_identifier` (and `approval_id` when
/// `content_digest` shows the content was retrieved). `activity_log` and `workflow` need nothing more.
/// Retrieval-time eligibility (still retained, not stale) belongs to P10/P17.
pub(crate) fn d11_05(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-05";
    let artifacts: Option<HashSet<_>> = match doc {
        AnyStore::Report(r) => Some(r.artifacts.iter().map(|a| a.artifact_id).collect()),
        _ => None,
    };
    for (loc, s) in source_refs(doc) {
        let at = loc.pointer();
        match s.kind {
            SourceKind::RepositoryFile => {
                if s.repo_id.is_none() {
                    flag(out, R, &format!("{at}/repo_id"));
                }
                if s.file_path.is_none() {
                    flag(out, R, &format!("{at}/file_path"));
                }
            }
            SourceKind::RenderedArtifact => {
                if s.artifact_id.is_none() {
                    flag(out, R, &format!("{at}/artifact_id"));
                }
            }
            SourceKind::ScanReport => {
                if s.run_id.is_none() {
                    flag(out, R, &format!("{at}/run_id"));
                }
            }
            SourceKind::RuleDocument | SourceKind::SchemaDocument => {
                if s.file_path.is_none()
                    && s.artifact_id.is_none()
                    && s.resource_identifier.is_none()
                {
                    flag(out, R, &format!("{at}/resource_identifier"));
                }
            }
            SourceKind::ExternalReference => {
                if s.resource_identifier.is_none() {
                    flag(out, R, &format!("{at}/resource_identifier"));
                }
                if s.content_digest.is_some() && s.approval_id.is_none() {
                    flag(out, R, &format!("{at}/approval_id"));
                }
            }
            SourceKind::ActivityLog | SourceKind::Workflow => {}
        }
        if let (Some(known), Some(id)) = (&artifacts, &s.artifact_id)
            && !known.contains(id)
        {
            flag(out, R, &format!("{at}/artifact_id"));
        }
    }
    if let AnyStore::Report(r) = doc {
        for (i, a) in r.artifacts.iter().enumerate() {
            if a.kind == ArtifactKind::External && a.approval_id.is_none() {
                flag(out, R, &format!("/artifacts/{i}/approval_id"));
            }
        }
    }
}

/// Rule `D11-06`: internal paths are confined and remote identifiers carry no credentials.
///
/// `repo.json`: `local_path` is `checkouts/<repo_id>`, `canonical_remote` is the canonical form of
/// `remote_url` (host lower-cased, scheme, user, port and a trailing `.git` or `/` removed; compared
/// ASCII-case-insensitively) and no two repositories share one. `log.json`: a report manifest's JSON,
/// Markdown and PDF paths sit in one directory, share one stem, carry the extensions `.json`, `.md` and
/// `.pdf` and do not point into `checkouts/`. A report: `remote_identifier_redacted` and every
/// `resource_identifier` carry no user information (a user at all for `http(s)`, a password for other
/// schemes) and no query string. Artifacts: `retained_path` is not under `checkouts/`. Path syntax
/// (relative, no `..`) is enforced by the types.
pub(crate) fn d11_06(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-06";
    match doc {
        AnyStore::Repositories(f) => {
            let mut canonical = HashSet::new();
            for (i, r) in f.repositories.iter().enumerate() {
                if r.local_path.as_str() != format!("checkouts/{}", r.repo_id) {
                    flag(out, R, &format!("/repositories/{i}/local_path"));
                }
                if let Some(expected) = canonical_of(r.remote_url.as_str())
                    && !expected.eq_ignore_ascii_case(r.canonical_remote.as_str())
                {
                    flag(out, R, &format!("/repositories/{i}/canonical_remote"));
                }
                if !canonical.insert(r.canonical_remote.as_str().to_ascii_lowercase()) {
                    flag(out, R, &format!("/repositories/{i}/canonical_remote"));
                }
            }
        }
        AnyStore::Log(f) => {
            for (i, m) in f.reports.iter().enumerate() {
                manifest_paths(i, m, out);
            }
        }
        AnyStore::Report(r) => {
            if exposes_credentials(r.remote_identifier_redacted.as_str()) {
                flag(out, R, "/remote_identifier_redacted");
            }
            for (i, a) in r.artifacts.iter().enumerate() {
                if a.retained_path
                    .as_ref()
                    .is_some_and(|p| in_checkouts(p.as_str()))
                {
                    flag(out, R, &format!("/artifacts/{i}/retained_path"));
                }
            }
        }
        _ => {}
    }
    for (loc, s) in source_refs(doc) {
        if s.resource_identifier
            .as_ref()
            .is_some_and(|t| exposes_credentials(t.as_str()))
        {
            flag(out, R, &format!("{}/resource_identifier", loc.pointer()));
        }
    }
}

/// `D11-06` for the paths of report manifest `i`.
fn manifest_paths(i: usize, m: &domain::store::log::ReportManifest, out: &mut Problems) {
    const R: &str = "D11-06";
    let json = split_path(m.json_path.as_str(), ".json");
    let md = split_path(m.md_path.as_str(), ".md");
    if json.is_none() {
        flag(out, R, &format!("/reports/{i}/json_path"));
    }
    if md.is_none() || (json.is_some() && md != json) {
        flag(out, R, &format!("/reports/{i}/md_path"));
    }
    if let Some(pdf) = &m.pdf_path {
        let pdf = split_path(pdf.as_str(), ".pdf");
        if pdf.is_none() || (json.is_some() && pdf != json) {
            flag(out, R, &format!("/reports/{i}/pdf_path"));
        }
    }
    for (field, path) in [
        ("json_path", Some(&m.json_path)),
        ("md_path", Some(&m.md_path)),
        ("pdf_path", m.pdf_path.as_ref()),
    ] {
        if path.is_some_and(|p| in_checkouts(p.as_str())) {
            flag(out, R, &format!("/reports/{i}/{field}"));
        }
    }
}

/// Splits `path` into its directory and file stem if it ends with `extension`.
fn split_path<'a>(path: &'a str, extension: &str) -> Option<(&'a str, &'a str)> {
    let without = path.strip_suffix(extension)?;
    let (dir, stem) = without.rsplit_once('/').unwrap_or(("", without));
    if stem.is_empty() {
        None
    } else {
        Some((dir, stem))
    }
}

/// Whether the relative `path` lies in the tool-managed `checkouts/` tree.
fn in_checkouts(path: &str) -> bool {
    path == "checkouts" || path.starts_with("checkouts/")
}

/// The canonical `host/path` form of a remote URL (`https://`, `ssh://` or scp-like `user@host:path`), or
/// `None` when the URL has no such shape.
fn canonical_of(url: &str) -> Option<String> {
    let (authority, path) = match url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("ssh://"))
    {
        Some(rest) => rest.split_once('/')?,
        None => url.split_once(':')?,
    };
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = if host_port.starts_with('[') {
        host_port
            .split_once(']')
            .map(|(host, _)| format!("{host}]"))?
    } else {
        host_port.split(':').next()?.to_owned()
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    Some(format!("{}/{path}", host.to_ascii_lowercase()))
}

/// Whether `text`, read as a URL, carries user information (any for `http` and `https`, a password for
/// other schemes) or a query string. Text without a `://` is not a URL and is never flagged.
fn exposes_credentials(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    if rest.contains('?') {
        return true;
    }
    let authority = rest.split(['/', '#']).next().unwrap_or("");
    match authority.rsplit_once('@') {
        Some((userinfo, _)) => {
            scheme.eq_ignore_ascii_case("http")
                || scheme.eq_ignore_ascii_case("https")
                || userinfo.contains(':')
        }
        None => false,
    }
}

/// Rule `D11-20`: line ranges are ordered and one-based, and a commit ID is null only where no usable
/// snapshot exists.
///
/// Source references: `line_start`, `line_end` and `document_index` are at least 1, `line_end` needs
/// `line_start`, and `line_start <= line_end`. Reports: a `completed` or `partial` run has a commit ID, and
/// so does a report with findings. Repositories: `last_update_at` and `last_commit_oid` are both set or
/// both null.
pub(crate) fn d11_20(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-20";
    for (loc, s) in source_refs(doc) {
        let at = loc.pointer();
        if s.document_index == Some(0) {
            flag(out, R, &format!("{at}/document_index"));
        }
        if s.line_start == Some(0) {
            flag(out, R, &format!("{at}/line_start"));
        }
        match (s.line_start, s.line_end) {
            (None, Some(_)) => flag(out, R, &format!("{at}/line_end")),
            (Some(start), Some(end)) if end < start || end == 0 => {
                flag(out, R, &format!("{at}/line_end"));
            }
            _ => {}
        }
    }
    match doc {
        AnyStore::Report(r) => {
            let needs_commit = matches!(r.execution, RunState::Completed | RunState::Partial)
                || !r.findings.is_empty();
            if r.commit_oid.is_none() && needs_commit {
                flag(out, R, "/commit_oid");
            }
        }
        AnyStore::Repositories(f) => {
            for (i, r) in f.repositories.iter().enumerate() {
                if r.last_update_at.is_some() && r.last_commit_oid.is_none() {
                    flag(out, R, &format!("/repositories/{i}/last_commit_oid"));
                }
                if r.last_update_at.is_none() && r.last_commit_oid.is_some() {
                    flag(out, R, &format!("/repositories/{i}/last_update_at"));
                }
            }
        }
        _ => {}
    }
}

/// Rule `D11-22`: a finding cites at least one source, or the report states at least one limitation (the
/// explicit source-unavailable statement; the text of a limitation is free-form, so only its presence is
/// checked).
pub(crate) fn d11_22(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-22";
    let AnyStore::Report(r) = doc else { return };
    if r.limitations.is_empty() {
        for (i, f) in r.findings.iter().enumerate() {
            if f.sources.is_empty() {
                flag(out, R, &format!("/findings/{i}/sources"));
            }
        }
    }
}

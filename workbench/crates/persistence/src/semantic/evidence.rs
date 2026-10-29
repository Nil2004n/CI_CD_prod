//! Semantic rule bodies for provenance, outcomes and integrity (Data Schema §11 rules 18, 19, 21, 25 and §18
//! rules 8, 14, 18).
//!
//! * `D11-18` timestamps are ordered and run states agree with the run timestamps.
//! * `D11-19` a chunk's content digest is the SHA-256 of its sanitised text.
//! * `D11-21` coverage counters agree with check records and selected-file snapshots.
//! * `D11-25` failure events are recorded as errors; the configured resource limits are bounded.
//! * `D18-08` retained evidence is redacted content only.
//! * `D18-14` `pdf_written` requires verified output and no unresolved error.
//! * `D18-18` a terminal report leaves no check pending or running; interrupted checks are explained.
//!
//! Every function is pure over one [`AnyStore`] and ignores the roots its rule does not concern.

use sha2::{Digest, Sha256};

use domain::states::{CheckOutcome, RunState};
use domain::store::AnyStore;
use domain::store::log::EventLevel;
use domain::store::log::EventType;
use domain::store::report::CoverageSummary;
use domain::time::UtcTimestamp;
use domain::vocab::Assessment;

use super::rules::flag;
use crate::validate::Problems;

/// Largest accepted `Config.max_scanner_processes`. Development default pending the FR-11 owner decision.
pub(crate) const MAX_SCANNER_PROCESSES: u64 = 64;

/// Largest accepted `Config.max_active_repositories`. Development default pending the FR-11 owner decision.
pub(crate) const MAX_ACTIVE_REPOSITORIES: u64 = 1_024;

/// SHA-256 of the empty byte string, lowercase hex: the digest of a zero-length file.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Whether both instants are present and `later` is before `earlier`.
fn out_of_order(earlier: Option<&UtcTimestamp>, later: Option<&UtcTimestamp>) -> bool {
    matches!((earlier, later), (Some(a), Some(b)) if b < a)
}

/// Rule `D11-18`: timestamps are ordered wherever a pair exists; a null start or finish is valid before
/// execution.
///
/// Repositories: `registered_at` is not after `last_update_at` or `inventory_at`. Workflows: `created_at`
/// is not after `updated_at`. Approvals: `requested_at <= decided_at`, `requested_at <= expires_at` and
/// `decided_at <= expires_at`. Runs: `created_at <= started_at <= finished_at` and `created_at <=
/// last_checkpoint_at`; a `queued` run has neither start nor finish, a `running` or `cancelling` run has a
/// start and no finish, `awaiting_approval` has no finish, `completed` has both, `partial`, `failed` and
/// `cancelled` have a finish. Reports: the plan is created before the report is generated, a check starts
/// before it finishes and a finding is observed before the report is generated. Chunks: retrieved before
/// indexed. Conversations: `created_at <= updated_at`, and messages are ordered and lie within the
/// conversation. Suppressions: `created_at <= expires_at`.
pub(crate) fn d11_18(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-18";
    match doc {
        AnyStore::Repositories(f) => {
            for (i, r) in f.repositories.iter().enumerate() {
                if out_of_order(Some(&r.registered_at), r.last_update_at.as_ref()) {
                    flag(out, R, &format!("/repositories/{i}/last_update_at"));
                }
                if out_of_order(Some(&r.registered_at), r.inventory_at.as_ref()) {
                    flag(out, R, &format!("/repositories/{i}/inventory_at"));
                }
            }
        }
        AnyStore::Workflows(f) => {
            for (i, w) in f.workflows.iter().enumerate() {
                if out_of_order(Some(&w.created_at), Some(&w.updated_at)) {
                    flag(out, R, &format!("/workflows/{i}/updated_at"));
                }
            }
        }
        AnyStore::Approvals(f) => {
            for (i, a) in f.approvals.iter().enumerate() {
                if out_of_order(Some(&a.requested_at), a.decided_at.as_ref()) {
                    flag(out, R, &format!("/approvals/{i}/decided_at"));
                }
                if out_of_order(Some(&a.requested_at), a.expires_at.as_ref())
                    || out_of_order(a.decided_at.as_ref(), a.expires_at.as_ref())
                {
                    flag(out, R, &format!("/approvals/{i}/expires_at"));
                }
            }
        }
        AnyStore::Log(f) => {
            for (i, run) in f.runs.iter().enumerate() {
                let at = format!("/runs/{i}");
                let (started, finished) = (run.started_at.as_ref(), run.finished_at.as_ref());
                if out_of_order(Some(&run.created_at), started) {
                    flag(out, R, &format!("{at}/started_at"));
                }
                if out_of_order(Some(&run.created_at), finished) || out_of_order(started, finished)
                {
                    flag(out, R, &format!("{at}/finished_at"));
                }
                if out_of_order(Some(&run.created_at), run.last_checkpoint_at.as_ref()) {
                    flag(out, R, &format!("{at}/last_checkpoint_at"));
                }
                run_state_dates(
                    &at,
                    run.execution,
                    started.is_some(),
                    finished.is_some(),
                    out,
                );
            }
        }
        AnyStore::Report(r) => {
            if out_of_order(Some(&r.plan.created_at), Some(&r.generated_at)) {
                flag(out, R, "/generated_at");
            }
            for (i, c) in r.checks.iter().enumerate() {
                if out_of_order(c.started_at.as_ref(), c.finished_at.as_ref()) {
                    flag(out, R, &format!("/checks/{i}/finished_at"));
                }
            }
            for (i, f) in r.findings.iter().enumerate() {
                if out_of_order(Some(&f.observed_at), Some(&r.generated_at)) {
                    flag(out, R, &format!("/findings/{i}/observed_at"));
                }
            }
        }
        AnyStore::RagIndex(f) => {
            for (i, c) in f.chunks.iter().enumerate() {
                if out_of_order(Some(&c.source.retrieved_at), Some(&c.indexed_at)) {
                    flag(out, R, &format!("/chunks/{i}/indexed_at"));
                }
            }
        }
        AnyStore::Conversations(f) => {
            for (i, c) in f.conversations.iter().enumerate() {
                if out_of_order(Some(&c.created_at), Some(&c.updated_at)) {
                    flag(out, R, &format!("/conversations/{i}/updated_at"));
                }
                let mut previous = &c.created_at;
                for (j, m) in c.messages.iter().enumerate() {
                    let outside = m.created_at < c.created_at || m.created_at > c.updated_at;
                    if outside || m.created_at < *previous {
                        flag(
                            out,
                            R,
                            &format!("/conversations/{i}/messages/{j}/created_at"),
                        );
                    }
                    previous = &m.created_at;
                }
            }
        }
        AnyStore::Suppressions(f) => {
            for (i, s) in f.suppressions.iter().enumerate() {
                if out_of_order(Some(&s.created_at), s.expires_at.as_ref()) {
                    flag(out, R, &format!("/suppressions/{i}/expires_at"));
                }
            }
        }
        AnyStore::Config(_) | AnyStore::ToolManifest(_) => {}
    }
}

/// `D11-18`: the run state agrees with which of the start and finish times are set.
fn run_state_dates(at: &str, state: RunState, started: bool, finished: bool, out: &mut Problems) {
    const R: &str = "D11-18";
    let needs_start = matches!(
        state,
        RunState::Running | RunState::Cancelling | RunState::Completed
    );
    let may_finish = !matches!(
        state,
        RunState::Queued | RunState::AwaitingApproval | RunState::Running | RunState::Cancelling
    );
    let needs_finish = matches!(
        state,
        RunState::Completed | RunState::Partial | RunState::Failed | RunState::Cancelled
    );
    if (state == RunState::Queued && started) || (needs_start && !started) {
        flag(out, R, &format!("{at}/started_at"));
    }
    if (finished && !may_finish) || (needs_finish && !finished) {
        flag(out, R, &format!("{at}/finished_at"));
    }
}

/// Lowercase hexadecimal SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest.iter() {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    hex
}

/// Rule `D11-19`: a chunk's `content_digest` is the lowercase hex SHA-256 of the UTF-8 bytes of its
/// `text_redacted` exactly as decoded, that is the sanitised (control characters escaped) text that is
/// persisted. The digest of the original source, which the index does not retain, is `source.content_digest`
/// and is not checked here.
pub(crate) fn d11_19(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-19";
    let AnyStore::RagIndex(f) = doc else { return };
    for (i, c) in f.chunks.iter().enumerate() {
        if sha256_hex(c.text_redacted.as_str().as_bytes()) != c.content_digest.as_str() {
            flag(out, R, &format!("/chunks/{i}/content_digest"));
        }
    }
}

/// Number of items as `u64`, saturating.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// `D11-21` for a coverage summary at `at`: assessed files never exceed selected files, and a
/// `no_findings` assessment has no check with findings.
fn coverage_basics(at: &str, c: &CoverageSummary, out: &mut Problems) {
    const R: &str = "D11-21";
    if c.assessed_files > c.selected_files {
        flag(out, R, &format!("{at}/assessed_files"));
    }
    if c.assessment == Assessment::NoFindings && c.checks_with_findings != 0 {
        flag(out, R, &format!("{at}/assessment"));
    }
}

/// Rule `D11-21` (store part): coverage counters agree with the records they summarise.
///
/// A report: each `checks_*` counter equals the number of checks with that outcome (pending and running
/// checks are counted nowhere, see `D18-18`); `selected_files` and `missing_files` equal the selected and
/// missing counts of the plan's target snapshot for this repository and reference, when the plan holds one;
/// `assessed_files <= selected_files`; a `no_findings` assessment has no check with findings and no
/// finding (suppression is deferred, RES-06), a `findings` assessment has at least one finding. A log: the same bounds on each
/// run summary's coverage. Whether the counts match the inventory, and how reports aggregate into a run, is P07.
pub(crate) fn d11_21(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-21";
    match doc {
        AnyStore::Report(r) => {
            let c = &r.coverage;
            coverage_basics("/coverage", c, out);
            let of = |o: CheckOutcome| count(r.checks.iter().filter(|k| k.outcome == o).count());
            for (field, stored, outcome) in [
                ("checks_passed", c.checks_passed, CheckOutcome::Passed),
                (
                    "checks_with_findings",
                    c.checks_with_findings,
                    CheckOutcome::Findings,
                ),
                ("checks_failed", c.checks_failed, CheckOutcome::Failed),
                (
                    "checks_unsupported",
                    c.checks_unsupported,
                    CheckOutcome::Unsupported,
                ),
                ("checks_skipped", c.checks_skipped, CheckOutcome::Skipped),
                (
                    "checks_incomplete",
                    c.checks_incomplete,
                    CheckOutcome::Incomplete,
                ),
                (
                    "checks_cancelled",
                    c.checks_cancelled,
                    CheckOutcome::Cancelled,
                ),
            ] {
                if stored != of(outcome) {
                    flag(out, R, &format!("/coverage/{field}"));
                }
            }
            let snapshot = r
                .plan
                .target_snapshots
                .iter()
                .find(|t| t.repo_id == r.repo_id && t.selected_ref == r.selected_ref);
            if let Some(t) = snapshot {
                if c.selected_files != count(t.selected_files.len()) {
                    flag(out, R, "/coverage/selected_files");
                }
                if c.missing_files != count(t.missing_paths.len()) {
                    flag(out, R, "/coverage/missing_files");
                }
            }
            if (c.assessment == Assessment::NoFindings && !r.findings.is_empty())
                || (c.assessment == Assessment::Findings && r.findings.is_empty())
            {
                flag(out, R, "/coverage/assessment");
            }
        }
        AnyStore::Log(f) => {
            for (i, run) in f.runs.iter().enumerate() {
                coverage_basics(&format!("/runs/{i}/coverage"), &run.coverage, out);
            }
        }
        _ => {}
    }
}

/// Rule `D11-25` (store part): storage errors are not swallowed.
///
/// A persisted `StorageFailure` or `OperationFailed` event has level `error` (Architecture §6 envelope), so a
/// failure is never recorded as information. The same rule bounds the recorded resource limits, the nearest
/// rule for the owner-approved bounds (security L-1): `max_scanner_processes` is in `1..=64`,
/// `max_active_repositories` in `1..=1024` (development caps pending the FR-11 owner decision) and
/// `history_retention_days`, if set, is at least 1. Halting new tasks when a checkpoint cannot be written is
/// the writer's duty (`persistence::writer`, T10).
pub(crate) fn d11_25(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-25";
    match doc {
        AnyStore::Log(f) => {
            for (i, e) in f.events.iter().enumerate() {
                let failure = matches!(
                    e.event_type,
                    EventType::StorageFailure | EventType::OperationFailed
                );
                if failure && e.level != EventLevel::Error {
                    flag(out, R, &format!("/events/{i}/level"));
                }
            }
        }
        AnyStore::Config(f) => {
            let c = &f.config;
            if !(1..=MAX_SCANNER_PROCESSES).contains(&c.max_scanner_processes) {
                flag(out, R, "/config/max_scanner_processes");
            }
            if !(1..=MAX_ACTIVE_REPOSITORIES).contains(&c.max_active_repositories) {
                flag(out, R, "/config/max_active_repositories");
            }
            if c.history_retention_days == Some(0) {
                flag(out, R, "/config/history_retention_days");
            }
        }
        _ => {}
    }
}

/// Rule `D18-08` (store part): an artifact with a retained path holds redacted content only
/// (`contains_only_redacted_retained_content` is true). That redaction was applied to tool output and that
/// fingerprints carry no raw secret is P06/P07; the persisted text types are escaped and redacted by
/// construction.
pub(crate) fn d18_08(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-08";
    let AnyStore::Report(r) = doc else { return };
    for (i, a) in r.artifacts.iter().enumerate() {
        if a.retained_path.is_some() && !a.contains_only_redacted_retained_content {
            flag(
                out,
                R,
                &format!("/artifacts/{i}/contains_only_redacted_retained_content"),
            );
        }
    }
}

/// Rule `D18-14`: report manifests record export results truthfully.
///
/// A written JSON file has a digest and an unwritten one has none; Markdown and PDF are derived from the
/// JSON report, so neither is written without it. A written PDF has a path, a renderer version and a digest
/// that is not the digest of an empty file (verified non-empty output), and no unresolved conversion error;
/// an unwritten PDF has no digest. The digest of a written JSON file is not the empty-file digest either.
pub(crate) fn d18_14(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-14";
    let AnyStore::Log(f) = doc else { return };
    for (i, m) in f.reports.iter().enumerate() {
        let at = format!("/reports/{i}");
        let empty = |d: &Option<domain::store::values::Digest>| {
            d.as_ref().is_some_and(|d| d.as_str() == EMPTY_SHA256)
        };
        if m.json_written != m.json_digest.is_some() || (m.json_written && empty(&m.json_digest)) {
            flag(out, R, &format!("{at}/json_digest"));
        }
        if m.md_written && !m.json_written {
            flag(out, R, &format!("{at}/md_written"));
        }
        if m.pdf_written && !m.json_written {
            flag(out, R, &format!("{at}/pdf_written"));
        }
        if m.pdf_written {
            if m.pdf_path.is_none() {
                flag(out, R, &format!("{at}/pdf_path"));
            }
            if m.pdf_digest.is_none() || empty(&m.pdf_digest) {
                flag(out, R, &format!("{at}/pdf_digest"));
            }
            if m.pdf_error_redacted.is_some() {
                flag(out, R, &format!("{at}/pdf_error_redacted"));
            }
            if m.pdf_renderer_version.is_none() {
                flag(out, R, &format!("{at}/pdf_renderer_version"));
            }
        } else if m.pdf_digest.is_some() {
            flag(out, R, &format!("{at}/pdf_digest"));
        }
    }
}

/// Rule `D18-18`: a report of a terminal run (`completed`, `partial`, `failed`, `cancelled`, `interrupted`)
/// has no check still `pending` or `running`, and every `incomplete` or `cancelled` check carries a
/// non-empty explanation (`reason_redacted`).
pub(crate) fn d18_18(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-18";
    let AnyStore::Report(r) = doc else { return };
    let terminal = r.execution.is_terminal();
    for (i, c) in r.checks.iter().enumerate() {
        if terminal && matches!(c.outcome, CheckOutcome::Pending | CheckOutcome::Running) {
            flag(out, R, &format!("/checks/{i}/outcome"));
        }
        let explained = c.reason_redacted.as_ref().is_some_and(|t| !t.is_empty());
        if matches!(
            c.outcome,
            CheckOutcome::Incomplete | CheckOutcome::Cancelled
        ) && !explained
        {
            flag(out, R, &format!("/checks/{i}/reason_redacted"));
        }
    }
}

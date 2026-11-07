//! Startup reconciliation (Data Schema §12.6–§12.7, request §6.5, T01-D8).
//!
//! Frozen public surface (request §5.12): [`ReconcileReport`]. The reconciliation itself runs inside
//! [`crate::writer::StoreWriter::open`], under the data-directory lock.
//!
//! The policy is deliberately conservative. The only things reconciliation *changes* are leftover temp
//! files of the application's exact temp pattern and runs that were in flight when the process stopped.
//! Everything else it finds is reported and left alone: missing and unreferenced report files, pending
//! approvals, and malformed or foreign roots (which are blocked, never repaired or rotated into `.prev`).
//!
//! The report type carries counts and identifiers only, never paths, file names or document content.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;

use domain::ids::{ReportId, RunId};
use domain::states::{ApprovalState, RunState};
use domain::store::{AnyStore, StoreKind};
use domain::time::UtcTimestamp;

use crate::atomic::is_temp_file_name;
use crate::error::{StoreError, WriteStep};
use crate::paths::StorePath;
use crate::report_name::split_report_file_name;
use crate::writer::{Env, RootState, StoreWriter, io_at};

/// What startup reconciliation found and did. Counts and IDs only: never paths, names or content.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ReconcileReport {
    /// Leftover temp files matching the app's exact temp pattern that were removed.
    pub temp_files_removed: u32,
    /// Runs found in `queued`, `awaiting_approval`, `running` or `cancelling` and set to `interrupted`.
    pub runs_interrupted: Vec<RunId>,
    /// Approvals still `pending` (left unchanged; never turned into grants).
    pub approvals_pending: u32,
    /// Report manifests in `log.json` whose JSON report file is missing (reported, not changed). Only
    /// manifests that claim `json_written = true` are checked.
    pub manifests_without_report: Vec<ReportId>,
    /// Report files in `backup/` with no manifest (reported, not changed or deleted). Only files whose
    /// name is exactly one the application generates are counted. Not computed while `log.json` is
    /// malformed, because without a readable log "no manifest" would be a guess.
    pub reports_without_manifest: u32,
    /// Roots that are malformed or foreign; writes to them are refused until `restore_previous`.
    pub malformed_roots: Vec<StoreKind>,
}

/// A directory that can hold temp files of the application.
#[derive(Clone, Copy, Debug)]
enum TempDir {
    /// The data directory itself: the single-segment store file names.
    Root,
    /// `rag/`: `index.json`, `conversations.json`.
    Rag,
    /// `backup/`: generated report file names.
    Backup,
}

impl TempDir {
    const ALL: [Self; 3] = [Self::Root, Self::Rag, Self::Backup];

    fn sub_dir(self) -> Option<&'static str> {
        match self {
            Self::Root => None,
            Self::Rag => Some("rag"),
            Self::Backup => Some("backup"),
        }
    }

    /// The directory on disk.
    fn path(self, env: &Env) -> PathBuf {
        self.sub_dir().map_or_else(
            || env.dir.as_path().to_path_buf(),
            |sub| env.dir.as_path().join(sub),
        )
    }

    /// Whether `base` is the name of a file the application writes in this directory.
    fn knows(self, base: &str) -> bool {
        match self {
            Self::Backup => split_report_file_name(base).is_some(),
            Self::Root | Self::Rag => StoreKind::ALL
                .into_iter()
                .filter_map(StoreKind::file_name)
                .any(|name| match (name.split_once('/'), self.sub_dir()) {
                    (Some((dir, file)), Some(here)) => dir == here && file == base,
                    (None, None) => name == base,
                    _ => false,
                }),
        }
    }
}

/// The store file name a temp file name belongs to: `.<base>[.prev].tmp-<16 hex>` → `<base>`.
fn temp_base(name: &str) -> Option<&str> {
    if !is_temp_file_name(name) {
        return None;
    }
    let (stem, _) = name.strip_prefix('.')?.rsplit_once(".tmp-")?;
    Some(stem.strip_suffix(".prev").unwrap_or(stem))
}

/// The entries of the sub-directory `dir` of the data directory; none if it does not exist.
fn list(env: &Env, dir: TempDir) -> Result<Vec<OsString>, StoreError> {
    match env.fs.list_dir(&dir.path(env)) {
        Ok(names) => Ok(names),
        Err(e) if dir.sub_dir().is_some() && e.kind() == std::io::ErrorKind::NotFound => {
            Ok(Vec::new())
        }
        Err(e) => Err(io_at(WriteStep::Read)(e)),
    }
}

/// Removes leftover temp files named exactly `.<known name>[.prev].tmp-<16 lowercase hex>` in the data
/// directory, `rag/` and `backup/`, and returns how many were removed. Only regular files are removed
/// (re-checked immediately before each removal, without following symlinks); a symlink, directory or
/// other entry with such a name, a name of another shape, or a non-UTF-8 name is left untouched.
fn remove_leftover_temps(env: &Env) -> Result<u32, StoreError> {
    let mut removed = 0u32;
    for dir in TempDir::ALL {
        let base_path = dir.path(env);
        for entry in list(env, dir)? {
            let Some(name) = entry.to_str() else { continue };
            if !temp_base(name).is_some_and(|base| dir.knows(base)) {
                continue;
            }
            let path = base_path.join(name);
            match env.fs.symlink_metadata(&path) {
                Ok(info) if info.is_regular() => {}
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_at(WriteStep::Read)(e)),
            }
            match env.fs.remove_file(&path) {
                Ok(()) => removed = removed.saturating_add(1),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                // There is no dedicated removal step; `Replace` (the step that publishes a file) is used.
                Err(e) => return Err(io_at(WriteStep::Replace)(e)),
            }
        }
    }
    Ok(removed)
}

/// Whether the manifest path `json_path` names an existing regular report file. Only `backup/<generated
/// name>` is ever looked at; any other value cannot be a report this application wrote and counts as
/// missing without touching the file system.
fn report_file_present(env: &Env, json_path: &str) -> Result<bool, StoreError> {
    let Some(name) = json_path.strip_prefix("backup/") else {
        return Ok(false);
    };
    let Ok(path) = StorePath::report(name) else {
        return Ok(false);
    };
    match env.fs.symlink_metadata(&env.dir.join(&path)) {
        Ok(info) => Ok(info.is_regular()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_at(WriteStep::Read)(e)),
    }
}

/// `usize` count as the report's `u32`, saturating.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Runs startup reconciliation on a freshly locked writer (request §6.5). See the module documentation and
/// [`StoreWriter::open`].
///
/// Interrupted runs get `execution = interrupted` (a transition the state machine allows from each of the
/// four in-flight states) and `finished_at` = the reconciliation time, raised to the latest timestamp the
/// run already carries so that "finish is not before start" (Data Schema §11.18) still holds if the clock
/// moved backwards. `RunSummary` has no reason field and the writer does not append events, so the reason
/// is the state itself.
pub(crate) fn reconcile(writer: &mut StoreWriter) -> Result<ReconcileReport, StoreError> {
    let env = writer.env();
    let temp_files_removed = remove_leftover_temps(&env)?;

    let mut malformed_roots = Vec::new();
    let mut approvals_pending = 0u32;
    let mut log = None;
    for (kind, state) in writer.scan_roots()? {
        match (kind, state) {
            (_, RootState::Malformed) => malformed_roots.push(kind),
            (StoreKind::Approvals, RootState::Valid(doc)) => {
                if let AnyStore::Approvals(file) = *doc {
                    approvals_pending = count(
                        file.approvals
                            .iter()
                            .filter(|a| a.status == ApprovalState::Pending)
                            .count(),
                    );
                }
            }
            (StoreKind::Approvals, RootState::Valid15(doc)) => {
                if let domain::store::OperationalStore15::Approvals(file) = *doc {
                    approvals_pending = count(
                        file.approvals
                            .iter()
                            .filter(|a| a.status == ApprovalState::Pending)
                            .count(),
                    );
                }
            }
            (StoreKind::Log, RootState::Valid(doc)) => {
                if let AnyStore::Log(file) = *doc {
                    log = Some(file);
                }
            }
            (StoreKind::Log, RootState::Valid15(doc)) => {
                if let domain::store::OperationalStore15::Log(file) = *doc {
                    log = Some(domain::store::log::LogFile {
                        schema_version: domain::store::SchemaVersion,
                        store_type: file.store_type,
                        updated_at: file.updated_at,
                        runs: file.runs,
                        events: file.events,
                        reports: file.reports,
                    });
                }
            }
            _ => {}
        }
    }
    let log_readable = !malformed_roots.contains(&StoreKind::Log);

    let mut runs_interrupted = Vec::new();
    // (report id, manifest json_path, json_written) for the report cross-check below.
    let mut manifests = Vec::new();
    if let Some(mut file) = log {
        let now = UtcTimestamp::now();
        for run in &mut file.runs {
            let in_flight = matches!(
                run.execution,
                RunState::Queued
                    | RunState::AwaitingApproval
                    | RunState::Running
                    | RunState::Cancelling
            );
            if in_flight && let Ok(next) = run.execution.transition(RunState::Interrupted) {
                let latest = [
                    Some(now),
                    Some(run.created_at),
                    run.started_at,
                    run.finished_at,
                    run.last_checkpoint_at,
                ]
                .into_iter()
                .flatten()
                .max()
                .unwrap_or(now);
                run.execution = next;
                run.finished_at = Some(latest);
                runs_interrupted.push(run.run_id);
            }
        }
        manifests = file
            .reports
            .iter()
            .map(|m| (m.report_id, m.json_path.as_str().to_owned(), m.json_written))
            .collect();
        if !runs_interrupted.is_empty() {
            file.updated_at = now;
            if writer.is_v15() {
                writer.replace15(&domain::store::OperationalStore15::Log(
                    domain::store::roots15::LogFile {
                        schema_version: domain::store::SchemaVersion15,
                        store_type: file.store_type,
                        updated_at: file.updated_at,
                        runs: file.runs,
                        events: file.events,
                        reports: file.reports,
                    },
                ))?;
            } else {
                writer.replace(&AnyStore::Log(file))?;
            }
        }
    }

    let mut manifests_without_report = Vec::new();
    for (report_id, json_path, json_written) in &manifests {
        if *json_written && !report_file_present(&env, json_path)? {
            manifests_without_report.push(*report_id);
        }
    }

    let mut reports_without_manifest = 0usize;
    if log_readable {
        let referenced: HashSet<&str> = manifests.iter().map(|(_, p, _)| p.as_str()).collect();
        for entry in list(&env, TempDir::Backup)? {
            let Some(name) = entry.to_str() else { continue };
            if !split_report_file_name(name).is_some_and(|(_, ext)| ext == "json")
                || referenced.contains(format!("backup/{name}").as_str())
            {
                continue;
            }
            if backup_entry_is_regular(&env, name)? {
                reports_without_manifest += 1;
            }
        }
    }

    Ok(ReconcileReport {
        temp_files_removed,
        runs_interrupted,
        approvals_pending,
        manifests_without_report,
        reports_without_manifest: count(reports_without_manifest),
        malformed_roots,
    })
}

/// Whether `backup/<name>` is a regular file (a symlink or directory with a report-like name is not a
/// report).
fn backup_entry_is_regular(env: &Env, name: &str) -> Result<bool, StoreError> {
    let path = env.dir.as_path().join("backup").join(name);
    match env.fs.symlink_metadata(&path) {
        Ok(info) => Ok(info.is_regular()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_at(WriteStep::Read)(e)),
    }
}

//! Semantic rule bodies for workflows, plans and tool manifests (Data Schema §11 rules 7, 8, 9, 11 and §18
//! rules 1, 3, 4, 6a, 7).
//!
//! * `D11-07` selector mode agrees with its paths and patterns.
//! * `D11-08` enabled workflows and plan snapshots are runnable.
//! * `D11-09` schedule fields match the schedule kind.
//! * `D11-11` renderer parameters cannot carry shell commands.
//! * `D18-01` tool manifests describe a verifiable, bounded tool (the tool resource bounds live here).
//! * `D18-03` a `none` network policy lists no network resource.
//! * `D18-04` exit codes are evidence: unique exit mapping, consistent native status on checks.
//! * `D18-06a` deferred values (`secret_scope = history`, deferred capabilities) are rejected.
//! * `D18-07` file selections keep their identity and disclosed context.
//!
//! Every function is pure over one [`AnyStore`] and ignores the roots its rule does not concern.

use std::collections::HashSet;

use domain::contract::{AdapterId, ExportFormat, SelectorMode, ToolAvailability};
use domain::states::CheckOutcome;
use domain::store::AnyStore;
use domain::store::evidence::ArtifactKind;
use domain::store::plan::{
    DiscoveryState, EngineAvailability, FileRecord, RepoTarget, ScanSettings, SecretScope,
};
use domain::store::report::NativeStatus;
use domain::store::tools::{LicenseReviewStatus, NetworkPolicy, ToolDistribution};
use domain::store::workflow::ScheduleKind;

use super::rules::{duplicates, flag, plans};
use crate::validate::Problems;

/// Largest accepted `ToolManifest.timeout_seconds`: 24 hours. Development default pending the FR-11 owner
/// decision.
pub(crate) const MAX_TOOL_TIMEOUT_SECONDS: u64 = 86_400;

/// Largest accepted `ToolManifest.max_output_bytes`: 64 MiB. Development default pending the FR-11 owner
/// decision.
pub(crate) const MAX_TOOL_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;

/// Every repository target of the document with its pointer and whether it is runnable: the targets of a
/// workflow (runnable when the workflow is enabled) and of every plan snapshot (always runnable).
fn targets(doc: &AnyStore) -> Vec<(String, &RepoTarget, bool)> {
    let mut found = Vec::new();
    if let AnyStore::Workflows(f) = doc {
        for (i, w) in f.workflows.iter().enumerate() {
            found.extend(
                w.targets
                    .iter()
                    .enumerate()
                    .map(|(j, t)| (format!("/workflows/{i}/targets/{j}"), t, w.enabled)),
            );
        }
    }
    for (prefix, plan) in plans(doc) {
        found.extend(
            plan.targets
                .iter()
                .enumerate()
                .map(|(j, t)| (format!("{prefix}/targets/{j}"), t, true)),
        );
    }
    found
}

/// Every `ScanSettings` of the document with its pointer: those of workflows and of plan snapshots.
fn scan_settings(doc: &AnyStore) -> Vec<(String, &ScanSettings)> {
    let mut found = Vec::new();
    if let AnyStore::Workflows(f) = doc {
        found.extend(
            f.workflows
                .iter()
                .enumerate()
                .map(|(i, w)| (format!("/workflows/{i}/scan_settings"), &w.scan_settings)),
        );
    }
    for (prefix, plan) in plans(doc) {
        found.push((format!("{prefix}/scan_settings"), &plan.scan_settings));
    }
    found
}

/// Rule `D11-07`: the selector mode agrees with the paths and patterns it carries (workflow targets and plan
/// targets).
///
/// `all_yaml` has no explicit paths and no include patterns (exclude patterns may narrow it); `explicit`
/// lists exact paths, without duplicates, and no patterns; `patterns` has no explicit paths. Whether a
/// selection may be empty is `D11-08`.
pub(crate) fn d11_07(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-07";
    for (at, t, _) in targets(doc) {
        let s = &t.selector;
        match s.mode {
            SelectorMode::AllYaml => {
                if !s.paths.is_empty() {
                    flag(out, R, &format!("{at}/selector/paths"));
                }
                if !s.include_patterns.is_empty() {
                    flag(out, R, &format!("{at}/selector/include_patterns"));
                }
            }
            SelectorMode::Explicit => {
                if !s.include_patterns.is_empty() {
                    flag(out, R, &format!("{at}/selector/include_patterns"));
                }
                if !s.exclude_patterns.is_empty() {
                    flag(out, R, &format!("{at}/selector/exclude_patterns"));
                }
                duplicates(out, R, s.paths.iter().map(|p| p.as_str()), |j| {
                    format!("{at}/selector/paths/{j}")
                });
            }
            SelectorMode::Patterns => {
                if !s.paths.is_empty() {
                    flag(out, R, &format!("{at}/selector/paths"));
                }
            }
        }
    }
}

/// Rule `D11-08` (store part): runnable workflows and plans are complete.
///
/// Every workflow has `revision >= 1` and no repeated output format. An enabled workflow and every plan
/// snapshot has at least one target and one category; an `explicit` target lists at least one path and a
/// `patterns` target at least one include pattern. An enabled workflow produces at least one output format
/// and JSON is among them (the JSON report is the authoritative evidence, Data Schema §12). Installed and
/// acknowledged capabilities are checked against `tools.json` and `approvals.json` in P14.
pub(crate) fn d11_08(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-08";
    if let AnyStore::Workflows(f) = doc {
        for (i, w) in f.workflows.iter().enumerate() {
            if w.revision == 0 {
                flag(out, R, &format!("/workflows/{i}/revision"));
            }
            duplicates(out, R, w.output_formats.iter().copied(), |j| {
                format!("/workflows/{i}/output_formats/{j}")
            });
            if w.enabled {
                if w.targets.is_empty() {
                    flag(out, R, &format!("/workflows/{i}/targets"));
                }
                if w.scan_settings.categories.is_empty() {
                    flag(out, R, &format!("/workflows/{i}/scan_settings/categories"));
                }
                if !w.output_formats.contains(&ExportFormat::Json) {
                    flag(out, R, &format!("/workflows/{i}/output_formats"));
                }
            }
        }
    }
    for (prefix, plan) in plans(doc) {
        if plan.targets.is_empty() {
            flag(out, R, &format!("{prefix}/targets"));
        }
        if plan.scan_settings.categories.is_empty() {
            flag(out, R, &format!("{prefix}/scan_settings/categories"));
        }
    }
    for (at, t, runnable) in targets(doc) {
        let s = &t.selector;
        if !runnable {
            continue;
        }
        match s.mode {
            SelectorMode::Explicit if s.paths.is_empty() => {
                flag(out, R, &format!("{at}/selector/paths"));
            }
            SelectorMode::Patterns if s.include_patterns.is_empty() => {
                flag(out, R, &format!("{at}/selector/include_patterns"));
            }
            _ => {}
        }
    }
}

/// Rule `D11-09` (store part): schedule fields match the schedule kind; inapplicable values are null.
///
/// `manual` has no interval, no daily time and no next due time; `interval` has `interval_seconds >= 1` and
/// no daily time; `daily` has a daily time and no interval. Syntax of the time and zone is enforced by the
/// types; membership of the zone in the time-zone database is P14.
pub(crate) fn d11_09(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-09";
    let AnyStore::Workflows(f) = doc else { return };
    for (i, w) in f.workflows.iter().enumerate() {
        let s = &w.schedule;
        let at = format!("/workflows/{i}/schedule");
        match s.kind {
            ScheduleKind::Manual => {
                if s.interval_seconds.is_some() {
                    flag(out, R, &format!("{at}/interval_seconds"));
                }
                if s.daily_local_time.is_some() {
                    flag(out, R, &format!("{at}/daily_local_time"));
                }
                if s.next_due_at.is_some() {
                    flag(out, R, &format!("{at}/next_due_at"));
                }
            }
            ScheduleKind::Interval => {
                if s.interval_seconds.is_none_or(|n| n == 0) {
                    flag(out, R, &format!("{at}/interval_seconds"));
                }
                if s.daily_local_time.is_some() {
                    flag(out, R, &format!("{at}/daily_local_time"));
                }
            }
            ScheduleKind::Daily => {
                if s.daily_local_time.is_none() {
                    flag(out, R, &format!("{at}/daily_local_time"));
                }
                if s.interval_seconds.is_some() {
                    flag(out, R, &format!("{at}/interval_seconds"));
                }
            }
        }
    }
}

/// Whether `value` contains shell command syntax: a backtick, `$(`, `${`, `;`, `|` or `&`.
fn has_shell_syntax(value: &str) -> bool {
    value.contains(['`', ';', '|', '&']) || value.contains("$(") || value.contains("${")
}

/// Rule `D11-11` (store part): no renderer parameter value contains shell command syntax (a backtick,
/// `$(`, `${`, `;`, `|` or `&`). This is a heuristic on the persisted text, not a proof that a value is
/// harmless. The allowlists for schema resources and renderer IDs are P08/P09. The pointer ends at
/// `parameters_redacted`: parameter names come from the document and are not used in pointers.
pub(crate) fn d11_11(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D11-11";
    for (at, s) in scan_settings(doc) {
        for (k, r) in s.renderers.iter().enumerate() {
            if r.parameters_redacted
                .values()
                .any(|v| has_shell_syntax(v.as_str()))
            {
                flag(out, R, &format!("{at}/renderers/{k}/parameters_redacted"));
            }
        }
    }
}

/// Rule `D18-01` (store part): a tool manifest describes a verifiable, bounded tool.
///
/// Every manifest: `timeout_seconds` is in `1..=86_400` and `max_output_bytes` in `1..=64 MiB` (development
/// caps pending the FR-11 owner decision; the nearest rule for these recorded bounds); a `bundled` tool has
/// a binary digest; a `missing` or `deferred` tool has no version (no invented version); an `approved`
/// licence names its SPDX expression, verified version and review time. A `ready` tool additionally has a
/// version, a non-zero binary digest, a shipped or prerequisite distribution, a non-empty exit mapping, an
/// `approved` licence review and a licence verified for exactly its version. Plan engine snapshots follow the
/// same version rules. Installed-version compatibility and binary identity are verified at preflight (P06/P13).
pub(crate) fn d18_01(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-01";
    if let AnyStore::ToolManifest(f) = doc {
        for (i, t) in f.tools.iter().enumerate() {
            let at = format!("/tools/{i}");
            if !(1..=MAX_TOOL_TIMEOUT_SECONDS).contains(&t.timeout_seconds) {
                flag(out, R, &format!("{at}/timeout_seconds"));
            }
            if !(1..=MAX_TOOL_OUTPUT_BYTES).contains(&t.max_output_bytes) {
                flag(out, R, &format!("{at}/max_output_bytes"));
            }
            if t.distribution == ToolDistribution::Bundled && t.binary_sha256.is_none() {
                flag(out, R, &format!("{at}/binary_sha256"));
            }
            if matches!(
                t.availability,
                ToolAvailability::Missing | ToolAvailability::Deferred
            ) && t.tool_version.is_some()
            {
                flag(out, R, &format!("{at}/tool_version"));
            }
            let lic = &t.license;
            if lic.review_status == LicenseReviewStatus::Approved {
                if lic.spdx.is_none() {
                    flag(out, R, &format!("{at}/license/spdx"));
                }
                if lic.verified_version.is_none() {
                    flag(out, R, &format!("{at}/license/verified_version"));
                }
                if lic.reviewed_at.is_none() {
                    flag(out, R, &format!("{at}/license/reviewed_at"));
                }
            }
            if t.availability == ToolAvailability::Ready {
                if t.tool_version.is_none() {
                    flag(out, R, &format!("{at}/tool_version"));
                }
                let placeholder = t
                    .binary_sha256
                    .as_ref()
                    .is_none_or(|d| d.as_str().bytes().all(|b| b == b'0'));
                if placeholder {
                    flag(out, R, &format!("{at}/binary_sha256"));
                }
                if t.distribution == ToolDistribution::NotShipped {
                    flag(out, R, &format!("{at}/distribution"));
                }
                if t.exit_mapping.is_empty() {
                    flag(out, R, &format!("{at}/exit_mapping"));
                }
                if lic.review_status != LicenseReviewStatus::Approved {
                    flag(out, R, &format!("{at}/license/review_status"));
                }
                if lic.verified_version != t.tool_version {
                    flag(out, R, &format!("{at}/license/verified_version"));
                }
            }
        }
    }
    for (prefix, plan) in plans(doc) {
        for (k, e) in plan.engine_snapshots.iter().enumerate() {
            let versionless = matches!(
                e.availability,
                EngineAvailability::Missing | EngineAvailability::Deferred
            );
            let ready = e.availability == EngineAvailability::Ready;
            if (versionless && e.tool_version.is_some()) || (ready && e.tool_version.is_none()) {
                flag(
                    out,
                    R,
                    &format!("{prefix}/engine_snapshots/{k}/tool_version"),
                );
            }
        }
    }
}

/// Rule `D18-03` (store part): a manifest with network policy `none` lists no network resource, and no
/// manifest lists a resource twice. That the policy is enforced by tool configuration or isolation is
/// P06/P13; a label is not a firewall.
pub(crate) fn d18_03(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-03";
    let AnyStore::ToolManifest(f) = doc else {
        return;
    };
    for (i, t) in f.tools.iter().enumerate() {
        if t.network_policy == NetworkPolicy::None && !t.network_resources.is_empty() {
            flag(out, R, &format!("/tools/{i}/network_resources"));
        }
        duplicates(
            out,
            R,
            t.network_resources.iter().map(|d| d.as_str()),
            |j| format!("/tools/{i}/network_resources/{j}"),
        );
    }
}

/// Rule `D18-04` (store part): the exit code is evidence and the native outcome is consistent.
///
/// A manifest's exit mapping lists each exit code once. A check has the same adapter as its engine, and
/// `term_signal >= 1`; it never has both an exit code and a terminating signal; a tool that was not started
/// or could not be launched has neither; a tool that exited has an exit code (except the in-process
/// `rust-schema` adapter); a `passed` check exited. Agreement of the parser with the manifest's mapping is P06.
pub(crate) fn d18_04(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-04";
    match doc {
        AnyStore::ToolManifest(f) => {
            for (i, t) in f.tools.iter().enumerate() {
                duplicates(out, R, t.exit_mapping.iter().map(|e| e.code), |j| {
                    format!("/tools/{i}/exit_mapping/{j}/code")
                });
            }
        }
        AnyStore::Report(r) => {
            for (i, c) in r.checks.iter().enumerate() {
                let at = format!("/checks/{i}");
                if c.adapter_id != c.engine.adapter_id {
                    flag(out, R, &format!("{at}/adapter_id"));
                }
                if c.term_signal == Some(0) || (c.exit_code.is_some() && c.term_signal.is_some()) {
                    flag(out, R, &format!("{at}/term_signal"));
                }
                match c.native_status {
                    NativeStatus::NotStarted | NativeStatus::LaunchFailed => {
                        if c.exit_code.is_some() {
                            flag(out, R, &format!("{at}/exit_code"));
                        }
                        if c.term_signal.is_some() {
                            flag(out, R, &format!("{at}/term_signal"));
                        }
                    }
                    NativeStatus::Exited
                        if c.exit_code.is_none() && c.adapter_id != AdapterId::RustSchema =>
                    {
                        flag(out, R, &format!("{at}/exit_code"));
                    }
                    _ => {}
                }
                if c.outcome == CheckOutcome::Passed && c.native_status != NativeStatus::Exited {
                    flag(out, R, &format!("{at}/native_status"));
                }
            }
        }
        _ => {}
    }
}

/// Rule `D18-06a`: pilot semantic validation rejects deferred values. Any `ScanSettings.secret_scope` of
/// `history` (SCAN-09), in a workflow or a plan snapshot, and any approval for the deferred capabilities
/// `non_mutating_verification`, `documentation_fetch` and `embedding_transmission`, whatever its status. The
/// rule ID is the explicit deferred reason; nothing is silently dropped.
pub(crate) fn d18_06a(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-06a";
    for (at, s) in scan_settings(doc) {
        if s.secret_scope == SecretScope::History {
            flag(out, R, &format!("{at}/secret_scope"));
        }
    }
    if let AnyStore::Approvals(f) = doc {
        for (i, a) in f.approvals.iter().enumerate() {
            if a.capability.is_deferred() {
                flag(out, R, &format!("/approvals/{i}/capability"));
            }
        }
    }
}

/// `D18-07` for one inventory or snapshot file at `at`: an eligible file has a content digest and an
/// unreadable one has none.
fn file_record(at: &str, f: &FileRecord, out: &mut Problems) {
    const R: &str = "D18-07";
    let wrong = match f.discovery_state {
        DiscoveryState::Eligible => f.content_digest.is_none(),
        DiscoveryState::Unreadable => f.content_digest.is_some(),
        DiscoveryState::Excluded => false,
    };
    if wrong {
        flag(out, R, &format!("{at}/content_digest"));
    }
}

/// Rule `D18-07` (store part): file selections keep their identity and disclosed context.
///
/// Inventory and snapshot files: an `eligible` file has a content digest and an `unreadable` file has none.
/// A target snapshot selects only `eligible` files, lists each path once and does not also list a selected
/// path as missing (non-YAML context is disclosed, never a silent extra selected file). A rendered artifact
/// names the sources it was generated from. One-based locations are `D11-20`; the working-tree digest check
/// against the checkout is P05/P07.
pub(crate) fn d18_07(doc: &AnyStore, out: &mut Problems) {
    const R: &str = "D18-07";
    if let AnyStore::Repositories(f) = doc {
        for (i, r) in f.repositories.iter().enumerate() {
            for (j, file) in r.files.iter().enumerate() {
                file_record(&format!("/repositories/{i}/files/{j}"), file, out);
            }
        }
    }
    for (prefix, plan) in plans(doc) {
        for (k, t) in plan.target_snapshots.iter().enumerate() {
            let at = format!("{prefix}/target_snapshots/{k}");
            let missing: HashSet<&str> = t.missing_paths.iter().map(|p| p.as_str()).collect();
            for (m, file) in t.selected_files.iter().enumerate() {
                let file_at = format!("{at}/selected_files/{m}");
                file_record(&file_at, file, out);
                if file.discovery_state != DiscoveryState::Eligible {
                    flag(out, R, &format!("{file_at}/discovery_state"));
                }
                if missing.contains(file.path.as_str()) {
                    flag(out, R, &format!("{file_at}/path"));
                }
            }
            duplicates(
                out,
                R,
                t.selected_files.iter().map(|f| f.path.as_str()),
                |m| format!("{at}/selected_files/{m}/path"),
            );
        }
    }
    if let AnyStore::Report(r) = doc {
        for (i, a) in r.artifacts.iter().enumerate() {
            if a.kind == ArtifactKind::Rendered && a.source_refs.is_empty() {
                flag(out, R, &format!("/artifacts/{i}/source_refs"));
            }
        }
    }
}

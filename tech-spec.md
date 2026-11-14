Version: 0.1 draft | Date: 2026-11-12, updated 2026-11-13 (native OpenCode integration), 2026-11-14 (1.1.0) | Status: implemented baseline (Phases 1–4 + 1.1.0); §9 is the shipped MCP contract; the GUI comparison view and full Phase 5 UI remain planned

# PWT Technical Specification

## 1. Proposed stack
- Language/runtime: Java; JDK 21 baseline proposed for the initial compatibility spike, not a claim that it is the latest JDK.
- Desktop: compatible OpenJFX release selected and pinned after the spike; JavaFX CSS for terminal-inspired styling.
- Build: Maven Wrapper; pinned dependencies and plugins; no floating latest versions.
- Storage: SQLite JDBC with numbered migrations.
- Serialization: Jackson version compatible with the selected MCP SDK, validated before pinning.
- MCP: official Java SDK, stdio transport.
- Tests: JUnit plus temporary-directory, subprocess-fixture, database, GUI-smoke and real-tool integration tests.
- Distribution: runtime-inclusive package using jpackage after platform validation.
No Spring Boot, Electron, embedded browser, cloud inference, or Rust runtime dependency is required.

## 2. Launch contract
```text
pwt gui [--data-dir <directory>]
pwt mcp [--data-dir <directory>] [--allow-scans] [--allow-analysis-writes] [--repo-id <id> | --project-root <abs-dir>]
pwt scan --root <dir> --file <relative>... [--adapter id,...] [--data-dir <directory>] [--export] [...]
pwt repo-add --root <dir> [--data-dir <directory>]
pwt repo-list [--data-dir <directory>]
pwt doctor [--data-dir <directory>]
pwt mcp-status [--data-dir <directory>]
```
CLI scan is a headless validation path using the same services; no independent scanner implementation. `--repo-id` restricts every MCP operation to one registered project; `--project-root` binds to the registered project owning that directory, resolved per call and pinned per session (see architecture §6); the two are mutually exclusive and unscoped mode is explicit local admin. The shared launcher `integrations/pwt-mcp-stdio.sh` adds `--project-root-cwd` (bind to the directory it was started in) and resolves the artifact with one policy for every entrypoint: `--jar`/`PWT_JAR`, else `pwt/target/pwt-<version declared in pwt/pom.xml>.jar`, else the single candidate jar, else an error listing candidates. `--export` is the only scan-time file writer; without it the database is authoritative and no report files exist.
Use an explicit application data directory resolved consistently by all modes. No implicit repository .env or working-directory config execution. CLI emits sanitized human output or a machine-readable result JSON; MCP stdout is reserved for protocol messages.

## 3. Adapter interface
```java
interface ScannerAdapter {
    AdapterDescriptor descriptor();
    PreflightResult preflight(ExecutionContext context);
    ScanPlan plan(TargetManifest targets, AdapterConfig config);
    AdapterResult execute(ScanPlan plan, ProcessRunner runner,
                          CancellationToken cancellation);
}
```
These are illustrative contracts; exact Java types are settled in Phase 1. Descriptor includes adapter ID, tool name, supported version ranges, capability tags, accepted file families, scope mode, and network requirement.
Preflight never installs tools. Distinguish missing executable, unsupported version, invalid configuration, no applicable target, and probe failure.
Each adapter documents scanner exit codes; nonzero does not universally mean execution failure. Fixture tests must cover clean, findings, malformed output, unknown exit code, timeout, cancellation, and unavailable tool.

## 4. Process runner
Use ProcessBuilder with a validated executable and argument list. Working directory is the private run snapshot. Construct a minimal environment; do not inherit credentials unnecessarily. Preserve OS variables only where required and documented.
Initial configurable limits: 2 workers/process, 120 seconds/tool, 10 MiB stdout, 2 MiB stderr, 5 MiB/file, 50 MiB selected snapshot, 1,000 selected files. These are proposed defensive defaults, not measured performance targets.
Read both output streams concurrently with hard byte limits. Limit breach produces FAILED/OUTPUT_LIMIT and terminates the process; never silently truncate parsable JSON and call it valid. Terminate process descendants on timeout/cancel; test cleanup per OS and disclose residual limitations.
No automatic scanner downloads, scripts from the assessed repo, package hooks, or network-required scans. Network-disabled configuration is not an OS sandbox; do not promise enforced egress isolation.

## 5. YAML handling
Use a maintained safe parser with explicit alias, nesting, input-size and document-count limits; select and pin after testing. Reject unsafe object construction. Support multi-document YAML deliberately.
Syntax checks identify parse errors; schema checks require an explicit schema and dialect. Do not infer full CI/CD security validity from syntax success.
Generated target manifests record repository-relative paths, SHA-256 of exact copied bytes, sizes, and selection. Snapshot layout preserves relative paths; adapters declare whether extra context is required. Missing context means skipped or partial coverage, not an invented clean result.

## 6. Findings and redaction
Normalize severities to CRITICAL/HIGH/MEDIUM/LOW/INFO/UNKNOWN while retaining tool severity. Severity is not a probability or aggregate vulnerability score.
Redact known sensitive fields and secret matches before persistence, logging, UI events, and MCP responses. For sensitive findings prefer metadata/location without code excerpts. Disable raw evidence storage in MVP.
Redaction is defense in depth, not a mathematical guarantee. Test seeded secrets across every output sink, including exception paths and parser failures. Never embed secret values in fingerprints; use adapter/rule/path and non-sensitive occurrence identity.
Fingerprint version 1: SHA-256 of normalized adapter ID, rule ID, relative path, and non-sensitive logical key or location fallback. Document location-shift limitations.

## 7. Run semantics
Execution state: CREATED -> RUNNING -> COMPLETED | PARTIAL | FAILED | CANCELLED | INTERRUPTED.
COMPLETED means every required applicable scanner finished successfully; findings may exist. PARTIAL means at least one successful assessment and at least one required applicable check unavailable, failed, or skipped. FAILED means no required applicable check completed successfully. CANCELLED/INTERRUPTED override completion while preserving completed evidence.
Assessment verdict is a separate field: FINDINGS, NO_FINDINGS, or INDETERMINATE. Findings can coexist with partial coverage. NO_FINDINGS requires complete declared coverage; otherwise absence of findings is INDETERMINATE. Coverage is relative to configured adapters and selected files, never a guarantee of repository security.
If no selected target or no applicable required scanner exists, reject before execution or return an explicit indeterminate terminal record; never report a clean scan.

## 8. Persistence and export
See data-schema.md for normative entity fields and invariants. Commit findings per scanner transaction. Finalize run state durably; scan completion writes no report files. Explicit export (`scan --export`, GUI Export) writes `<data-dir>/artifacts/<run-id>/report.{json,md}` atomically (temp file, flush, rename) and records READY/FAILED artifacts independently; export failure never erases scan results.
Write temp files inside the destination data directory, flush, then atomic rename where supported. Reports contain relative paths by default, provenance, tool versions, target hashes, findings, and coverage gaps.
On-demand rendering (`get_report`, GUI View) uses retained sanitized data only and requires no files. Report schema and database migration versions are independent.

## 9. MCP contract
| Tool | Input | Result / constraint |
|---|---|---|
| pwt_info | — | Identity, capabilities, planned scanner catalog |
| list_repositories | cursor?, limit? | Registered IDs, names, availability; scoped servers return only their project; no arbitrary registration |
| list_runs | repository_id?, state?, cursor?, limit? | Paginated summaries with finding/target counts; repository_id optional when the server is project-scoped |
| get_run | run_id | Status, coverage, snapshot hashes, scanner outcomes; each scanner carries `execution` (executed, execution_failure for FAILED/TIMED_OUT/INTERRUPTED, not_executed, tool_unavailable, cancelled, in_progress); run carries `final` (false = active, provisional) and `recorded_error`; cross-project IDs are NOT_FOUND under scope |
| get_project_status | repository_id? (unscoped admin only) | Bound project identity and root availability, binding mode, run count, latest run brief (final or provisional), active runs, journal path/availability, effective server permissions; never registers or scans |
| compare_runs | base_run_id, head_run_id, limit? | Two runs of one repository by recorded fingerprint: only_in_base / only_in_head (each with `comparable` and a reason), in_both, possible_line_shifts (same adapter/rule/file, different line; candidates only), selected-file and content-hash differences, per-adapter status in both runs, explicit limitations; never labels anything fixed |
| list_findings | run_id, severity?, adapter_id?, rule_id?, relative_path?, cursor?, limit? | Sanitized structured evidence, stable order; filters are exact match, bounded (≤512 chars, no control characters); evidence over 4,000 chars is returned as a capped excerpt |
| get_report | run_id, format, offset?, length? | JSON or Markdown rendered from persisted records; bounded chunks with `next_offset`; files never required |
| start_scan | repository_id, relative_paths[], adapter_ids[]?, bindings/... | Requires --allow-scans; validated IDs/scope; empty adapter set means all six (local-schema without bindings fails fast with CONFIGURATION_REQUIRED); returns run ID promptly |
| cancel_scan | run_id | Requires scan permission; reports actual state (already-terminal is honest, not an error) |
| get_analysis | run_id, repository_id?, revision? | Current or requested revision, structured assessments, priorities, verification status, available revision list; read-only (a run without analysis returns revision null and creates nothing) |
| save_analysis | run_id, expected_revision, priority, summary?, assessments[]?, ... | Requires --allow-analysis-writes; expected_revision must equal current or CONFLICT (nothing written); cross-project references rejected |
Default list page size 100, maximum 500; bounded strings and response bytes. Report chunks default 12,000 chars, maximum 64,000, addressed by offset/length with `next_offset`. Reject unknown adapter IDs, absolute paths, traversal, and paths not inside the registered root. No shell, edit, Git push, arbitrary file read, SQL, or export-path tool.
Exposed error codes: NOT_FOUND, INVALID_INPUT, OUT_OF_SCOPE, PERMISSION_DENIED, REPOSITORY_BUSY, CONFIGURATION_REQUIRED, CONFLICT, INTERNAL_ERROR, and for project-root-bound servers PROJECT_NOT_REGISTERED, PROJECT_UNAVAILABLE, PROJECT_AMBIGUOUS, PROJECT_SCOPE_CHANGED. Sanitized errors with correlation IDs, never raw stack traces.
Run comparison is available over MCP (`compare_runs`, 1.1.0); a GUI comparison view remains planned (§10/§11). MCP SDK 0.8.1 has no tool annotations, so read-only hints are not advertised; authorization is server-side regardless.

## 10. Desktop behavior
Open repo registration dialog; discover file tree; select targets; inspect available checks; start/cancel; filter findings; view evidence; open history/reports; inspect saved agent analysis (read-only); compare selected runs (planned).
Use background executors and JavaFX-safe dispatch. Poll the shared store every second while visible with incremental bounded queries; stop polling on shutdown. Debounce file discovery; cancel stale discovery tasks.
Empty, loading, unavailable, partial, failed, stale-analysis, and permission-denied states need explicit views. Log panel consumes sanitized structured events, not a fake shell. Analysis loading shares the run-display path with scan completion; a newer scan than the displayed analysis is flagged STALE.

## 11. Comparison semantics (MCP implemented in 1.1.0; GUI view planned)

Only compare runs of the same registered repository. `compare_runs` matches findings by recorded fingerprint (SHA-256 of adapter/rule/path/line identity, multiset-aware) into only_in_base, only_in_head and in_both. Because the line is part of the identity, an edit that shifts lines changes fingerprints; same adapter/rule/file at a different line is reported separately as a possible line shift (a candidate, not a confirmed match). A one-sided finding is `comparable` only if its file was selected in both runs and its adapter executed successfully (SUCCEEDED) in both; otherwise a reason is given. Limitations list active (provisional) runs, differing file selections, changed content hashes, adapters that did not execute in both runs, and recorded coverage gaps. Not-observed is never reported as fixed: a fix is verified only by a new dashboard scan with comparable coverage. The GUI comparison view does not exist yet.

## 12. Validation and release
Pin the complete dependency/tool matrix in the first build spike. Package and test on each claimed OS. Keep scanner license notices and redistribution decisions separate from the application license. Shipping an app runtime does not automatically ship or license external scanners.
All proposed defaults remain configurable through validated local settings, not executable repository configuration.

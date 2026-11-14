Version: 0.1 draft | Date: 2026-11-12, updated 2026-11-14 (1.1.0: no schema change) | Status: implemented baseline (migrations V1–V3)

Release 1.1.0 adds no migration and no table. Its new MCP tools
(`get_project_status`, `compare_runs`) read existing rows only; project-root
binding reads `repositories` and never inserts. `analysis_threads` rows are
created only by `save_analysis` (since 1.1.0 `get_analysis` no longer creates
an empty thread on read).

# PWT Data Schema

## 1. Conventions
SQLite is authoritative. UUIDs are application-generated TEXT; UTC instants use RFC3339 TEXT; booleans use INTEGER constrained to 0/1. Paths in scan records are repository-relative forward-slash paths; registration stores a local absolute root privately.
All connections enable foreign keys. Database migration version is tracked separately from export schema_version, initially "1.0". JSON arrays/objects use validated JSON TEXT when a normalized child table provides no practical query value.
All persisted text is sanitized. Raw scanner output and raw secret values are forbidden.

## 2. Entities
### repositories
- id TEXT PRIMARY KEY
- display_name TEXT NOT NULL
- canonical_root TEXT NOT NULL UNIQUE
- root_identity_hash TEXT NOT NULL UNIQUE: hash used for lock naming, not an anonymization guarantee
- created_at TEXT NOT NULL
- updated_at TEXT NOT NULL
Availability is checked at read/use time; do not mistake a cached availability flag for authorization.

### scanner_configs
- adapter_id TEXT PRIMARY KEY
- executable_path TEXT NOT NULL
- enabled INTEGER NOT NULL
- required INTEGER NOT NULL
- timeout_seconds INTEGER NOT NULL CHECK > 0
- options_json TEXT NOT NULL: adapter-validated allowlisted keys; never shell command strings
- updated_at TEXT NOT NULL
No provider credentials. Scanner network behavior and required environment keys are declared in descriptor/config metadata, without storing secrets.

### runs
- id TEXT PRIMARY KEY
- repository_id TEXT NOT NULL REFERENCES repositories(id)
- source TEXT NOT NULL CHECK IN (GUI, CLI, MCP)
- state TEXT NOT NULL CHECK IN (CREATED, RUNNING, COMPLETED, PARTIAL, FAILED, CANCELLED, INTERRUPTED)
- verdict TEXT NOT NULL CHECK IN (FINDINGS, NO_FINDINGS, INDETERMINATE)
- created_at TEXT NOT NULL; started_at TEXT NULL; finished_at TEXT NULL
- cancel_requested INTEGER NOT NULL DEFAULT 0
- owner_instance_id TEXT NOT NULL: random process-instance UUID, not a reusable PID identity
- heartbeat_at TEXT NULL: informational; lock acquisition determines recovery authority
- manifest_sha256 TEXT NULL
- fingerprint_version INTEGER NOT NULL DEFAULT 1
- app_version TEXT NOT NULL
- config_snapshot_json TEXT NOT NULL: sanitized immutable configuration used for this run
- summary_json TEXT NOT NULL: derived counts and coverage, rebuildable from child records
- error_code TEXT NULL; error_message TEXT NULL: sanitized
- revision INTEGER NOT NULL DEFAULT 0: increment on visible changes for refresh
Indexes: (repository_id, created_at DESC), (state, created_at).

### run_files
- id TEXT PRIMARY KEY
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- relative_path TEXT NOT NULL
- content_sha256 TEXT NOT NULL
- size_bytes INTEGER NOT NULL CHECK >= 0
- detected_family TEXT NOT NULL: GENERIC_YAML, GITHUB_ACTIONS, KUBERNETES, UNKNOWN, or future documented value
- UNIQUE(run_id, relative_path)
Hash describes the exact copied bytes used by scanners. No full file content is retained in this table.

### scanner_runs
- id TEXT PRIMARY KEY
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- adapter_id TEXT NOT NULL
- tool_name TEXT NOT NULL; tool_version TEXT NULL
- status TEXT NOT NULL CHECK IN (PENDING, RUNNING, SUCCEEDED, FAILED, SKIPPED, UNAVAILABLE, TIMED_OUT, CANCELLED, INTERRUPTED)
- required INTEGER NOT NULL
- applicable INTEGER NOT NULL
- started_at TEXT NULL; finished_at TEXT NULL; exit_code INTEGER NULL
- reason_code TEXT NULL; message TEXT NULL
- scope_mode TEXT NOT NULL: SELECTED_FILES or DECLARED_CONTEXT
- effective_config_json TEXT NOT NULL: sanitized immutable adapter config
- UNIQUE(run_id, adapter_id)
SUCCEEDED includes both clean results and findings. SKIPPED needs a machine-readable reason; not applicable is distinct from missing context.

### scanner_targets
- scanner_run_id TEXT NOT NULL REFERENCES scanner_runs(id) ON DELETE CASCADE
- run_file_id TEXT NOT NULL REFERENCES run_files(id) ON DELETE CASCADE
- coverage_status TEXT NOT NULL CHECK IN (PENDING, CHECKED, FAILED, SKIPPED, UNKNOWN)
- reason_code TEXT NULL
- PRIMARY KEY(scanner_run_id, run_file_id)
Both references must belong to the same run; enforce with service validation and integration tests (or composite foreign keys). Do not infer per-file checks solely from a tool-wide exit code unless adapter semantics support it.

### findings
- id TEXT PRIMARY KEY
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- scanner_run_id TEXT NOT NULL REFERENCES scanner_runs(id) ON DELETE CASCADE
- run_file_id TEXT NULL REFERENCES run_files(id)
- rule_id TEXT NOT NULL; title TEXT NOT NULL
- severity TEXT NOT NULL CHECK IN (CRITICAL, HIGH, MEDIUM, LOW, INFO, UNKNOWN)
- original_severity TEXT NULL
- category TEXT NOT NULL: SECRET, MISCONFIGURATION, SYNTAX, SCHEMA, or documented extension
- relative_path TEXT NULL: null only for clearly marked run-level finding
- line_start INTEGER NULL; line_end INTEGER NULL; column_start INTEGER NULL
- message TEXT NOT NULL
- evidence_json TEXT NOT NULL: redacted structured metadata; sensitive excerpts omitted
- remediation TEXT NULL: advisory text, not an executable command
- reference_urls_json TEXT NOT NULL
- fingerprint TEXT NOT NULL
- created_at TEXT NOT NULL
Locations are 1-based, positive when present. Validate line_end >= line_start. Scanner, file and finding must belong to the same run. Report out-of-scope scanner findings as coverage warnings, not silently accepted file access.
Index: (run_id, severity), (run_id, fingerprint). Do not merge findings from different tools merely because titles match.

### artifacts
- id TEXT PRIMARY KEY
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- kind TEXT NOT NULL CHECK IN (JSON_REPORT, MARKDOWN_REPORT)
- status TEXT NOT NULL CHECK IN (READY, FAILED)
- relative_storage_path TEXT NULL
- content_sha256 TEXT NULL; size_bytes INTEGER NULL
- report_schema_version TEXT NOT NULL
- generated_at TEXT NOT NULL
- error_code TEXT NULL
- UNIQUE(run_id, kind)
Storage paths are relative to the application artifact root, never caller-provided. READY requires path/hash/size. FAILED may have no file.

### run_events
- sequence INTEGER PRIMARY KEY AUTOINCREMENT
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- timestamp TEXT NOT NULL
- level TEXT NOT NULL CHECK IN (DEBUG, INFO, WARN, ERROR)
- code TEXT NOT NULL; message TEXT NOT NULL
- details_json TEXT NOT NULL
Bound event count and message bytes. Never persist unfiltered scanner output. MCP connection presence is not inferred from these events.

### analysis_threads (V3, agent analysis; scanner tables untouched)
- id TEXT PRIMARY KEY
- repository_id TEXT NOT NULL REFERENCES repositories(id)
- run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE
- current_revision INTEGER NOT NULL DEFAULT 0
- created_at/updated_at TEXT NOT NULL
- UNIQUE(repository_id, run_id): one thread per assessed run.

### analysis_revisions (V3, append-only; prior revisions preserved)
- id TEXT PRIMARY KEY
- thread_id TEXT NOT NULL REFERENCES analysis_threads(id) ON DELETE CASCADE
- revision INTEGER NOT NULL; UNIQUE(thread_id, revision)
- summary TEXT (sanitized, capped); priority TEXT CHECK IN (LOW, MEDIUM, HIGH)
- priority_rationale TEXT (sanitized, capped)
- category TEXT; change_status TEXT (free-text workflow state, e.g. proposed/applied/verified)
- verification_run_id TEXT NULL REFERENCES runs(id) (must belong to the same repository)
- author_client TEXT NULL (e.g. "mcp"); author_name TEXT NULL; created_at TEXT NOT NULL
Scanner severity is never stored here and never modified by analysis writes.

### analysis_assessments (V3, structured per-finding judgments)
- id TEXT PRIMARY KEY; revision_id TEXT NOT NULL REFERENCES analysis_revisions(id) ON DELETE CASCADE
- finding_id TEXT NULL REFERENCES findings(id) (must belong to the thread's run)
- rule_id TEXT; relative_path TEXT NULL; line_start INTEGER NULL
- priority TEXT CHECK IN (LOW, MEDIUM, HIGH); rationale TEXT (sanitized, capped)
- category TEXT; change_status TEXT; verification_run_id TEXT NULL REFERENCES runs(id)
- message TEXT (sanitized, capped)
No raw secret values: agents reference findings by rule/file/line; control
characters are stripped and texts length-capped before persistence.

### mcp_sessions (V2 presence + V3 analysis_writes, additive)
- session_id TEXT PRIMARY KEY; server_instance_id TEXT NOT NULL
- client_name/version, protocol_version TEXT NULL (self-reported)
- permission_mode TEXT CHECK IN (READ_ONLY, SCANS_ENABLED)
- analysis_writes TEXT NOT NULL DEFAULT 'DENIED' (V3; values ALLOWED/DENIED written by the server from its flags, no DB CHECK constraint — enforced at the write path)
- lifecycle_state TEXT; connected_at, server_heartbeat_at, last_client_activity_at,
  disconnected_at TEXT NULL; close_reason TEXT NULL

## 3. Storage relationships
```text
repositories 1 -> N runs
runs 1 -> N run_files
runs 1 -> N scanner_runs
scanner_runs N <-> N run_files through scanner_targets
scanner_runs 1 -> N findings
runs 1 -> N artifacts
runs 1 -> N analysis_threads (V3; at most one per (repository, run))
analysis_threads 1 -> N analysis_revisions (append-only; current_revision tracks head)
analysis_revisions 1 -> N analysis_assessments (structured finding judgments)
runs 1 -> N run_events
```
Repository deletion with retained history is restricted by default. History purge is an explicit local-user action with confirmation; delete dependent rows and associated artifacts together with recoverable cleanup tracking. No MCP purge tool in MVP.

## 4. Lifecycle and invariants
Terminal runs cannot restart under the same ID. Rescan creates a new run. Terminal run evidence is immutable except regenerable artifacts and explicit deletion.
CREATED/RUNNING transitions update revision. Findings are inserted with their scanner outcome transaction. Final summary is derived, not independently edited.
Verdict NO_FINDINGS requires state COMPLETED and no findings plus complete declared coverage. Any findings yield FINDINGS even when partial. Otherwise verdict INDETERMINATE.
An optional scanner failure remains visible but does not by itself make required coverage incomplete. Required applicability is recorded at preflight. A run with no required applicable scanner cannot be called clean.
Cancel requested is cooperative and does not itself imply CANCELLED; workers record the actual terminal outcome. Persisted evidence from successful scanners survives cancellation.
Analysis staleness is advisory, never automatic: a saved revision stays attached to its source run, and readers (GUI included) flag it STALE when a newer scan exists for the same repository — the stored revision is never rewritten or deleted by later scans. Stale `save_analysis` calls fail with CONFLICT and write nothing; the caller reloads and merges. Cross-project references are rejected, not re-scoped: verification runs and finding IDs must belong to the thread's repository/run (mismatches are errors), and scoped servers answer cross-project run IDs with NOT_FOUND rather than leaking their existence.

## 5. JSON report envelope
```json
{
  "schema_version": "1.0",
  "run": {
    "id": "example-run-id",
    "repository_id": "example-repository-id",
    "state": "PARTIAL",
    "verdict": "FINDINGS",
    "created_at": "2026-11-12T12:33:00Z",
    "source": "GUI",
    "fingerprint_version": 1
  },
  "repository": {"display_name": "demo"},
  "targets": [{"relative_path": ".github/workflows/test.yml", "content_sha256": "<64-hex-digest>", "size_bytes": 120}],
  "scanner_runs": [{"adapter_id": "example-adapter", "status": "SUCCEEDED", "tool_version": "<validated-version>"}],
  "findings": [{"rule_id": "example-rule", "severity": "HIGH", "relative_path": ".github/workflows/test.yml", "message": "Illustrative finding; no real scanner claim", "evidence": {"redacted": true}}],
  "coverage": {"complete": false, "gaps": [{"adapter_id": "example-required-adapter", "reason": "UNAVAILABLE"}]}
}
```
This envelope is illustrative, not executable seed data or a complete formal JSON Schema. Implement a machine-validated report schema in Phase 2 and test all examples against it. Exclude absolute roots, snapshot paths, environment values and raw output by default.

## 6. Data directory
```text
<data-dir>/
  pwt.db
  locks/<root-identity-hash>.lock
  artifacts/<run-id>/report.json   # explicit export only; never required
  artifacts/<run-id>/report.md      # explicit export only; never required
  transient/<run-id>/snapshot/    # sensitive, deleted after use
  schemas/kubeconform/...         # explicitly provisioned local schemas (scanner cache)
```
SQLite may create WAL/SHM files. Data directory requires local-user access restrictions; filesystem encryption is not supplied by this schema. Recovery cleans orphaned transient snapshots only after repository lock ownership is established.

## 7. Retention and migrations
No automatic destructive retention in MVP. Show storage usage and explain sensitive metadata. Migrations run under a cross-process migration lock, inside transactions where supported; refuse unsupported newer schema versions. Do not drop existing records silently or create unrequested backup copies. Recommend user-controlled backup before destructive future migrations.
Migration history: V1 scanner evidence tables, V2 `mcp_sessions` presence (additive), V3 analysis threads/revisions/assessments plus `mcp_sessions.analysis_writes` (additive, PRAGMA-guarded ALTER). V1/V2 definitions were preserved, never rewritten; no historical artifact deletion was required. Raw secret-bearing scanner output is sanitized before persistence and is not authoritative retained evidence.

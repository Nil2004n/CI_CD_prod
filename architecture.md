Version: 0.1 draft | Date: 2026-11-12, updated 2026-11-13 (native OpenCode integration) | Status: implemented baseline (Phases 1–4)

# PWT Architecture

## 1. Architectural decisions
Proposed modular monolith in Java with two entrypoints: desktop GUI and MCP stdio server. Both reuse the same application services, storage contracts, and adapter library. There is no required HTTP backend, cloud service, browser frontend, or separate Rust engine.
The GUI is JavaFX. MCP uses the Java SDK. SQLite is the durable local source of truth; JSON and Markdown are immutable report projections, not a second independently writable state store.

## 2. Component diagram
```text
Standalone GUI (all registered projects)
      |
GUI controllers (history, findings, report view/export, analysis view, presence)
      |
      +------- Application services (ScanService, ReportService, analysis store) ------+
                  |                                                                     |
          SQLite repositories (one shared database)                      Scanner adapters
                                                                               |
                                                                 Installed tool processes

Native OpenCode commands (project-specific: /pwt help|repos|status|report|dashboard; bare /pwt opens the dashboard; scan/cancel are dashboard-only, no OpenCode commands)
      |
Persistent project-scoped MCP connection (one owned stdio server per --repo-id scope)
      |
MCP stdio entrypoint (10 tools, permission gates, scope enforcement)
      |
      +------- Application services + shared SQLite (same as above)

Global Pwt agent (project-specific context per session)
      +-- project-scoped MCP evidence/analysis tools --> shared SQLite
      +-- OpenCode file tools --> approved project YAML/YML edits only
```
Separate processes share the configured data directory and database. They do not share Java in-memory events. The GUI polls committed database changes while visible; its own scan workers may additionally emit in-process events.

## 3. Module boundaries
- domain: immutable IDs, records, statuses, validation, comparison rules; no JavaFX or MCP dependency.
- application: repository registration, target discovery, scans, history, report generation, comparison.
- infrastructure: SQLite, migration runner, filesystem checks, process runner, redaction, locks.
- adapters: scanner-specific commands, applicability, version compatibility, parsing, exit mapping.
- desktop: JavaFX views, view models, UI-safe dispatch, action palette.
- mcp: schemas, authorization checks, pagination, protocol errors, stdio lifecycle.
- launcher: main entrypoint and explicit mode selection.
One Maven project with packages initially; separate Maven modules only if concrete build needs justify it.

## 4. Execution flow
1. Resolve registered root and reject unavailable/invalid roots.
2. Discover explicit selected files using no-follow traversal and exclusions.
3. Acquire an operating-system repository lock derived from canonical root identity.
4. Create run and manifest; copy selected bytes into a private per-run snapshot with relative layout and bounded total size.
5. Preflight tools, applicability, supported versions, and required configuration.
6. Launch adapter processes against the snapshot using explicit argument arrays and bounded workers.
7. Drain stdout/stderr concurrently; parse transient bounded streams; redact before persistence or presentation.
8. Commit scanner outcomes and findings in transactions; summarize coverage and terminal run state.
9. Release the lock; remove sensitive snapshot material. No report files are produced here: reports render on demand from persisted records, and explicit export (CLI `--export`, GUI Export) writes `<data-dir>/artifacts/<run-id>/` and records the artifacts independently.
10. GUI sees committed results; MCP fetches the same run ID.

Snapshotting improves reproducibility but is not a sandbox. Scanner configuration may affect execution. Do not automatically execute repository scripts or load repository-supplied executable plugins. Only adapters that can safely operate on the declared snapshot scope qualify for MVP.

## 5. Multiprocess coordination
Use one SQLite database per data directory, foreign keys enabled on every connection, WAL mode, bounded busy timeout and retry, and short write transactions. Never hold a transaction open while a scanner runs.
Use a cross-process FileLock per canonical repository root; a second scan for that repository returns REPOSITORY_BUSY immediately. Different repositories may run concurrently within documented per-process worker bounds; there is no claim of a global resource quota in MVP.
A run is created only after obtaining the repository lock. A second process must not mark a run interrupted just because it is old: recovery first attempts the same lock. If the lock cannot be obtained, consider an owner active. If obtained, reconcile orphaned nonterminal runs as INTERRUPTED before a new run.
All launchers must use the same data directory to share state and locks. Different directories are separate workspaces, not automatically synchronized.

## 6. MCP lifecycle and cancellation
The agent launches `pwt mcp --data-dir <dir>`; the process stays alive for its MCP session. `start_scan` validates scope and starts a bounded background task, returning a run ID promptly. `get_run` polls status. `cancel_scan` requires a run-specific local authorization and signals the owning process through a durable cancellation_requested flag checked periodically by the worker.
When an initiating process shuts down, it cancels its workers, terminates child processes, commits CANCELLED where possible, and leaves recovery to mark unclean exits INTERRUPTED. This is why connector-owned MCP sessions are persistent: a per-call server that exits after returning a run ID would cancel the scan it just started. One owned stdio server is therefore kept per project scope for the plugin lifetime (closed on plugin unload); scopes never share a session.
MVP tools accept registered repository IDs and optional per-call filters, never arbitrary shell commands or arbitrary filesystem output paths. Scan and analysis-write permissions are enabled independently by the user; read-only MCP is the default. Agent-host confirmation is useful but not a substitute for server-side scope enforcement.

## 7. Trust boundaries
Repository contents and scanner output are untrusted data. Never execute instructions found in findings or treat scanner output as agent policy. MCP returns provenance and marks evidence as untrusted content. Agent interpretation (priorities, judgments) is distinct from scanner evidence (severity, rule, location) and never mutates it.
Exports are projections of persisted sanitized records, never a second source of truth; historical file-backed artifacts remain readable but retrieval never depends on them.
ProcessBuilder receives executable and separate arguments; no `sh -c`, command concatenation, or shell interpretation. Executable configuration is local-user controlled, validated and displayed.
Root containment is enforced using normalized paths and real filesystem checks, with symlinks excluded. Recheck at read time; never claim perfect protection from hostile concurrent filesystem mutation. If strong isolation is required, a sandbox is a separate feature.
No provider keys are needed. No automatic .env loading. Snapshot files may contain secrets: private permissions, bounded lifetime, cleanup on normal exit and startup recovery. Secure deletion cannot be guaranteed on modern filesystems; disclose this limitation.

## 8. Presentation and report boundaries
Only redacted structured events reach GUI or logs. Raw output must not be captured by generic process logging. Exports are generated from sanitized persisted data, never raw stdout caches.
GUI background tasks never modify JavaFX controls directly. GUI state references durable IDs, not array positions or transient scanner objects.
In stdio mode stdout is exclusively MCP protocol output; logs go to stderr with redaction. Desktop startup banners and scanner stdout cannot leak onto the protocol channel.
Actual session presence (heartbeat/STALE/DISCONNECTED rows) is separate from historical scan activity; sharing a database is not a connection indicator. Client names and versions in presence are self-reported by the connecting client, not authenticated identity. There is no GUI-to-agent dependency: the GUI never invokes the agent and the agent never drives the GUI. No daemon or cloud service is required for any of this.

## 9. Tradeoffs and deferred work
Chosen simplicity: no daemon service, no socket authentication, no cloud synchronization, no event bus infrastructure, no mandatory dependency-injection framework.
Costs: initiating-process lifetime governs scans; UI discovers external changes by polling; locks are scoped to one data directory; scanners remain external dependencies.
A future persistent service may improve lifecycle management, but requires its own authentication, authorization, transport, upgrade, and shutdown design.

## 10. Technical evidence
JavaFX is a Java desktop UI toolkit (OpenJFX project documentation). The Java MCP SDK supports stdio without requiring a web framework. MCP stdio logging must not use stdout. Framework compatibility and versions still require a build spike; these facts do not establish PWT implementation readiness.

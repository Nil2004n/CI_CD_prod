# Changelog

All notable changes to PWT. Versions follow semantic versioning; the
version is declared in `pwt/pom.xml` and reported by `pwt --version` and the
MCP `serverInfo`.

## 1.1.0 — unreleased (release candidate)

### Added

- **Read-only MCP tools.**
  - `get_project_status`: the connection's project, root availability, latest
    run (final or provisional), active runs, run count, scan-journal
    availability and the server's effective permissions.
  - `compare_runs`: compares two runs of one project by recorded finding
    fingerprint. It reports findings only in one run (each marked
    `comparable` or not, with a reason), findings in both, possible line
    shifts, differences in selected files, content and scanner execution, and
    explicit limitations. It never labels anything "fixed".
  - `list_findings`: new exact-match `rule_id` and `relative_path` filters;
    large evidence is capped.
  - `get_run`: each scanner entry gets an `execution` class (a timeout or
    failure is an execution failure, not a rule result), plus `final` and the
    recorded run error.
- **Project-root binding for MCP servers.** `pwt mcp --project-root <abs dir>`
  binds every call to the registered repository owning that directory.
  - Paths are canonicalised, so subdirectories and symlinked paths resolve to
    the same project; the deepest registered root wins for nested projects.
  - The first successful binding is pinned for the session. A later
    registration change fails closed with `PROJECT_SCOPE_CHANGED`.
  - Unregistered directories fail with `PROJECT_NOT_REGISTERED`. Nothing is
    registered and nothing falls back to another project.
  - New error codes: `PROJECT_NOT_REGISTERED`, `PROJECT_UNAVAILABLE`,
    `PROJECT_AMBIGUOUS`, `PROJECT_SCOPE_CHANGED`.
  - The launcher accepts `--project-root` and `--project-root-cwd`. The
    latter binds to the directory the client starts the server in.
- **Claude Code integration** (scans stay dashboard-only):
  - `/pwt [help|repos|status [run-id]|report [latest|<run-id>]|dashboard]` and
    the aliases `/pwt-help`, `/pwt-repos`, `/pwt-status`, `/pwt-report`,
    `/pwt-dashboard`.
  - `/pwt-ask <question>`, which delegates to a read-only `pwt` subagent. The
    subagent runs on the `haiku` model, reads the project's journal first,
    uses read-only MCP tools only when needed, and returns proposed YAML
    patches for the main session to apply after your approval.
  - Make targets: `install-claude`, `setup-claude ROOT=…` (project entry),
    `setup-claude-global` (user-wide entry, bound per session to the start
    directory), `claude-status [ROOT=…] [TEST=1]`, `uninstall-claude [ROOT=…]`.
  - `make install` installs the user-wide Claude commands and subagent when
    Claude Code is present. It never adds an MCP entry; `make uninstall`
    removes the managed Claude files and the PWT-created global entry.
- **Desktop dashboard restyle**: three-column workbench layout with Progress,
  Findings, Coverage and Activity tabs and a token-based dark theme
  (`pwt-theme.css`). Presentation only: no behaviour, data or action changes.
- **`make test-integrations`**: Python installer and integration tests plus
  Node OpenCode launch tests.

### Fixed

- **OpenCode `/pwt` ghost text.** The dashboard's jar lookup let launcher
  stderr reach the TUI's terminal, where it was painted over the prompt. The
  lookup now captures stdout and stderr and parses only the jar path. Launch
  failures become controlled toasts instead of raw output or stack traces.
- **`get_analysis` wrote on read.** It created an empty analysis thread the
  first time a run's analysis was read. It is now read-only.
- **Artifact selection was inconsistent.** The MCP launcher picked the
  newest jar, while the installed `pwt` launcher (and the desktop entry)
  refused when more than one jar existed. All entrypoints now share one
  policy: `--jar`/`PWT_JAR`, then the jar for the version declared in
  `pwt/pom.xml`, then a single candidate, otherwise an error listing the
  candidates. Never "newest wins".

### Changed

- The Claude Code project MCP entry (`make setup-claude`) is now bound with
  `--project-root` and is never scan-enabled. `ALLOW_SCANS=1` is refused, an
  older scan-enabled entry is downgraded, and the old scan-capable
  project-local `/pwt` template is removed.
- `pwt-dashboard.sh` resolves the jar itself when `--jar` is omitted and
  accepts `--detach`.

### Known limitations

- The subagent's project boundary is an instruction, not an operating-system
  sandbox.
- PWT does not persist per-run event logs. Diagnostics come from recorded
  scanner outcomes and run errors.
- MCP SDK 0.8.1 has no read-only tool annotations; none are advertised.
- A running OpenCode session must be restarted (`opencode service restart`)
  to load the ghost-text fix.
- The OpenCode MCP client pipes the server's stderr without reading it. This
  is not a terminal leak, but a very long session could fill the pipe.

## 1.0.0 — 2026-11-14

First tagged release (GitHub Release `v1.0.0`, `pwt-1.0.0.jar`).

# PWT — YAML security assessment workbench (Java rebuild)

PWT assesses YAML/YML files with installed open-source scanners, keeps
sanitized evidence in a shared local SQLite database, and exposes it to a
standalone desktop GUI, native OpenCode commands, and a project-scoped
analysis agent. PWT orchestrates existing scanners; it is not a newly
written universal vulnerability engine.

For developers, security engineers, and pentesters assessing repositories
they are authorized to inspect. Local-first: no cloud account, no embedded
model, no telemetry.

## Current status

Phases 1–4 implemented: foundation/discovery/minimal MCP, first scanners
and persistence, remaining scanner integrations, then native OpenCode
integration with scoped MCP, shared state, and database-backed analysis.
Phase 5 (complete desktop workflow/UI) has not begun. Phase 6 hardening
is deferred. Approach is build-first, harden-later. Details and evidence:
`status.md`. Handover for the next session: `handover.md`.

Known verification limits: the GUI history-row click was not driven in the
last synthetic-input probe (loader and store paths are tested); the native
plugin headless harness is session evidence, not yet a repo-owned script;
`@pwt` is not a routed mention in either client (use the agents dialog or
`/pwt-ask`). No completion percentages are claimed anywhere.

## Architecture overview

Standalone GUI (all registered projects) → existing application services
→ shared SQLite.

Native OpenCode commands (project-specific) → persistent project-scoped
MCP connection → existing application services → shared SQLite.

Global Pwt agent (project-specific context) → project-scoped MCP
evidence/analysis tools → shared SQLite; plus OpenCode file tools for
approved project YAML/YML edits.

One shared local database, not one per project; scope is enforced by MCP
handlers. Exports are projections, never a second source of truth. There
is no required cloud or backend service, no daemon, and no GUI-to-agent
dependency. Technical authority: `architecture.md`, `tech-spec.md`,
`data-schema.md`; product baseline: `prd.md`.

## Prerequisites

Tested baseline: JDK 21, Linux, OpenCode v2.0.26. Build needs Maven
Central access on first run (cached under `~/.m2`). Scanners are external
dependencies and must be installed separately: `yamllint`, `gitleaks`,
`checkov`, `actionlint`, `kubeconform`. Missing tools surface as
UNAVAILABLE, never as clean. Kubeconform and local-schema checks need
locally provided schemas (see `pwt/README.md`); scans never download
schemas. The GUI needs a local graphical session (or `xvfb-run`).
Windows/macOS are not validated targets.

Before every Maven invocation:

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export PATH="$JAVA_HOME/bin:$PATH"
```

## Quick start

One-time user installation (run from the PWT checkout):

```sh
make install                       # global pwt launcher + OpenCode plugin/agent/ask,
                                 # desktop entry, then enable the current project
                                 # (read-only; scans run only from the dashboard)
```

Then, per assessed project (or open the project in OpenCode):

```sh
make enable ROOT="/abs/project"  # register + wire MCP (read-only unless flags given)
opencode                         # /pwt status … | /pwt-ask … | pwt gui …
```

Lower-level equivalents (the install/enable targets wrap these):

```sh
make build                       # builds pwt/target/pwt-1.0.0.jar (skips tests)
make test                        # full JUnit suite
make setup                       # create <app-root>/.pwt-data, print scanner prerequisites
make repo-add ROOT="/abs/project"
make doctor                      # read-only environment report
make start                       # launch the desktop GUI (needs a display)
```

Then, for OpenCode (project-local, preserves unrelated config) — done by
`make enable`, or manually:

```sh
make setup-opencode ROOT="/abs/project" [ALLOW_ANALYSIS=0|1]
make install-opencode            # one-time global plugin + Pwt agent
```

Host MCP from any directory (no project setup): the global fallback entry
`make setup-opencode-global` (read-only; pass `ALLOW_ANALYSIS=1` only
deliberately — scan permission is never written). It is unscoped by
design — every call is still repository-scoped server-side, per-project
entries override it, unknown roots are never registered, and `make enable`
remains the explicit project route.

`--allow-scans` gates `start_scan`/`cancel_scan` server-side for deliberate
non-OpenCode uses only (OpenCode configuration never carries it);
`ALLOW_ANALYSIS=1` enables `save_analysis` per project. Analysis defaults to
deny; both are enforced server-side. Scanning happens only in the dashboard
(`/pwt` opens it).
Verify with `make repo-list` and `make mcp-status`.
Quote any path that may contain spaces.

## OpenCode usage

Native commands call MCP directly; they use no model and no credits.
Scans and cancellation are dashboard-only — there are no OpenCode scan
or cancel commands (typing them explains this and points at `/pwt`):

```sh
/pwt help
/pwt repos
/pwt status [run-id]
/pwt report [latest|<run-id>]
/pwt dashboard
```

Aliases `/pwt-help`, `/pwt-repos`, `/pwt-status`, `/pwt-report`,
`/pwt-dashboard` are discoverable in the command palette;
`/pwt <subcommand> <arguments>` is the argument-bearing route.
Bare `/pwt` opens the dashboard helper.
`/pwt repos` shows only the current project repository; nothing else is
listed. Unregistered directories get: "No PWT assessment is available
for this project. Open /pwt and run its first scan in the dashboard."
(running that first scan also registers the project and writes its
`.pwt/context.md` pointer).

Agent conversation and analysis use your configured OpenCode model.
Explicit route: `/pwt-ask <message>` (global command; frontmatter selects
the `Pwt` agent). There is no `@pwt` shortcut: alternatively select the
`Pwt` agent in the agents dialog (verified client shortcut: shift+tab to
cycle agents, OpenCode v2.0.26 TUI), then ask normally. The agent resolves the working directory to its
registered repository and refuses unregistered directories; one project
per session, never other registered projects. Guide:
`integrations/opencode/README.md`.

## Storage and exports

Effective data directory: `<application-root>/.pwt-data` by default
(the PWT checkout root holding the root Makefile; `--data-dir` /
`DATA_DIR=` overrides it everywhere, and must be absolute). One SQLite
database (`pwt.db`) holds every repository and scan; project scoping
filters records, never selects another file. SQLite is the authoritative
assessment and analysis records. There is no required
scan-report.json/md or analysis_report.md: `get_report` renders from
persisted records, and files appear only after explicit export
(`scan --export`, GUI Export button, recorded under
`<data-dir>/artifacts/<run-id>/`). Historical file-backed artifacts
remain readable. Every terminal scan also appends a Markdown section to the
repository's scan journal at `<data-dir>/reports/<name>--<repo-id>.md`
(context pointer written by `pwt repo-add` into `<repo>/.pwt/context.md`;
sections appended on every terminal scan, backfill/repair via
`pwt journal`); the Pwt agent answers scan questions from that journal
with plain file reads, no MCP/SQL needed. Project YAML/YML stays in the project; only explicitly
selected files are staged to transient snapshots that are cleaned up.
Scanner-required cache (e.g. kubeconform local schemas) is separate from
retained reports.

## Permissions

`--allow-scans` gates `start_scan`/`cancel_scan`;
`--allow-analysis-writes` gates `save_analysis`; independent flags, both
deny by default. Everything else is read-only. File edits happen only
through OpenCode's own file tools after your approval — there are no
PWT source-edit, shell, commit, or push MCP tools, and no arbitrary SQL
access. MCP tool reference: `pwt/README.md`.

## Scanner coverage

Six integrations: yamllint, Gitleaks, Checkov, actionlint, kubeconform,
local user-supplied schema validation. Applicability is per file:
Checkov runs on K8s-manifest-shaped files, actionlint only on
`.github/workflows/` files, kubeconform and local-schema only with
applicable local schemas; otherwise NOT_APPLICABLE/SKIPPED with visible
gaps. Partial coverage yields INDETERMINATE, never clean. Never present
a scoped verdict as repository-wide and never call anything "secure".
Applicability table: `pwt/README.md`.

## Troubleshooting

- `doctor: application jar missing` → run `make build` first.
- Scanner MISSING in `make setup`/`doctor` → install that tool; results
  show UNAVAILABLE, not clean.
- `CONFIGURATION_REQUIRED` for local-schema → pass
  `GLOB=/abs/schema.json` bindings; schema paths must be absolute,
  regular files, no symlinks.
- `PERMISSION_DENIED` on analysis writes → the project MCP entry needs
  `--allow-analysis-writes` (re-run setup with `ALLOW_ANALYSIS=1`).
  Scan/cancel requests from OpenCode are denied by configuration: run
  scans from the dashboard instead (`/pwt`). Client approval alone
  never enables writes.
- `NOT_FOUND`/`OUT_OF_SCOPE` or "not inside a registered repository" →
  `make repo-add ROOT="…"`, and point every client at the same
  `--data-dir`.
- GUI will not start headless → use a local display or `xvfb-run`;
  dashboard helper refuses without `DISPLAY`/`WAYLAND_DISPLAY` and never
  opens a second window.
- `mcp-status` shows STALE → owner process exited (heartbeat older than
  10 s); DISCONNECTED → orderly shutdown. Client names are self-reported,
  not authenticated identity.
- Old `.opencode/commands/pwt.md` prompt template shadows native
  commands → delete it; `make setup-opencode` removes it automatically.

## Documentation links

- `fde.md` — installation and operations manual (setup, database location, runbook)
- `pwt/README.md` — build, MCP tool reference, scanner applicability, stack
- `prd.md` — product requirements (original vs current vs planned)
- `tech-spec.md` — launch/MCP/persistence contracts as implemented
- `architecture.md` — components, scoping, trust boundaries
- `data-schema.md` — migrations V1–V3, tables, retention
- `plan.md` — roadmap phases
- `integrations/opencode/README.md` — OpenCode install/use/verify
- `integrations/claude-code/README.md` — Claude Code state (deferred scope)
- `status.md`, `handover.md` — verification evidence and next session

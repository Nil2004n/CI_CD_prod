# PWT — Installation and Operations Manual (FDE)

Field Deployment Edition: everything needed to install, configure, run, and
verify PWT on a Linux machine. PWT assesses YAML/YML files with installed
open-source scanners and keeps sanitized evidence in one shared SQLite
database. No cloud account, no embedded model, no telemetry.

## 1. What gets installed

| Piece | Location | Purpose |
|---|---|---|
| Application code + build | `<app-root>/pwt/` (Maven) | Java services, adapters, GUI, MCP server |
| Built artifact | `<app-root>/pwt/target/pwt-0.4.0.jar` | Single runnable jar for every entrypoint |
| Shared database | `<app-root>/.pwt-data/pwt.db` | ONE database for all repos and runs (see §5) |
| Native OpenCode plugin | `~/.config/opencode/plugins/pwt/` | Model-free `/pwt` commands |
| Global Pwt agent | `~/.config/opencode/agents/pwt.md` | Project-scoped conversation agent |
| Global ask command | `~/.config/opencode/commands/pwt-ask.md` | `/pwt-ask <message>` entrypoint |
| Project MCP entry | `<project>/opencode.json` | Host-managed `pwt` MCP server config |
| Project plugin copy | `<project>/.opencode/plugins/pwt/` | Project-local native commands |

`<app-root>` is the PWT checkout root holding the root `Makefile`
(this repository). Never hand-copy files into these locations; use the
`make` targets below.

## 2. Prerequisites

- Linux (tested baseline; Windows/macOS are not validated targets).
- JDK 21 (`/usr/lib/jvm/java-21-openjdk-amd64` on this host). Export before
  every Maven invocation:
  `export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64`
  `export PATH="$JAVA_HOME/bin:$PATH"`
- Scanner executables on PATH, installed separately (never auto-installed):
  `yamllint` 1.38.0, `gitleaks` 8.30.1, `checkov` 3.4.4, `actionlint`
  1.7.12, `kubeconform` (dev build). Missing tools surface as UNAVAILABLE,
  never as clean. Kubeconform and local-schema checks additionally need
  locally provided schemas (see §8); scans never download anything.
- Python 3 (setup/install scripts only; stdlib, no packages).
- OpenCode v2.0.26 for the native commands and agent.
- A local graphical session (or `xvfb-run`) for the dashboard GUI.
- Maven Central access on first build only (cached under `~/.m2`).

## 3. Build and first-time setup

From the application root:

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export PATH="$JAVA_HOME/bin:$PATH"
make build        # builds pwt/target/pwt-0.4.0.jar (skips tests)
make test         # full JUnit suite (expect 147 pass; 1 known environmental
                  # skip if upstream schema download times out)
make setup        # creates <app-root>/.pwt-data, prints scanner prerequisites
make doctor       # read-only report: Java, artifact, data dir, database path,
                  # repositories, scanners, OpenCode version, permissions
```

Useful everyday targets:
`make repo-add ROOT="/abs/project"` (register a repository, idempotent),
`make repo-list`, `make start` (desktop GUI), `make dashboard` (GUI via
helper, local display only), `make mcp [ALLOW_SCANS=1] [ALLOW_ANALYSIS=1]`
(stdio server; stdout is protocol-only), `make mcp-status`, `make clean`.

## 4. OpenCode integration

One-time global install (refuses to overwrite unmanaged files):

```sh
make install-opencode      # plugin + Pwt agent + /pwt-ask command
make uninstall-opencode    # removes only what the installer created
```

Per project (preserves unrelated config; never touches global config):

```sh
make setup-opencode ROOT="/abs/project" [ALLOW_ANALYSIS=1]
```

This merges a read-only `pwt` MCP entry into `<project>/opencode.json`
(scan permission is never written; analysis writes stay opt-in), installs
the project plugin, deletes any superseded prompt template, and copies the
conversation reference. Admin headless registration remains available via
`make repo-add`, but first GUI scans self-register, so preregistration is
no longer required.

Native commands (no model, no credits) — `/pwt <subcommand> <arguments>`
is the argument-bearing route; bare aliases fire with empty input.
Scanning and cancellation happen only in the dashboard; there are no
OpenCode scan or cancel commands:

```sh
/pwt help | /pwt repos | /pwt status [run-id] | /pwt report [latest|<run-id>]
/pwt dashboard
```

Bare `/pwt` opens the dashboard helper (pending owner decision; a design
had it show help instead — code unchanged for now).

Conversation uses the model through one explicit route:
`/pwt-ask <message>` (frontmatter selects the `Pwt` agent; verified).
Alternatively select the `Pwt` agent in the agents dialog (shift+tab to
cycle agents) and ask normally. `@pwt` is not a routed mention — typing it
shows no agent completion. Telling the agent "act as PWT" is not
supported; use the two routes above.

## 5. The database: location and rules

- **Active database:** `<app-root>/.pwt-data/pwt.db`
  (here: `/home/kali/Desktop/CI_CD_prod/.pwt-data/pwt.db`).
  One file holds every repository, run, finding, artifact record, MCP
  session row, and agent-analysis revision.
- **Resolution order** (Java `AppPaths`, both launcher scripts, native
  plugin, setup helpers all follow it): explicit `--data-dir` /
  `DATA_DIR=` / `pwt.data.dir` wins; otherwise `<application-root>/.pwt-data`
  resolved from the checkout (root `Makefile` + `integrations/` marker);
  otherwise fail loudly — never a silent database elsewhere.
- **Overrides must be explicit and absolute**: relative `--data-dir`
  values are rejected by the setup scripts; the effective absolute path is
  printed by `make doctor` (`database` field), `make mcp-status`, and the
  setup scripts. MCP diagnostics go to stderr, never protocol stdout.
- **Project scoping filters records, never selects another file.**
  `--repo-id` restricts every MCP operation; cross-project IDs return
  NOT_FOUND/OUT_OF_SCOPE with a correlation ID.
- **No report files by default.** `get_report`, the GUI View action, and
  analysis all read persisted records. Files appear only on explicit
  export: CLI `scan --export` or the GUI Export button, recorded under
  `<data-dir>/artifacts/<run-id>/`. Historical file-backed artifacts remain
  readable.
- **WAL/SHM files** (`pwt.db-wal`, `pwt.db-shm`) belong to the same
  database — never delete them to "fix" anything. `transient/` holds
  per-run scan snapshots, deleted after each run; it is not storage.
- **Legacy database (do not merge without approval):**
  `/home/kali/.pwt-data/pwt.db` (schema v3, CI_CD_prod history) is intact
  and was deliberately left alone. Consolidation needs an explicit
  owner-approved import with a repo/run ID overlap review. Never copy a
  live database file; use SQLite backup procedures with writers stopped.
- `.pwt-data/` is git-ignored; database contents are never committed.
  Tests use isolated temporary databases and are unaffected.

## 6. Permissions

Two independent flags, both deny by default, both enforced server-side
(client approval alone never enables writes):

- `--allow-scans` gates `start_scan` / `cancel_scan`.
- `--allow-analysis-writes` gates `save_analysis`.

Everything else is read-only. A 5-minute execution timeout bounds every
MCP tool call (`"timeout": {"execution": 300000}` on the `pwt` entry), so
a wedged transport fails fast instead of hanging; reconnect via `/mcps`.
There are no PWT source-edit, shell, commit, push, arbitrary-SQL, or
export-path MCP tools. File edits happen only through OpenCode's own file
tools after your approval.

## 7. Scanner coverage and honesty rules

Six integrations: yamllint, Gitleaks, Checkov, actionlint, kubeconform,
local user-supplied schema validation. Applicability is per file: Checkov
runs on K8s-manifest-shaped files, actionlint only under
`.github/workflows/`, kubeconform and local-schema only with applicable
local schemas; otherwise NOT_APPLICABLE/SKIPPED with visible gaps. Partial
coverage yields INDETERMINATE, never clean. Never present a scoped verdict
as repository-wide and never call anything "secure". Omitting `--adapter`
selects all six; `local-schema` without bindings fails fast with
configuration guidance.

## 8. Kubeconform and local schemas (explicit setup)

```sh
java -jar pwt/target/pwt-0.4.0.jar kubeconform-setup \
  --k8s-version 1.29.0 --kinds Pod,Service,Deployment.apps --data-dir <dir>
```

Schemas live under `<data-dir>/schemas/kubeconform/`; `--kubeconform-schema`
overrides with a local `.../{{.ResourceKind}}{{.KindSuffix}}.json` template
(note: kubeconform lowercases kinds, so local schema files use lowercase
names). Local-schema bindings: `--schema-binding 'GLOB=/abs/schema.json'`
(repeatable; absolute regular file, no symlinks; dialects 2020-12 default,
2019-09 allowed; remote `$ref` never fetched).

## 9. Verification checklist (run after install)

1. `make doctor` — shows the `<app-root>/.pwt-data` database path, repos,
   all six scanners, OpenCode version, read-only default.
2. `make repo-add ROOT="<projA>"` and a second repo; `make repo-list`.
3. Headless scans from different working directories without `--data-dir`
   (or first GUI scans of unregistered directories, which auto-register);
   confirm one `pwt.db` holds both repos and all runs
   (`sqlite3 <app-root>/.pwt-data/pwt.db "SELECT ... FROM runs"`), each
   repo has its own journal under `<data-dir>/reports/`, and its own
   `.pwt/context.md` pointer.
4. Native `/pwt status` in the project TUI (zero chat turns); unregistered
   directories get the no-assessment message, never another project's data.
6. `/pwt-ask What does the latest scan say?` — Pwt agent answers from saved
   evidence only.
7. Approve an edit → run a new dashboard scan → agent reads the new
   journal section and reports before/after; `save_analysis` only on
   explicit request. Confirm the Pwt agent never calls MCP tools or SQLite
   for questions (answer from journal file reads), and never claims a fix
   is verified without a rescan.
8. Failure paths: unregistered directory message, "No runs yet" on empty
   repos, `REPOSITORY_BUSY` on concurrent same-repo scans, no stray
   `pwt.db` under assessed repos or staging dirs.

## 10. Troubleshooting

- `doctor: application jar missing` → run `make build` first.
- Scanner MISSING → install that tool; results show UNAVAILABLE, not clean.
- `CONFIGURATION_REQUIRED` for local-schema → pass absolute-schema bindings.
- `PERMISSION_DENIED` on analysis writes (`save_analysis`) → the project
  MCP entry needs `--allow-analysis-writes` (opt in explicitly per
  project). Scan/cancel are dashboard-only; OpenCode never requests scan
  permission, and no flag enables it there.
- `NOT_FOUND`/`OUT_OF_SCOPE` or "not inside a registered repository" →
  `make repo-add ROOT="..."`, and point every entrypoint at the same
  `--data-dir` (verify with `make doctor`).
- GUI won't start headless → use a local display or `xvfb-run`; the
  dashboard helper refuses without `DISPLAY`/`WAYLAND_DISPLAY` and never
  opens a second window.
- `mcp-status` shows STALE → owner process exited (heartbeat older than
  10 s); DISCONNECTED → orderly shutdown. Client names are self-reported.
- Old `.opencode/commands/pwt.md` shadows native commands → delete it
  (`make setup-opencode` removes it automatically).
- `/pwt scan …` or `/pwt cancel …` typed anywhere → both answer with the
  dashboard pointer ("Scanning and cancellation now happen in the dashboard.
  Use /pwt."); there are no OpenCode scan/cancel commands or aliases.

## 11. Document map

- `README.md` — product entrypoint. `pwt/README.md` — build, MCP tool
  reference, applicability, pinned stack.
- `prd.md` — requirements (original vs current vs planned). `tech-spec.md`
  — launch/MCP/persistence contracts as implemented. `architecture.md` —
  components, scoping, trust boundaries. `data-schema.md` — migrations
  V1–V3, tables, retention. `plan.md` — roadmap phases.
- `integrations/opencode/README.md` — OpenCode install/use/verify.
  `integrations/claude-code/README.md` — Claude Code state (deferred scope).
  `status.md`, `handover.md` — verification evidence and next session.

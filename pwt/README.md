# PWT — Phase 4 rebuild (Java)

MCP operations over the shared SQLite store, native OpenCode command
workflow, a project-scoped analysis agent, and the standalone JavaFX
dashboard. All six scanner integrations from Phase 3 are preserved and
reused unchanged.

## Prerequisites

- JDK 21 (`/usr/lib/jvm/java-21-openjdk-amd64` on this host)
- Installed tools: `yamllint` 1.38.0, `gitleaks` 8.30.1, `checkov` 3.4.4,
  `actionlint` 1.7.12, `kubeconform` (dev build, reports `development`).
  Missing tools surface as UNAVAILABLE, never as clean.
- Network access to Maven Central on first build (cached under `~/.m2`)
- A display for `gui` / dashboard (or `xvfb-run`)

Export before every Maven invocation:

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export PATH="$JAVA_HOME/bin:$PATH"
```

## Build and test

```sh
cd pwt
./mvnw -B clean test        # 146 tests, all must pass, 0 skipped
./mvnw -B -q package -DskipTests   # produces target/pwt-1.1.0.jar
```

Root-level shortcuts live in `../Makefile`: `make build|test|setup|
repo-add|repo-list|start|dashboard|setup-opencode|setup-claude|
install-opencode|uninstall-opencode|mcp|mcp-status|doctor|clean`.

## MCP server

```sh
# Read-only (default): pwt_info, list_repositories, list_runs, get_run,
# list_findings, get_report, get_analysis, get_project_status, compare_runs
java -jar target/pwt-1.1.0.jar mcp --data-dir /abs/pwt-data

# Scan-enabled: additionally start_scan, cancel_scan
java -jar target/pwt-1.1.0.jar mcp --data-dir /abs/pwt-data --allow-scans

# Analysis writes: additionally save_analysis (independent flag)
java -jar target/pwt-1.1.0.jar mcp --data-dir /abs/pwt-data --allow-analysis-writes

# Project-scoped: every operation restricted to one registered repository
java -jar target/pwt-1.1.0.jar mcp --data-dir /abs/pwt-data --repo-id <id>

# Project-root-bound (Claude Code): every call resolves the registered
# repository owning that directory (canonical path, deepest registered root);
# fails closed with PROJECT_NOT_REGISTERED until the first dashboard scan
# registers it. Mutually exclusive with --repo-id; never registers anything.
java -jar target/pwt-1.1.0.jar mcp --data-dir /abs/pwt-data --project-root /abs/project
```

stdout is MCP-protocol-only; diagnostics go to stderr. Errors are
structured (`code`, `message`, `correlation_id`): INVALID_INPUT,
NOT_FOUND, OUT_OF_SCOPE, PERMISSION_DENIED, REPOSITORY_BUSY,
CONFIGURATION_REQUIRED, CONFLICT, INTERNAL_ERROR, and for project-root-bound
servers PROJECT_NOT_REGISTERED, PROJECT_UNAVAILABLE, PROJECT_AMBIGUOUS,
PROJECT_SCOPE_CHANGED (binding pinned per session).
`list_findings` filters: `severity`, `adapter_id`, `rule_id`,
`relative_path` (exact match). `get_run` scanner entries carry an
`execution` class (executed / execution_failure / not_executed /
tool_unavailable / cancelled / in_progress) and the recorded run error;
`final: false` marks an active, provisional run. `compare_runs` matches two
runs of one project by recorded fingerprint and reports comparability and
limitations instead of "fixed/new" verdicts. `get_analysis` is read-only (it
no longer creates an empty analysis thread on first read). List pagination:
`cursor` (offset string) + `limit` (1–500, default 100). Reports are
chunked (`offset`/`length`, `next_offset`, never silently truncated).
`start_scan` returns the run ID promptly and scans in a bounded
background pool; `cancel_scan` sets the durable flag observed by any
owning process and reports the actual terminal state. `save_analysis`
appends a revision only when `expected_revision` matches current,
otherwise CONFLICT with nothing written.

## Coding-agent setup

`make install-opencode` installs the global native command plugin and
Pwt agent; `make setup-opencode ROOT=...` writes the project-local MCP
entry plus project plugin (see `../integrations/opencode/README.md`).
Claude Code: `make install-claude` (user-wide `/pwt`, `/pwt-ask`,
read-only `pwt` subagent) and `make setup-claude ROOT=...` (project
`.mcp.json` bound with `--project-root`, scans always denied); see
`../integrations/claude-code/README.md`. Assessment operations always go through MCP, never
shell scans. OpenCode native commands use no model and no credits;
agent conversation/analysis uses the configured OpenCode model.

## Dashboard

```sh
integrations/pwt-dashboard.sh --jar /abs/pwt-1.1.0.jar --data-dir /abs/pwt-data
# or: make start | make dashboard
```

Opens the standalone JavaFX window on a local graphical machine
(refuses without a display, never opens a second window). The GUI shows
every registered project: run history with click-to-load, per-scanner
outcomes, findings, View/Export report, saved agent analysis
(read-only), live MCP presence, and registered-root picker. GUI accepts
`--data-dir` to share one directory with MCP/CLI. Bare `/pwt` in
OpenCode opens the same dashboard helper.

## Headless scan / discovery / setup

```sh
java -jar target/pwt-1.1.0.jar scan --root <dir> --file <rel>... [--adapter id,...] \
  [--data-dir <dir>] [--timeout-sec N] [--source CLI] [--export] \
  [--schema-binding 'GLOB=SCHEMA_PATH' ...] [--schema-dialect 2020-12|2019-09] \
  [--kubeconform-schema <local-template>] [--kubeconform-version V] \
  [--kubeconform-strict|--kubeconform-lenient]
java -jar target/pwt-1.1.0.jar discover --root <dir> [...]
java -jar target/pwt-1.1.0.jar kubeconform-setup --k8s-version 1.29.0 --kinds Pod,Service [...]
```

Result JSON on stdout (`run_id`, `state`, `verdict`, targets with
SHA-256, per-scanner outcomes with real versions, `gaps`). Exit codes:
0 clean, 4 findings observed, 2 invalid input/repository busy,
1 execution failure. Only explicitly selected files are staged and
scanned; the database is authoritative and no report files are written
unless `--export` is passed (files land under
`<data-dir>/artifacts/<run-id>/` and are recorded). Verdicts apply to
the selected scope, never "repository secure". A file with no matching
schema binding (or schema) is reported as a gap, never as valid.

## Local kubeconform schemas (explicit setup, never silent downloads)

```sh
# Download schemas for the kinds you validate into the managed cache:
java -jar target/pwt-1.1.0.jar kubeconform-setup \
  --k8s-version 1.29.0 --kinds Pod,Service,Deployment.apps --data-dir <dir>
```

Scans use `<data-dir>/schemas/kubeconform/...` by default; `--kubeconform-schema`
overrides with your own local `.../{{.ResourceKind}}{{.KindSuffix}}.json`
template. Note: kubeconform lowercases kinds, so local schema files must use
lowercase names (e.g. `widget.json`, `deployment-apps-v1.json`).

## Local schema validation

- Bindings: `--schema-binding 'GLOB=/abs/schema.json'` (repeatable; GLOB is
  matched against repository-relative paths). Schema root = the schema file's
  directory.
- Dialects: `2020-12` (default), `2019-09`. Anything else is rejected.
- Rules enforced: remote `http(s)` `$ref` never fetched; local `$ref` must
  stay inside the schema root and name an existing file; remote `$id` with
  relative refs rejected; YAML parsed with safe construction (bounded
  aliases/nesting, duplicate keys rejected — surfaced as SYNTAX findings);
  every document of a multi-document file validated separately with its index
  recorded; findings carry binding, keyword, and JSON-Pointer instance path —
  never line numbers (not reliable) and never rejected instance values.

## Headless discovery

```sh
java -jar target/pwt-1.1.0.jar discover --root <directory> \
  [--max-files N] [--max-depth N] [--exclude name,...]
```

## Analysis storage (SQLite, no report files required)

Agent analysis lives in `analysis_threads` / `analysis_revisions`
(append-only) / `analysis_assessments` (see `../data-schema.md`). Scanner
severity is never modified by analysis writes. `get_report` and the GUI
View action render from persisted records; `scan --export` and GUI
Export are the only file writers. Historical file-backed artifacts
remain readable.

## Applicability summary

| Adapter | Runs on | Otherwise |
|---|---|---|
| yamllint, gitleaks | every selected file | — |
| checkov | files sniffed as K8s manifests (`apiVersion`+`kind`) | NOT_APPLICABLE |
| actionlint | files directly in `.github/workflows/*.yml\|yaml` | NOT_APPLICABLE |
| kubeconform | K8s manifests; needs local schemas | NOT_APPLICABLE / gaps / SKIPPED |
| local-schema | files matching a binding | NOT_APPLICABLE / SKIPPED |

NOT_APPLICABLE is neutral (correctly out of scope); SKIPPED/missing-schema
breaks NO_FINDINGS into INDETERMINATE with visible gaps. Omitting
`--adapter` selects all six, and `local-schema` without bindings fails
fast with configuration guidance.

## Notes

- The SQLite JDBC driver prints a benign native-access warning to stderr on
  recent JDKs (never stdout).
- Staged snapshots live under `<data-dir>/transient/<run-id>/` with
  owner-only POSIX permissions where supported and are deleted best-effort
  after each run. Checkov/kubeconform/actionlint see only staged files.
- MCP session presence (2 s heartbeat server-side, STALE after 10 s
  without heartbeat, DISCONNECTED on orderly shutdown) is actual observed
  session state, not proof of database activity; client names/versions are
  self-reported.

## Pinned stack

JDK 21.0.11-ea · Maven 3.9.11 (wrapper 3.3.2) · OpenJFX 21.0.8 (linux) ·
`io.modelcontextprotocol.sdk:mcp` 0.8.1 · Jackson 2.17.0 · SnakeYAML 2.7 ·
`com.networknt:json-schema-validator` 1.5.9 · slf4j 2.0.16 ·
`org.xerial:sqlite-jdbc` 3.49.1.0 · JUnit 5.13.4 ·
yamllint 1.38.0 · Gitleaks 8.30.1 · Checkov 3.4.4 · actionlint 1.7.12 ·
kubeconform (unversioned dev build). See `pom.xml` header.
Clients: OpenCode v2.0.26, Claude Code 2.1.322. Python 3 stdlib only for
setup scripts (json/shutil/pathlib).

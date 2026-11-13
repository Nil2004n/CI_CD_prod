# PWT — Phase 4 rebuild (Java)

MCP operations over the shared SQLite store, coding-agent command workflow
for OpenCode and Claude Code, and basic dashboard launch. All six scanner
integrations from Phase 3 are preserved and reused unchanged.

## Prerequisites

- JDK 21 (`/usr/lib/jvm/java-21-openjdk-amd64` on this host)
- Installed tools: `yamllint` 1.38.0, `gitleaks` 8.30.1, `checkov` 3.4.4,
  `actionlint` 1.7.12, `kubeconform` (dev build)
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
./mvnw -B clean test        # 131 tests, all must pass, 0 skipped
./mvnw -B -q package -DskipTests   # produces target/pwt-0.4.0.jar
```

Root-level shortcuts: `make start|build|test|discover|scan|mcp`
(`make mcp` runs read-only; add `--allow-scans` only deliberately).

## MCP server

```sh
# Read-only (default): list_repositories, list_runs, get_run,
# list_findings, get_report, pwt_info
java -jar target/pwt-0.4.0.jar mcp --data-dir /abs/pwt-data

# Scan-enabled: additionally start_scan, cancel_scan
java -jar target/pwt-0.4.0.jar mcp --data-dir /abs/pwt-data --allow-scans
```

stdout is MCP-protocol-only; diagnostics on stderr. Errors are structured
(`code`, `message`, `correlation_id`): INVALID_INPUT, NOT_FOUND,
OUT_OF_SCOPE, PERMISSION_DENIED, REPOSITORY_BUSY, CONFIGURATION_REQUIRED,
INTERNAL_ERROR. Pagination: `cursor` (offset string) + `limit` (1–500,
default 100). Reports are chunked (`offset`/`length`, `next_offset`).
`start_scan` returns the run ID promptly and scans in a bounded background
pool; `cancel_scan` sets the durable flag observed by any owning process.

## Coding-agent setup

See `integrations/`: `pwt-dashboard.sh`, `pwt-conversation.md`,
`opencode/` (config template + `/pwt` command), `claude-code/`
(setup commands + `/pwt` slash command). Copy the project-local files into
your own project; replace `/abs/...` placeholders. `/pwt help|status|
scan|report|cancel|dashboard|ask` are agent-interpreted; assessment work
always goes through MCP, never shell scans.

## Dashboard

```sh
integrations/pwt-dashboard.sh --jar /abs/pwt-0.4.0.jar --data-dir /abs/pwt-data
```

Opens the existing JavaFX window on a local graphical machine (refuses
without a display, never opens a second window). The GUI refreshes its run
history from the shared store every second and loads MCP/CLI-created runs
on click. GUI accepts `--data-dir` to share one directory with MCP/CLI.

## Headless scan / discovery / setup

```sh
java -jar target/pwt-0.4.0.jar scan --root <dir> --file <rel>... [adapters/bindings/...]
java -jar target/pwt-0.4.0.jar discover --root <dir> [...]
java -jar target/pwt-0.4.0.jar kubeconform-setup --k8s-version 1.29.0 --kinds Pod,Service [...]
```

## Pinned stack

JDK 21.0.11-ea · Maven 3.9.11 (wrapper 3.3.2) · OpenJFX 21.0.8 (linux) ·
`io.modelcontextprotocol.sdk:mcp` 0.8.1 · Jackson 2.17.0 · SnakeYAML 2.7 ·
`com.networknt:json-schema-validator` 1.5.9 · slf4j 2.0.16 ·
`org.xerial:sqlite-jdbc` 3.49.1.0 · JUnit 5.13.4 ·
yamllint 1.38.0 · Gitleaks 8.30.1 · Checkov 3.4.4 · actionlint 1.7.12 ·
kubeconform (unversioned dev build).


## Prerequisites

- JDK 21 (`/usr/lib/jvm/java-21-openjdk-amd64` on this host)
- Installed tools: `yamllint` 1.38.0, `gitleaks` 8.30.1, `checkov` 3.4.4,
  `actionlint` 1.7.12, `kubeconform` (dev build, reports `development`).
  Missing tools surface as UNAVAILABLE, never as clean.
- Network access to Maven Central on first build (cached under `~/.m2`)
- A display for `gui` (or `xvfb-run`)

Export before every Maven invocation:

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64
export PATH="$JAVA_HOME/bin:$PATH"
```

## Build and test

```sh
cd pwt
./mvnw -B clean test        # 121 tests, all must pass, 0 skipped
./mvnw -B -q package -DskipTests   # produces target/pwt-0.3.0.jar
```

Root-level shortcuts live in `../Makefile`: `make start|build|test|discover|scan|mcp`.

## Launch the desktop GUI

```sh
java -jar target/pwt-0.3.0.jar gui
```

Open directory → select files → tick enabled checks → optionally add local
schema bindings (`glob=path`, comma-separated) and the kubeconform version →
Run scan: named stages, Cancel scan, per-scanner outcomes, verdict banner
(scope-limited wording), findings table, report locations + "Open reports
folder". Catalog shows all six INTEGRATED with probed availability
(including kubeconform cached-schema count). GUI scans write to the default
data dir `~/.pwt-data`.

## Headless scan (same service as the GUI)

```sh
java -jar target/pwt-0.3.0.jar scan --root <directory> --file <relative>... \
  [--adapter yamllint,gitleaks,checkov,actionlint,kubeconform,local-schema] \
  [--data-dir <directory>] [--timeout-sec N] [--source CLI] \
  [--schema-binding 'GLOB=SCHEMA_PATH' ...] [--schema-dialect 2020-12|2019-09] \
  [--kubeconform-schema <local-template>] [--kubeconform-version V] \
  [--kubeconform-strict|--kubeconform-lenient]
```

Result JSON on stdout (`run_id`, `state`, `verdict`, targets with SHA-256,
per-scanner outcomes with real versions, `gaps`, report paths). Exit codes:
0 clean, 4 findings observed, 2 invalid input/repository busy, 1 execution
failure. Only explicitly selected files are staged and scanned. Verdicts
apply to the selected scope, never "repository secure". A file with no
matching schema binding (or schema) is reported as a gap, never as valid.

## Local kubeconform schemas (explicit setup, never silent downloads)

```sh
# Download schemas for the kinds you validate into the managed cache:
java -jar target/pwt-0.3.0.jar kubeconform-setup \
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
java -jar target/pwt-0.3.0.jar discover --root <directory> \
  [--max-files N] [--max-depth N] [--exclude name,...]
```

## MCP stdio server

```sh
java -jar target/pwt-0.3.0.jar mcp
```

stdout is MCP-protocol-only. Exposes `pwt_info` (identity, Phase 3
capabilities, six-entry catalog all INTEGRATED). No scans, no file reads, no
shell, no edits. No MCP scan/report tools until Phase 4.

## Applicability summary

| Adapter | Runs on | Otherwise |
|---|---|---|
| yamllint, gitleaks | every selected file | — |
| checkov | files sniffed as K8s manifests (`apiVersion`+`kind`) | NOT_APPLICABLE |
| actionlint | files directly in `.github/workflows/*.yml\|yaml` | NOT_APPLICABLE |
| kubeconform | K8s manifests; needs local schemas | NOT_APPLICABLE / gaps / SKIPPED |
| local-schema | files matching a binding | NOT_APPLICABLE / SKIPPED |

NOT_APPLICABLE is neutral (correctly out of scope); SKIPPED/missing-schema
breaks NO_FINDINGS into INDETERMINATE with visible gaps.

## Notes

- The SQLite JDBC driver prints a benign native-access warning to stderr on
  recent JDKs (never stdout).
- Staged snapshots live under `<data-dir>/transient/<run-id>/` with
  owner-only POSIX permissions where supported and are deleted best-effort
  after each run. Checkov/kubeconform/actionlint see only staged files.

## Pinned stack

JDK 21.0.11-ea · Maven 3.9.11 (wrapper 3.3.2) · OpenJFX 21.0.8 (linux) ·
`io.modelcontextprotocol.sdk:mcp` 0.8.1 · Jackson 2.17.0 · SnakeYAML 2.7 ·
`com.networknt:json-schema-validator` 1.5.9 · slf4j 2.0.16 ·
`org.xerial:sqlite-jdbc` 3.49.1.0 · JUnit 5.13.4 ·
yamllint 1.38.0 · Gitleaks 8.30.1 · Checkov 3.4.4 · actionlint 1.7.12 ·
kubeconform (unversioned dev build). See `pom.xml` header.

# Handover — PWT Java rebuild, Phases 1–4 (CLOSEOUT B)

Checkpoint: 2026-11-13 (Phase 4). Implementation + all executable
verification passed; one external check is blocked (OpenCode model credits).
Working tree on `main`, uncommitted by instruction (HEAD `ac896c9`).
Closeout outcome: **B — implementation complete, external verification
blocked** (see status.md). Next step needs the owner: accept B with the
limitation, or direct Phase 5. Live state: `status.md`.

## 1. What exists and where (new in Phase 4)

- `pwt.mcp.McpScanTools` — 7 tools (5 read-only + start/cancel), cursor/limit
  pagination, bounded report chunks, `OUT_OF_SCOPE`/`PERMISSION_DENIED`/
  `CONFIGURATION_REQUIRED` errors with correlation IDs.
- `pwt.mcp.McpScanManager` — validates synchronously, begins durable runs
  under the repo lock, finishes in a bounded pool; durable + in-process
  cancel; orderly shutdown.
- `pwt.mcp.McpErrors`, `McpPresence` (live/stale/unknown classifier +
  status lines), `McpActivity` (STARTING→CONNECTED flip, 2s heartbeat,
  no-resurrection updates, shutdown close).
- `SqliteStore` V2 (`mcp_sessions`, additive) + session queries.
- `ScanService` validate/begin/finish split + `loadSummary` (+reconcile
  unchanged). GUI/CLI paths behavior-identical.
- GUI: 1s refresh of history + registered-root picker + live presence label
  and per-client details + effective data-dir note; click-to-load runs.
- Launcher: `repo-add`, `repo-list`, `doctor`, `mcp-status`,
  `mcp [--data-dir] [--allow-scans]`, `gui [--data-dir]`.
- `integrations/`: `pwt-mcp-stdio.sh` (single Java+JAR resolution),
  `pwt-dashboard.sh` (single-instance, headless refusal, bracket-trick
  pgrep), `setup_opencode.py`, `setup_claude.py` (merge + preserve mode),
  `pwt-conversation.md`, `opencode/` + `claude-code/` (configs, `/pwt`
  commands with bare-`/pwt`-opens-dashboard, READMEs).
- `Makefile`: full target set (help/doctor/build/test/setup/repo-add/
  repo-list/start/dashboard/setup-opencode/setup-claude/mcp/mcp-status).
- Tests: `McpHarness` + `McpToolsTest` (12), `McpPresenceTest` (5),
  `LauncherCommandsTest` (3, incl. V1→V2 migration + session lifecycle).
  141 total, 0 skipped final. `GuiProbe5` (throwaway, /tmp) for
  history/roots/presence.

## 2. How to verify (from repo root)

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64; export PATH="$JAVA_HOME/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
cd pwt && ./mvnw -B clean test        # expect 141/0/0/0, BUILD SUCCESS
./mvnw -B -q package -DskipTests      # target/pwt-0.4.0.jar
make doctor DATA_DIR=<dir>
make setup DATA_DIR=<dir> && make repo-add ROOT=<abs project>
make setup-opencode ROOT=<abs project> ALLOW_SCANS=1
make setup-claude ROOT=<abs project>
make mcp-status DATA_DIR=<dir>
integrations/pwt-mcp-stdio.sh --data-dir <dir> [--allow-scans]
java -jar pwt/target/pwt-0.4.0.jar gui --data-dir <dir>
python3 /tmp/opencode/mcp_check.py    # independent harness (in /tmp)
```

Client checks: `/tmp/pwtagent` pattern (seed via CLI, register with
`claude mcp add --scope project pwt -- ...`, copy command files, then
`claude -p "/pwt status|report|scan|ask"`). Needs Claude auth (worked
2026-11-13). OpenCode: `opencode mcp list` for health, but `opencode run`
needs funded credits.

## 3. Evidence from this session (all in `/tmp`, not in git)

- `mcp_check.py` (v0.4.0, 8 tools) — ALL PASSED.
- `GuiProbe5.java` + run — history/roots/presence (ALL CHECKS PASSED).
- `pwt-dash2.png` — helper-launched 0.4.0 window (Phase 4 GUI + history pane).
- `/tmp/pwtagent/` — seed runs, `.mcp.json`, `/pwt` command, agent transcripts
  (status/report/scan/ask + stale-note bug catch).
- `opencode.json` in repo root (user-connected PWT MCP, read-only) plus
  `.opencode/commands/pwt.md` + `.opencode/pwt-conversation.md` (installed
  this session for the user's "can't see the command" report).

## 4. Traps for the next implementer

- `getSchema(String)` parses CONTENT in networknt 1.5.9 — always use
  `SchemaLocation.of(uri)` + config builder (unchanged).
- MCP tool handlers must return isError results, never throw (wrapper
  converts `McpServiceException`; anything else → INTERNAL_ERROR).
- `context.activity()` may be null in tests constructing McpContext directly.
- Presence: STARTING flips only via tool calls; heartbeats/close are
  conditional on non-DISCONNECTED; GUI staleness threshold 10s.
- Never match own command lines with plain-string `pgrep -f` (use the
  bracket trick); same for `pkill` in test/debug shells.
- `System.setProperty("pwt.tools.<id>", ...)` must be cleared in `finally`.
- Tests that spawn the MCP server need `pwt-mcp-stdio.sh`-equivalent env
  (java on PATH); the Python check harness needs `--data-dir` (never CWD).
- `make mcp` stdout must stay empty: recipe lines are @-prefixed, the
  launcher prints nothing on stdout, diagnostics go to stderr.
- OpenCode `mcp list` may show "No MCP servers configured" on first touch
  of a fresh dir; retry shows the truth (observed twice, cause unknown —
  likely service caching).
- `ui_files/` PNGs are stale pre-rebuild mockups; ignore them.

## 5. Deferred to Phase 6 (do not sneak into Phase 5)

Fault injection, sandboxing, secure deletion, crash/orphan recovery,
tuning MCP limits under adversarial load, auth model beyond local stdio,
cross-platform validation, performance budgets, log-leak fuzzing, MCP rate
limiting/quotas, broader kind coverage, remote refs.

## 6. Next task (Phase 5, per status.md)

Complete desktop workflows: repository list, full history + filtering,
evidence detail, redacted logs, visible refresh, before/after comparison,
rescan-after-edit. Reuse services as-is.

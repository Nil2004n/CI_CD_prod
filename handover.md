# Handover — PWT Java rebuild, dashboard-only scanning

Checkpoint: 2026-11-13 (dashboard-only scanning complete: no OpenCode
scan/cancel; GUI first-scan self-registers via idempotent runScan hook;
strict current-project agent; OpenCode MCP never scan-enabled; installed
artifacts synced; full suite 161/0/0/2; uncommitted). Live state:
`status.md` (top entry). Previous entries kept as record.

Checkpoint: 2026-11-13 (`make setup-opencode-global`: unscoped read-only
user-global MCP entry; native plugin falls back to it, then resolves cwd
to registered roots; uncommitted). Verified: `mcp list` connected; native
status with no project config (hook log, no model); journal-only agent
answer (frozen presence timestamp); cross-project denial with correlation
ID; honest unregistered-dir error. `make enable` stays the explicit
per-project route. Live state: `status.md` (top entry). Previous entries
kept as record.

## 0z. Dashboard-only delta (this session, read first)

- `ScanService.runScan`: single first-scan hook (register idempotent +
  `ProjectContext.writeIfManaged` + journal path); duplicate pointer block
  removed. `ProjectContext` + setup pointer text now say exactly: "No PWT
  assessment is available for this project. Open /pwt and run its first
  scan in the dashboard."
- `setup_opencode.py`: never writes `--allow-scans` (deprecated flag warns
  on 1, strips + reports legacy flags); analysis-writes unchanged.
  Makefile OpenCode targets (`setup-opencode`, `setup-opencode-global`,
  `enable`) drop `ALLOW_SCANS` passthrough (warn when set); `make mcp` and
  `setup-claude` keep it for deliberate non-OpenCode use.
- TUI/agent/ask already dashboard-only from the prior stack; verified, not
  re-cut: 6 commands, no scan/cancel aliases, repos scoped, ambiguous
  roots ask, unregistered dirs get the dashboard-first message.
- Verify: `make test` (full suite) + `/tmp/pwttest/harness.mjs` with
  `PWT_JAR=pwt/target/pwt-0.4.0.jar` + `repo-list` on one shared DB. Do not
  alter real project YAML for demos (use /tmp fixtures). Never run pkill -f
  with a pattern matching your own shell line — kill by PID.

## 0. New/changed since Phase 4 (for the next developer)

- `integrations/opencode-plugin/tui.ts` (NEW, single file — keep it that
  way): dispatcher (quote-aware splitArgs, repeatable `--file`, deterministic
  PwtAction) + owned persistent MCP client (one ChildProcess per exact spawn
  command incl. `--repo-id`; serialized writes; transparent respawn;
  cleanup on plugin unload). `/pwt <sub> <args>` is the args path; bare
  aliases (`/pwt-scan` …) fire with empty input when accepted from the
  palette. TRAP: ephemeral per-call servers cancel their own scans on
  shutdown (server hook cancels live tokens) — never go back to spawn-per-call.
- `integrations/opencode-plugin/agent-pwt.md` (NEW): global agent rules.
  Installed to `~/.config/opencode/agents/pwt.md` only (never into projects).
- `integrations/install-opencode.py` (NEW) + `make install-opencode /
  uninstall-opencode`: global plugin + agent with managed-markers + refuse
  unmanaged-overwrite. `setup_opencode.py` now installs the project plugin
  dir, merges `--allow-analysis-writes`, deletes stale prompt `pwt.md`.
- Backend: V3 migration (analysis_threads/revisions/assessments +
  `mcp_sessions.analysis_writes`, PRAGMA-guarded ALTER); `ReportService.
  render()` (DB-only) vs `generate()` (explicit export); `ScanService`
  finalize no longer exports; CLI `scan --export`; `pwt mcp --repo-id /
  --allow-analysis-writes`; scoped helpers in `McpScanTools`
  (scopedRepository/scopedRun); `get_analysis`/`save_analysis`;
  `McpContext(allowAnalysisWrites, scopedRepositoryId)`; `pwt-mcp-stdio.sh`
  passes `--repo-id/--allow-analysis-writes/--print-jar`.
- GUI: AGENT ANALYSIS section (read-only, `loadAnalysis` on
  showScanSummary/selection incl. STALE indicator), View/Export report
  buttons, reports label "in database (not exported)", presence "+ analysis".
- `make doctor` now reports opencode version, native-command compatibility,
  and both permission flags (additive JSON fields).
- REMOVED: `integrations/opencode/commands/pwt.md` (prompt template),
  `integrations/opencode/opencode.json` (stale direct-java template);
  README rewritten for native flow. `pwt-conversation.md` += never paste
  secrets into analysis.
- Gotchas: `summarizeRun` must accept both `id` (get_run) and `run_id`
  (list_runs); OpenCode TUI palette accepts the command first — full text
  with args typed in one go may execute the alias with empty input (use
  `/pwt <sub> <args>`); `@pwt` is not routed (use `/pwt-ask` or agents
  dialog); markdown command frontmatter MUST open with `---` (a missing
  first fence silently disables agent selection); `send-keys` needs `-l`
  for `--flags`; `===` echo breaks this zsh tool. Never run pkill -f with
  a pattern that also matches your own shell command line — kill by PID.

## 0a. Agent routing + hang fix (this session)

- Hang: host gateway MCP session went stale (8 tools vs current 10) and
  parked `tools/call` with the 12h default execution timeout; orphans with
  no presence activity were the symptom. Server proven healthy via bounded
  harness (0.7s full handshake both JVMs; missing `notifications/initialized`
  is the only server-side silent drop). Fix: `"timeout": {"execution":
  300000}` on the pwt MCP entry (repo + setup-generated, preserves user
  values); reconnect via `/mcps` if a session wedges.
- `/pwt-ask`: global markdown command with `agent: pwt` frontmatter
  (verified supported), installed by `install-opencode.py` with marker
  protection + uninstall; agent frontmatter now `description` + `mode: all`
  (body unchanged); native dispatcher `ask` case points at `/pwt-ask`.
- Verified live: `/pwt status` native; `/pwt-ask` → Pwt agent answered
  from persisted evidence; propose → approve → edit → rescan
  (COMPLETED/NO_FINDINGS) → save_analysis rev 1 both threads (SQLite
  confirmed); read-only denials PERMISSION_DENIED; unconfigured/empty
  projects honest errors; GUI standalone on same data dir with live
  presence (row-click driving still synthetic-input limited).

## 0b. How to verify (delta)

```sh
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64; export PATH="$JAVA_HOME/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
cd pwt && ./mvnw -B test                       # expect 146/0/0/0
make doctor | python3 -m json.tool             # opencode + permissions fields
rm -rf /tmp/oc3 && mkdir -p /tmp/oc3 && python3 integrations/setup_opencode.py \
  --project-dir /tmp/oc3 --data-dir /tmp/oc3/data --allow-scans 1 --allow-analysis 1
make install-opencode && ls ~/.config/opencode/plugins/pwt ~/.config/opencode/agents/
```
Headless plugin harness: copy `integrations/opencode-plugin/tui.ts` to
scratch, stub `@opencode/plugin`, drive all 8 `run()` handlers against
`java -jar pwt/target/pwt-0.4.0.jar mcp` (expect run-id + COMPLETED +
rendered report + honest cancel; no OpenCode needed). Real TUI: launch in a
registered project dir, `/pwt status|scan|report|cancel` must append to
`PWT_TEST_OUT` hook log with zero chat turns; select `Pwt` in agents dialog
for analysis save (needs `--allow-analysis-writes` in project opencode.json).

## 0c. Known gaps + next concrete Phase 5 task

- GUI history-row click was not driven (synthetic clicks did not reach the
  app under Xvfb); store + loader paths are tested. Drive it with a real
  display session before calling the analysis view fully verified.
- Native plugin headless harness is session evidence only; check a
  repo-owned reproducible script into `integrations/` (Phase 6 at latest).
- Run comparison (fingerprints recorded, no compare tool/view) and the full
  desktop workflow/UI redesign are the Phase 5 scope; start only on
  explicit owner go-ahead. No owner acceptance of any closeout is recorded.
- Current architecture in one paragraph: one shared SQLite database;
  standalone GUI with cross-project visibility; native OpenCode commands via
  persistent per-scope MCP sessions calling the same application services;
  global Pwt agent with per-session project context via scoped MCP tools
  plus OpenCode file tools for approved edits; exports are explicit
  projections, never a second store; no daemon, no cloud, no model needed
  for operations.

---
# Previous handover — Phases 1–4 (CLOSEOUT B, kept as record)

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
cd pwt && ./mvnw -B clean test        # expect 146/0/0/0, BUILD SUCCESS (Phase 4 was 141)
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
2026-11-13). OpenCode: native commands need no credits (verified live 2026-11-13);
agent turns also ran in this environment ($0.00 spend).

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

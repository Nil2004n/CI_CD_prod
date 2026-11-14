# Status — PWT Java rebuild

Updated: 2026-11-13 (dashboard-only scanning: /pwt opens the dashboard;
OpenCode scan/cancel removed; GUI first-scan self-registration; strict
current-project /pwt-ask; OpenCode MCP never scan-enabled). Outcome: **COMPLETE**.

## Release v1.0.0 (2026-11-14)

Owner-requested freeze of the current state as 1.0.0: version bumped from
0.4.0 (pom, `BuildInfo.VERSION`, plugin package.json, MCP serverInfo test,
jar name in READMEs) on top of `dec6114`; `.gitignore` adds `.claude/` and
`.opencode/` (already-tracked `.opencode/` files stay tracked). Tagged
`v1.0.0`; GitHub Release carries `pwt-1.0.0.jar`. Older entries below that
say "0.4.0" are historical. Phase 5–7 scope in `plan.md` is unchanged.

## Dashboard-only scanning (this session)

- **Native commands now**: `/pwt` (dashboard), `/pwt help`, `/pwt repos`
  (current project only), `/pwt status [run-id]`, `/pwt report
  [latest|<run-id>]`, `/pwt dashboard`, plus `/pwt-ask <message>` (agent
  conversation). Aliases: `/pwt-help`, `/pwt-repos`, `/pwt-status`,
  `/pwt-report`, `/pwt-dashboard`. `/pwt scan`, `/pwt cancel`,
  `/pwt-scan`, `/pwt-cancel` are gone: typing scan/cancel answers
  "Scanning and cancellation now happen in the dashboard. Use /pwt."
- **GUI first-scan registration**: explicit scan press validates and
  canonicalizes the chosen readable directory via the idempotent
  `registerRoot` service (plain dirs and Git worktrees; no hooks, no
  clone/fetch/commit/push, no filesystem sweep), persists the row in the
  single `<app-root>/.pwt-data/pwt.db`, writes PWT-owned
  `.pwt/context.md` (unmarked files preserved), scans under the returned
  ID, appends the per-repo journal. Browsing/selecting never scans;
  repeat scans reuse the same ID. `ScanService.runScan` owns the write so
  GUI and headless admin share it; MCP `beginRun` still requires a
  pre-registered repo and never self-registers.
- **A/B isolation proof** (fixtures `/tmp/pwtA`, `/tmp/pwtB`, isolated
  `/tmp/pwt-verify-data`, real checkout YAML untouched): first A scan
  registered A + wrote its pointer; second A scan reused A's ID; first B
  scan registered B independently; `repo-list` shows both repos in one
  DB; journals `<data-dir>/reports/<name>--<repoId>.md` carry distinct
  finding SHAs with no crossover; subdirectory `/tmp/pwtA/sub` resolves
  to `/tmp/pwtA`; unregistered dirs get exactly "No PWT assessment is
  available for this project. Open /pwt and run its first scan in the
  dashboard." with no repo list leaked; copied pointers naming another
  root/journal are refused by rule.
- **Scan permission removed**: repo + global `opencode.json` carry no
  `--allow-scans` (repo keeps opt-in `--allow-analysis-writes` only);
  `setup_opencode.py` never writes `--allow-scans` (warns on `1`,
  strips/reports legacy flags, preserves unrelated keys); Makefile
  OpenCode targets ignore `ALLOW_SCANS` with a warning (`make mcp` and
  `setup-claude` retain it for deliberate non-OpenCode use). Live gate:
  MCP `start_scan` without the flag fails PERMISSION_DENIED with
  correlation ID; headless TUI harness 10/10 (registered set exact, scan
  + cancel route to dashboard, help has no scan/cancel, status read-only,
  repos scoped, unregistered honest). Dashboard path unchanged:
  `pwt-dashboard.sh` present with headless refusal; scan/cancel execute
  through `ScanService.runScan` (suite `ScanServiceTest` 12/12 incl.
  first-scan pointer test); approved edits wrote zero new runs until an
  explicit scan; agent/pwt-ask texts authorize no MCP/SQLite/shell
  retrieval.
- **Installed artifacts synced** via `make install-opencode`: global
  plugin `tui.ts` + `package.json`, agent `pwt.md`, command `pwt-ask.md`
  all byte-identical to source. Full suite: 161 run, 0 failures,
  0 errors, 2 skipped (BUILD SUCCESS).
- **Remaining limitations**: GUI row-click driving still synthetic-input
  limited (scan path verified through shared `runScan` + headless
  harness, not pixel-driven); `@pwt` stays unrouted (use `/pwt-ask`);
  bare `/pwt` opens the dashboard helper (help-via-bare still pending
  owner decision); legacy `~/.pwt-data/pwt.db` untouched (no
  consolidation).

Previous entry (global MCP access) below is kept as record.

Updated: 2026-11-13 (MCP reachable from any directory: global host entry
+ native-plugin global fallback; read-only default). Outcome: **COMPLETE**.

## Global MCP access (this session)

- **`make setup-opencode-global`** writes the `pwt` MCP entry to
  user-global `~/.config/opencode/opencode.json` (merge-preserving; other
  servers untouched), read-only unless flags are passed explicitly. Native
  plugin falls back to it when no project config exists, then resolves cwd
  against registered roots as usual (unregistered dirs get an honest
  error, never silent registration).
- **Verified**: `opencode mcp list` shows `pwt connected`; native
  `/pwt status` works from a directory with no project config (hook log,
  zero model turns); Pwt agent answered from the journal there with zero
  MCP tool calls processed server-side (presence timestamp frozen);
  cross-project `get_run` denied with correlation ID; unregistered
  directory names the exact setup action.
- Unscoped-by-design (explicit local admin): every call is still
  repository-scoped server-side, per-project entries fully replace the
  global one, unknown roots are never auto-registered.

Previous entry (global installation) below is kept as record.

## Global installation (this session)

- **`make install`** (run from the checkout): builds the jar, installs
  `~/.local/bin/pwt` (generated shim, `export PWT_DATA_DIR`, arg-preserving
  `exec` into `integrations/pwt.sh`, which resolves Java + newest jar),
  installs global plugin/agent/`/pwt-ask` (overwrite-guarded), installs a
  `.desktop` entry, warns when `PREFIX` is off PATH (opt-in
  `make install-shell-rc`), then `make enable ROOT=$(CURDIR)` with
  scan+analysis on by default (`ALLOW_*=0` for read-only).
- **`make enable ROOT=<dir>`**: explicit project wiring (repo-add +
  setup-opencode); refuses without an explicit ROOT.
- **Doctor reports** effective DB path, Java, versions, repo/scanner state,
  plus install state (app root, launcher on PATH, plugin/agent/ask
  presence, data writability/owner). New `LauncherCommandsTest` assertions;
  full suite 160/160/0/1 (1 known upstream-network skip).
- **Verified in fake HOME** (`HOME=/tmp/fakehome`, spaces in cwd):
  install/uninstall round-trip; launcher works from unrelated dirs;
  pre-existing unrelated config and decoy files untouched; flags 1/0
  honored; single DB only (`/tmp/fakedata/pwt.db`, 0700), no per-repo DBs;
  project-scoped MCP isolation (NOT_FOUND + correlation ID); stdout stays
  pure JSON-RPC; `make purge-data` asks twice (YES) before deleting.
- **Real install performed**: user-owned locations only
  (`~/.local/bin/pwt`, desktop entry, OpenCode plugin/agent/ask, repo
  entry flags). No root, no sudo, no chmod of user files, no global-config
  changes, no data merges. Data dir set 0700 (Java/setup/Makefile on
  creation; existing dir corrected at install).
- **Agent journal access re-verified** on the installed chain: Pwt agent
  answered from the journal with zero MCP tool calls server-side (presence
  timestamp frozen), no auto-save. Edit approval still conversational, not
  filesystem-enforced (documented, not claimed otherwise).
- **Limitations**: host-managed `start_scan` via agent Code Mode still
  times out while the same server answers direct harnesses in <1s (server
  proven healthy; gateway-side dispatch issue, unchanged); journal chat
  avoids MCP entirely so it is unaffected. No @pwt routing (unchanged).
- **Updates/removal**: rerun `make install` to update; `make uninstall`
  removes only PWT-owned artifacts (data/projects/source kept);
  `make purge-data` is the only destructive path (explicit YES).

Previous entry (scan journals) below is kept as record.

## Scan journals (this session)

- **What**: every terminal scan appends a sanitized Markdown section to
  `<data-dir>/reports/<sanitized-name>--<repoId>.md` (run/state/verdict,
  targets+hashes, per-scanner outcomes with versions, findings with
  rule/severity/category/message/fingerprint, coverage gaps; findings capped
  at 100/section with overflow note). Idempotent (complete run-ID marker
  pair = skip), tail-repair on incomplete sections, foreign files refused,
  journal errors logged without touching scan evidence. `pwt journal`
  command: backfill/retry (`--run-id`, `--check` dry-run) under the repo
  lock. Covers COMPLETED/PARTIAL/FAILED/CANCELLED/INTERRUPTED (incl.
  reconcile path); non-terminal runs rejected.
- **Context pointer**: `pwt repo-add` writes PWT-owned
  `<repo>/.pwt/context.md` (id/repo root/journal path/app+data dirs,
  read-the-journal instruction); `setup_opencode.py` backstops it via
  read-only DB lookup. Unmarked files never overwritten.
- **Agent**: same Pwt ID; conversation reads pointer + journal with ranged
  file reads only. Verified live: "What does the latest scan say?" answered
  from the journal with zero MCP tool calls processed server-side (presence
  timestamp frozen) and zero analysis writes; proposal → approval → edit →
  CLI rescan → new section read back. `/pwt ask` stays a deterministic
  pointer to `/pwt-ask` (no supported forward API); `@pwt` unrouted.
- **No schema change** (journal path derives from existing repo rows);
  backend, scanners, MCP, GUI untouched; no daemon/queue, no new transport.
- **Tests**: 160/160/0/0 (`JournalWriterTest`: 13 checks — create/append,
  separate journals, idempotency, partial/failed/cancelled, skipped
  non-terminal, concurrent no-interleave under repo lock, tail repair,
  foreign-file refusal, error-preserves-evidence, secret hygiene, backfill +
  check, context-pointer marker guard).
- **Known limitation carried over**: host-managed `start_scan` from agent/Code
  Mode context times out (server proven healthy via direct harness on both
  JVMs, 0.5–0.8s); rescans verified via CLI/native paths instead, per
  owner direction to ignore start/cancel for this task.

Previous entry (one-database default) below is kept as record.

## Takeover check (2026-11-13, new development session — no code changes)

Verified now:
- HEAD `2ff8850`, ~41 modified/2 staged-deleted/5 untracked paths, nothing
  committed or discarded.
- `./mvnw -B test` (JDK 21): **147 run, 0 failures, 0 errors, 1 skipped**.
  Skip = `ScanServicePhase3Test.kubeconformUpstreamSchemasValidateForReal`
  ("cannot fetch upstream schemas: Connect timed out", network). The
  "146/0/0/0" figure below is historical.
- Effective DB (Makefile dry-run, repo `opencode.json`, launcher, AppPaths):
  `/home/kali/Desktop/CI_CD_prod/.pwt-data/pwt.db` — schema v3, 2 repos,
  4 runs, 0 analysis threads/revisions/assessments (read-only query).
- Discrepancy: installed global plugin `~/.config/opencode/plugins/pwt/tui.ts`
  (18:05) predates repo `integrations/opencode-plugin/tui.ts` (18:31); the
  installed copy defaults its dashboard/help data dir to `~/.pwt-data` when a
  project MCP entry has no `--data-dir`. Not reinstalled (needs owner OK:
  `make install-opencode`). Agent + `/pwt-ask` installed copies match repo.
- Legacy `/home/kali/.pwt-data/pwt.db`: 155,648 bytes, mtime 17:59
  (unchanged); no PWT GUI process is running now. A read-only `sqlite3`
  open during this check created empty `pwt.db-wal` + `pwt.db-shm` sidecars
  there (no content change); left in place.
- Live process: one `java -jar pwt-0.4.0.jar mcp --data-dir <app>/.pwt-data`
  (OpenCode host-managed); jar was not rebuilt to avoid disturbing it.

## One database for all repositories (this session)

- **Root cause**: no production code ever generated per-repo/per-scan
  databases — every entrypoint already resolved ONE data directory
  (default `~/.pwt-data`, explicit `--data-dir` overrides). Separate
  databases found were explicit scratch/test directories only. The default
  is now centralized: `<application-root>/.pwt-data/pwt.db`, resolved from
  the checkout holding the root Makefile (Java `AppPaths`, both launcher
  scripts, native plugin launcher-path detection, setup helpers default).
  No silent fallbacks: unresolvable root or inaccessible directory fails
  loudly instead of scattering databases.
- **Verified**: GUI (from PWT root), CLI scans from three working
  directories without `--data-dir`, and a project-scoped MCP scan from `/`
  all landed in `/home/kali/Desktop/CI_CD_prod/.pwt-data/pwt.db` (2 repos,
  4 runs). Scoped MCP access still rejects cross-project runs (NOT_FOUND +
  correlation ID). No `pwt.db` inside assessed repos or staging dirs.
  Locking unchanged (busy-path unit tests pass).
- **Existing data preserved, NOT merged**: `/home/kali/.pwt-data/pwt.db`
  (schema v3, 1 repo CI_CD_prod, 2 runs, ~184K; a GUI session is live on
  it) is intact. Consolidation needs explicit owner approval with a
  same-repository dedup/import plan — no backup-file clutter, no copies
  while writers are active.
- `.pwt-data/` added to root `.gitignore`; doctor/mcp-status print the
  effective absolute database path; setup helpers require absolute
  `--data-dir` and print the effective directory.

Previous entry (agent routing + hanging-MCP fix) below is kept as record.

## Agent routing + hanging `pwt_info` fix (this session)

- **Hang root cause**: the host-managed MCP session serving agent/Code Mode
  calls was stale (enumerated 8 tools vs the current 10) and parked
  `tools/call` requests with no timeout (default execution timeout is 12h),
  so `pwt_info` sat at "Waiting for output…" indefinitely. Orphaned PWT
  servers from those attempts never logged presence activity and never
  processed a call. The PWT server itself was proven healthy throughout:
  exact-command bounded harness answered initialize → tools/list (10 tools)
  → tools/call `pwt_info` in 0.7–0.8s on Java 21 and 25; the only
  server-side silent-drop found is a client that skips
  `notifications/initialized` (verified: no response ever, by design of the
  MCP Java SDK). Forcing a gateway reconnect restored a fresh 10-tool
  session: `pwt_info` in ~60ms, full chain
  list_repositories → list_runs → get_run → list_findings in 69ms.
- **Bounded failure added**: `"timeout": {"execution": 300000}` on the
  `pwt` MCP entry (repo `opencode.json` + `setup_opencode.py` generated
  entries, preserving any user-set value). A wedged transport now fails
  fast instead of hanging; reconnect via `/mcps`.
- **Explicit conversation route**: new global `/pwt-ask` markdown command
  (`agent: pwt` frontmatter, verified supported mechanism) + agent
  frontmatter (`description`, `mode: all`; body unchanged). Native
  dispatcher gains an `ask` case pointing at `/pwt-ask`; operational
  commands unchanged and still model-free.
- **Verified live in OpenCode v2.0.26 TUI** (scratch project, seeded
  scan): native `/pwt status` (hook log, zero chat turns); `/pwt-ask`
  invoked the Pwt agent, which retrieved run/findings/coverage, proposed a
  targeted YAML fix, waited, applied on approval, rescanned
  (COMPLETED/NO_FINDINGS), saved revision 1 on both threads (SQLite
  confirmed), no commits. Read-only denials return PERMISSION_DENIED with
  useful messages; unconfigured project and empty registered repo return
  honest errors. GUI launched standalone on the same data dir (presence
  shows the live session); row-click driving still limited to synthetic
  input, same as before.
- `@pwt` remains unrouted (not advertised); `/pwt ask` inside the native
  command is a deterministic pointer, not a model call.

Previous entry (OpenCode native integration) below is kept as record.

## OpenCode native integration — what now works

- **Native commands, no model**: `/pwt help|repos|scan|status|report|
  cancel|dashboard` (+ discoverable aliases `/pwt-help` … `/pwt-dashboard`)
  execute as TUI `run` handlers → owned persistent MCP stdio session →
  direct tool calls. Verified in the real OpenCode v2.0.26 TUI AND a
  headless harness driving the shipped `tui.ts`: 10/10 checks, zero chat
  turns, zero model requests for all command paths.
- **Global install**: `make install-opencode` → `~/.config/opencode/
  plugins/pwt/` (single-file native plugin: dispatcher + persistent
  per-scope MCP client) + `~/.config/opencode/agents/pwt.md` (global Pwt
  agent). Refuses to overwrite unmanaged files; `make uninstall-opencode`
  removes only what it installed. Project setup stays separate:
  `make setup-opencode ROOT=…` (MCP entry + project plugin + conversation
  reference, preserves unrelated config; removes superseded prompt template).
- **Project scoping enforced in code**: `pwt mcp --repo-id <id>` restricts
  every handler (list/get/run/findings/report/start/cancel/analysis);
  cross-project IDs return NOT_FOUND/OUT_OF_SCOPE. Connector spawns one
  persistent server per (command incl. `--repo-id`) — scopes never share a
  session. Unscoped mode is explicit local-admin only.
- **Database-first**: scans complete with no report files
  (`ScanService.finalizeRun` no longer exports); `get_report` renders from
  persisted records; CLI `scan --export` and GUI Export write files
  explicitly. Historical artifacts remain readable.
- **Agent analysis (V3, additive; V1/V2 untouched)**: `analysis_threads`/
  `analysis_revisions` (append-only, optimistic `expected_revision` →
  CONFLICT)/`analysis_assessments` + `get_analysis`/`save_analysis` MCP tools
  gated by independent `--allow-analysis-writes`. Priorities LOW/MEDIUM/HIGH
  separate from scanner severity; cross-project finding/verification refs
  rejected; control chars stripped, texts capped. Sanitized analysis visible
  in the GUI read-only view.
- **Global Pwt agent verified live**: shift+tab agents → `Pwt`; resolved
  project via `list_repositories`, loaded run evidence
  (`list_runs/get_run/list_findings/get_report/get_analysis`), saved
  revision 1 via `save_analysis` (PERMISSION_DENIED path also verified:
  agent stopped and explained instead of working around), no file edits
  without approval, no commit/push. `@pwt` is NOT a routed mention in the
  TUI (typed `@pwt` shows no agent completion) — the agent is selected via
  the agents dialog, then asked directly.
- **GUI**: standalone launch, shared runs + saved analysis sections render,
  View/Export report, presence with analysis permission; selection-driven
  history→analysis population is code-reviewed + store-tested (synthetic
  clicks did not reach the app under Xvfb, so the row-click itself was not
  driven; scan-completion path calls the same loader).

## Verification (2026-11-13 implementation session — historical evidence, not rerun today)

- `./mvnw -B test` in `pwt/` (2026-11-13): **146/146, 0 failures, 0 errors,
  0 skipped** (was 141; +analysis/scoping/report tests, −stale export
  assertions). Full log was ephemeral session evidence.
- Headless plugin harness (ships in this session's evidence, drives real
  `tui.ts` against a real `java -jar` MCP server): registration of all 8
  commands, help/repos/status-empty/scan→RUNNING+run-id/status-terminal/
  report-renders/cancel-honest/bare-`/pwt`-dashboard/unknown-input-error —
  ALL PASS. Plugin bug found + fixed by it: ephemeral per-call servers
  cancelled their own scans on shutdown; connector now caches one persistent
  session per scope.
- Real TUI: all commands appear in-command-palette; `/pwt-help`,
  `/pwt-repos`, `/pwt scan/status/report/cancel` executed natively (hook
  log), scan→COMPLETED shared across clients (model-started run visible to
  native status), zero chat turns for command invocations. `/pwt <args>`
  (main command) is the args path; bare aliases fire with empty input.
- Agent: PERMISSION_DENIED stop-and-explain + allowed save rev 0→1, both
  live; SQLite confirms thread rev 1; scanner tables untouched.
- GUI under Xvfb: launches, sections render, presence "2 active clients",
  screenshots reviewed. `make doctor` shows opencode v2.0.26,
  native-compatible, both permission flags.
- Claude Code artifacts untouched (only shared-backend behavior changes:
  export decoupling + additive V3, both backward compatible).

## Remaining notes (not blockers)

- Owner acceptance status: no explicit owner acceptance of any closeout
  outcome is recorded in the repo. Do not claim it; Phase 5 starts only on
  explicit owner go-ahead.
- GUI history-row click not driven (see above); full Phase 5 redesign not
  started (per instruction).
- `make test` covers Java; the plugin harness is session evidence
  (integrate as a repo script in Phase 6 if wanted).
- Old prompt template `integrations/opencode/commands/pwt.md` and stale
  `integrations/opencode/opencode.json` removed; README rewritten for the
  native flow. Nothing committed (per instruction).

---
# Previous: Phase 4 closeout (superseded for OpenCode, kept as record)

Updated: 2026-11-13 (Phase 4 closeout). This file tracks the **Java rebuild**
(baseline docs `prd.md`/`tech-spec.md`/`architecture.md`/`data-schema.md`,
v0.1 draft; roadmap: Phase 4 MCP → Phase 5 desktop → Phase 6 hardening →
Phase 7 release, see `plan.md` header note). The older Rust P00–P03 track and
its `workbench/` tree are preserved as reference only.

## Closeout outcome: B — IMPLEMENTATION COMPLETE, EXTERNAL VERIFICATION BLOCKED

All required implementation and executable acceptance paths passed, except
one external check blocked by missing model credits:

- **BLOCKED:** OpenCode model-driven `/pwt` command execution. The
  configuration, MCP server health (`✓ pwt connected`), and live presence
  from a real OpenCode spawn are verified, but no agent turn could run:
  `opencode run` fails with "Insufficient balance". Manual verification
  steps: fund the account, open the repo, type `/pwt help`, `/pwt status`,
  `/pwt report latest`, and `/pwt scan --file <f>` (needs `--allow-scans`).

Owner acceptance of this limitation is still required for final closure.
Do not begin Phase 5 without the owner.

## Where the project stands

| Item | State |
|---|---|
| Phase 1 | Implemented + verified: buildable project, JavaFX shell, safe discovery, headless discovery, MCP `pwt_info`. 33 tests. |
| Phase 2 | Implemented + verified: yamllint + Gitleaks adapters, process runner, SQLite V1, reports, GUI/headless scan. 70 tests. |
| Phase 3 | Implemented + verified: Checkov, actionlint, kubeconform, local-schema. 121 tests. |
| Phase 4 | Implemented + verified except the OpenCode UX check above. **141/141 tests, 0 skipped, 0 failures.** |
| Phase 5 | Not started (next task at the bottom of this file). |

## Implemented behavior (Phase 4 closeout)

- **MCP tools** (stdio, protocol-only stdout): `pwt_info` + `list_repositories`
  (availability + canonical roots), `list_runs` (state filter, cursor/limit,
  finding/target counts, bounded gaps), `get_run` (repo/targets/outcomes/
  versions/coverage/reports + source), `list_findings` (severity/adapter
  filters, stable order, evidence objects, provenance), `get_report`
  (json|markdown, offset/length chunks, `next_offset`, never silently
  truncated), `start_scan` + `cancel_scan` (gated by `--allow-scans`,
  PERMISSION_DENIED otherwise). Structured errors with correlation IDs.
- **ScanService split** validate/begin/finish with identical GUI/CLI
  behavior; `loadSummary` reassembles persisted runs for MCP + GUI.
  Durable `cancel_requested` observed by workers in any process; reconcile
  still honors live-process locks (never marks another process's active run
  merely because it is old).
- **Presence (new V2 table, additive)**: per-process session rows
  (STARTING→CONNECTED on first post-handshake tool call with self-reported
  client name/version, 2s heartbeat, last-activity per call, DISCONNECTED on
  orderly shutdown). Heartbeats/close are conditional so nothing resurrects
  a closed session; abrupt death expires to STALE via heartbeat age (10s).
  Multiple independent clients supported; reconnect gets a fresh session.
- **GUI**: 1s shared-store refresh of history + registered-root picker +
  live presence ("MCP: \<client\> connected · \<mode\>", per-client details,
  effective data dir + mismatch note). Click any history row to load that
  persisted run. All DB work off the FX thread; list updates only on change.
- **Launch/config**: `Makefile` (help/doctor/build/test/setup/repo-add/
  repo-list/start/dashboard/setup-opencode/setup-claude/mcp/mcp-status,
  JAVA_HOME/DATA_DIR/ROOT/ALLOW_SCANS overrides), `integrations/pwt-mcp-stdio.sh`
  (single Java+JAR resolution, no version hardcoding, no stdout pollution),
  `repo-add`/`repo-list`/`doctor`/`mcp-status` headless commands,
  `pwt-dashboard.sh` (single instance, headless refusal), GUI `--data-dir`.
- **Agents**: `/pwt` (bare = dashboard), help/status/scan/report/cancel/ask;
  agent-interpreted; shared `pwt-conversation.md`; OpenCode + Claude Code
  setups with rerun-preserves-mode semantics (fresh = read-only).
- Fixed during verification: stale report scope note; MCP tool description.

## Files changed (working tree vs `main`)

- `M pwt/` — MCP tools/scan manager/errors/presence/activity, ScanService
  split + loadSummary, V2 migration + session queries, GUI history/roots/
  presence, launcher modes (repo-add/list, doctor, mcp-status), BuildInfo
  0.4.0, PwtMcpServer flags/lifecycle, report scope note.
- `M Makefile` — full target set + setup scripts wiring.
- `M pwt/pom.xml` — version 0.4.0 (only change).
- `M pwt/README.md` — Phase 4 MCP/integrations/dashboard docs.
- New `integrations/` — mcp-stdio launcher, setup scripts, dashboard
  helper, conversation rules, both clients' configs + commands + READMEs.
- `M status.md`, `M handover.md`, `M plan.md` (one-line: current phase).
- `M .opencode/commands/pwt.md` + `M .opencode/pwt-conversation.md`
  (project-local command install; MCP entry already in repo `opencode.json`).

## Exact commands and actual results

```
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64; export PATH="$JAVA_HOME/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
cd pwt && ./mvnw -B clean test     # Tests run: 141, Failures: 0, Errors: 0, Skipped: 0 — BUILD SUCCESS
./mvnw -B -q package -DskipTests   # target/pwt-0.4.0.jar
make doctor DATA_DIR=<dir>         # java/repos/scanners JSON report
make repo-add ROOT=<dir>           # idempotent registration, same ID twice
make setup-opencode ROOT=<dir> ALLOW_SCANS=1   # rerun preserves mode
make setup-claude ROOT=<dir>                   # fresh default read-only
make mcp-status DATA_DIR=<dir>    # live/stale/disconnected sessions
python3 /tmp/opencode/mcp_check.py # ALL MCP CHECKS PASSED (v0.4.0, 8 tools)
```

- MCP: JUnit stdio harness (10 tool tests incl. prompt start_scan, durable
  cross-call cancel, busy, bindings, secret hygiene) + Python harness —
  ALL PASSED. Stdout JSON-RPC-only throughout.
- Presence: unit classifier/status tests; server-spawn tests (CONNECTED on
  tool call, DISCONNECTED on shutdown, abrupt stays CONNECTED then STALE);
  real Claude spawn observed; `opencode mcp list` health check spawns the
  server (connected ✓).
- GUI: xvfb probe — registered roots combo, history refresh with an
  externally created run, click-to-load, live presence label with client
  identity, cleared on disconnect: ALL CHECKS PASSED. Real :0.0 screenshot.
- Client: Claude Code end to end (`/pwt status|report|scan|ask`, MCP-created
  run visible via get_run, dashboard helper verified directly). OpenCode:
  config + server health + presence verified; model-driven UX BLOCKED.
- One honest skip in one full run (upstream schema download timeout);
  final clean run: 0 skipped.

## Dependency versions (all verified, none invented)

Unchanged from Phase 3 (JDK 21.0.11-ea · Maven 3.9.11 · OpenJFX 21.0.8 ·
MCP SDK 0.8.1 · Jackson 2.17.0 · SnakeYAML 2.7 · networknt 1.5.9 ·
slf4j 2.0.16 · sqlite-jdbc 3.49.1.0 · JUnit 5.13.4 · yamllint 1.38.0 ·
Gitleaks 8.30.1 · Checkov 3.4.4 · actionlint 1.7.12 · kubeconform dev).
Clients: OpenCode v2.0.26, Claude Code 2.1.322. Python 3 stdlib only for
setup scripts (json/shutil/pathlib).

## Known limitations (deferred, mostly Phase 6)

Phase 3 list still applies (no sandbox, best-effort cleanup, reconcile
limits, env sanitation, sqlite stderr warning), plus: MCP limits/sizes are
practical not tuned; history refresh is poll-based; dashboard cannot focus
a specific run in an existing window (documented); `@pwt` unsupported by
both clients (fallbacks documented); OpenCode UX unverified (credits);
`doctor` needs a built jar; `pgrep`-based single-instance check documents
its pattern limitation in code.

## Phase 4 acceptance

| Criterion | Result |
|---|---|
| Read-only MCP tools over shared store | PASS |
| Pagination, bounds, unknown-ID errors | PASS |
| Default denies scans; --allow-scans enables | PASS |
| Prompt start_scan; get_run observes progress/state | PASS |
| Cross-call cancel reaches worker (CANCELLED observed) | PASS |
| Busy/traversal/secret hygiene/stdout purity | PASS |
| Agent retrieves GUI-created run, explains evidence | PASS (Claude) |
| MCP-created run visible in GUI + presence display | PASS (probe) |
| /pwt commands + conversation + dashboard (both clients) | PASS Claude / BLOCKED OpenCode UX |
| Unified setup, repo registration, shared data dir | PASS |
| No second pipeline/storage/format; no edit/Git tools | PASS |
| Regressions (121 Phase 1–3 tests) | PASS (all still green) |

## Next executable Phase 5 task: complete the desktop workflow and UI

Repository list, full run history with filtering, evidence detail view,
redacted logs, visible 1s refresh of active content, before/after
comparison (fingerprints + coverage caveats), rescan-after-edit flow.
Reuse services as-is; no schema redesign. Verify with the probe pattern
plus reopen-persistence proof.

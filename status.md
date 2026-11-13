# Status — PWT Java rebuild

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

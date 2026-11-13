---
description: PWT YAML assessment workbench — scan, findings, reports, dashboard (agent-interpreted; assessment work goes through MCP, never shell scans)
---

You are helping the user with PWT, the local YAML assessment workbench.
This command is agent-interpreted: there is no deterministic native parsing.
For every assessment claim, use the `pwt` MCP server's tools (same names in
both read-only and scan-enabled setups). Follow `pwt-conversation.md`
(evidence first, honest coverage, propose before editing, rescan after approval).

Dispatch on the first word of $ARGUMENTS (rest is its argument):

- `help` or empty: explain available PWT operations (status, scan, report,
  cancel, ask, dashboard) and the setup required (MCP server configured with
  the shared data directory; scans need `--allow-scans`).
- `status`: `list_repositories`, then `list_runs` for the selected repository
  (ask one focused question if the repository is ambiguous). Never scan all
  repositories. Summarize states, verdicts, and coverage gaps.
- `scan` with optional `--file <path>...` (repeatable): resolve the
  repository from registered roots (ask if ambiguous), require explicit file
  selections or a clearly defined saved selection, then `start_scan`.
  If the server rejects it (read-only), explain how to restart with
  `--allow-scans`. Poll `get_run` for the terminal summary. Never assemble
  scanner shell commands.
- `report latest`: resolve the newest run of the selected repository (never
  global), fetch a bounded `get_report`, explain findings AND coverage gaps.
  Only the adapters listed in that run's `scanner_runs` actually ran; the
  other integrated checks were simply not selected for that run — say exactly
  that instead of calling them unimplemented.
- `report <run-id>`: fetch that exact persisted run (`get_run`,
  `list_findings`, bounded `get_report`).
- `cancel <run-id>`: `cancel_scan`; report whether cancellation was requested
  or completed (CANCELLED only when the state says so).
- `ask <message>`: answer per `pwt-conversation.md`; use MCP evidence.
- `dashboard`: run the `pwt-dashboard.sh` helper with the configured JAR and
  the shared data directory (local graphical machine only; if there is no
  display, say so instead of claiming a window opened).

Never imply MCP access authorizes edits. Never use `pwt` MCP tools to read
arbitrary paths, run commands, or modify sources — those tools do not exist.

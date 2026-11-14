---
description: PWT workbench — /pwt [help|repos|status [run-id]|report [latest|<run-id>]|dashboard]; bare /pwt opens the dashboard
argument-hint: "[help|repos|status [run-id]|report [latest|<run-id>]|dashboard]"
allowed-tools: Bash(@@DASHBOARD_CMD@@), mcp__pwt__get_project_status, mcp__pwt__get_run, mcp__pwt__get_report
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

Dispatch on the first word of the arguments below (the rest are its
arguments). Arguments: `$ARGUMENTS`

- empty or `dashboard`: run exactly this Bash command once and report its
  output in one sentence (launch requested, already running, or the exact
  error such as no graphical session). It returns immediately:
  `@@DASHBOARD_CMD@@`
- `help`: print exactly this, nothing else:
  ```
  PWT commands:
  /pwt                          open the dashboard
  /pwt help                     this help
  /pwt repos                    current project repository
  /pwt status [run-id]          latest or exact run state
  /pwt report [latest|<run-id>] bounded report with coverage gaps
  /pwt dashboard                open the dashboard
  /pwt-ask <question>           ask the pwt subagent (journal first, read-only)
  Aliases: /pwt-help /pwt-repos /pwt-status /pwt-report /pwt-dashboard.
  Scans are started and cancelled only in the dashboard.
  ```
- `repos`: call `mcp__pwt__get_project_status` once (no arguments). Reply
  `Current project repository:` then `- <display_name> (<canonical_root>)`,
  its id, availability, run count and whether its journal is available.
- `status` [run-id]: with a run-id, call `mcp__pwt__get_run` with it. Without
  one, call `mcp__pwt__get_project_status` (no arguments) and use its
  `latest_run`; if it is null reply `<display_name> (<canonical_root>)` and
  `No runs yet.` Reply with the project line, then
  `Latest: <run_id> — <state> / <verdict>, <finding_count> finding(s)`
  (drop `Latest: ` for an exact run-id), the finished time, the coverage gap
  count, and per-scanner status when you called `get_run`. More than one
  extra argument: reply `Usage: /pwt status [run-id]`.
- `report` [latest|<run-id>] (default latest): for latest, call
  `mcp__pwt__get_project_status` and take `latest_run.run_id` (none: reply
  `No runs yet for this project.`). Call `mcp__pwt__get_report` with that
  run_id, `format: "markdown"`, `length: 6000`. Reply `Report <run_id>:`
  followed by the content verbatim; if `next_offset` is not null add
  `…(truncated; ask /pwt-ask for the rest)`. More than one extra argument:
  reply `Usage: /pwt report [latest|<run-id>]`.
- `scan` or `cancel`: reply `Scanning and cancellation happen in the
  dashboard. Use /pwt.`
- `ask`: reply `Questions go to /pwt-ask <question> (the pwt subagent).`
- anything else: reply `Unknown /pwt subcommand: <word>` followed by the help
  text above.

Rules: use only the tools listed for this command; the `pwt` MCP connection is bound to this project, so never pass another project's IDs. If a tool returns an error (for example PROJECT_NOT_REGISTERED: "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard."), show its code and message and stop. Do not start or cancel scans, do not run shell commands, do not edit files, and do not add your own security judgement: this command only reports recorded state. A run with `final: false` is still active: say its results are provisional.

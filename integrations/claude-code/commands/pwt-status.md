---
description: PWT run status, latest or exact run (same as /pwt status [run-id])
argument-hint: "[run-id]"
allowed-tools: mcp__pwt__get_project_status, mcp__pwt__get_run
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

Arguments: `$ARGUMENTS`

More than one argument: reply `Usage: /pwt-status [run-id]` and stop.

With a run-id, call `mcp__pwt__get_run` with it. Without one, call
`mcp__pwt__get_project_status` (no arguments) and use its `latest_run`; if
it is null reply `<display_name> (<canonical_root>)` and `No runs yet.`

Reply with the project line `<display_name> (<canonical_root>)`, then
`Latest: <run_id> — <state> / <verdict>, <finding_count> finding(s)` (drop
`Latest: ` for an exact run-id), the finished time, the coverage gap count,
and per-scanner status when you called `get_run`.

Rules: use only the tools listed for this command; the `pwt` MCP connection is bound to this project, so never pass another project's IDs. If a tool returns an error (for example PROJECT_NOT_REGISTERED: "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard."), show its code and message and stop. Do not start or cancel scans, do not run shell commands, do not edit files, and do not add your own security judgement: this command only reports recorded state. A run with `final: false` is still active: say its results are provisional.

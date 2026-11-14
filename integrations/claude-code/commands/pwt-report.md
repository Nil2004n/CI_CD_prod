---
description: PWT bounded report with coverage gaps (same as /pwt report [latest|<run-id>])
argument-hint: "[latest|<run-id>]"
allowed-tools: mcp__pwt__get_project_status, mcp__pwt__get_report
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

Arguments: `$ARGUMENTS` (empty means latest)

More than one argument: reply `Usage: /pwt-report [latest|<run-id>]` and stop.

For latest, call `mcp__pwt__get_project_status` (no arguments) and take
`latest_run.run_id`; if there is none reply `No runs yet for this
project.` Otherwise use the given run-id.

Call `mcp__pwt__get_report` with that run_id, `format: "markdown"`,
`length: 6000`. Reply `Report <run_id>:` followed by the content verbatim;
if `next_offset` is not null add `…(truncated; ask /pwt-ask for the rest)`.

Rules: use only the tools listed for this command; the `pwt` MCP connection is bound to this project, so never pass another project's IDs. If a tool returns an error (for example PROJECT_NOT_REGISTERED: "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard."), show its code and message and stop. Do not start or cancel scans, do not run shell commands, do not edit files, and do not add your own security judgement: this command only reports recorded state. A run with `final: false` is still active: say its results are provisional.

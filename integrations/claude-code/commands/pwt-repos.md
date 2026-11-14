---
description: PWT current project repository (same as /pwt repos)
allowed-tools: mcp__pwt__get_project_status
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

Call `mcp__pwt__get_project_status` once (no arguments). Reply
`Current project repository:` then `- <display_name> (<canonical_root>)`,
its id, availability, run count and whether its journal is available.

Rules: use only the tools listed for this command; the `pwt` MCP connection is bound to this project, so never pass another project's IDs. If a tool returns an error (for example PROJECT_NOT_REGISTERED: "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard."), show its code and message and stop. Do not start or cancel scans, do not run shell commands, do not edit files, and do not add your own security judgement: this command only reports recorded state. A run with `final: false` is still active: say its results are provisional.

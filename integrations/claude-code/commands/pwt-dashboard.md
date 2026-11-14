---
description: Open the PWT dashboard (same as bare /pwt; scans are started and cancelled only there)
allowed-tools: Bash(@@DASHBOARD_CMD@@)
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

The PWT dashboard launcher already ran before this message; its output:

!`@@DASHBOARD_CMD@@`

Tell the user in one or two sentences what the output above says (window
launch requested, already running, or the exact error such as "no graphical
session"). Do not claim a window opened unless the output says a launch was
requested or a window is already running.

Then stop. Do not run any other command, do not call any `pwt` MCP tool,
and do not start, cancel, or schedule scans: scans are started and cancelled
only in the dashboard. If the user typed text after `/pwt-dashboard` ($ARGUMENTS),
explain that `/pwt-dashboard` only opens the dashboard and that questions about this
project's assessment go to `/pwt-ask <question>`.

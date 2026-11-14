---
description: Ask the PWT subagent about this project's YAML assessment (journal first, read-only tools)
argument-hint: <question about this project's PWT findings, coverage, failures or runs>
---
<!-- PWT-CLAUDE-MANAGED: installed by `make install-claude`; removed by `make uninstall-claude`. Edits are overwritten on reinstall. -->

If the question below is empty, ask the user what they want to know about
this project's PWT assessment and stop.

Otherwise delegate it to the `pwt` subagent (Agent/Task tool with
subagent_type `pwt`). Pass the question verbatim plus the current working
directory. Do not investigate it yourself first and do not call `pwt` MCP
tools yourself.

Question: $ARGUMENTS

When the subagent returns:

1. Relay its answer faithfully. Keep its separation of recorded evidence,
   interpretation, unknowns and proposed remediation, and keep the run IDs
   and times it cites. Do not upgrade "not run", "skipped" or "timed out"
   into "passed", and do not call anything fixed or secure.
2. If it returned a proposed patch, show the patch and ask the user to
   approve it. Apply only what the user explicitly approves, with your normal
   Edit tool (the usual permission prompt applies), only to YAML/YML files
   inside this project. Re-read each file first; if the current content no
   longer matches the patch context, stop and say the proposal is stale.
3. After any edit: list the files changed, say that the saved scan describes
   the pre-edit files, and ask the user to open `/pwt` and run a verification
   scan from the dashboard. Do not claim the finding is fixed until a new scan
   shows it.
4. Never call `start_scan` or `cancel_scan`, never run scanners or PWT
   commands through the shell, and never commit or push.

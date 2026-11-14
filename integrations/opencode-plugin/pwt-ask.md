---
description: Ask the Pwt security agent about the latest PWT assessment (PWT-ASK-MANAGED)
agent: pwt
---

$ARGUMENTS

(Answer from this repository's PWT scan journal only: resolve the current project root, read its .pwt/context.md pointer, validate that the pointer names this same project, then read the latest completed scan section with ranged file reads. Do not call PWT MCP tools, SQLite, or shell commands for the question; do not read other projects' journals; do not start scans or edits unless explicitly asked. If no pointer or journal exists, say exactly: "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard." Never claim repository-wide conclusions.)

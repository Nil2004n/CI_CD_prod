# PWT integrations

Thin, client-native wiring between coding agents and the PWT MCP server.
No embedded chatbot, no model backend, no source-edit/Git MCP tools.

| Path | What |
|---|---|
| `pwt-dashboard.sh` | Fixed-entrypoint dashboard launcher (local display only, single instance). Usage: `pwt-dashboard.sh --jar /abs/pwt-X.jar [--data-dir DIR] [--java BIN]`. |
| `pwt-conversation.md` | Reusable agent instructions (evidence-first, honest coverage, propose-before-edit, rescan-after-approval). Referenced by both clients below. |
| `opencode/` | OpenCode setup: MCP config template, `/pwt` custom command, dashboard instructions. |
| `claude-code/` | Claude Code setup: `claude mcp add` lines, `/pwt` slash command, dashboard instructions. |

Placeholders like `/abs/path/pwt-0.4.0.jar` and `/abs/path/pwt-data` must be
replaced with real absolute paths. Nothing here modifies global client
configuration: copy the project-local files into your own project (or install
the command files where your client looks for project commands).

Removal: delete the copied files and the MCP server entry
(`opencode mcp remove` is not a command — edit `opencode.json`;
`claude mcp remove pwt`), then delete your data directory if desired.

`@pwt` mentions: neither OpenCode nor Claude Code documents custom `@`-mention
routing. Use `/pwt ask <message>` as the consistent entry point.

# PWT integrations

Thin, client-native wiring between coding agents and the PWT MCP server.
No embedded chatbot, no model backend, no source-edit/Git MCP tools.

| Path | What |
|---|---|
| `pwt-dashboard.sh` | Fixed-entrypoint dashboard launcher (local display only, single instance). Usage: `pwt-dashboard.sh [--jar /abs/pwt-X.jar] [--data-dir DIR] [--java BIN] [--detach]` (no `--jar`: resolved like the MCP launcher; `--detach`: return immediately). |
| `pwt-conversation.md` | Reusable agent instructions (evidence-first, honest coverage, propose-before-edit, rescan-after-approval). Referenced by both clients below. |
| `opencode/` | OpenCode setup: native command plugin, global plugin/agent installer, project setup helper, per-project MCP entry. See `opencode/README.md`. |
| `opencode-plugin/` | Shipped native TUI plugin source (`tui.ts`, single file) and the global agent definition (`agent-pwt.md`). Installed by `install-opencode.py` / `setup_opencode.py`; do not hand-edit installed copies. |
| `install-opencode.py` | Global installer: `~/.config/opencode/plugins/pwt/` + `~/.config/opencode/agents/pwt.md`, with unmanaged-overwrite protection and uninstall. |
| `claude-code/` | Claude Code templates (`/pwt`, `/pwt-ask`, `pwt` subagent) and guide. |
| `install_claude.py` | Claude Code user-wide installer/uninstaller and `claude-status` (optional real MCP handshake). |
| `setup_claude.py` | Project `.mcp.json` `pwt` entry bound with `--project-root` (never `--allow-scans`). |
| `tests/` | Installer regression tests (`make test-integrations`). |

Placeholders like `/abs/path/pwt-1.1.0.jar` and `/abs/path/pwt-data` must be
replaced with real absolute paths. Nothing here modifies global client
configuration: copy the project-local files into your own project (or install
the command files where your client looks for project commands).

Removal: delete the copied files and the MCP server entry
(`opencode mcp remove` is not a command — edit `opencode.json`;
`claude mcp remove pwt`), then delete your data directory if desired.

`@pwt` mentions: neither OpenCode nor Claude Code routes custom `@`-mention
shortcuts to the Pwt agent. In OpenCode, use `/pwt-ask <message>` (selects
the `Pwt` agent) or select the `Pwt` agent in the agents dialog and ask
directly (native `/pwt` commands take no model-driven `ask` path; typing
`/pwt ask` returns a deterministic pointer to `/pwt-ask`). In Claude Code,
use `/pwt-ask <message>` (delegates to the `pwt` subagent).

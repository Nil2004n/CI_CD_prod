# OpenCode setup for PWT

Verified against OpenCode v2.0.26 (`opencode mcp add` writes project-local
`opencode.json`; commands per https://opencode.ai/docs/commands/).

## 1. Register the MCP server (project-local, read-only)

From your project directory, either copy the template or run:

```sh
cp /abs/path/integrations/opencode/opencode.json ./opencode.json
# then replace /abs/path/pwt-0.4.0.jar and /abs/path/pwt-data
```

or equivalently:

```sh
opencode mcp add pwt -- java -jar /abs/path/pwt-0.4.0.jar mcp --data-dir /abs/path/pwt-data
```

Check it: `opencode mcp list` (the server starts on demand via stdio; stdout
stays protocol-only, diagnostics go to stderr).

To enable scans, add `"--allow-scans"` to the command array:

```jsonc
"command": ["java", "-jar", "/abs/path/pwt-0.4.0.jar",
            "mcp", "--data-dir", "/abs/path/pwt-data", "--allow-scans"]
```

Read-only is the default. MCP permission gating is enforced server-side; a
client approval alone never enables scans.

## 2. Install the `/pwt` command

```sh
mkdir -p .opencode/commands
cp /abs/path/integrations/opencode/commands/pwt.md .opencode/commands/pwt.md
cp /abs/path/integrations/pwt-conversation.md .opencode/pwt-conversation.md
```

Use: `/pwt help`, `/pwt status`, `/pwt scan --file k8s/pod.yaml`,
`/pwt report latest`, `/pwt report <run-id>`, `/pwt cancel <run-id>`,
`/pwt dashboard`, `/pwt ask <message>`. Bare `/pwt` opens the dashboard.
The command is agent-interpreted; assessment operations go through the
`pwt` MCP tools, never shell scans. To register another repository:
`make repo-add ROOT=/abs/project` (shared store) or `pwt repo-add`.

## 3. Dashboard

```sh
/abs/path/integrations/pwt-dashboard.sh --jar /abs/path/pwt-0.4.0.jar --data-dir /abs/path/pwt-data
```

Local graphical machine only; refuses without a display; never opens a
second window if one is already running.

## 4. Shared data directory

GUI, CLI, and MCP must point at the same directory (GUI:
`java -jar ... gui --data-dir DIR`; CLI: `scan --data-dir DIR`). That one
directory holds repositories, runs, findings, and reports for all three.

## 5. Removal

Delete `.opencode/commands/pwt.md`, `.opencode/pwt-conversation.md`, and the
`pwt` entry from `opencode.json`. Delete the data directory if desired.

## 6. Verified vs not

- MCP registration + server startup: VERIFIED (project-local config).
- `/pwt` command UX end-to-end: NOT VERIFIED — the installed OpenCode
  account has no model credits (`opencode run` fails with "Insufficient
  balance"), so no agent turn could be executed. Templates follow the
  documented commands mechanism; re-verify once the account is funded.

# Claude Code setup for PWT

## 1. Register the MCP server (read-only default)

Scope `user` keeps one entry for all projects; scope `project` writes
`.mcp.json` in the current project. Read-only first:

```sh
claude mcp add --scope user pwt -- java -jar /abs/path/pwt-0.4.0.jar mcp --data-dir /abs/path/pwt-data
```

Scan-enabled variant (separate name so the default stays read-only):

```sh
claude mcp add --scope user pwt-scan -- java -jar /abs/path/pwt-0.4.0.jar mcp --data-dir /abs/path/pwt-data --allow-scans
```

Check it: `claude mcp list`. MCP permission gating is enforced server-side;
a client approval alone never enables scans.

## 2. Install the `/pwt` command (project-local)

```sh
mkdir -p .claude/commands
cp /abs/path/integrations/claude-code/commands/pwt.md .claude/commands/pwt.md
cp /abs/path/integrations/pwt-conversation.md .claude/pwt-conversation.md
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

`claude mcp remove pwt` (and `pwt-scan` if added); delete
`.claude/commands/pwt.md` and `.claude/pwt-conversation.md`. Delete the data
directory if desired.

## 6. Verified vs not

End-to-end verification results are recorded in the project `handover.md`.
`@pwt` mentions are not a routed mechanism in Claude Code (mentions address
files); use `/pwt ask <message>`.

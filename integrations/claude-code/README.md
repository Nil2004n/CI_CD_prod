# Claude Code integration for PWT

Workflow:

1. `/pwt` opens the PWT dashboard. Scans are started and cancelled only there.
   Like OpenCode, `/pwt help|repos|status [run-id]|report [latest|<run-id>]|dashboard`
   and the aliases `/pwt-help`, `/pwt-repos`, `/pwt-status`, `/pwt-report`,
   `/pwt-dashboard` report recorded state through the project-bound read-only
   tools (`get_project_status`, `get_run`, `get_report`; pre-approved in each
   command's `allowed-tools`). `/pwt scan|cancel` only point to the
   dashboard. In Claude Code every command is a model turn; `/pwt-dashboard`
   launches deterministically before the model runs, bare `/pwt` runs the same
   pre-approved, non-blocking dashboard command as its first step.
2. `/pwt-ask <question>` delegates to the `pwt` subagent.
3. The subagent reads this project's scan journal first and uses read-only,
   project-bound MCP tools only when the journal is not enough.
4. It explains findings, coverage gaps, execution failures and differences
   between this project's runs, and drafts minimal YAML fixes as proposed
   patches. The parent session applies a patch only after you approve it,
   through Claude Code's normal Edit permission prompt.
5. Nothing in this integration starts or cancels a scan, registers a project,
   or commits/pushes.

## Install

```sh
make build                              # the MCP server needs the current jar
make install-claude                     # user-wide /pwt, /pwt-ask, pwt subagent
make setup-claude ROOT="/abs/project"   # project .mcp.json entry, bound to that project
make claude-status ROOT="/abs/project" TEST=1
```

- `install-claude` writes `commands/pwt.md`, `pwt-dashboard.md`, `pwt-help.md`,
  `pwt-repos.md`, `pwt-status.md`, `pwt-report.md`, `pwt-ask.md` and
  `agents/pwt.md` under `$CLAUDE_CONFIG_DIR` (default `~/.claude`). Each file
  carries a `PWT-CLAUDE-MANAGED` marker; an existing file without it is never
  overwritten (nothing is written). Rerunning is idempotent. `DATA_DIR=` is
  baked into the dashboard command.
- `setup-claude` merges one `pwt` server into `<project>/.mcp.json` and keeps
  every other server. The entry is
  `pwt-mcp-stdio.sh --data-dir <shared dir> --project-root <canonical project>`.
  It never adds `--allow-scans`; `ALLOW_SCANS=1` is refused and an older entry
  that had it is downgraded. `ALLOW_ANALYSIS=1|0` independently enables or
  removes `--allow-analysis-writes` (omitted preserves; fresh entries: denied).
  An older PWT-generated, scan-capable project `.claude/commands/pwt.md` is
  removed; a user-owned file with that name is kept and reported.
- Claude Code asks you to approve a project `.mcp.json` server the first time
  it is used. The installer does not change client trust or permission
  settings; approve `pwt` when prompted.
- `claude-status` reports artifacts (current / outdated / unmanaged),
  registration (read-only database query), the project MCP entry and its
  permissions. With `TEST=1` it runs a real stdio MCP handshake with the
  configured command (initialize, tools/list, `get_project_status`, and a
  `start_scan` call that must return PERMISSION_DENIED); without it the
  connection is reported NOT TESTED.
- `uninstall-claude [ROOT=...]` removes only PWT-managed user files and, with
  `ROOT`, that project's PWT-created `pwt` entry. The shared data directory,
  the launcher and OpenCode files are untouched.

`make install` runs the same `install-claude` step when Claude Code is present
(`claude` on PATH or an existing config directory). A missing Claude Code or a
conflicting unmanaged file only prints a warning; it never adds an MCP entry,
registers a project or scans. `make uninstall` removes the PWT-managed Claude
files too. Project `.mcp.json` entries are only ever added or removed
explicitly, with `make setup-claude ROOT=...` and `make uninstall-claude ROOT=...`.

### Global entry (like OpenCode's `setup-opencode-global`)

```sh
make setup-claude-global [ALLOW_ANALYSIS=1|0]
```

Adds one user-scope `pwt` MCP server with the supported
`claude mcp add-json -s user` CLI, so PWT is available in every directory
without per-project setup. It is not unscoped: it runs the launcher with
`--project-root-cwd`, and Claude Code starts stdio servers in the directory
the session was started in (verified with Claude Code 2.1.325), so each
session binds to the registered project owning that directory. In an
unregistered directory every tool returns PROJECT_NOT_REGISTERED and never
falls back to another project. A project's own `.mcp.json` `pwt` entry
(`make setup-claude`) overrides the global one. `ALLOW_SCANS=1` is refused;
`ALLOW_ANALYSIS` works as for `setup-claude`. A user-scope `pwt` entry that PWT
did not create is never replaced. `uninstall-claude` / `make uninstall` remove
the PWT-created entry. Like the OpenCode global entry, it is not part of
`make install`. User-scope servers do not need the project approval prompt.

Registration is unchanged: the first dashboard scan registers a project, or
use `make repo-add ROOT=...` / `pwt repo-add --root <dir>`.

## Project identity and scope

The MCP server is started with `--project-root <dir>` and, on every call,
resolves the registered repository owning that directory: the directory is
canonicalised (symlinked spellings and subdirectories resolve to the same
project) and the deepest registered root containing it wins (nested
projects bind to the inner one). There is no user-wide unscoped entry.

- Not registered yet: every tool returns `PROJECT_NOT_REGISTERED` ("open
  /pwt and run its first scan"). Nothing is registered, and nothing falls back
  to another project. A later dashboard scan is picked up without
  restarting Claude Code.
- Directory missing: `PROJECT_UNAVAILABLE`. Two registrations of the same
  canonical root: `PROJECT_AMBIGUOUS`.
- The first successful binding is pinned for the session. If the directory
  later resolves to a different repository (for example a nested project
  registered mid-session), calls fail with `PROJECT_SCOPE_CHANGED`; start a
  new session.
- Run IDs or repository IDs from another project return `NOT_FOUND` /
  `OUT_OF_SCOPE`, without naming that project, its root or its findings.

## MCP tools the subagent may use (read-only)

| Tool | Purpose |
|---|---|
| `get_project_status` | identity, root availability, latest run (`final` or provisional), active runs, run count, journal path/availability, server permissions |
| `list_runs` | this project's history, paginated (limit 1–500, cursor) |
| `get_run` | one run: targets, per-scanner status/exit code/message with `execution` class, coverage gaps, recorded run error |
| `list_findings` | findings of one run; exact filters `severity`, `adapter_id`, `rule_id`, `relative_path`; evidence capped |
| `compare_runs` | two runs by recorded fingerprint: only in base / only in head (each with `comparable`), in both, possible line shifts, file and scanner differences, limitations; never labels anything fixed |
| `get_report`, `get_analysis` | bounded rendered report; saved analysis |

`start_scan`, `cancel_scan` and `save_analysis` exist on the server but are
not in the subagent's tool list. On a Claude connection the server denies the
first two (no `--allow-scans`); `save_analysis` needs the explicit
`ALLOW_ANALYSIS=1`. Per-run event logs are not persisted by PWT; execution
diagnostics come from the recorded scanner outcomes and run error.

## Edits

The subagent runs on the `haiku` model (`model: haiku` in its frontmatter;
the parent session keeps its own model) and has Read/Grep/Glob plus the read
tools above: no Edit, Write or Bash. It returns proposed patches; the parent session shows them and edits
only after your approval. Its project boundary is an instruction, not an
operating-system sandbox. After an edit, the saved scan still describes the
pre-edit files: run a verification scan from `/pwt` before calling anything
fixed.

## Not supported

`@pwt` mentions are not a routed mechanism (they address files). Use
`/pwt-ask <question>`.

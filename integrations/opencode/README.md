# OpenCode setup for PWT (native commands, no model involved)

Verified against OpenCode v2.0.26. PWT ships a native TUI plugin: slash
commands call the `pwt` MCP server directly through an owned persistent
stdio session. No prompt expansion, no model turn, no credits required.

## 1. One-time global install

```sh
make install                       # recommended: launcher + plugin/agent/ask +
                                 # desktop entry + enable current project
# or, lower level:
make install-opencode              # only the global plugin/agent/ask files
```

`make install` (run from the checkout) installs the `pwt` launcher into
`~/.local/bin`, the native command plugin into
`~/.config/opencode/plugins/pwt/`, the global Pwt agent, the global
`/pwt-ask` command, a desktop entry, then enables the current project
read-only (pass explicit flags to change modes). Everything refuses to
clobber unmanaged files; rerun to update. Removal: `make uninstall`
(data, projects, and source kept).

## 2. Per-project setup (read-only by default)

```sh
make enable ROOT=/abs/project
# equivalent lower level:
make repo-add ROOT=/abs/project
make setup-opencode ROOT=/abs/project
```

Add `ALLOW_SCANS=0|1` / `ALLOW_ANALYSIS=0|1` only to explicitly change a
permission; omitting a flag preserves the existing entry (fresh projects
stay read-only). OpenCode configuration must not request scanning:
scans run only from the dashboard.

## 2b. Host MCP from any directory (no project setup)

```sh
make setup-opencode-global [ALLOW_ANALYSIS=0|1]
```

Writes the `pwt` entry to user-global `~/.config/opencode/opencode.json`
(merge-preserving; other servers untouched), so host-managed MCP tools work
in sessions without a project `opencode.json`. The native plugin also falls
back to this entry when no project config is found, resolving the current
directory against registered roots as usual; unregistered directories get
an honest error, never silent registration.

The global entry is deliberately unscoped (explicit local admin) and
read-only — pass `ALLOW_ANALYSIS=1` only if you accept project-wide analysis
writes from anywhere (scan permission is never written). Every call is still repository-scoped
server-side (`--repo-id` per call or per resolved project; unknown and
cross-project IDs are rejected), and any per-project entry fully replaces
the global one for that project. `make enable` remains the explicit,
preferred project route.

- Merges the `pwt` MCP server entry into `/abs/project/opencode.json`
  (preserves unrelated keys/servers; never touches global config).
- Installs the native plugin into `/abs/project/.opencode/plugins/pwt/`
  and removes any superseded model-interpreted `.opencode/commands/pwt.md`.
- Copies the conversation reference to `.opencode/pwt-conversation.md`
  (documentation for the agent; not a command).
- Registers nothing by itself: first dashboard scans self-register;
  `make repo-add ROOT=/abs/project` remains for headless admin.

Permission modes are independent flags: `--allow-scans` gates
`start_scan`/`cancel_scan`; `--allow-analysis-writes` gates
`save_analysis`. Gating is enforced server-side. PWT-owned setup does not
pass scan flags; analysis writes stay opt-in per project. Unscoped mode
exists only as explicit local admin.

## 3. Commands (deterministic, direct MCP calls)

```sh
/pwt help                         # this help
/pwt repos                         # current project repository only
/pwt status [run-id]               # latest or exact run state
/pwt report [latest|<run-id>]      # bounded report rendered from the database
/pwt dashboard                     # opens the GUI (local display only)
/pwt-ask <message>                 # ask the Pwt agent (conversation, uses model)
/pwt-help /pwt-repos /pwt-status /pwt-report /pwt-dashboard
```

Scanning and cancellation happen only in the dashboard: pick files and
press scan there, or cancel the running scan there. There are no OpenCode
scan or cancel commands; `/pwt scan`, `/pwt cancel` (and the removed
`/pwt-scan`, `/pwt-cancel` aliases) answer with a pointer to `/pwt`.

Bare `/pwt` opens the dashboard helper (refuses headless with a clear
message; pending owner decision: a requested design had bare `/pwt` show
help instead — code unchanged for now).

Conversation uses the model through one explicit route:

```sh
/pwt-ask <message>              # selects the Pwt agent, asks in current session
```

`/pwt-ask` is a global markdown command (installed by
`make install-opencode`) whose frontmatter selects the `Pwt` agent; your
text is submitted as the prompt. Typing `/pwt ask ...` inside the native
command instead returns a deterministic pointer to `/pwt-ask` — native
commands never invoke the model. `@pwt` is not a routed mention in the
TUI (typing it shows no agent completion); use `/pwt-ask` or the agents
dialog.

## 4. Agent (`Pwt` agent: one project only, journal-first)

Select the `Pwt` agent (agents dialog or `/pwt-ask <message>`), then talk
normally. The agent serves exactly the current OpenCode project: it
resolves the project root from the working directory, reads that
project's `.pwt/context.md` pointer, validates the pointer names this
same project, and then reads only that project's scan journal
(`<data-dir>/reports/<name>--<repoId>.md`, latest completed section
first, ranged reads only). No MCP, SQLite, or shell calls in the
conversation path; no other project's journal is ever read; unregistered
directories and copied pointers from other repositories are refused with
the exact setup action. The journal is appended automatically at every
terminal scan; it is a reading interface, not a second source of truth
(SQLite remains authoritative). If no readable journal exists, the agent
says: "No PWT assessment is available for this project. Open /pwt and run
its first scan in the dashboard." — and scans nothing itself.

The agent explains findings with LOW/MEDIUM/HIGH priorities (scanner
severity untouched) and proposes targeted YAML edits, waiting for
approval. After you approve an edit, the agent states the previous scan
describes the pre-edit files and asks you to run a new dashboard scan;
only then does it read the new journal section and report before/after.
The agent never calls `start_scan`/`cancel_scan` and never runs scanner
commands. `save_analysis` stays available for explicitly requested,
authorized analysis records only (needs `--allow-analysis-writes`);
nothing is auto-saved after a conversational answer. No commits, no
pushes, no network operations, no shell scans.

## 5. Dashboard

```sh
make start   # or: integrations/pwt-dashboard.sh --jar <jar> --data-dir <dir>
```

The GUI launches independently and does not depend on OpenCode. Local
display only; single instance; shows shared runs plus saved agent
analysis (read-only view, MCP writes only). Bare `/pwt` also opens the
same dashboard helper (see §3). To assess a project, pick its directory
with "Open directory…", select files, and press scan: the first scan
registers the project automatically and writes its `.pwt/context.md`
pointer, and every terminal scan appends to its journal. Browsing a
directory never scans anything by itself.

Pending owner decision: a requested design had bare `/pwt` show help
instead of opening the dashboard. Code is unchanged; `/pwt help`
explicitly displays help in the meantime.

## 6. Shared data directory

GUI, CLI, MCP, commands, and agent use one SQLite database at
`<application-root>/.pwt-data/pwt.db` unless every entrypoint is
explicitly pointed elsewhere: GUI via `gui --data-dir DIR`, CLI via
`scan --data-dir DIR`, MCP servers via their configured `--data-dir`
(`make setup-opencode` writes the application default). Reports render on
demand from persisted records; files appear only after explicit export.
Per-repository scan journals live at `<data-dir>/reports/`; the agent
reads them as ordinary files (ranged reads), which needs no extra
permission because OpenCode allows `external_directory` reads by default.

## 7. Permissions (what is and is not configured)

No OpenCode permission rules are installed: the defaults already allow
reading files (including the journal outside the project), running the
`pwt` MCP tools, and using the normal edit/file tools. Deliberately
*not* granted: unrestricted shell, arbitrary filesystem writes, arbitrary
SQL, auto commit/push, global allow-all rules, or edit approval bypasses.
Source edits still need your per-action approval in the client; the agent
instructions additionally require it to wait for explicit approval, but
that is a prompt-level rule, not an enforced filesystem restriction —
do not treat it as a sandbox. Real enforcement lives server-side:
`start_scan`/`cancel_scan` require `--allow-scans` and `save_analysis`
requires `--allow-analysis-writes`, and PWT-owned OpenCode configuration
ships neither scan flag (read-only); unknown or out-of-scope repositories
are rejected per call. Analysis writes stay strictly opt-in per project.

## 8. Removal and deferred scope

Project: delete `.opencode/plugins/pwt`, `.opencode/pwt-conversation.md`,
and the `pwt` entry from `opencode.json`. Global: `make uninstall`
(removes launcher, desktop entry, plugin, agent, and ask command; keeps
data, projects, and source).

Claude Code has its own integration since 1.1.0 (`/pwt`, aliases,
`/pwt-ask` with a read-only subagent, project-root-bound MCP; see
`../claude-code/README.md`). The native OpenCode plugin does not apply
there, and nothing in this guide changes for it.

After upgrading PWT, restart OpenCode (`opencode service restart`): a
running session keeps the plugin it loaded at start (for example the 1.1.0
`/pwt` stderr fix is not active until a restart).

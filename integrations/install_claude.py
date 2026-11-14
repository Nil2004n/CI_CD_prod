#!/usr/bin/env python3
"""Claude Code integration for PWT: user-level artifacts and status.

Usage:
  install_claude.py install   [--claude-dir DIR] [--data-dir DIR]
  install_claude.py global    [--data-dir DIR] [--allow-analysis 0|1] [--allow-scans 0]
  install_claude.py uninstall [--claude-dir DIR] [--project-dir DIR]
  install_claude.py status    [--claude-dir DIR] [--data-dir DIR] [--project-dir DIR] [--test]

install    Writes the user-wide /pwt (+ dashboard/help/repos/status/report
           aliases) and /pwt-ask commands and the `pwt`
           subagent into <claude-dir> (default: $CLAUDE_CONFIG_DIR or
           ~/.claude), with this checkout's absolute paths baked in. Files
           carry the PWT-CLAUDE-MANAGED marker; an existing file without it is
           never overwritten. Idempotent. Writes no MCP entry: project MCP
           connections come from `make setup-claude ROOT=...` (project-scoped
           .mcp.json bound with --project-root, never --allow-scans).
global     Adds the user-wide (user scope) `pwt` MCP entry through the supported
           `claude mcp add-json -s user` CLI, like OpenCode's global entry: it
           applies in every directory, and a project's own .mcp.json `pwt` entry
           overrides it. It is never unscoped: it runs the launcher with
           --project-root-cwd, so each session is bound to the project owning
           the directory Claude Code was started in (unregistered directory:
           PROJECT_NOT_REGISTERED, never another project). Never --allow-scans;
           --allow-analysis 1 adds --allow-analysis-writes (default denied,
           omitted preserves). Refuses to replace a user `pwt` entry PWT did not
           create. Idempotent.
uninstall  Removes only PWT-managed user files and the PWT-created user-scope
           `pwt` MCP entry; with --project-dir also removes
           that project's PWT-owned `pwt` entry from .mcp.json. Never touches
           the shared data directory, the launcher, or OpenCode files.
status     Reports artifacts, project registration (read-only database
           query), project MCP configuration and effective server-side
           permissions. --test runs a real stdio MCP handshake with the
           configured command (initialize, tools/list, get_project_status and
           a start_scan call that must be denied). Without --test the
           connection is reported as NOT TESTED.

No shell startup files, client trust state or permission settings are
modified. Python 3 standard library only.
"""
import argparse
import json
import os
import select
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
APP_ROOT = HERE.parent
DEFAULT_DATA_DIR = APP_ROOT / ".pwt-data"
MARKER = "PWT-CLAUDE-MANAGED"
COMMANDS = ("pwt", "pwt-dashboard", "pwt-help", "pwt-repos", "pwt-status", "pwt-report", "pwt-ask")
SOURCES = {
    **{Path("commands") / f"{name}.md": HERE / "claude-code" / "commands" / f"{name}.md" for name in COMMANDS},
    Path("agents") / "pwt.md": HERE / "claude-code" / "agents" / "pwt.md",
}
EXPECTED_READ_TOOLS = ["get_project_status", "list_runs", "get_run", "list_findings",
                       "compare_runs", "get_report", "get_analysis"]


def claude_dir(arg):
    if arg:
        return Path(arg)
    env = os.environ.get("CLAUDE_CONFIG_DIR")
    if env:
        return Path(env)
    return Path.home() / ".claude"


def sh_quote(value):
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$").replace("`", "\\`") + '"'


def dashboard_command(data_dir):
    return "{} --detach --data-dir {}".format(
        sh_quote(str(HERE / "pwt-dashboard.sh")), sh_quote(str(data_dir)))


def render(source, data_dir):
    text = source.read_text(encoding="utf-8")
    return text.replace("@@DASHBOARD_CMD@@", dashboard_command(data_dir))


def is_managed(path):
    try:
        return MARKER in path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return False


def cmd_install(args):
    base = claude_dir(args.claude_dir)
    data_dir = Path(args.data_dir)
    if not data_dir.is_absolute():
        print(f"install-claude: --data-dir must be absolute: {data_dir}", file=sys.stderr)
        return 2
    blocked = []
    for rel in SOURCES:
        target = base / rel
        if target.exists() and not is_managed(target):
            blocked.append(target)
    if blocked:
        for target in blocked:
            print(f"install-claude: refusing to overwrite unmanaged file: {target}", file=sys.stderr)
        print("install-claude: move or rename it, then rerun; nothing was written", file=sys.stderr)
        return 2
    for rel, source in SOURCES.items():
        target = base / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        content = render(source, data_dir)
        current = target.read_text(encoding="utf-8") if target.exists() else None
        if current == content:
            print(f"install-claude: up to date: {target}")
            continue
        tmp = target.with_suffix(target.suffix + ".pwt-tmp")
        tmp.write_text(content, encoding="utf-8")
        os.replace(tmp, target)
        print(f"install-claude: {'updated' if current is not None else 'installed'}: {target}")
    print(f"install-claude: dashboard data directory: {data_dir}")
    print("install-claude: next, per project: make setup-claude ROOT=/abs/project")
    return 0


def remove_project_entry(project):
    config_path = project / ".mcp.json"
    try:
        config = json.loads(config_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        print(f"uninstall-claude: no {config_path}")
        return 0
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"uninstall-claude: refusing to modify {config_path}: not parseable JSON ({exc})", file=sys.stderr)
        return 2
    servers = config.get("mcpServers") if isinstance(config, dict) else None
    entry = servers.get("pwt") if isinstance(servers, dict) else None
    if not isinstance(entry, dict):
        print(f"uninstall-claude: no pwt entry in {config_path}")
        return 0
    if not str(entry.get("command", "")).endswith("pwt-mcp-stdio.sh"):
        print(f"uninstall-claude: leaving {config_path} pwt entry alone (not created by PWT setup)")
        return 0
    del servers["pwt"]
    config_path.write_text(json.dumps(config, indent=4) + "\n", encoding="utf-8")
    print(f"uninstall-claude: removed pwt entry from {config_path} (other servers preserved)")
    return 0


def claude_json_path(base):
    """Claude Code's user config file (holds user-scope MCP servers); read-only use."""
    if os.environ.get("CLAUDE_CONFIG_DIR"):
        return Path(os.environ["CLAUDE_CONFIG_DIR"]) / ".claude.json"
    return Path.home() / ".claude.json"


def user_entry(base):
    try:
        data = json.loads(claude_json_path(base).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    servers = data.get("mcpServers") if isinstance(data, dict) else None
    entry = servers.get("pwt") if isinstance(servers, dict) else None
    return entry if isinstance(entry, dict) else None


def is_pwt_entry(entry):
    return isinstance(entry, dict) and str(entry.get("command", "")).endswith("pwt-mcp-stdio.sh")


def claude_cli():
    from shutil import which
    return which("claude")


def run_claude(args):
    return subprocess.run([claude_cli()] + args, capture_output=True, text=True, timeout=120)


def cmd_global(args):
    if args.allow_scans == "1":
        print("setup-claude-global: refusing ALLOW_SCANS=1: Claude Code connections are dashboard-only",
              file=sys.stderr)
        return 2
    if claude_cli() is None:
        print("setup-claude-global: the `claude` CLI was not found on PATH; install Claude Code first",
              file=sys.stderr)
        return 2
    data_dir = Path(args.data_dir)
    if not data_dir.is_absolute():
        print(f"setup-claude-global: --data-dir must be absolute: {data_dir}", file=sys.stderr)
        return 2
    base = claude_dir(args.claude_dir)
    existing = user_entry(base)
    if existing is not None and not is_pwt_entry(existing):
        print("setup-claude-global: refusing to replace a user-scope `pwt` MCP server that PWT setup did "
              "not create; remove or rename it first", file=sys.stderr)
        return 2
    existing_args = existing.get("args", []) if existing and isinstance(existing.get("args"), list) else []
    command_args = ["--data-dir", str(data_dir), "--project-root-cwd"]
    if args.allow_analysis == "1" or (args.allow_analysis == "" and "--allow-analysis-writes" in existing_args):
        command_args.append("--allow-analysis-writes")
    desired = {"type": "stdio", "command": str(HERE / "pwt-mcp-stdio.sh"), "args": command_args, "env": {}}
    current = None if existing is None else {"type": existing.get("type", "stdio"),
                                             "command": existing.get("command"),
                                             "args": existing_args, "env": existing.get("env", {}) or {}}
    if current == desired:
        print(f"setup-claude-global: user-scope pwt entry already current ({claude_json_path(base)})")
    else:
        if existing is not None:
            removed = run_claude(["mcp", "remove", "pwt", "-s", "user"])
            if removed.returncode != 0:
                print(f"setup-claude-global: could not replace the old entry: {removed.stderr.strip()}",
                      file=sys.stderr)
                return 2
        added = run_claude(["mcp", "add-json", "-s", "user", "pwt", json.dumps(desired)])
        if added.returncode != 0:
            print(f"setup-claude-global: claude mcp add-json failed: {(added.stderr or added.stdout).strip()}",
                  file=sys.stderr)
            return 2
        print(f"setup-claude-global: {'updated' if existing else 'added'} user-scope pwt entry "
              f"({claude_json_path(base)})")
    if "--allow-scans" in existing_args:
        print("setup-claude-global: removed --allow-scans from the previous entry (dashboard-only)")
    analysis = "enabled" if "--allow-analysis-writes" in command_args else "denied"
    print(f"setup-claude-global: command: {desired['command']} {' '.join(command_args)}")
    print("setup-claude-global: each session binds to the project owning the directory Claude Code starts in;"
          " a project's own .mcp.json pwt entry overrides this one")
    print(f"setup-claude-global: server permissions: scans denied (dashboard-only), analysis writes {analysis}")
    print("setup-claude-global: nothing was registered or scanned")
    return 0


def remove_user_entry(base):
    existing = user_entry(base)
    if existing is None:
        return 0
    if not is_pwt_entry(existing):
        print("uninstall-claude: kept user-scope `pwt` MCP server (not created by PWT setup)")
        return 0
    if claude_cli() is None:
        print("uninstall-claude: user-scope pwt entry present but the `claude` CLI is missing; remove it with "
              "`claude mcp remove pwt -s user`", file=sys.stderr)
        return 1
    removed = run_claude(["mcp", "remove", "pwt", "-s", "user"])
    if removed.returncode != 0:
        print(f"uninstall-claude: could not remove the user-scope pwt entry: {removed.stderr.strip()}",
              file=sys.stderr)
        return 1
    print("uninstall-claude: removed user-scope pwt MCP entry")
    return 0


def cmd_uninstall(args):
    base = claude_dir(args.claude_dir)
    for rel in SOURCES:
        target = base / rel
        if not target.exists():
            continue
        if is_managed(target):
            target.unlink()
            print(f"uninstall-claude: removed {target}")
        else:
            print(f"uninstall-claude: kept unmanaged file {target}")
    rc = remove_user_entry(base)
    if args.project_dir:
        rc = remove_project_entry(Path(args.project_dir)) or rc
    print("uninstall-claude: shared data directory, launcher and OpenCode files untouched")
    return rc


def registered_project(data_dir, project):
    """Read-only lookup mirroring the server's ProjectScope rule (deepest containing root)."""
    db = Path(data_dir) / "pwt.db"
    if not db.is_file():
        return None, "no shared database yet"
    try:
        here = project.resolve(strict=True)
    except OSError:
        return None, "project directory missing"
    try:
        con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
        try:
            rows = con.execute("SELECT id, display_name, canonical_root FROM repositories").fetchall()
        finally:
            con.close()
    except sqlite3.Error as exc:
        return None, f"cannot read database: {exc}"
    best = None
    for repo_id, name, root in rows:
        try:
            root_path = Path(root).resolve()
        except OSError:
            root_path = Path(root)
        if here == root_path or root_path in here.parents:
            if best is None or len(root_path.parts) > len(best[2].parts):
                best = (repo_id, name, root_path)
    if best is None:
        return None, "not registered (the first dashboard scan registers it)"
    return best, None


def mcp_entry(project):
    """The `pwt` entry of the nearest .mcp.json at or above the project directory."""
    try:
        start = project.resolve(strict=True)
    except OSError:
        return None
    config = None
    for directory in [start, *start.parents]:
        candidate = directory / ".mcp.json"
        if candidate.is_file():
            try:
                config = json.loads(candidate.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                return None
            break
    if config is None:
        return None
    servers = config.get("mcpServers") if isinstance(config, dict) else None
    entry = servers.get("pwt") if isinstance(servers, dict) else None
    return entry if isinstance(entry, dict) else None


class StdioClient:
    def __init__(self, command, args, cwd=None):
        self.proc = subprocess.Popen([command] + list(args), stdin=subprocess.PIPE, cwd=cwd,
                                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.next_id = 1
        self.buffer = b""

    def request(self, method, params, timeout=60):
        rid = self.next_id
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        deadline = time.time() + timeout
        while time.time() < deadline:
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                if not line.strip():
                    continue
                message = json.loads(line)  # non-JSON stdout = protocol pollution -> raises
                if message.get("jsonrpc") != "2.0":
                    raise RuntimeError("stdout line is not JSON-RPC 2.0")
                if message.get("id") == rid:
                    return message
            ready, _, _ = select.select([self.proc.stdout], [], [], 0.5)
            if ready:
                chunk = os.read(self.proc.stdout.fileno(), 65536)
                if not chunk:
                    raise RuntimeError("server closed stdout")
                self.buffer += chunk
        raise TimeoutError(f"no response to {method} within {timeout}s")

    def send(self, message):
        self.proc.stdin.write((json.dumps(message) + "\n").encode("utf-8"))
        self.proc.stdin.flush()

    def call(self, name, arguments):
        response = self.request("tools/call", {"name": name, "arguments": arguments})
        result = response.get("result", {})
        text = result.get("content", [{}])[0].get("text", "{}")
        return bool(result.get("isError")), json.loads(text)

    def close(self):
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=10)
        self.proc.stdout.close()


def connection_test(entry, cwd=None):
    client = StdioClient(entry["command"], entry.get("args", []), cwd=cwd)
    try:
        init = client.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                              "clientInfo": {"name": "pwt-claude-status", "version": "1"}})
        if "result" not in init:
            return False, ["initialize failed"]
        client.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        tools = [t["name"] for t in client.request("tools/list", {})["result"]["tools"]]
        lines = [f"tools listed: {len(tools)}"]
        missing = [t for t in EXPECTED_READ_TOOLS if t not in tools]
        if missing:
            lines.append("missing read tools: " + ", ".join(missing))
        is_error, status = client.call("get_project_status", {})
        if is_error:
            lines.append(f"get_project_status: {status.get('code')}: {status.get('message')}")
            return status.get("code") == "PROJECT_NOT_REGISTERED" and not missing, lines
        lines.append(f"get_project_status: bound to {status['project']['display_name']} "
                     f"({status['project']['id']}), runs {status['run_count']}, "
                     f"journal {'available' if status['journal']['available'] else 'missing'}")
        perms = status.get("server_permissions", {})
        lines.append(f"server permissions: scans {perms.get('scans')}, analysis writes {perms.get('analysis_writes')}")
        denied, payload = client.call("start_scan", {"repository_id": status["project"]["id"],
                                                     "relative_paths": ["pwt-status-probe.yaml"]})
        scan_denied = denied and payload.get("code") == "PERMISSION_DENIED"
        lines.append("start_scan: " + ("PERMISSION_DENIED (as required)" if scan_denied
                                       else f"NOT DENIED: {payload}"))
        return (not missing) and scan_denied, lines
    finally:
        client.close()


def cmd_status(args):
    base = claude_dir(args.claude_dir)
    data_dir = Path(args.data_dir)
    print(f"Claude config dir: {base}")
    print("Artifacts:")
    for rel, source in SOURCES.items():
        target = base / rel
        if not target.exists():
            state = "MISSING"
        elif not is_managed(target):
            state = "PRESENT, NOT PWT-MANAGED"
        elif target.read_text(encoding="utf-8") == render(source, data_dir):
            state = "installed, current"
        else:
            state = "installed, outdated (rerun make install-claude)"
        print(f"  {rel}: {state}")
    glob_entry = user_entry(base)
    if glob_entry is None:
        print("Global (user-scope) MCP: not configured (make setup-claude-global)")
    elif not is_pwt_entry(glob_entry):
        print("Global (user-scope) MCP: a `pwt` entry exists that PWT did not create")
    else:
        gargs = glob_entry.get("args", [])
        print(f"Global (user-scope) MCP: configured, {'bound per session to the start directory' if '--project-root-cwd' in gargs else 'NOT BOUND (rerun make setup-claude-global)'};"
              f" scans {'ENABLED (not allowed)' if '--allow-scans' in gargs else 'denied'},"
              f" analysis writes {'enabled' if '--allow-analysis-writes' in gargs else 'denied'}")
    if not args.project_dir:
        print("Project: not checked (pass ROOT=/abs/project)")
        print("Connection: NOT TESTED")
        return 0
    project = Path(args.project_dir)
    print(f"Project: {project}")
    repo, why = registered_project(data_dir, project)
    if repo:
        print(f"  registered: yes — {repo[1]} ({repo[0]}) root {repo[2]}")
    else:
        print(f"  registered: no — {why}")
    entry = mcp_entry(project)
    test_cwd = None
    if entry is None and is_pwt_entry(glob_entry):
        entry = glob_entry
        test_cwd = str(project)
        print("  project MCP: none in this project; the global (user-scope) entry applies here")
    elif entry is None:
        print("  project MCP: NOT CONFIGURED (run make setup-claude ROOT=... or make setup-claude-global)")
        print("Connection: NOT TESTED")
        return 0
    else:
        print(f"  project MCP: configured ({entry.get('command')})")
    margs = entry.get("args", []) if isinstance(entry.get("args"), list) else []
    bound = margs[margs.index("--project-root") + 1] if "--project-root" in margs[:-1] else None
    if bound is None and "--project-root-cwd" in margs:
        bound = f"{project} (start directory)"
    print(f"  bound project root: {bound or 'NOT BOUND (unsafe: rerun make setup-claude)'}")
    print(f"  configured permissions: scans {'ENABLED (not allowed for Claude)' if '--allow-scans' in margs else 'denied'}, "
          f"analysis writes {'enabled' if '--allow-analysis-writes' in margs else 'denied'}")
    if not args.test:
        print("Connection: NOT TESTED (rerun with TEST=1 to run a real MCP handshake)")
        return 0
    try:
        ok, lines = connection_test(entry, cwd=test_cwd)
    except Exception as exc:  # report, never hide
        print(f"Connection: TEST FAILED — {exc}")
        return 1
    for line in lines:
        print(f"  {line}")
    print(f"Connection: {'TESTED OK' if ok else 'TEST FAILED'}")
    return 0 if ok else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="action", required=True)
    p_install = sub.add_parser("install")
    p_install.add_argument("--claude-dir")
    p_install.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    p_global = sub.add_parser("global")
    p_global.add_argument("--claude-dir")
    p_global.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    p_global.add_argument("--allow-analysis", default="", choices=["", "0", "1"])
    p_global.add_argument("--allow-scans", default="", choices=["", "0", "1"])
    p_uninstall = sub.add_parser("uninstall")
    p_uninstall.add_argument("--claude-dir")
    p_uninstall.add_argument("--project-dir")
    p_status = sub.add_parser("status")
    p_status.add_argument("--claude-dir")
    p_status.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    p_status.add_argument("--project-dir")
    p_status.add_argument("--test", action="store_true")
    args = parser.parse_args()
    return {"install": cmd_install, "global": cmd_global, "uninstall": cmd_uninstall,
            "status": cmd_status}[args.action](args)


if __name__ == "__main__":
    sys.exit(main())

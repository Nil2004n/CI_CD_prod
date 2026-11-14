#!/usr/bin/env python3
"""Configure one project's Claude Code MCP connection to PWT.

Usage: setup_claude.py --project-dir DIR [--data-dir DIR] [--allow-analysis 0|1]
                       [--allow-scans 0] [--launcher PATH]

- Merges a project-scoped `pwt` server into <project>/.mcp.json, preserving
  every unrelated server. The entry runs the shared launcher with
  `--data-dir <shared dir> --project-root <canonical project dir>`: the server
  binds every call to the registered project owning that directory and fails
  closed while it is unregistered (the first dashboard scan registers it).
- Scanning is dashboard-only for Claude Code: --allow-scans is never written;
  an existing entry that had it is downgraded. ALLOW_SCANS=1 is refused.
- --allow-analysis 1 adds --allow-analysis-writes, 0 removes it, omitted
  preserves the existing entry (fresh entries: denied).
- Removes a stale project-local `/pwt` command only when it is PWT's own legacy
  template (it allowed scans and would shadow the user-wide command).
- Never registers the project, never scans, never touches global Claude Code
  configuration, client trust state, or permission settings.
"""
import argparse
import json
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
APP_ROOT = HERE.parent
DEFAULT_DATA_DIR = APP_ROOT / ".pwt-data"
LEGACY_COMMAND_HEADER = ("description: PWT YAML assessment workbench — scan, findings, reports, "
                         "dashboard (agent-interpreted")


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"setup-claude: refusing to modify {path}: not parseable JSON ({exc})", file=sys.stderr)
        sys.exit(2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-dir", required=True)
    parser.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    parser.add_argument("--allow-scans", default="", choices=["", "0", "1"])
    parser.add_argument("--allow-analysis", default="", choices=["", "0", "1"])
    parser.add_argument("--launcher", default=str(HERE / "pwt-mcp-stdio.sh"))
    args = parser.parse_args()

    if args.allow_scans == "1":
        print("setup-claude: refusing ALLOW_SCANS=1: Claude Code connections are dashboard-only "
              "(scans start and cancel only in the dashboard)", file=sys.stderr)
        return 2
    project = Path(args.project_dir)
    if not project.is_dir():
        print(f"setup-claude: project dir missing: {project}", file=sys.stderr)
        return 2
    canonical = project.resolve(strict=True)
    launcher = Path(args.launcher)
    if not launcher.is_file():
        print(f"setup-claude: launcher missing: {launcher}", file=sys.stderr)
        return 2
    data_dir = Path(args.data_dir)
    if not data_dir.is_absolute():
        print(f"setup-claude: --data-dir must be absolute: {args.data_dir}", file=sys.stderr)
        return 2

    config_path = canonical / ".mcp.json"
    config = load_json(config_path)
    if not isinstance(config, dict):
        print(f"setup-claude: refusing to modify {config_path}: top level is not an object", file=sys.stderr)
        return 2
    servers = config.setdefault("mcpServers", {})
    if not isinstance(servers, dict):
        print(f"setup-claude: refusing to modify {config_path}: mcpServers is not an object", file=sys.stderr)
        return 2
    entry = servers.get("pwt")
    if not isinstance(entry, dict):
        entry = {}
    existing = entry.get("args", []) if isinstance(entry.get("args"), list) else []
    if entry and not str(entry.get("command", "")).endswith("pwt-mcp-stdio.sh"):
        print(f"setup-claude: refusing to replace a `pwt` server in {config_path} that PWT setup did not "
              "create; rename or remove it first", file=sys.stderr)
        return 2
    downgraded = "--allow-scans" in existing
    command_args = ["--data-dir", str(data_dir), "--project-root", str(canonical)]
    if args.allow_analysis == "1" or (args.allow_analysis == "" and "--allow-analysis-writes" in existing):
        command_args.append("--allow-analysis-writes")
    new_entry = {"type": "stdio", "command": str(launcher), "args": command_args,
                 "env": entry.get("env", {}) if isinstance(entry.get("env"), dict) else {}}
    if entry == new_entry:
        print(f"setup-claude: {config_path} pwt entry already current")
    else:
        servers["pwt"] = new_entry
        tmp = config_path.with_name(".mcp.json.pwt-tmp")
        tmp.write_text(json.dumps(config, indent=4) + "\n", encoding="utf-8")
        os.replace(tmp, config_path)
        print(f"setup-claude: wrote pwt entry to {config_path} (other servers preserved)")
    if downgraded:
        print("setup-claude: removed --allow-scans from the existing entry (Claude Code is dashboard-only)")

    legacy = canonical / ".claude" / "commands" / "pwt.md"
    if legacy.is_file():
        try:
            text = legacy.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            text = ""
        if LEGACY_COMMAND_HEADER in text:
            legacy.unlink()
            print(f"setup-claude: removed legacy scan-capable project command {legacy}")
        else:
            print(f"setup-claude: note: {legacy} exists and is not PWT's; it shadows the user-wide /pwt command")

    analysis = "enabled (--allow-analysis-writes)" if "--allow-analysis-writes" in command_args else "denied"
    print(f"setup-claude: bound project root: {canonical}")
    print(f"setup-claude: data directory: {data_dir}")
    print(f"setup-claude: server permissions: scans denied (dashboard-only), analysis writes {analysis}")
    print("setup-claude: nothing was registered or scanned; Claude Code asks you to approve this project's "
          ".mcp.json server on first use")
    return 0


if __name__ == "__main__":
    sys.exit(main())

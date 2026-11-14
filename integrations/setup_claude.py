#!/usr/bin/env python3
"""Install/merge project-local Claude Code PWT configuration.

Usage: setup_claude.py --project-dir DIR [--data-dir DIR] [--allow-scans 0|1]
                       [--allow-analysis 0|1]  (data dir defaults to <application-root>/.pwt-data;
must be absolute if given)
                       [--launcher PATH]

- Merges the `pwt` MCP server entry into <project>/.mcp.json (project scope),
  preserving every unrelated server.
- ALLOW_SCANS=1 adds --allow-scans; 0 removes it; omitted preserves the
  existing entry's mode (fresh entries default to read-only).
- Copies the /pwt slash command and conversation instructions (safe to rerun).
- Prints the effective mode and data directory.
- Never touches global Claude Code configuration.
"""
import argparse
import json
import os
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
APP_ROOT = HERE.parent
DEFAULT_DATA_DIR = APP_ROOT / ".pwt-data"


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"setup-claude: refusing to modify {path}: not parseable JSON ({exc})",
              file=sys.stderr)
        sys.exit(2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-dir", required=True)
    parser.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR))
    parser.add_argument("--allow-scans", default="", choices=["", "0", "1"])
    parser.add_argument("--allow-analysis", default="", choices=["", "0", "1"])
    parser.add_argument("--launcher", default=str(HERE / "pwt-mcp-stdio.sh"))
    parser.add_argument("--command-src", default=str(HERE / "claude-code" / "commands" / "pwt.md"))
    parser.add_argument("--instructions-src", default=str(HERE / "pwt-conversation.md"))
    args = parser.parse_args()

    project = Path(args.project_dir)
    if not project.is_dir():
        print(f"setup-claude: project dir missing: {project}", file=sys.stderr)
        return 2
    launcher = Path(args.launcher)
    if not launcher.is_file():
        print(f"setup-claude: launcher missing: {launcher}", file=sys.stderr)
        return 2
    data_dir = Path(args.data_dir)
    if not data_dir.is_absolute():
        print(f"setup-claude: --data-dir must be absolute: {args.data_dir}", file=sys.stderr)
        return 2
    try:
        data_dir.mkdir(parents=True, exist_ok=True)
        try:
            os.chmod(data_dir, 0o700)
        except OSError:
            pass  # non-POSIX filesystem: umask applies
    except OSError as exc:
        print(f"setup-claude: cannot create data dir {data_dir}: {exc}", file=sys.stderr)
        return 2

    config_path = project / ".mcp.json"
    config = load_json(config_path)
    if not isinstance(config, dict):
        print(f"setup-claude: refusing to modify {config_path}: top level is not an object",
              file=sys.stderr)
        return 2
    servers = config.setdefault("mcpServers", {})
    entry = servers.get("pwt")
    if not isinstance(entry, dict):
        entry = {}
    command_args = ["--data-dir", str(data_dir)]
    existing = entry.get("args", [])
    for flag, value in (("--allow-scans", args.allow_scans),
                        ("--allow-analysis-writes", args.allow_analysis)):
        if value == "1":
            command_args.append(flag)
        elif value == "0":
            pass
        elif isinstance(existing, list) and flag in existing:
            command_args.append(flag)  # preserve existing mode
    entry["type"] = "stdio"
    entry["command"] = str(launcher)
    entry["args"] = command_args
    entry["env"] = entry.get("env", {}) if isinstance(entry.get("env"), dict) else {}
    servers["pwt"] = entry
    config_path.write_text(json.dumps(config, indent=4) + "\n", encoding="utf-8")

    commands_dir = project / ".claude" / "commands"
    commands_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.command_src, commands_dir / "pwt.md")
    shutil.copyfile(args.instructions_src, project / ".claude" / "pwt-conversation.md")

    mode = "scan-enabled (--allow-scans)" if "--allow-scans" in command_args else "read-only"
    if "--allow-analysis-writes" in command_args:
        mode += " + analysis writes (--allow-analysis-writes)"
    print(f"setup-claude: pwt MCP server configured ({mode})")
    print(f"setup-claude: data directory: {data_dir}")
    print(f"setup-claude: command installed: {commands_dir / 'pwt.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

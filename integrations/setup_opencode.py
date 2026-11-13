#!/usr/bin/env python3
"""Install/merge project-local OpenCode PWT configuration.

Usage: setup_opencode.py --project-dir DIR --data-dir DIR [--allow-scans 0|1]
                         [--launcher PATH]

- Merges the `pwt` MCP server entry into <project>/opencode.json, preserving
  every unrelated key/server.
- ALLOW_SCANS=1 adds --allow-scans; 0 removes it; omitted preserves the
  existing entry's mode (fresh entries default to read-only).
- Copies the /pwt command and conversation instructions (safe to rerun).
- Prints the effective mode and data directory.
- Never touches global OpenCode configuration.
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        print(f"setup-opencode: refusing to modify {path}: not parseable JSON ({exc})",
              file=sys.stderr)
        print("setup-opencode: back it up, or remove it, then rerun", file=sys.stderr)
        sys.exit(2)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-dir", required=True)
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--allow-scans", default="", choices=["", "0", "1"])
    parser.add_argument("--launcher", default=str(HERE / "pwt-mcp-stdio.sh"))
    parser.add_argument("--command-src", default=str(HERE / "opencode" / "commands" / "pwt.md"))
    parser.add_argument("--instructions-src", default=str(HERE / "pwt-conversation.md"))
    args = parser.parse_args()

    project = Path(args.project_dir)
    if not project.is_dir():
        print(f"setup-opencode: project dir missing: {project}", file=sys.stderr)
        return 2
    launcher = Path(args.launcher)
    if not launcher.is_file():
        print(f"setup-opencode: launcher missing: {launcher}", file=sys.stderr)
        return 2
    data_dir = Path(args.data_dir)
    try:
        data_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        print(f"setup-opencode: cannot create data dir {data_dir}: {exc}", file=sys.stderr)
        return 2

    config_path = project / "opencode.json"
    config = load_json(config_path)
    if not isinstance(config, dict):
        print(f"setup-opencode: refusing to modify {config_path}: top level is not an object",
              file=sys.stderr)
        return 2
    servers = config.setdefault("mcp", {}).setdefault("servers", {})
    entry = servers.get("pwt")
    if not isinstance(entry, dict):
        entry = {}
    command = [str(launcher), "--data-dir", str(data_dir)]
    if args.allow_scans == "1":
        command.append("--allow-scans")
    elif args.allow_scans == "0":
        pass  # read-only: no flag
    else:
        # Preserve: keep an existing --allow-scans if the user already had it.
        existing = entry.get("command", [])
        if isinstance(existing, list) and "--allow-scans" in existing:
            command.append("--allow-scans")
    entry["type"] = "local"
    entry["command"] = command
    servers["pwt"] = entry
    config_path.write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")

    commands_dir = project / ".opencode" / "commands"
    commands_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.command_src, commands_dir / "pwt.md")
    shutil.copyfile(args.instructions_src, project / ".opencode" / "pwt-conversation.md")

    mode = "scan-enabled (--allow-scans)" if "--allow-scans" in command else "read-only"
    print(f"setup-opencode: pwt MCP server configured ({mode})")
    print(f"setup-opencode: data directory: {data_dir}")
    print(f"setup-opencode: command installed: {commands_dir / 'pwt.md'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Install or remove the global PWT OpenCode integration.

Usage:
  install-opencode.py [--uninstall] [--home DIR] [--source DIR]

Installs (default):
  - the native PWT command plugin into ~/.config/opencode/plugins/pwt/
  - the global PWT subagent definition into ~/.config/opencode/agents/pwt.md
  - the global /pwt-ask conversation command into ~/.config/opencode/commands/pwt-ask.md

Refuses to overwrite an existing pwt plugin/agent/command that it did not install
(checked via marker content) unless --force is given. Never touches MCP
servers, models, providers, or unrelated configuration. Removal deletes only
files this installer created (verified by the same markers).
"""
import argparse
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PLUGIN_MARKER = '"name": "pwt-opencode-plugin"'
AGENT_MARKER = "PWT-AGENT-MANAGED"
ASK_MARKER = "PWT-ASK-MANAGED"


def plugin_dir(home):
    return Path(home) / ".config" / "opencode" / "plugins" / "pwt"


def agent_file(home):
    return Path(home) / ".config" / "opencode" / "agents" / "pwt.md"


def ask_file(home):
    return Path(home) / ".config" / "opencode" / "commands" / "pwt-ask.md"


def is_managed_plugin(path):
    package = path / "package.json"
    return package.is_file() and PLUGIN_MARKER in package.read_text(encoding="utf-8")


def is_managed_agent(path):
    return path.is_file() and AGENT_MARKER in path.read_text(encoding="utf-8")


def is_managed_ask(path):
    return path.is_file() and ASK_MARKER in path.read_text(encoding="utf-8")


def install(home, source, force):
    plugin_src = source / "opencode-plugin"
    if not (plugin_src / "package.json").is_file():
        print(f"install-opencode: plugin source missing: {plugin_src}", file=sys.stderr)
        return 2
    dest = plugin_dir(home)
    if dest.exists() and not (force or is_managed_plugin(dest)):
        print(f"install-opencode: refusing to overwrite unmanaged {dest} (use --force)",
              file=sys.stderr)
        return 2
    agent = agent_file(home)
    if agent.exists() and not (force or is_managed_agent(agent)):
        print(f"install-opencode: refusing to overwrite unmanaged {agent} (use --force)",
              file=sys.stderr)
        return 2
    ask = ask_file(home)
    if ask.exists() and not (force or is_managed_ask(ask)):
        print(f"install-opencode: refusing to overwrite unmanaged {ask} (use --force)",
              file=sys.stderr)
        return 2
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True, exist_ok=True)
    for name in ("package.json", "tui.ts"):
        src = plugin_src / name
        if src.is_file():
            shutil.copyfile(src, dest / name)
    agent.parent.mkdir(parents=True, exist_ok=True)
    agent.write_text((source / "opencode-plugin" / "agent-pwt.md").read_text(encoding="utf-8"),
                     encoding="utf-8")
    ask.parent.mkdir(parents=True, exist_ok=True)
    ask.write_text((source / "opencode-plugin" / "pwt-ask.md").read_text(encoding="utf-8"),
                   encoding="utf-8")
    print(f"install-opencode: plugin installed: {dest}")
    print(f"install-opencode: agent installed: {agent}")
    print(f"install-opencode: ask command installed: {ask}")
    print("install-opencode: restart OpenCode (or run `opencode service restart`) to load the plugin")
    return 0


def uninstall(home):
    code = 0
    dest = plugin_dir(home)
    if dest.exists():
        if is_managed_plugin(dest):
            shutil.rmtree(dest)
            print(f"uninstall-opencode: removed: {dest}")
        else:
            print(f"uninstall-opencode: kept unmanaged directory: {dest}", file=sys.stderr)
            code = 1
    else:
        print(f"uninstall-opencode: not installed: {dest}")
    agent = agent_file(home)
    if agent.exists():
        if is_managed_agent(agent):
            agent.unlink()
            print(f"uninstall-opencode: removed: {agent}")
        else:
            print(f"uninstall-opencode: kept unmanaged file: {agent}", file=sys.stderr)
            code = 1
    else:
        print(f"uninstall-opencode: not installed: {agent}")
    ask = ask_file(home)
    if ask.exists():
        if is_managed_ask(ask):
            ask.unlink()
            print(f"uninstall-opencode: removed: {ask}")
        else:
            print(f"uninstall-opencode: kept unmanaged file: {ask}", file=sys.stderr)
            code = 1
    else:
        print(f"uninstall-opencode: not installed: {ask}")
    return code


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uninstall", action="store_true")
    parser.add_argument("--force", action="store_true",
                        help="overwrite an existing pwt plugin/agent this tool did not install")
    parser.add_argument("--home", default=str(Path.home()))
    parser.add_argument("--source", default=str(HERE))
    args = parser.parse_args()
    home = Path(args.home)
    if args.uninstall:
        return uninstall(home)
    return install(home, Path(args.source), args.force)


if __name__ == "__main__":
    sys.exit(main())

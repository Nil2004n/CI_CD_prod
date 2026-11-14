#!/usr/bin/env python3
"""Regression tests for the Claude Code integration installers (stdlib only).

Run: python3 -m unittest discover -s integrations/tests -v
Optional: PWT_TEST_CLASSPATH=<pwt/target/classes:deps> enables the tests that
register projects with the real `pwt repo-add` CLI and drive the real MCP
server (otherwise they are skipped and reported as such).
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

INTEGRATIONS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(INTEGRATIONS))
import install_claude  # noqa: E402

PY = sys.executable
CLASSPATH = os.environ.get("PWT_TEST_CLASSPATH")
JAVA = shutil.which("java")


def run(args, env=None):
    full_env = dict(os.environ)
    full_env.update(env or {})
    return subprocess.run(args, capture_output=True, text=True, env=full_env, timeout=120)


class ClaudeInstallTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pwt claude test "))  # space on purpose
        self.home = self.tmp / "home"
        self.claude = self.home / ".claude"
        self.data = self.tmp / "data dir"
        self.data.mkdir(parents=True)
        (self.data / "pwt.db").write_bytes(b"")  # sentinel: uninstall must keep data
        self.env = {"HOME": str(self.home), "CLAUDE_CONFIG_DIR": str(self.claude)}

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def install(self):
        return run([PY, str(INTEGRATIONS / "install_claude.py"), "install", "--data-dir", str(self.data)], self.env)

    def test_install_is_idempotent_and_bakes_quoted_paths(self):
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stderr)
        files = {rel: (self.claude / rel).read_text() for rel in install_claude.SOURCES}
        second = self.install()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(second.stdout.count("up to date"), len(install_claude.SOURCES))
        self.assertEqual(len(install_claude.SOURCES), 8)
        for rel, text in files.items():
            self.assertEqual((self.claude / rel).read_text(), text)
            self.assertIn(install_claude.MARKER, text)
        for name in ("pwt.md", "pwt-dashboard.md"):
            command = files[Path("commands") / name]
            self.assertIn(f'--detach --data-dir "{self.data}"', command)
        for text in files.values():
            self.assertNotIn("@@", text)
        for rel, text in files.items():
            allowed = next((l for l in text.splitlines() if l.startswith("allowed-tools:")), "")
            for forbidden in ("start_scan", "cancel_scan", "save_analysis", "Edit", "Write"):
                self.assertNotIn(forbidden, allowed, rel)
            bash = [t for t in allowed.split("Bash(")[1:]]
            for rule in bash:
                self.assertIn("pwt-dashboard.sh", rule, f"{rel}: only the dashboard helper may be pre-approved")
        agent = files[Path("agents") / "pwt.md"]
        tools_line = next(l for l in agent.splitlines() if l.startswith("tools:"))
        for forbidden in ("start_scan", "cancel_scan", "save_analysis", "Bash", "Edit", "Write"):
            self.assertNotIn(forbidden, tools_line)
        self.assertIn("mcp__pwt__compare_runs", tools_line)

    def test_unmanaged_files_are_protected(self):
        target = self.claude / "agents" / "pwt.md"
        target.parent.mkdir(parents=True)
        target.write_text("my own agent\n")
        result = self.install()
        self.assertEqual(result.returncode, 2)
        self.assertEqual(target.read_text(), "my own agent\n")
        self.assertFalse((self.claude / "commands" / "pwt.md").exists(), "nothing written when blocked")
        uninstall = run([PY, str(INTEGRATIONS / "install_claude.py"), "uninstall"], self.env)
        self.assertEqual(uninstall.returncode, 0)
        self.assertEqual(target.read_text(), "my own agent\n")

    def test_uninstall_keeps_data_and_unrelated_files(self):
        self.install()
        other = self.claude / "commands" / "other.md"
        other.write_text("unrelated\n")
        result = run([PY, str(INTEGRATIONS / "install_claude.py"), "uninstall"], self.env)
        self.assertEqual(result.returncode, 0, result.stderr)
        for rel in install_claude.SOURCES:
            self.assertFalse((self.claude / rel).exists())
        self.assertTrue(other.exists())
        self.assertTrue((self.data / "pwt.db").exists())
        self.assertTrue((INTEGRATIONS / "pwt-mcp-stdio.sh").exists())


class ClaudeProjectSetupTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pwt claude project "))
        self.project = self.tmp / "my project"
        self.project.mkdir()
        self.data = self.tmp / "data"
        self.data.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def setup_claude(self, *extra):
        return run([PY, str(INTEGRATIONS / "setup_claude.py"), "--project-dir", str(self.project),
                    "--data-dir", str(self.data), *extra])

    def config(self):
        return json.loads((self.project / ".mcp.json").read_text())

    def test_entry_is_bound_read_only_and_preserves_other_servers(self):
        (self.project / ".mcp.json").write_text(json.dumps(
            {"mcpServers": {"other": {"command": "x"}}, "keep": True}))
        result = self.setup_claude()
        self.assertEqual(result.returncode, 0, result.stderr)
        config = self.config()
        self.assertEqual(config["keep"], True)
        self.assertEqual(config["mcpServers"]["other"], {"command": "x"})
        args = config["mcpServers"]["pwt"]["args"]
        self.assertEqual(args, ["--data-dir", str(self.data), "--project-root", str(self.project.resolve())])
        self.assertNotIn("--allow-scans", args)
        again = self.setup_claude()
        self.assertIn("already current", again.stdout)
        self.assertEqual(self.config(), config)
        self.assertFalse((self.data / "pwt.db").exists(), "setup must not create or register anything")

    def test_scans_refused_and_downgraded(self):
        refused = self.setup_claude("--allow-scans", "1")
        self.assertEqual(refused.returncode, 2)
        self.assertFalse((self.project / ".mcp.json").exists())
        launcher = str(INTEGRATIONS / "pwt-mcp-stdio.sh")
        (self.project / ".mcp.json").write_text(json.dumps({"mcpServers": {"pwt": {
            "command": launcher, "args": ["--data-dir", str(self.data), "--allow-scans",
                                          "--allow-analysis-writes"]}}}))
        result = self.setup_claude()
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.config()["mcpServers"]["pwt"]["args"]
        self.assertNotIn("--allow-scans", args)
        self.assertIn("--allow-analysis-writes", args, "analysis option is preserved independently")
        self.assertIn("removed --allow-scans", result.stdout)
        denied = self.setup_claude("--allow-analysis", "0")
        self.assertNotIn("--allow-analysis-writes", self.config()["mcpServers"]["pwt"]["args"])
        self.assertEqual(denied.returncode, 0)

    def test_foreign_entry_and_legacy_command(self):
        (self.project / ".mcp.json").write_text(json.dumps({"mcpServers": {"pwt": {"command": "/opt/other"}}}))
        self.assertEqual(self.setup_claude().returncode, 2)
        self.assertEqual(self.config()["mcpServers"]["pwt"]["command"], "/opt/other")
        (self.project / ".mcp.json").unlink()
        legacy = self.project / ".claude" / "commands" / "pwt.md"
        legacy.parent.mkdir(parents=True)
        legacy.write_text((INTEGRATIONS / "tests" / "legacy_pwt_command.md").read_text())
        self.assertEqual(self.setup_claude().returncode, 0)
        self.assertFalse(legacy.exists(), "legacy scan-capable template removed")
        legacy.write_text("user command\n")
        self.setup_claude()
        self.assertEqual(legacy.read_text(), "user command\n", "user-owned command kept")

    def test_uninstall_removes_only_pwt_entry(self):
        self.setup_claude()
        config = self.config()
        config["mcpServers"]["other"] = {"command": "y"}
        (self.project / ".mcp.json").write_text(json.dumps(config))
        result = run([PY, str(INTEGRATIONS / "install_claude.py"), "uninstall", "--project-dir", str(self.project)],
                     {"CLAUDE_CONFIG_DIR": str(self.tmp / "cfg")})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.config()["mcpServers"], {"other": {"command": "y"}})


@unittest.skipUnless(shutil.which("claude"), "needs the claude CLI (real user-scope MCP configuration)")
class ClaudeGlobalEntryTest(unittest.TestCase):
    """setup-claude-global against a scratch CLAUDE_CONFIG_DIR with the real claude CLI."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pwt claude global "))
        self.cfg = self.tmp / "cfg"
        self.cfg.mkdir()
        self.data = self.tmp / "data dir"
        self.env = {"CLAUDE_CONFIG_DIR": str(self.cfg)}

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_global(self, *extra):
        return run([PY, str(INTEGRATIONS / "install_claude.py"), "global", "--data-dir", str(self.data), *extra],
                   self.env)

    def servers(self):
        return json.loads((self.cfg / ".claude.json").read_text()).get("mcpServers", {})

    def test_global_entry_is_bound_per_session_idempotent_and_removable(self):
        added = run(["claude", "mcp", "add-json", "-s", "user", "other", json.dumps({"type": "stdio", "command": "/bin/true"})],
                    self.env)
        self.assertEqual(added.returncode, 0, added.stderr)
        first = self.run_global()
        self.assertEqual(first.returncode, 0, first.stderr + first.stdout)
        entry = self.servers()["pwt"]
        self.assertTrue(entry["command"].endswith("pwt-mcp-stdio.sh"))
        self.assertEqual(entry["args"], ["--data-dir", str(self.data), "--project-root-cwd"])
        self.assertIn("other", self.servers(), "unrelated user servers preserved")
        again = self.run_global()
        self.assertIn("already current", again.stdout)
        with_analysis = self.run_global("--allow-analysis", "1")
        self.assertEqual(with_analysis.returncode, 0, with_analysis.stderr)
        self.assertIn("--allow-analysis-writes", self.servers()["pwt"]["args"])
        preserved = self.run_global()
        self.assertIn("already current", preserved.stdout, "omitted ALLOW_ANALYSIS preserves the mode")
        refused = self.run_global("--allow-scans", "1")
        self.assertEqual(refused.returncode, 2)
        self.assertNotIn("--allow-scans", self.servers()["pwt"]["args"])
        status = run([PY, str(INTEGRATIONS / "install_claude.py"), "status", "--data-dir", str(self.data)], self.env)
        self.assertIn("Global (user-scope) MCP: configured, bound per session", status.stdout)
        removed = run([PY, str(INTEGRATIONS / "install_claude.py"), "uninstall"], self.env)
        self.assertEqual(removed.returncode, 0, removed.stderr)
        self.assertNotIn("pwt", self.servers())
        self.assertIn("other", self.servers())

    def test_foreign_user_entry_is_protected(self):
        run(["claude", "mcp", "add-json", "-s", "user", "pwt", json.dumps({"type": "stdio", "command": "/opt/mine"})],
            self.env)
        result = self.run_global()
        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.servers()["pwt"]["command"], "/opt/mine")
        run([PY, str(INTEGRATIONS / "install_claude.py"), "uninstall"], self.env)
        self.assertEqual(self.servers()["pwt"]["command"], "/opt/mine", "uninstall keeps a foreign entry")


@unittest.skipUnless(CLASSPATH and JAVA, "needs PWT_TEST_CLASSPATH and java (real repo-add + MCP server)")
class ClaudeLiveServerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pwt claude live "))
        self.data = self.tmp / "data"
        self.a = self.tmp / "project a"
        self.b = self.tmp / "project-b"
        for p in (self.a / "sub", self.b):
            p.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def pwt(self, *args):
        return run([JAVA, "-cp", CLASSPATH, "pwt.launcher.PwtMain", *args])

    def entry(self, root):
        return {"command": JAVA, "args": ["-cp", CLASSPATH, "pwt.launcher.PwtMain", "mcp",
                                          "--data-dir", str(self.data), "--project-root", str(root)]}

    def test_registration_cli_unchanged_and_status_reports_binding(self):
        for root in (self.a, self.b):
            added = self.pwt("repo-add", "--root", str(root), "--data-dir", str(self.data))
            self.assertEqual(added.returncode, 0, added.stderr)
        again = self.pwt("repo-add", "--root", str(self.a), "--data-dir", str(self.data))
        self.assertEqual(again.returncode, 0, again.stderr)
        listed = self.pwt("repo-list", "--data-dir", str(self.data))
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertIn("project a", listed.stdout + listed.stderr)
        repo, why = install_claude.registered_project(self.data, self.a / "sub")
        self.assertIsNotNone(repo, why)
        self.assertEqual(repo[1], "project a")
        ok, lines = install_claude.connection_test(self.entry(self.a / "sub"))
        text = "\n".join(lines)
        self.assertTrue(ok, text)
        self.assertIn("project a", text)
        self.assertNotIn("project-b", text)
        self.assertIn("PERMISSION_DENIED", text)
        self.assertIn("scans denied", text)

    def test_global_entry_binds_to_the_start_directory(self):
        for root in (self.a, self.b):
            self.pwt("repo-add", "--root", str(root), "--data-dir", str(self.data))
        launcher = str(INTEGRATIONS / "pwt-mcp-stdio.sh")
        entry = {"command": launcher, "args": ["--data-dir", str(self.data), "--project-root-cwd"]}
        ok, lines = install_claude.connection_test(entry, cwd=str(self.a / "sub"))
        text = "\n".join(lines)
        self.assertTrue(ok, text)
        self.assertIn("bound to project a", text)
        ok_b, lines_b = install_claude.connection_test(entry, cwd=str(self.b))
        self.assertTrue(ok_b, lines_b)
        self.assertIn("bound to project-b", "\n".join(lines_b))
        ok_none, lines_none = install_claude.connection_test(entry, cwd=str(self.tmp))
        self.assertIn("PROJECT_NOT_REGISTERED", "\n".join(lines_none), "no fallback to another project")

    def test_unregistered_project_reports_not_registered_without_registering(self):
        self.pwt("repo-add", "--root", str(self.b), "--data-dir", str(self.data))
        ok, lines = install_claude.connection_test(self.entry(self.a))
        self.assertTrue(ok, lines)
        self.assertIn("PROJECT_NOT_REGISTERED", "\n".join(lines))
        repo, _ = install_claude.registered_project(self.data, self.a)
        self.assertIsNone(repo, "a status check never registers the project")


if __name__ == "__main__":
    unittest.main()

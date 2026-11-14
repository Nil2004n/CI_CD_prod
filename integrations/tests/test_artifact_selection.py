#!/usr/bin/env python3
"""Artifact-selection regression tests: one policy for every entrypoint.

Policy (pwt-mcp-stdio.sh, used by pwt.sh, the dashboard helper and
--print-jar): --jar, else PWT_JAR, else pwt/target/pwt-<pom version>.jar,
else exactly one pwt/target/pwt-*.jar, else fail listing the candidates.
Runs the real scripts in a fake application tree with a stub `java`.
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

INTEGRATIONS = Path(__file__).resolve().parent.parent
TOOLS = ("sh", "dirname", "sed", "grep", "head", "cat", "mkdir", "printf", "env")


class ArtifactSelectionTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pwt artifact "))
        self.app = self.tmp / "app root"
        integ = self.app / "integrations"
        integ.mkdir(parents=True)
        for name in ("pwt-mcp-stdio.sh", "pwt.sh", "pwt-dashboard.sh"):
            shutil.copy2(INTEGRATIONS / name, integ / name)
        self.target = self.app / "pwt" / "target"
        self.target.mkdir(parents=True)
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        for tool in TOOLS:
            found = shutil.which(tool)
            if found:
                (self.bin / tool).symlink_to(found)
        self.java = self.tmp / "stub java"
        self.java.write_text('#!/bin/sh\nif [ "$1" = "-version" ]; then exit 0; fi\nprintf "%s\\n" "$@"\n')
        self.java.chmod(0o755)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def pom(self, version):
        (self.app / "pwt" / "pom.xml").write_text(
            f"<project>\n  <artifactId>pwt</artifactId>\n  <version>{version}</version>\n"
            "  <dependency><version>21.0.8</version></dependency>\n</project>\n")

    def jars(self, *names):
        for name in names:
            (self.target / name).write_bytes(b"")

    def env(self, **extra):
        env = {"PATH": str(self.bin), "HOME": str(self.tmp)}
        env.update(extra)
        return env

    def print_jar(self, *args, **env):
        return subprocess.run(["/bin/sh", str(self.app / "integrations" / "pwt-mcp-stdio.sh"),
                               "--java", str(self.java), *args, "--print-jar"],
                              capture_output=True, text=True, env=self.env(**env), timeout=30)

    def resolved(self, result):
        self.assertEqual(result.returncode, 0, result.stderr)
        return Path(result.stdout.strip()).name

    def test_declared_version_wins_over_old_and_renamed_jars(self):
        self.pom("1.1.0")
        self.jars("pwt-0.4.0.jar", "pwt-1.0.0.jar", "pwt-1.1.0.jar", "original-pwt-1.1.0.jar",
                  "pwt-1.0.0.jar.inuse")
        result = self.print_jar()
        self.assertEqual(self.resolved(result), "pwt-1.1.0.jar")
        self.assertEqual(result.stderr, "", "no warning when the declared version is present")

    def test_single_candidate_is_used_when_declared_jar_is_missing(self):
        self.pom("1.1.0")
        self.jars("pwt-1.0.0.jar", "original-pwt-1.0.0.jar", "pwt-0.4.0.jar.inuse")
        self.assertEqual(self.resolved(self.print_jar()), "pwt-1.0.0.jar")

    def test_ambiguous_without_declared_match_fails_and_lists_candidates(self):
        self.pom("1.1.0")
        self.jars("pwt-0.4.0.jar", "pwt-1.0.0.jar")
        result = self.print_jar()
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn("pwt-0.4.0.jar", result.stderr)
        self.assertIn("pwt-1.0.0.jar", result.stderr)
        self.assertIn("declared version 1.1.0", result.stderr)

    def test_no_artifact_fails(self):
        self.pom("1.1.0")
        self.jars("original-pwt-1.1.0.jar")
        result = self.print_jar()
        self.assertEqual(result.returncode, 2)
        self.assertIn("no application artifact", result.stderr)

    def test_explicit_jar_and_pwt_jar_override(self):
        self.pom("1.1.0")
        self.jars("pwt-1.1.0.jar")
        explicit = self.tmp / "elsewhere" / "pwt-custom.jar"
        explicit.parent.mkdir()
        explicit.write_bytes(b"")
        self.assertEqual(self.resolved(self.print_jar("--jar", str(explicit))), "pwt-custom.jar")
        self.assertEqual(self.resolved(self.print_jar(PWT_JAR=str(explicit))), "pwt-custom.jar")

    def test_installed_launcher_uses_the_same_policy(self):
        self.pom("1.1.0")
        self.jars("pwt-0.4.0.jar", "pwt-1.1.0.jar")
        run = subprocess.run(["/bin/sh", str(self.app / "integrations" / "pwt.sh"), "doctor", "--x"],
                             capture_output=True, text=True, timeout=30,
                             env=self.env(PWT_JAVA=str(self.java)))
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(run.stdout.splitlines(), ["-jar", str(self.target / "pwt-1.1.0.jar"), "doctor", "--x"])
        self.jars("pwt-1.0.0.jar")
        (self.target / "pwt-1.1.0.jar").unlink()
        ambiguous = subprocess.run(["/bin/sh", str(self.app / "integrations" / "pwt.sh"), "doctor"],
                                   capture_output=True, text=True, timeout=30,
                                   env=self.env(PWT_JAVA=str(self.java)))
        self.assertEqual(ambiguous.returncode, 2)
        self.assertIn("ambiguous application artifacts", ambiguous.stderr)

    def test_dashboard_helper_uses_the_same_policy(self):
        self.pom("1.1.0")
        self.jars("pwt-0.4.0.jar", "pwt-1.1.0.jar")
        data = self.tmp / "data dir"
        run = subprocess.run(["/bin/sh", str(self.app / "integrations" / "pwt-dashboard.sh"),
                              "--java", str(self.java), "--data-dir", str(data)],
                             capture_output=True, text=True, timeout=30, env=self.env(DISPLAY=":99"))
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(run.stdout.splitlines(),
                         ["-jar", str(self.target / "pwt-1.1.0.jar"), "gui", "--data-dir", str(data)])
        (self.target / "pwt-1.1.0.jar").unlink()
        self.jars("pwt-1.0.0.jar")
        refused = subprocess.run(["/bin/sh", str(self.app / "integrations" / "pwt-dashboard.sh"),
                                  "--java", str(self.java), "--data-dir", str(data)],
                                 capture_output=True, text=True, timeout=30, env=self.env(DISPLAY=":99"))
        self.assertEqual(refused.returncode, 2)
        self.assertIn("no JAR configured", refused.stderr)


if __name__ == "__main__":
    unittest.main()

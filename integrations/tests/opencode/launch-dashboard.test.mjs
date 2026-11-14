// Regression tests for the OpenCode /pwt dashboard launch: no PWT subprocess
// output may reach the TUI's terminal (it was painted over the prompt as a
// "ghost"), failures surface as controlled messages, paths with spaces stay
// intact. Run: node --test integrations/tests/opencode/launch-dashboard.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const SOURCE = process.env.PWT_TUI_SOURCE ?? join(HERE, "..", "..", "opencode-plugin", "tui.ts");
const MARKER = /PWT-LEAK-[A-Z0-9-]+/;

function lab() {
  const root = mkdtempSync(join(tmpdir(), "pwt dashboard lab "));
  // Module copy with a stub for the OpenCode plugin API (only Plugin.define is used at import time).
  const mod = join(root, "plugin");
  mkdirSync(join(mod, "node_modules", "@opencode", "plugin"), { recursive: true });
  writeFileSync(join(mod, "node_modules", "@opencode", "plugin", "package.json"),
    JSON.stringify({ name: "@opencode/plugin", type: "module", exports: { "./tui": "./tui.js" } }));
  writeFileSync(join(mod, "node_modules", "@opencode", "plugin", "tui.js"),
    "export const Plugin = { define: (definition) => definition };\n");
  writeFileSync(join(mod, "package.json"), JSON.stringify({ type: "module" }));
  copyFileSync(SOURCE, join(mod, "tui.ts"));
  const integrations = join(root, "app dir", "integrations");
  mkdirSync(integrations, { recursive: true });
  const data = join(root, "data dir");
  mkdirSync(data);
  return { root, mod, integrations, data };
}

function script(path, body) {
  writeFileSync(path, "#!/bin/sh\n" + body);
  chmodSync(path, 0o755);
}

function runScenario(env, scenario) {
  const scenarioFile = join(env.root, "scenario.json");
  const resultFile = join(env.root, "result.json");
  writeFileSync(scenarioFile, JSON.stringify(scenario));
  const run = spawnSync(process.execPath, [join(HERE, "launch-runner.mjs"), join(env.mod, "tui.ts"),
    scenarioFile, resultFile], { encoding: "utf8", timeout: 30_000 });
  assert.equal(run.status, 0, `runner failed: ${run.stderr}`);
  const terminal = (run.stdout + run.stderr)
    .split("\n").filter((line) => !line.includes("ExperimentalWarning") && !line.includes("--trace-warnings"))
    .join("\n");
  return { result: JSON.parse(readFileSync(resultFile, "utf8")), terminal };
}

const DISPLAY_ENV = { DISPLAY: ":99" };

test("valid jar on stdout plus launcher warning on stderr: launches, nothing reaches the terminal", () => {
  const env = lab();
  try {
    const jar = join(env.root, "app dir", "pwt", "target", "pwt-1.0.0.jar");
    script(join(env.integrations, "pwt-mcp-stdio.sh"),
      `echo "PWT-LEAK-LAUNCHER-STDERR note: multiple artifacts" >&2\nprintf '%s\\n' "${jar}"\n`);
    const argsFile = join(env.root, "helper args.txt");
    script(join(env.integrations, "pwt-dashboard.sh"),
      `echo "PWT-LEAK-HELPER-STDOUT"\necho "PWT-LEAK-HELPER-STDERR" >&2\n` +
      `( sleep 0.5; echo "PWT-LEAK-DESCENDANT-STDOUT"; echo "PWT-LEAK-DESCENDANT-STDERR" >&2; ` +
      `printf '%s\\n' "$@" > "${argsFile}" ) &\nexit 0\n`);
    const { result, terminal } = runScenario(env, {
      entry: { command: [join(env.integrations, "pwt-mcp-stdio.sh"), "--data-dir", env.data], dataDir: env.data },
      env: DISPLAY_ENV,
      waitForFile: argsFile,
    });
    assert.match(result.message, /Dashboard launch requested/);
    assert.equal(result.failure, null);
    assert.doesNotMatch(terminal, MARKER, `leaked onto the terminal: ${terminal}`);
    // Paths containing spaces arrive as single, intact arguments.
    assert.deepEqual(readFileSync(argsFile, "utf8").trim().split("\n"), ["--jar", jar, "--data-dir", env.data]);
  } finally {
    rmSync(env.root, { recursive: true, force: true });
  }
});

test("failed jar lookup: controlled message with the launcher's reason, no raw terminal output", () => {
  const env = lab();
  try {
    script(join(env.integrations, "pwt-mcp-stdio.sh"),
      `echo "PWT-LEAK-LOOKUP-FAILED no application artifact found" >&2\nexit 2\n`);
    script(join(env.integrations, "pwt-dashboard.sh"), "exit 0\n");
    const { result, terminal } = runScenario(env, {
      entry: { command: [join(env.integrations, "pwt-mcp-stdio.sh")], dataDir: env.data },
      env: DISPLAY_ENV,
    });
    assert.match(result.message, /^Could not resolve the PWT application jar for the dashboard: /);
    assert.match(result.message, /no application artifact found/);
    assert.doesNotMatch(terminal, MARKER, `leaked onto the terminal: ${terminal}`);
  } finally {
    rmSync(env.root, { recursive: true, force: true });
  }
});

test("helper fails after starting: reported through the notification callback, no raw output", () => {
  const env = lab();
  try {
    script(join(env.integrations, "pwt-mcp-stdio.sh"), `printf '%s\\n' "/x/pwt-1.0.0.jar"\n`);
    script(join(env.integrations, "pwt-dashboard.sh"),
      `echo "PWT-LEAK-HELPER-ERROR JAR not found" >&2\nexit 7\n`);
    const { result, terminal } = runScenario(env, {
      entry: { command: [join(env.integrations, "pwt-mcp-stdio.sh")], dataDir: env.data },
      env: DISPLAY_ENV,
      expectFailureCallback: true,
    });
    assert.match(result.message, /Dashboard launch requested/);
    assert.match(result.failure ?? "", /exited with code 7/);
    assert.doesNotMatch(terminal, MARKER, `leaked onto the terminal: ${terminal}`);
  } finally {
    rmSync(env.root, { recursive: true, force: true });
  }
});

test("launch cannot start (java missing on the fallback path): controlled error, no stack trace", () => {
  const env = lab();
  try {
    const empty = join(env.root, "empty bin");
    mkdirSync(empty);
    const { result, terminal } = runScenario(env, {
      entry: { command: ["/opt/custom/java", "-jar", "/abs/pwt-1.0.0.jar", "mcp"], dataDir: env.data },
      env: DISPLAY_ENV,
      path: empty,
    });
    assert.match(result.message, /^Could not start the PWT dashboard: /);
    assert.match(result.message, /ENOENT/);
    assert.doesNotMatch(terminal, /Error:|at .*\(|Uncaught/, `raw error reached the terminal: ${terminal}`);
  } finally {
    rmSync(env.root, { recursive: true, force: true });
  }
});

test("no graphical session: refuses before running any subprocess", () => {
  const env = lab();
  try {
    script(join(env.integrations, "pwt-mcp-stdio.sh"), `echo "PWT-LEAK-SHOULD-NOT-RUN" >&2\nexit 9\n`);
    const { result, terminal } = runScenario(env, {
      entry: { command: [join(env.integrations, "pwt-mcp-stdio.sh")], dataDir: env.data },
      env: {},
    });
    assert.match(result.message, /No graphical session detected/);
    assert.doesNotMatch(terminal, MARKER);
  } finally {
    rmSync(env.root, { recursive: true, force: true });
  }
});

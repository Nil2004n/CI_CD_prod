import { Plugin } from "@opencode/plugin/tui";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

export interface McpEntry {
  command: string[];
  dataDir: string;
  allowScans: boolean;
  jarPath?: string;
  helperDir?: string;
}

function parseMcpEntry(file: string, projectDir: string):
    | { entry: McpEntry; projectDir: string }
    | null {
  if (!existsSync(file)) {
    return null;
  }
  try {
    const text = readFileSync(file, "utf8");
    const stripped = text
      .split("\n")
      .filter((line) => {
        const trimmed = line.trim();
        return !trimmed.startsWith("//");
      })
      .join("\n");
    const config = JSON.parse(stripped) as {
      mcp?: { servers?: Record<string, { command?: string[] }> };
    };
    const server = config?.mcp?.servers?.["pwt"];
    if (!server || !Array.isArray(server.command) || server.command.length === 0) {
      return null;
    }
    return { entry: buildMcpEntry(server.command as string[]), projectDir };
  } catch {
    return null;
  }
}

function buildMcpEntry(command: string[]): McpEntry {
  // Default data dir: resolved from the launcher location so every
  // entrypoint shares ONE database at <application-root>/.pwt-data.
  // An explicit --data-dir in the entry always wins.
  let dataDir = join(homedir(), ".pwt-data");
  const absFirst = command[0].startsWith("/") ? resolve(command[0]) : null;
  const launcherBase = absFirst !== null && absFirst.endsWith("pwt-mcp-stdio.sh")
    ? dirname(dirname(absFirst))
    : null;
  const jarBase = (() => {
    const jarIndex = command.indexOf("-jar");
    if (jarIndex >= 0 && jarIndex + 1 < command.length) {
      const jarPath = command[jarIndex + 1];
      if (!jarPath.startsWith("/")) {
        return null;
      }
      const targetDir = dirname(resolve(jarPath));
      if (targetDir.endsWith("target") && dirname(targetDir).endsWith("pwt")) {
        return dirname(dirname(targetDir));
      }
    }
    return null;
  })();
  const appRoot = launcherBase ?? jarBase;
  if (appRoot !== null) {
    dataDir = join(appRoot, ".pwt-data");
  }
  let allowScans = false;
  for (let i = 0; i < command.length; i++) {
    if (command[i] === "--data-dir" && i + 1 < command.length) {
      dataDir = command[i + 1];
    }
    if (command[i] === "--allow-scans") {
      allowScans = true;
    }
  }
  let jarPath: string | undefined;
  let helperDir: string | undefined;
  if (command[0] === "java") {
    const jarIndex = command.indexOf("-jar");
    if (jarIndex >= 0 && jarIndex + 1 < command.length) {
      jarPath = command[jarIndex + 1];
    }
  } else {
    helperDir = dirname(command[0]);
  }
  return { command, dataDir, allowScans, jarPath, helperDir };
}

function readProjectEntry(startDir: string): { entry: McpEntry; projectDir: string } {
  let dir = resolve(startDir);
  for (let depth = 0; depth < 12; depth++) {
    for (const name of ["opencode.json", "opencode.jsonc"]) {
      const found = parseMcpEntry(join(dir, name), dir);
      if (found !== null) {
        return found;
      }
    }
    const parent = dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  // Fallback: user-global config, so native commands work from any directory
  // (unscoped entry; every call is still repository-scoped server-side).
  const here = resolve(startDir);
  for (const name of ["opencode.json", "opencode.jsonc"]) {
    const found = parseMcpEntry(join(homedir(), ".config", "opencode", name), here);
    if (found !== null) {
      return found;
    }
  }
  throw new Error(
    "PWT MCP server is not configured for this project (make setup-opencode ROOT=<project>)",
  );
}

interface JsonRpcResponse {
  jsonrpc: string;
  id?: number;
  result?: unknown;
  error?: { message?: string };
}

/**
 * Owned persistent MCP stdio clients, one per distinct spawn command
 * (data dir + flags + --repo-id scope are part of the command, so scopes
 * never share a mutable session). A process is spawned on first use and
 * kept for the plugin lifetime: scan workers belong to the server process,
 * so killing the server after start_scan would cancel the scan it just
 * started (the server shutdown hook cancels live tokens). No model
 * involved; pure JSON-RPC over stdio.
 */
class PersistentMcpClient {
  private child: ChildProcess | null = null;
  private readonly command: string[];
  private pending = new Map<number, (message: JsonRpcResponse) => void>();
  private nextId = 0;
  private buffer = "";
  private ready: Promise<void> | null = null;
  private writeQueue: Promise<void> = Promise.resolve();
  private deadReason: string | null = null;

  constructor(command: string[]) {
    this.command = command;
  }

  private ensureStarted(): Promise<void> {
    if (this.ready !== null) {
      return this.ready;
    }
    this.ready = new Promise((resolve, reject) => {
      let child: ChildProcess;
      try {
        child = spawn(this.command[0], this.command.slice(1), {
          stdio: ["pipe", "pipe", "pipe"],
        });
      } catch (error) {
        reject(error instanceof Error ? error : new Error(String(error)));
        return;
      }
      this.child = child;
      child.stdout?.on("data", (chunk: Buffer) => {
        this.buffer += chunk.toString("utf8");
        let index = this.buffer.indexOf("\n");
        while (index >= 0) {
          const line = this.buffer.slice(0, index).trim();
          this.buffer = this.buffer.slice(index + 1);
          if (line.length > 0) {
            try {
              const message = JSON.parse(line) as JsonRpcResponse;
              const handler =
                typeof message.id === "number" ? this.pending.get(message.id) : undefined;
              if (handler) {
                this.pending.delete(message.id as number);
                handler(message);
              }
            } catch {
              // Non-protocol stdout would break the contract; ignore and keep waiting.
            }
          }
          index = this.buffer.indexOf("\n");
        }
      });
      child.on("error", (error) => {
        this.failAll(error instanceof Error ? error : new Error(String(error)));
      });
      child.on("exit", (code) => {
        this.failAll(new Error(`MCP server exited (code ${code ?? "unknown"})`));
      });
      this.request("initialize", {
        protocolVersion: "2024-11-05",
        capabilities: {},
        clientInfo: { name: "pwt-opencode-plugin", version: "1" },
      }).then(
        () => {
          this.send({ jsonrpc: "2.0", method: "notifications/initialized" });
          resolve();
        },
        (error: unknown) => {
          reject(error instanceof Error ? error : new Error(String(error)));
        },
      );
    });
    return this.ready;
  }

  private send(message: object): void {
    // Serialize writes so pipelined requests never interleave on the pipe.
    this.writeQueue = this.writeQueue.then(
      () =>
        new Promise<void>((resolve) => {
          const line = JSON.stringify(message) + "\n";
          const child = this.child;
          if (!child?.stdin || child.stdin.destroyed) {
            resolve();
            return;
          }
          if (child.stdin.write(line)) {
            resolve();
          } else {
            child.stdin.once("drain", () => resolve());
          }
        }),
    );
  }

  private failAll(error: Error): void {
    this.deadReason = error.message;
    for (const [, handler] of this.pending) {
      try {
        handler({ jsonrpc: "2.0", error: { message: error.message } });
      } catch {
        // handler errors must not break shutdown
      }
    }
    this.pending.clear();
    this.child = null;
    this.ready = null;
  }

  private request(method: string, methodParams: object, timeoutMs = 60000): Promise<unknown> {
    return new Promise((resolveRequest, rejectRequest) => {
      const id = ++this.nextId;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        rejectRequest(new Error(`MCP ${method} timed out`));
      }, timeoutMs);
      this.pending.set(id, (message) => {
        clearTimeout(timer);
        if (message.error) {
          rejectRequest(new Error(message.error.message ?? "MCP error"));
        } else {
          resolveRequest(message.result);
        }
      });
      this.send({ jsonrpc: "2.0", id, method, params: methodParams });
    });
  }

  async call(tool: string, params: Record<string, unknown>): Promise<unknown> {
    await this.ensureStarted();
    if (this.deadReason !== null) {
      throw new Error(this.deadReason);
    }
    return this.request("tools/call", { name: tool, arguments: params });
  }

  close(): void {
    try {
      this.child?.kill();
    } catch {
      // already gone
    }
    this.child = null;
  }
}

const mcpClients = new Map<string, PersistentMcpClient>();

function mcpClientFor(command: string[]): PersistentMcpClient {
  const key = JSON.stringify(command);
  let client = mcpClients.get(key);
  if (!client) {
    client = new PersistentMcpClient(command);
    mcpClients.set(key, client);
  }
  return client;
}

function closeAllMcpClients(): void {
  for (const client of mcpClients.values()) {
    client.close();
  }
  mcpClients.clear();
}

interface ToolResult {
  content?: Array<{ type?: string; text?: string }>;
  isError?: boolean;
}

function toolText(result: unknown): string {
  const typed = result as ToolResult;
  const first = typed?.content?.[0];
  return typeof first?.text === "string" ? first.text : JSON.stringify(result);
}

function toolError(result: unknown): Error {
  const text = toolText(result);
  try {
    const parsed = JSON.parse(text) as { code?: string; message?: string };
    return new Error(`${parsed.code ?? "MCP_ERROR"}: ${parsed.message ?? text}`);
  } catch {
    return new Error(text.slice(0, 500));
  }
}

async function callTool(
  serverCommand: string[],
  tool: string,
  params: Record<string, unknown>,
): Promise<string> {
  const result = await mcpClientFor(serverCommand).call(tool, params);
  const typed = result as ToolResult;
  if (typed?.isError === true) {
    throw toolError(result);
  }
  return toolText(result);
}

function summarizeRun(run: Record<string, unknown>): string {
  const id = run["run_id"] ?? run["id"] ?? "?";
  const state = String(run["state"] ?? "?");
  const verdict = String(run["verdict"] ?? "?");
  const findings = typeof run["finding_count"] === "number" ? run["finding_count"] : "?";
  return `${id} — ${state} / ${verdict}, ${findings} finding(s)`;
}

/**
 * Pure argument dispatcher for PWT native commands (no OpenCode imports,
 * unit-testable with plain node). Input is the raw slash-command argument
 * string; output describes deterministic actions for the TUI layer.
 */

type PwtAction =
  | { kind: "help" }
  | { kind: "repos" }
  | { kind: "status"; runId?: string }
  | { kind: "report"; runId?: string; latest: boolean }
  | { kind: "dashboard" }
  | { kind: "error"; message: string };

interface ParsedArgs {
  action: string;
  flags: Record<string, string | boolean | string[]>;
  positionals: string[];
}

/** Split an argument string honoring single/double quotes. */
function splitArgs(input: string): string[] {
  const out: string[] = [];
  let current = "";
  let quote: string | null = null;
  for (let i = 0; i < input.length; i++) {
    const ch = input[i];
    if (quote !== null) {
      if (ch === quote) {
        quote = null;
      } else if (ch === "\\" && i + 1 < input.length && input[i + 1] === quote) {
        current += input[++i];
      } else {
        current += ch;
      }
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === " " || ch === "\t") {
      if (current.length > 0) {
        out.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current.length > 0) {
    out.push(current);
  }
  return out;
}

/** Parse `--flag value` / `--flag=value` / `--no-flag` / positional tokens.
 * Repeatable flags accumulate into arrays. */
function parseArgs(tokens: string[]): ParsedArgs {
  const flags: Record<string, string | boolean | string[]> = {};
  const positionals: string[] = [];
  const pushFlag = (key: string, value: string | boolean) => {
    const existing = flags[key];
    if (existing === undefined) {
      flags[key] = value;
    } else if (Array.isArray(existing)) {
      existing.push(String(value));
    } else {
      flags[key] = [String(existing), String(value)];
    }
  };
  let action = "";
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token.startsWith("--no-")) {
      flags[token.slice(5)] = false;
    } else if (token.startsWith("--")) {
      const eq = token.indexOf("=");
      if (eq >= 0) {
        pushFlag(token.slice(2, eq), token.slice(eq + 1));
      } else if (i + 1 < tokens.length && !tokens[i + 1].startsWith("--")) {
        pushFlag(token.slice(2), tokens[++i]);
      } else {
        flags[token.slice(2)] = true;
      }
    } else if (action === "") {
      action = token;
    } else {
      positionals.push(token);
    }
  }
  return { action, flags, positionals };
}

function usage(): string {
  return [
    "PWT commands (direct, no model involved):",
    "/pwt                         open the dashboard",
    "/pwt help                    this help",
    "/pwt repos                   current project repository",
    "/pwt status [run-id]         latest or exact run state",
    "/pwt report [latest|<run-id>] bounded report with gaps",
    "/pwt dashboard               open the dashboard",
    "/pwt-ask <message>            ask the Pwt agent (conversation, uses model)",
  ].join("\n");
}

/** Deterministic dispatch: subcommand word -> action. Never guesses. */
function dispatch(rawInput: string | undefined | null): PwtAction {
  const text = (rawInput ?? "").trim();
  if (text === "") {
    return { kind: "dashboard" };
  }
  const { action, positionals } = parseArgs(splitArgs(text));
  switch (action) {
    case "help":
      return { kind: "help" };
    case "repos":
      return { kind: "repos" };
    case "scan":
    case "cancel":
      return {
        kind: "error",
        message:
          "Scanning and cancellation now happen in the dashboard. Use /pwt.",
      };
    case "status":
      if (positionals.length > 1) {
        return { kind: "error", message: "Usage: /pwt status [run-id]" };
      }
      return { kind: "status", runId: positionals[0] };
    case "report": {
      const target = positionals[0] ?? "latest";
      if (positionals.length > 1) {
        return { kind: "error", message: "Usage: /pwt report [latest|<run-id>]" };
      }
      if (target === "latest") {
        return { kind: "report", latest: true };
      }
      return { kind: "report", runId: target, latest: false };
    }
    case "dashboard":
      return { kind: "dashboard" };
    case "ask":
      // Explicit conversation route; native commands never invoke the model.
      return {
        kind: "error",
        message:
          "Conversation lives outside native commands: use /pwt-ask <message> (selects the Pwt agent) or select the Pwt agent in the agents dialog and ask directly.",
      };
    default:
      return { kind: "error", message: `Unknown /pwt subcommand: ${action}\n\n${usage()}` };
  }
}

function helpText(): string {
  return usage();
}

function firstLine(value: unknown): string {
  const text = typeof value === "string" ? value : Buffer.isBuffer(value) ? value.toString("utf8") : "";
  const line = text.split("\n").map((item) => item.trim()).find((item) => item.length > 0) ?? "";
  return line.slice(0, 300);
}

/**
 * Opens the dashboard (the helper's single-instance check focuses/keeps an
 * existing window) without letting any child process write onto the TUI's
 * terminal: the jar lookup runs with captured stdout/stderr and only its
 * stdout is parsed; the launched helper and all of its descendants get
 * stdio "ignore". Failures are returned as text for the caller's toast;
 * a later non-zero helper exit is reported through onFailure.
 */
export async function launchDashboard(
  entry: McpEntry,
  env: Record<string, string | undefined>,
  onFailure: (message: string) => void,
): Promise<string> {
  const display = env["DISPLAY"] ?? "";
  const wayland = env["WAYLAND_DISPLAY"] ?? "";
  if (display === "" && wayland === "") {
    return "No graphical session detected (DISPLAY and WAYLAND_DISPLAY are unset). The dashboard can only open on a machine with a display.";
  }
  let command: string;
  let args: string[];
  // Prefer the dashboard helper next to the launcher; fall back to a
  // direct java invocation parsed from the MCP entry.
  const helper = entry.command[0].endsWith("pwt-mcp-stdio.sh")
    ? join(dirname(entry.command[0]), "pwt-dashboard.sh")
    : undefined;
  if (helper !== undefined && existsSync(helper)) {
    let jar: string | undefined;
    try {
      const stdout = execFileSync(entry.command[0], ["--print-jar"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
        timeout: 60_000,
      });
      jar = stdout.split("\n").map((line) => line.trim()).filter((line) => line.length > 0).pop();
    } catch (error) {
      const detail = firstLine((error as { stderr?: unknown }).stderr);
      return `Could not resolve the PWT application jar for the dashboard${detail ? `: ${detail}` : "."}`;
    }
    if (!jar) {
      return "Could not resolve the PWT application jar for the dashboard.";
    }
    command = helper;
    args = ["--jar", jar, "--data-dir", entry.dataDir];
  } else {
    const jarIndex = entry.command.indexOf("-jar");
    const jar = jarIndex >= 0 ? entry.command[jarIndex + 1] : undefined;
    if (!jar) {
      return "Could not resolve the PWT application jar for the dashboard.";
    }
    command = "java";
    args = ["-jar", jar, "gui", "--data-dir", entry.dataDir];
  }
  let child: ChildProcess;
  try {
    child = spawn(command, args, { detached: true, stdio: "ignore" });
  } catch (error) {
    return `Could not start the PWT dashboard: ${firstLine(error instanceof Error ? error.message : String(error))}`;
  }
  const failure = await new Promise<string | null>((resolveStart) => {
    child.once("spawn", () => resolveStart(null));
    child.once("error", (error) => resolveStart(error.message));
  });
  if (failure !== null) {
    return `Could not start the PWT dashboard: ${firstLine(failure)}`;
  }
  child.on("error", () => {
    // already started; later errors (e.g. signalling) must not become uncaught exceptions
  });
  child.once("exit", (code) => {
    if (code !== null && code !== 0) {
      onFailure(`PWT dashboard helper exited with code ${code}; no window was opened.`);
    }
  });
  child.unref();
  return "Dashboard launch requested (check your local graphical session).";
}

export default Plugin.define({
  id: "pwt",
  setup(context) {
    const serverCommand = (entry: McpEntry, extraArgs: string[]): string[] => {
      // Scoped sessions always carry --repo-id; unscoped mode is never used
      // by this connector (it exists only as an explicit local-admin mode).
      const base = [...entry.command];
      return [...base, ...extraArgs];
    };

    const resolveRepo = async (entry: McpEntry, projectDir: string): Promise<{ id: string; display: string }> => {
      const text = await callTool(serverCommand(entry, []), "list_repositories", {});
      const parsed = JSON.parse(text) as {
        repositories?: Array<{ id: string; display_name: string; canonical_root: string; availability?: string }>;
      };
      const repos = parsed.repositories ?? [];
      if (repos.length === 0) {
        throw new Error(
          "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard.",
        );
      }
      const here = resolve(projectDir);
      let best: (typeof repos)[number] | undefined;
      let ambiguous = false;
      for (const repo of repos) {
        const root = resolve(repo.canonical_root);
        if (here === root || here.startsWith(root + "/")) {
          if (!best || root.length > resolve(best.canonical_root).length) {
            best = repo;
            ambiguous = false;
          } else if (root.length === resolve(best.canonical_root).length && repo.id !== best.id) {
            ambiguous = true;
          }
        }
      }
      if (ambiguous && best) {
        throw new Error(
          "Multiple registered repositories contain this directory; open /pwt and pick one, or run from inside a single project root.",
        );
      }
      if (!best) {
        throw new Error(
          "No PWT assessment is available for this project. Open /pwt and run its first scan in the dashboard.",
        );
      }
      return { id: best.id, display: `${best.display_name} (${best.canonical_root})` };
    };

    const runAction = async (rawInput: unknown): Promise<string> => {
      const action = dispatch(typeof rawInput === "string" ? rawInput : "");
      const projectDir = process.cwd();
      const { entry } = readProjectEntry(projectDir);
      // Every data call below is project-scoped.
      const scoped = (repoId: string, extra: string[]): string[] => [
        ...entry.command,
        "--repo-id",
        repoId,
        ...extra,
      ];
      switch (action.kind) {
        case "help": {
          return [
            "PWT commands (direct, no model involved):",
            "/pwt                         open the dashboard",
            "/pwt help                    this help",
            "/pwt repos                   current project repository",
            "/pwt status [run-id]         latest or exact run state",
            "/pwt report [latest|<run-id>] bounded report with gaps",
            "/pwt dashboard               open the dashboard",
            "/pwt-ask <message>            ask the Pwt agent (conversation, uses model)",
            "",
            "Scanning happens only in the dashboard: open /pwt, pick files, and scan.",
            `Data dir: ${entry.dataDir}`,
          ].join("\n");
        }
        case "repos": {
          const { display } = await resolveRepo(entry, projectDir);
          return `Current project repository:\n- ${display}`;
        }
        case "status": {
          const { id: repositoryId, display } = await resolveRepo(entry, projectDir);
          if (action.runId) {
            const run = JSON.parse(
              await callTool(scoped(repositoryId, []), "get_run", { run_id: action.runId }),
            ) as { run?: Record<string, unknown> };
            const summary = summarizeRun((run.run ?? {}) as Record<string, unknown>);
            return `${display}\n${summary}`;
          }
          const runs = JSON.parse(
            await callTool(scoped(repositoryId, []), "list_runs", { repository_id: repositoryId, limit: 1 }),
          ) as { runs?: Array<{ run_id: string }> };
          if (!runs.runs || runs.runs.length === 0) {
            return `${display}\nNo runs yet.`;
          }
          const run = JSON.parse(
            await callTool(scoped(repositoryId, []), "get_run", { run_id: runs.runs[0].run_id }),
          ) as { run?: Record<string, unknown> };
          return `${display}\nLatest: ${summarizeRun((run.run ?? {}) as Record<string, unknown>)}`;
        }
        case "report": {
          const { id: repositoryId } = await resolveRepo(entry, projectDir);
          let runId = action.runId;
          if (action.latest || !runId) {
            const runs = JSON.parse(
              await callTool(scoped(repositoryId, []), "list_runs", { repository_id: repositoryId, limit: 1 }),
            ) as { runs?: Array<{ run_id: string }> };
            if (!runs.runs || runs.runs.length === 0) {
              return "No runs yet for this project.";
            }
            runId = runs.runs[0].run_id;
          }
          const report = JSON.parse(
            await callTool(scoped(repositoryId, []), "get_report", { run_id: runId, format: "markdown", length: 6000 }),
          ) as { content?: string; truncated?: boolean };
          const suffix = report.truncated === true ? "\n…(truncated; use get_report with offset)" : "";
          return `Report ${runId}:\n${report.content ?? ""}${suffix}`;
        }
        case "dashboard":
          return launchDashboard(entry, process.env, (message) => {
            try {
              void context.ui.toast.show({ title: "PWT dashboard", message, variant: "error" });
            } catch {
              // plugin unloaded: nothing left to notify
            }
          });
        case "error":
          return action.message;
      }
    };

    const showResult = async (text: string) => {
      await context.ui.toast.show({ title: "PWT", message: text.slice(0, 4000), variant: "info" });
    };

    // Test hook (inactive unless PWT_TEST_OUT is set): appends one JSON line
    // per executed command so headless harnesses can verify native dispatch
    // without reading pixels.
    const recordTestHook = async (command: string, input: string, ok: boolean, output: string) => {
      const sink = process.env["PWT_TEST_OUT"];
      if (!sink) {
        return;
      }
      try {
        const fs = await import("node:fs");
        fs.appendFileSync(sink, JSON.stringify({ command, input, ok, output }) + "\n");
      } catch {
        // test hook must never break real commands
      }
    };

    const register = (id: string, title: string, name: string, fixedArgs: string | undefined) => ({
      id,
      title,
      slash: { name, arguments: fixedArgs === undefined },
      run: async (input: unknown) => {
        const argText = typeof input === "string" ? input : "";
        const fullText = fixedArgs !== undefined ? `${fixedArgs} ${argText}`.trim() : argText;
        try {
          const out = await runAction(fullText);
          await recordTestHook(name, argText, true, out);
          await showResult(out);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          await recordTestHook(name, argText, false, message);
          await context.ui.toast.show({
            title: "PWT failed",
            message: error instanceof Error ? error.message.slice(0, 2000) : String(error).slice(0, 2000),
            variant: "error",
          });
        }
      },
    });

    context.keymap.layer(() => ({
      mode: "global",
      commands: [
        register("pwt", "PWT: assessment workbench", "pwt", undefined),
        register("pwt-help", "PWT: help", "pwt-help", "help"),
        register("pwt-repos", "PWT: current project repository", "pwt-repos", "repos"),
        register("pwt-status", "PWT: run status", "pwt-status", "status"),
        register("pwt-report", "PWT: report", "pwt-report", "report"),
        register("pwt-dashboard", "PWT: dashboard", "pwt-dashboard", "dashboard"),
      ],
    }));
    // Owned MCP sessions die with the plugin: closing cancels nothing that
    // already finished and marks presence disconnected server-side.
    return () => closeAllMcpClients();
  },
});

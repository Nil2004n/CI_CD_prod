package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import com.fasterxml.jackson.databind.JsonNode;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.application.CancellationToken;
import pwt.application.ScanConfig;
import pwt.application.ScanException;
import pwt.application.ScanService;
import pwt.domain.ScanState;
import pwt.domain.ScanSummary;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;

/**
 * MCP tool behavior against the real server subprocess over a shared data
 * directory: read-only queries, pagination, errors, permission gating,
 * and opt-in scan operations with durable cancellation.
 */
class McpToolsTest {

    private static final String FAKE_TOKEN = "ghp_M1nB2vC3xZ4lK5jH6gF7dS8aP09oI7uY6tR5e";

    @TempDir
    Path work;

    private void write(Path repo, String relative, String content) throws Exception {
        Path file = repo.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
    }

    private static void assumeToolsPresent() {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
    }

    private Path seedDataDir(String name) throws Exception {
        Path repo = work.resolve(name);
        Files.createDirectories(repo);
        write(repo, "ok.yaml", "---\nkey: value\n");
        write(repo, "leak.yaml", "---\ntoken: \"" + FAKE_TOKEN + "\"\n");
        Path dataDir = work.resolve(name + "-data");
        ScanService service = new ScanService(ScanConfig.defaults());
        service.runScan(repo, List.of("ok.yaml", "leak.yaml"), Set.of("yamllint", "gitleaks"),
                "CLI", dataDir, CancellationToken.none(), null);
        return dataDir;
    }

    private String repoIdOf(Path dataDir, McpHarness mcp) throws Exception {
        JsonNode repos = mcp.callResult("list_repositories", Map.of());
        assertTrue(repos.get("repositories").size() >= 1);
        return repos.get("repositories").get(0).get("id").asText();
    }

    @Test
    void readOnlyToolsReturnPersistedData() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("seed");
        try (McpHarness mcp = new McpHarness(dataDir, false)) {
            JsonNode repos = mcp.callResult("list_repositories", Map.of());
            assertEquals(1, repos.get("total").asInt());
            String repoId = repos.get("repositories").get(0).get("id").asText();
            assertEquals("available", repos.get("repositories").get(0).get("availability").asText());

            JsonNode runs = mcp.callResult("list_runs", McpHarness.params("repository_id", repoId));
            assertEquals(1, runs.get("total").asInt());
            String runId = runs.get("runs").get(0).get("run_id").asText();
            assertEquals("FINDINGS", runs.get("runs").get(0).get("verdict").asText());

            JsonNode run = mcp.callResult("get_run", McpHarness.params("run_id", runId));
            assertEquals(2, run.get("targets").size());
            assertEquals(2, run.get("scanner_runs").size());
            assertEquals("CLI", run.get("run").get("source").asText());
            assertNotNull(run.get("reports").get("json_report").asText());
            assertTrue(run.get("coverage").get("gaps").isArray());

            JsonNode findings = mcp.callResult("list_findings", McpHarness.params("run_id", runId));
            assertTrue(findings.get("total").asInt() > 0);
            JsonNode first = findings.get("findings").get(0);
            assertNotNull(first.get("rule_id").asText());
            assertNotNull(first.get("provenance").get("adapter_id").asText());

            JsonNode report = mcp.callResult("get_report",
                    McpHarness.params("run_id", runId, "format", "json"));
            assertEquals("1.0", report.get("schema_version").asText());
            assertEquals(false, report.get("truncated").asBoolean());
            assertTrue(report.get("content").asText().contains(runId));
            JsonNode md = mcp.callResult("get_report",
                    McpHarness.params("run_id", runId, "format", "markdown", "length", 500));
            assertTrue(md.get("truncated").asBoolean());
            assertNotNull(md.get("next_offset"));
            JsonNode md2 = mcp.callResult("get_report", McpHarness.params("run_id", runId,
                    "format", "markdown", "offset", md.get("next_offset").asInt(), "length", 64000));
            assertTrue(md2.get("content").asText().length() > 0);
        }
    }

    @Test
    void secretNeverLeavesThroughMcp() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("hygiene");
        try (McpHarness mcp = new McpHarness(dataDir, false)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode runs = mcp.callResult("list_runs", McpHarness.params("repository_id", repoId));
            String runId = runs.get("runs").get(0).get("run_id").asText();
            JsonNode findings = mcp.callResult("list_findings",
                    McpHarness.params("run_id", runId, "limit", 500));
            assertFalse(findings.toString().contains(FAKE_TOKEN));
            JsonNode report = mcp.callResult("get_report",
                    McpHarness.params("run_id", runId, "format", "json", "length", 64000));
            assertFalse(report.get("content").asText().contains(FAKE_TOKEN));
            assertFalse(mcp.stderrText().contains(FAKE_TOKEN), "server diagnostics must not leak secrets");
        }
    }

    @Test
    void paginationAndValidation() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("pages");
        String repoId;
        try (McpHarness mcp = new McpHarness(dataDir, false)) {
            repoId = repoIdOf(dataDir, mcp);
        }
        // Two extra persisted runs make three total (store-level rows, not scans).
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow repo = store.findRepositoryById(repoId);
            String extra1 = store.createRun(repo.id(), "0.4.0", "{}");
            store.startRun(extra1);
            store.finalizeRun(extra1, pwt.domain.ScanState.COMPLETED, pwt.domain.ScanVerdict.NO_FINDINGS,
                    "{\"finding_count\":0,\"target_count\":0,\"gaps\":[]}", null, null);
            String extra2 = store.createRun(repo.id(), "0.4.0", "{}");
            store.startRun(extra2);
            store.finalizeRun(extra2, pwt.domain.ScanState.FAILED, pwt.domain.ScanVerdict.INDETERMINATE,
                    "{\"finding_count\":0,\"target_count\":0,\"gaps\":[]}", "FAILED", null);
        }
        try (McpHarness mcp = new McpHarness(dataDir, false)) {
            JsonNode page1 = mcp.callResult("list_runs",
                    McpHarness.params("repository_id", repoId, "limit", 1));
            assertEquals(3, page1.get("total").asInt());
            assertEquals(1, page1.get("runs").size());
            assertNotNull(page1.get("next_cursor"));
            JsonNode page2 = mcp.callResult("list_runs", McpHarness.params("repository_id", repoId,
                    "limit", 2, "cursor", page1.get("next_cursor").asText()));
            assertEquals(2, page2.get("runs").size());
            JsonNode failed = mcp.callResult("list_runs",
                    McpHarness.params("repository_id", repoId, "state", "FAILED"));
            assertEquals(1, failed.get("total").asInt());
            JsonNode badState = mcp.callToolError("list_runs",
                    McpHarness.params("repository_id", repoId, "state", "BOGUS"));
            assertEquals("INVALID_INPUT", badState.get("code").asText());
            JsonNode badLimit = mcp.callToolError("list_runs",
                    McpHarness.params("repository_id", repoId, "limit", 501));
            assertEquals("INVALID_INPUT", badLimit.get("code").asText());
            JsonNode badCursor = mcp.callToolError("list_runs",
                    McpHarness.params("repository_id", repoId, "cursor", "nope"));
            assertEquals("INVALID_INPUT", badCursor.get("code").asText());
            JsonNode badSeverity = mcp.callToolError("list_findings", McpHarness.params(
                    "run_id", page1.get("runs").get(0).get("run_id").asText(), "severity", "BOGUS"));
            assertEquals("INVALID_INPUT", badSeverity.get("code").asText());
            JsonNode badFormat = mcp.callToolError("get_report", McpHarness.params(
                    "run_id", page1.get("runs").get(0).get("run_id").asText(), "format", "pdf"));
            assertEquals("INVALID_INPUT", badFormat.get("code").asText());
        }
    }

    @Test
    void unknownIdsReturnNotFound() throws Exception {
        Path dataDir = work.resolve("empty-data");
        Files.createDirectories(dataDir);
        try (McpHarness mcp = new McpHarness(dataDir, true)) {
            JsonNode run = mcp.callToolError("get_run", McpHarness.params("run_id", "nope"));
            assertEquals("NOT_FOUND", run.get("code").asText());
            JsonNode findings = mcp.callToolError("list_findings", McpHarness.params("run_id", "nope"));
            assertEquals("NOT_FOUND", findings.get("code").asText());
            JsonNode report = mcp.callToolError("get_report",
                    McpHarness.params("run_id", "nope", "format", "json"));
            assertEquals("NOT_FOUND", report.get("code").asText());
            JsonNode runs = mcp.callToolError("list_runs", McpHarness.params("repository_id", "nope"));
            assertEquals("NOT_FOUND", runs.get("code").asText());
            JsonNode cancel = mcp.callToolError("cancel_scan", McpHarness.params("run_id", "nope"));
            assertEquals("NOT_FOUND", cancel.get("code").asText());
            JsonNode start = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", "nope", "relative_paths", List.of("a.yaml")));
            assertEquals("NOT_FOUND", start.get("code").asText());
            assertNotNull(run.get("correlation_id"));
        }
    }

    @Test
    void defaultModeDeniesScanOperations() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("readonly");
        try (McpHarness mcp = new McpHarness(dataDir, false)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode start = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml")));
            assertEquals("PERMISSION_DENIED", start.get("code").asText());
            JsonNode cancel = mcp.callToolError("cancel_scan",
                    McpHarness.params("run_id", "anything"));
            assertEquals("PERMISSION_DENIED", cancel.get("code").asText());
            // Read-only tools still work.
            JsonNode repos = mcp.callResult("list_repositories", Map.of());
            assertEquals(1, repos.get("total").asInt());
        }
    }

    @Test
    void startScanValidatesScopeAndAdapters() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("validate");
        try (McpHarness mcp = new McpHarness(dataDir, true)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode traversal = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("../escape.yaml")));
            assertTrue(List.of("OUT_OF_SCOPE", "INVALID_INPUT").contains(traversal.get("code").asText()));
            JsonNode absolute = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("/abs.yaml")));
            assertTrue(List.of("OUT_OF_SCOPE", "INVALID_INPUT").contains(absolute.get("code").asText()));
            JsonNode unknownAdapter = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                    "adapter_ids", List.of("nope")));
            assertEquals("INVALID_INPUT", unknownAdapter.get("code").asText());
            JsonNode missingBindings = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                    "adapter_ids", List.of("local-schema")));
            assertEquals("CONFIGURATION_REQUIRED", missingBindings.get("code").asText());
            JsonNode missingSchema = mcp.callToolError("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                    "adapter_ids", List.of("local-schema"),
                    "schema_bindings", List.of("*.yaml=/nonexistent-schema.json")));
            assertEquals("CONFIGURATION_REQUIRED", missingSchema.get("code").asText());
        }
    }

    @Test
    void startScanRunsRealScanVisibleViaGetRun() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("exec");
        try (McpHarness mcp = new McpHarness(dataDir, true)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode started = mcp.callResult("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                    "adapter_ids", List.of("yamllint", "gitleaks")));
            String runId = started.get("run_id").asText();
            assertNotNull(runId);
            assertTrue(List.of("RUNNING", "COMPLETED", "PARTIAL", "FAILED")
                    .contains(started.get("state").asText()));
            // Poll for the terminal state through the same MCP surface.
            String state = started.get("state").asText();
            long deadline = System.currentTimeMillis() + 120_000;
            JsonNode run = null;
            while (System.currentTimeMillis() < deadline) {
                run = mcp.callResult("get_run", McpHarness.params("run_id", runId));
                state = run.get("run").get("state").asText();
                if (!state.equals("RUNNING") && !state.equals("CREATED")) {
                    break;
                }
                TimeUnit.MILLISECONDS.sleep(500);
            }
            assertEquals("COMPLETED", state);
            assertEquals("MCP", run.get("run").get("source").asText());
            assertTrue(run.get("targets").size() == 1);
        }
    }

    @Test
    void cancelScanReachesOwningWorkerReal() throws Exception {
        assumeToolsPresent();
        Path repo = work.resolve("cancel-repo2");
        Files.createDirectories(repo);
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("cancel-data2");
        ScanService service = new ScanService(ScanConfig.defaults());
        ScanSummary first = service.runScan(repo, List.of("a.yaml"), Set.of("gitleaks"),
                "CLI", dataDir, CancellationToken.none(), null);
        assertNotNull(first.runId());
        // Now the slow tool only sees the sleeper yamllint.
        Path bindir = work.resolve("fakebin2");
        Files.createDirectories(bindir);
        Path sleeper = bindir.resolve("yamllint");
        Files.writeString(sleeper, "#!/bin/sh\nsleep 60\n");
        sleeper.toFile().setExecutable(true);
        Map<String, String> env = Map.of("PATH",
                bindir.toString() + File.pathSeparator + System.getenv("PATH"));
        try (McpHarness mcp = new McpHarness(dataDir, true, env)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode started = mcp.callResult("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("a.yaml"),
                    "adapter_ids", List.of("yamllint")));
            String runId = started.get("run_id").asText();
            // Give the worker a moment to start the tool process.
            TimeUnit.SECONDS.sleep(3);
            JsonNode cancelled = mcp.callResult("cancel_scan", McpHarness.params("run_id", runId));
            assertEquals(true, cancelled.get("cancel_requested").asBoolean());
            long deadline = System.currentTimeMillis() + 90_000;
            String state = cancelled.get("state").asText();
            while (!state.equals("CANCELLED") && System.currentTimeMillis() < deadline) {
                TimeUnit.MILLISECONDS.sleep(1000);
                JsonNode run = mcp.callResult("get_run", McpHarness.params("run_id", runId));
                state = run.get("run").get("state").asText();
            }
            assertEquals("CANCELLED", state);
            // Cancelling a terminal run reports no-op honestly.
            JsonNode again = mcp.callResult("cancel_scan", McpHarness.params("run_id", runId));
            assertEquals(false, again.get("cancel_requested").asBoolean());
            assertEquals("CANCELLED", again.get("state").asText());
        }
    }

    @Test
    void busyRepositoryReported() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("busy");
        String repoId;
        String canonical;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.listRepositories().get(0);
            repoId = row.id();
            canonical = row.canonicalRoot();
        }
        String hash = pwt.domain.RepositoryRoot.identityHashFor(Path.of(canonical).toRealPath());
        try (RepoLock ignored = RepoLock.tryAcquire(dataDir, hash)
                .orElseThrow(() -> new IllegalStateException("cannot hold test lock"))) {
            try (McpHarness mcp = new McpHarness(dataDir, true)) {
                JsonNode busy = mcp.callToolError("start_scan", McpHarness.params(
                        "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                        "adapter_ids", List.of("gitleaks")));
                assertEquals("REPOSITORY_BUSY", busy.get("code").asText());
            }
        }
    }

    @Test
    void serverLifecyclePresence() throws Exception {
        Path dataDir = work.resolve("presence");
        Files.createDirectories(dataDir);
        String sessionId;
        try (McpHarness mcp = new McpHarness(dataDir, true)) {
            JsonNode repos = mcp.callResult("list_repositories", Map.of());
            assertNotNull(repos.get("total"));
            try (SqliteStore store = SqliteStore.open(dataDir)) {
                var rows = store.listLiveMcpSessions();
                assertEquals(1, rows.size());
                assertEquals("CONNECTED", rows.get(0).lifecycleState());
                assertEquals("SCANS_ENABLED", rows.get(0).permissionMode());
                assertTrue(rows.get(0).lastClientActivityAt() != null);
                sessionId = rows.get(0).sessionId();
                String firstBeat = rows.get(0).serverHeartbeatAt();
                TimeUnit.SECONDS.sleep(3);
                assertTrue(store.findMcpSession(sessionId).serverHeartbeatAt().compareTo(firstBeat) > 0,
                        "heartbeat must advance while the server lives");
            }
        }
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals("DISCONNECTED", store.findMcpSession(sessionId).lifecycleState(),
                    "orderly shutdown must mark the session disconnected");
            assertTrue(store.listLiveMcpSessions().isEmpty());
        }
    }

    @Test
    void abruptDeathLeavesStalePresence() throws Exception {
        Path dataDir = work.resolve("abrupt");
        Files.createDirectories(dataDir);
        String sessionId;
        McpHarness mcp = new McpHarness(dataDir, true);
        try {
            JsonNode info = mcp.callResult("pwt_info", Map.of());
            assertEquals("pwt", info.get("app").asText());
            try (SqliteStore store = SqliteStore.open(dataDir)) {
                assertEquals(1, store.listLiveMcpSessions().size());
                sessionId = store.listLiveMcpSessions().get(0).sessionId();
            }
        } finally {
            // No graceful shutdown: the row must stay CONNECTED with a frozen
            // heartbeat, which readers classify as stale - never resurrected,
            // never silently flipped.
            mcp.abort();
        }
        TimeUnit.SECONDS.sleep(3);
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals("CONNECTED", store.findMcpSession(sessionId).lifecycleState());
            var views = pwt.mcp.McpPresence.summarize(
                    store.listLiveMcpSessions(), java.time.Instant.now().plusSeconds(3600));
            assertEquals(1, views.size());
            assertEquals(pwt.mcp.McpPresence.Presence.STALE, views.get(0).presence());
        }
    }

    @Test
    void startScanWithBindingsValidatesSchemas() throws Exception {
        assumeToolsPresent();
        Path dataDir = seedDataDir("bindings");
        Path schema = Path.of(LocalSchemaTest.class.getResource("/localschema/widget-config.json").toURI());
        try (McpHarness mcp = new McpHarness(dataDir, true)) {
            String repoId = repoIdOf(dataDir, mcp);
            JsonNode started = mcp.callResult("start_scan", McpHarness.params(
                    "repository_id", repoId, "relative_paths", List.of("ok.yaml"),
                    "adapter_ids", List.of("local-schema"),
                    "schema_bindings", List.of("*.yaml=" + schema.toString())));
            assertNotNull(started.get("run_id").asText());
            long deadline = System.currentTimeMillis() + 60_000;
            String state;
            do {
                TimeUnit.MILLISECONDS.sleep(1000);
                JsonNode run = mcp.callResult("get_run",
                        McpHarness.params("run_id", started.get("run_id").asText()));
                state = run.get("run").get("state").asText();
            } while ((state.equals("RUNNING") || state.equals("CREATED"))
                    && System.currentTimeMillis() < deadline);
            assertTrue(List.of("COMPLETED", "PARTIAL", "FAILED").contains(state));
        }
    }
}

package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.stream.Stream;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.RepositoryRoot;
import pwt.domain.ScanState;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.infrastructure.SqliteStore;

/**
 * Project-root-bound MCP connections (the Claude Code integration): read-only
 * tools are confined to the one registered project owning the bound
 * directory. Fixtures are seeded directly into one shared store (no scanner
 * binaries needed): project A has two runs with an edit, a timeout, a skipped
 * adapter and coverage gaps; project B has a distinct run, findings and a
 * journal. Every test drives the real server subprocess over stdio.
 */
class McpProjectScopeTest {

    private static final String B_DISPLAY = "bravo-secret-project";
    private static final String B_RULE = "bravo-only-rule";

    @TempDir
    Path work;

    private Path dataDir;
    private Path rootA;
    private Path rootB;
    private String repoA;
    private String repoB;
    private String runA1;
    private String runA2;
    private String runB1;

    private static Finding finding(String adapter, String rule, Severity severity, String path, int line) {
        return new Finding(rule, rule + " title", severity, severity.name().toLowerCase(),
                FindingCategory.MISCONFIGURATION, path, line, line, 1, rule + " message",
                "{\"snippet\":\"redacted\"}",
                pwt.domain.Fingerprints.v1(adapter, rule, path, line, null));
    }

    private static String register(SqliteStore store, Path root, String name) throws Exception {
        Path canonical = root.toRealPath();
        return store.registerRepository(canonical.toString(), name,
                RepositoryRoot.identityHashFor(canonical)).id();
    }

    private static String run(SqliteStore store, String repoId, ScanState state, ScanVerdict verdict,
            String gapsJson) throws Exception {
        String id = store.createRun(repoId, "1.0.0", "{\"source\":\"GUI\"}");
        store.startRun(id);
        if (state != ScanState.RUNNING) {
            store.finalizeRun(id, state, verdict, gapsJson, null, null);
        }
        return id;
    }

    @BeforeEach
    void seed() throws Exception {
        dataDir = work.resolve("data");
        Files.createDirectories(dataDir);
        rootA = Files.createDirectories(work.resolve("alpha"));
        Files.createDirectories(rootA.resolve("sub/deeper"));
        rootB = Files.createDirectories(work.resolve(B_DISPLAY));
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            repoA = register(store, rootA, "alpha");
            repoB = register(store, rootB, B_DISPLAY);

            runA1 = run(store, repoA, ScanState.COMPLETED, ScanVerdict.FINDINGS, "{\"gaps\":[]}");
            store.addRunFile(runA1, "a.yaml", "sha-a-1", 10);
            store.addRunFile(runA1, "k8s.yaml", "sha-k-1", 20);
            String a1Lint = store.recordScannerRun(runA1, "yamllint", "yamllint", "1.38.0",
                    ScannerStatus.SUCCEEDED, 1, null);
            String a1Ckv = store.recordScannerRun(runA1, "checkov", "Checkov", "3.4.4",
                    ScannerStatus.SUCCEEDED, 1, null);
            store.addFindings(runA1, a1Lint, List.of(finding("yamllint", "truthy", Severity.LOW, "a.yaml", 3)));
            store.addFindings(runA1, a1Ckv, List.of(
                    finding("checkov", "CKV_K8S_20", Severity.HIGH, "k8s.yaml", 10)));

            runA2 = run(store, repoA, ScanState.PARTIAL, ScanVerdict.INDETERMINATE,
                    "{\"gaps\":[\"kubeconform: timed out before results for k8s.yaml\"]}");
            store.addRunFile(runA2, "a.yaml", "sha-a-2", 11);
            store.addRunFile(runA2, "k8s.yaml", "sha-k-1", 20);
            String a2Lint = store.recordScannerRun(runA2, "yamllint", "yamllint", "1.38.0",
                    ScannerStatus.SUCCEEDED, 1, null);
            store.recordScannerRun(runA2, "checkov", "Checkov", "3.4.4", ScannerStatus.SKIPPED, null,
                    "checkov not selected");
            store.recordScannerRun(runA2, "kubeconform", "kubeconform", "dev", ScannerStatus.TIMED_OUT, null,
                    "timed out before results");
            store.addFindings(runA2, a2Lint, List.of(
                    finding("yamllint", "truthy", Severity.LOW, "a.yaml", 4),
                    finding("yamllint", "indentation", Severity.MEDIUM, "a.yaml", 7)));

            runB1 = run(store, repoB, ScanState.COMPLETED, ScanVerdict.FINDINGS, "{\"gaps\":[]}");
            store.addRunFile(runB1, "b.yaml", "sha-b", 5);
            String b1Lint = store.recordScannerRun(runB1, "yamllint", "yamllint", "1.38.0",
                    ScannerStatus.SUCCEEDED, 1, null);
            store.addFindings(runB1, b1Lint, List.of(finding("yamllint", B_RULE, Severity.HIGH, "b.yaml", 1)));
        }
        // Project B has a journal; project A deliberately does not.
        Path reports = Files.createDirectories(dataDir.resolve("reports"));
        Files.writeString(reports.resolve(B_DISPLAY + "--" + repoB + ".md"), "# PWT scan journal\n");
    }

    private McpHarness boundTo(Path dir) throws Exception {
        return new McpHarness(dataDir, List.of("--project-root", dir.toAbsolutePath().toString()));
    }

    private static void assertNoLeak(JsonNode error, String... forbidden) {
        String text = error.toString();
        for (String value : forbidden) {
            assertFalse(text.contains(value), "error must not disclose '" + value + "': " + text);
        }
    }

    private String fingerprintOfStore() throws Exception {
        StringBuilder out = new StringBuilder();
        try (Connection c = DriverManager.getConnection("jdbc:sqlite:" + dataDir.resolve("pwt.db"));
                Statement st = c.createStatement()) {
            for (String table : List.of("repositories", "runs", "scanner_runs", "findings", "run_files",
                    "analysis_threads", "analysis_revisions", "analysis_assessments", "artifacts")) {
                try (ResultSet rs = st.executeQuery("SELECT * FROM " + table + " ORDER BY 1")) {
                    int cols = rs.getMetaData().getColumnCount();
                    out.append(table).append(':');
                    while (rs.next()) {
                        for (int i = 1; i <= cols; i++) {
                            out.append(rs.getString(i)).append('|');
                        }
                        out.append(';');
                    }
                }
            }
        }
        return out.toString();
    }

    @Test
    void projectStatusIsBoundToTheOwningProjectAndNeverFallsBack() throws Exception {
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode status = mcp.callResult("get_project_status", Map.of());
            assertEquals(repoA, status.at("/project/id").asText());
            assertEquals("project-root", status.at("/binding/mode").asText());
            assertEquals(2, status.get("run_count").asInt());
            assertEquals(runA2, status.at("/latest_run/run_id").asText());
            assertTrue(status.at("/latest_run/final").asBoolean());
            assertEquals("denied", status.at("/server_permissions/scans").asText());
            assertEquals("denied", status.at("/server_permissions/analysis_writes").asText());
            // A has no journal: reported missing, never B's journal.
            assertFalse(status.at("/journal/available").asBoolean());
            assertTrue(status.at("/journal/path").asText().contains(repoA));
            assertNoLeak(status, repoB, B_DISPLAY, runB1);
            // A different repository_id on a bound connection is OUT_OF_SCOPE, not disclosed.
            JsonNode cross = mcp.callToolError("get_project_status", McpHarness.params("repository_id", repoB));
            assertEquals("OUT_OF_SCOPE", cross.get("code").asText());
            assertNoLeak(cross, B_DISPLAY, rootB.toString());
        }
    }

    @Test
    void copiedContextPointerCannotRedirectTheJournal() throws Exception {
        Files.createDirectories(rootA.resolve(".pwt"));
        Files.writeString(rootA.resolve(".pwt/context.md"), "<!-- PWT-OWNED -->\n- Repository ID: `" + repoB
                + "`\n- Scan journal: `" + dataDir.resolve("reports/" + B_DISPLAY + "--" + repoB + ".md") + "`\n");
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode status = mcp.callResult("get_project_status", Map.of());
            assertEquals(repoA, status.at("/project/id").asText());
            assertFalse(status.at("/journal/available").asBoolean());
            assertNoLeak(status, repoB, B_DISPLAY);
        }
    }

    @Test
    void historyAndRunDetailAreProjectOnly() throws Exception {
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode runs = mcp.callResult("list_runs", Map.of());
            assertEquals(2, runs.get("total").asInt());
            for (JsonNode run : runs.get("runs")) {
                assertNotEquals(runB1, run.get("run_id").asText());
            }
            JsonNode page = mcp.callResult("list_runs", McpHarness.params("limit", 1));
            assertEquals(1, page.get("runs").size());
            assertEquals("1", page.get("next_cursor").asText());
            assertEquals("INVALID_INPUT",
                    mcp.callToolError("list_runs", McpHarness.params("limit", 0)).get("code").asText());
            assertEquals("INVALID_INPUT",
                    mcp.callToolError("list_runs", McpHarness.params("limit", 501)).get("code").asText());
            assertEquals("INVALID_INPUT",
                    mcp.callToolError("list_runs", McpHarness.params("cursor", -1)).get("code").asText());

            JsonNode run = mcp.callResult("get_run", McpHarness.params("run_id", runA2));
            assertTrue(run.at("/run/final").asBoolean());
            boolean sawTimeout = false;
            for (JsonNode scanner : run.get("scanner_runs")) {
                if (scanner.get("adapter_id").asText().equals("kubeconform")) {
                    assertEquals("TIMED_OUT", scanner.get("status").asText());
                    assertEquals("execution_failure", scanner.get("execution").asText());
                    sawTimeout = true;
                }
                if (scanner.get("adapter_id").asText().equals("checkov")) {
                    assertEquals("not_executed", scanner.get("execution").asText());
                }
            }
            assertTrue(sawTimeout);
            assertFalse(run.at("/coverage/complete").asBoolean());

            JsonNode crossRun = mcp.callToolError("get_run", McpHarness.params("run_id", runB1));
            assertEquals("NOT_FOUND", crossRun.get("code").asText());
            assertNoLeak(crossRun, B_DISPLAY, rootB.toString(), repoB, B_RULE);
            JsonNode missing = mcp.callToolError("get_run", McpHarness.params("run_id", "nope"));
            assertEquals("NOT_FOUND", missing.get("code").asText());
            assertEquals("INVALID_INPUT", mcp.callToolError("get_run",
                    McpHarness.params("run_id", "x".repeat(200))).get("code").asText());
            assertEquals("INVALID_INPUT", mcp.callToolError("get_run", Map.of()).get("code").asText());
        }
    }

    @Test
    void findingFiltersAreBoundedAndProjectOnly() throws Exception {
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode byRule = mcp.callResult("list_findings",
                    McpHarness.params("run_id", runA1, "rule_id", "CKV_K8S_20"));
            assertEquals(1, byRule.get("total").asInt());
            assertEquals("HIGH", byRule.get("findings").get(0).get("severity").asText());
            assertEquals("high", byRule.get("findings").get(0).get("original_severity").asText());
            JsonNode byPath = mcp.callResult("list_findings",
                    McpHarness.params("run_id", runA2, "relative_path", "a.yaml", "severity", "MEDIUM"));
            assertEquals(1, byPath.get("total").asInt());
            JsonNode empty = mcp.callResult("list_findings",
                    McpHarness.params("run_id", runA2, "relative_path", "nothing.yaml"));
            assertEquals(0, empty.get("total").asInt());
            assertTrue(empty.get("findings").isEmpty());
            assertEquals("INVALID_INPUT", mcp.callToolError("list_findings",
                    McpHarness.params("run_id", runA2, "rule_id", "r".repeat(600))).get("code").asText());
            assertEquals("INVALID_INPUT", mcp.callToolError("list_findings",
                    McpHarness.params("run_id", runA2, "relative_path", "a\u0001.yaml")).get("code").asText());
            assertEquals("INVALID_INPUT", mcp.callToolError("list_findings",
                    McpHarness.params("run_id", runA2, "severity", "SUPER")).get("code").asText());
            JsonNode cross = mcp.callToolError("list_findings",
                    McpHarness.params("run_id", runB1, "rule_id", B_RULE));
            assertEquals("NOT_FOUND", cross.get("code").asText());
            assertNoLeak(cross, B_DISPLAY, repoB);
        }
    }

    @Test
    void compareRunsStatesLimitsAndNeverCrossesProjects() throws Exception {
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode diff = mcp.callResult("compare_runs",
                    McpHarness.params("base_run_id", runA1, "head_run_id", runA2));
            JsonNode findings = diff.get("findings");
            // truthy moved from line 3 to 4 after an edit: a candidate shift, not new/removed.
            assertEquals(1, findings.get("possible_line_shifts").size());
            assertEquals("truthy", findings.get("possible_line_shifts").get(0).get("rule_id").asText());
            // checkov finding is absent in head, but checkov was SKIPPED there: not comparable.
            assertEquals(1, findings.get("only_in_base_total").asInt());
            JsonNode gone = findings.get("only_in_base").get(0);
            assertEquals("CKV_K8S_20", gone.get("rule_id").asText());
            assertFalse(gone.get("comparable").asBoolean());
            assertEquals(1, findings.get("only_in_head_total").asInt());
            assertEquals("indentation", findings.get("only_in_head").get(0).get("rule_id").asText());
            assertTrue(findings.get("only_in_head").get(0).get("comparable").asBoolean());
            assertEquals("a.yaml", diff.at("/selected_files/content_changed/0").asText());
            String limits = diff.get("limitations").toString();
            assertTrue(limits.contains("checkov"), limits);
            assertTrue(limits.contains("kubeconform"), limits);
            assertTrue(limits.contains("coverage gaps"), limits);
            assertFalse(diff.toString().toLowerCase().contains("\"fixed"), "never labels anything fixed");

            JsonNode cross = mcp.callToolError("compare_runs",
                    McpHarness.params("base_run_id", runA1, "head_run_id", runB1));
            assertEquals("NOT_FOUND", cross.get("code").asText());
            assertNoLeak(cross, B_DISPLAY, repoB, B_RULE);
            assertEquals("INVALID_INPUT", mcp.callToolError("compare_runs",
                    McpHarness.params("base_run_id", runA1, "head_run_id", runA1)).get("code").asText());
            assertEquals("INVALID_INPUT", mcp.callToolError("compare_runs",
                    McpHarness.params("base_run_id", runA1)).get("code").asText());
        }
    }

    @Test
    void scanAndCancelAreDeniedServerSide() throws Exception {
        try (McpHarness mcp = boundTo(rootA)) {
            JsonNode start = mcp.callToolError("start_scan", McpHarness.params("repository_id", repoA,
                    "relative_paths", List.of("a.yaml"), "adapter_ids", List.of("yamllint")));
            assertEquals("PERMISSION_DENIED", start.get("code").asText());
            JsonNode cancel = mcp.callToolError("cancel_scan", McpHarness.params("run_id", runA2));
            assertEquals("PERMISSION_DENIED", cancel.get("code").asText());
            JsonNode save = mcp.callToolError("save_analysis", McpHarness.params("run_id", runA2,
                    "expected_revision", 0, "summary", "x", "priority", "LOW"));
            assertEquals("PERMISSION_DENIED", save.get("code").asText());
        }
    }

    @Test
    void subdirectorySymlinkAndNestedProjectsResolveCanonically() throws Exception {
        Path nested = Files.createDirectories(rootA.resolve("nested-c"));
        Files.createDirectories(nested.resolve("deep"));
        String repoC;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            repoC = register(store, nested, "charlie");
        }
        Path link = work.resolve("alpha-link");
        Files.createSymbolicLink(link, rootA);
        try (McpHarness sub = boundTo(rootA.resolve("sub/deeper"))) {
            assertEquals(repoA, sub.callResult("get_project_status", Map.of()).at("/project/id").asText());
        }
        try (McpHarness viaLink = boundTo(link.resolve("sub"))) {
            assertEquals(repoA, viaLink.callResult("get_project_status", Map.of()).at("/project/id").asText());
        }
        try (McpHarness inner = boundTo(nested.resolve("deep"))) {
            JsonNode status = inner.callResult("get_project_status", Map.of());
            assertEquals(repoC, status.at("/project/id").asText());
            assertEquals(0, status.get("run_count").asInt());
            assertTrue(status.get("latest_run").isNull());
            // The outer project's runs are not visible from the inner project.
            assertEquals("NOT_FOUND", inner.callToolError("get_run",
                    McpHarness.params("run_id", runA1)).get("code").asText());
        }
        try (McpHarness outer = boundTo(rootA)) {
            assertEquals(repoA, outer.callResult("get_project_status", Map.of()).at("/project/id").asText());
        }
    }

    @Test
    void scopeIsPinnedForTheSessionAndNeverMovesToANewlyRegisteredProject() throws Exception {
        Path inner = Files.createDirectories(rootA.resolve("later-nested"));
        try (McpHarness mcp = boundTo(inner)) {
            assertEquals(repoA, mcp.callResult("get_project_status", Map.of()).at("/project/id").asText());
            try (SqliteStore store = SqliteStore.open(dataDir)) {
                register(store, inner, "registered-mid-session");
            }
            for (String tool : List.of("get_project_status", "list_runs")) {
                JsonNode changed = mcp.callToolError(tool, Map.of());
                assertEquals("PROJECT_SCOPE_CHANGED", changed.get("code").asText());
                assertNoLeak(changed, "registered-mid-session");
            }
            assertEquals("PROJECT_SCOPE_CHANGED", mcp.callToolError("get_run",
                    McpHarness.params("run_id", runA1)).get("code").asText());
        }
        // A new session binds to the now-registered inner project, never the old one.
        try (McpHarness fresh = boundTo(inner)) {
            JsonNode status = fresh.callResult("get_project_status", Map.of());
            assertEquals("registered-mid-session", status.at("/project/display_name").asText());
            assertEquals(0, status.get("run_count").asInt());
        }
    }

    @Test
    void unregisteredAndMissingProjectsFailClosedWithoutRegistering() throws Exception {
        Path stranger = Files.createDirectories(work.resolve("unregistered-project"));
        int before;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            before = store.listRepositories().size();
        }
        try (McpHarness mcp = boundTo(stranger)) {
            for (String tool : List.of("get_project_status", "list_runs", "list_repositories")) {
                JsonNode response = mcp.call(tool, Map.of());
                JsonNode text = new com.fasterxml.jackson.databind.ObjectMapper().readTree(
                        response.at("/result/content/0/text").asText());
                assertTrue(response.at("/result/isError").asBoolean(), tool + " must fail closed: " + response);
                assertEquals("PROJECT_NOT_REGISTERED", text.get("code").asText());
                assertTrue(text.get("message").asText().contains("first scan"));
                assertNoLeak(text, "alpha", B_DISPLAY, repoA, repoB);
            }
            JsonNode run = mcp.callToolError("get_run", McpHarness.params("run_id", runA1));
            assertEquals("PROJECT_NOT_REGISTERED", run.get("code").asText());
            // Registration by a later dashboard scan is picked up without restarting the client.
            try (SqliteStore store = SqliteStore.open(dataDir)) {
                register(store, stranger, "unregistered-project");
            }
            JsonNode status = mcp.callResult("get_project_status", Map.of());
            assertEquals("unregistered-project", status.at("/project/display_name").asText());
        }
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals(before + 1, store.listRepositories().size(), "only the explicit registration added a row");
        }
        Path gone = work.resolve("deleted-project");
        try (McpHarness mcp = boundTo(gone)) {
            JsonNode error = mcp.callToolError("get_project_status", Map.of());
            assertEquals("PROJECT_UNAVAILABLE", error.get("code").asText());
            assertNoLeak(error, "alpha", B_DISPLAY);
        }
    }

    @Test
    void readCallsDoNotMutateAssessmentDataOrCreateDatabases() throws Exception {
        String before = fingerprintOfStore();
        try (McpHarness mcp = boundTo(rootA)) {
            mcp.callResult("get_project_status", Map.of());
            mcp.callResult("list_repositories", Map.of());
            mcp.callResult("list_runs", Map.of());
            mcp.callResult("get_run", McpHarness.params("run_id", runA1));
            mcp.callResult("list_findings", McpHarness.params("run_id", runA1));
            mcp.callResult("compare_runs", McpHarness.params("base_run_id", runA1, "head_run_id", runA2));
            mcp.callResult("get_report", McpHarness.params("run_id", runA1, "format", "markdown"));
            mcp.callResult("get_analysis", McpHarness.params("run_id", runA1));
            String stderr = mcp.stderrText();
            assertFalse(stderr.contains(B_DISPLAY), "diagnostics must not mention other projects");
        }
        assertEquals(before, fingerprintOfStore(), "read-only tools must not change assessment tables");
        List<Path> databases = new ArrayList<>();
        try (Stream<Path> walk = Files.walk(work)) {
            walk.filter(p -> p.getFileName().toString().endsWith(".db")).forEach(databases::add);
        }
        assertEquals(List.of(dataDir.resolve("pwt.db")), databases, "exactly one shared database");
    }

    @Test
    void repoIdAndProjectRootAreMutuallyExclusiveAndRootMustBeAbsolute() throws Exception {
        String javaBin = System.getProperty("java.home") + File.separator + "bin" + File.separator + "java";
        for (List<String> extra : List.of(
                List.of("--repo-id", repoA, "--project-root", rootA.toString()),
                List.of("--project-root", "relative/dir"))) {
            List<String> command = new ArrayList<>(List.of(javaBin, "-cp", System.getProperty("java.class.path"),
                    "pwt.launcher.PwtMain", "mcp", "--data-dir", dataDir.toString()));
            command.addAll(extra);
            Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
            assertTrue(process.waitFor(30, TimeUnit.SECONDS));
            String output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertEquals(2, process.exitValue(), output);
        }
    }
}

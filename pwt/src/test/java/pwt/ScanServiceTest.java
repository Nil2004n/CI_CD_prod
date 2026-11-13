package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.application.CancellationToken;
import pwt.application.ScanConfig;
import pwt.application.ScanException;
import pwt.application.ScanService;
import pwt.domain.RepositoryRoot;
import pwt.domain.ScanState;
import pwt.domain.ScanVerdict;
import pwt.domain.ScanSummary;
import pwt.domain.ScannerStatus;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;

/**
 * End-to-end scans through the real installed tools (yamllint 1.38.0,
 * Gitleaks 8.30.1). Tool-dependent tests assume the executable probes
 * succeed, so a missing tool skips explicitly instead of failing.
 * All secrets used are benign fixtures, never real credentials.
 */
class ScanServiceTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final String FAKE_TOKEN = "ghp_X7sK9mQ2vB4nL8wZ1pD6fH3jR5tY0cV1eA9b";

    @TempDir
    Path work;

    private Path repoDir(String name) throws Exception {
        Path repo = work.resolve(name);
        Files.createDirectories(repo);
        return repo;
    }

    private void write(Path repo, String relative, String content) throws Exception {
        Path file = repo.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
    }

    private static void assumeToolsPresent() {
        assumeTrue(new YamllintAdapter().probe().available(),
                "SKIP: yamllint not available on PATH");
        assumeTrue(new GitleaksAdapter().probe().available(),
                "SKIP: gitleaks not available on PATH");
    }

    private static ScanService service() {
        return new ScanService(ScanConfig.defaults());
    }

    @Test
    void cleanScanCompletesWithoutFindings() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("clean");
        write(repo, "ok.yaml", "---\nkey: value\n");
        write(repo, "sub/ok2.yml", "---\nother: 1\n");
        Path dataDir = work.resolve("data");

        ScanSummary summary = service().runScan(repo, List.of("ok.yaml", "sub/ok2.yml"),
                Set.of("yamllint", "gitleaks"), "CLI", dataDir, CancellationToken.none(), null);

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.NO_FINDINGS, summary.verdict());
        assertEquals(0, summary.findingCount());
        assertEquals(2, summary.targets().size());
        assertEquals(2, summary.scannerOutcomes().size());
        for (var outcome : summary.scannerOutcomes()) {
            assertEquals(ScannerStatus.SUCCEEDED, outcome.status(), outcome.adapterId());
            assertNotNull(outcome.toolVersion());
        }
        assertNotNull(summary.jsonReportPath());
        assertNotNull(summary.markdownReportPath());
        assertTrue(Files.exists(dataDir.resolve(summary.jsonReportPath())));
        assertTrue(Files.exists(dataDir.resolve(summary.markdownReportPath())));
        // Transient snapshot must be gone after a finished run.
        assertTrue(!Files.exists(dataDir.resolve("transient").resolve(summary.runId())));
        // yamllint records its real version.
        String yamllintVersion = summary.scannerOutcomes().stream()
                .filter(o -> o.adapterId().equals("yamllint")).findFirst().orElseThrow().toolVersion();
        assertTrue(yamllintVersion.matches("\\d+\\.\\d+.*"), yamllintVersion);
    }

    @Test
    void lintFailureProducesFindingsOnlyForSelectedFiles() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("lint");
        write(repo, "bad.yaml", "key: [unclosed\n  bad indent loooooooooooooooooooooooooooooooooooooooooooooooooooong\n");
        write(repo, "unselected.yaml", "also: [broken\n");
        Path dataDir = work.resolve("data");

        ScanSummary summary = service().runScan(repo, List.of("bad.yaml"), Set.of("yamllint"),
                "CLI", dataDir, CancellationToken.none(), null);

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        assertTrue(summary.findingCount() > 0);
        assertEquals(List.of("bad.yaml"), summary.targets().stream()
                .map(ScanSummary.SelectedTarget::relativePath).toList());
        for (var outcome : summary.scannerOutcomes()) {
            for (var finding : outcome.findings()) {
                assertEquals("bad.yaml", finding.relativePath(),
                        "unselected files must never become targets");
            }
        }
    }

    @Test
    void seededSecretDetectedAndNeverPersisted() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("secret");
        write(repo, "clean.yaml", "---\nkey: value\n");
        write(repo, "leak.yaml", "---\ntoken: \"" + FAKE_TOKEN + "\"\n");
        Path dataDir = work.resolve("data");

        ScanSummary summary = service().runScan(repo, List.of("clean.yaml", "leak.yaml"),
                Set.of(), "CLI", dataDir, CancellationToken.none(), null);

        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        var gitleaks = summary.scannerOutcomes().stream()
                .filter(o -> o.adapterId().equals("gitleaks")).findFirst().orElseThrow();
        assertEquals(ScannerStatus.SUCCEEDED, gitleaks.status());
        assertTrue(gitleaks.findings().stream().anyMatch(f -> f.relativePath().equals("leak.yaml")));

        // Secret hygiene across every persisted sink.
        String jsonReport = Files.readString(dataDir.resolve(summary.jsonReportPath()));
        String mdReport = Files.readString(dataDir.resolve(summary.markdownReportPath()));
        assertTrue(!jsonReport.contains(FAKE_TOKEN), "JSON report must not contain the secret");
        assertTrue(!mdReport.contains(FAKE_TOKEN), "Markdown report must not contain the secret");
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            for (var row : store.listFindings(summary.runId())) {
                assertTrue(!row.finding().message().contains(FAKE_TOKEN));
                assertTrue(!row.finding().evidenceJson().contains(FAKE_TOKEN));
                assertTrue(!row.finding().evidenceJson().contains("Secret"));
            }
            for (var scanner : store.listScannerRuns(summary.runId())) {
                if (scanner.message() != null) {
                    assertTrue(!scanner.message().contains(FAKE_TOKEN));
                }
            }
        }
        // The JSON report validates against the published schema.
        validateAgainstSchema(jsonReport);
    }

    @Test
    void generatedJsonValidatesAgainstReportSchema() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("schema");
        write(repo, "ok.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        ScanSummary summary = service().runScan(repo, List.of("ok.yaml"),
                Set.of("yamllint", "gitleaks"), "CLI", dataDir, CancellationToken.none(), null);
        String jsonReport = Files.readString(dataDir.resolve(summary.jsonReportPath()));
        validateAgainstSchema(jsonReport);
        JsonNode root = MAPPER.readTree(jsonReport);
        assertEquals("1.0", root.get("schema_version").asText());
        assertEquals("NO_FINDINGS", root.get("run").get("verdict").asText());
        assertEquals(summary.runId(), root.get("run").get("id").asText());
        assertTrue(root.get("scope_note").asText().contains("not a repository-wide verdict"));
        assertTrue(root.get("scope_note").asText().contains("only the checks selected for"));
    }

    private static void validateAgainstSchema(String jsonReport) throws Exception {
        String schemaText = new String(ScanServiceTest.class
                .getResourceAsStream("/pwt/schema/report-1.0.json").readAllBytes(), StandardCharsets.UTF_8);
        com.networknt.schema.JsonSchema schema = com.networknt.schema.JsonSchemaFactory
                .getInstance(com.networknt.schema.SpecVersion.VersionFlag.V202012)
                .getSchema(schemaText);
        var errors = schema.validate(MAPPER.readTree(jsonReport));
        assertTrue(errors.isEmpty(), "report must validate against report-1.0.json: " + errors);
    }

    @Test
    void busyRepositoryRejected() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("busy");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        Files.createDirectories(dataDir);
        Path canonical = repo.toRealPath();
        String hash = RepositoryRoot.identityHashFor(canonical);
        try (RepoLock ignored = RepoLock.tryAcquire(dataDir, hash).orElseThrow()) {
            ScanException e = assertThrows(ScanException.class, () -> service().runScan(
                    repo, List.of("a.yaml"), Set.of(), "CLI", dataDir, CancellationToken.none(), null));
            assertEquals(ScanException.Code.REPOSITORY_BUSY, e.code());
        }
    }

    @Test
    void defaultAdapterSetIsHonestAboutUnconfiguredChecks() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("defaults");
        write(repo, "ok.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        // Default = all six adapters. yamllint+gitleaks succeed; checkov,
        // actionlint and kubeconform find nothing applicable; local-schema has
        // no bindings. The result must be explicit, never silently clean.
        ScanSummary summary = service().runScan(repo, List.of("ok.yaml"), Set.of(),
                "CLI", dataDir, CancellationToken.none(), null);
        assertEquals(ScanVerdict.INDETERMINATE, summary.verdict());
        assertTrue(summary.scannerOutcomes().stream()
                .anyMatch(o -> o.status() == ScannerStatus.NOT_APPLICABLE));
        assertTrue(summary.scannerOutcomes().stream()
                .anyMatch(o -> o.status() == ScannerStatus.SKIPPED));
        assertTrue(!summary.coverageGaps().isEmpty());
    }

    @Test
    void invalidInputsRejectedBeforeAnyRun() throws Exception {
        Path repo = repoDir("invalid");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        assertEquals(ScanException.Code.INVALID_INPUT, assertThrows(ScanException.class,
                () -> service().runScan(repo, List.of(), Set.of(), "CLI", dataDir,
                        CancellationToken.none(), null)).code());
        assertEquals(ScanException.Code.INVALID_INPUT, assertThrows(ScanException.class,
                () -> service().runScan(repo, List.of("a.yaml"), Set.of("not-a-scanner"), "CLI", dataDir,
                        CancellationToken.none(), null)).code());
        assertEquals(ScanException.Code.INVALID_INPUT, assertThrows(ScanException.class,
                () -> service().runScan(repo, List.of("../escape.yaml"), Set.of(), "CLI", dataDir,
                        CancellationToken.none(), null)).code());
        assertEquals(ScanException.Code.INVALID_INPUT, assertThrows(ScanException.class,
                () -> service().runScan(work.resolve("missing"), List.of("a.yaml"), Set.of(), "CLI", dataDir,
                        CancellationToken.none(), null)).code());

        // No run rows may exist after purely invalid input.
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertTrue(store.listNonTerminalRuns().isEmpty());
        } catch (Exception noDb) {
            // acceptable: the store may never have been created
        }
    }

    @Test
    void missingToolBecomesUnavailableOutcomeNotSilentSuccess() throws Exception {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint not available on PATH");
        Path repo = repoDir("missing-tool");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        System.setProperty("pwt.tools.gitleaks", "/nonexistent-gitleaks-binary-xyz");
        try {
            ScanSummary summary = service().runScan(repo, List.of("a.yaml"), Set.of(),
                    "CLI", dataDir, CancellationToken.none(), null);
            assertEquals(ScanState.PARTIAL, summary.state());
            assertEquals(ScanVerdict.INDETERMINATE, summary.verdict());
            var gitleaks = summary.scannerOutcomes().stream()
                    .filter(o -> o.adapterId().equals("gitleaks")).findFirst().orElseThrow();
            assertEquals(ScannerStatus.UNAVAILABLE, gitleaks.status());
            assertTrue(summary.coverageGaps().stream().anyMatch(g -> g.contains("gitleaks")));
        } finally {
            System.clearProperty("pwt.tools.gitleaks");
        }
    }

    @Test
    void timedOutToolRecordedWithoutHanging() throws Exception {
        Path repo = repoDir("timeout");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        Path sleeper = work.resolve("sleeper.sh");
        Files.writeString(sleeper, "#!/bin/sh\nsleep 60\n");
        sleeper.toFile().setExecutable(true);
        System.setProperty("pwt.tools.yamllint", sleeper.toString());
        try {
            ScanService fast = new ScanService(new ScanConfig(Duration.ofSeconds(2),
                    ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                    ScanConfig.KubeconformConfig.defaults(), null));
            ScanSummary summary = fast.runScan(repo, List.of("a.yaml"), Set.of("yamllint"),
                    "CLI", dataDir, CancellationToken.none(), null);
            var yamllint = summary.scannerOutcomes().get(0);
            assertEquals(ScannerStatus.TIMED_OUT, yamllint.status());
            assertEquals(ScanState.FAILED, summary.state());
        } finally {
            System.clearProperty("pwt.tools.yamllint");
        }
    }

    @Test
    void preCancelledTokenAbortsRun() throws Exception {
        Path repo = repoDir("cancel");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        CancellationToken cancelled = new CancellationToken();
        cancelled.cancel();
        ScanSummary summary = service().runScan(repo, List.of("a.yaml"), Set.of(),
                "CLI", dataDir, cancelled, null);
        assertEquals(ScanState.CANCELLED, summary.state());
        assertEquals(ScanVerdict.INDETERMINATE, summary.verdict());
    }

    @Test
    void interruptedRunReconciledOnStartup() throws Exception {
        Path repo = repoDir("reconcile");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        Path canonical = repo.toRealPath();
        SqliteStore.RepositoryRow repoRow;
        String orphanRun;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            repoRow = store.registerRepository(canonical.toString(), "reconcile",
                    RepositoryRoot.identityHashFor(canonical));
            orphanRun = store.createRun(repoRow.id(), "0.2.0", "{}");
            store.startRun(orphanRun);
        }
        ScanService.reconcileInterrupted(dataDir);
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals(ScanState.INTERRUPTED, store.getRun(orphanRun).state());
        }
        Optional<RepoLock> free = RepoLock.tryAcquire(dataDir, repoRow.identityHash());
        assertTrue(free.isPresent(), "reconcile must release the lock it acquired");
        free.get().close();
    }
}

package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.ScanState;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.infrastructure.SqliteStore;

class SqliteStoreTest {

    @TempDir
    Path dataDir;

    private static Finding sampleFinding() {
        return new Finding("gitleaks/github-pat", "desc", Severity.HIGH, "leak",
                FindingCategory.SECRET, "a.yaml", 2, 2, 10, "desc at a.yaml:2",
                "{\"entropy\":5.1}", "fingerprint-1");
    }

    @Test
    void runLifecyclePersistsAndReloads() throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow repo = store.registerRepository("/repo", "repo", "hash1");
            SqliteStore.RepositoryRow same = store.registerRepository("/repo", "repo", "hash1");
            assertEquals(repo.id(), same.id(), "re-registration must be idempotent");

            String runId = store.createRun(repo.id(), "0.2.0", "{}");
            store.startRun(runId);
            store.addRunFile(runId, "a.yaml", "ab".repeat(32), 12);
            String scannerRunId = store.recordScannerRun(runId, "yamllint", "yamllint", "1.38.0",
                    ScannerStatus.SUCCEEDED, 0, "ok");
            store.addFindings(runId, scannerRunId, List.of(sampleFinding()));
            store.finalizeRun(runId, ScanState.COMPLETED, ScanVerdict.FINDINGS, "{\"gaps\":[]}", null, null);
            store.recordArtifact(runId, "JSON_REPORT", "READY", "artifacts/" + runId + "/report.json",
                    "cd".repeat(32), 100L, null);

            SqliteStore.RunView view = store.getRun(runId);
            assertNotNull(view);
            assertEquals(ScanState.COMPLETED, view.state());
            assertEquals(ScanVerdict.FINDINGS, view.verdict());

            assertEquals(1, store.listRunFiles(runId).size());
            assertEquals("a.yaml", store.listRunFiles(runId).get(0).relativePath());
            assertEquals(1, store.listScannerRuns(runId).size());
            assertEquals(ScannerStatus.SUCCEEDED, store.listScannerRuns(runId).get(0).status());
            List<SqliteStore.FindingRow> findings = store.listFindings(runId);
            assertEquals(1, findings.size());
            assertEquals("gitleaks/github-pat", findings.get(0).finding().ruleId());
            assertEquals("{\"entropy\":5.1}", findings.get(0).finding().evidenceJson());
            assertTrue(store.listNonTerminalRuns().isEmpty(), "terminal runs must not list as non-terminal");
        }
    }

    @Test
    void nonTerminalRunsListedForReconcile() throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow repo = store.registerRepository("/repo", "repo", "hash2");
            String runId = store.createRun(repo.id(), "0.2.0", "{}");
            store.startRun(runId);
            assertEquals(1, store.listNonTerminalRuns().size());
            assertEquals(runId, store.listNonTerminalRuns().get(0).id());
        }
    }

    @Test
    void schemaVersionGuarded() throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.registerRepository("/repo", "repo", "hash3");
        }
        // Reopening the same database must succeed (idempotent migrations),
        // and unknown run IDs resolve to null rather than throwing.
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals(null, store.getRun("missing"));
        }
    }
}

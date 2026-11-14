package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.application.CancellationToken;
import pwt.application.JournalWriter;
import pwt.application.ScanConfig;
import pwt.application.ScanService;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.ScanState;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;
import pwt.launcher.JournalCommand;
import pwt.launcher.RepoCommand;

/**
 * Per-repository Markdown journals. All secrets used are benign fixtures,
 * never real credentials.
 */
class JournalWriterTest {

    private static final String FAKE_TOKEN = "glpat-X9q2mK7vRt4wZ8nB3cD6fH1jL5sA0eP7oI2uY6";

    @TempDir
    Path work;

    private static void assumeToolsPresent() {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
    }

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

    private static String readJournal(Path dataDir, SqliteStore.RepositoryRow repo) throws Exception {
        return Files.readString(JournalWriter.journalPath(dataDir, repo), StandardCharsets.UTF_8);
    }

    private static int countOccurrences(String text, String marker) {
        int count = 0;
        int at = 0;
        while ((at = text.indexOf(marker, at)) >= 0) {
            count++;
            at += marker.length();
        }
        return count;
    }

    private SqliteStore.RepositoryRow register(Path dataDir, Path root, String name) throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            return store.registerRepository(root.toString(), name, "identity-" + name);
        }
    }

    private String craftTerminalRun(Path dataDir, SqliteStore.RepositoryRow repo, String state,
            String verdict, String errorMessage) throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            String runId = store.createRun(repo.id(), "test", "{\"finding_count\":1,\"target_count\":1,\"gaps\":[]}");
            store.startRun(runId);
            store.addRunFile(runId, "a.yaml", "abc123", 12);
            String scannerRun = store.recordScannerRun(runId, "yamllint", "yamllint", "1.38.0",
                    ScannerStatus.SUCCEEDED, 0, "ok");
            store.addFindings(runId, scannerRun, List.of(new Finding("yamllint/test", "t",
                    Severity.INFO, "warning", FindingCategory.LINT, "a.yaml", 1, 1, 1, "m", "{}", "fp-1")));
            String summary = "{\"finding_count\":1}";
            store.finalizeRun(runId, ScanState.valueOf(state), ScanVerdict.valueOf(verdict),
                    summary, null, errorMessage);
            return runId;
        }
    }

    @Test
    void firstScanCreatesJournalWithCompleteSection() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("one");
        write(repo, "ok.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        serviceRun(repo, List.of("ok.yaml"), dataDir);

        SqliteStore.RepositoryRow row;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            row = store.listRepositories().get(0);
            String runId = store.listRuns(row.id(), null, 10, 0).get(0).id();
            String journal = readJournal(dataDir, row);
            assertTrue(journal.contains("<!-- pwt:run " + runId + " -->"));
            assertTrue(journal.contains("<!-- /pwt:run " + runId + " -->"));
            assertTrue(journal.contains("COMPLETED"));
            assertTrue(journal.contains("ok.yaml"));
            assertTrue(journal.contains("yamllint"));
            assertEquals(1, countOccurrences(journal, "<!-- pwt:run "));
        }
    }

    @Test
    void secondScanAppendsWithoutLosingFirst() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("two");
        write(repo, "a.yaml", "---\nkey: value\n");
        write(repo, "b.yaml", "broken: [unclosed\n");
        Path dataDir = work.resolve("data");

        serviceRun(repo, List.of("a.yaml"), dataDir);
        serviceRun(repo, List.of("b.yaml"), dataDir);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.listRepositories().get(0);
            String journal = readJournal(dataDir, row);
            assertEquals(2, store.listRuns(row.id(), null, 10, 0).size());
            assertEquals(2, countOccurrences(journal, "<!-- pwt:run "));
            assertEquals(2, countOccurrences(journal, "<!-- /pwt:run "));
            assertTrue(journal.indexOf("<!-- pwt:run ") < journal.lastIndexOf("<!-- pwt:run "));
        }
    }

    @Test
    void twoRepositoriesGetSeparateJournals() throws Exception {
        assumeToolsPresent();
        Path repoA = repoDir("repoA");
        write(repoA, "a.yaml", "---\nkey: value\n");
        Path repoB = repoDir("repoB");
        write(repoB, "b.yaml", "---\nother: 1\n");
        Path dataDir = work.resolve("data");

        serviceRun(repoA, List.of("a.yaml"), dataDir);
        serviceRun(repoB, List.of("b.yaml"), dataDir);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals(2, store.listRepositories().size());
            for (SqliteStore.RepositoryRow row : store.listRepositories()) {
                String journal = readJournal(dataDir, row);
                assertTrue(journal.contains(row.id()));
                assertEquals(1, countOccurrences(journal, "<!-- pwt:run "));
            }
            SqliteStore.RepositoryRow a = store.listRepositories().stream()
                    .filter(r -> r.displayName().equals("repoA")).findFirst().orElseThrow();
            SqliteStore.RepositoryRow b = store.listRepositories().stream()
                    .filter(r -> r.displayName().equals("repoB")).findFirst().orElseThrow();
            assertTrue(!JournalWriter.journalPath(dataDir, a)
                    .equals(JournalWriter.journalPath(dataDir, b)));
        }
    }

    @Test
    void duplicatePublishIsIdempotent() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("idem");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        serviceRun(repo, List.of("a.yaml"), dataDir);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.listRepositories().get(0);
            String runId = store.listRuns(row.id(), null, 10, 0).get(0).id();
            assertEquals("current", JournalWriter.appendRun(store, dataDir, runId));
            String journal = readJournal(dataDir, row);
            assertEquals(1, countOccurrences(journal, "<!-- pwt:run " + runId + " -->"));
        }
    }

    @Test
    void partialFailedCancelledRunsAreRepresented() throws Exception {
        Path dataDir = work.resolve("data");
        Path repo = repoDir("states");
        SqliteStore.RepositoryRow row = register(dataDir, repo, "states");

        String partial = craftTerminalRun(dataDir, row, "PARTIAL", "INDETERMINATE", null);
        String failed = craftTerminalRun(dataDir, row, "FAILED", "INDETERMINATE", "boom");
        String cancelled = craftTerminalRun(dataDir, row, "CANCELLED", "INDETERMINATE", null);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals("appended", JournalWriter.appendRun(store, dataDir, partial));
            assertEquals("appended", JournalWriter.appendRun(store, dataDir, failed));
            assertEquals("appended", JournalWriter.appendRun(store, dataDir, cancelled));
            String journal = readJournal(dataDir, row);
            assertTrue(journal.contains("PARTIAL"));
            assertTrue(journal.contains("FAILED"));
            assertTrue(journal.contains("CANCELLED"));
            assertTrue(journal.contains("boom"));
            assertEquals(3, countOccurrences(journal, "<!-- pwt:run "));
            assertEquals(3, countOccurrences(journal, "<!-- /pwt:run "));
        }
    }

    @Test
    void nonTerminalRunIsSkipped() throws Exception {
        Path dataDir = work.resolve("data");
        Path repo = repoDir("running");
        SqliteStore.RepositoryRow row = register(dataDir, repo, "running");
        String runId;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            runId = store.createRun(row.id(), "test", "{}");
            boolean thrown = false;
            try {
                JournalWriter.appendRun(store, dataDir, runId);
            } catch (JournalWriter.JournalException e) {
                thrown = true;
            }
            assertTrue(thrown, "non-terminal run must be rejected");
        }
    }

    @Test
    void concurrentPublicationDoesNotInterleave() throws Exception {
        Path dataDir = work.resolve("data");
        Path repo = repoDir("conc");
        SqliteStore.RepositoryRow row = register(dataDir, repo, "conc");
        String first = craftTerminalRun(dataDir, row, "COMPLETED", "FINDINGS", null);
        String second = craftTerminalRun(dataDir, row, "COMPLETED", "NO_FINDINGS", null);

        ExecutorService pool = Executors.newFixedThreadPool(2);
        try {
            CountDownLatch ready = new CountDownLatch(2);
            CountDownLatch go = new CountDownLatch(1);
            List<Future<?>> futures = new ArrayList<>();
            for (String runId : List.of(first, second)) {
                futures.add(pool.submit(() -> {
                    ready.countDown();
                    if (!go.await(30, TimeUnit.SECONDS)) {
                        throw new IllegalStateException("test setup timeout");
                    }
                    // Production callers hold the repo lock across the whole
                    // append; retry acquisition here to model ordered access.
                    RepoLock held = null;
                    for (int attempt = 0; attempt < 200 && held == null; attempt++) {
                        try (SqliteStore store = SqliteStore.open(dataDir)) {
                            var acquired = RepoLock.tryAcquire(dataDir, "identity-conc");
                            if (acquired.isPresent()) {
                                try {
                                    JournalWriter.appendRun(store, dataDir, runId);
                                } finally {
                                    acquired.get().close();
                                }
                                held = acquired.get();
                            }
                        }
                        if (held == null) {
                            Thread.sleep(25);
                        }
                    }
                    if (held == null) {
                        throw new IllegalStateException("could not acquire test lock");
                    }
                    return null;
                }));
            }
            ready.await(30, TimeUnit.SECONDS);
            go.countDown();
            for (Future<?> f : futures) {
                f.get(60, TimeUnit.SECONDS);
            }
        } finally {
            pool.shutdownNow();
        }
        String journal = readJournal(dataDir, row);
        for (String runId : List.of(first, second)) {
            String start = "<!-- pwt:run " + runId + " -->";
            String end = "<!-- /pwt:run " + runId + " -->";
            assertEquals(1, countOccurrences(journal, start));
            assertTrue(journal.indexOf(start) < journal.indexOf(end));
        }
    }

    @Test
    void incompleteTailIsRepaired() throws Exception {
        Path dataDir = work.resolve("data");
        Path repo = repoDir("repair");
        SqliteStore.RepositoryRow row = register(dataDir, repo, "repair");
        String first = craftTerminalRun(dataDir, row, "COMPLETED", "NO_FINDINGS", null);
        String second = craftTerminalRun(dataDir, row, "COMPLETED", "NO_FINDINGS", null);
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertEquals("appended", JournalWriter.appendRun(store, dataDir, first));
            Path journal = JournalWriter.journalPath(dataDir, row);
            String broken = Files.readString(journal, StandardCharsets.UTF_8)
                    + "<!-- pwt:run " + second + " -->\n## truncated\n";
            Files.writeString(journal, broken, StandardCharsets.UTF_8);
            assertEquals("appended", JournalWriter.appendRun(store, dataDir, second));
            String fixed = Files.readString(journal, StandardCharsets.UTF_8);
            assertTrue(fixed.contains("<!-- /pwt:run " + second + " -->"));
            assertEquals(0, countOccurrences(fixed, "## truncated"), "broken tail must be repaired away");
            assertEquals(2, countOccurrences(fixed, "<!-- pwt:run "));
        }
    }

    @Test
    void foreignContentIsRefused() throws Exception {
        Path dataDir = work.resolve("data");
        Path repo = repoDir("foreign");
        SqliteStore.RepositoryRow row = register(dataDir, repo, "foreign");
        String runId = craftTerminalRun(dataDir, row, "COMPLETED", "NO_FINDINGS", null);
        Path journal = JournalWriter.journalPath(dataDir, row);
        Files.createDirectories(journal.getParent());
        Files.writeString(journal, "# someone else's notes\n", StandardCharsets.UTF_8);
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            boolean thrown = false;
            try {
                JournalWriter.appendRun(store, dataDir, runId);
            } catch (JournalWriter.JournalException e) {
                thrown = true;
            }
            assertTrue(thrown, "foreign content must be refused");
            assertEquals("# someone else's notes\n",
                    Files.readString(journal, StandardCharsets.UTF_8));
        }
    }

    @Test
    void journalErrorPreservesScanEvidence() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("errdir");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        Files.createDirectories(dataDir.resolve("reports"));
        // Sabotage: reports path is a file, not a directory.
        deleteRecursively(dataDir.resolve("reports"));
        Files.write(dataDir.resolve("reports"), "not-a-directory".getBytes(StandardCharsets.UTF_8));

        serviceRun(repo, List.of("a.yaml"), dataDir);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.listRepositories().get(0);
            assertEquals(1, store.listRuns(row.id(), null, 10, 0).size());
            assertEquals("COMPLETED", store.listRuns(row.id(), null, 10, 0).get(0).state().name());
        }
    }

    @Test
    void secretsNeverReachTheJournal() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("leak");
        write(repo, "leak.yaml", "---\nkey: value\ntoken: " + FAKE_TOKEN + "\n");
        Path dataDir = work.resolve("data");

        serviceRun(repo, List.of("leak.yaml"), dataDir);

        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.listRepositories().get(0);
            String journal = readJournal(dataDir, row);
            assertFalse(journal.contains(FAKE_TOKEN), "journal must not contain the seeded secret");
            assertTrue(journal.contains("FINDINGS") || journal.contains("findings"));
        }
    }

    @Test
    void journalCommandBackfillsAndReportsCurrency() throws Exception {
        assumeToolsPresent();
        Path repo = repoDir("backfill");
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");
        serviceRun(repo, List.of("a.yaml"), dataDir);

        SqliteStore.RepositoryRow row;
        String runId;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            row = store.listRepositories().get(0);
            runId = store.listRuns(row.id(), null, 10, 0).get(0).id();
        }
        Files.delete(JournalWriter.journalPath(dataDir, row));

        String checkMissing = captureStdout(() -> JournalCommand.execute(List.of(
                "--root", repo.toString(), "--data-dir", dataDir.toString(), "--check")));
        assertTrue(checkMissing.contains("\"status\":\"missing\""), checkMissing);

        String filled = captureStdout(() -> JournalCommand.execute(List.of(
                "--root", repo.toString(), "--data-dir", dataDir.toString())));
        assertTrue(filled.contains("\"status\":\"appended\""), filled);

        String checkCurrent = captureStdout(() -> JournalCommand.execute(List.of(
                "--root", repo.toString(), "--data-dir", dataDir.toString(), "--check")));
        assertTrue(checkCurrent.contains("\"status\":\"current\""), checkCurrent);

        String single = captureStdout(() -> JournalCommand.execute(List.of(
                "--root", repo.toString(), "--run-id", runId,
                "--data-dir", dataDir.toString())));
        assertTrue(single.contains(runId), single);
        assertEquals(2, JournalCommand.execute(List.of(
                "--root", repo.toString(), "--run-id", "no-such-run",
                "--data-dir", dataDir.toString())));
    }

    @Test
    void repoAddWritesContextPointerAndPreservesForeignFile() throws Exception {
        Path repo = repoDir("ctx");
        Path dataDir = work.resolve("data");
        String out = captureStdout(() -> RepoCommand.add(List.of(
                "--root", repo.toString(), "--data-dir", dataDir.toString())));
        assertTrue(out.contains("\"display_name\""), out);
        Path pointer = repo.resolve(".pwt/context.md");
        assertTrue(Files.isRegularFile(pointer), ".pwt/context.md must be created");
        String content = Files.readString(pointer, StandardCharsets.UTF_8);
        assertTrue(content.contains("PWT-OWNED"));
        assertTrue(content.contains((dataDir.resolve("reports")).toString())
                || content.contains("reports"));

        // Unrelated files elsewhere are untouched; now test the marker guard:
        Files.writeString(pointer, "# my own notes\n", StandardCharsets.UTF_8);
        captureStdout(() -> RepoCommand.add(List.of(
                "--root", repo.toString(), "--data-dir", dataDir.toString())));
        assertEquals("# my own notes\n", Files.readString(pointer, StandardCharsets.UTF_8));
    }

    private void serviceRun(Path repo, List<String> files, Path dataDir) throws Exception {
        pwt.application.ScanService service =
                new pwt.application.ScanService(pwt.application.ScanConfig.defaults());
        service.runScan(repo, files, Set.of("yamllint", "gitleaks"), "CLI", dataDir,
                CancellationToken.none(), null);
    }

    private static String captureStdout(ThrowingIntSupplier runnable) throws Exception {
        java.io.PrintStream original = System.out;
        ByteArrayOutputStream captured = new ByteArrayOutputStream();
        System.setOut(new PrintStream(captured, true, StandardCharsets.UTF_8));
        try {
            int code = runnable.run();
            assertEquals(0, code);
        } finally {
            System.setOut(original);
        }
        return captured.toString(StandardCharsets.UTF_8).trim();
    }

    @FunctionalInterface
    interface ThrowingIntSupplier {
        int run() throws Exception;
    }

    private static void deleteRecursively(Path path) throws Exception {
        if (!Files.exists(path)) {
            return;
        }
        List<Path> walked;
        try (var stream = Files.walk(path)) {
            walked = stream.sorted(java.util.Comparator.reverseOrder()).toList();
        }
        for (Path p : walked) {
            Files.deleteIfExists(p);
        }
    }
}

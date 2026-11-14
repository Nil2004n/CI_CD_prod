package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.application.ScanException;
import pwt.launcher.ScanCommand;

/**
 * Headless scan command: argument validation plus real end-to-end exit codes.
 * The secret-hygiene case runs the packaged-equivalent main class in a
 * subprocess and asserts the seeded fixture secret appears neither on stdout
 * nor in application logs on stderr.
 */
class ScanCommandTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final String FAKE_TOKEN = "ghp_Q2vB8nL4wZ6pD1fH7jR3tY9cV0eA5bN8xU2m";

    @TempDir
    Path work;

    private void write(Path repo, String relative, String content) throws Exception {
        Path file = repo.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
    }

    @Test
    void missingRootAndFilesRejected() {
        assertThrowsScanException(() -> ScanCommand.parseArgs(List.of()));
        assertThrowsScanException(
                () -> ScanCommand.parseArgs(List.of("--root", work.toString())));
    }

    private static void assertThrowsScanException(ThrowingRunnable runnable) {
        try {
            runnable.run();
        } catch (ScanException e) {
            return;
        } catch (RuntimeException e) {
            throw e;
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        throw new AssertionError("expected ScanException");
    }

    private interface ThrowingRunnable {
        void run() throws Exception;
    }

    @Test
    void invalidRootExitsTwo() throws Exception {
        Path dataDir = work.resolve("data");
        int code = ScanCommand.execute(List.of("--root", work.resolve("missing").toString(),
                "--file", "a.yaml", "--data-dir", dataDir.toString()));
        assertEquals(2, code);
    }

    @Test
    void unknownAdapterExitsTwo() throws Exception {
        Path repo = work.resolve("repo");
        Files.createDirectories(repo);
        write(repo, "a.yaml", "---\nkey: value\n");
        int code = ScanCommand.execute(List.of("--root", repo.toString(), "--file", "a.yaml",
                "--adapter", "not-a-scanner", "--data-dir", work.resolve("data").toString()));
        assertEquals(2, code);
    }

    @Test
    void schemaBindingFlagsParsed() throws Exception {
        var parsed = ScanCommand.parseArgs(List.of("--root", work.toString(), "--file", "a.yaml",
                "--schema-binding", "*.yaml=/tmp/schema.json",
                "--schema-dialect", "2019-09",
                "--kubeconform-schema", "/tmp/t.json",
                "--kubeconform-version", "1.28.0",
                "--kubeconform-lenient"));
        assertEquals(List.of("*.yaml=/tmp/schema.json"), parsed.schemaBindings());
        assertEquals("2019-09", parsed.schemaDialect());
        assertEquals("/tmp/t.json", parsed.kubeconformSchema());
        assertEquals("1.28.0", parsed.kubeconformVersion());
        assertTrue(!parsed.kubeconformStrict());
        assertEquals(1, parsed.localSchema().bindings().size());
        assertEquals("2019-09", parsed.localSchema().dialect());
    }

    @Test
    void invalidSchemaOptionsRejected() {
        assertThrowsScanException(() -> ScanCommand.parseArgs(List.of("--root", work.toString(),
                "--file", "a.yaml", "--schema-dialect", "draft-07")));
        assertThrowsScanException(() -> ScanCommand.parseArgs(List.of("--root", work.toString(),
                "--file", "a.yaml", "--schema-binding", "no-equals")));
        assertThrowsScanException(() -> ScanCommand.parseArgs(List.of("--root", work.toString(),
                "--file", "a.yaml", "--kubeconform-strict", "--bogus")));
    }

    @Test
    void cleanScanExitsZeroWithJsonSummary() throws Exception {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
        Path repo = work.resolve("repo");
        Files.createDirectories(repo);
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        Captured captured = runInProcess(
                List.of("--root", repo.toString(), "--file", "a.yaml",
                        "--adapter", "yamllint,gitleaks",
                        "--data-dir", dataDir.toString()));
        assertEquals(0, captured.exit());
        JsonNode json = MAPPER.readTree(captured.stdout().trim());
        assertEquals("COMPLETED", json.get("state").asText());
        assertEquals("NO_FINDINGS", json.get("verdict").asText());
        assertTrue(json.get("json_report").isNull(), "no automatic report export");
        assertTrue(json.get("markdown_report").isNull(), "no automatic report export");
    }

    @Test
    void exportFlagWritesReportFiles() throws Exception {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
        Path repo = work.resolve("repo");
        Files.createDirectories(repo);
        write(repo, "a.yaml", "---\nkey: value\n");
        Path dataDir = work.resolve("data");

        Captured captured = runInProcess(
                List.of("--root", repo.toString(), "--file", "a.yaml",
                        "--adapter", "yamllint,gitleaks", "--export",
                        "--data-dir", dataDir.toString()));
        assertEquals(0, captured.exit());
        String[] lines = captured.stdout().trim().split("\\R");
        assertEquals(2, lines.length, "summary JSON plus export JSON");
        JsonNode exported = MAPPER.readTree(lines[1]);
        assertTrue(exported.get("exported_json").asText().endsWith("report.json"));
        assertTrue(exported.get("exported_markdown").asText().endsWith("report.md"));
        assertTrue(Files.exists(dataDir.resolve(exported.get("exported_json").asText())));
        assertTrue(Files.exists(dataDir.resolve(exported.get("exported_markdown").asText())));
    }

    @Test
    void findingsScanExitsFour() throws Exception {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
        Path repo = work.resolve("repo");
        Files.createDirectories(repo);
        write(repo, "bad.yaml", "key: [unclosed\n");
        Path dataDir = work.resolve("data");

        Captured captured = runInProcess(
                List.of("--root", repo.toString(), "--file", "bad.yaml",
                        "--data-dir", dataDir.toString()));
        assertEquals(4, captured.exit());
        JsonNode json = MAPPER.readTree(captured.stdout().trim());
        assertEquals("FINDINGS", json.get("verdict").asText());
    }

    @Test
    void seededSecretAbsentFromSubprocessOutputs() throws Exception {
        assumeTrue(new YamllintAdapter().probe().available(), "SKIP: yamllint missing");
        assumeTrue(new GitleaksAdapter().probe().available(), "SKIP: gitleaks missing");
        Path repo = work.resolve("repo");
        Files.createDirectories(repo);
        write(repo, "leak.yaml", "---\ntoken: \"" + FAKE_TOKEN + "\"\n");
        Path dataDir = work.resolve("data");

        SubprocessResult result = runInSubprocess(
                List.of("--root", repo.toString(), "--file", "leak.yaml",
                        "--data-dir", dataDir.toString(), "--export"));
        assertEquals(4, result.exit(), "seeded leak must produce findings exit code");
        assertTrue(!result.stdout().contains(FAKE_TOKEN), "stdout must not contain the secret");
        assertTrue(!result.stderr().contains(FAKE_TOKEN),
                "application logs on stderr must not contain the secret");
        String[] lines = result.stdout().trim().split("\\R");
        JsonNode json = MAPPER.readTree(lines[0]);
        assertEquals("FINDINGS", json.get("verdict").asText());
        JsonNode exported = MAPPER.readTree(lines[lines.length - 1]);
        String reportPath = exported.get("exported_json").asText();
        String report = Files.readString(dataDir.resolve(reportPath));
        assertTrue(!report.contains(FAKE_TOKEN), "exported report must not contain the secret");
    }

    private record Captured(int exit, String stdout) {
    }

    private static Captured runInProcess(List<String> args) {
        java.io.PrintStream original = System.out;
        java.io.ByteArrayOutputStream captured = new java.io.ByteArrayOutputStream();
        System.setOut(new java.io.PrintStream(captured, true, StandardCharsets.UTF_8));
        int code;
        try {
            code = ScanCommand.execute(args);
        } finally {
            System.setOut(original);
        }
        return new Captured(code, captured.toString(StandardCharsets.UTF_8));
    }

    private record SubprocessResult(int exit, String stdout, String stderr) {
    }

    private static SubprocessResult runInSubprocess(List<String> scanArgs) throws Exception {
        String javaBin = System.getProperty("java.home") + File.separator + "bin" + File.separator + "java";
        List<String> command = new ArrayList<>();
        command.add(javaBin);
        command.add("-cp");
        command.add(System.getProperty("java.class.path"));
        command.add("pwt.launcher.PwtMain");
        command.add("scan");
        command.addAll(scanArgs);
        Process process = new ProcessBuilder(command).start();
        String stdout;
        String stderr;
        try (BufferedReader out = new BufferedReader(
                new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
                BufferedReader err = new BufferedReader(
                        new InputStreamReader(process.getErrorStream(), StandardCharsets.UTF_8))) {
            StringBuilder outText = new StringBuilder();
            StringBuilder errText = new StringBuilder();
            Thread outReader = new Thread(() -> out.lines().forEach(l -> outText.append(l).append('\n')));
            Thread errReader = new Thread(() -> err.lines().forEach(l -> errText.append(l).append('\n')));
            outReader.setDaemon(true);
            errReader.setDaemon(true);
            outReader.start();
            errReader.start();
            boolean finished = process.waitFor(180, TimeUnit.SECONDS);
            if (!finished) {
                process.destroyForcibly();
                throw new IllegalStateException("scan subprocess timed out");
            }
            outReader.join(10_000);
            errReader.join(10_000);
            stdout = outText.toString();
            stderr = errText.toString();
        }
        return new SubprocessResult(process.exitValue(), stdout, stderr);
    }
}

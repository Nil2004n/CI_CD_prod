package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.launcher.DoctorCommand;
import pwt.launcher.McpStatusCommand;
import pwt.launcher.RepoCommand;

/**
 * Headless management commands: repository registration/listing,
 * environment report, and recorded presence — all against temporary
 * data directories, never the real store.
 */
class LauncherCommandsTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    @TempDir
    Path work;

    private static String captureStdout(ThrowingIntSupplier runnable) throws Exception {
        PrintStream original = System.out;
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

    private interface ThrowingIntSupplier {
        int run() throws Exception;
    }

    @Test
    void repoAddIsIdempotentAndRepoListShowsIt() throws Exception {
        Path root = work.resolve("proj");
        Files.createDirectories(root);
        Path dataDir = work.resolve("data");

        String first = captureStdout(
                () -> RepoCommand.add(List.of("--root", root.toString(), "--data-dir", dataDir.toString())));
        JsonNode firstJson = MAPPER.readTree(first);
        assertTrue(firstJson.get("id").asText().length() > 8);
        assertEquals("available", firstJson.get("availability").asText());

        String second = captureStdout(
                () -> RepoCommand.add(List.of("--root", root.toString(), "--data-dir", dataDir.toString())));
        assertEquals(firstJson.get("id").asText(), MAPPER.readTree(second).get("id").asText(),
                "same canonical root must return the same repository");

        String listed = captureStdout(
                () -> RepoCommand.list(List.of("--data-dir", dataDir.toString())));
        JsonNode list = MAPPER.readTree(listed);
        assertEquals(1, list.get("repositories").size());
        assertEquals(firstJson.get("id").asText(), list.get("repositories").get(0).get("id").asText());

        assertEquals(2, RepoCommand.add(List.of()));
        assertEquals(2, RepoCommand.add(List.of("--root", work.resolve("missing").toString(),
                "--data-dir", dataDir.toString())));
    }

    @Test
    void doctorReportsEnvironment() throws Exception {
        Path dataDir = work.resolve("data");
        String out = captureStdout(() -> DoctorCommand.execute(List.of("--data-dir", dataDir.toString())));
        JsonNode report = MAPPER.readTree(out);
        assertTrue(report.get("java_version").asText().startsWith("21"));
        assertEquals(0, report.get("repository_count").asInt());
        assertTrue(report.get("scanners").size() == 6);
        boolean yamllint = false;
        for (JsonNode scanner : report.get("scanners")) {
            if (scanner.get("adapter_id").asText().equals("yamllint")) {
                yamllint = scanner.get("available").asBoolean();
            }
        }
        assertTrue(yamllint, "yamllint must be available in this environment");
        assertEquals(2, DoctorCommand.execute(List.of("--bogus")));
        // Diagnostics print the effective absolute database path.
        assertEquals(dataDir.toAbsolutePath().normalize().resolve("pwt.db").toString(),
                report.get("database").asText());
        assertEquals(dataDir.toAbsolutePath().normalize().toString(),
                report.get("data_dir").asText());
        // Installation state is reported read-only; absence is data, not failure.
        JsonNode install = report.get("install");
        assertNotNull(install);
        assertTrue(install.has("app_root"));
        assertTrue(install.has("launcher_on_path"));
        assertTrue(install.has("plugin_installed"));
        assertTrue(install.has("agent_installed"));
        assertTrue(install.has("ask_command_installed"));
        assertTrue(install.get("data_writable").asBoolean());
    }

    @Test
    void defaultDataDirResolvesToApplicationRoot() {
        // One shared database: the default lives under the application root
        // (directory holding the root Makefile), never under a repository,
        // scan directory, or working directory.
        java.nio.file.Path root = pwt.launcher.AppPaths.appRoot();
        assertTrue(java.nio.file.Files.isRegularFile(root.resolve("Makefile")));
        assertTrue(java.nio.file.Files.isExecutable(root.resolve("integrations/pwt-mcp-stdio.sh")));
        assertEquals(root.resolve(".pwt-data"),
                pwt.launcher.AppPaths.defaultDataDir());
        assertEquals(root.resolve(".pwt-data"),
                pwt.application.ScanService.defaultDataDir());
    }

    @Test
    void mcpStatusShowsRecordedSessions() throws Exception {
        Path dataDir = work.resolve("data");
        String empty = captureStdout(
                () -> McpStatusCommand.execute(List.of("--data-dir", dataDir.toString())));
        JsonNode emptyJson = MAPPER.readTree(empty);
        assertEquals("MCP: No active clients.", emptyJson.get("status_line").asText());

        try (pwt.infrastructure.SqliteStore store = pwt.infrastructure.SqliteStore.open(dataDir)) {
            store.insertMcpSession("sess-1", "srv-1", "SCANS_ENABLED", true);
            store.markMcpSessionConnected("sess-1", "Claude Code", "2.1", "2024-11-05");
        }
        String live = captureStdout(
                () -> McpStatusCommand.execute(List.of("--data-dir", dataDir.toString())));
        JsonNode liveJson = MAPPER.readTree(live);
        assertTrue(liveJson.get("status_line").asText().contains("connected"));
        assertEquals(1, liveJson.get("sessions").size());
        assertEquals("sess-1", liveJson.get("sessions").get(0).get("session_id").asText());
        assertEquals(2, McpStatusCommand.execute(List.of("--bogus")));
    }
}

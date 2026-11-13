package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.application.CancellationToken;
import pwt.infrastructure.ProcessRunner;

class ProcessRunnerTest {

    @TempDir
    Path temp;

    @Test
    void successfulExecutionCapturesStreamsAndExitCode() throws Exception {
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of("/bin/echo", "hello"), temp, null,
                Duration.ofSeconds(10), 1024, 1024, CancellationToken.none());
        assertEquals(0, result.exitCode());
        assertEquals("hello\n", result.stdout());
        assertEquals("", result.stderr());
        assertTrue(!result.timedOut() && !result.cancelled());
        assertTrue(!result.stdoutTruncated());
    }

    @Test
    void nonzeroExitPreserved() throws Exception {
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of("/bin/sh", "-c", "exit 3"), temp, null,
                Duration.ofSeconds(10), 1024, 1024, CancellationToken.none());
        assertEquals(3, result.exitCode());
        assertTrue(!result.timedOut());
    }

    @Test
    void missingExecutableThrows() {
        assertThrows(Exception.class, () -> ProcessRunner.run(
                List.of("/nonexistent-tool-xyz-123"), temp, null,
                Duration.ofSeconds(10), 1024, 1024, CancellationToken.none()));
    }

    @Test
    void timeoutTerminatesProcess() throws Exception {
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of("/bin/sleep", "30"), temp, null,
                Duration.ofMillis(500), 1024, 1024, CancellationToken.none());
        assertTrue(result.timedOut(), "sleep 30 must exceed a 500ms budget");
        assertEquals(-1, result.exitCode());
    }

    @Test
    void outputBeyondCapIsTruncated() throws Exception {
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of("/bin/sh", "-c", "yes x | head -c 100000"), temp, null,
                Duration.ofSeconds(15), 1024, 1024, CancellationToken.none());
        assertTrue(result.stdoutTruncated(), "100KB must exceed a 1KB cap");
        assertTrue(result.stdout().length() <= 1100);
    }

    @Test
    void preCancelledTokenCancelsWithoutHanging() throws Exception {
        CancellationToken cancelled = new CancellationToken();
        cancelled.cancel();
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of("/bin/sleep", "30"), temp, null,
                Duration.ofSeconds(60), 1024, 1024, cancelled);
        assertTrue(result.cancelled());
    }

    @Test
    void environmentBlocklistRemovesVariables() throws Exception {
        Path script = temp.resolve("show-env.sh");
        Files.writeString(script, "#!/bin/sh\necho \"v=${PWT_TEST_VAR:-absent}\"\n");
        script.toFile().setExecutable(true);
        ProcessRunner.ProcessResult result = ProcessRunner.run(
                List.of(script.toString()), temp, java.util.Map.of("PWT_TEST_VAR", "present"),
                java.util.Set.of("PWT_TEST_VAR"),
                Duration.ofSeconds(10), 1024, 1024, CancellationToken.none());
        assertEquals(0, result.exitCode());
        assertTrue(result.stdout().contains("v=absent"), "blocklisted variable must be removed");
    }
}

package pwt.infrastructure;

import java.time.Duration;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.TimeUnit;
import pwt.application.CancellationToken;

/**
 * Bounded external-process execution. Uses {@link ProcessBuilder} with a
 * separate executable/argument list — never a shell. Stdout and stderr are
 * drained concurrently with hard byte caps; the process tree is terminated
 * best-effort on timeout or cancellation.
 */
public final class ProcessRunner {

    public record ProcessResult(
            int exitCode,
            String stdout,
            String stderr,
            boolean stdoutTruncated,
            boolean stderrTruncated,
            boolean timedOut,
            boolean cancelled) {
    }

    private ProcessRunner() {
    }

    public static ProcessResult run(
            List<String> command,
            java.nio.file.Path workDir,
            java.util.Map<String, String> environmentOverlay,
            Duration timeout,
            long maxStdoutBytes,
            long maxStderrBytes,
            CancellationToken cancellation) throws java.io.IOException, InterruptedException {
        return run(command, workDir, environmentOverlay, null, timeout,
                maxStdoutBytes, maxStderrBytes, cancellation);
    }

    public static ProcessResult run(
            List<String> command,
            java.nio.file.Path workDir,
            java.util.Map<String, String> environmentOverlay,
            java.util.Set<String> environmentBlocklist,
            Duration timeout,
            long maxStdoutBytes,
            long maxStderrBytes,
            CancellationToken cancellation) throws java.io.IOException, InterruptedException {
        Objects.requireNonNull(command, "command");
        if (command.isEmpty()) {
            throw new IllegalArgumentException("command must not be empty");
        }
        ProcessBuilder builder = new ProcessBuilder(command);
        builder.redirectErrorStream(false);
        if (workDir != null) {
            builder.directory(workDir.toFile());
        }
        if (environmentOverlay != null) {
            builder.environment().putAll(environmentOverlay);
        }
        if (environmentBlocklist != null) {
            for (String name : environmentBlocklist) {
                builder.environment().remove(name);
            }
        }
        Process process = builder.start();
        StreamGobbler out = new StreamGobbler(process.getInputStream(), maxStdoutBytes);
        StreamGobbler err = new StreamGobbler(process.getErrorStream(), maxStderrBytes);
        Thread outThread = new Thread(out, "pwt-proc-stdout");
        Thread errThread = new Thread(err, "pwt-proc-stderr");
        outThread.setDaemon(true);
        errThread.setDaemon(true);
        outThread.start();
        errThread.start();

        boolean finished;
        boolean cancelled = false;
        long deadline = System.nanoTime() + timeout.toNanos();
        while (true) {
            if (cancellation != null && cancellation.isCancelled()) {
                cancelled = true;
                break;
            }
            long remaining = deadline - System.nanoTime();
            if (remaining <= 0) {
                break;
            }
            finished = process.waitFor(Math.min(remaining, 100_000_000L), TimeUnit.NANOSECONDS);
            if (finished) {
                break;
            }
        }
        boolean completed = false;
        try {
            completed = process.waitFor(0, TimeUnit.NANOSECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            destroyTree(process);
            throw e;
        }
        boolean timedOut = !completed && !cancelled;
        if (!completed) {
            destroyTree(process);
            process.waitFor(10, TimeUnit.SECONDS);
        }
        outThread.join(10_000);
        errThread.join(10_000);
        int exit = completed ? process.exitValue() : -1;
        return new ProcessResult(exit, out.text(), err.text(), out.truncated(), err.truncated(), timedOut, cancelled);
    }

    private static void destroyTree(Process process) {
        try {
            process.descendants().forEach(handle -> {
                try {
                    handle.destroyForcibly();
                } catch (Exception ignored) {
                    // best effort per descendant
                }
            });
        } catch (Exception ignored) {
            // best effort; ProcessHandle may be restricted
        }
        process.destroyForcibly();
    }

    private static final class StreamGobbler implements Runnable {
        private final java.io.InputStream stream;
        private final long cap;
        private final StringBuilder text = new StringBuilder();
        private boolean truncated;

        StreamGobbler(java.io.InputStream stream, long cap) {
            this.stream = stream;
            this.cap = cap;
        }

        @Override
        public void run() {
            byte[] buffer = new byte[8192];
            long kept = 0;
            try {
                int read;
                while ((read = stream.read(buffer)) != -1) {
                    if (kept < cap) {
                        int take = (int) Math.min(read, cap - kept);
                        text.append(new String(buffer, 0, take, java.nio.charset.StandardCharsets.UTF_8));
                        kept += take;
                    } else {
                        truncated = true;
                    }
                }
            } catch (java.io.IOException ignored) {
                // stream closed by process teardown; partial output is kept
            } finally {
                try {
                    stream.close();
                } catch (java.io.IOException ignored) {
                    // best effort
                }
            }
        }

        String text() {
            return text.toString();
        }

        boolean truncated() {
            return truncated;
        }
    }
}

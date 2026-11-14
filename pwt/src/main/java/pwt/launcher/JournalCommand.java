package pwt.launcher;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pwt.application.JournalWriter;
import pwt.domain.RepositoryRoot;
import pwt.infrastructure.NioDiscoveryService;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;

/**
 * Journal maintenance over the shared store.
 *
 * <pre>
 * pwt journal --root &lt;directory&gt; [--run-id &lt;id&gt;] [--check] [--data-dir &lt;dir&gt;]
 * </pre>
 *
 * <p>Appends missing scan sections to the repository journal
 * ({@code <data-dir>/reports/<name>--<id>.md}) from persisted runs
 * (backfill + retry after a failed append). With {@code --check} nothing
 * is written; each terminal run is reported as {@code current} or
 * {@code missing}. Non-terminal runs are reported
 * {@code skipped-non-terminal} and never written.
 *
 * <p>Diagnostics on stderr; one JSON object per processed run on stdout.
 * Exit 0 ok, 2 invalid input, 1 storage failure. Acquires the repository
 * lock so concurrent publication cannot interleave sections.
 */
public final class JournalCommand {

    private JournalCommand() {
    }

    public static int execute(List<String> args) {
        Path root = null;
        String runId = null;
        boolean check = false;
        Path dataDir = pwt.application.ScanService.defaultDataDir();
        for (int i = 0; i < args.size(); i++) {
            switch (args.get(i)) {
                case "--root" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --root");
                    }
                    root = Path.of(args.get(i));
                }
                case "--run-id" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --run-id");
                    }
                    runId = args.get(i);
                }
                case "--check" -> check = true;
                case "--data-dir" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --data-dir");
                    }
                    dataDir = Path.of(args.get(i));
                }
                default -> {
                    return usage("unknown journal argument: " + args.get(i));
                }
            }
        }
        if (root == null) {
            return usage("Usage: journal --root <directory> [--run-id <id>] [--check] [--data-dir <dir>]");
        }
        RepositoryRoot registered;
        try {
            registered = new NioDiscoveryService().registerRoot(root);
        } catch (Exception e) {
            System.err.println("pwt journal: invalid input: " + e.getMessage());
            return 2;
        }
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow repo = store.registerRepository(
                    registered.root().toString(), registered.displayName(), registered.identityHash());
            List<SqliteStore.RunView> runs;
            if (runId != null) {
                SqliteStore.RunView single = store.getRun(runId);
                if (single == null) {
                    System.err.println("pwt journal: unknown run_id: " + runId);
                    return 2;
                }
                if (!single.repositoryId().equals(repo.id())) {
                    System.err.println("pwt journal: run " + runId
                            + " belongs to a different repository");
                    return 2;
                }
                runs = List.of(single);
            } else {
                runs = new ArrayList<>(store.listRuns(repo.id(), null, Integer.MAX_VALUE, 0));
                runs.sort((a, b) -> a.createdAt().compareTo(b.createdAt()));
            }
            Path journal = JournalWriter.journalPath(dataDir, repo);
            RepoLock lock = null;
            if (!check) {
                try {
                    var acquired = RepoLock.tryAcquire(dataDir, repo.identityHash());
                    if (acquired.isEmpty()) {
                        System.err.println("pwt journal: repository is busy (another operation holds the lock)");
                        return 2;
                    }
                    lock = acquired.get();
                } catch (Exception e) {
                    System.err.println("pwt journal: cannot acquire repository lock: " + e.getMessage());
                    return 1;
                }
            }
            try {
                List<String> lines = new ArrayList<>();
                for (SqliteStore.RunView run : runs) {
                    String status;
                    String error = null;
                    if (!isTerminal(run.state().name())) {
                        status = "skipped-non-terminal";
                    } else if (check) {
                        status = wouldAppend(store, dataDir, journal, run.id()) ? "missing" : "current";
                    } else {
                        try {
                            status = JournalWriter.appendRun(store, dataDir, run.id());
                        } catch (JournalWriter.JournalException e) {
                            status = "failed";
                            error = e.getMessage();
                        }
                    }
                    StringBuilder line = new StringBuilder("{\"run_id\":\"")
                            .append(JsonWriter.escape(run.id())).append("\",\"status\":\"").append(status)
                            .append("\",\"journal\":\"").append(JsonWriter.escape(journal.toString()))
                            .append("\"");
                    if (error != null) {
                        line.append(",\"error\":\"").append(JsonWriter.escape(error)).append('"');
                    } else {
                        line.append(",\"error\":null");
                    }
                    line.append('}');
                    lines.add(line.toString());
                }
                for (String line : lines) {
                    System.out.println(line);
                }
                return 0;
            } finally {
                if (lock != null) {
                    try {
                        lock.close();
                    } catch (Exception ignored) {
                        // best effort
                    }
                }
            }
        } catch (Exception e) {
            System.err.println("pwt journal: storage failure: " + e.getMessage());
            return 1;
        }
    }

    private static boolean wouldAppend(SqliteStore store, Path dataDir, Path journal, String runId) {
        try {
            SqliteStore.RunView run = store.getRun(runId);
            if (run == null || !isTerminal(run.state().name())) {
                return false;
            }
            if (!java.nio.file.Files.exists(journal)) {
                return true;
            }
            String content = java.nio.file.Files.readString(journal,
                    java.nio.charset.StandardCharsets.UTF_8);
            String start = JournalWriter.MARKER_START_PREFIX + runId + JournalWriter.MARKER_SUFFIX;
            String end = JournalWriter.MARKER_END_PREFIX + runId + JournalWriter.MARKER_SUFFIX;
            int at = content.indexOf(start);
            return at < 0 || content.indexOf(end, at + start.length()) < 0;
        } catch (Exception e) {
            return true;
        }
    }

    private static boolean isTerminal(String state) {
        return switch (state) {
            case "COMPLETED", "PARTIAL", "FAILED", "CANCELLED", "INTERRUPTED" -> true;
            default -> false;
        };
    }

    private static int usage(String message) {
        System.err.println("pwt journal: " + message);
        return 2;
    }

    static List<String> split(String[] args, int from) {
        List<String> rest = new ArrayList<>();
        for (int i = from; i < args.length; i++) {
            rest.add(args[i]);
        }
        return rest;
    }
}

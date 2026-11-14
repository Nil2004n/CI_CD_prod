package pwt.launcher;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pwt.application.DiscoveryService;
import pwt.domain.RepositoryRoot;
import pwt.infrastructure.NioDiscoveryService;
import pwt.infrastructure.SqliteStore;

/**
 * Headless repository registration and listing over the shared store.
 *
 * <pre>
 * pwt repo-add --root &lt;directory&gt; [--name &lt;display&gt;] [--data-dir &lt;dir&gt;]
 * pwt repo-list [--data-dir &lt;dir&gt;]
 * </pre>
 *
 * <p>Registration validates and canonicalizes an explicitly supplied local
 * directory and persists it through the existing repository service
 * (idempotent: same canonical root returns the same repository ID). No Git
 * interaction, no hooks, no network. Roots registered here are immediately
 * visible to the GUI, MCP, and headless scans sharing the data directory.
 * Diagnostics on stderr; JSON on stdout. Exit 0 ok, 2 invalid input,
 * 1 storage failure.
 */
public final class RepoCommand {

    private RepoCommand() {
    }

    public static int add(List<String> args) {
        Path root = null;
        String name = null;
        Path dataDir = pwt.application.ScanService.defaultDataDir();
        for (int i = 0; i < args.size(); i++) {
            switch (args.get(i)) {
                case "--root" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --root");
                    }
                    root = Path.of(args.get(i));
                }
                case "--name" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --name");
                    }
                    name = args.get(i);
                }
                case "--data-dir" -> {
                    if (++i >= args.size()) {
                        return usage("missing value for --data-dir");
                    }
                    dataDir = Path.of(args.get(i));
                }
                default -> {
                    return usage("unknown repo-add argument: " + args.get(i));
                }
            }
        }
        if (root == null) {
            return usage("Usage: repo-add --root <directory> [--name <display>] [--data-dir <dir>]");
        }
        RepositoryRoot registered;
        try {
            registered = new NioDiscoveryService().registerRoot(root);
        } catch (Exception e) {
            System.err.println("pwt repo-add: invalid input: " + e.getMessage());
            return 2;
        }
        String display = name == null || name.isBlank() ? registered.displayName() : name.trim();
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            SqliteStore.RepositoryRow row = store.registerRepository(
                    registered.root().toString(), display, registered.identityHash());
            boolean available = Files.isDirectory(Path.of(row.canonicalRoot()));
            System.out.println("{\"id\":\"" + JsonWriter.escape(row.id()) + "\","
                    + "\"display_name\":\"" + JsonWriter.escape(row.displayName()) + "\","
                    + "\"canonical_root\":\"" + JsonWriter.escape(row.canonicalRoot()) + "\","
                    + "\"availability\":\"" + (available ? "available" : "unavailable") + "\"}");
            try {
                Path journal = pwt.application.JournalWriter.journalPath(
                        dataDir.toAbsolutePath().normalize(), row);
                Path written = ProjectContext.writeIfManaged(registered.root(), row, journal,
                        AppPaths.appRoot(), dataDir.toAbsolutePath().normalize());
                if (written != null) {
                    System.err.println("pwt repo-add: context pointer: " + written);
                } else {
                    System.err.println("pwt repo-add: kept existing non-PWT .pwt/context.md untouched");
                }
            } catch (Exception e) {
                System.err.println("pwt repo-add: warning: context pointer not written: " + e.getMessage());
            }
            return 0;
        } catch (Exception e) {
            System.err.println("pwt repo-add: storage failure: " + e.getMessage());
            return 1;
        }
    }

    public static int list(List<String> args) {
        Path dataDir = pwt.application.ScanService.defaultDataDir();
        for (int i = 0; i < args.size(); i++) {
            if (args.get(i).equals("--data-dir")) {
                if (++i >= args.size()) {
                    return usage("missing value for --data-dir");
                }
                dataDir = Path.of(args.get(i));
            } else {
                return usage("unknown repo-list argument: " + args.get(i));
            }
        }
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            List<SqliteStore.RepositoryRow> rows = store.listRepositories();
            StringBuilder out = new StringBuilder("{\"repositories\":[");
            for (SqliteStore.RepositoryRow row : rows) {
                boolean available = Files.isDirectory(Path.of(row.canonicalRoot()));
                out.append("{\"id\":\"").append(JsonWriter.escape(row.id())).append("\","
                        + "\"display_name\":\"").append(JsonWriter.escape(row.displayName())).append("\","
                        + "\"canonical_root\":\"").append(JsonWriter.escape(row.canonicalRoot())).append("\","
                        + "\"availability\":\"").append(available ? "available" : "unavailable").append("\"},");
            }
            if (!rows.isEmpty()) {
                out.deleteCharAt(out.length() - 1);
            }
            out.append("]}");
            System.out.println(out);
            return 0;
        } catch (Exception e) {
            System.err.println("pwt repo-list: storage failure: " + e.getMessage());
            return 1;
        }
    }

    private static int usage(String message) {
        System.err.println("pwt: " + message);
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

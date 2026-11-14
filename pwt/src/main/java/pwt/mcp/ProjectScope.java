package pwt.mcp;

import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import pwt.infrastructure.SqliteStore;

/**
 * Resolves a project-root-bound MCP connection to exactly one registered
 * repository, on every call (so a project registered later by its first
 * dashboard scan becomes visible without restarting the client).
 *
 * <p>Rules (same as the OpenCode connector): the bound directory is
 * canonicalized with {@code toRealPath()} so symlinked spellings and
 * subdirectories resolve consistently; the deepest registered canonical
 * root that equals or contains it wins (nested projects bind to the inner
 * one). Never registers anything, never falls back to another project, and
 * error messages never name other repositories.
 *
 * <p>Pinning: the first successful resolution is pinned for the life of the
 * server process (one process serves one client connection). If the bound
 * directory later resolves to a different repository (for example a nested
 * project registered mid-session), every call fails closed with
 * PROJECT_SCOPE_CHANGED instead of silently moving the session to another
 * project. "Unregistered at start, registered later" still works because
 * nothing is pinned until the first success.
 */
public final class ProjectScope {

    private static final Map<Path, String> PINNED = new ConcurrentHashMap<>();

    private ProjectScope() {
    }

    /** Canonical form of the bound directory, or PROJECT_UNAVAILABLE. */
    public static Path canonicalDirectory(Path boundRoot) throws McpErrors.McpServiceException {
        try {
            if (boundRoot == null || !Files.isDirectory(boundRoot)) {
                throw new McpErrors.McpServiceException(McpErrors.PROJECT_UNAVAILABLE,
                        "the project directory this connection is bound to is missing or not a directory");
            }
            return boundRoot.toRealPath();
        } catch (McpErrors.McpServiceException e) {
            throw e;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.PROJECT_UNAVAILABLE,
                    "the project directory this connection is bound to cannot be resolved");
        }
    }

    /** The single registered repository owning the bound directory. */
    public static SqliteStore.RepositoryRow resolve(SqliteStore store, Path boundRoot)
            throws McpErrors.McpServiceException {
        Path here = canonicalDirectory(boundRoot);
        SqliteStore.RepositoryRow best = null;
        int bestDepth = -1;
        boolean ambiguous = false;
        try {
            for (SqliteStore.RepositoryRow repo : store.listRepositories()) {
                Path root = canonicalRoot(repo.canonicalRoot());
                if (root == null || !here.startsWith(root)) {
                    continue;
                }
                int depth = root.getNameCount();
                if (depth > bestDepth) {
                    best = repo;
                    bestDepth = depth;
                    ambiguous = false;
                } else if (depth == bestDepth && !repo.id().equals(best.id())) {
                    ambiguous = true;
                }
            }
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                    "cannot read registered repositories");
        }
        if (ambiguous) {
            throw new McpErrors.McpServiceException(McpErrors.PROJECT_AMBIGUOUS,
                    "more than one registered repository claims this project directory;"
                            + " open /pwt and resolve the registration in the dashboard");
        }
        String pinned = PINNED.get(here);
        if (pinned != null && (best == null || !pinned.equals(best.id()))) {
            throw new McpErrors.McpServiceException(McpErrors.PROJECT_SCOPE_CHANGED,
                    "the project registration for this directory changed during this session;"
                            + " start a new client session to use the current registration");
        }
        if (best == null) {
            throw new McpErrors.McpServiceException(McpErrors.PROJECT_NOT_REGISTERED,
                    "this project is not registered with PWT yet; open /pwt and run its first scan"
                            + " in the dashboard (that registers it). Nothing was registered by this call.");
        }
        PINNED.putIfAbsent(here, best.id());
        String winner = PINNED.get(here);
        if (!winner.equals(best.id())) {
            throw new McpErrors.McpServiceException(McpErrors.PROJECT_SCOPE_CHANGED,
                    "the project registration for this directory changed during this session;"
                            + " start a new client session to use the current registration");
        }
        return best;
    }

    private static Path canonicalRoot(String stored) {
        if (stored == null || stored.isBlank()) {
            return null;
        }
        Path path = Path.of(stored).toAbsolutePath().normalize();
        try {
            if (Files.exists(path, LinkOption.NOFOLLOW_LINKS) || Files.exists(path)) {
                return path.toRealPath();
            }
        } catch (Exception ignored) {
            // fall through to the stored spelling
        }
        return path;
    }
}

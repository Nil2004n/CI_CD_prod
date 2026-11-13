package pwt.infrastructure;

import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.PosixFilePermission;
import java.util.Comparator;
import java.util.Set;
import java.util.stream.Stream;

/**
 * Private per-run staging: selected files are copied byte-for-byte into a
 * run-specific snapshot directory that preserves relative layout. Scanners
 * run against the snapshot, never the live repository. Staged files can
 * contain secrets: POSIX directories are created local-user-only where
 * supported, and the tree is deleted best-effort when the run finishes.
 * Secure deletion and crash-time cleanup are Phase 6 hardening work.
 */
public final class Staging {

    private Staging() {
    }

    public static Path createSnapshot(Path dataDir, String runId) throws IOException {
        Path snapshot = dataDir.resolve("transient").resolve(runId).resolve("snapshot");
        Files.createDirectories(snapshot);
        restrict(snapshot);
        Path parent = snapshot.getParent();
        if (parent != null) {
            restrict(parent);
        }
        return snapshot;
    }

    private static void restrict(Path dir) {
        try {
            Files.setPosixFilePermissions(dir, Set.of(
                    PosixFilePermission.OWNER_READ,
                    PosixFilePermission.OWNER_WRITE,
                    PosixFilePermission.OWNER_EXECUTE));
        } catch (UnsupportedOperationException | IOException ignored) {
            // non-POSIX filesystem: permissions stay as created
        }
    }

    public static long stageFile(Path repoRoot, String relativePath, Path snapshot) throws IOException {
        Path source = repoRoot.resolve(relativePath).normalize();
        if (!source.startsWith(repoRoot) || Files.isSymbolicLink(source)) {
            throw new IOException("Refusing to stage non-contained or symlinked path: " + relativePath);
        }
        Path target = snapshot.resolve(relativePath).normalize();
        if (!target.startsWith(snapshot)) {
            throw new IOException("Refusing to stage escaping path: " + relativePath);
        }
        Files.createDirectories(target.getParent());
        Files.copy(source, target);
        try {
            Files.setPosixFilePermissions(target, Set.of(
                    PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE));
        } catch (UnsupportedOperationException | IOException ignored) {
            // non-POSIX filesystem: best effort only
        }
        return Files.size(target);
    }

    public static void deleteTree(Path root) {
        if (root == null || !Files.exists(root, LinkOption.NOFOLLOW_LINKS)) {
            return;
        }
        try (Stream<Path> walk = Files.walk(root)) {
            walk.sorted(Comparator.reverseOrder()).forEach(path -> {
                try {
                    Files.deleteIfExists(path);
                } catch (IOException ignored) {
                    // best effort; orphan cleanup is Phase 6 work
                }
            });
        } catch (IOException ignored) {
            // best effort
        }
    }

    public static void deleteRunTransient(Path dataDir, String runId) {
        deleteTree(dataDir.resolve("transient").resolve(runId));
    }
}

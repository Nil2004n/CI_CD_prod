package pwt.infrastructure;

import java.io.IOException;
import java.nio.file.FileVisitOption;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.EnumSet;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import pwt.application.CancellationToken;
import pwt.application.DiscoveryException;
import pwt.application.DiscoveryService;
import pwt.domain.DiscoveredYamlFile;
import pwt.domain.DiscoveryOptions;
import pwt.domain.DiscoveryResult;
import pwt.domain.DiscoveryWarning;
import pwt.domain.RepositoryRoot;

/**
 * {@link DiscoveryService} implemented on {@code java.nio.file} traversal.
 *
 * <p>Safety properties:
 * <ul>
 *   <li>No symlink following: {@code NOFOLLOW_LINKS} is never enabled;
 *       symlinked files and directories are skipped with a
 *       {@code SYMLINK_SKIPPED} warning.</li>
 *   <li>No parent traversal: the walk starts at the canonical root and only
 *       descends; {@code ".."} can never appear because traversal never
 *       ascends.</li>
 *   <li>No execution: only directory listings and attribute reads occur.</li>
 *   <li>No content reads: file selection matches on the name only; sizes come
 *       from attributes gathered during the walk.</li>
 *   <li>Bounded: {@code maxDepth} and {@code maxDiscoveredFiles} stop the
 *       walk; hitting a bound sets {@code incomplete} with a warning.</li>
 *   <li>Cooperative cancellation is checked on every visited path.</li>
 * </ul>
 *
 * <p>Limitation: results describe the filesystem at walk time. A hostile
 * party mutating the tree concurrently (swapping directories for symlinks,
 * etc.) can invalidate observations; this walker does not claim to detect or
 * prevent such races.
 */
public final class NioDiscoveryService implements DiscoveryService {

    @Override
    public RepositoryRoot registerRoot(Path supplied) throws DiscoveryException {
        Path canonical = DiscoveryService.checkInspectableDirectory(supplied);
        String display = canonical.getFileName() != null ? canonical.getFileName().toString() : canonical.toString();
        return new RepositoryRoot(canonical, display, RepositoryRoot.identityHashFor(canonical));
    }

    @Override
    public DiscoveryResult discover(RepositoryRoot root, DiscoveryOptions options, CancellationToken cancellation)
            throws DiscoveryException {
        Objects.requireNonNull(root, "root");
        Objects.requireNonNull(options, "options");
        Objects.requireNonNull(cancellation, "cancellation");
        cancellation.throwIfCancelled();

        List<DiscoveredYamlFile> found = new ArrayList<>();
        List<DiscoveryWarning> warnings = new ArrayList<>();
        boolean[] incomplete = {false};
        boolean[] limitWarned = {false};

        var visitor = new SimpleFileVisitor<Path>() {

            private void checkCancelled() throws IOException {
                if (cancellation.isCancelled()) {
                    incomplete[0] = true;
                    warnings.add(DiscoveryWarning.of("CANCELLED",
                            "Discovery cancelled by caller; results are partial.", null));
                    throw new CancelTraversal();
                }
            }

            @Override
            public java.nio.file.FileVisitResult preVisitDirectory(Path dir, BasicFileAttributes attrs)
                    throws IOException {
                checkCancelled();
                if (Files.isSymbolicLink(dir)) {
                    warnings.add(DiscoveryWarning.of("SYMLINK_SKIPPED",
                            "Symlinked directory not followed.", relOf(root, dir)));
                    return FileVisitResult.SKIP_SUBTREE;
                }
                Path name = dir.getFileName();
                // Never skip the root itself even if its name were excluded.
                if (name != null && !dir.equals(root.root())
                        && options.excludedDirectoryNames().contains(name.toString())) {
                    warnings.add(DiscoveryWarning.of("EXCLUDED_DIRECTORY",
                            "Directory skipped by exclusion rule: " + name, relOf(root, dir)));
                    return FileVisitResult.SKIP_SUBTREE;
                }
                int depth = root.root().relativize(dir).getNameCount();
                // relativize(root, root) yields empty path; guard its count.
                int actualDepth = dir.equals(root.root()) ? 0 : depth;
                if (actualDepth >= options.maxDepth()) {
                    incomplete[0] = true;
                    warnings.add(DiscoveryWarning.of("DEPTH_LIMIT",
                            "Maximum traversal depth reached at: " + relOf(root, dir) + "; deeper paths not visited.",
                            relOf(root, dir)));
                    return FileVisitResult.SKIP_SUBTREE;
                }
                return FileVisitResult.CONTINUE;
            }

            @Override
            public java.nio.file.FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                checkCancelled();
                if (Files.isSymbolicLink(file)) {
                    warnings.add(DiscoveryWarning.of("SYMLINK_SKIPPED",
                            "Symlinked file not followed.", relOf(root, file)));
                    return FileVisitResult.CONTINUE;
                }
                String fileName = file.getFileName().toString();
                if (isYamlName(fileName)) {
                    if (found.size() >= options.maxDiscoveredFiles()) {
                        incomplete[0] = true;
                        if (!limitWarned[0]) {
                            limitWarned[0] = true;
                            warnings.add(DiscoveryWarning.of("FILE_LIMIT",
                                    "Discovery file limit reached (" + options.maxDiscoveredFiles()
                                            + " files); remaining matches omitted and the result is incomplete.",
                                    null));
                        }
                        return FileVisitResult.TERMINATE;
                    }
                    found.add(new DiscoveredYamlFile(relOf(root, file), attrs.size()));
                }
                return FileVisitResult.CONTINUE;
            }

            @Override
            public java.nio.file.FileVisitResult visitFileFailed(Path file, IOException exc) throws IOException {
                checkCancelled();
                String detail = exc == null ? "unknown error" : String.valueOf(exc.getMessage());
                warnings.add(DiscoveryWarning.of("UNREADABLE_PATH",
                        "Path could not be inspected (" + detail + "); coverage of this path is unknown.",
                        relOf(root, file)));
                return FileVisitResult.CONTINUE;
            }
        };

        try {
            // Depth: root children are depth 1. Integer.MAX_VALUE lets our own
            // maxDepth check (with its warning) govern instead of silent stop.
            Files.walkFileTree(root.root(), EnumSet.noneOf(FileVisitOption.class), Integer.MAX_VALUE, visitor);
        } catch (CancelTraversal cancelled) {
            throw new DiscoveryException(DiscoveryException.Code.CANCELLED, "Discovery cancelled by caller",
                    cancelled);
        } catch (IOException e) {
            throw new DiscoveryException(DiscoveryException.Code.IO_ERROR,
                    "Discovery failed: " + e.getMessage(), e);
        } catch (SecurityException e) {
            throw new DiscoveryException(DiscoveryException.Code.IO_ERROR,
                    "Discovery denied access: " + e.getMessage(), e);
        }

        found.sort(Comparator.comparing(DiscoveredYamlFile::relativePath));
        return new DiscoveryResult(root, found, warnings, incomplete[0]);
    }

    private static String relOf(RepositoryRoot root, Path path) {
        try {
            Path rel = root.root().relativize(path);
            String s = rel.toString().replace('\\', '/');
            return s.isEmpty() ? "." : s;
        } catch (IllegalArgumentException e) {
            return path.toString();
        }
    }

    static boolean isYamlName(String fileName) {
        String lower = fileName.toLowerCase(Locale.ROOT);
        return lower.endsWith(".yaml") || lower.endsWith(".yml");
    }

    /** Control-flow signal for cooperative cancellation (not an error). */
    private static final class CancelTraversal extends IOException {
        CancelTraversal() {
            super("cancelled");
        }
    }
}

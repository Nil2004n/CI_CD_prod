package pwt.application;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;
import pwt.domain.DiscoveryOptions;
import pwt.domain.DiscoveryResult;
import pwt.domain.RepositoryRoot;

/**
 * Safe YAML discovery: validates an explicitly supplied local directory,
 * resolves its canonical identity, and lists {@code .yaml}/{@code .yml}
 * files beneath it without following symlinks, leaving the root, executing
 * repository content, or reading file contents.
 *
 * <p>Implementations must honour {@link CancellationToken} promptly and
 * report bounds via {@link DiscoveryResult#incomplete()} plus warnings
 * instead of silent truncation.
 *
 * <p>Threading: implementations must be safe to call from a worker thread;
 * desktop callers keep this off the JavaFX application thread.
 */
public interface DiscoveryService {

    /**
     * Validates {@code supplied} (must exist, be a directory, be inspectable)
     * and returns its canonical session-only descriptor.
     */
    RepositoryRoot registerRoot(Path supplied) throws DiscoveryException;

    /** Runs discovery beneath a previously registered root. */
    DiscoveryResult discover(RepositoryRoot root, DiscoveryOptions options, CancellationToken cancellation)
            throws DiscoveryException;

    /**
     * Validates caller-supplied relative-path input against a registered
     * root. Rejects absolute paths and anything escaping the root, using
     * filesystem containment on the normalized absolute path — not string
     * prefixes. Returns the clean forward-slash relative path.
     */
    static String requireInsideRoot(RepositoryRoot root, String input) throws DiscoveryException {
        Objects.requireNonNull(root, "root");
        Objects.requireNonNull(input, "input");
        if (input.isEmpty()) {
            throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH, "Empty path is not a valid selection");
        }
        Path candidate = Path.of(input);
        if (candidate.isAbsolute()) {
            throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                    "Absolute paths are not accepted where a root-relative path is required: " + input);
        }
        Path normalizedAbsolute = root.root().resolve(candidate).normalize();
        if (!normalizedAbsolute.startsWith(root.root())) {
            throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                    "Path escapes the registered root: " + input);
        }
        // Filesystem containment: the root itself is canonical (toRealPath at
        // registration), so a normalized path inside it cannot name a parent
        // via "..". Symlink escape inside the root is excluded by the
        // no-follow traversal rule, and re-checked at read time by callers.
        //
        // Limitation (documented): this check is not protection against a
        // hostile party concurrently mutating the filesystem between the check
        // and a later read. Phase 1 offers no sandbox against concurrent
        // mutation; later phases re-validate at snapshot time.
        return root.root().relativize(normalizedAbsolute).toString().replace('\\', '/');
    }

    /** Shared root validation used before any traversal. */
    static Path checkInspectableDirectory(Path supplied) throws DiscoveryException {
        Objects.requireNonNull(supplied, "supplied");
        if (!Files.exists(supplied)) {
            throw new DiscoveryException(DiscoveryException.Code.ROOT_NOT_FOUND,
                    "Directory does not exist: " + supplied);
        }
        if (!Files.isDirectory(supplied)) {
            throw new DiscoveryException(DiscoveryException.Code.ROOT_NOT_DIRECTORY,
                    "Not a directory: " + supplied);
        }
        try {
            Path canonical = supplied.toRealPath();
            // Proof of inspectability: opening a directory stream must succeed.
            try (var ignored = Files.newDirectoryStream(canonical)) {
                // successfully opened
            }
            return canonical;
        } catch (IOException e) {
            throw new DiscoveryException(DiscoveryException.Code.ROOT_NOT_READABLE,
                    "Directory cannot be inspected: " + supplied + " (" + e.getMessage() + ")", e);
        } catch (SecurityException e) {
            throw new DiscoveryException(DiscoveryException.Code.ROOT_NOT_READABLE,
                    "Directory cannot be inspected (access denied): " + supplied, e);
        }
    }
}

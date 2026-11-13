package pwt.domain;

import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Objects;

/**
 * A validated, session-only repository root.
 *
 * <p>Phase 1: registration is in-memory only. No persistence is performed.
 * The canonical identity is the filesystem real path (symlinks in the root
 * itself are resolved once, at registration). Discovery below the root never
 * follows symlinks.
 */
public record RepositoryRoot(Path root, String displayName, String identityHash) {

    public RepositoryRoot {
        Objects.requireNonNull(root, "root");
        Objects.requireNonNull(displayName, "displayName");
        Objects.requireNonNull(identityHash, "identityHash");
        if (!root.isAbsolute()) {
            throw new IllegalArgumentException("RepositoryRoot must hold an absolute path: " + root);
        }
    }

    /**
     * Canonical identity string: the real path itself. The hash is derived
     * from it with SHA-256 and is used only as a compact, stable identifier
     * (for example lock-file naming in later phases). It is not an
     * anonymisation mechanism.
     */
    public static String identityHashFor(Path canonicalRoot) {
        try {
            MessageDigest sha256 = MessageDigest.getInstance("SHA-256");
            byte[] digest = sha256.digest(canonicalRoot.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(digest);
        } catch (java.security.NoSuchAlgorithmException impossible) {
            throw new IllegalStateException("SHA-256 unavailable", impossible);
        }
    }
}

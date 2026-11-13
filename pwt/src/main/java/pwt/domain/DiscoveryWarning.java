package pwt.domain;

import java.util.Objects;

/**
 * A non-fatal discovery observation. Warnings explain reduced coverage; they
 * never pretend coverage is complete. Machine-readable {@code code} values:
 * <ul>
 *   <li>{@code EXCLUDED_DIRECTORY} — a directory was skipped by exclusion rule.</li>
 *   <li>{@code UNREADABLE_PATH} — a path could not be inspected.</li>
 *   <li>{@code SYMLINK_SKIPPED} — a symlinked file or directory was not followed.</li>
 *   <li>{@code DEPTH_LIMIT} — traversal stopped descending past maxDepth.</li>
 *   <li>{@code FILE_LIMIT} — maxDiscoveredFiles reached; result is incomplete.</li>
 *   <li>{@code CANCELLED} — traversal stopped at the caller's request.</li>
 * </ul>
 */
public record DiscoveryWarning(String code, String message, String path) {

    public DiscoveryWarning {
        Objects.requireNonNull(code, "code");
        Objects.requireNonNull(message, "message");
    }

    public static DiscoveryWarning of(String code, String message) {
        return new DiscoveryWarning(code, message, null);
    }

    public static DiscoveryWarning of(String code, String message, String path) {
        return new DiscoveryWarning(code, message, path);
    }
}

package pwt.domain;

import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Objects;
import java.util.Set;

/**
 * Tunable bounds for a single discovery run.
 *
 * <p>Defaults are defensive, not performance targets:
 * <ul>
 *   <li>{@code maxDepth} bounds directory traversal below the root.</li>
 *   <li>{@code maxDiscoveredFiles} bounds the number of returned YAML files.
 *       This is a <em>discovery</em> limit. The later scan-selection limit
 *       (how many selected files one assessment may include) is a separate
 *       concern owned by Phase 2 execution, not this option.</li>
 *   <li>{@code excludedDirectoryNames} holds plain directory names (never
 *       paths) that are skipped wholesale. The default set is
 *       {@code {.git}}: version-control metadata is not assessment input and
 *       skipping it is reported, not silent.</li>
 * </ul>
 */
public final class DiscoveryOptions {

    public static final Set<String> DEFAULT_EXCLUDED_DIRECTORIES = Set.of(".git");

    /**
     * Default discovery bound: at most this many YAML files are returned per
     * run. Reaching it marks the result incomplete (see
     * {@link DiscoveryResult#incomplete()}), it never silently truncates.
     */
    public static final int DEFAULT_MAX_DISCOVERED_FILES = 10_000;

    /** Default traversal depth below the root. */
    public static final int DEFAULT_MAX_DEPTH = 64;

    private final Set<String> excludedDirectoryNames;
    private final int maxDepth;
    private final int maxDiscoveredFiles;

    public DiscoveryOptions(Set<String> excludedDirectoryNames, int maxDepth, int maxDiscoveredFiles) {
        Objects.requireNonNull(excludedDirectoryNames, "excludedDirectoryNames");
        for (String name : excludedDirectoryNames) {
            if (name == null || name.isEmpty() || name.contains("/") || name.contains("\\")
                    || name.equals(".") || name.equals("..")) {
                throw new IllegalArgumentException("exclusions must be plain directory names: " + name);
            }
        }
        if (maxDepth < 1) {
            throw new IllegalArgumentException("maxDepth must be >= 1");
        }
        if (maxDiscoveredFiles < 1) {
            throw new IllegalArgumentException("maxDiscoveredFiles must be >= 1");
        }
        this.excludedDirectoryNames = Collections.unmodifiableSet(new LinkedHashSet<>(excludedDirectoryNames));
        this.maxDepth = maxDepth;
        this.maxDiscoveredFiles = maxDiscoveredFiles;
    }

    public static DiscoveryOptions defaults() {
        return new DiscoveryOptions(DEFAULT_EXCLUDED_DIRECTORIES, DEFAULT_MAX_DEPTH, DEFAULT_MAX_DISCOVERED_FILES);
    }

    public DiscoveryOptions withExcludedDirectoryNames(Set<String> names) {
        return new DiscoveryOptions(names, maxDepth, maxDiscoveredFiles);
    }

    public DiscoveryOptions withMaxDepth(int depth) {
        return new DiscoveryOptions(excludedDirectoryNames, depth, maxDiscoveredFiles);
    }

    public DiscoveryOptions withMaxDiscoveredFiles(int max) {
        return new DiscoveryOptions(excludedDirectoryNames, maxDepth, max);
    }

    public Set<String> excludedDirectoryNames() {
        return excludedDirectoryNames;
    }

    public int maxDepth() {
        return maxDepth;
    }

    public int maxDiscoveredFiles() {
        return maxDiscoveredFiles;
    }
}

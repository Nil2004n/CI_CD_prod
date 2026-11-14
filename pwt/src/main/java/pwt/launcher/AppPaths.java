package pwt.launcher;

import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Single place that answers "which application data directory?".
 *
 * <p>Resolution order, no silent alternatives:
 * <ol>
 *   <li>Explicit {@code pwt.data.dir} system property or {@code --data-dir}
 *       flag (every entrypoint accepts one; tests use isolated temp dirs).</li>
 *   <li>{@code PWT_DATA_DIR} environment variable, which must be absolute
 *       (used by the installed user-wide launcher to persist the configured
 *       data directory).</li>
 *   <li>{@code <application-root>/.pwt-data}, where the application root is
 *       the directory containing the root {@code Makefile} and
 *       {@code integrations/}, resolved from this class's own code source
 *       (works for the built jar and for IDE/test class directories).</li>
 * </ol>
 * <p>If neither applies, startup fails loudly instead of scattering
 * databases across home directories, repositories, or scan working
 * directories. Project scoping filters records inside the one database;
 * it never selects a different database file.
 */
public final class AppPaths {

    private AppPaths() {
    }

    /** Application root: the directory holding the root Makefile. */
    public static Path appRoot() {
        try {
            String location = AppPaths.class.getProtectionDomain().getCodeSource()
                    .getLocation().toURI().getPath();
            Path dir = Path.of(location);
            if (!Files.isDirectory(dir)) {
                dir = dir.getParent();
            }
            for (Path candidate = dir; candidate != null && candidate.getNameCount() > 0;
                    candidate = candidate.getParent()) {
                if (Files.isRegularFile(candidate.resolve("Makefile"))
                        && Files.isExecutable(candidate.resolve("integrations/pwt-mcp-stdio.sh"))) {
                    return candidate.toAbsolutePath().normalize();
                }
                if (candidate.getNameCount() <= 1) {
                    break;
                }
            }
        } catch (Exception e) {
            throw new IllegalStateException(
                    "Cannot resolve the PWT application root (set -Dpwt.data.dir=<dir> "
                            + "or pass --data-dir): " + e.getMessage(), e);
        }
        throw new IllegalStateException(
                "Cannot resolve the PWT application root (no Makefile + integrations/ "
                        + "found above the running code; set -Dpwt.data.dir=<dir> or pass --data-dir)");
    }

    /**
     * Shared data directory for GUI, CLI, and MCP. Explicit system-property
     * override wins, then the {@code PWT_DATA_DIR} environment variable
     * (must be absolute), otherwise {@code <application-root>/.pwt-data}.
     * Never falls back to a per-repository, per-scan, or working-directory
     * location.
     */
    public static Path defaultDataDir() {
        String override = System.getProperty("pwt.data.dir");
        if (override != null && !override.isBlank()) {
            return Path.of(override);
        }
        String env = System.getenv("PWT_DATA_DIR");
        if (env != null && !env.isBlank()) {
            Path configured = Path.of(env);
            if (!configured.isAbsolute()) {
                throw new IllegalStateException("PWT_DATA_DIR must be absolute: " + env);
            }
            return configured;
        }
        return appRoot().resolve(".pwt-data");
    }
}

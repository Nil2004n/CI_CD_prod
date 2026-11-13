package pwt.infrastructure;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Optional;

/**
 * Executable resolution for scanner adapters: an explicit override wins,
 * otherwise the executable is looked up on {@code PATH}.
 *
 * <p>Overrides are supplied as JVM system properties
 * ({@code -Dpwt.tools.<adapterId>=/absolute/path}), which keeps test fixtures
 * hermetic without touching the user environment. Only absolute paths are
 * accepted as overrides.
 */
public final class ExecutableResolver {

    private ExecutableResolver() {
    }

    public static Optional<Path> resolve(String adapterId, String executableName) {
        String override = System.getProperty("pwt.tools." + adapterId);
        if (override != null && !override.isBlank()) {
            Path candidate = Paths.get(override.trim());
            if (candidate.isAbsolute() && Files.isRegularFile(candidate)) {
                return Optional.of(candidate);
            }
            return Optional.empty();
        }
        String pathEnv = System.getenv("PATH");
        if (pathEnv == null || pathEnv.isBlank()) {
            return Optional.empty();
        }
        for (String dir : pathEnv.split(java.util.regex.Pattern.quote(java.io.File.pathSeparator))) {
            if (dir.isBlank()) {
                continue;
            }
            Path candidate = Paths.get(dir, executableName);
            if (Files.isRegularFile(candidate) && Files.isExecutable(candidate)) {
                return Optional.of(candidate);
            }
        }
        return Optional.empty();
    }
}

package pwt.domain;

import java.util.Objects;

/**
 * One YAML file found beneath a registered root.
 *
 * <p>{@code relativePath} uses forward slashes regardless of platform and is
 * relative to the repository root. {@code sizeBytes} comes from file
 * attributes only; Phase 1 discovery never reads file contents.
 */
public record DiscoveredYamlFile(String relativePath, long sizeBytes) {

    public DiscoveredYamlFile {
        Objects.requireNonNull(relativePath, "relativePath");
        if (relativePath.isEmpty() || relativePath.startsWith("/") || relativePath.contains("\\")
                || relativePath.equals(".") || relativePath.startsWith("./")
                || relativePath.contains("//")) {
            throw new IllegalArgumentException("relativePath must be a clean root-relative forward-slash path: "
                    + relativePath);
        }
        if (sizeBytes < 0) {
            throw new IllegalArgumentException("sizeBytes must be >= 0");
        }
    }
}

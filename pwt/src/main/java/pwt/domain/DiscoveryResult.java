package pwt.domain;

import java.util.Collections;
import java.util.List;
import java.util.Objects;

/**
 * Outcome of one discovery run: sorted root-relative YAML paths plus the
 * warnings that qualify coverage.
 *
 * <p>{@code incomplete} is true when a bound (file count, depth,
 * cancellation) stopped traversal early. Consumers must surface that state;
 * an incomplete result must never be presented as full coverage.
 */
public record DiscoveryResult(
        RepositoryRoot root,
        List<DiscoveredYamlFile> files,
        List<DiscoveryWarning> warnings,
        boolean incomplete) {

    public DiscoveryResult {
        Objects.requireNonNull(root, "root");
        Objects.requireNonNull(files, "files");
        Objects.requireNonNull(warnings, "warnings");
        files = Collections.unmodifiableList(List.copyOf(files));
        warnings = Collections.unmodifiableList(List.copyOf(warnings));
    }

    public int discoveredCount() {
        return files.size();
    }
}

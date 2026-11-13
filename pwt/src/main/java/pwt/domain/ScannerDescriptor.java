package pwt.domain;

import java.util.Objects;

/**
 * Static metadata for one planned scanner/validation integration.
 * Phase 1 describes intent only; {@link #execution()} is always
 * {@link ExecutionState#NOT_IMPLEMENTED}.
 */
public record ScannerDescriptor(String id, String toolName, String purpose, ExecutionState execution) {

    public ScannerDescriptor {
        Objects.requireNonNull(id, "id");
        Objects.requireNonNull(toolName, "toolName");
        Objects.requireNonNull(purpose, "purpose");
        Objects.requireNonNull(execution, "execution");
    }
}

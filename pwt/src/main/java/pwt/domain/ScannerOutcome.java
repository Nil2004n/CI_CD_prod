package pwt.domain;

import java.util.List;
import java.util.Objects;

/** Outcome of one adapter execution within a run. */
public record ScannerOutcome(
        String adapterId,
        String toolName,
        String toolVersion,
        ScannerStatus status,
        Integer exitCode,
        String detail,
        List<Finding> findings,
        List<String> coverageGaps) {

    public ScannerOutcome {
        Objects.requireNonNull(adapterId, "adapterId");
        Objects.requireNonNull(toolName, "toolName");
        Objects.requireNonNull(status, "status");
        findings = findings == null ? List.of() : List.copyOf(findings);
        coverageGaps = coverageGaps == null ? List.of() : List.copyOf(coverageGaps);
    }

    /** Convenience for outcomes without coverage gaps. */
    public static ScannerOutcome withoutGaps(String adapterId, String toolName, String toolVersion,
            ScannerStatus status, Integer exitCode, String detail, List<Finding> findings) {
        return new ScannerOutcome(adapterId, toolName, toolVersion, status, exitCode, detail, findings, List.of());
    }
}

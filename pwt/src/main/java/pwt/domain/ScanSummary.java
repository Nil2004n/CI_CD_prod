package pwt.domain;

import java.util.List;
import java.util.Objects;

/** Immutable summary of one finished (or terminally failed) scan session. */
public record ScanSummary(
        String runId,
        String repositoryDisplayName,
        String canonicalRoot,
        ScanState state,
        ScanVerdict verdict,
        List<SelectedTarget> targets,
        List<ScannerOutcome> scannerOutcomes,
        int findingCount,
        String jsonReportPath,
        String markdownReportPath,
        String startedAt,
        String finishedAt,
        List<String> coverageGaps) {

    public ScanSummary {
        Objects.requireNonNull(runId, "runId");
        Objects.requireNonNull(state, "state");
        Objects.requireNonNull(verdict, "verdict");
        targets = targets == null ? List.of() : List.copyOf(targets);
        scannerOutcomes = scannerOutcomes == null ? List.of() : List.copyOf(scannerOutcomes);
        coverageGaps = coverageGaps == null ? List.of() : List.copyOf(coverageGaps);
    }

    public record SelectedTarget(String relativePath, String sha256, long sizeBytes) {
    }
}

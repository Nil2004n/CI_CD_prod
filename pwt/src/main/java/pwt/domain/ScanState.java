package pwt.domain;

/** Terminal-aware run execution states (subset used in Phase 2). */
public enum ScanState {
    CREATED,
    RUNNING,
    COMPLETED,
    PARTIAL,
    FAILED,
    CANCELLED,
    INTERRUPTED
}

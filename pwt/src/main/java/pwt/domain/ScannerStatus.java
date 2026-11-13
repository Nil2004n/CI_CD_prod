package pwt.domain;

/** Per-scanner outcome states. NOT_APPLICABLE means the adapter determined
 * none of the selected targets falls in its scope; it is neither success nor
 * failure and does not by itself block a NO_FINDINGS verdict. */
public enum ScannerStatus {
    PENDING,
    RUNNING,
    SUCCEEDED,
    FAILED,
    SKIPPED,
    NOT_APPLICABLE,
    UNAVAILABLE,
    TIMED_OUT,
    CANCELLED,
    INTERRUPTED
}

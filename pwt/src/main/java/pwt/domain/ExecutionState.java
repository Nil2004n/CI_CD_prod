package pwt.domain;

/**
 * Integration state of one catalog entry.
 * NOT_IMPLEMENTED: no adapter exists; never claim execution or availability.
 * INTEGRATED: a Java adapter exists; actual availability depends on a local
 * executable probe performed at scan time (GUI/CLI), never assumed.
 */
public enum ExecutionState {
    NOT_IMPLEMENTED,
    INTEGRATED
}

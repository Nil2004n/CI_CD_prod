package pwt.domain;

/**
 * Assessment verdict, kept separate from execution state.
 * NO_FINDINGS requires complete coverage of the selected scope; any observed
 * finding yields FINDINGS even on partial coverage; otherwise INDETERMINATE.
 */
public enum ScanVerdict {
    FINDINGS,
    NO_FINDINGS,
    INDETERMINATE
}

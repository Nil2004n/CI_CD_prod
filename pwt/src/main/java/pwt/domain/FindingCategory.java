package pwt.domain;

/**
 * Finding categories. SECRET, MISCONFIGURATION, SYNTAX, and SCHEMA follow the
 * storage contract; LINT is the documented Phase 2 extension for
 * style/diagnostic findings (for example yamllint rule hits) that are not
 * confirmed vulnerabilities or syntax errors.
 */
public enum FindingCategory {
    SECRET,
    MISCONFIGURATION,
    SYNTAX,
    SCHEMA,
    LINT
}

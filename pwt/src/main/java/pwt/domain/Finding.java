package pwt.domain;

import java.util.Objects;

/**
 * One normalized, sanitized finding. Never carries secret values, unredacted
 * matches, or raw source excerpts: adapters drop those fields before
 * constructing findings.
 */
public record Finding(
        String ruleId,
        String title,
        Severity severity,
        String originalSeverity,
        FindingCategory category,
        String relativePath,
        Integer lineStart,
        Integer lineEnd,
        Integer columnStart,
        String message,
        String evidenceJson,
        String fingerprint) {

    public Finding {
        Objects.requireNonNull(ruleId, "ruleId");
        Objects.requireNonNull(title, "title");
        Objects.requireNonNull(severity, "severity");
        Objects.requireNonNull(category, "category");
        Objects.requireNonNull(message, "message");
        Objects.requireNonNull(fingerprint, "fingerprint");
        if (evidenceJson == null) {
            evidenceJson = "{}";
        }
    }
}

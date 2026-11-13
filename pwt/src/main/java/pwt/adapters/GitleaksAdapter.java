package pwt.adapters;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.Fingerprints;
import pwt.domain.ScannerOutcome;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.domain.ToolProbe;
import pwt.infrastructure.ExecutableResolver;
import pwt.infrastructure.ProcessRunner;

/**
 * Gitleaks adapter (verified against Gitleaks 8.30.1).
 *
 * <p>Probe: {@code gitleaks version} → {@code 8.30.1}.
 * Run (file-content scan of the staged selection, never Git history):
 * {@code gitleaks detect --no-git --no-banner --no-color --redact
 * -s <staging> -f json -r <private report> --exit-code 7 --log-level error}.
 *
 * <p>Exit codes with {@code --exit-code 7}: 0 = executed, no leaks; 7 =
 * executed, leaks found and written to the report; anything else (observed 1
 * for unreadable sources) = execution failure.
 *
 * <p>Secret hygiene: {@code --redact} is one layer only. The adapter drops
 * the {@code Secret} and {@code Match} fields entirely and persists only
 * rule metadata, location, entropy, and tags. {@code GITLEAKS_CONFIG} /
 * {@code GITLEAKS_CONFIG_TOML} are removed from the child environment so a
 * repository or user config cannot silently change the rule set; the default
 * embedded rules apply and the tool version is recorded.
 *
 * <p>Scope: report entries whose file does not name a selected staged target
 * are skipped and reported as coverage gaps, never silently absorbed.
 */
public final class GitleaksAdapter implements ScannerAdapter {

    public static final String ID = "gitleaks";
    private static final int FINDINGS_EXIT_CODE = 7;
    private static final Set<String> CONFIG_ENV_BLOCKLIST = Set.of("GITLEAKS_CONFIG", "GITLEAKS_CONFIG_TOML");
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "Gitleaks";
    }

    @Override
    public ToolProbe probe() {
        Optional<Path> executable = ExecutableResolver.resolve(ID, "gitleaks");
        if (executable.isEmpty()) {
            return ToolProbe.unavailable("gitleaks executable not found on PATH"
                    + " (override with -Dpwt.tools.gitleaks=/absolute/path)");
        }
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.get().toString(), "version"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() != 0) {
                return ToolProbe.unavailable("gitleaks version exited with " + result.exitCode());
            }
            return ToolProbe.available(executable.get().toString(), result.stdout().strip());
        } catch (Exception e) {
            return ToolProbe.unavailable("gitleaks could not start: " + e.getMessage());
        }
    }

    @Override
    public ScannerOutcome execute(ScanContext context) {
        Path executable = context.executableOverride() != null
                ? context.executableOverride()
                : ExecutableResolver.resolve(ID, "gitleaks").orElse(null);
        if (executable == null) {
            return statusOnly(ScannerStatus.UNAVAILABLE, null, "gitleaks executable not found on PATH");
        }
        Path reportFile = context.stagedDir().resolveSibling("gitleaks-report.json");
        List<String> command = List.of(
                executable.toString(), "detect",
                "--no-git", "--no-banner", "--no-color", "--redact",
                "-s", context.stagedDir().toString(),
                "-f", "json", "-r", reportFile.toString(),
                "--exit-code", String.valueOf(FINDINGS_EXIT_CODE),
                "--log-level", "error");

        ProcessRunner.ProcessResult result;
        try {
            result = ProcessRunner.run(command, null, null, CONFIG_ENV_BLOCKLIST, context.timeout(),
                    2 * 1024 * 1024, 2 * 1024 * 1024, context.cancellation());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return statusOnly(ScannerStatus.INTERRUPTED, null, "gitleaks run interrupted");
        } catch (Exception e) {
            return failure(null, "gitleaks could not start: " + e.getMessage());
        }
        if (result.cancelled()) {
            return statusOnly(ScannerStatus.CANCELLED, null, "gitleaks run cancelled");
        }
        if (result.timedOut()) {
            return statusOnly(ScannerStatus.TIMED_OUT, null,
                    "gitleaks exceeded its time budget and was terminated");
        }
        String version = queryVersion(executable);
        if (result.exitCode() == 0) {
            return ScannerOutcome.withoutGaps(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                    0, "No leaks found", List.of());
        }
        if (result.exitCode() != FINDINGS_EXIT_CODE) {
            String detail = firstLine(result.stderr());
            if (detail.isEmpty()) {
                detail = firstLine(result.stdout());
            }
            return failure(version, "gitleaks failed with exit code " + result.exitCode()
                    + (detail.isEmpty() ? "" : ": " + detail));
        }
        ParseResult parsed;
        try {
            String report = Files.readString(reportFile);
            parsed = parseReport(report, context.stagedDir(), new HashSet<>(context.relativeTargets()));
        } catch (Exception e) {
            return failure(version, "gitleaks reported findings but its report could not be read/parsed: "
                    + e.getMessage());
        }
        String detail = parsed.findings().isEmpty() ? "No in-scope leaks found"
                : parsed.findings().size() + " leak(s) in selected files";
        if (parsed.skippedOutOfScope() > 0) {
            detail += "; " + parsed.skippedOutOfScope()
                    + " report entr(y/ies) outside the selected files were skipped (coverage gap)";
        }
        List<String> outcomeGaps = parsed.skippedOutOfScope() > 0
                ? List.of(parsed.skippedOutOfScope() + " report entr(y/ies) outside the selected files were skipped")
                : List.of();
        return new ScannerOutcome(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                result.exitCode(), detail, parsed.findings(), outcomeGaps);
    }

    private String queryVersion(Path executable) {
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.toString(), "version"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() == 0) {
                String version = result.stdout().strip();
                return version.isEmpty() ? "unknown" : version;
            }
        } catch (Exception ignored) {
            // best effort; version stays unknown
        }
        return "unknown";
    }

    /** Parsed report: in-scope findings plus the count of skipped out-of-scope entries. */
    public record ParseResult(List<Finding> findings, int skippedOutOfScope) {
    }

    /**
     * Parses a Gitleaks JSON report. {@code Secret} and {@code Match} are
     * deliberately dropped — only rule metadata, location, entropy, and tags
     * survive. Entries outside {@code selectedTargets} are skipped.
     *
     * <p>{@code File} values are made relative to {@code stagedDir} when they
     * are absolute paths beneath it (Gitleaks reports absolute paths when
     * its source argument is absolute); otherwise forward-slash normalization
     * applies.
     */
    public static ParseResult parseReport(String reportJson, Path stagedDir, Set<String> selectedTargets)
            throws Exception {
        List<Finding> findings = new ArrayList<>();
        int skipped = 0;
        JsonNode root = MAPPER.readTree(reportJson);
        if (!root.isArray()) {
            throw new IllegalArgumentException("Gitleaks report is not a JSON array");
        }
        for (JsonNode entry : root) {
            String file = textOrNull(entry, "File");
            String normalized = relativizeReportPath(file, stagedDir);
            if (normalized == null || !selectedTargets.contains(normalized)) {
                skipped++;
                continue;
            }
            String ruleId = textOrDefault(entry, "RuleID", "unknown-rule");
            String description = textOrDefault(entry, "Description", "Possible secret");
            Integer startLine = intOrNull(entry, "StartLine");
            Integer endLine = intOrNull(entry, "EndLine");
            Integer startColumn = intOrNull(entry, "StartColumn");
            double entropy = entry.path("Entropy").asDouble(Double.NaN);
            List<String> tags = new ArrayList<>();
            JsonNode tagsNode = entry.path("Tags");
            if (tagsNode.isArray()) {
                for (JsonNode tag : tagsNode) {
                    if (tag.isTextual()) {
                        tags.add(tag.asText());
                    }
                }
            }
            String toolFingerprint = textOrNull(entry, "Fingerprint");
            StringBuilder evidence = new StringBuilder("{");
            if (!Double.isNaN(entropy)) {
                evidence.append("\"entropy\":").append(entropy).append(',');
            }
            evidence.append("\"tags\":[");
            for (int i = 0; i < tags.size(); i++) {
                if (i > 0) {
                    evidence.append(',');
                }
                evidence.append('"').append(escape(tags.get(i))).append('"');
            }
            evidence.append(']');
            if (toolFingerprint != null) {
                evidence.append(",\"tool_fingerprint\":\"").append(escape(toolFingerprint)).append('"');
            }
            evidence.append('}');
            String message = description + " at " + normalized
                    + (startLine == null ? "" : ":" + startLine);
            String fingerprint = Fingerprints.v1(ID, ruleId, normalized, startLine, startColumn);
            findings.add(new Finding("gitleaks/" + ruleId, description,
                    Severity.HIGH, "leak", FindingCategory.SECRET,
                    normalized, startLine, endLine, startColumn, message,
                    evidence.toString(), fingerprint));
        }
        return new ParseResult(List.copyOf(findings), skipped);
    }

    /** Report paths as written by Gitleaks without a staged-directory context (tests). */
    public static ParseResult parseReport(String reportJson, Set<String> selectedTargets) throws Exception {
        return parseReport(reportJson, null, selectedTargets);
    }

    private static String relativizeReportPath(String file, Path stagedDir) {
        if (file == null) {
            return null;
        }
        String forward = file.replace('\\', '/');
        if (stagedDir != null) {
            try {
                Path reported = Path.of(forward);
                if (reported.isAbsolute()) {
                    Path relative = stagedDir.normalize().relativize(reported.normalize());
                    String rel = relative.toString().replace('\\', '/');
                    if (!rel.isEmpty() && !rel.startsWith("..")) {
                        return rel;
                    }
                    return null;
                }
            } catch (IllegalArgumentException notRelative) {
                return null;
            }
        }
        String normalized = forward;
        while (normalized.startsWith("/")) {
            normalized = normalized.substring(1);
        }
        return normalized;
    }

    private static String textOrNull(JsonNode node, String field) {
        JsonNode child = node.path(field);
        return child.isTextual() ? child.asText() : null;
    }

    private static String textOrDefault(JsonNode node, String field, String fallback) {
        String value = textOrNull(node, field);
        return value == null || value.isBlank() ? fallback : value;
    }

    private static Integer intOrNull(JsonNode node, String field) {
        JsonNode child = node.path(field);
        return child.isInt() ? child.asInt() : null;
    }

    private static String escape(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    private ScannerOutcome failure(String version, String detail) {
        return statusOnly(ScannerStatus.FAILED, version, detail);
    }

    private ScannerOutcome statusOnly(ScannerStatus status, String version, String detail) {
        return ScannerOutcome.withoutGaps(ID, toolName(), version, status, null, detail, List.of());
    }

    private static String firstLine(String text) {
        if (text == null) {
            return "";
        }
        int newline = text.indexOf('\n');
        String first = (newline == -1 ? text : text.substring(0, newline)).strip();
        return first.length() > 300 ? first.substring(0, 300) : first;
    }
}

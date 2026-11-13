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
 * Checkov adapter (verified against Checkov 3.4.4), framework scope:
 * {@code kubernetes} only.
 *
 * <p>Probe: {@code checkov --version} → {@code 3.4.4}.
 * Run: {@code checkov -f <applicable staged files...> --framework kubernetes
 * -o json --quiet} with the staging directory as working directory, so only
 * explicitly selected applicable files are scanned — never Git history or
 * the whole repository. No cloud auth, no external checks, no uploads, no
 * repository configuration loading (the snapshot contains only selected
 * files).
 *
 * <p>Exit codes (verified): 0 = executed, no failed checks; 1 = executed,
 * failed checks present; anything else, or output that is not a Checkov
 * report envelope, = execution failure.
 *
 * <p>Severity: preserved when the tool reports one, otherwise UNKNOWN
 * (Checkov's Kubernetes checks do not carry severities by default; the
 * adapter does not invent them from a generic FAILED label). Passed checks
 * produce no findings; skipped checks and their reasons are recorded in the
 * outcome detail.
 */
public final class CheckovAdapter implements ScannerAdapter {

    public static final String ID = "checkov";
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "Checkov";
    }

    @Override
    public ToolProbe probe() {
        Optional<Path> executable = ExecutableResolver.resolve(ID, "checkov");
        if (executable.isEmpty()) {
            return ToolProbe.unavailable("checkov executable not found on PATH"
                    + " (override with -Dpwt.tools.checkov=/absolute/path)");
        }
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.get().toString(), "--version"), null, null,
                    Duration.ofSeconds(60), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() != 0) {
                return ToolProbe.unavailable("checkov --version exited with " + result.exitCode());
            }
            return ToolProbe.available(executable.get().toString(), result.stdout().strip());
        } catch (Exception e) {
            return ToolProbe.unavailable("checkov could not start: " + e.getMessage());
        }
    }

    /** Kubernetes-manifest applicability: content sniff, not extension. */
    public static List<String> applicableTargets(Path stagedDir, List<String> relativeTargets) {
        List<String> applicable = new ArrayList<>();
        for (String relative : relativeTargets) {
            if (YamlSniff.isKubernetesManifest(stagedDir.resolve(relative))) {
                applicable.add(relative);
            }
        }
        return applicable;
    }

    @Override
    public ScannerOutcome execute(ScanContext context) {
        Path executable = context.executableOverride() != null
                ? context.executableOverride()
                : ExecutableResolver.resolve(ID, "checkov").orElse(null);
        if (executable == null) {
            return statusOnly(ScannerStatus.UNAVAILABLE, null, "checkov executable not found on PATH");
        }
        List<String> applicable = applicableTargets(context.stagedDir(), context.relativeTargets());
        if (applicable.isEmpty()) {
            return ScannerOutcome.withoutGaps(ID, toolName(), queryVersion(executable),
                    ScannerStatus.NOT_APPLICABLE, null,
                    "No selected file is a recognized Kubernetes manifest; checkov did not run.", List.of());
        }
        List<String> command = new ArrayList<>();
        command.add(executable.toString());
        for (String target : applicable) {
            command.add("-f");
            command.add(target);
        }
        command.add("--framework");
        command.add("kubernetes");
        command.add("-o");
        command.add("json");
        command.add("--quiet");

        ProcessRunner.ProcessResult result;
        try {
            result = ProcessRunner.run(command, context.stagedDir(), null, context.timeout(),
                    10 * 1024 * 1024, 2 * 1024 * 1024, context.cancellation());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return statusOnly(ScannerStatus.INTERRUPTED, null, "checkov run interrupted");
        } catch (Exception e) {
            return failure(null, "checkov could not start: " + e.getMessage());
        }
        if (result.cancelled()) {
            return statusOnly(ScannerStatus.CANCELLED, null, "checkov run cancelled");
        }
        if (result.timedOut()) {
            return statusOnly(ScannerStatus.TIMED_OUT, null,
                    "checkov exceeded its time budget and was terminated");
        }
        String version = queryVersion(executable);
        if (result.exitCode() != 0 && result.exitCode() != 1) {
            return failure(version, "checkov failed with exit code " + result.exitCode() + ": "
                    + firstLine(result.stderr().isEmpty() ? result.stdout() : result.stderr()));
        }
        ParseResult parsed;
        try {
            parsed = parseReport(result.stdout(), new HashSet<>(context.relativeTargets()));
        } catch (Exception e) {
            return failure(version, "checkov output was not a parseable report: " + e.getMessage());
        }
        String detail = parsed.findings().size() + " failed check(s), " + parsed.passed() + " passed, "
                + parsed.skipped() + " skipped across " + applicable.size() + " applicable file(s)";
        if (parsed.skipped() > 0) {
            detail += " (" + parsed.skipped() + " skipped: " + parsed.skipReasons() + ")";
        }
        List<String> outcomeGaps = parsed.skipped() > 0
                ? List.of(parsed.skipped() + " checkov check(s) skipped: " + parsed.skipReasons())
                : List.of();
        return new ScannerOutcome(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                result.exitCode(), detail, parsed.findings(), outcomeGaps);
    }

    private String queryVersion(Path executable) {
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.toString(), "--version"), null, null,
                    Duration.ofSeconds(60), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() == 0) {
                String version = result.stdout().strip();
                return version.isEmpty() ? "unknown" : version;
            }
        } catch (Exception ignored) {
            // best effort; version stays unknown
        }
        return "unknown";
    }

    public record ParseResult(List<Finding> findings, int passed, int skipped, String skipReasons) {
    }

    /**
     * Parses a Checkov JSON report. Only failed checks for in-scope files
     * become findings; unknown shapes are rejected rather than guessed.
     */
    public static ParseResult parseReport(String reportJson, Set<String> selectedTargets) throws Exception {
        JsonNode root = MAPPER.readTree(reportJson);
        if (!root.isObject() || !root.path("check_type").isTextual() || !root.path("results").isObject()) {
            throw new IllegalArgumentException("Not a Checkov JSON report envelope");
        }
        if (!"kubernetes".equals(root.path("check_type").asText())) {
            throw new IllegalArgumentException(
                    "Unsupported Checkov check_type: " + root.path("check_type").asText());
        }
        List<Finding> findings = new ArrayList<>();
        JsonNode failed = root.path("results").path("failed_checks");
        if (!failed.isArray()) {
            throw new IllegalArgumentException("Checkov report has no failed_checks array");
        }
        for (JsonNode check : failed) {
            String repoPath = check.path("repo_file_path").asText(null);
            String normalized = normalizeRepoPath(repoPath);
            if (normalized == null || !selectedTargets.contains(normalized)) {
                continue;
            }
            String checkId = check.path("check_id").asText("unknown-check");
            String title = check.path("check_name").asText(checkId);
            String severityName = check.path("severity").asText(null);
            Severity severity = mapSeverity(severityName);
            String resource = check.path("resource").asText(null);
            Integer lineStart = null;
            Integer lineEnd = null;
            JsonNode range = check.path("file_line_range");
            if (range.isArray() && range.size() >= 1 && range.get(0).isInt()) {
                lineStart = range.get(0).asInt();
            }
            if (range.isArray() && range.size() >= 2 && range.get(1).isInt()) {
                lineEnd = range.get(1).asInt();
            }
            StringBuilder message = new StringBuilder(title);
            if (resource != null && !resource.isBlank()) {
                message.append(" on ").append(resource);
            }
            message.append(" at ").append(normalized);
            if (lineStart != null) {
                message.append(":").append(lineStart);
            }
            StringBuilder evidence = new StringBuilder("{\"check_id\":\"").append(escape(checkId)).append('"');
            String guideline = check.path("guideline").asText(null);
            if (guideline != null && !guideline.isBlank()) {
                evidence.append(",\"guideline\":\"").append(escape(guideline)).append('"');
            }
            JsonNode evaluated = check.path("check_result").path("evaluated_keys");
            if (evaluated.isArray() && !evaluated.isEmpty()) {
                evidence.append(",\"evaluated_keys\":[");
                boolean first = true;
                for (JsonNode key : evaluated) {
                    if (key.isTextual()) {
                        if (!first) {
                            evidence.append(',');
                        }
                        evidence.append('"').append(escape(key.asText())).append('"');
                        first = false;
                    }
                }
                evidence.append(']');
            }
            evidence.append('}');
            String fingerprint = Fingerprints.v1(ID, checkId, normalized, lineStart, null);
            findings.add(new Finding("checkov/" + checkId, title, severity,
                    severityName, FindingCategory.MISCONFIGURATION, normalized,
                    lineStart, lineEnd, null, message.toString(), evidence.toString(), fingerprint));
        }
        JsonNode summary = root.path("summary");
        int passed = summary.path("passed").asInt(0);
        int skipped = summary.path("skipped").asInt(0);
        List<String> reasons = new ArrayList<>();
        JsonNode skippedChecks = root.path("results").path("skipped_checks");
        if (skippedChecks.isArray()) {
            for (JsonNode check : skippedChecks) {
                String reason = check.path("suppress_comment").asText(
                        check.path("check_id").asText("unknown"));
                if (!reasons.contains(reason) && reasons.size() < 5) {
                    reasons.add(reason);
                }
            }
        }
        return new ParseResult(List.copyOf(findings), passed, skipped, String.join("; ", reasons));
    }

    public static String normalizeRepoPath(String repoPath) {
        if (repoPath == null || repoPath.isBlank()) {
            return null;
        }
        String forward = repoPath.replace('\\', '/');
        while (forward.startsWith("/")) {
            forward = forward.substring(1);
        }
        return forward.isEmpty() ? null : forward;
    }

    public static Severity mapSeverity(String severityName) {
        if (severityName == null || severityName.isBlank() || severityName.equalsIgnoreCase("null")) {
            return Severity.UNKNOWN;
        }
        return switch (severityName.trim().toUpperCase()) {
            case "CRITICAL" -> Severity.CRITICAL;
            case "HIGH" -> Severity.HIGH;
            case "MEDIUM", "MODERATE" -> Severity.MEDIUM;
            case "LOW", "MINOR" -> Severity.LOW;
            case "INFO", "INFORMATIONAL" -> Severity.INFO;
            default -> Severity.UNKNOWN;
        };
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

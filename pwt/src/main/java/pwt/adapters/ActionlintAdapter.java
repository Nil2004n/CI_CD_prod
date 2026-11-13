package pwt.adapters;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
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
 * actionlint adapter (verified against actionlint 1.7.12).
 *
 * <p>Probe: {@code actionlint -version}.
 * Run: {@code actionlint -format '{{json .}}' -shellcheck= -pyflakes=
 * <relative staged workflow files...>} with the staging directory as working
 * directory. Disabling the optional shellcheck/pyflakes helpers is
 * deliberate for this first integration: helper availability is reported in
 * the probe detail instead of silently changing results.
 *
 * <p>Applicability: repository-relative paths under
 * {@code .github/workflows/} with a {@code .yml}/{@code .yaml} extension
 * (case-insensitive). Generic YAML and Kubernetes manifests never run
 * actionlint. Workflow commands are linted, never executed.
 *
 * <p>Exit codes (verified): 0 = executed, no problems ({@code []}); 1 =
 * executed, problems found; anything else = execution failure.
 *
 * <p>Secret hygiene: the {@code snippet} field (raw source excerpt) is
 * dropped; only rule kind, message, location, and file survive.
 */
public final class ActionlintAdapter implements ScannerAdapter {

    public static final String ID = "actionlint";
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "actionlint";
    }

    @Override
    public ToolProbe probe() {
        Optional<Path> executable = ExecutableResolver.resolve(ID, "actionlint");
        if (executable.isEmpty()) {
            return ToolProbe.unavailable("actionlint executable not found on PATH"
                    + " (override with -Dpwt.tools.actionlint=/absolute/path)");
        }
        String version = queryVersion(executable.get());
        if (version == null) {
            return ToolProbe.unavailable("actionlint -version exited abnormally");
        }
        String helpers = probeHelpers(executable.get());
        return ToolProbe.available(executable.get().toString(), version + helpers);
    }

    private String queryVersion(Path executable) {
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.toString(), "-version"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() != 0) {
                return null;
            }
            // First line is like "v1.7.12\ninstalled by building from source".
            String first = result.stdout().strip().split("\\R", 2)[0].strip();
            return first.isEmpty() ? "unknown" : first;
        } catch (Exception e) {
            return null;
        }
    }

    private static String probeHelpers(Path executable) {
        String shellcheck = ExecutableResolver.resolve("shellcheck", "shellcheck").isPresent()
                ? "present" : "absent";
        String pyflakes = ExecutableResolver.resolve("pyflakes", "pyflakes").isPresent()
                ? "present" : "absent";
        return " (helpers disabled for this integration; shellcheck " + shellcheck
                + ", pyflakes " + pyflakes + ")";
    }

    /** Workflow applicability: files directly in the repository's
     * {@code .github/workflows/} directory with a {@code .yml}/{@code .yaml}
     * extension (case-insensitive). Subdirectories are not valid workflow
     * locations (GitHub ignores them); generic YAML and Kubernetes manifests
     * never run actionlint. Workflow commands are linted, never executed. */
    public static List<String> applicableTargets(List<String> relativeTargets) {
        List<String> applicable = new ArrayList<>();
        for (String relative : relativeTargets) {
            String lower = relative.toLowerCase(Locale.ROOT);
            boolean yaml = lower.endsWith(".yml") || lower.endsWith(".yaml");
            if (!yaml || !lower.startsWith(".github/workflows/")) {
                continue;
            }
            String rest = relative.substring(".github/workflows/".length());
            if (!rest.isEmpty() && rest.indexOf('/') == -1 && rest.indexOf('\\') == -1) {
                applicable.add(relative);
            }
        }
        return applicable;
    }

    @Override
    public ScannerOutcome execute(ScanContext context) {
        Path executable = context.executableOverride() != null
                ? context.executableOverride()
                : ExecutableResolver.resolve(ID, "actionlint").orElse(null);
        if (executable == null) {
            return statusOnly(ScannerStatus.UNAVAILABLE, null, "actionlint executable not found on PATH");
        }
        List<String> applicable = applicableTargets(context.relativeTargets());
        if (applicable.isEmpty()) {
            return ScannerOutcome.withoutGaps(ID, toolName(), queryVersion(executable),
                    ScannerStatus.NOT_APPLICABLE, null, "No selected file is a GitHub Actions workflow "
                            + "(.github/workflows/*.yml|yaml); actionlint did not run.", List.of());
        }
        List<String> command = new ArrayList<>();
        command.add(executable.toString());
        command.add("-format");
        command.add("{{json .}}");
        command.add("-shellcheck=");
        command.add("-pyflakes=");
        command.addAll(applicable);

        ProcessRunner.ProcessResult result;
        try {
            result = ProcessRunner.run(command, context.stagedDir(), null, context.timeout(),
                    10 * 1024 * 1024, 2 * 1024 * 1024, context.cancellation());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return statusOnly(ScannerStatus.INTERRUPTED, null, "actionlint run interrupted");
        } catch (Exception e) {
            return failure(null, "actionlint could not start: " + e.getMessage());
        }
        if (result.cancelled()) {
            return statusOnly(ScannerStatus.CANCELLED, null, "actionlint run cancelled");
        }
        if (result.timedOut()) {
            return statusOnly(ScannerStatus.TIMED_OUT, null,
                    "actionlint exceeded its time budget and was terminated");
        }
        String version = queryVersion(executable);
        if (result.exitCode() != 0 && result.exitCode() != 1) {
            return failure(version, "actionlint failed with exit code " + result.exitCode() + ": "
                    + firstLine(result.stderr().isEmpty() ? result.stdout() : result.stderr()));
        }
        List<Finding> findings;
        try {
            findings = parseReport(result.stdout(), new java.util.HashSet<>(context.relativeTargets()));
        } catch (Exception e) {
            return failure(version, "actionlint output was not parseable: " + e.getMessage());
        }
        String detail = findings.isEmpty() ? "No actionlint problems in " + applicable.size() + " workflow(s)"
                : findings.size() + " problem(s) in " + applicable.size() + " workflow(s)";
        return ScannerOutcome.withoutGaps(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                result.exitCode(), detail, findings);
    }

    /**
     * Parses actionlint {@code -format '{{json .}}'} output (a JSON array).
     * The {@code snippet} source excerpt is dropped for secret hygiene.
     */
    public static List<Finding> parseReport(String reportJson, java.util.Set<String> selectedTargets)
            throws Exception {
        JsonNode root = MAPPER.readTree(reportJson);
        if (!root.isArray()) {
            throw new IllegalArgumentException("actionlint output is not a JSON array");
        }
        List<Finding> findings = new ArrayList<>();
        for (JsonNode problem : root) {
            String file = problem.path("filepath").asText(null);
            String normalized = file == null ? null : file.replace('\\', '/');
            if (normalized == null || !selectedTargets.contains(normalized)) {
                continue;
            }
            String kind = problem.path("kind").asText("check");
            String message = problem.path("message").asText(kind);
            Integer line = problem.path("line").isInt() ? problem.path("line").asInt() : null;
            Integer column = problem.path("column").isInt() ? problem.path("column").asInt() : null;
            boolean syntax = kind.equals("syntax-check");
            Severity severity = syntax ? Severity.MEDIUM : Severity.LOW;
            FindingCategory category = syntax ? FindingCategory.SYNTAX : FindingCategory.LINT;
            String text = message + " at " + normalized + (line == null ? "" : ":" + line);
            String fingerprint = Fingerprints.v1(ID, kind, normalized, line, column);
            findings.add(new Finding("actionlint/" + kind, kind, severity, kind, category,
                    normalized, line, line, column, text,
                    "{\"kind\":\"" + escape(kind) + "\"}", fingerprint));
        }
        return findings;
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

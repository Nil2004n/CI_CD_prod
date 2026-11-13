package pwt.adapters;

import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.Fingerprints;
import pwt.domain.ScanSummary;
import pwt.domain.ScannerOutcome;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.domain.ToolProbe;
import pwt.infrastructure.ExecutableResolver;
import pwt.infrastructure.ProcessRunner;

/**
 * yamllint adapter (verified against yamllint 1.38.0).
 *
 * <p>Probe: {@code yamllint -v} → {@code yamllint 1.38.0}.
 * Run: {@code yamllint -f parsable -c <app-owned config> <relative files...>}
 * with the staging directory as working directory.
 *
 * <p>Exit codes (non-strict mode, verified): 0 = executed, no error-level
 * problems (warnings may still be parsed as findings); 1 = executed,
 * error-level problems found; anything else (observed 255 for unreadable
 * paths) = execution failure. Both 0 and 1 are parsed for findings.
 *
 * <p>Severity mapping (lint diagnostics are hygiene, not confirmed
 * vulnerabilities): {@code syntax} rule → SYNTAX/MEDIUM; {@code error} level →
 * LINT/LOW; {@code warning} level → LINT/INFO.
 *
 * <p>Configuration: an application-owned file ({@code extends: default})
 * written into the private staging directory. Repository {@code .yamllint}
 * files are never loaded because the tool only sees the snapshot.
 */
public final class YamllintAdapter implements ScannerAdapter {

    public static final String ID = "yamllint";
    private static final String APP_CONFIG = "extends: default\n";

    private static final Pattern PARSABLE = Pattern.compile(
            "^(.*):(\\d+):(\\d+): \\[(warning|error)\\] (.*) \\(([A-Za-z0-9_-]+)\\)$");

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "yamllint";
    }

    @Override
    public ToolProbe probe() {
        Optional<Path> executable = ExecutableResolver.resolve(ID, "yamllint");
        if (executable.isEmpty()) {
            return ToolProbe.unavailable("yamllint executable not found on PATH"
                    + " (override with -Dpwt.tools.yamllint=/absolute/path)");
        }
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.get().toString(), "-v"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() != 0) {
                return ToolProbe.unavailable("yamllint -v exited with " + result.exitCode());
            }
            return ToolProbe.available(executable.get().toString(), parseVersion(result.stdout()));
        } catch (Exception e) {
            return ToolProbe.unavailable("yamllint could not start: " + e.getMessage());
        }
    }

    public static String parseVersion(String output) {
        if (output == null) {
            return "unknown";
        }
        Matcher matcher = Pattern.compile("yamllint\\s+([0-9][0-9A-Za-z.+-]*)").matcher(output.trim());
        return matcher.find() ? matcher.group(1) : "unknown";
    }

    @Override
    public ScannerOutcome execute(ScanContext context) {
        Path executable = context.executableOverride() != null
                ? context.executableOverride()
                : ExecutableResolver.resolve(ID, "yamllint").orElse(null);
        if (executable == null) {
            return unavailable("yamllint executable not found on PATH");
        }
        Path configFile = context.stagedDir().resolve("pwt-yamllint.conf");
        try {
            java.nio.file.Files.writeString(configFile, APP_CONFIG);
        } catch (Exception e) {
            return failure(null, "Could not write yamllint configuration: " + e.getMessage());
        }
        List<String> command = new ArrayList<>();
        command.add(executable.toString());
        command.add("-f");
        command.add("parsable");
        command.add("-c");
        command.add(configFile.toString());
        command.addAll(context.relativeTargets());

        ProcessRunner.ProcessResult result;
        try {
            result = ProcessRunner.run(command, context.stagedDir(), null, context.timeout(),
                    10 * 1024 * 1024, 2 * 1024 * 1024, context.cancellation());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return statusOnly(ScannerStatus.INTERRUPTED, null, "yamllint run interrupted");
        } catch (Exception e) {
            return failure(null, "yamllint could not start: " + e.getMessage());
        }
        if (result.cancelled()) {
            return statusOnly(ScannerStatus.CANCELLED, null, "yamllint run cancelled");
        }
        if (result.timedOut()) {
            return statusOnly(ScannerStatus.TIMED_OUT, null,
                    "yamllint exceeded its time budget and was terminated");
        }
        if (result.exitCode() != 0 && result.exitCode() != 1) {
            String detail = firstLine(result.stderr());
            if (detail.isEmpty()) {
                detail = firstLine(result.stdout());
            }
            return failure(null, "yamllint failed with exit code " + result.exitCode()
                    + (detail.isEmpty() ? "" : ": " + detail));
        }
        String version = queryVersion(executable);
        List<Finding> findings = parseParsable(result.stdout());
        return ScannerOutcome.withoutGaps(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                result.exitCode(), findings.isEmpty() ? "No yamllint problems" : findings.size() + " problem(s)",
                findings);
    }

    private String queryVersion(Path executable) {
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.toString(), "-v"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() == 0) {
                return parseVersion(result.stdout());
            }
        } catch (Exception ignored) {
            // best effort; version stays unknown
        }
        return "unknown";
    }

    /** Parses {@code -f parsable} output; unparseable lines are skipped with no finding. */
    public static List<Finding> parseParsable(String output) {
        List<Finding> findings = new ArrayList<>();
        if (output == null || output.isEmpty()) {
            return findings;
        }
        for (String line : output.split("\n")) {
            String trimmed = line.strip();
            if (trimmed.isEmpty()) {
                continue;
            }
            Matcher matcher = PARSABLE.matcher(trimmed);
            if (!matcher.matches()) {
                continue;
            }
            String path = matcher.group(1).replace('\\', '/');
            int lineNo = Integer.parseInt(matcher.group(2));
            int colNo = Integer.parseInt(matcher.group(3));
            String level = matcher.group(4);
            String message = matcher.group(5);
            String rule = matcher.group(6);
            FindingCategory category = rule.equals("syntax") ? FindingCategory.SYNTAX : FindingCategory.LINT;
            Severity severity;
            if (rule.equals("syntax")) {
                severity = Severity.MEDIUM;
            } else if (level.equals("error")) {
                severity = Severity.LOW;
            } else {
                severity = Severity.INFO;
            }
            String fingerprint = Fingerprints.v1(ID, rule, path, lineNo, colNo);
            findings.add(new Finding("yamllint/" + rule,
                    "yamllint " + rule + " (" + level + ")",
                    severity, level, category, path, lineNo, lineNo, colNo,
                    message, "{\"rule\":\"" + rule + "\",\"level\":\"" + level + "\"}",
                    fingerprint));
        }
        return findings;
    }

    private ScannerOutcome unavailable(String detail) {
        return statusOnly(ScannerStatus.UNAVAILABLE, null, detail);
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

    /** Builds staged-file rows for tests and services without running a tool. */
    public static List<ScanSummary.SelectedTarget> toTargets(List<String> paths) {
        List<ScanSummary.SelectedTarget> targets = new ArrayList<>();
        for (String path : paths) {
            targets.add(new ScanSummary.SelectedTarget(path, "", 0));
        }
        return targets;
    }
}

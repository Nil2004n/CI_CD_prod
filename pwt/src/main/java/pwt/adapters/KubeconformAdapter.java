package pwt.adapters;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
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
 * kubeconform adapter. The installed binary reports version
 * {@code development} (built from source, no release version); availability
 * therefore means "executable found and responding", and the exact version
 * string is recorded verbatim.
 *
 * <p>Run: {@code kubeconform -output json -verbose -strict
 * -schema-location <explicit local template> -kubernetes-version <v>
 * <relative staged files...>} with the staging directory as working
 * directory. The schema template must be local: this adapter never relies on
 * network downloads during a scan. A blank template means "no local schema
 * source" and yields SKIPPED with setup instructions; configured-but-missing
 * schemas yield per-resource coverage gaps instead. Note: template kinds are
 * lowercased by kubeconform, so local schema files must use lowercase names
 * (e.g. {@code widget.json}, {@code deployment-apps-v1.json}).
 *
 * <p>Applicability: Kubernetes-manifest content sniff (apiVersion + kind),
 * shared with the Checkov adapter.
 *
 * <p>Exit codes: 1 = at least one invalid resource; 0 = no invalid resources
 * (errors and skips are still parsed from the resources array); anything else
 * or unparseable output = execution failure. Statuses: statusValid → no
 * finding; statusInvalid → SCHEMA/MEDIUM finding with per-document
 * validation errors; statusError/statusSkipped → coverage gaps.
 */
public final class KubeconformAdapter implements ScannerAdapter {

    public static final String ID = "kubeconform";
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "kubeconform";
    }

    @Override
    public ToolProbe probe() {
        Optional<Path> executable = ExecutableResolver.resolve(ID, "kubeconform");
        if (executable.isEmpty()) {
            return ToolProbe.unavailable("kubeconform executable not found on PATH"
                    + " (override with -Dpwt.tools.kubeconform=/absolute/path)");
        }
        String version = queryVersion(executable.get());
        if (version == null) {
            return ToolProbe.unavailable("kubeconform -v exited abnormally");
        }
        return ToolProbe.available(executable.get().toString(), version);
    }

    private String queryVersion(Path executable) {
        try {
            ProcessRunner.ProcessResult result = ProcessRunner.run(
                    List.of(executable.toString(), "-v"), null, null,
                    Duration.ofSeconds(30), 64 * 1024, 64 * 1024, null);
            if (result.exitCode() != 0) {
                return null;
            }
            String version = result.stdout().strip();
            return version.isEmpty() ? "unknown" : version;
        } catch (Exception e) {
            return null;
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
                : ExecutableResolver.resolve(ID, "kubeconform").orElse(null);
        if (executable == null) {
            return statusOnly(ScannerStatus.UNAVAILABLE, null, "kubeconform executable not found on PATH");
        }
        List<String> applicable = applicableTargets(context.stagedDir(), context.relativeTargets());
        if (applicable.isEmpty()) {
            return ScannerOutcome.withoutGaps(ID, toolName(), queryVersion(executable),
                    ScannerStatus.NOT_APPLICABLE, null,
                    "No selected file is a recognized Kubernetes manifest; kubeconform did not run.", List.of());
        }
        String template = context.kubeconformSchemaTemplate();
        String kubeVersion = context.kubernetesVersion();
        boolean strict = context.kubernetesStrict();
        if (template == null || template.isBlank()) {
            return statusOnly(ScannerStatus.SKIPPED, queryVersion(executable),
                    "No local kubeconform schema source configured: refusing silent network downloads. "
                            + "Run `pwt kubeconform-setup` or pass --kubeconform-schema <local-template>.");
        }
        List<String> command = new ArrayList<>();
        command.add(executable.toString());
        command.add("-output");
        command.add("json");
        command.add("-verbose");
        if (strict) {
            command.add("-strict");
        }
        command.add("-schema-location");
        command.add(template);
        command.add("-kubernetes-version");
        command.add(kubeVersion);
        command.addAll(applicable);

        ProcessRunner.ProcessResult result;
        try {
            result = ProcessRunner.run(command, context.stagedDir(), null, context.timeout(),
                    10 * 1024 * 1024, 2 * 1024 * 1024, context.cancellation());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return statusOnly(ScannerStatus.INTERRUPTED, null, "kubeconform run interrupted");
        } catch (Exception e) {
            return failure(null, "kubeconform could not start: " + e.getMessage());
        }
        if (result.cancelled()) {
            return statusOnly(ScannerStatus.CANCELLED, null, "kubeconform run cancelled");
        }
        if (result.timedOut()) {
            return statusOnly(ScannerStatus.TIMED_OUT, null,
                    "kubeconform exceeded its time budget and was terminated");
        }
        String version = queryVersion(executable);
        if (result.exitCode() != 0 && result.exitCode() != 1) {
            return failure(version, "kubeconform failed with exit code " + result.exitCode() + ": "
                    + firstLine(result.stderr().isEmpty() ? result.stdout() : result.stderr()));
        }
        ParseResult parsed;
        try {
            parsed = parseReport(result.stdout(), new java.util.HashSet<>(context.relativeTargets()));
        } catch (Exception e) {
            return failure(version, "kubeconform output was not a parseable report: " + e.getMessage());
        }
        String detail = parsed.valid() + " valid, " + parsed.findings().size() + " invalid, "
                + parsed.gaps().size() + " schema-missing resource(s)";
        StringBuilder gaps = new StringBuilder(detail);
        for (String gap : parsed.gaps()) {
            gaps.append("; ").append(gap);
        }
        return new ScannerOutcome(ID, toolName(), version, ScannerStatus.SUCCEEDED,
                result.exitCode(), gaps.toString(), parsed.findings(), parsed.gaps());
    }

    public record ParseResult(List<Finding> findings, int valid, List<String> gaps) {
    }

    /** Parses {@code kubeconform -output json -verbose} output. */
    public static ParseResult parseReport(String reportJson, java.util.Set<String> selectedTargets)
            throws Exception {
        JsonNode root = MAPPER.readTree(reportJson);
        JsonNode resources = root.path("resources");
        if (!resources.isArray()) {
            throw new IllegalArgumentException("kubeconform output has no resources array");
        }
        List<Finding> findings = new ArrayList<>();
        List<String> gaps = new ArrayList<>();
        int valid = 0;
        for (JsonNode resource : resources) {
            String file = resource.path("filename").asText(null);
            String normalized = file == null ? null : file.replace('\\', '/');
            if (normalized == null || !selectedTargets.contains(normalized)) {
                continue;
            }
            String kind = resource.path("kind").asText("?");
            String name = resource.path("name").asText("?");
            String status = resource.path("status").asText("");
            switch (status) {
                case "statusValid" -> valid++;
                case "statusInvalid" -> {
                    List<String> problems = new ArrayList<>();
                    JsonNode errors = resource.path("validationErrors");
                    if (errors.isArray()) {
                        for (JsonNode error : errors) {
                            String path = error.path("path").asText("");
                            String msg = error.path("msg").asText("");
                            problems.add((path.isEmpty() ? msg : path + ": " + msg));
                        }
                    }
                    if (problems.isEmpty()) {
                        String msg = resource.path("msg").asText("schema validation failed");
                        problems.add(msg);
                    }
                    String message = kind + "/" + name + " is not schema-valid at " + normalized
                            + ": " + String.join("; ", problems.subList(0, Math.min(3, problems.size())));
                    String fingerprint = Fingerprints.v1(ID, kind, normalized, null, null);
                    findings.add(new Finding("kubeconform/" + kind, kind + " schema violation",
                            Severity.MEDIUM, status, FindingCategory.SCHEMA, normalized,
                            null, null, null, message,
                            "{\"kind\":\"" + escape(kind) + "\",\"name\":\"" + escape(name) + "\"}",
                            fingerprint));
                }
                case "statusError" -> gaps.add(kind + "/" + name + " at " + normalized + ": "
                        + resource.path("msg").asText("schema error") + " (no usable schema)");
                case "statusSkipped" -> gaps.add(kind + "/" + name + " at " + normalized + ": skipped");
                default -> gaps.add(kind + "/" + name + " at " + normalized + ": unknown status " + status);
            }
        }
        return new ParseResult(List.copyOf(findings), valid, List.copyOf(gaps));
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

    /** Counts locally cached schemas under a cache root (test/availability helper). */
    public static long countCachedSchemas(Path cacheRoot) {
        if (cacheRoot == null) {
            return 0;
        }
        try (var walk = Files.walk(cacheRoot)) {
            return walk.filter(p -> p.toString().endsWith(".json")).count();
        } catch (Exception e) {
            return 0;
        }
    }
}

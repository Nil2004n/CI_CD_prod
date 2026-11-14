package pwt.launcher;

import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import pwt.application.CancellationToken;
import pwt.application.LocalSchemaConfig;
import pwt.application.ScanConfig;
import pwt.application.ScanException;
import pwt.application.ScanService;
import pwt.domain.ScanSummary;
import pwt.domain.ScanVerdict;

/**
 * Headless scan entrypoint using the same {@link ScanService} as the GUI.
 *
 * <pre>
 * pwt scan --root &lt;directory&gt; --file &lt;relative&gt;... [--adapter id,...]
 *          [--data-dir &lt;directory&gt;] [--timeout-sec N] [--source CLI|GUI]
 *          [--schema-binding GLOB=SCHEMA_PATH]... [--schema-dialect 2020-12|2019-09]
 *          [--kubeconform-schema &lt;local-template&gt;] [--kubeconform-version V]
 *          [--kubeconform-strict|--kubeconform-lenient]
 * </pre>
 *
 * <p>Exit codes: 0 = assessment completed with no findings; 4 = assessment
 * completed or partially completed with findings observed; 2 = invalid input
 * or repository busy; 1 = execution/storage failure or interruption.
 * Diagnostics go to stderr; a JSON result object goes to stdout.
 */
public final class ScanCommand {

    private ScanCommand() {
    }

    public record ParsedArgs(Path root, List<String> files, Set<String> adapters,
            Path dataDir, Duration timeout, String source, List<String> schemaBindings,
            String schemaDialect, String kubeconformSchema, String kubeconformVersion,
            boolean kubeconformStrict, boolean exportReports, LocalSchemaConfig localSchema,
            ScanConfig.KubeconformConfig kubeconform) {
    }

    public static ParsedArgs parseArgs(List<String> args) throws ScanException {
        Path root = null;
        List<String> files = new ArrayList<>();
        Set<String> adapters = new LinkedHashSet<>();
        Path dataDir = ScanService.defaultDataDir();
        Duration timeout = ScanConfig.defaults().toolTimeout();
        String source = "CLI";
        List<String> schemaBindings = new ArrayList<>();
        String schemaDialect = LocalSchemaConfig.DIALECT_2020_12;
        String kubeconformSchema = null;
        String kubeconformVersion = ScanConfig.KubeconformConfig.defaults().kubernetesVersion();
        boolean kubeconformStrict = true;
        boolean exportReports = false;
        for (int i = 0; i < args.size(); i++) {
            String arg = args.get(i);
            switch (arg) {
                case "--root" -> root = Path.of(next(args, ++i, "--root"));
                case "--file" -> files.add(next(args, ++i, "--file"));
                case "--adapter" -> {
                    for (String part : next(args, ++i, "--adapter").split(",")) {
                        String trimmed = part.trim();
                        if (!trimmed.isEmpty()) {
                            adapters.add(trimmed);
                        }
                    }
                }
                case "--data-dir" -> dataDir = Path.of(next(args, ++i, "--data-dir"));
                case "--timeout-sec" -> {
                    long seconds;
                    try {
                        seconds = Long.parseLong(next(args, ++i, "--timeout-sec"));
                    } catch (NumberFormatException e) {
                        throw new ScanException(ScanException.Code.INVALID_INPUT,
                                "Invalid --timeout-sec value", e);
                    }
                    if (seconds < 1) {
                        throw new ScanException(ScanException.Code.INVALID_INPUT,
                                "--timeout-sec must be >= 1");
                    }
                    timeout = Duration.ofSeconds(seconds);
                }
                case "--source" -> {
                    source = next(args, ++i, "--source");
                    if (!source.equals("CLI") && !source.equals("GUI")) {
                        throw new ScanException(ScanException.Code.INVALID_INPUT,
                                "--source must be CLI or GUI");
                    }
                }
                case "--schema-binding" -> schemaBindings.add(next(args, ++i, "--schema-binding"));
                case "--schema-dialect" -> {
                    schemaDialect = next(args, ++i, "--schema-dialect");
                    if (!schemaDialect.equals(LocalSchemaConfig.DIALECT_2020_12)
                            && !schemaDialect.equals(LocalSchemaConfig.DIALECT_2019_09)) {
                        throw new ScanException(ScanException.Code.INVALID_INPUT,
                                "--schema-dialect must be 2020-12 or 2019-09");
                    }
                }
                case "--kubeconform-schema" ->
                        kubeconformSchema = next(args, ++i, "--kubeconform-schema");
                case "--kubeconform-version" ->
                        kubeconformVersion = next(args, ++i, "--kubeconform-version");
                case "--kubeconform-strict" -> kubeconformStrict = true;
                case "--kubeconform-lenient" -> kubeconformStrict = false;
                case "--export" -> exportReports = true;
                default -> throw new ScanException(ScanException.Code.INVALID_INPUT,
                        "Unknown scan argument: " + arg);
            }
        }
        if (root == null) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "Usage: scan --root <directory> --file <relative>... [--adapter id,...]"
                    + " [--data-dir <directory>] [--timeout-sec N] [--export]");
        }
        if (files.isEmpty()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "No --file selections supplied");
        }
        LocalSchemaConfig localSchema;
        try {
            localSchema = LocalSchemaConfig.parse(schemaBindings, schemaDialect, Path.of("").toAbsolutePath());
        } catch (IllegalArgumentException e) {
            throw new ScanException(ScanException.Code.INVALID_INPUT, e.getMessage(), e);
        }
        ScanConfig.KubeconformConfig kubeconform = new ScanConfig.KubeconformConfig(
                kubeconformSchema, kubeconformVersion, kubeconformStrict);
        return new ParsedArgs(root, files, adapters, dataDir, timeout, source, schemaBindings,
                schemaDialect, kubeconformSchema, kubeconformVersion, kubeconformStrict, exportReports,
                localSchema, kubeconform);
    }

    private static String next(List<String> args, int index, String flag) throws ScanException {
        if (index >= args.size()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT, "Missing value for " + flag);
        }
        return args.get(index);
    }

    /** Executes the command; returns the process exit code. */
    public static int execute(List<String> args) {
        ParsedArgs parsed;
        try {
            parsed = parseArgs(args);
        } catch (ScanException e) {
            System.err.println("pwt scan: invalid input: " + e.getMessage());
            return 2;
        }
        ScanService service = new ScanService(new ScanConfig(parsed.timeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                parsed.kubeconform(), parsed.localSchema()));
        ScanService.reconcileInterrupted(parsed.dataDir());
        ScanSummary summary;
        try {
            summary = service.runScan(parsed.root(), parsed.files(), parsed.adapters(), parsed.source(),
                    parsed.dataDir(), CancellationToken.none(), null);
        } catch (ScanException e) {
            System.err.println("pwt scan: " + e.code().name().toLowerCase().replace('_', ' ')
                    + ": " + e.getMessage());
            return switch (e.code()) {
                case INVALID_INPUT, REPOSITORY_BUSY -> 2;
                case STORAGE_ERROR, INTERRUPTED -> 1;
            };
        }
        System.out.println(ScanSummaryJson.toJson(summary));
        if (parsed.exportReports()) {
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(parsed.dataDir())) {
                pwt.application.ReportService.ReportPaths paths =
                        new pwt.application.ReportService().generate(store, parsed.dataDir(), summary.runId());
                System.out.println("{\"exported_json\":" + jsonOrNull(paths.jsonPath())
                        + ",\"exported_markdown\":" + jsonOrNull(paths.markdownPath()) + "}");
                if (paths.jsonError() != null || paths.markdownError() != null) {
                    System.err.println("pwt scan: export failed: "
                            + (paths.jsonError() != null ? paths.jsonError() : paths.markdownError()));
                    return 1;
                }
            } catch (Exception e) {
                System.err.println("pwt scan: export failed: " + e.getMessage());
                return 1;
            }
        }
        if (summary.verdict() == ScanVerdict.FINDINGS) {
            return 4;
        }
        return switch (summary.state()) {
            case COMPLETED -> 0;
            default -> 1;
        };
    }

    private static String jsonOrNull(String value) {
        return value == null ? "null" : "\"" + JsonWriter.escape(value) + "\"";
    }

    static List<String> split(String[] args, int from) {
        List<String> rest = new ArrayList<>();
        for (int i = from; i < args.length; i++) {
            rest.add(args[i]);
        }
        return rest;
    }
}

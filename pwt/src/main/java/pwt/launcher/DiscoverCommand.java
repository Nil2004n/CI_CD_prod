package pwt.launcher;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import pwt.application.BuildInfo;
import pwt.application.CancellationToken;
import pwt.application.DiscoveryException;
import pwt.application.DiscoveryService;
import pwt.domain.DiscoveredYamlFile;
import pwt.domain.DiscoveryOptions;
import pwt.domain.DiscoveryResult;
import pwt.domain.DiscoveryWarning;
import pwt.domain.RepositoryRoot;
import pwt.infrastructure.NioDiscoveryService;

/**
 * Headless discovery command. Runs the SAME {@link DiscoveryService} as the
 * desktop shell and emits one JSON object on stdout:
 *
 * <pre>
 * {"root":"&lt;canonical&gt;","files":[{"path":"...","size":12}],"warnings":[{"code":"...","message":"...","path":"..."}],
 *  "discovered":1,"incomplete":false}
 * </pre>
 *
 * <p>Exit codes: 0 success; 2 invalid input (bad flags, missing/invalid
 * root); 1 operational failure or cancellation. Diagnostics go to stderr;
 * stdout carries only the result object.
 */
public final class DiscoverCommand {

    public record ParsedArgs(Path root, DiscoveryOptions options) {
    }

    private DiscoverCommand() {
    }

    public static ParsedArgs parseArgs(List<String> args) throws DiscoveryException {
        Path root = null;
        DiscoveryOptions options = DiscoveryOptions.defaults();
        for (int i = 0; i < args.size(); i++) {
            String arg = args.get(i);
            switch (arg) {
                case "--root" -> {
                    if (i + 1 >= args.size()) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Missing value for --root");
                    }
                    root = Path.of(args.get(++i));
                }
                case "--max-files" -> {
                    if (i + 1 >= args.size()) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Missing value for --max-files");
                    }
                    int max;
                    try {
                        max = Integer.parseInt(args.get(++i));
                    } catch (NumberFormatException e) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Invalid --max-files value", e);
                    }
                    try {
                        options = options.withMaxDiscoveredFiles(max);
                    } catch (IllegalArgumentException e) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Invalid --max-files value: " + e.getMessage(), e);
                    }
                }
                case "--exclude" -> {
                    if (i + 1 >= args.size()) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Missing value for --exclude");
                    }
                    Set<String> names = new LinkedHashSet<>(options.excludedDirectoryNames());
                    for (String part : args.get(++i).split(",")) {
                        String trimmed = part.trim();
                        if (!trimmed.isEmpty()) {
                            names.add(trimmed);
                        }
                    }
                    try {
                        options = options.withExcludedDirectoryNames(names);
                    } catch (IllegalArgumentException e) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Invalid --exclude value: " + e.getMessage(), e);
                    }
                }
                case "--max-depth" -> {
                    if (i + 1 >= args.size()) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Missing value for --max-depth");
                    }
                    int depth;
                    try {
                        depth = Integer.parseInt(args.get(++i));
                    } catch (NumberFormatException e) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Invalid --max-depth value", e);
                    }
                    try {
                        options = options.withMaxDepth(depth);
                    } catch (IllegalArgumentException e) {
                        throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                                "Invalid --max-depth value: " + e.getMessage(), e);
                    }
                }
                default -> throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                        "Unknown discover argument: " + arg);
            }
        }
        if (root == null) {
            throw new DiscoveryException(DiscoveryException.Code.INVALID_PATH,
                    "Usage: discover --root <directory> [--max-files N] [--max-depth N] [--exclude name,...]");
        }
        return new ParsedArgs(root, options);
    }

    public static DiscoveryResult runDiscovery(Path root, DiscoveryOptions options) throws DiscoveryException {
        DiscoveryService service = new NioDiscoveryService();
        RepositoryRoot registered = service.registerRoot(root);
        return service.discover(registered, options, CancellationToken.none());
    }

    public static String toJson(DiscoveryResult result) {
        JsonWriter files = new JsonWriter();
        for (DiscoveredYamlFile file : result.files()) {
            JsonWriter entry = new JsonWriter().beginObject()
                    .field("path", file.relativePath())
                    .field("size", file.sizeBytes())
                    .endObject();
            files.arrayObject(entry);
        }
        JsonWriter warnings = new JsonWriter();
        for (DiscoveryWarning warning : result.warnings()) {
            JsonWriter entry = new JsonWriter().beginObject()
                    .field("code", warning.code())
                    .field("message", warning.message())
                    .field("path", warning.path())
                    .endObject();
            warnings.arrayObject(entry);
        }
        JsonWriter out = new JsonWriter().beginObject()
                .field("app", BuildInfo.APP_NAME)
                .field("version", BuildInfo.VERSION)
                .field("root", result.root().root().toString())
                .field("discovered", result.discoveredCount())
                .field("incomplete", result.incomplete());
        // Embed arrays via object fragments.
        String filesJson = "[" + files + "]";
        String warningsJson = "[" + warnings + "]";
        return "{\"app\":\"" + JsonWriter.escape(BuildInfo.APP_NAME) + "\","
                + "\"version\":\"" + JsonWriter.escape(BuildInfo.VERSION) + "\","
                + "\"root\":\"" + JsonWriter.escape(result.root().root().toString()) + "\","
                + "\"discovered\":" + result.discoveredCount() + ","
                + "\"incomplete\":" + (result.incomplete() ? "true" : "false") + ","
                + "\"files\":" + filesJson + ","
                + "\"warnings\":" + warningsJson + "}";
    }

    /** Executes the command; returns the process exit code. */
    public static int execute(List<String> args) {
        ParsedArgs parsed;
        try {
            parsed = parseArgs(args);
        } catch (DiscoveryException e) {
            System.err.println("pwt discover: invalid input: " + e.getMessage());
            return 2;
        } catch (IllegalArgumentException e) {
            System.err.println("pwt discover: invalid input: " + e.getMessage());
            return 2;
        }
        DiscoveryResult result;
        try {
            result = runDiscovery(parsed.root(), parsed.options());
        } catch (DiscoveryException e) {
            System.err.println("pwt discover: " + e.code() + ": " + e.getMessage());
            return switch (e.code()) {
                case INVALID_PATH, ROOT_NOT_FOUND, ROOT_NOT_DIRECTORY, ROOT_NOT_READABLE -> 2;
                case CANCELLED, IO_ERROR -> 1;
            };
        }
        System.out.println(toJson(result));
        return 0;
    }

    static List<String> split(String[] args, int from) {
        List<String> rest = new ArrayList<>();
        for (int i = from; i < args.length; i++) {
            rest.add(args[i]);
        }
        return rest;
    }
}

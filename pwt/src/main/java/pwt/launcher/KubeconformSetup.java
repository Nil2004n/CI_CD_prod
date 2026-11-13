package pwt.launcher;

import java.io.InputStream;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import pwt.application.ScanException;

/**
 * Explicit, user-invoked kubeconform schema acquisition.
 *
 * <pre>
 * pwt kubeconform-setup [--k8s-version V] [--kinds Kind[.group],...]
 *                       [--data-dir &lt;directory&gt;]
 * </pre>
 *
 * <p>Downloads the named Kubernetes JSON schemas from the configured
 * upstream base into the PWT-managed local cache
 * ({@code <data-dir>/schemas/kubeconform/<version>-standalone-strict/}).
 * This is deliberately separate from scanning: scans never download schemas
 * silently. Re-running overwrites cached files for the requested kinds.
 *
 * <p>Exit codes: 0 all requested schemas cached; 2 invalid input; 1 download
 * or storage failure.
 */
public final class KubeconformSetup {

    static final String UPSTREAM_BASE =
            "https://raw.githubusercontent.com/yannh/kubernetes-json-schema/master";
    static final List<String> DEFAULT_KINDS = List.of(
            "Pod", "Service", "ConfigMap", "ServiceAccount", "Deployment.apps",
            "DaemonSet.apps", "StatefulSet.apps", "Job.batch", "CronJob.batch");

    private KubeconformSetup() {
    }

    public record ParsedArgs(String version, List<String> kinds, Path dataDir) {
    }

    public static ParsedArgs parseArgs(List<String> args, Path defaultDataDir) throws ScanException {
        String version = "1.29.0";
        Set<String> kinds = new LinkedHashSet<>(DEFAULT_KINDS);
        Path dataDir = defaultDataDir;
        for (int i = 0; i < args.size(); i++) {
            String arg = args.get(i);
            switch (arg) {
                case "--k8s-version" -> version = next(args, ++i, "--k8s-version");
                case "--kinds" -> {
                    kinds.clear();
                    for (String part : next(args, ++i, "--kinds").split(",")) {
                        String trimmed = part.trim();
                        if (!trimmed.isEmpty()) {
                            kinds.add(trimmed);
                        }
                    }
                }
                case "--data-dir" -> dataDir = Path.of(next(args, ++i, "--data-dir"));
                default -> throw new ScanException(ScanException.Code.INVALID_INPUT,
                        "Unknown kubeconform-setup argument: " + arg);
            }
        }
        if (!version.matches("[0-9]+\\.[0-9]+\\.[0-9]+")) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "Invalid --k8s-version (expected N.N.N): " + version);
        }
        if (kinds.isEmpty()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT, "No --kinds supplied");
        }
        for (String kind : kinds) {
            if (!kind.matches("[A-Z][A-Za-z0-9]*([.][a-z0-9]([a-z0-9.-]*[a-z0-9])?)?")) {
                throw new ScanException(ScanException.Code.INVALID_INPUT,
                        "Invalid kind (expected Kind or Kind.group): " + kind);
            }
        }
        return new ParsedArgs(version, List.copyOf(kinds), dataDir);
    }

    private static String next(List<String> args, int index, String flag) throws ScanException {
        if (index >= args.size()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT, "Missing value for " + flag);
        }
        return args.get(index);
    }

    /** Maps Kind[.group] to the upstream standalone-strict filename. The API
     * version is derived from the group (core kinds use {@code v1}); the
     * Kubernetes release selects the directory, not the filename. */
    public static String schemaFileName(String kindAndGroup) {
        int dot = kindAndGroup.indexOf('.');
        String kind = dot == -1 ? kindAndGroup : kindAndGroup.substring(0, dot);
        String group = dot == -1 ? "" : kindAndGroup.substring(dot + 1);
        String apiVersion = group.isEmpty() ? "v1" : group + "/v1";
        int slash = apiVersion.indexOf('/');
        String versionPart = slash == -1 ? apiVersion : apiVersion.substring(slash + 1);
        String base = kind.toLowerCase();
        if (!group.isEmpty()) {
            base += "-" + group.toLowerCase().replace('.', '-');
        }
        return base + "-" + versionPart.toLowerCase() + ".json";
    }

    public static int execute(List<String> args, Path defaultDataDir) {
        ParsedArgs parsed;
        try {
            parsed = parseArgs(args, defaultDataDir);
        } catch (ScanException e) {
            System.err.println("pwt kubeconform-setup: invalid input: " + e.getMessage());
            return 2;
        }
        String dirName = "v" + parsed.version() + "-standalone-strict";
        Path cacheDir = parsed.dataDir().resolve("schemas").resolve("kubeconform").resolve(dirName);
        try {
            Files.createDirectories(cacheDir);
        } catch (Exception e) {
            System.err.println("pwt kubeconform-setup: cannot create cache directory: " + e.getMessage());
            return 1;
        }
        List<String> failures = new ArrayList<>();
        int cached = 0;
        for (String kind : parsed.kinds()) {
            String fileName = schemaFileName(kind);
            String url = UPSTREAM_BASE + "/" + dirName + "/" + fileName;
            Path target = cacheDir.resolve(fileName);
            try {
                java.net.URLConnection connection = URI.create(url).toURL().openConnection();
                connection.setConnectTimeout(30_000);
                connection.setReadTimeout(120_000);
                try (InputStream in = connection.getInputStream()) {
                    byte[] bytes = in.readAllBytes();
                    if (bytes.length == 0) {
                        throw new java.io.IOException("empty response");
                    }
                    Files.write(target, bytes);
                }
                System.err.println("cached " + kind + " -> " + target);
                cached++;
            } catch (Exception e) {
                failures.add(kind + ": " + e.getMessage());
            }
        }
        if (!failures.isEmpty()) {
            System.err.println("pwt kubeconform-setup: failed for " + failures.size() + " kind(s): "
                    + String.join("; ", failures));
            return 1;
        }
        System.out.println("{\"cached\":" + cached + ",\"directory\":\""
                + JsonWriter.escape(parsed.dataDir().resolve("schemas").resolve("kubeconform").toString())
                + "\"}");
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

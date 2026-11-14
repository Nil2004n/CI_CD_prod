package pwt.launcher;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pwt.adapters.ActionlintAdapter;
import pwt.adapters.CheckovAdapter;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.KubeconformAdapter;
import pwt.adapters.LocalSchemaAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.application.ScanService;
import pwt.domain.ToolProbe;

/**
 * Environment diagnostics. Prints the resolved Java runtime, application
 * artifact, data directory, registered repositories, and per-scanner
 * availability (distinguishing missing executables from missing local
 * schemas/configuration). Read-only; installs and modifies nothing.
 * Diagnostics on stderr; the report object on stdout. Exit 0 always
 * (absence is data, not failure).
 */
public final class DoctorCommand {

    private DoctorCommand() {
    }

    public static int execute(List<String> args) {
        Path dataDir = ScanService.defaultDataDir();
        for (int i = 0; i < args.size(); i++) {
            if (args.get(i).equals("--data-dir")) {
                if (++i >= args.size()) {
                    System.err.println("pwt doctor: missing value for --data-dir");
                    return 2;
                }
                dataDir = Path.of(args.get(i));
            } else {
                System.err.println("pwt doctor: unknown argument: " + args.get(i));
                return 2;
            }
        }
        JsonWriter out = new JsonWriter();
        List<String> repos = new ArrayList<>();
        int repoCount = -1;
        try (pwt.infrastructure.SqliteStore store = pwt.infrastructure.SqliteStore.open(dataDir)) {
            for (pwt.infrastructure.SqliteStore.RepositoryRow row : store.listRepositories()) {
                boolean available = java.nio.file.Files.isDirectory(Path.of(row.canonicalRoot()));
                repos.add((available ? "available " : "unavailable ") + row.displayName());
            }
            repoCount = repos.size();
        } catch (Exception e) {
            System.err.println("pwt doctor: store unreadable: " + e.getMessage());
        }
        out.field("repository_count", repoCount);
        JsonWriter repoArray = new JsonWriter();
        for (String repo : repos) {
            JsonWriter entry = new JsonWriter().beginObject().field("summary", repo).endObject();
            repoArray.arrayObject(entry);
        }

        record Probe(String id, ToolProbe probe) {
        }
        List<Probe> probes = List.of(
                new Probe("yamllint", new YamllintAdapter().probe()),
                new Probe("gitleaks", new GitleaksAdapter().probe()),
                new Probe("checkov", new CheckovAdapter().probe()),
                new Probe("actionlint", new ActionlintAdapter().probe()),
                new Probe("kubeconform", new KubeconformAdapter().probe()),
                new Probe("local-schema", new LocalSchemaAdapter().probe()));
        JsonWriter scanners = new JsonWriter();
        for (Probe probe : probes) {
            JsonWriter entry = new JsonWriter().beginObject()
                    .field("adapter_id", probe.id())
                    .field("available", probe.probe().available())
                    .field("detail", probe.probe().available() ? probe.probe().version() : probe.probe().reason())
                    .endObject();
            scanners.arrayObject(entry);
        }
        // Embed pre-rendered arrays (JsonWriter is a minimal builder).
        System.out.println("{\"app\":\"" + JsonWriter.escape(pwt.application.BuildInfo.APP_NAME) + "\","
                + "\"version\":\"" + JsonWriter.escape(pwt.application.BuildInfo.VERSION) + "\","
                + "\"java_home\":\"" + JsonWriter.escape(System.getProperty("java.home", "")) + "\","
                + "\"java_version\":\"" + JsonWriter.escape(System.getProperty("java.version", "")) + "\","
                + "\"data_dir\":\"" + JsonWriter.escape(dataDir.toAbsolutePath().normalize().toString()) + "\","
                + "\"database\":\"" + JsonWriter.escape(
                        dataDir.toAbsolutePath().normalize().resolve("pwt.db").toString()) + "\","
                + "\"repository_count\":" + repoCount + ","
                + "\"repositories\":[" + repoArray + "],"
                + "\"scanners\":[" + scanners + "],"
                + "\"opencode\":" + opencodeInfo() + ","
                + "\"install\":" + installInfo(dataDir) + ","
                + "\"permissions\":" + permissionsInfo() + "}");
        return 0;
    }

    /** Best-effort user installation state. Never fails doctor; absent pieces report false/null. */
    static String installInfo(Path dataDir) {
        String appRoot;
        try {
            appRoot = AppPaths.appRoot().toString();
        } catch (Exception e) {
            appRoot = null;
        }
        String launcherPath = findOnPath("pwt");
        Path home = Path.of(System.getProperty("user.home", ""));
        Path pluginPackage = home.resolve(".config/opencode/plugins/pwt/package.json");
        Path agentFile = home.resolve(".config/opencode/agents/pwt.md");
        Path askFile = home.resolve(".config/opencode/commands/pwt-ask.md");
        boolean writable;
        try {
            writable = java.nio.file.Files.isWritable(dataDir);
        } catch (Exception e) {
            writable = false;
        }
        String owner = null;
        try {
            owner = String.valueOf(java.nio.file.Files.getOwner(dataDir));
        } catch (Exception ignored) {
            // non-POSIX filesystems or unreadable attributes: absence is data
        }
        return "{\"app_root\":" + jsonString(appRoot)
                + ",\"launcher_on_path\":" + (launcherPath != null)
                + ",\"launcher_path\":" + jsonString(launcherPath)
                + ",\"plugin_installed\":" + java.nio.file.Files.isRegularFile(pluginPackage)
                + ",\"agent_installed\":" + java.nio.file.Files.isRegularFile(agentFile)
                + ",\"ask_command_installed\":" + java.nio.file.Files.isRegularFile(askFile)
                + ",\"data_writable\":" + writable
                + ",\"data_owner\":" + jsonString(owner) + "}";
    }

    private static String findOnPath(String name) {
        String pathEnv = System.getenv("PATH");
        if (pathEnv == null) {
            return null;
        }
        for (String dir : pathEnv.split(java.util.regex.Pattern.quote(java.io.File.pathSeparator))) {
            if (dir.isEmpty()) {
                continue;
            }
            Path candidate = Path.of(dir, name);
            try {
                if (java.nio.file.Files.isRegularFile(candidate)
                        && java.nio.file.Files.isExecutable(candidate)) {
                    return candidate.toAbsolutePath().normalize().toString();
                }
            } catch (Exception ignored) {
                // unreadable PATH entry: keep looking
            }
        }
        return null;
    }

    private static String jsonString(String value) {
        return value == null ? "null" : "\"" + JsonWriter.escape(value) + "\"";
    }

    /** Best-effort OpenCode version + native TUI-plugin compatibility. Never fails doctor. */
    static String opencodeInfo() {
        String version = "not-installed";
        try {
            Process probe = new ProcessBuilder("opencode", "--version")
                    .redirectErrorStream(true).start();
            String text = new String(probe.getInputStream().readAllBytes(),
                    java.nio.charset.StandardCharsets.UTF_8);
            if (probe.waitFor(10, java.util.concurrent.TimeUnit.SECONDS)) {
                String trimmed = text.strip().split("\\s+")[text.strip().split("\\s+").length - 1];
                version = trimmed.replaceAll("[^0-9A-Za-z._-]", "").isEmpty() ? "unknown" : trimmed;
            }
        } catch (Exception ignored) {
            // absence is data
        }
        // Native PWT commands need the OpenCode v2 TUI plugin API (keymap
        // layer + slash entries with direct run handlers). v2.x verified.
        boolean nativeCompatible = version.matches("v?2\\..*");
        JsonWriter info = new JsonWriter().beginObject()
                .field("version", version)
                .field("native_commands_supported", nativeCompatible)
                .field("note", "native /pwt commands need OpenCode v2 TUI plugin API")
                .endObject();
        return info.toString();
    }

    /** Documents the two independent MCP permission flags. */
    static String permissionsInfo() {
        JsonWriter info = new JsonWriter().beginObject()
                .field("scan_writes_flag", "--allow-scans")
                .field("analysis_writes_flag", "--allow-analysis-writes")
                .field("note", "independent flags: scans do not imply analysis writes; "
                        + "project connectors use --repo-id scoping, unscoped mode is explicit local admin")
                .endObject();
        return info.toString();
    }

    static List<String> split(String[] args, int from) {
        List<String> rest = new ArrayList<>();
        for (int i = from; i < args.length; i++) {
            rest.add(args[i]);
        }
        return rest;
    }
}

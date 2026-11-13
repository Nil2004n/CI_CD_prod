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
                + "\"data_dir\":\"" + JsonWriter.escape(dataDir.toString()) + "\","
                + "\"repository_count\":" + repoCount + ","
                + "\"repositories\":[" + repoArray + "],"
                + "\"scanners\":[" + scanners + "]}");
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

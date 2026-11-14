package pwt.launcher;

import java.util.List;
import pwt.application.BuildInfo;
import pwt.application.ScanService;

/**
 * Single entrypoint for all Phase 1 modes.
 *
 * <ul>
 *   <li>{@code gui} — launch the JavaFX desktop shell.</li>
 *   <li>{@code discover --root <dir> [...]} — headless discovery (same service
 *       as the GUI); JSON on stdout, diagnostics on stderr.</li>
 *   <li>{@code scan --root <dir> --file <rel>... [...]} — headless scan of
 *       explicitly selected files through the integrated adapters; result
 *       JSON on stdout, diagnostics on stderr.</li>
 *   <li>{@code kubeconform-setup [--k8s-version V] [--kinds ...] [--data-dir D]}
 *       — explicitly download kubeconform schemas into the local cache
 *       (scans never download).</li>
 *   <li>{@code repo-add --root <dir> [--name N] [--data-dir D]} — register a
 *       repository root (idempotent, shared across GUI/CLI/MCP).</li>
 *   <li>{@code repo-list [--data-dir D]} — list registered roots.</li>
 *   <li>{@code journal --root <dir> [--run-id <id>] [--check] [--data-dir D]}
 *       — append missing scan sections to the repository Markdown journal
 *       from persisted runs (backfill + retry); {@code --check} only reports
 *       currency without writing.</li>
 *   <li>{@code doctor [--data-dir D]} — environment/dependency report.</li>
 *   <li>{@code mcp-status [--data-dir D]} — recorded MCP session presence.</li>
 *   <li>{@code mcp} — MCP stdio server; stdout is MCP-protocol-only.</li>
 * </ul>
 *
 * <p>No-argument invocation prints usage to stderr and exits 2 rather than
 * guessing a mode. In {@code mcp} mode every diagnostic (including startup
 * banners and logging configuration) goes to stderr; nothing but the MCP
 * SDK may write to stdout.
 */
public final class PwtMain {

    private PwtMain() {
    }

    public static void main(String[] args) {
        if (args.length == 0) {
            usage();
            System.exit(2);
            return;
        }
        String mode = args[0];
        switch (mode) {
            case "gui" -> launchGui(stripFirst(args));
            case "discover" -> System.exit(DiscoverCommand.execute(DiscoverCommand.split(args, 1)));
            case "scan" -> System.exit(ScanCommand.execute(ScanCommand.split(args, 1)));
            case "kubeconform-setup" -> System.exit(KubeconformSetup.execute(
                    KubeconformSetup.split(args, 1), ScanService.defaultDataDir()));
            case "repo-add" -> System.exit(RepoCommand.add(RepoCommand.split(args, 1)));
            case "repo-list" -> System.exit(RepoCommand.list(RepoCommand.split(args, 1)));
            case "journal" -> System.exit(JournalCommand.execute(JournalCommand.split(args, 1)));
            case "doctor" -> System.exit(DoctorCommand.execute(DoctorCommand.split(args, 1)));
            case "mcp-status" -> System.exit(McpStatusCommand.execute(McpStatusCommand.split(args, 1)));
            case "mcp" -> runMcp(ScanCommand.split(args, 1));
            case "--help", "-h", "help" -> {
                usage();
                System.exit(0);
            }
            case "--version", "-V", "version" -> {
                System.out.println(BuildInfo.APP_NAME + " " + BuildInfo.VERSION + " (" + BuildInfo.PHASE + ")");
                System.exit(0);
            }
            default -> {
                System.err.println("pwt: unknown mode: " + mode);
                usage();
                System.exit(2);
            }
        }
    }

    private static void launchGui(String[] args) {
        // Optional shared data directory so the GUI observes the same store
        // as headless/MCP entrypoints: `gui [--data-dir <directory>]`.
        // Defaults to <application-root>/.pwt-data when absent.
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--data-dir") && i + 1 < args.length) {
                System.setProperty("pwt.data.dir", args[i + 1]);
            }
        }
        // JavaFX startup; any failure (e.g. no display) surfaces here on stderr.
        javafx.application.Application.launch(pwt.desktop.PwtDesktopApp.class, args);
    }

    private static void runMcp(List<String> args) {
        java.nio.file.Path dataDir = pwt.application.ScanService.defaultDataDir();
        boolean allowScans = false;
        boolean allowAnalysis = false;
        String repoId = null;
        java.nio.file.Path projectRoot = null;
        for (int i = 0; i < args.size(); i++) {
            String arg = args.get(i);
            switch (arg) {
                case "--data-dir" -> {
                    if (i + 1 >= args.size()) {
                        System.err.println("pwt mcp: missing value for --data-dir");
                        System.exit(2);
                        return;
                    }
                    dataDir = java.nio.file.Path.of(args.get(++i));
                }
                case "--allow-scans" -> allowScans = true;
                case "--allow-analysis-writes" -> allowAnalysis = true;
                case "--repo-id" -> {
                    if (i + 1 >= args.size()) {
                        System.err.println("pwt mcp: missing value for --repo-id");
                        System.exit(2);
                        return;
                    }
                    repoId = args.get(++i);
                }
                case "--project-root" -> {
                    if (i + 1 >= args.size()) {
                        System.err.println("pwt mcp: missing value for --project-root");
                        System.exit(2);
                        return;
                    }
                    java.nio.file.Path candidate = java.nio.file.Path.of(args.get(++i));
                    if (!candidate.isAbsolute()) {
                        System.err.println("pwt mcp: --project-root must be an absolute path");
                        System.exit(2);
                        return;
                    }
                    projectRoot = candidate.normalize();
                }
                default -> {
                    System.err.println("pwt mcp: unknown argument: " + arg);
                    usage();
                    System.exit(2);
                    return;
                }
            }
        }
        if (repoId != null && projectRoot != null) {
            System.err.println("pwt mcp: use either --repo-id or --project-root, not both");
            System.exit(2);
            return;
        }
        if (projectRoot != null && !java.nio.file.Files.isDirectory(projectRoot)) {
            // Not fatal: every tool call reports PROJECT_UNAVAILABLE until it exists.
            System.err.println("pwt mcp: note: --project-root is not a directory right now: " + projectRoot);
        }
        if (repoId != null) {
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(dataDir)) {
                if (store.findRepositoryById(repoId) == null) {
                    System.err.println("pwt mcp: unknown --repo-id: " + repoId);
                    System.exit(2);
                    return;
                }
            } catch (Exception e) {
                System.err.println("pwt mcp: cannot validate --repo-id: " + e.getMessage());
                System.exit(2);
                return;
            }
        }
        pwt.mcp.PwtMcpServer.run(dataDir, allowScans, allowAnalysis, repoId, projectRoot);
    }

    private static String[] stripFirst(String[] args) {
        String[] rest = new String[args.length - 1];
        System.arraycopy(args, 1, rest, 0, rest.length);
        return rest;
    }

    private static void usage() {
        List<String> lines = List.of(
                "Usage: pwt <mode> [options]",
                "",
                "Modes:",
                "  gui                          Launch the JavaFX desktop shell.",
                "  discover --root <directory>  Run headless YAML discovery; JSON on stdout.",
                "     [--max-files N] [--max-depth N] [--exclude name,...]",
                "  scan --root <dir> --file <rel>...  Scan selected files (DB is authoritative;",
                "     result JSON on stdout. [--adapter id,...] [--data-dir <dir>] [--timeout-sec N]",
                "     [--schema-binding GLOB=SCHEMA ...] [--kubeconform-schema T] [--export]",
                "     Exit: 0 clean, 4 findings, 2 invalid/busy, 1 failure.",
                "  mcp [--data-dir <dir>] [--allow-scans] [--allow-analysis-writes]",
                "     [--repo-id <id> | --project-root <abs-dir>]",
                "                              Start the MCP stdio server (stdout is MCP-protocol-only).",
                "     Read-only tools always work; scan/analysis writes need their flags.",
                "     --repo-id restricts every operation to one registered project;",
                "     --project-root binds to the registered project owning that directory,",
                "     resolved per call (fails closed while it is unregistered; never registers).",
                "  kubeconform-setup            Download kubeconform schemas into the local cache.",
                "  repo-add --root <dir> ...    Register a repository root (idempotent).",
                "  repo-list                    List registered roots and availability.",
                "  journal --root <dir> [...]   Append missing scan sections to the repo journal;",
                "     --run-id <id>, --check (report only), --data-dir <dir>.",
                "  doctor                       Java/artifact/data-dir/repos/scanner report.",
                "  mcp-status                   Recorded MCP session presence and modes.",
                "",
                BuildInfo.APP_NAME + " " + BuildInfo.VERSION + " (" + BuildInfo.PHASE + "). "
                        + BuildInfo.PHASE_SUMMARY);
        for (String line : lines) {
            System.err.println(line);
        }
    }
}

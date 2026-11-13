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
            case "mcp" -> pwt.mcp.PwtMcpServer.run();
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
        // JavaFX startup; any failure (e.g. no display) surfaces here on stderr.
        javafx.application.Application.launch(pwt.desktop.PwtDesktopApp.class, args);
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
                "  scan --root <dir> --file <rel>...  Scan selected files (yamllint+gitleaks);",
                "     result JSON on stdout. [--adapter id,...] [--data-dir <dir>] [--timeout-sec N]",
                "     Exit: 0 clean, 4 findings, 2 invalid/busy, 1 failure.",
                "  mcp                          Start the MCP stdio server (stdout is MCP-protocol-only).",
                "  kubeconform-setup            Download kubeconform schemas into the local cache.",
                "",
                BuildInfo.APP_NAME + " " + BuildInfo.VERSION + " (" + BuildInfo.PHASE + "). "
                        + BuildInfo.PHASE_SUMMARY);
        for (String line : lines) {
            System.err.println(line);
        }
    }
}

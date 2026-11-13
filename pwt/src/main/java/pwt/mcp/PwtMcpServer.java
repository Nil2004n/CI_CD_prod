package pwt.mcp;

import io.modelcontextprotocol.server.McpServer;
import io.modelcontextprotocol.server.McpServerFeatures;
import io.modelcontextprotocol.server.McpSyncServer;
import io.modelcontextprotocol.server.transport.StdioServerTransportProvider;
import io.modelcontextprotocol.spec.McpSchema;
import java.util.concurrent.CountDownLatch;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.application.BuildInfo;
import pwt.application.ScanService;

/**
 * Minimal MCP stdio server spike.
 *
 * <p>Protocol contract:
 * <ul>
 *   <li>stdout is reserved exclusively for MCP protocol output. This class
 *       never writes to {@code System.out}; the SDK transport owns it.</li>
 *   <li>All diagnostics (startup notice, errors, logging) go to stderr via
 *       SLF4J SimpleLogger (configured to {@code System.err}).</li>
 * </ul>
 *
 * <p>Lifecycle: the server stays alive until the process is terminated
 * (normal MCP hosting: the client kills the server process). A shutdown hook
 * closes the server gracefully.
 */
public final class PwtMcpServer {

    private static final Logger LOG = LoggerFactory.getLogger(PwtMcpServer.class);

    private PwtMcpServer() {
    }

    /** Starts the server on stdio and blocks until the process is stopped. */
    public static void run() {
        run(ScanService.defaultDataDir(), false);
    }

    /**
     * Starts the server on stdio over one data directory. Read-only tools
     * always work; {@code start_scan}/{@code cancel_scan} additionally require
     * {@code allowScans} (the {@code --allow-scans} flag).
     */
    public static void run(java.nio.file.Path dataDir, boolean allowScans) {
        System.setProperty("org.slf4j.simpleLogger.logFile", "System.err");

        StdioServerTransportProvider transport = new StdioServerTransportProvider();
        McpScanManager scans = new McpScanManager(dataDir);
        java.util.concurrent.atomic.AtomicReference<McpSyncServer> serverRef =
                new java.util.concurrent.atomic.AtomicReference<>();
        McpActivity activity = new McpActivity(dataDir, allowScans, serverRef::get);
        McpScanTools.McpContext context = new McpScanTools.McpContext(dataDir, allowScans, scans, activity);
        java.util.List<McpServerFeatures.SyncToolSpecification> tools = new java.util.ArrayList<>();
        tools.add(PwtMcpTools.pwtInfoTool(activity::observed));
        tools.addAll(McpScanTools.allTools(context));
        McpSyncServer server = McpServer.sync(transport)
                .serverInfo(BuildInfo.APP_NAME, BuildInfo.VERSION)
                .capabilities(McpSchema.ServerCapabilities.builder().tools(true).build())
                .tools(tools)
                .build();
        serverRef.set(server);
        activity.begin();

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            try {
                scans.close();
            } catch (Exception e) {
                LOG.warn("Error closing MCP scan workers: {}", e.toString());
            }
            try {
                activity.close();
            } catch (Exception e) {
                LOG.warn("Error closing MCP session presence: {}", e.toString());
            }
            try {
                server.closeGracefully();
            } catch (Exception e) {
                LOG.warn("Error during MCP server shutdown: {}", e.toString());
            }
        }, "pwt-mcp-shutdown"));

        LOG.warn("pwt {} MCP stdio server started (data-dir {}, scans {})",
                BuildInfo.VERSION, dataDir, allowScans ? "enabled" : "read-only");
        CountDownLatch stayAlive = new CountDownLatch(1);
        try {
            stayAlive.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            LOG.warn("MCP server interrupted, shutting down");
        } finally {
            try {
                server.close();
            } catch (Exception e) {
                LOG.warn("Error closing MCP server: {}", e.toString());
            }
        }
    }
}

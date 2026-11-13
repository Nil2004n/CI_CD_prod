package pwt.mcp;

import io.modelcontextprotocol.server.McpServer;
import io.modelcontextprotocol.server.McpSyncServer;
import io.modelcontextprotocol.server.transport.StdioServerTransportProvider;
import io.modelcontextprotocol.spec.McpSchema;
import java.util.concurrent.CountDownLatch;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.application.BuildInfo;

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
        System.setProperty("org.slf4j.simpleLogger.logFile", "System.err");

        StdioServerTransportProvider transport = new StdioServerTransportProvider();
        McpSyncServer server = McpServer.sync(transport)
                .serverInfo(BuildInfo.APP_NAME, BuildInfo.VERSION)
                .capabilities(McpSchema.ServerCapabilities.builder().tools(true).build())
                .tools(PwtMcpTools.pwtInfoTool())
                .build();

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            try {
                server.closeGracefully();
            } catch (Exception e) {
                LOG.warn("Error during MCP server shutdown: {}", e.toString());
            }
        }, "pwt-mcp-shutdown"));

        LOG.warn("pwt {} MCP stdio server started (tools: {})", BuildInfo.VERSION, PwtMcpTools.TOOL_NAME);
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

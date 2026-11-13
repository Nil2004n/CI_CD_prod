package pwt.mcp;

import io.modelcontextprotocol.server.McpSyncServer;
import java.nio.file.Path;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Supplier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.infrastructure.SqliteStore;

/**
 * MCP session presence for one stdio server process. Lifecycle, all on
 * stderr/DB (never protocol stdout):
 * <ul>
 *   <li>STARTING row written at server build;</li>
 *   <li>first observed tool call flips to CONNECTED with self-reported
 *       client name/version (a tool call can only happen post-initialize);</li>
 *   <li>2-second heartbeat proves server liveness (not client
 *       responsiveness);</li>
 *   <li>every tool call refreshes last-client-activity;</li>
 *   <li>orderly shutdown marks DISCONNECTED; abrupt death leaves a fresh
 *       heartbeat to expire so the GUI reports STALE, never eternal
 *       CONNECTED.</li>
 * </ul>
 * All updates are conditional on the row not being DISCONNECTED, so neither
 * heartbeats nor late tool calls can resurrect a closed session.
 */
public final class McpActivity implements AutoCloseable {

    private static final Logger LOG = LoggerFactory.getLogger(McpActivity.class);

    private final Path dataDir;
    private final String sessionId;
    private final String serverInstanceId;
    private final String permissionMode;
    private final Supplier<McpSyncServer> server;
    private final ScheduledExecutorService heartbeat;
    private final AtomicBoolean warned = new AtomicBoolean(false);

    public McpActivity(Path dataDir, boolean allowScans, Supplier<McpSyncServer> server) {
        this.dataDir = dataDir;
        this.sessionId = java.util.UUID.randomUUID().toString();
        this.serverInstanceId = java.util.UUID.randomUUID().toString();
        this.permissionMode = allowScans ? "SCANS_ENABLED" : "READ_ONLY";
        this.server = server;
        this.heartbeat = Executors.newSingleThreadScheduledExecutor(runnable -> {
            Thread worker = new Thread(runnable, "pwt-mcp-heartbeat");
            worker.setDaemon(true);
            return worker;
        });
    }

    /** Writes the STARTING row and begins heartbeats. Best-effort: never breaks server startup. */
    public void begin() {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.insertMcpSession(sessionId, serverInstanceId, permissionMode);
        } catch (Exception e) {
            warnOnce("cannot record MCP session presence: " + e);
            return;
        }
        heartbeat.scheduleWithFixedDelay(() -> {
            try (SqliteStore store = SqliteStore.open(dataDir)) {
                store.heartbeatMcpSession(sessionId);
            } catch (Exception e) {
                warnOnce("MCP heartbeat failed: " + e);
            }
        }, 2, 2, TimeUnit.SECONDS);
    }

    /** Call at the top of every tool handler with the live exchange. */
    public void observed(io.modelcontextprotocol.server.McpSyncServerExchange exchange) {
        String clientName = null;
        String clientVersion = null;
        if (exchange != null) {
            try {
                var info = exchange.getClientInfo();
                if (info != null) {
                    clientName = info.name();
                    clientVersion = info.version();
                }
            } catch (Exception ignored) {
                // pre-initialize or unavailable: activity still counts
            }
        }
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.touchMcpSessionActivity(sessionId);
            if (clientName != null || clientVersion != null) {
                try {
                    store.markMcpSessionConnected(sessionId, clientName, clientVersion, null);
                } catch (Exception e) {
                    LOG.debug("MCP session connect flip failed: {}", e.toString());
                }
            } else {
                // No client identity yet: still flip STARTING to CONNECTED,
                // since only a post-handshake client can invoke tools.
                try {
                    store.markMcpSessionConnected(sessionId, null, null, null);
                } catch (Exception e) {
                    LOG.debug("MCP session connect flip failed: {}", e.toString());
                }
            }
        } catch (Exception e) {
            LOG.debug("MCP activity update failed: {}", e.toString());
        }
    }

    private void warnOnce(String message) {
        if (warned.compareAndSet(false, true)) {
            LOG.warn(message);
        }
    }

    public String sessionId() {
        return sessionId;
    }

    @Override
    public void close() {
        heartbeat.shutdownNow();
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.closeMcpSession(sessionId, "shutdown");
        } catch (Exception e) {
            LOG.warn("MCP session close failed: {}", e.toString());
        }
    }
}

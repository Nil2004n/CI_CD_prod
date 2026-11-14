package pwt.launcher;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import pwt.application.ScanService;
import pwt.infrastructure.SqliteStore;
import pwt.mcp.McpPresence;

/**
 * Reports recorded MCP session presence from the shared store: live, stale,
 * and disconnected sessions with permission modes. Reads recorded lifecycle
 * state only — it never infers a connection from installed configuration.
 * Diagnostics on stderr; the report object on stdout. Exit 0 always.
 */
public final class McpStatusCommand {

    private McpStatusCommand() {
    }

    public static int execute(List<String> args) {
        java.nio.file.Path dataDir = ScanService.defaultDataDir();
        for (int i = 0; i < args.size(); i++) {
            if (args.get(i).equals("--data-dir")) {
                if (++i >= args.size()) {
                    System.err.println("pwt mcp-status: missing value for --data-dir");
                    return 2;
                }
                dataDir = java.nio.file.Path.of(args.get(i));
            } else {
                System.err.println("pwt mcp-status: unknown argument: " + args.get(i));
                return 2;
            }
        }
        List<McpPresence.SessionView> views;
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            views = McpPresence.summarize(store.listLiveMcpSessions(), Instant.now());
        } catch (Exception e) {
            System.err.println("pwt mcp-status: cannot open data directory: " + e.getMessage());
            return 1;
        }
        StringBuilder sessions = new StringBuilder();
        for (McpPresence.SessionView view : views) {
            // Session IDs are random UUIDs, safe to print in full for operator matching.
            sessions.append("{\"session_id\":\"").append(view.sessionId()).append("\","
                    + "\"client\":\"").append(JsonWriter.escape(view.clientLabel())).append("\","
                    + "\"permission_mode\":\"").append(JsonWriter.escape(view.permissionMode())).append("\","
                    + "\"analysis_writes\":\"").append(view.analysisAllowed() ? "ALLOWED" : "DENIED")
                    .append("\","
                    + "\"presence\":\"").append(view.presence().name()).append("\","
                    + "\"connected_at\":").append(jsonString(view.connectedAt())).append(","
                    + "\"last_activity\":").append(jsonString(view.lastActivityAt())).append(","
                    + "\"heartbeat\":").append(jsonString(view.heartbeatAge())).append("},");
        }
        if (!views.isEmpty()) {
            sessions.deleteCharAt(sessions.length() - 1);
        }
        System.out.println("{\"data_dir\":\"" + JsonWriter.escape(dataDir.toAbsolutePath().normalize().toString()) + "\","
                + "\"database\":\"" + JsonWriter.escape(
                        dataDir.toAbsolutePath().normalize().resolve("pwt.db").toString()) + "\","
                + "\"sessions\":[" + sessions + "],"
                + "\"status_line\":\"" + JsonWriter.escape(McpPresence.statusLine(views)) + "\"}");
        return 0;
    }

    private static String jsonString(String value) {
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

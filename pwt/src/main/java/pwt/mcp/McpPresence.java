package pwt.mcp;

import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import pwt.infrastructure.SqliteStore;

/**
 * Toolkit-free interpretation of MCP session rows for the GUI presence
 * display. A session counts as live only when it is not closed AND its
 * server heartbeat is fresh; anything else is stale/unknown — never
 * CONNECTED merely because a process started or a config file exists.
 */
public final class McpPresence {

    /** Heartbeat older than this means the server is gone (abrupt death). */
    public static final Duration STALE_AFTER = Duration.ofSeconds(10);

    public enum Presence {
        CONNECTED,
        STARTING,
        STALE,
        UNKNOWN
    }

    public record SessionView(String sessionId, String clientLabel, String permissionMode,
            Presence presence, String connectedAt, String lastActivityAt, String heartbeatAge) {
    }

    private McpPresence() {
    }

    public static Presence classify(SqliteStore.McpSessionRow row, Instant now) {
        if (row == null || "DISCONNECTED".equals(row.lifecycleState())) {
            return Presence.UNKNOWN;
        }
        Instant heartbeat = parse(row.serverHeartbeatAt());
        if (heartbeat == null || Duration.between(heartbeat, now).compareTo(STALE_AFTER) > 0) {
            return Presence.STALE;
        }
        if ("CONNECTED".equals(row.lifecycleState())) {
            return Presence.CONNECTED;
        }
        return Presence.STARTING;
    }

    private static Instant parse(String timestamp) {
        if (timestamp == null || timestamp.isBlank()) {
            return null;
        }
        try {
            return Instant.parse(timestamp);
        } catch (Exception e) {
            return null;
        }
    }

    private static String ageOf(String timestamp, Instant now) {
        Instant parsed = parse(timestamp);
        if (parsed == null) {
            return "unknown";
        }
        long seconds = Math.max(0, Duration.between(parsed, now).getSeconds());
        if (seconds < 60) {
            return seconds + "s ago";
        }
        return (seconds / 60) + "m ago";
    }

    /** Summarizes live (non-disconnected) rows for one data directory. */
    public static List<SessionView> summarize(List<SqliteStore.McpSessionRow> rows, Instant now) {
        List<SessionView> views = new ArrayList<>();
        for (SqliteStore.McpSessionRow row : rows) {
            Presence presence = classify(row, now);
            if (presence == Presence.UNKNOWN) {
                continue;
            }
            String client = row.clientName() == null || row.clientName().isBlank()
                    ? "unknown client" : row.clientName();
            if (row.clientVersion() != null && !row.clientVersion().isBlank()) {
                client += " " + row.clientVersion();
            }
            views.add(new SessionView(row.sessionId(), client,
                    "SCANS_ENABLED".equals(row.permissionMode()) ? "Scans enabled" : "Read-only",
                    presence, row.connectedAt(), row.lastClientActivityAt(),
                    ageOf(row.serverHeartbeatAt(), now)));
        }
        return views;
    }

    /** One-line GUI status, e.g. "MCP: Claude Code connected · Read-only". */
    public static String statusLine(List<SessionView> views) {
        List<SessionView> live = new ArrayList<>();
        boolean stale = false;
        for (SessionView view : views) {
            if (view.presence() == Presence.CONNECTED) {
                live.add(view);
            } else {
                stale = true;
            }
        }
        if (live.isEmpty()) {
            return stale ? "MCP: session stale / status unknown." : "MCP: No active clients.";
        }
        if (live.size() == 1) {
            SessionView view = live.get(0);
            return "MCP: " + view.clientLabel() + " connected · " + view.permissionMode() + ".";
        }
        return "MCP: " + live.size() + " active clients.";
    }
}

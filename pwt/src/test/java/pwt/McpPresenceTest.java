package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Path;
import java.sql.DriverManager;
import java.time.Instant;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.infrastructure.SqliteStore;
import pwt.mcp.McpPresence;

/**
 * MCP session presence: additive V1→V2 migration, lifecycle transitions,
 * heartbeat semantics (never resurrects), and GUI-facing classification.
 */
class McpPresenceTest {

    @TempDir
    Path dataDir;

    private static String oldHeartbeat() {
        return Instant.now().minusSeconds(60).toString();
    }

    @Test
    void migrationIsAdditiveAndIdempotent() throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.registerRepository("/repo", "repo", "hash1");
        }
        // Simulate a pre-Phase-4 database: drop V2 artifacts, forget version 2.
        try (java.sql.Connection raw =
                DriverManager.getConnection("jdbc:sqlite:" + dataDir.resolve("pwt.db"))) {
            try (java.sql.Statement stmt = raw.createStatement()) {
                stmt.execute("DELETE FROM schema_migrations WHERE version=2");
                stmt.execute("DROP TABLE mcp_sessions");
                stmt.execute("DROP INDEX IF EXISTS idx_mcp_sessions_state");
            }
        }
        // Reopening must re-apply V2 without touching V1 data.
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            assertFalse(store.listRepositories().isEmpty(), "V1 data must survive migration");
            store.insertMcpSession("s1", "server-1", "READ_ONLY");
            assertEquals(1, store.listLiveMcpSessions().size());
        }
    }

    @Test
    void lifecycleTransitions() throws Exception {
        try (SqliteStore store = SqliteStore.open(dataDir)) {
            store.insertMcpSession("s1", "server-1", "READ_ONLY");
            assertEquals("STARTING", store.findMcpSession("s1").lifecycleState());

            // Unknown client info still flips to CONNECTED (handshake happened).
            store.markMcpSessionConnected("s1", null, null, null);
            assertEquals("CONNECTED", store.findMcpSession("s1").lifecycleState());
            String firstHeartbeat = store.findMcpSession("s1").serverHeartbeatAt();

            Thread.sleep(1100);
            store.heartbeatMcpSession("s1");
            assertTrue(store.findMcpSession("s1").serverHeartbeatAt().compareTo(firstHeartbeat) > 0);
            assertEquals(null, store.findMcpSession("s1").closeReason());

            store.touchMcpSessionActivity("s1");
            assertTrue(store.findMcpSession("s1").lastClientActivityAt() != null);

            store.closeMcpSession("s1", "shutdown");
            SqliteStore.McpSessionRow closed = store.findMcpSession("s1");
            assertEquals("DISCONNECTED", closed.lifecycleState());
            assertEquals("shutdown", closed.closeReason());

            // Closed sessions are final: heartbeat and activity cannot resurrect.
            store.heartbeatMcpSession("s1");
            store.touchMcpSessionActivity("s1");
            store.markMcpSessionConnected("s1", "x", "1", "p");
            SqliteStore.McpSessionRow still = store.findMcpSession("s1");
            assertEquals("DISCONNECTED", still.lifecycleState());
            assertEquals("shutdown", still.closeReason());
        }
    }

    @Test
    void staleClassification() {
        Instant now = Instant.now();
        SqliteStore.McpSessionRow live = new SqliteStore.McpSessionRow(
                "a", "srv", "Claude Code", "2.1", "2024-11-05", "READ_ONLY", "CONNECTED",
                now.toString(), now.toString(), now.toString(), null, null);
        assertEquals(McpPresence.Presence.CONNECTED, McpPresence.classify(live, now));

        SqliteStore.McpSessionRow dead = new SqliteStore.McpSessionRow(
                "b", "srv", "x", "1", null, "SCANS_ENABLED", "CONNECTED",
                now.toString(), oldHeartbeat(), null, null, null);
        assertEquals(McpPresence.Presence.STALE, McpPresence.classify(dead, now));

        SqliteStore.McpSessionRow starting = new SqliteStore.McpSessionRow(
                "c", "srv", null, null, null, "READ_ONLY", "STARTING",
                now.toString(), now.toString(), null, null, null);
        assertEquals(McpPresence.Presence.STARTING, McpPresence.classify(starting, now));

        SqliteStore.McpSessionRow gone = new SqliteStore.McpSessionRow(
                "d", "srv", null, null, null, "READ_ONLY", "DISCONNECTED",
                now.toString(), now.toString(), null, now.toString(), "shutdown");
        assertEquals(McpPresence.Presence.UNKNOWN, McpPresence.classify(gone, now));
        assertEquals(McpPresence.Presence.UNKNOWN, McpPresence.classify(null, now));
    }

    @Test
    void statusLines() {
        Instant now = Instant.now();
        assertEquals("MCP: No active clients.", McpPresence.statusLine(List.of()));
        McpPresence.SessionView live = new McpPresence.SessionView(
                "s", "Claude Code 2.1", "Read-only", McpPresence.Presence.CONNECTED,
                now.toString(), now.toString(), "2s ago");
        assertEquals("MCP: Claude Code 2.1 connected · Read-only.",
                McpPresence.statusLine(List.of(live)));
        McpPresence.SessionView live2 = new McpPresence.SessionView(
                "t", "opencode", "Scans enabled", McpPresence.Presence.CONNECTED,
                now.toString(), now.toString(), "1s ago");
        assertEquals("MCP: 2 active clients.",
                McpPresence.statusLine(List.of(live, live2)));
        McpPresence.SessionView stale = new McpPresence.SessionView(
                "u", "x", "Read-only", McpPresence.Presence.STALE,
                now.toString(), null, "5m ago");
        assertEquals("MCP: session stale / status unknown.",
                McpPresence.statusLine(List.of(stale)));
    }

    @Test
    void summarizeSkipsDisconnected() {
        Instant now = Instant.now();
        SqliteStore.McpSessionRow gone = new SqliteStore.McpSessionRow(
                "d", "srv", "x", "1", null, "READ_ONLY", "DISCONNECTED",
                now.toString(), now.toString(), null, now.toString(), "bye");
        SqliteStore.McpSessionRow live = new SqliteStore.McpSessionRow(
                "a", "srv", "Claude Code", "2.1", null, "SCANS_ENABLED", "CONNECTED",
                now.toString(), now.toString(), now.toString(), null, null);
        var views = McpPresence.summarize(List.of(gone, live), now);
        assertEquals(1, views.size());
        assertEquals("Scans enabled", views.get(0).permissionMode());
        assertEquals("Claude Code 2.1", views.get(0).clientLabel());
    }
}

package pwt.infrastructure;

import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import pwt.domain.Finding;
import pwt.domain.ScanState;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerStatus;

/**
 * SQLite-backed durable state for Phase 2 scans. One database per data
 * directory; short transactions only (never held open while a scanner runs).
 * Connections are single-thread-guarded with synchronized methods; separate
 * processes coordinate through {@link RepoLock}, not through this class.
 */
public final class SqliteStore implements AutoCloseable {

    private static final int SCHEMA_VERSION = 3;

    private final Connection connection;

    private SqliteStore(Connection connection) {
        this.connection = connection;
    }

    public static SqliteStore open(Path dataDir) throws SQLException {
        try {
            boolean created = !Files.isDirectory(dataDir);
            Files.createDirectories(dataDir);
            if (created) {
                // Owner-only by default; best effort on non-POSIX filesystems.
                try {
                    Files.setPosixFilePermissions(dataDir, java.util.EnumSet.of(
                            java.nio.file.attribute.PosixFilePermission.OWNER_READ,
                            java.nio.file.attribute.PosixFilePermission.OWNER_WRITE,
                            java.nio.file.attribute.PosixFilePermission.OWNER_EXECUTE));
                } catch (Exception ignored) {
                    // non-POSIX filesystem: umask applies
                }
            }
        } catch (Exception e) {
            throw new SQLException("Cannot create data directory: " + dataDir, e);
        }
        Path db = dataDir.resolve("pwt.db");
        Properties props = new Properties();
        props.setProperty("busy_timeout", "5000");
        props.setProperty("foreign_keys", "true");
        Connection connection;
        try {
            connection = DriverManager.getConnection("jdbc:sqlite:" + db, props);
        } catch (SQLException e) {
            throw new SQLException("Cannot open SQLite database at " + db + ": " + e.getMessage(), e);
        }
        try (Statement pragma = connection.createStatement()) {
            pragma.execute("PRAGMA journal_mode=WAL");
            pragma.execute("PRAGMA foreign_keys=ON");
        }
        SqliteStore store = new SqliteStore(connection);
        store.migrate();
        return store;
    }

    private static String now() {
        return Instant.now().toString();
    }

    private synchronized void migrate() throws SQLException {
        try (Statement stmt = connection.createStatement()) {
            stmt.execute("CREATE TABLE IF NOT EXISTS schema_migrations("
                    + "version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL)");
        }
        int current = 0;
        try (Statement stmt = connection.createStatement();
                ResultSet rs = stmt.executeQuery("SELECT COALESCE(MAX(version),0) FROM schema_migrations")) {
            if (rs.next()) {
                current = rs.getInt(1);
            }
        }
        if (current < 1) {
            applyV1();
            try (PreparedStatement ps = connection.prepareStatement(
                    "INSERT INTO schema_migrations(version, applied_at) VALUES(1, ?)")) {
                ps.setString(1, now());
                ps.executeUpdate();
            }
            current = 1;
        }
        if (current < 2) {
            applyV2();
            try (PreparedStatement ps = connection.prepareStatement(
                    "INSERT INTO schema_migrations(version, applied_at) VALUES(2, ?)")) {
                ps.setString(1, now());
                ps.executeUpdate();
            }
            current = 2;
        }
        if (current < 3) {
            applyV3();
            try (PreparedStatement ps = connection.prepareStatement(
                    "INSERT INTO schema_migrations(version, applied_at) VALUES(3, ?)")) {
                ps.setString(1, now());
                ps.executeUpdate();
            }
            current = 3;
        }
        if (current > SCHEMA_VERSION) {
            throw new SQLException("Database schema version " + current + " is newer than supported "
                    + SCHEMA_VERSION + "; refusing to open");
        }
    }

    private synchronized void applyV1() throws SQLException {
        String[] ddl = {
            "CREATE TABLE IF NOT EXISTS repositories("
                + "id TEXT PRIMARY KEY, display_name TEXT NOT NULL, canonical_root TEXT NOT NULL UNIQUE, "
                + "identity_hash TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS runs("
                + "id TEXT PRIMARY KEY, repository_id TEXT NOT NULL REFERENCES repositories(id), "
                + "state TEXT NOT NULL, verdict TEXT NOT NULL, created_at TEXT NOT NULL, "
                + "started_at TEXT, finished_at TEXT, cancel_requested INTEGER NOT NULL DEFAULT 0, "
                + "app_version TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', "
                + "summary_json TEXT NOT NULL DEFAULT '{}', error_code TEXT, error_message TEXT)",
            "CREATE TABLE IF NOT EXISTS run_files("
                + "id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE, "
                + "relative_path TEXT NOT NULL, content_sha256 TEXT NOT NULL, size_bytes INTEGER NOT NULL, "
                + "UNIQUE(run_id, relative_path))",
            "CREATE TABLE IF NOT EXISTS scanner_runs("
                + "id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE, "
                + "adapter_id TEXT NOT NULL, tool_name TEXT NOT NULL, tool_version TEXT, status TEXT NOT NULL, "
                + "exit_code INTEGER, message TEXT, UNIQUE(run_id, adapter_id))",
            "CREATE TABLE IF NOT EXISTS findings("
                + "id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE, "
                + "scanner_run_id TEXT NOT NULL REFERENCES scanner_runs(id) ON DELETE CASCADE, "
                + "rule_id TEXT NOT NULL, title TEXT NOT NULL, severity TEXT NOT NULL, original_severity TEXT, "
                + "category TEXT NOT NULL, relative_path TEXT, line_start INTEGER, line_end INTEGER, "
                + "column_start INTEGER, message TEXT NOT NULL, evidence_json TEXT NOT NULL DEFAULT '{}', "
                + "fingerprint TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS artifacts("
                + "id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE, "
                + "kind TEXT NOT NULL CHECK(kind IN ('JSON_REPORT','MARKDOWN_REPORT')), status TEXT NOT NULL, "
                + "relative_path TEXT, content_sha256 TEXT, size_bytes INTEGER, generated_at TEXT NOT NULL, "
                + "error_code TEXT, UNIQUE(run_id, kind))",
            "CREATE INDEX IF NOT EXISTS idx_runs_repo ON runs(repository_id, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_findings_run ON findings(run_id, severity)",
        };
        try (Statement stmt = connection.createStatement()) {
            for (String sql : ddl) {
                stmt.execute(sql);
            }
        }
    }

    public record RepositoryRow(String id, String displayName, String canonicalRoot, String identityHash) {
    }

    /**
     * Additive migration V3: agent analysis threads. One thread per
     * (repository, run); each save appends a revision row; structured
     * per-finding assessments reference findings without mutating them.
     * Scanner-owned tables are untouched.
     */
    private synchronized void applyV3() throws SQLException {
        try (Statement stmt = connection.createStatement()) {
            stmt.execute("CREATE TABLE IF NOT EXISTS analysis_threads("
                    + "id TEXT PRIMARY KEY, repository_id TEXT NOT NULL REFERENCES repositories(id), "
                    + "run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE, "
                    + "current_revision INTEGER NOT NULL DEFAULT 0, "
                    + "created_at TEXT NOT NULL, updated_at TEXT NOT NULL, "
                    + "UNIQUE(repository_id, run_id))");
            stmt.execute("CREATE TABLE IF NOT EXISTS analysis_revisions("
                    + "id TEXT PRIMARY KEY, thread_id TEXT NOT NULL REFERENCES analysis_threads(id) ON DELETE CASCADE, "
                    + "revision INTEGER NOT NULL, summary TEXT NOT NULL DEFAULT '', "
                    + "priority TEXT NOT NULL CHECK(priority IN ('LOW','MEDIUM','HIGH')), "
                    + "priority_rationale TEXT NOT NULL DEFAULT '', "
                    + "category TEXT NOT NULL DEFAULT '', change_status TEXT NOT NULL DEFAULT '', "
                    + "verification_run_id TEXT NULL REFERENCES runs(id), "
                    + "author_client TEXT NULL, author_name TEXT NULL, created_at TEXT NOT NULL, "
                    + "UNIQUE(thread_id, revision))");
            stmt.execute("CREATE TABLE IF NOT EXISTS analysis_assessments("
                    + "id TEXT PRIMARY KEY, revision_id TEXT NOT NULL REFERENCES analysis_revisions(id) ON DELETE CASCADE, "
                    + "finding_id TEXT NULL REFERENCES findings(id), "
                    + "rule_id TEXT NOT NULL DEFAULT '', relative_path TEXT NULL, line_start INTEGER NULL, "
                    + "priority TEXT NOT NULL CHECK(priority IN ('LOW','MEDIUM','HIGH')), "
                    + "rationale TEXT NOT NULL DEFAULT '', category TEXT NOT NULL DEFAULT '', "
                    + "change_status TEXT NOT NULL DEFAULT '', verification_run_id TEXT NULL REFERENCES runs(id), "
                    + "message TEXT NOT NULL DEFAULT '')");
            stmt.execute("CREATE INDEX IF NOT EXISTS idx_analysis_threads_repo "
                    + "ON analysis_threads(repository_id, run_id)");
            stmt.execute("CREATE INDEX IF NOT EXISTS idx_analysis_revisions_thread "
                    + "ON analysis_revisions(thread_id, revision)");
            boolean hasAnalysisWrites = false;
            try (java.sql.ResultSet columns = stmt.executeQuery("PRAGMA table_info(mcp_sessions)")) {
                while (columns.next()) {
                    if ("analysis_writes".equalsIgnoreCase(columns.getString("name"))) {
                        hasAnalysisWrites = true;
                    }
                }
            }
            if (!hasAnalysisWrites) {
                stmt.execute("ALTER TABLE mcp_sessions ADD COLUMN analysis_writes TEXT NOT NULL DEFAULT 'DENIED'");
            }
        }
    }

    public record McpSessionRow(String sessionId, String serverInstanceId, String clientName,
            String clientVersion, String protocolVersion, String permissionMode, String lifecycleState,
            String connectedAt, String serverHeartbeatAt, String lastClientActivityAt,
            String disconnectedAt, String closeReason, String analysisWrites) {
        public boolean analysisAllowed() {
            return "ALLOWED".equals(analysisWrites);
        }
    }

    /**
     * Additive migration V2: MCP session presence. One row per server
     * process session; the GUI derives live/stale/disconnected from
     * lifecycle_state plus heartbeat age. No existing table is altered.
     */
    private synchronized void applyV2() throws SQLException {
        try (Statement stmt = connection.createStatement()) {
            stmt.execute("CREATE TABLE IF NOT EXISTS mcp_sessions("
                    + "session_id TEXT PRIMARY KEY, server_instance_id TEXT NOT NULL, "
                    + "client_name TEXT, client_version TEXT, protocol_version TEXT, "
                    + "permission_mode TEXT NOT NULL CHECK(permission_mode IN ('READ_ONLY','SCANS_ENABLED')), "
                    + "lifecycle_state TEXT NOT NULL "
                    + "CHECK(lifecycle_state IN ('STARTING','CONNECTED','DISCONNECTED')), "
                    + "connected_at TEXT NOT NULL, server_heartbeat_at TEXT NOT NULL, "
                    + "last_client_activity_at TEXT, disconnected_at TEXT, close_reason TEXT)");
            stmt.execute("CREATE INDEX IF NOT EXISTS idx_mcp_sessions_state "
                    + "ON mcp_sessions(lifecycle_state, server_heartbeat_at)");
        }
    }

    public record ScannerRunRow(String id, String adapterId, String toolName, String toolVersion,
            ScannerStatus status, Integer exitCode, String message) {
    }

    public record FindingRow(String id, String scannerRunId, Finding finding) {
    }

    public record RunView(String id, String repositoryId, ScanState state, ScanVerdict verdict,
            String createdAt, String startedAt, String finishedAt, String appVersion,
            String configJson, String summaryJson, String errorCode, String errorMessage,
            boolean cancelRequested) {
    }

    public record ArtifactRow(String kind, String status, String relativePath, String sha256,
            Long sizeBytes, String generatedAt, String errorCode) {
    }

    public record AnalysisThreadView(String threadId, String repositoryId, String runId, int currentRevision) {
    }

    /** Finds the analysis thread for a run, or null when none was ever saved. */
    public synchronized AnalysisThreadView findAnalysisThread(String repositoryId, String runId)
            throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, repository_id, run_id, current_revision FROM analysis_threads"
                        + " WHERE repository_id=? AND run_id=?")) {
            ps.setString(1, repositoryId);
            ps.setString(2, runId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return new AnalysisThreadView(rs.getString(1), rs.getString(2), rs.getString(3),
                            rs.getInt(4));
                }
            }
        }
        return null;
    }

    public record AnalysisRevisionRow(String id, String threadId, int revision, String summary,
            String priority, String priorityRationale, String category, String changeStatus,
            String verificationRunId, String authorClient, String authorName, String createdAt) {
    }

    public record AnalysisAssessmentInput(String findingId, String ruleId, String relativePath,
            Integer lineStart, String priority, String rationale, String category,
            String changeStatus, String verificationRunId, String message) {
    }

    public record AnalysisAssessmentRow(String id, String revisionId, String findingId, String ruleId,
            String relativePath, Integer lineStart, String priority, String rationale,
            String category, String changeStatus, String verificationRunId, String message) {
    }

    public record AnalysisSaveResult(boolean conflict, int currentRevision, int savedRevision,
            String revisionId) {
    }

    /** Gets or creates the analysis thread for one (repository, run) pair. */
    public synchronized AnalysisThreadView getOrCreateAnalysisThread(String repositoryId, String runId)
            throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, repository_id, run_id, current_revision FROM analysis_threads"
                        + " WHERE repository_id=? AND run_id=?")) {
            ps.setString(1, repositoryId);
            ps.setString(2, runId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return new AnalysisThreadView(rs.getString(1), rs.getString(2), rs.getString(3),
                            rs.getInt(4));
                }
            }
        }
        String id = java.util.UUID.randomUUID().toString();
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT INTO analysis_threads(id, repository_id, run_id, current_revision, created_at, updated_at)"
                        + " VALUES(?,?,?,0,?,?)")) {
            String timestamp = now();
            ps.setString(1, id);
            ps.setString(2, repositoryId);
            ps.setString(3, runId);
            ps.setString(4, timestamp);
            ps.setString(5, timestamp);
            ps.executeUpdate();
        }
        return new AnalysisThreadView(id, repositoryId, runId, 0);
    }

    private static void checkText(String field, String value, int max) throws SQLException {
        if (value != null && value.length() > max) {
            throw new SQLException("analysis field too long: " + field + " (max " + max + " chars)");
        }
    }

    static String sanitizeAnalysisText(String value) {
        if (value == null) {
            return "";
        }
        StringBuilder clean = new StringBuilder();
        for (int i = 0; i < value.length() && clean.length() < 4000; i++) {
            char c = value.charAt(i);
            if (c == '\n' || c == '\r' || c == '\t') {
                clean.append(c);
            } else if (c < 0x20 || c == 0x7F) {
                clean.append('?');
            } else {
                clean.append(c);
            }
        }
        return clean.toString().strip();
    }

    private static void checkPriority(String priority) throws SQLException {
        if (!"LOW".equals(priority) && !"MEDIUM".equals(priority) && !"HIGH".equals(priority)) {
            throw new SQLException("analysis priority must be LOW, MEDIUM, or HIGH");
        }
    }

    private String runRepository(String runId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT repository_id FROM runs WHERE id=?")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return rs.getString(1);
                }
            }
        }
        return null;
    }

    private String findingRun(String findingId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT run_id FROM findings WHERE id=?")) {
            ps.setString(1, findingId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return rs.getString(1);
                }
            }
        }
        return null;
    }

    /**
     * Appends a revision with optimistic concurrency: expectedRevision must
     * equal the thread's current revision or a conflict is reported (no write).
     */
    public synchronized AnalysisSaveResult saveAnalysis(String threadId, int expectedRevision,
            String summary, String priority, String priorityRationale, String category,
            String changeStatus, String verificationRunId, String authorClient, String authorName,
            List<AnalysisAssessmentInput> assessments) throws SQLException {
        AnalysisThreadView thread;
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, repository_id, run_id, current_revision FROM analysis_threads WHERE id=?")) {
            ps.setString(1, threadId);
            try (ResultSet rs = ps.executeQuery()) {
                if (!rs.next()) {
                    throw new SQLException("unknown analysis thread: " + threadId);
                }
                thread = new AnalysisThreadView(rs.getString(1), rs.getString(2), rs.getString(3), rs.getInt(4));
            }
        }
        if (expectedRevision != thread.currentRevision()) {
            return new AnalysisSaveResult(true, thread.currentRevision(), -1, null);
        }
        checkPriority(priority);
        checkText("summary", summary, 4000);
        checkText("priority_rationale", priorityRationale, 2000);
        if (verificationRunId != null && !verificationRunId.isBlank()) {
            String verificationRepo = runRepository(verificationRunId);
            if (verificationRepo == null) {
                throw new SQLException("verification run not found: " + verificationRunId);
            }
            if (!verificationRepo.equals(thread.repositoryId())) {
                throw new SQLException("verification run belongs to a different repository");
            }
        }
        List<AnalysisAssessmentInput> items = assessments == null ? List.of() : assessments;
        for (AnalysisAssessmentInput item : items) {
            checkPriority(item.priority());
            checkText("assessment message", item.message(), 2000);
            checkText("assessment rationale", item.rationale(), 2000);
            if (item.findingId() != null && !item.findingId().isBlank()) {
                String findingRun = findingRun(item.findingId());
                if (findingRun == null) {
                    throw new SQLException("finding not found: " + item.findingId());
                }
                if (!findingRun.equals(thread.runId())) {
                    throw new SQLException("finding belongs to a different run: " + item.findingId());
                }
            }
            if (item.verificationRunId() != null && !item.verificationRunId().isBlank()) {
                String verificationRepo = runRepository(item.verificationRunId());
                if (verificationRepo == null) {
                    throw new SQLException("verification run not found: " + item.verificationRunId());
                }
                if (!verificationRepo.equals(thread.repositoryId())) {
                    throw new SQLException("verification run belongs to a different repository");
                }
            }
        }
        int nextRevision = thread.currentRevision() + 1;
        String revisionId = java.util.UUID.randomUUID().toString();
        String timestamp = now();
        boolean previous = connection.getAutoCommit();
        connection.setAutoCommit(false);
        try {
            try (PreparedStatement ps = connection.prepareStatement(
                    "INSERT INTO analysis_revisions(id, thread_id, revision, summary, priority,"
                            + " priority_rationale, category, change_status, verification_run_id,"
                            + " author_client, author_name, created_at)"
                            + " VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")) {
                ps.setString(1, revisionId);
                ps.setString(2, threadId);
                ps.setInt(3, nextRevision);
                ps.setString(4, sanitizeAnalysisText(summary));
                ps.setString(5, priority);
                ps.setString(6, sanitizeAnalysisText(priorityRationale));
                ps.setString(7, sanitizeAnalysisText(category));
                ps.setString(8, sanitizeAnalysisText(changeStatus));
                setNullable(ps, 9, verificationRunId);
                setNullable(ps, 10, authorClient);
                setNullable(ps, 11, authorName);
                ps.setString(12, timestamp);
                ps.executeUpdate();
            }
            try (PreparedStatement ps = connection.prepareStatement(
                    "INSERT INTO analysis_assessments(id, revision_id, finding_id, rule_id,"
                            + " relative_path, line_start, priority, rationale, category,"
                            + " change_status, verification_run_id, message)"
                            + " VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")) {
                for (AnalysisAssessmentInput item : items) {
                    ps.setString(1, java.util.UUID.randomUUID().toString());
                    ps.setString(2, revisionId);
                    setNullable(ps, 3, blankToNull(item.findingId()));
                    ps.setString(4, sanitizeAnalysisText(item.ruleId()));
                    setNullable(ps, 5, item.relativePath());
                    if (item.lineStart() == null) {
                        ps.setNull(6, java.sql.Types.INTEGER);
                    } else {
                        ps.setInt(6, item.lineStart());
                    }
                    ps.setString(7, item.priority());
                    ps.setString(8, sanitizeAnalysisText(item.rationale()));
                    ps.setString(9, sanitizeAnalysisText(item.category()));
                    ps.setString(10, sanitizeAnalysisText(item.changeStatus()));
                    setNullable(ps, 11, blankToNull(item.verificationRunId()));
                    ps.setString(12, sanitizeAnalysisText(item.message()));
                    ps.addBatch();
                }
                ps.executeBatch();
            }
            try (PreparedStatement ps = connection.prepareStatement(
                    "UPDATE analysis_threads SET current_revision=?, updated_at=? WHERE id=?")) {
                ps.setInt(1, nextRevision);
                ps.setString(2, timestamp);
                ps.setString(3, threadId);
                ps.executeUpdate();
            }
            connection.commit();
        } catch (SQLException e) {
            connection.rollback();
            throw e;
        } finally {
            connection.setAutoCommit(previous);
        }
        return new AnalysisSaveResult(false, nextRevision, nextRevision, revisionId);
    }

    private static void setNullable(PreparedStatement ps, int index, String value) throws SQLException {
        if (value == null) {
            ps.setNull(index, java.sql.Types.VARCHAR);
        } else {
            ps.setString(index, value);
        }
    }

    private static String blankToNull(String value) {
        return value == null || value.isBlank() ? null : value;
    }

    public synchronized AnalysisRevisionRow getAnalysisRevision(String threadId, int revision)
            throws SQLException {
        String sql = revision < 0
                ? "SELECT id, thread_id, revision, summary, priority, priority_rationale, category,"
                        + " change_status, verification_run_id, author_client, author_name, created_at"
                        + " FROM analysis_revisions WHERE thread_id=? ORDER BY revision DESC LIMIT 1"
                : "SELECT id, thread_id, revision, summary, priority, priority_rationale, category,"
                        + " change_status, verification_run_id, author_client, author_name, created_at"
                        + " FROM analysis_revisions WHERE thread_id=? AND revision=?";
        try (PreparedStatement ps = connection.prepareStatement(sql)) {
            ps.setString(1, threadId);
            if (revision >= 0) {
                ps.setInt(2, revision);
            }
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return new AnalysisRevisionRow(rs.getString(1), rs.getString(2), rs.getInt(3),
                            rs.getString(4), rs.getString(5), rs.getString(6), rs.getString(7),
                            rs.getString(8), rs.getString(9), rs.getString(10), rs.getString(11),
                            rs.getString(12));
                }
            }
        }
        return null;
    }

    public synchronized List<Integer> listAnalysisRevisions(String threadId) throws SQLException {
        List<Integer> revisions = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT revision FROM analysis_revisions WHERE thread_id=? ORDER BY revision")) {
            ps.setString(1, threadId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    revisions.add(rs.getInt(1));
                }
            }
        }
        return revisions;
    }

    public synchronized List<AnalysisAssessmentRow> listAnalysisAssessments(String revisionId)
            throws SQLException {
        List<AnalysisAssessmentRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, revision_id, finding_id, rule_id, relative_path, line_start, priority,"
                        + " rationale, category, change_status, verification_run_id, message"
                        + " FROM analysis_assessments WHERE revision_id=? ORDER BY rowid")) {
            ps.setString(1, revisionId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    Object line = rs.getObject(6);
                    rows.add(new AnalysisAssessmentRow(rs.getString(1), rs.getString(2), rs.getString(3),
                            rs.getString(4), rs.getString(5),
                            line == null ? null : rs.getInt(6),
                            rs.getString(7), rs.getString(8), rs.getString(9),
                            rs.getString(10), rs.getString(11), rs.getString(12)));
                }
            }
        }
        return rows;
    }

    public synchronized void insertMcpSession(String sessionId, String serverInstanceId,
            String permissionMode, boolean allowAnalysis) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT INTO mcp_sessions(session_id, server_instance_id, permission_mode, analysis_writes,"
                        + " lifecycle_state, connected_at, server_heartbeat_at)"
                        + " VALUES(?,?,?,?,'STARTING',?,?)")) {
            String timestamp = now();
            ps.setString(1, sessionId);
            ps.setString(2, serverInstanceId);
            ps.setString(3, permissionMode);
            ps.setString(4, allowAnalysis ? "ALLOWED" : "DENIED");
            ps.setString(5, timestamp);
            ps.setString(6, timestamp);
            ps.executeUpdate();
        }
    }

    /** Marks first interaction: CONNECTED plus self-reported client metadata. Never resurrects closed rows. */
    public synchronized void markMcpSessionConnected(String sessionId, String clientName,
            String clientVersion, String protocolVersion) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE mcp_sessions SET lifecycle_state='CONNECTED', client_name=?, client_version=?,"
                        + " protocol_version=?, last_client_activity_at=?"
                        + " WHERE session_id=? AND lifecycle_state='STARTING'")) {
            ps.setString(1, clientName);
            ps.setString(2, clientVersion);
            ps.setString(3, protocolVersion);
            ps.setString(4, now());
            ps.setString(5, sessionId);
            ps.executeUpdate();
        }
    }

    /** Records client activity; never flips state, never resurrects closed rows. */
    public synchronized void touchMcpSessionActivity(String sessionId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE mcp_sessions SET last_client_activity_at=?"
                        + " WHERE session_id=? AND lifecycle_state!='DISCONNECTED'")) {
            ps.setString(1, now());
            ps.setString(2, sessionId);
            ps.executeUpdate();
        }
    }

    /** Heartbeat proves server liveness only; the WHERE clause prevents resurrecting closed sessions. */
    public synchronized void heartbeatMcpSession(String sessionId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE mcp_sessions SET server_heartbeat_at=?"
                        + " WHERE session_id=? AND lifecycle_state!='DISCONNECTED'")) {
            ps.setString(1, now());
            ps.setString(2, sessionId);
            ps.executeUpdate();
        }
    }

    public synchronized void closeMcpSession(String sessionId, String closeReason) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE mcp_sessions SET lifecycle_state='DISCONNECTED', disconnected_at=?, close_reason=?"
                        + " WHERE session_id=? AND lifecycle_state!='DISCONNECTED'")) {
            ps.setString(1, now());
            ps.setString(2, closeReason);
            ps.setString(3, sessionId);
            ps.executeUpdate();
        }
    }

    public synchronized McpSessionRow findMcpSession(String sessionId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT session_id, server_instance_id, client_name, client_version, protocol_version,"
                        + " permission_mode, lifecycle_state, connected_at, server_heartbeat_at,"
                        + " last_client_activity_at, disconnected_at, close_reason,"
                        + " COALESCE(analysis_writes, 'DENIED')"
                        + " FROM mcp_sessions WHERE session_id=?")) {
            ps.setString(1, sessionId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return readMcpSession(rs);
                }
            }
        }
        return null;
    }

    /** Non-disconnected sessions, newest heartbeat first. */
    public synchronized List<McpSessionRow> listLiveMcpSessions() throws SQLException {
        List<McpSessionRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT session_id, server_instance_id, client_name, client_version, protocol_version,"
                        + " permission_mode, lifecycle_state, connected_at, server_heartbeat_at,"
                        + " last_client_activity_at, disconnected_at, close_reason,"
                        + " COALESCE(analysis_writes, 'DENIED')"
                        + " FROM mcp_sessions WHERE lifecycle_state!='DISCONNECTED'"
                        + " ORDER BY server_heartbeat_at DESC")) {
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    rows.add(readMcpSession(rs));
                }
            }
        }
        return rows;
    }

    private static McpSessionRow readMcpSession(ResultSet rs) throws SQLException {
        return new McpSessionRow(rs.getString(1), rs.getString(2), rs.getString(3), rs.getString(4),
                rs.getString(5), rs.getString(6), rs.getString(7), rs.getString(8), rs.getString(9),
                rs.getString(10), rs.getString(11), rs.getString(12), rs.getString(13));
    }

    public synchronized RepositoryRow registerRepository(String canonicalRoot, String displayName,
            String identityHash) throws SQLException {
        try (PreparedStatement find = connection.prepareStatement(
                "SELECT id, display_name, canonical_root, identity_hash FROM repositories WHERE canonical_root=?")) {
            find.setString(1, canonicalRoot);
            try (ResultSet rs = find.executeQuery()) {
                if (rs.next()) {
                    return new RepositoryRow(rs.getString(1), rs.getString(2), rs.getString(3), rs.getString(4));
                }
            }
        }
        String id = java.util.UUID.randomUUID().toString();
        try (PreparedStatement insert = connection.prepareStatement(
                "INSERT INTO repositories(id, display_name, canonical_root, identity_hash, created_at, updated_at)"
                        + " VALUES(?,?,?,?,?,?)")) {
            insert.setString(1, id);
            insert.setString(2, displayName);
            insert.setString(3, canonicalRoot);
            insert.setString(4, identityHash);
            insert.setString(5, now());
            insert.setString(6, now());
            insert.executeUpdate();
        }
        return new RepositoryRow(id, displayName, canonicalRoot, identityHash);
    }

    public synchronized String createRun(String repositoryId, String appVersion, String configJson)
            throws SQLException {
        String id = java.util.UUID.randomUUID().toString();
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT INTO runs(id, repository_id, state, verdict, created_at, app_version, config_json)"
                        + " VALUES(?,?,'CREATED','INDETERMINATE',?,?,?)")) {
            ps.setString(1, id);
            ps.setString(2, repositoryId);
            ps.setString(3, now());
            ps.setString(4, appVersion);
            ps.setString(5, configJson);
            ps.executeUpdate();
        }
        return id;
    }

    public synchronized void startRun(String runId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE runs SET state='RUNNING', started_at=? WHERE id=?")) {
            ps.setString(1, now());
            ps.setString(2, runId);
            ps.executeUpdate();
        }
    }

    public synchronized void setCancelRequested(String runId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE runs SET cancel_requested=1 WHERE id=?")) {
            ps.setString(1, runId);
            ps.executeUpdate();
        }
    }

    public synchronized void addRunFile(String runId, String relativePath, String sha256, long sizeBytes)
            throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT OR IGNORE INTO run_files(id, run_id, relative_path, content_sha256, size_bytes)"
                        + " VALUES(?,?,?,?,?)")) {
            ps.setString(1, java.util.UUID.randomUUID().toString());
            ps.setString(2, runId);
            ps.setString(3, relativePath);
            ps.setString(4, sha256);
            ps.setLong(5, sizeBytes);
            ps.executeUpdate();
        }
    }

    public synchronized String recordScannerRun(String runId, String adapterId, String toolName, String toolVersion,
            ScannerStatus status, Integer exitCode, String message) throws SQLException {
        String id = java.util.UUID.randomUUID().toString();
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT INTO scanner_runs(id, run_id, adapter_id, tool_name, tool_version, status, exit_code, message)"
                        + " VALUES(?,?,?,?,?,?,?,?)")) {
            ps.setString(1, id);
            ps.setString(2, runId);
            ps.setString(3, adapterId);
            ps.setString(4, toolName);
            if (toolVersion == null) {
                ps.setNull(5, java.sql.Types.VARCHAR);
            } else {
                ps.setString(5, toolVersion);
            }
            ps.setString(6, status.name());
            if (exitCode == null) {
                ps.setNull(7, java.sql.Types.INTEGER);
            } else {
                ps.setInt(7, exitCode);
            }
            if (message == null) {
                ps.setNull(8, java.sql.Types.VARCHAR);
            } else {
                ps.setString(8, message);
            }
            ps.executeUpdate();
        }
        return id;
    }

    public synchronized void addFindings(String runId, String scannerRunId, List<Finding> findings)
            throws SQLException {
        boolean previous = connection.getAutoCommit();
        connection.setAutoCommit(false);
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT INTO findings(id, run_id, scanner_run_id, rule_id, title, severity, original_severity,"
                        + " category, relative_path, line_start, line_end, column_start, message, evidence_json,"
                        + " fingerprint) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")) {
            for (Finding finding : findings) {
                ps.setString(1, java.util.UUID.randomUUID().toString());
                ps.setString(2, runId);
                ps.setString(3, scannerRunId);
                ps.setString(4, finding.ruleId());
                ps.setString(5, finding.title());
                ps.setString(6, finding.severity().name());
                if (finding.originalSeverity() == null) {
                    ps.setNull(7, java.sql.Types.VARCHAR);
                } else {
                    ps.setString(7, finding.originalSeverity());
                }
                ps.setString(8, finding.category().name());
                if (finding.relativePath() == null) {
                    ps.setNull(9, java.sql.Types.VARCHAR);
                } else {
                    ps.setString(9, finding.relativePath());
                }
                setNullableInt(ps, 10, finding.lineStart());
                setNullableInt(ps, 11, finding.lineEnd());
                setNullableInt(ps, 12, finding.columnStart());
                ps.setString(13, finding.message());
                ps.setString(14, finding.evidenceJson());
                ps.setString(15, finding.fingerprint());
                ps.addBatch();
            }
            ps.executeBatch();
            connection.commit();
        } catch (SQLException e) {
            connection.rollback();
            throw e;
        } finally {
            connection.setAutoCommit(previous);
        }
    }

    private static void setNullableInt(PreparedStatement ps, int index, Integer value) throws SQLException {
        if (value == null) {
            ps.setNull(index, java.sql.Types.INTEGER);
        } else {
            ps.setInt(index, value);
        }
    }

    public synchronized void finalizeRun(String runId, ScanState state, ScanVerdict verdict, String summaryJson,
            String errorCode, String errorMessage) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "UPDATE runs SET state=?, verdict=?, finished_at=?, summary_json=?, error_code=?, error_message=?"
                        + " WHERE id=?")) {
            ps.setString(1, state.name());
            ps.setString(2, verdict.name());
            ps.setString(3, now());
            ps.setString(4, summaryJson == null ? "{}" : summaryJson);
            if (errorCode == null) {
                ps.setNull(5, java.sql.Types.VARCHAR);
            } else {
                ps.setString(5, errorCode);
            }
            if (errorMessage == null) {
                ps.setNull(6, java.sql.Types.VARCHAR);
            } else {
                ps.setString(6, errorMessage);
            }
            ps.setString(7, runId);
            ps.executeUpdate();
        }
    }

    public synchronized void recordArtifact(String runId, String kind, String status, String relativePath,
            String sha256, Long sizeBytes, String errorCode) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "INSERT OR REPLACE INTO artifacts(id, run_id, kind, status, relative_path, content_sha256,"
                        + " size_bytes, generated_at, error_code) VALUES("
                        + "COALESCE((SELECT id FROM artifacts WHERE run_id=? AND kind=?),?),"
                        + "?,?,?,?,?,?,?,?)")) {
            ps.setString(1, runId);
            ps.setString(2, kind);
            ps.setString(3, java.util.UUID.randomUUID().toString());
            ps.setString(4, runId);
            ps.setString(5, kind);
            ps.setString(6, status);
            if (relativePath == null) {
                ps.setNull(7, java.sql.Types.VARCHAR);
            } else {
                ps.setString(7, relativePath);
            }
            if (sha256 == null) {
                ps.setNull(8, java.sql.Types.VARCHAR);
            } else {
                ps.setString(8, sha256);
            }
            if (sizeBytes == null) {
                ps.setNull(9, java.sql.Types.INTEGER);
            } else {
                ps.setLong(9, sizeBytes);
            }
            ps.setString(10, now());
            if (errorCode == null) {
                ps.setNull(11, java.sql.Types.VARCHAR);
            } else {
                ps.setString(11, errorCode);
            }
            ps.executeUpdate();
        }
    }

    public synchronized List<RunView> listNonTerminalRuns() throws SQLException {
        List<RunView> runs = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, repository_id, state, verdict, created_at, started_at, finished_at, app_version,"
                        + " config_json, summary_json, error_code, error_message, cancel_requested FROM runs"
                        + " WHERE state IN ('CREATED','RUNNING') ORDER BY created_at")) {
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    runs.add(readRunView(rs));
                }
            }
        }
        return runs;
    }

    private static RunView readRunView(ResultSet rs) throws SQLException {
        return new RunView(rs.getString(1), rs.getString(2),
                ScanState.valueOf(rs.getString(3)), ScanVerdict.valueOf(rs.getString(4)),
                rs.getString(5), rs.getString(6), rs.getString(7), rs.getString(8),
                rs.getString(9), rs.getString(10), rs.getString(11), rs.getString(12),
                rs.getInt(13) != 0);
    }

    public synchronized RepositoryRow findRepositoryById(String id) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, display_name, canonical_root, identity_hash FROM repositories WHERE id=?")) {
            ps.setString(1, id);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return new RepositoryRow(rs.getString(1), rs.getString(2), rs.getString(3), rs.getString(4));
                }
            }
        }
        return null;
    }

    public record TargetRow(String relativePath, String sha256, long sizeBytes) {
    }

    public synchronized List<TargetRow> listRunFiles(String runId) throws SQLException {
        List<TargetRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT relative_path, content_sha256, size_bytes FROM run_files WHERE run_id=? ORDER BY relative_path")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    rows.add(new TargetRow(rs.getString(1), rs.getString(2), rs.getLong(3)));
                }
            }
        }
        return rows;
    }

    public synchronized List<ScannerRunRow> listScannerRuns(String runId) throws SQLException {
        List<ScannerRunRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, adapter_id, tool_name, tool_version, status, exit_code, message FROM scanner_runs"
                        + " WHERE run_id=? ORDER BY adapter_id")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    Integer exit = rs.getObject(6) == null ? null : rs.getInt(6);
                    rows.add(new ScannerRunRow(rs.getString(1), rs.getString(2), rs.getString(3), rs.getString(4),
                            ScannerStatus.valueOf(rs.getString(5)), exit, rs.getString(7)));
                }
            }
        }
        return rows;
    }

    public synchronized List<FindingRow> listFindings(String runId) throws SQLException {
        List<FindingRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, scanner_run_id, rule_id, title, severity, original_severity, category, relative_path,"
                        + " line_start, line_end, column_start, message, evidence_json, fingerprint"
                        + " FROM findings WHERE run_id=? ORDER BY severity, rule_id, relative_path")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    Finding finding = new Finding(
                            rs.getString(3), rs.getString(4),
                            pwt.domain.Severity.valueOf(rs.getString(5)), rs.getString(6),
                            pwt.domain.FindingCategory.valueOf(rs.getString(7)), rs.getString(8),
                            rs.getObject(9) == null ? null : rs.getInt(9),
                            rs.getObject(10) == null ? null : rs.getInt(10),
                            rs.getObject(11) == null ? null : rs.getInt(11),
                            rs.getString(12), rs.getString(13), rs.getString(14));
                    rows.add(new FindingRow(rs.getString(1), rs.getString(2), finding));
                }
            }
        }
        return rows;
    }

    public synchronized RunView getRun(String runId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, repository_id, state, verdict, created_at, started_at, finished_at, app_version,"
                        + " config_json, summary_json, error_code, error_message, cancel_requested"
                        + " FROM runs WHERE id=?")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return readRunView(rs);
                }
            }
        }
        return null;
    }

    /** Paginated run history for one repository, newest first. */
    public synchronized List<RunView> listRuns(String repositoryId, String stateFilter, int limit, int offset)
            throws SQLException {
        boolean filtered = stateFilter != null && !stateFilter.isBlank();
        String sql = "SELECT id, repository_id, state, verdict, created_at, started_at, finished_at, app_version,"
                + " config_json, summary_json, error_code, error_message, cancel_requested FROM runs"
                + " WHERE repository_id=?" + (filtered ? " AND state=?" : "")
                + " ORDER BY created_at DESC LIMIT ? OFFSET ?";
        List<RunView> runs = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(sql)) {
            ps.setString(1, repositoryId);
            int index = 2;
            if (filtered) {
                ps.setString(index++, stateFilter);
            }
            ps.setInt(index++, Math.max(1, limit));
            ps.setInt(index, Math.max(0, offset));
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    runs.add(readRunView(rs));
                }
            }
        }
        return runs;
    }

    public synchronized int countRuns(String repositoryId, String stateFilter) throws SQLException {
        boolean filtered = stateFilter != null && !stateFilter.isBlank();
        String sql = "SELECT COUNT(*) FROM runs WHERE repository_id=?" + (filtered ? " AND state=?" : "");
        try (PreparedStatement ps = connection.prepareStatement(sql)) {
            ps.setString(1, repositoryId);
            if (filtered) {
                ps.setString(2, stateFilter);
            }
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next() ? rs.getInt(1) : 0;
            }
        }
    }

    public synchronized int countFindings(String runId) throws SQLException {
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT COUNT(*) FROM findings WHERE run_id=?")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                return rs.next() ? rs.getInt(1) : 0;
            }
        }
    }

    public synchronized List<RepositoryRow> listRepositories() throws SQLException {
        List<RepositoryRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT id, display_name, canonical_root, identity_hash FROM repositories ORDER BY display_name")) {
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    rows.add(new RepositoryRow(rs.getString(1), rs.getString(2), rs.getString(3), rs.getString(4)));
                }
            }
        }
        return rows;
    }

    public synchronized List<ArtifactRow> listArtifacts(String runId) throws SQLException {
        List<ArtifactRow> rows = new ArrayList<>();
        try (PreparedStatement ps = connection.prepareStatement(
                "SELECT kind, status, relative_path, content_sha256, size_bytes, generated_at, error_code"
                        + " FROM artifacts WHERE run_id=? ORDER BY kind")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    Object size = rs.getObject(5);
                    rows.add(new ArtifactRow(rs.getString(1), rs.getString(2), rs.getString(3),
                            rs.getString(4), size == null ? null : rs.getLong(5),
                            rs.getString(6), rs.getString(7)));
                }
            }
        }
        return rows;
    }

    @Override
    public synchronized void close() throws SQLException {
        connection.close();
    }
}

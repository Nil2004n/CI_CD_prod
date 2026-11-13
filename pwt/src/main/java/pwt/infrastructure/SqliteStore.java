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

    private static final int SCHEMA_VERSION = 1;

    private final Connection connection;

    private SqliteStore(Connection connection) {
        this.connection = connection;
    }

    public static SqliteStore open(Path dataDir) throws SQLException {
        try {
            Files.createDirectories(dataDir);
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

    public record ScannerRunRow(String id, String adapterId, String toolName, String toolVersion,
            ScannerStatus status, Integer exitCode, String message) {
    }

    public record FindingRow(String id, String scannerRunId, Finding finding) {
    }

    public record RunView(String id, String repositoryId, ScanState state, ScanVerdict verdict,
            String createdAt, String startedAt, String finishedAt, String appVersion,
            String configJson, String summaryJson, String errorCode, String errorMessage) {
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
                        + " config_json, summary_json, error_code, error_message FROM runs"
                        + " WHERE state IN ('CREATED','RUNNING') ORDER BY created_at")) {
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    runs.add(new RunView(rs.getString(1), rs.getString(2),
                            ScanState.valueOf(rs.getString(3)), ScanVerdict.valueOf(rs.getString(4)),
                            rs.getString(5), rs.getString(6), rs.getString(7), rs.getString(8),
                            rs.getString(9), rs.getString(10), rs.getString(11), rs.getString(12)));
                }
            }
        }
        return runs;
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
                        + " config_json, summary_json, error_code, error_message FROM runs WHERE id=?")) {
            ps.setString(1, runId);
            try (ResultSet rs = ps.executeQuery()) {
                if (rs.next()) {
                    return new RunView(rs.getString(1), rs.getString(2),
                            ScanState.valueOf(rs.getString(3)), ScanVerdict.valueOf(rs.getString(4)),
                            rs.getString(5), rs.getString(6), rs.getString(7), rs.getString(8),
                            rs.getString(9), rs.getString(10), rs.getString(11), rs.getString(12));
                }
            }
        }
        return null;
    }

    @Override
    public synchronized void close() throws SQLException {
        connection.close();
    }
}

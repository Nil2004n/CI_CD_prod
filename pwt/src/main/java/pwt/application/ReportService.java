package pwt.application;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pwt.domain.Finding;
import pwt.domain.ScanState;
import pwt.infrastructure.ReportWriter;
import pwt.infrastructure.SqliteStore;
import pwt.launcher.JsonWriter;

/**
 * Builds the JSON and Markdown reports for one run from sanitized persisted
 * data (never by reparsing raw scanner output). Report-generation failures
 * are recorded as artifact rows and surfaced to the caller; they never erase
 * scan evidence.
 */
public final class ReportService {

    public static final String SCHEMA_VERSION = "1.0";

    public record ReportPaths(String jsonPath, String jsonError, String markdownPath, String markdownError) {
    }

    public ReportPaths generate(SqliteStore store, Path dataDir, String runId) throws Exception {
        SqliteStore.RunView run = store.getRun(runId);
        if (run == null) {
            throw new IllegalArgumentException("Unknown run: " + runId);
        }
        SqliteStore.RepositoryRow repo = store.findRepositoryById(run.repositoryId());
        List<SqliteStore.TargetRow> targets = store.listRunFiles(runId);
        List<SqliteStore.ScannerRunRow> scanners = store.listScannerRuns(runId);
        List<SqliteStore.FindingRow> findings = store.listFindings(runId);

        String source = "CLI";
        String config = run.configJson();
        if (config != null && config.contains("\"source\":\"GUI\"")) {
            source = "GUI";
        } else if (config != null && config.contains("\"source\":\"MCP\"")) {
            source = "MCP";
        }
        boolean complete = run.state() == ScanState.COMPLETED;
        List<String> gaps = extractGaps(run.summaryJson());
        String scopeNote = scopeNote(targets.size(), scanners.size());

        String json = buildJson(run, repo, targets, scanners, findings, complete, gaps, source, scopeNote);
        String markdown = buildMarkdown(run, repo, targets, scanners, findings, complete, gaps, source, scopeNote);

        Path artifactDir = dataDir.resolve("artifacts").resolve(runId);
        String jsonPath = null;
        String jsonError = null;
        String mdPath = null;
        String mdError = null;
        try {
            ReportWriter.WrittenFile written = ReportWriter.writeAtomically(artifactDir, "report.json", json);
            jsonPath = dataDir.relativize(written.path()).toString().replace('\\', '/');
            store.recordArtifact(runId, "JSON_REPORT", "READY", jsonPath, written.sha256(), written.sizeBytes(), null);
        } catch (Exception e) {
            jsonError = e.getMessage();
            store.recordArtifact(runId, "JSON_REPORT", "FAILED", null, null, null, "EXPORT_FAILED");
        }
        try {
            ReportWriter.WrittenFile written = ReportWriter.writeAtomically(artifactDir, "report.md", markdown);
            mdPath = dataDir.relativize(written.path()).toString().replace('\\', '/');
            store.recordArtifact(runId, "MARKDOWN_REPORT", "READY", mdPath, written.sha256(), written.sizeBytes(), null);
        } catch (Exception e) {
            mdError = e.getMessage();
            store.recordArtifact(runId, "MARKDOWN_REPORT", "FAILED", null, null, null, "EXPORT_FAILED");
        }
        return new ReportPaths(jsonPath, jsonError, mdPath, mdError);
    }

    static String scopeNote(int targetCount, int scannerCount) {
        return "This assessment applies only to " + targetCount + " explicitly selected file(s) and "
                + scannerCount + " configured check(s). It is not a repository-wide verdict:"
                + " never display or describe this result as \"repository secure\"."
                + " The six integrated checks are yamllint, gitleaks, checkov, actionlint,"
                + " kubeconform, and local-schema validation; only the checks selected for"
                + " this run were applied, and unselected or out-of-scope checks are outside"
                + " this assessment's scope.";
    }

    private static List<String> extractGaps(String summaryJson) {
        return ScanService.extractSummaryGaps(summaryJson);
    }

    private static String buildJson(SqliteStore.RunView run, SqliteStore.RepositoryRow repo,
            List<SqliteStore.TargetRow> targets, List<SqliteStore.ScannerRunRow> scanners,
            List<SqliteStore.FindingRow> findings, boolean complete, List<String> gaps,
            String source, String scopeNote) {
        StringBuilder out = new StringBuilder();
        out.append("{\"schema_version\":\"").append(SCHEMA_VERSION).append('"');
        out.append(",\"run\":{");
        field(out, "id", run.id());
        field(out, "repository_id", run.repositoryId());
        field(out, "state", run.state().name());
        field(out, "verdict", run.verdict().name());
        field(out, "created_at", run.createdAt());
        field(out, "started_at", run.startedAt());
        field(out, "finished_at", run.finishedAt());
        field(out, "source", source);
        field(out, "app_version", run.appVersion());
        field(out, "error_code", run.errorCode());
        field(out, "error_message", run.errorMessage());
        trimComma(out);
        out.append('}');
        out.append(",\"repository\":{\"display_name\":\"")
                .append(JsonWriter.escape(repo == null ? "unknown" : repo.displayName())).append("\"}");
        out.append(",\"targets\":[");
        for (SqliteStore.TargetRow target : targets) {
            out.append("{\"relative_path\":\"").append(JsonWriter.escape(target.relativePath())).append('"');
            out.append(",\"content_sha256\":\"").append(target.sha256()).append('"');
            out.append(",\"size_bytes\":").append(target.sizeBytes()).append("},");
        }
        trimComma(out);
        out.append(']');
        out.append(",\"scanner_runs\":[");
        for (SqliteStore.ScannerRunRow scanner : scanners) {
            out.append('{');
            field(out, "adapter_id", scanner.adapterId());
            field(out, "tool_name", scanner.toolName());
            field(out, "tool_version", scanner.toolVersion());
            field(out, "status", scanner.status().name());
            if (scanner.exitCode() == null) {
                out.append("\"exit_code\":null,");
            } else {
                out.append("\"exit_code\":").append(scanner.exitCode()).append(',');
            }
            field(out, "message", scanner.message());
            trimComma(out);
            out.append("},");
        }
        trimComma(out);
        out.append(']');
        out.append(",\"findings\":[");
        for (SqliteStore.FindingRow row : findings) {
            Finding finding = row.finding();
            out.append('{');
            field(out, "rule_id", finding.ruleId());
            field(out, "title", finding.title());
            field(out, "severity", finding.severity().name());
            field(out, "original_severity", finding.originalSeverity());
            field(out, "category", finding.category().name());
            field(out, "relative_path", finding.relativePath());
            numberOrNull(out, "line_start", finding.lineStart());
            numberOrNull(out, "line_end", finding.lineEnd());
            numberOrNull(out, "column_start", finding.columnStart());
            field(out, "message", finding.message());
            out.append("\"evidence\":").append(finding.evidenceJson()).append(',');
            field(out, "fingerprint", finding.fingerprint());
            trimComma(out);
            out.append("},");
        }
        trimComma(out);
        out.append(']');
        out.append(",\"coverage\":{\"complete\":").append(complete ? "true" : "false").append(",\"gaps\":[");
        for (String gap : gaps) {
            out.append('"').append(JsonWriter.escape(gap)).append("\",");
        }
        trimComma(out);
        out.append("]}");
        out.append(",\"scope_note\":\"").append(JsonWriter.escape(scopeNote)).append('"');
        out.append('}');
        return out.toString();
    }

    private static void field(StringBuilder out, String key, String value) {
        out.append('"').append(key).append("\":");
        if (value == null) {
            out.append("null,");
        } else {
            out.append('"').append(JsonWriter.escape(value)).append("\",");
        }
    }

    private static void numberOrNull(StringBuilder out, String key, Integer value) {
        out.append('"').append(key).append("\":");
        if (value == null) {
            out.append("null,");
        } else {
            out.append(value).append(',');
        }
    }

    private static void trimComma(StringBuilder out) {
        int n = out.length();
        if (n > 0 && out.charAt(n - 1) == ',') {
            out.deleteCharAt(n - 1);
        }
    }

    private static String buildMarkdown(SqliteStore.RunView run, SqliteStore.RepositoryRow repo,
            List<SqliteStore.TargetRow> targets, List<SqliteStore.ScannerRunRow> scanners,
            List<SqliteStore.FindingRow> findings, boolean complete, List<String> gaps,
            String source, String scopeNote) {
        StringBuilder md = new StringBuilder();
        md.append("# PWT scan report\n\n");
        md.append("- Run: `").append(run.id()).append("`\n");
        md.append("- Repository: ").append(repo == null ? "unknown" : repo.displayName()).append('\n');
        md.append("- Source: ").append(source).append('\n');
        md.append("- State: **").append(run.state().name()).append("**\n");
        md.append("- Verdict: **").append(run.verdict().name()).append("**\n");
        md.append("- Started: ").append(nullToDash(run.startedAt())).append('\n');
        md.append("- Finished: ").append(nullToDash(run.finishedAt())).append('\n');
        md.append("- App version: ").append(nullToDash(run.appVersion())).append('\n');
        if (run.errorMessage() != null) {
            md.append("- Note: ").append(run.errorMessage()).append('\n');
        }
        md.append("\n## Selected targets (").append(targets.size()).append(")\n\n");
        md.append("| File | SHA-256 | Bytes |\n|---|---|---|\n");
        for (SqliteStore.TargetRow target : targets) {
            md.append("| `").append(target.relativePath()).append("` | `")
                    .append(shortSha(target.sha256())).append("` | ")
                    .append(target.sizeBytes()).append(" |\n");
        }
        md.append("\n## Scanner outcomes\n\n");
        md.append("| Adapter | Tool | Version | Status | Findings |\n|---|---|---|---|---|\n");
        java.util.Map<String, Integer> counts = new java.util.HashMap<>();
        for (SqliteStore.FindingRow row : findings) {
            counts.merge(row.scannerRunId(), 1, Integer::sum);
        }
        java.util.Map<String, String> scannerIdByRow = new java.util.HashMap<>();
        for (SqliteStore.ScannerRunRow scanner : scanners) {
            scannerIdByRow.put(scanner.id(), scanner.adapterId());
            md.append("| ").append(scanner.adapterId()).append(" | ").append(scanner.toolName()).append(" | ")
                    .append(nullToDash(scanner.toolVersion())).append(" | ").append(scanner.status().name())
                    .append(" | ").append(counts.getOrDefault(scanner.id(), 0)).append(" |\n");
            if (scanner.message() != null && !scanner.message().isBlank()) {
                md.append("  - ").append(scanner.message()).append('\n');
            }
        }
        md.append("\n## Findings (").append(findings.size()).append(")\n\n");
        if (findings.isEmpty()) {
            md.append("No findings were observed in the selected scope.\n");
        } else {
            for (SqliteStore.FindingRow row : findings) {
                Finding finding = row.finding();
                md.append("### ").append(finding.ruleId()).append(" [").append(finding.severity().name())
                        .append("]\n\n");
                md.append("- File: `").append(nullToDash(finding.relativePath())).append('`');
                if (finding.lineStart() != null) {
                    md.append(" line ").append(finding.lineStart());
                }
                md.append('\n');
                md.append("- Category: ").append(finding.category().name()).append('\n');
                md.append("- Message: ").append(finding.message()).append('\n');
                md.append("- Fingerprint: `").append(finding.fingerprint()).append("`\n\n");
            }
        }
        md.append("\n## Coverage\n\n");
        md.append(complete ? "Complete for the selected scope.\n" : "Incomplete — see gaps.\n");
        for (String gap : gaps) {
            md.append("- ").append(gap).append('\n');
        }
        md.append("\n---\n").append(scopeNote).append('\n');
        return md.toString();
    }

    private static String nullToDash(String value) {
        return value == null ? "-" : value;
    }

    private static String shortSha(String sha) {
        if (sha == null) {
            return "-";
        }
        return sha.length() > 16 ? sha.substring(0, 16) + "..." : sha;
    }
}

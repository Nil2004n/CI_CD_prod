package pwt.application;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.List;
import pwt.domain.Finding;
import pwt.infrastructure.SqliteStore;

/**
 * Append-only per-repository Markdown scan journal.
 *
 * <p>Location: {@code <data-dir>/reports/<sanitized-name>--<repository-id>.md}.
 * The repository ID in the filename prevents collisions; the sanitized display
 * name keeps it human-readable. The journal is a generated reading interface
 * for the PWT agent (plain file reads, no MCP/SQL in the conversation path).
 * SQLite remains the authoritative record; this writer only ever appends
 * self-contained sections rendered from persisted sanitized rows.
 *
 * <p>Concurrency: callers must hold the repository lock while appending
 * (scan completion and reconcile paths already do; the {@code pwt journal}
 * command acquires it). Each section carries an unambiguous run-ID marker
 * and end marker, so concurrent or repeated publication can be detected:
 * a complete pair means "already current" (skip), a lone start marker
 * means "repair the tail, then append".
 *
 * <p>Failures never touch scan evidence: {@link JournalException} leaves the
 * database exactly as it was; callers log and continue, and {@code pwt journal}
 * retries later from the same stored rows.
 */
public final class JournalWriter {

    /** Max findings rendered per section; overflow is noted with a count. */
    public static final int MAX_FINDINGS_PER_SECTION = 100;

    public static final String MARKER_START_PREFIX = "<!-- pwt:run ";
    public static final String MARKER_END_PREFIX = "<!-- /pwt:run ";
    public static final String MARKER_SUFFIX = " -->";

    private static final String JOURNAL_HEADER_TITLE = "# PWT scan journal";

    private JournalWriter() {
    }

    public static final class JournalException extends Exception {
        public JournalException(String message) {
            super(message);
        }

        public JournalException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /**
     * File-safe display name. Keeps letters, digits, dot, underscore and
     * dash; every other character becomes a dash, runs collapse, leading
     * and trailing dashes/dots are stripped, capped at 48 characters.
     * The Python setup helper mirrors this exact rule.
     */
    public static String sanitizeName(String displayName) {
        String base = displayName == null ? "" : displayName.strip();
        StringBuilder clean = new StringBuilder();
        boolean lastDash = true;
        for (int i = 0; i < base.length() && clean.length() < 48; i++) {
            char c = base.charAt(i);
            boolean ok = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')
                    || (c >= '0' && c <= '9') || c == '.' || c == '_' || c == '-';
            if (ok) {
                clean.append(c);
                lastDash = false;
            } else if (!lastDash && clean.length() > 0) {
                clean.append('-');
                lastDash = true;
            }
        }
        String name = clean.toString().replaceAll("[-.]+$", "").replaceAll("^[-.]+", "");
        if (name.isBlank()) {
            name = "repo";
        }
        return name;
    }

    /** Deterministic journal path for a repository; no database lookup needed. */
    public static Path journalPath(Path dataDir, SqliteStore.RepositoryRow repo) {
        String file = sanitizeName(repo.displayName()) + "--" + repo.id() + ".md";
        return dataDir.resolve("reports").resolve(file);
    }

    /**
     * Appends the terminal run's section unless a complete section for it
     * already exists. Repairs an incomplete trailing section first.
     *
     * @return "appended" or "current".
     * @throws JournalException on non-terminal runs, unknown runs, foreign
     *         file content, or any I/O failure. The database is untouched.
     */
    public static String appendRun(SqliteStore store, Path dataDir, String runId)
            throws JournalException {
        SqliteStore.RunView run;
        SqliteStore.RepositoryRow repo;
        try {
            run = store.getRun(runId);
            if (run == null) {
                throw new JournalException("unknown run: " + runId);
            }
            if (!isTerminal(run.state().name())) {
                throw new JournalException("run " + runId + " is not terminal (state="
                        + run.state().name() + ")");
            }
            repo = store.findRepositoryById(run.repositoryId());
            if (repo == null) {
                throw new JournalException("run " + runId + " has no registered repository");
            }
        } catch (JournalException e) {
            throw e;
        } catch (Exception e) {
            throw new JournalException("cannot load run " + runId + ": " + e.getMessage(), e);
        }
        String section;
        try {
            section = renderSection(store, repo, run);
        } catch (JournalException e) {
            throw e;
        } catch (Exception e) {
            throw new JournalException("cannot render run " + runId + ": " + e.getMessage(), e);
        }
        Path journal = journalPath(dataDir, repo);
        try {
            Files.createDirectories(journal.getParent());
        } catch (Exception e) {
            throw new JournalException("cannot create reports directory: " + e.getMessage(), e);
        }
        String startMarker = MARKER_START_PREFIX + run.id() + MARKER_SUFFIX;
        String endMarker = MARKER_END_PREFIX + run.id() + MARKER_SUFFIX;
        String existing = "";
        if (Files.exists(journal)) {
            try {
                existing = Files.readString(journal, StandardCharsets.UTF_8);
            } catch (Exception e) {
                throw new JournalException("cannot read journal " + journal + ": " + e.getMessage(), e);
            }
            if (containsCompletePair(existing, startMarker, endMarker)) {
                return "current";
            }
            if (!existing.isBlank() && !existing.startsWith(JOURNAL_HEADER_TITLE)) {
                throw new JournalException("journal path holds foreign content, refusing to write: "
                        + journal);
            }
        }
        StringBuilder out = new StringBuilder();
        if (existing.isBlank()) {
            out.append(JOURNAL_HEADER_TITLE).append('\n');
            out.append("- Repository: ").append(repo.displayName()).append('\n');
            out.append("- Repository ID: `").append(repo.id()).append("`\n");
            out.append("- Root: `").append(repo.canonicalRoot()).append("`\n");
            out.append("\n---\n");
        } else {
            out.append(repairTail(existing, endMarker));
        }
        out.append('\n').append(section);
        try {
            Files.writeString(journal, out.toString(), StandardCharsets.UTF_8,
                    StandardOpenOption.CREATE, StandardOpenOption.TRUNCATE_EXISTING);
        } catch (Exception e) {
            throw new JournalException("cannot write journal " + journal + ": " + e.getMessage(), e);
        }
        return "appended";
    }

    /**
     * Appends sections for every terminal run of a repository that lacks a
     * complete section (backfill + retry). Returns one status per terminal
     * run, newest last.
     */
    public static List<JournalStatus> ensureJournal(SqliteStore store, Path dataDir,
            String repositoryId) throws JournalException {
        List<JournalStatus> done = new ArrayList<>();
        List<SqliteStore.RunView> runs;
        try {
            runs = store.listRuns(repositoryId, null, Integer.MAX_VALUE, 0);
        } catch (Exception e) {
            throw new JournalException("cannot list runs: " + e.getMessage(), e);
        }
        for (int i = runs.size() - 1; i >= 0; i--) {
            SqliteStore.RunView run = runs.get(i);
            if (!isTerminal(run.state().name())) {
                done.add(new JournalStatus(run.id(), "skipped-non-terminal", null));
                continue;
            }
            try {
                String result = appendRun(store, dataDir, run.id());
                done.add(new JournalStatus(run.id(), result, null));
            } catch (JournalException e) {
                done.add(new JournalStatus(run.id(), "failed", e.getMessage()));
            }
        }
        return done;
    }

    public record JournalStatus(String runId, String status, String error) {
    }

    private static boolean isTerminal(String state) {
        return switch (state) {
            case "COMPLETED", "PARTIAL", "FAILED", "CANCELLED", "INTERRUPTED" -> true;
            default -> false;
        };
    }

    private static boolean containsCompletePair(String content, String startMarker, String endMarker) {
        int start = content.indexOf(startMarker);
        if (start < 0) {
            return false;
        }
        return content.indexOf(endMarker, start + startMarker.length()) >= 0;
    }

    /**
     * Drops an incomplete trailing section: keeps everything through the
     * last complete end marker so the new section appends onto clean
     * history. A lone start marker for any run is discarded with its tail.
     */
    private static String repairTail(String content, String ownEndMarker) {
        int lastEnd = content.lastIndexOf(MARKER_END_PREFIX);
        if (lastEnd < 0) {
            int headerEnd = content.indexOf("\n---\n");
            return headerEnd >= 0 ? content.substring(0, headerEnd + 5) : "";
        }
        int lineEnd = content.indexOf('\n', lastEnd);
        String kept = lineEnd >= 0 ? content.substring(0, lineEnd + 1) : content;
        if (!kept.endsWith("\n")) {
            kept += "\n";
        }
        return kept;
    }

    private static String renderSection(SqliteStore store, SqliteStore.RepositoryRow repo,
            SqliteStore.RunView run) throws Exception {
        List<SqliteStore.TargetRow> targets = store.listRunFiles(run.id());
        List<SqliteStore.ScannerRunRow> scanners = store.listScannerRuns(run.id());
        List<SqliteStore.FindingRow> findingRows = store.listFindings(run.id());
        List<String> gaps = extractGaps(run.summaryJson());
        boolean complete = "COMPLETED".equals(run.state().name());

        StringBuilder md = new StringBuilder();
        md.append(MARKER_START_PREFIX).append(run.id()).append(MARKER_SUFFIX).append('\n');
        md.append("## Scan `").append(shortId(run.id())).append("` — ")
                .append(run.state().name()).append(" / ").append(run.verdict().name()).append('\n');
        md.append("- Repository ID: `").append(repo.id()).append("`\n");
        md.append("- Started: ").append(nullToDash(run.startedAt())).append('\n');
        md.append("- Finished: ").append(nullToDash(run.finishedAt())).append('\n');
        if (run.errorMessage() != null && !run.errorMessage().isBlank()) {
            md.append("- Error: ").append(run.errorMessage().strip()).append('\n');
        }
        md.append("- Targets (").append(targets.size()).append("):");
        if (targets.isEmpty()) {
            md.append(" none\n");
        } else {
            md.append('\n');
            for (SqliteStore.TargetRow target : targets) {
                md.append("  - `").append(target.relativePath()).append("` `")
                        .append(shortSha(target.sha256())).append("` ").append(target.sizeBytes())
                        .append(" bytes\n");
            }
        }
        md.append("- Scanners:");
        if (scanners.isEmpty()) {
            md.append(" none ran\n");
        } else {
            md.append('\n');
            for (SqliteStore.ScannerRunRow scanner : scanners) {
                long count = findingRows.stream()
                        .filter(f -> f.scannerRunId().equals(scanner.id())).count();
                md.append("  - ").append(scanner.adapterId()).append(' ')
                        .append(scanner.toolVersion() == null ? "?" : scanner.toolVersion())
                        .append(' ').append(scanner.status().name())
                        .append(" (").append(count).append(" finding(s))");
                if (scanner.message() != null && !scanner.message().isBlank()) {
                    md.append(" — ").append(scanner.message().strip());
                }
                md.append('\n');
            }
        }
        md.append("- Findings (").append(findingRows.size()).append("):\n");
        if (findingRows.isEmpty()) {
            md.append("  - none in the selected scope\n");
        } else {
            int shown = Math.min(findingRows.size(), MAX_FINDINGS_PER_SECTION);
            for (int i = 0; i < shown; i++) {
                Finding finding = findingRows.get(i).finding();
                md.append("  - [").append(finding.severity().name()).append("] ")
                        .append(finding.ruleId()).append(" | category ")
                        .append(finding.category().name())
                        .append(" | scanner severity ").append(finding.originalSeverity())
                        .append(" | `")
                        .append(finding.relativePath() == null ? "?" : finding.relativePath())
                        .append('`');
                if (finding.lineStart() != null) {
                    md.append(':').append(finding.lineStart());
                }
                md.append(" | fingerprint `").append(finding.fingerprint()).append("`\n");
                md.append("    Message: ").append(finding.message()).append('\n');
            }
            if (findingRows.size() > shown) {
                md.append("  - …and ").append(findingRows.size() - shown)
                        .append(" more findings; query the database via MCP get_run/list_findings ")
                        .append("for the complete list.\n");
            }
        }
        md.append("- Coverage: ").append(complete && gaps.isEmpty() ? "complete" : "incomplete");
        if (!gaps.isEmpty()) {
            md.append(" — missing/failed/not-applicable:\n");
            for (String gap : gaps) {
                md.append("  - ").append(gap).append('\n');
            }
        } else {
            md.append('\n');
        }
        md.append("- Scope note: this assessment covers only the selected files and checks "
                + "above; it is not a repository-wide verdict and nothing here claims the "
                + "repository is secure.\n");
        md.append(MARKER_END_PREFIX).append(run.id()).append(MARKER_SUFFIX).append('\n');
        return md.toString();
    }

    private static List<String> extractGaps(String summaryJson) {
        List<String> gaps = new ArrayList<>();
        if (summaryJson == null) {
            return gaps;
        }
        int gapsAt = summaryJson.indexOf("\"gaps\"");
        if (gapsAt < 0) {
            return gaps;
        }
        int open = summaryJson.indexOf('[', gapsAt);
        int close = summaryJson.indexOf(']', open);
        if (open < 0 || close < 0) {
            return gaps;
        }
        String body = summaryJson.substring(open + 1, close);
        boolean inString = false;
        StringBuilder current = new StringBuilder();
        for (int i = 0; i < body.length(); i++) {
            char c = body.charAt(i);
            if (c == '"' && (i == 0 || body.charAt(i - 1) != '\\')) {
                if (inString) {
                    gaps.add(current.toString());
                    current.setLength(0);
                }
                inString = !inString;
            } else if (inString) {
                current.append(c);
            }
        }
        return gaps;
    }

    private static String nullToDash(String value) {
        return value == null || value.isBlank() ? "-" : value;
    }

    private static String shortSha(String sha) {
        if (sha == null) {
            return "?";
        }
        return sha.length() > 12 ? sha.substring(0, 12) : sha;
    }

    private static String shortId(String id) {
        if (id == null) {
            return "?";
        }
        return id.length() > 8 ? id.substring(0, 8) : id;
    }
}

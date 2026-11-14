package pwt.mcp;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.modelcontextprotocol.server.McpServerFeatures;
import io.modelcontextprotocol.spec.McpSchema;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.application.LocalSchemaConfig;
import pwt.application.ScanConfig;
import pwt.application.ScanService;
import pwt.domain.ScanSummary;
import pwt.infrastructure.SqliteStore;

/**
 * MCP tools beyond {@code pwt_info}: read-only queries over the shared
 * SQLite store plus opt-in {@code start_scan}/{@code cancel_scan}.
 *
 * <p>Every handler reads only the configured data directory, validates
 * inputs, bounds responses, and returns structured errors — never stack
 * traces or raw scanner output. Scan operations require the server to run
 * with {@code --allow-scans}; otherwise they fail with PERMISSION_DENIED.
 */
public final class McpScanTools {

    private static final Logger LOG = LoggerFactory.getLogger(McpScanTools.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final int DEFAULT_PAGE = 100;
    private static final int MAX_PAGE = 500;
    private static final int DEFAULT_REPORT_CHARS = 12_000;
    private static final int MAX_REPORT_CHARS = 64_000;
    private static final long MAX_REPORT_BYTES = 8L * 1024 * 1024;

    public record McpContext(Path dataDir, boolean allowScans, boolean allowAnalysisWrites,
            McpScanManager scans, McpActivity activity, String scopedRepositoryId) {
        public McpContext(Path dataDir, boolean allowScans, McpScanManager scans) {
            this(dataDir, allowScans, false, scans, null, null);
        }
    }

    private McpScanTools() {
    }

    public static List<McpServerFeatures.SyncToolSpecification> allTools(McpContext context) {
        return List.of(
                listRepositoriesTool(context),
                listRunsTool(context),
                getRunTool(context),
                listFindingsTool(context),
                getReportTool(context),
                startScanTool(context),
                cancelScanTool(context),
                getAnalysisTool(context),
                saveAnalysisTool(context));
    }

    // Argument helpers -------------------------------------------------------

    private static String reqString(Map<String, Object> args, String name)
            throws McpErrors.McpServiceException {
        Object value = args.get(name);
        if (!(value instanceof String string) || string.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                    "missing required string argument: " + name);
        }
        return string;
    }

    private static String optString(Map<String, Object> args, String name) {
        Object value = args.get(name);
        return value instanceof String string ? string : null;
    }

    private static int optOffset(Map<String, Object> args) throws McpErrors.McpServiceException {
        Object cursor = args.get("cursor");
        if (cursor == null) {
            return 0;
        }
        if (cursor instanceof Number number) {
            int offset = number.intValue();
            if (offset < 0) {
                throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                        "cursor must be a non-negative offset");
            }
            return offset;
        }
        if (cursor instanceof String text) {
            try {
                int offset = Integer.parseInt(text.trim());
                if (offset < 0) {
                    throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                            "cursor must be a non-negative offset");
                }
                return offset;
            } catch (NumberFormatException e) {
                throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                        "cursor must be a non-negative offset");
            }
        }
        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                "cursor must be a non-negative offset");
    }

    private static int optLimit(Map<String, Object> args, int def) throws McpErrors.McpServiceException {
        Object value = args.get("limit");
        if (value == null) {
            return def;
        }
        if (value instanceof Number number) {
            int limit = number.intValue();
            if (limit < 1 || limit > MAX_PAGE) {
                throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                        "limit must be between 1 and " + MAX_PAGE);
            }
            return limit;
        }
        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                "limit must be between 1 and " + MAX_PAGE);
    }

    @SuppressWarnings("unchecked")
    private static List<String> optStringList(Map<String, Object> args, String name)
            throws McpErrors.McpServiceException {
        Object value = args.get(name);
        if (value == null) {
            return List.of();
        }
        if (value instanceof List<?> list) {
            List<String> strings = new ArrayList<>();
            for (Object item : list) {
                if (!(item instanceof String string)) {
                    throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                            name + " must be an array of strings");
                }
                strings.add(string);
            }
            return strings;
        }
        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                name + " must be an array of strings");
    }

    private static String nextCursor(int offset, int returned, int total) {
        int next = offset + returned;
        return next < total ? String.valueOf(next) : null;
    }

    private static McpSchema.JsonSchema schema(Map<String, Object> properties, List<String> required) {
        return new McpSchema.JsonSchema("object", properties, required, false);
    }

    private static Map<String, Object> prop(String type) {
        return Map.of("type", type);
    }

    private static Map<String, Object> arrayProp() {
        Map<String, Object> items = Map.of("type", "string");
        Map<String, Object> property = new LinkedHashMap<>();
        property.put("type", "array");
        property.put("items", items);
        return property;
    }

    private static McpServerFeatures.SyncToolSpecification tool(McpContext context, String name,
            String description, McpSchema.JsonSchema input, ThrowingHandler handler) {
        McpSchema.Tool tool = new McpSchema.Tool(name, description, input);
        return new McpServerFeatures.SyncToolSpecification(tool, (exchange, args) -> {
            if (context.activity() != null) {
                context.activity().observed(exchange);
            }
            try {
                return McpErrors.ok(handler.handle(args == null ? Map.of() : args));
            } catch (McpErrors.McpServiceException e) {
                LOG.warn("MCP tool {} failed [{}]: {} {}",
                        name, e.correlationId(), e.code(), McpErrors.sanitize(e.getMessage()));
                return McpErrors.error(e);
            } catch (Exception e) {
                String correlation = java.util.UUID.randomUUID().toString();
                LOG.warn("MCP tool {} failed [{}]: {}", name, correlation, e.toString());
                return McpErrors.error(McpErrors.INTERNAL_ERROR, "unexpected server error", correlation);
            }
        });
    }

    private interface ThrowingHandler {
        Map<String, Object> handle(Map<String, Object> args) throws Exception;
    }

    private static SqliteStore openStore(Path dataDir) throws McpErrors.McpServiceException {
        try {
            return SqliteStore.open(dataDir);
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                    "cannot open application data directory: " + e.getMessage());
        }
    }

    /**
     * Resolves the effective repository: an explicit ID must equal the
     * server's project scope when one is configured; otherwise the scope
     * itself is used. Unknown IDs are NOT_FOUND.
     */
    private static SqliteStore.RepositoryRow scopedRepository(SqliteStore store, McpContext context,
            String repositoryId, String operation) throws McpErrors.McpServiceException {
        String effective = repositoryId;
        if (effective == null || effective.isBlank()) {
            effective = context.scopedRepositoryId();
        }
        if (context.scopedRepositoryId() != null && !context.scopedRepositoryId().equals(effective)) {
            throw new McpErrors.McpServiceException(McpErrors.OUT_OF_SCOPE,
                    operation + " is restricted to the configured project repository");
        }
        if (effective == null || effective.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                    "repository_id is required");
        }
        try {
            SqliteStore.RepositoryRow repo = store.findRepositoryById(effective);
            if (repo == null) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND,
                        "unknown repository_id: " + effective);
            }
            return repo;
        } catch (McpErrors.McpServiceException e) {
            throw e;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
        }
    }

    /** Loads a run and enforces project scope (cross-project IDs are NOT_FOUND). */
    private static SqliteStore.RunView scopedRun(SqliteStore store, McpContext context, String runId)
            throws McpErrors.McpServiceException {
        if (runId == null || runId.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, "run_id is required");
        }
        try {
            SqliteStore.RunView view = store.getRun(runId);
            if (view == null) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND, "unknown run_id: " + runId);
            }
            if (context.scopedRepositoryId() != null
                    && !context.scopedRepositoryId().equals(view.repositoryId())) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND, "unknown run_id: " + runId);
            }
            return view;
        } catch (McpErrors.McpServiceException e) {
            throw e;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
        }
    }

    // list_repositories --------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification listRepositoriesTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("cursor", prop("string"));
        properties.put("limit", prop("integer"));
        return tool(context, "list_repositories",
                "Lists registered repositories (id, display name, availability). "
                        + "Roots originate from GUI/CLI scans; MCP cannot register new roots.",
                schema(properties, List.of()), args -> {
                    int offset = optOffset(args);
                    int limit = optLimit(args, DEFAULT_PAGE);
                    try (SqliteStore store = openStore(context.dataDir())) {
                        List<SqliteStore.RepositoryRow> rows;
                        if (context.scopedRepositoryId() != null) {
                            SqliteStore.RepositoryRow only =
                                    store.findRepositoryById(context.scopedRepositoryId());
                            rows = only == null ? List.of() : List.of(only);
                        } else {
                            rows = store.listRepositories();
                        }
                        List<Map<String, Object>> items = new ArrayList<>();
                        for (SqliteStore.RepositoryRow row : rows) {
                            boolean available = Files.isDirectory(Path.of(row.canonicalRoot()));
                            Map<String, Object> item = new LinkedHashMap<>();
                            item.put("id", row.id());
                            item.put("display_name", row.displayName());
                            item.put("canonical_root", row.canonicalRoot());
                            item.put("availability", available ? "available" : "unavailable");
                            if (!available) {
                                item.put("availability_note", "registered root is missing or not a directory");
                            }
                            items.add(item);
                        }
                        int total = items.size();
                        List<Map<String, Object>> page =
                                items.subList(Math.min(offset, total), Math.min(offset + limit, total));
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("repositories", page);
                        result.put("next_cursor", nextCursor(offset, page.size(), total));
                        result.put("total", total);
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    // list_runs ----------------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification listRunsTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("repository_id", prop("string"));
        properties.put("cursor", prop("string"));
        properties.put("limit", prop("integer"));
        properties.put("state", prop("string"));
        return tool(context, "list_runs",
                "Lists scan runs for one registered repository, newest first. "
                        + "Paginated (default 100/page, max 500) with optional state filter.",
                schema(properties, List.of()), args -> {
                    String repositoryId = optString(args, "repository_id");
                    String state = optString(args, "state");
                    if (state != null && !state.isBlank()) {
                        try {
                            pwt.domain.ScanState.valueOf(state);
                        } catch (IllegalArgumentException e) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "unknown state filter: " + state);
                        }
                        state = state.trim();
                    } else {
                        state = null;
                    }
                    int offset = optOffset(args);
                    int limit = optLimit(args, DEFAULT_PAGE);
                    try (SqliteStore store = openStore(context.dataDir())) {
                        SqliteStore.RepositoryRow repo =
                                scopedRepository(store, context, repositoryId, "list_runs");
                        String effectiveRepoId = repo.id();
                        int total = store.countRuns(effectiveRepoId, state);
                        List<Map<String, Object>> items = new ArrayList<>();
                        for (SqliteStore.RunView run : store.listRuns(effectiveRepoId, state, limit, offset)) {
                            Map<String, Object> item = new LinkedHashMap<>();
                            item.put("run_id", run.id());
                            item.put("created_at", run.createdAt());
                            item.put("finished_at", run.finishedAt());
                            item.put("state", run.state().name());
                            item.put("verdict", run.verdict().name());
                            item.put("finding_count", store.countFindings(run.id()));
                            item.put("target_count", store.listRunFiles(run.id()).size());
                            List<String> gaps = ScanService.extractSummaryGaps(run.summaryJson());
                            item.put("coverage_gap_count", gaps.size());
                            item.put("coverage_gaps", gaps.size() > 10 ? gaps.subList(0, 10) : gaps);
                            items.add(item);
                        }
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("repository_id", repositoryId);
                        result.put("runs", items);
                        result.put("next_cursor", nextCursor(offset, items.size(), total));
                        result.put("total", total);
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    // get_run -----------------------------------------------------------------

    static Map<String, Object> runPayload(SqliteStore store, ScanSummary summary)
            throws McpErrors.McpServiceException {
        try {
            SqliteStore.RepositoryRow repo = store.findRepositoryById(
                    store.getRun(summary.runId()).repositoryId());
            Map<String, Object> run = new LinkedHashMap<>();
            run.put("id", summary.runId());
            run.put("repository_id", repo == null ? null : repo.id());
            run.put("state", summary.state().name());
            run.put("verdict", summary.verdict().name());
            run.put("created_at", summary.startedAt() == null ? null : summary.startedAt());
            run.put("started_at", summary.startedAt());
            run.put("finished_at", summary.finishedAt());
            run.put("source", sourceOf(store, summary.runId()));
            run.put("finding_count", summary.findingCount());
            Map<String, Object> repository = new LinkedHashMap<>();
            repository.put("id", repo == null ? null : repo.id());
            repository.put("display_name", summary.repositoryDisplayName());
            boolean available = repo != null && Files.isDirectory(Path.of(repo.canonicalRoot()));
            repository.put("availability", available ? "available" : "unavailable");
            List<Map<String, Object>> targets = new ArrayList<>();
            for (ScanSummary.SelectedTarget target : summary.targets()) {
                Map<String, Object> entry = new LinkedHashMap<>();
                entry.put("relative_path", target.relativePath());
                entry.put("content_sha256", target.sha256());
                entry.put("size_bytes", target.sizeBytes());
                targets.add(entry);
            }
            List<Map<String, Object>> scanners = new ArrayList<>();
            for (pwt.domain.ScannerOutcome outcome : summary.scannerOutcomes()) {
                Map<String, Object> entry = new LinkedHashMap<>();
                entry.put("adapter_id", outcome.adapterId());
                entry.put("tool_name", outcome.toolName());
                entry.put("tool_version", outcome.toolVersion());
                entry.put("status", outcome.status().name());
                entry.put("exit_code", outcome.exitCode());
                entry.put("message", outcome.detail());
                entry.put("finding_count", outcome.findings().size());
                scanners.add(entry);
            }
            Map<String, Object> coverage = new LinkedHashMap<>();
            coverage.put("complete", summary.state() == pwt.domain.ScanState.COMPLETED
                    && summary.coverageGaps().isEmpty());
            coverage.put("gaps", summary.coverageGaps());
            Map<String, Object> reports = new LinkedHashMap<>();
            reports.put("json_report", summary.jsonReportPath());
            reports.put("markdown_report", summary.markdownReportPath());
            reports.put("rendered_on_demand", true);
            reports.put("note", "Reports render from persisted records via get_report;"
                    + " files exist only after an explicit export.");
            Map<String, Object> result = new LinkedHashMap<>();
            result.put("run", run);
            result.put("repository", repository);
            result.put("targets", targets);
            result.put("scanner_runs", scanners);
            result.put("coverage", coverage);
            result.put("reports", reports);
            return result;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
        }
    }

    private static String sourceOf(SqliteStore store, String runId) {
        try {
            SqliteStore.RunView view = store.getRun(runId);
            if (view != null && view.configJson() != null) {
                if (view.configJson().contains("\"source\":\"GUI\"")) {
                    return "GUI";
                }
                if (view.configJson().contains("\"source\":\"MCP\"")) {
                    return "MCP";
                }
            }
        } catch (Exception ignored) {
            // best effort; callers default to CLI
        }
        return "CLI";
    }

    private static McpServerFeatures.SyncToolSpecification getRunTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("run_id", prop("string"));
        return tool(context, "get_run",
                "Returns one persisted run: repository, targets, state/verdict, per-scanner "
                        + "outcomes with tool versions, coverage gaps, and report availability.",
                schema(properties, List.of("run_id")), args -> {
                    String runId = reqString(args, "run_id");
                    try (SqliteStore store = openStore(context.dataDir())) {
                        scopedRun(store, context, runId);
                        ScanSummary summary = ScanService.loadSummary(store, runId);
                        if (summary == null) {
                            throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND,
                                    "unknown run_id: " + runId);
                        }
                        return runPayload(store, summary);
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    // list_findings -------------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification listFindingsTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("run_id", prop("string"));
        properties.put("severity", prop("string"));
        properties.put("adapter_id", prop("string"));
        properties.put("cursor", prop("string"));
        properties.put("limit", prop("integer"));
        return tool(context, "list_findings",
                "Lists sanitized normalized findings for one run (stable order). "
                        + "Optional severity/adapter filters; paginated (default 100/page, max 500).",
                schema(properties, List.of("run_id")), args -> {
                    String runId = reqString(args, "run_id");
                    String severity = optString(args, "severity");
                    if (severity != null && !severity.isBlank()) {
                        try {
                            pwt.domain.Severity.valueOf(severity);
                        } catch (IllegalArgumentException e) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "unknown severity filter: " + severity);
                        }
                        severity = severity.trim();
                    } else {
                        severity = null;
                    }
                    String adapterId = optString(args, "adapter_id");
                    if (adapterId != null) {
                        adapterId = adapterId.trim();
                        if (adapterId.isEmpty()) {
                            adapterId = null;
                        } else if (!ScanService.INTEGRATED_ADAPTERS.contains(adapterId)) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "unknown adapter_id filter: " + adapterId);
                        }
                    }
                    int offset = optOffset(args);
                    int limit = optLimit(args, DEFAULT_PAGE);
                    try (SqliteStore store = openStore(context.dataDir())) {
                        scopedRun(store, context, runId);
                        Map<String, String[]> provenance = new LinkedHashMap<>();
                        for (SqliteStore.ScannerRunRow scanner : store.listScannerRuns(runId)) {
                            provenance.put(scanner.id(), new String[]{
                                    scanner.adapterId(), scanner.toolName(), scanner.toolVersion()});
                        }
                        List<Map<String, Object>> matched = new ArrayList<>();
                        for (SqliteStore.FindingRow row : store.listFindings(runId)) {
                            String[] info = provenance.getOrDefault(row.scannerRunId(),
                                    new String[]{"unknown", "unknown", null});
                            if (severity != null && !row.finding().severity().name().equals(severity)) {
                                continue;
                            }
                            if (adapterId != null && !info[0].equals(adapterId)) {
                                continue;
                            }
                            pwt.domain.Finding finding = row.finding();
                            Map<String, Object> item = new LinkedHashMap<>();
                            item.put("rule_id", finding.ruleId());
                            item.put("title", finding.title());
                            item.put("severity", finding.severity().name());
                            item.put("original_severity", finding.originalSeverity());
                            item.put("category", finding.category().name());
                            item.put("relative_path", finding.relativePath());
                            item.put("line_start", finding.lineStart());
                            item.put("line_end", finding.lineEnd());
                            item.put("column_start", finding.columnStart());
                            item.put("message", finding.message());
                            item.put("evidence", parseEvidence(finding.evidenceJson()));
                            item.put("fingerprint", finding.fingerprint());
                            Map<String, Object> provenanceMap = new LinkedHashMap<>();
                            provenanceMap.put("adapter_id", info[0]);
                            provenanceMap.put("tool_name", info[1]);
                            provenanceMap.put("tool_version", info[2]);
                            item.put("provenance", provenanceMap);
                            matched.add(item);
                        }
                        int total = matched.size();
                        List<Map<String, Object>> page = matched.subList(
                                Math.min(offset, total), Math.min(offset + limit, total));
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("run_id", runId);
                        result.put("findings", page);
                        result.put("next_cursor", nextCursor(offset, page.size(), total));
                        result.put("total", total);
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    private static Object parseEvidence(String evidenceJson) {
        if (evidenceJson == null || evidenceJson.isBlank()) {
            return Map.of();
        }
        try {
            return MAPPER.readValue(evidenceJson, Object.class);
        } catch (Exception e) {
            return evidenceJson;
        }
    }

    // get_report -----------------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification getReportTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("run_id", prop("string"));
        properties.put("format", prop("string"));
        properties.put("offset", prop("integer"));
        properties.put("length", prop("integer"));
        return tool(context, "get_report",
                "Returns sanitized report content for one run (JSON or Markdown), in bounded "
                        + "chunks with continuation metadata. Never truncated silently: a null "
                        + "next_offset means the content is complete.",
                schema(properties, List.of("run_id", "format")), args -> {
                    String runId = reqString(args, "run_id");
                    String format = reqString(args, "format").trim().toLowerCase();
                    if (!format.equals("json") && !format.equals("markdown")) {
                        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                "format must be json or markdown");
                    }
                    int offset = 0;
                    Object offsetArg = args.get("offset");
                    if (offsetArg != null) {
                        if (!(offsetArg instanceof Number number) || number.intValue() < 0) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "offset must be a non-negative integer");
                        }
                        offset = number.intValue();
                    }
                    int length = DEFAULT_REPORT_CHARS;
                    Object lengthArg = args.get("length");
                    if (lengthArg != null) {
                        if (!(lengthArg instanceof Number number)
                                || number.intValue() < 1 || number.intValue() > MAX_REPORT_CHARS) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "length must be between 1 and " + MAX_REPORT_CHARS);
                        }
                        length = number.intValue();
                    }
                    try (SqliteStore store = openStore(context.dataDir())) {
                        scopedRun(store, context, runId);
                        // Reports render from persisted records; files are
                        // optional exports and never required for retrieval.
                        String content;
                        try {
                            pwt.application.ReportService.RenderedReport rendered =
                                    new pwt.application.ReportService().render(store, runId);
                            content = format.equals("json") ? rendered.json() : rendered.markdown();
                        } catch (McpErrors.McpServiceException e) {
                            throw e;
                        } catch (Exception e) {
                            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                                    "report cannot be rendered: " + e.getMessage());
                        }
                        if (content.length() > MAX_REPORT_BYTES) {
                            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                                    "rendered report exceeds the readable bound");
                        }
                        int total = content.length();
                        int from = Math.min(offset, total);
                        int to = Math.min(from + length, total);
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("run_id", runId);
                        result.put("format", format);
                        result.put("schema_version", format.equals("json") ? "1.0" : null);
                        result.put("total_chars", total);
                        result.put("offset", from);
                        result.put("content", content.substring(from, to));
                        result.put("next_offset", to < total ? to : null);
                        result.put("truncated", to < total);
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    // start_scan -----------------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification startScanTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("repository_id", prop("string"));
        properties.put("relative_paths", arrayProp());
        properties.put("adapter_ids", arrayProp());
        properties.put("schema_bindings", arrayProp());
        properties.put("schema_dialect", prop("string"));
        properties.put("kubeconform_schema", prop("string"));
        properties.put("kubeconform_version", prop("string"));
        properties.put("kubeconform_strict", Map.of("type", "boolean"));
        return tool(context, "start_scan",
                "Starts an assessment over explicitly selected files of a REGISTERED repository "
                        + "and returns the durable run ID promptly (poll get_run for progress). "
                        + "Requires the server to run with --allow-scans. Never scans unregistered "
                        + "roots; never accepts shell commands, executables, or output paths.",
                schema(properties, List.of("repository_id", "relative_paths")), args -> {
                    if (!context.allowScans()) {
                        throw new McpErrors.McpServiceException(McpErrors.PERMISSION_DENIED,
                                "scan operations are disabled: restart the MCP server with --allow-scans"
                                        + " to enable start_scan/cancel_scan");
                    }
                    String repositoryId = reqString(args, "repository_id");
                    if (context.scopedRepositoryId() != null
                            && !context.scopedRepositoryId().equals(repositoryId)) {
                        throw new McpErrors.McpServiceException(McpErrors.OUT_OF_SCOPE,
                                "this MCP server is restricted to one registered project repository");
                    }
                    List<String> relativePaths = optStringList(args, "relative_paths");
                    if (relativePaths.isEmpty()) {
                        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                "relative_paths must name at least one file");
                    }
                    Set<String> adapterIds = new java.util.LinkedHashSet<>(optStringList(args, "adapter_ids"));
                    List<String> schemaBindings = optStringList(args, "schema_bindings");
                    String schemaDialect = optString(args, "schema_dialect");
                    if (schemaDialect == null || schemaDialect.isBlank()) {
                        schemaDialect = LocalSchemaConfig.DIALECT_2020_12;
                    }
                    LocalSchemaConfig localSchema;
                    try {
                        localSchema = LocalSchemaConfig.parse(schemaBindings, schemaDialect,
                                Path.of("").toAbsolutePath());
                    } catch (IllegalArgumentException e) {
                        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, e.getMessage());
                    }
                    for (LocalSchemaConfig.Binding binding : localSchema.bindings()) {
                        if (!binding.schemaPath().isAbsolute()) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "schema paths must be absolute: " + binding.glob());
                        }
                        if (!Files.isRegularFile(binding.schemaPath())
                                || Files.isSymbolicLink(binding.schemaPath())) {
                            throw new McpErrors.McpServiceException(McpErrors.CONFIGURATION_REQUIRED,
                                    "schema file missing or not a regular file: " + binding.schemaPath());
                        }
                    }
                    String kubeTemplate = optString(args, "kubeconform_schema");
                    if (kubeTemplate != null && kubeTemplate.isBlank()) {
                        kubeTemplate = null;
                    }
                    String kubeVersion = optString(args, "kubeconform_version");
                    if (kubeVersion == null || kubeVersion.isBlank()) {
                        kubeVersion = ScanConfig.KubeconformConfig.defaults().kubernetesVersion();
                    }
                    Object strictArg = args.get("kubeconform_strict");
                    boolean strict = true;
                    if (strictArg != null) {
                        if (!(strictArg instanceof Boolean flag)) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "kubeconform_strict must be a boolean");
                        }
                        strict = flag;
                    }
                    ScanConfig.KubeconformConfig kubeconform =
                            new ScanConfig.KubeconformConfig(kubeTemplate, kubeVersion, strict);
                    ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                            ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                            kubeconform, localSchema.isEmpty() ? null : localSchema);
                    McpScanManager.StartResult started = context.scans().startScan(
                            repositoryId, relativePaths, adapterIds, config, "MCP");
                    Map<String, Object> result = new LinkedHashMap<>();
                    result.put("run_id", started.runId());
                    result.put("state", started.state());
                    result.put("repository_id", repositoryId);
                    result.put("note", "RUNNING means the assessment is executing in the background; "
                            + "poll get_run for progress and the terminal summary.");
                    return result;
                });
    }

    // cancel_scan ------------------------------------------------------------------

    private static McpServerFeatures.SyncToolSpecification cancelScanTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("run_id", prop("string"));
        return tool(context, "cancel_scan",
                "Requests cancellation of a running scan (durable flag; owning workers "
                        + "observe it). Requires --allow-scans. Reports actual state — a run "
                        + "is CANCELLED only when its workers actually stopped.",
                schema(properties, List.of("run_id")), args -> {
                    if (!context.allowScans()) {
                        throw new McpErrors.McpServiceException(McpErrors.PERMISSION_DENIED,
                                "scan operations are disabled: restart the MCP server with --allow-scans"
                                        + " to enable start_scan/cancel_scan");
                    }
                    String runId = reqString(args, "run_id");
                    if (context.scopedRepositoryId() != null) {
                        try (SqliteStore scopeStore = openStore(context.dataDir())) {
                            scopedRun(scopeStore, context, runId);
                        } catch (McpErrors.McpServiceException e) {
                            throw e;
                        } catch (Exception e) {
                            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                                    e.getMessage());
                        }
                    }
                    McpScanManager.CancelResult cancelled = context.scans().cancelScan(runId);
                    Map<String, Object> result = new LinkedHashMap<>();
                    result.put("run_id", cancelled.runId());
                    result.put("state", cancelled.state());
                    result.put("cancel_requested", cancelled.cancelRequested());
                    result.put("note", cancelled.note());
                    return result;
                });
    }
    // get_analysis / save_analysis ---------------------------------------------

    private static void checkPriority(String value, String field) throws McpErrors.McpServiceException {
        if (!"LOW".equals(value) && !"MEDIUM".equals(value) && !"HIGH".equals(value)) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                    field + " must be LOW, MEDIUM, or HIGH");
        }
    }

    private static McpServerFeatures.SyncToolSpecification getAnalysisTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("repository_id", prop("string"));
        properties.put("run_id", prop("string"));
        properties.put("revision", Map.of("type", "integer"));
        return tool(context, "get_analysis",
                "Returns the agent analysis for one run: current or requested revision, "
                        + "structured per-finding assessments, priorities, change and "
                        + "verification status, plus the list of available revisions.",
                schema(properties, List.of("run_id")), args -> {
                    String runId = reqString(args, "run_id");
                    String repositoryId = optString(args, "repository_id");
                    Object revisionArg = args.get("revision");
                    int revision = -1;
                    if (revisionArg != null) {
                        if (!(revisionArg instanceof Number number) || number.intValue() < 0) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "revision must be a non-negative integer");
                        }
                        revision = number.intValue();
                    }
                    try (SqliteStore store = openStore(context.dataDir())) {
                        SqliteStore.RunView view = scopedRun(store, context, runId);
                        if (repositoryId != null && !repositoryId.isBlank()
                                && !repositoryId.equals(view.repositoryId())) {
                            throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND,
                                    "unknown run_id: " + runId);
                        }
                        SqliteStore.AnalysisThreadView thread =
                                store.getOrCreateAnalysisThread(view.repositoryId(), runId);
                        SqliteStore.AnalysisRevisionRow resolved =
                                store.getAnalysisRevision(thread.threadId(), revision);
                        List<Integer> available = store.listAnalysisRevisions(thread.threadId());
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("repository_id", view.repositoryId());
                        result.put("run_id", runId);
                        result.put("current_revision", thread.currentRevision());
                        result.put("available_revisions", available);
                        if (resolved == null) {
                            result.put("revision", null);
                            result.put("assessments", List.of());
                            result.put("note", "no analysis saved yet for this run");
                            return result;
                        }
                        result.put("revision", resolved.revision());
                        Map<String, Object> analysis = new LinkedHashMap<>();
                        analysis.put("summary", resolved.summary());
                        analysis.put("priority", resolved.priority());
                        analysis.put("priority_rationale", resolved.priorityRationale());
                        analysis.put("category", resolved.category());
                        analysis.put("change_status", resolved.changeStatus());
                        analysis.put("verification_run_id", resolved.verificationRunId());
                        analysis.put("author_client", resolved.authorClient());
                        analysis.put("author_name", resolved.authorName());
                        analysis.put("created_at", resolved.createdAt());
                        result.put("analysis", analysis);
                        List<Map<String, Object>> assessments = new ArrayList<>();
                        for (SqliteStore.AnalysisAssessmentRow row :
                                store.listAnalysisAssessments(resolved.id())) {
                            Map<String, Object> item = new LinkedHashMap<>();
                            item.put("finding_id", row.findingId());
                            item.put("rule_id", row.ruleId());
                            item.put("relative_path", row.relativePath());
                            item.put("line_start", row.lineStart());
                            item.put("priority", row.priority());
                            item.put("rationale", row.rationale());
                            item.put("category", row.category());
                            item.put("change_status", row.changeStatus());
                            item.put("verification_run_id", row.verificationRunId());
                            item.put("message", row.message());
                            assessments.add(item);
                        }
                        result.put("assessments", assessments);
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    @SuppressWarnings("unchecked")
    private static McpServerFeatures.SyncToolSpecification saveAnalysisTool(McpContext context) {
        Map<String, Object> properties = new LinkedHashMap<>();
        properties.put("repository_id", prop("string"));
        properties.put("run_id", prop("string"));
        properties.put("expected_revision", Map.of("type", "integer"));
        properties.put("summary", prop("string"));
        properties.put("priority", prop("string"));
        properties.put("priority_rationale", prop("string"));
        properties.put("category", prop("string"));
        properties.put("change_status", prop("string"));
        properties.put("verification_run_id", prop("string"));
        properties.put("author_name", prop("string"));
        Map<String, Object> assessmentItems = new LinkedHashMap<>();
        assessmentItems.put("type", "object");
        properties.put("assessments", Map.of("type", "array", "items", assessmentItems));
        return tool(context, "save_analysis",
                "Saves a new agent-analysis revision for one run (optimistic concurrency: "
                        + "expected_revision must match current, else CONFLICT is returned and "
                        + "nothing is written). Requires the server to run with "
                        + "--allow-analysis-writes. Never edits scanner findings.",
                schema(properties, List.of("run_id", "expected_revision", "priority")), args -> {
                    if (!context.allowAnalysisWrites()) {
                        throw new McpErrors.McpServiceException(McpErrors.PERMISSION_DENIED,
                                "analysis writes are disabled: restart the MCP server with "
                                        + "--allow-analysis-writes to enable save_analysis");
                    }
                    String runId = reqString(args, "run_id");
                    String repositoryId = optString(args, "repository_id");
                    Object expectedArg = args.get("expected_revision");
                    if (!(expectedArg instanceof Number expectedNumber) || expectedNumber.intValue() < 0) {
                        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                "expected_revision must be a non-negative integer");
                    }
                    String priority = reqString(args, "priority");
                    checkPriority(priority, "priority");
                    try (SqliteStore store = openStore(context.dataDir())) {
                        SqliteStore.RunView view = scopedRun(store, context, runId);
                        if (repositoryId != null && !repositoryId.isBlank()
                                && !repositoryId.equals(view.repositoryId())) {
                            throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND,
                                    "unknown run_id: " + runId);
                        }
                        SqliteStore.AnalysisThreadView thread =
                                store.getOrCreateAnalysisThread(view.repositoryId(), runId);
                        String verificationRunId = optString(args, "verification_run_id");
                        List<SqliteStore.AnalysisAssessmentInput> assessments = new ArrayList<>();
                        Object rawAssessments = args.get("assessments");
                        if (rawAssessments instanceof List<?> list) {
                            for (Object raw : list) {
                                if (!(raw instanceof Map)) {
                                    throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                            "assessments must be objects");
                                }
                                Map<String, Object> item = (Map<String, Object>) raw;
                                Object itemPriority = item.get("priority");
                                if (!(itemPriority instanceof String)) {
                                    throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                            "assessment priority is required");
                                }
                                checkPriority((String) itemPriority, "assessment priority");
                                Object line = item.get("line_start");
                                Integer lineStart = null;
                                if (line != null) {
                                    if (!(line instanceof Number number) || number.intValue() < 1) {
                                        throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                                "assessment line_start must be a positive integer");
                                    }
                                    lineStart = number.intValue();
                                }
                                assessments.add(new SqliteStore.AnalysisAssessmentInput(
                                        asString(item.get("finding_id")),
                                        asString(item.get("rule_id")),
                                        asString(item.get("relative_path")),
                                        lineStart,
                                        (String) itemPriority,
                                        asString(item.get("rationale")),
                                        asString(item.get("category")),
                                        asString(item.get("change_status")),
                                        asString(item.get("verification_run_id")),
                                        asString(item.get("message"))));
                            }
                        } else if (rawAssessments != null) {
                            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                                    "assessments must be an array");
                        }
                        SqliteStore.AnalysisSaveResult saved;
                        try {
                            saved = store.saveAnalysis(thread.threadId(), expectedNumber.intValue(),
                                    optString(args, "summary"), priority,
                                    optString(args, "priority_rationale"), optString(args, "category"),
                                    optString(args, "change_status"), blankToNull(verificationRunId),
                                    "mcp", optString(args, "author_name"), assessments);
                        } catch (java.sql.SQLException e) {
                            throw new McpErrors.McpServiceException(
                                    e.getMessage() != null && e.getMessage().contains("CHECK constraint")
                                            ? McpErrors.INVALID_INPUT
                                            : McpErrors.INTERNAL_ERROR,
                                    e.getMessage() == null ? "analysis save failed" : e.getMessage());
                        }
                        if (saved.conflict()) {
                            throw new McpErrors.McpServiceException(McpErrors.CONFLICT,
                                    "stale revision: expected " + expectedNumber.intValue()
                                            + " but current is " + saved.currentRevision()
                                            + "; reload and merge before retrying");
                        }
                        Map<String, Object> result = new LinkedHashMap<>();
                        result.put("repository_id", view.repositoryId());
                        result.put("run_id", runId);
                        result.put("saved_revision", saved.savedRevision());
                        result.put("current_revision", saved.currentRevision());
                        return result;
                    } catch (McpErrors.McpServiceException e) {
                        throw e;
                    } catch (Exception e) {
                        throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
                    }
                });
    }

    private static String asString(Object value) {
        return value instanceof String text ? text : null;
    }

    private static String blankToNull(String value) {
        return value == null || value.isBlank() ? null : value;
    }
}

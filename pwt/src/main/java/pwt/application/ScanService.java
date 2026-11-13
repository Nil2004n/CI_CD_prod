package pwt.application;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.adapters.ActionlintAdapter;
import pwt.adapters.CheckovAdapter;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.KubeconformAdapter;
import pwt.adapters.LocalSchemaAdapter;
import pwt.adapters.ScannerAdapter;
import pwt.adapters.YamllintAdapter;
import pwt.domain.Finding;
import pwt.domain.RepositoryRoot;
import pwt.domain.ScanState;
import pwt.domain.ScanSummary;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerOutcome;
import pwt.domain.ScannerStatus;
import pwt.infrastructure.NioDiscoveryService;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;
import pwt.infrastructure.Staging;

/**
 * Application service for Phase 2 scans: validates an explicit target
 * selection, stages a private per-run snapshot, executes the integrated
 * adapters, persists normalized sanitized results in SQLite, and produces
 * JSON/Markdown reports.
 *
 * <p>Headless and GUI entrypoints share this service. No JavaFX or MCP types
 * appear here; progress reporting is the caller's job via {@link ScanListener}.
 */
public final class ScanService {

    private static final Logger LOG = LoggerFactory.getLogger(ScanService.class);

    public static final Set<String> INTEGRATED_ADAPTERS = Set.of(
            YamllintAdapter.ID, GitleaksAdapter.ID, CheckovAdapter.ID, ActionlintAdapter.ID,
            KubeconformAdapter.ID, LocalSchemaAdapter.ID);

    public static final List<String> ADAPTER_ORDER = List.of(
            YamllintAdapter.ID, GitleaksAdapter.ID, CheckovAdapter.ID, ActionlintAdapter.ID,
            KubeconformAdapter.ID, LocalSchemaAdapter.ID);

    /** Stage names reported to listeners; completed-scanner counts, never percentages. */
    public static final String STAGE_VALIDATE = "validating selection";
    public static final String STAGE_STAGE = "staging selected files";
    public static final String STAGE_REPORTS = "generating reports";

    public interface ScanListener {
        void stage(String stage);

        void adapterStarted(String adapterId);

        void adapterFinished(String adapterId, ScannerStatus status, int findingCount);
    }

    private final NioDiscoveryService discovery = new NioDiscoveryService();
    private final ScanConfig config;

    public ScanService(ScanConfig config) {
        this.config = config == null ? ScanConfig.defaults() : config;
    }

    /**
     * Application data directory shared by GUI, CLI, and MCP. Defaults to
     * {@code ~/.pwt-data}; the GUI honors {@code --data-dir} by setting the
     * {@code pwt.data.dir} system property at startup.
     */
    public static Path defaultDataDir() {
        String override = System.getProperty("pwt.data.dir");
        if (override != null && !override.isBlank()) {
            return Path.of(override);
        }
        return Path.of(System.getProperty("user.home"), ".pwt-data");
    }

    private static ScannerAdapter adapterFor(String adapterId) {
        return switch (adapterId) {
            case YamllintAdapter.ID -> new YamllintAdapter();
            case GitleaksAdapter.ID -> new GitleaksAdapter();
            case CheckovAdapter.ID -> new CheckovAdapter();
            case ActionlintAdapter.ID -> new ActionlintAdapter();
            case KubeconformAdapter.ID -> new KubeconformAdapter();
            case LocalSchemaAdapter.ID -> new LocalSchemaAdapter();
            default -> throw new IllegalArgumentException("Not an integrated adapter: " + adapterId);
        };
    }

    public record ValidatedTarget(String relativePath, String sha256, long sizeBytes) {
    }

    /** Validated selection: canonical root plus hash-verified targets and skip notes. */
    public record ValidatedSelection(
            RepositoryRoot root, List<ValidatedTarget> targets, List<String> gaps) {
    }

    /**
     * Validates adapter IDs against the integrated set. Empty/null selects all
     * integrated adapters. Order is always {@link #ADAPTER_ORDER}.
     */
    public static List<String> resolveAdapters(Set<String> adapterIds) throws ScanException {
        Set<String> adapters = adapterIds == null || adapterIds.isEmpty()
                ? new LinkedHashSet<>(INTEGRATED_ADAPTERS)
                : new LinkedHashSet<>(adapterIds);
        for (String id : adapters) {
            if (!INTEGRATED_ADAPTERS.contains(id)) {
                throw new ScanException(ScanException.Code.INVALID_INPUT,
                        "Unknown or not-integrated adapter: " + id
                                + " (integrated: yamllint, gitleaks, checkov, actionlint,"
                                + " kubeconform, local-schema)");
            }
        }
        return ADAPTER_ORDER.stream().filter(adapters::contains).toList();
    }

    /**
     * Validates the supplied root and relative paths: canonical root
     * resolution, containment, existence, regular files, no symlinks, content
     * hashes. Throws only when nothing selectable remains.
     */
    public ValidatedSelection validateSelection(Path rootSupplied, List<String> relativePaths)
            throws ScanException {
        if (relativePaths == null || relativePaths.isEmpty()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "No files selected: pass at least one repository-relative YAML path");
        }
        RepositoryRoot root;
        try {
            root = discovery.registerRoot(rootSupplied);
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "Invalid repository root: " + e.getMessage(), e);
        }
        List<ValidatedTarget> targets = new ArrayList<>();
        List<String> gaps = new ArrayList<>();
        for (String input : new LinkedHashSet<>(relativePaths)) {
            String rel;
            try {
                rel = pwt.application.DiscoveryService.requireInsideRoot(root, input);
            } catch (Exception e) {
                gaps.add("Rejected selection '" + input + "': outside the registered root");
                continue;
            }
            Path candidate = root.root().resolve(rel);
            try {
                if (!Files.exists(candidate) || Files.isSymbolicLink(candidate)) {
                    gaps.add("Skipped '" + rel + "': missing or symlinked");
                    continue;
                }
                if (!Files.isRegularFile(candidate)) {
                    gaps.add("Skipped '" + rel + "': not a regular file");
                    continue;
                }
                byte[] bytes = Files.readAllBytes(candidate);
                String sha = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
                targets.add(new ValidatedTarget(rel, sha, bytes.length));
            } catch (Exception e) {
                gaps.add("Skipped '" + rel + "': unreadable (" + e.getMessage() + ")");
            }
        }
        if (targets.isEmpty()) {
            throw new ScanException(ScanException.Code.INVALID_INPUT,
                    "No selectable targets: " + String.join("; ", gaps));
        }
        return new ValidatedSelection(root, targets, gaps);
    }

    /**
     * Runs a scan and returns its terminal summary. Throws only for
     * invalid input, repository-busy, storage failures, or interruption;
     * scanner-level problems become persisted outcomes, never exceptions.
     */
    public ScanSummary runScan(
            Path rootSupplied,
            List<String> relativePaths,
            Set<String> adapterIds,
            String source,
            Path dataDir,
            CancellationToken cancellation,
            ScanListener listener) throws ScanException {
        List<String> adapters = resolveAdapters(adapterIds);
        emit(listener, STAGE_VALIDATE);
        ValidatedSelection selection = validateSelection(rootSupplied, relativePaths);
        RepositoryRoot root = selection.root();
        List<ValidatedTarget> targets = selection.targets();

        SqliteStore store;
        try {
            store = SqliteStore.open(dataDir);
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot open application data directory " + dataDir + ": " + e.getMessage(), e);
        }
        try {
            reconcileInterrupted(dataDir);
            SqliteStore.RepositoryRow repo;
            try {
                repo = store.registerRepository(root.root().toString(), root.displayName(), root.identityHash());
            } catch (Exception e) {
                throw new ScanException(ScanException.Code.STORAGE_ERROR,
                        "Cannot register repository: " + e.getMessage(), e);
            }
            RepoLock lock;
            try {
                Optional<RepoLock> acquired = RepoLock.tryAcquire(dataDir, root.identityHash());
                if (acquired.isEmpty()) {
                    throw new ScanException(ScanException.Code.REPOSITORY_BUSY,
                            "Repository is busy: another scan of this repository is already running"
                                    + " in this data directory (" + dataDir + ")");
                }
                lock = acquired.get();
            } catch (ScanException busy) {
                throw busy;
            } catch (Exception e) {
                throw new ScanException(ScanException.Code.STORAGE_ERROR,
                        "Cannot acquire repository lock: " + e.getMessage(), e);
            }
            try {
                return executeLocked(store, dataDir, repo, root, targets,
                        new ArrayList<>(selection.gaps()),
                        new ArrayList<>(adapters), source, cancellation, listener);
            } finally {
                lock.close();
            }
        } finally {
            try {
                store.close();
            } catch (Exception ignored) {
                // best effort
            }
        }
    }

    /** Result of run setup: either a live run (id + snapshot) or an already-terminal summary. */
    public record BeginResult(String runId, Path snapshot, ScanSummary terminal) {
    }

    private ScanSummary executeLocked(
            SqliteStore store, Path dataDir, SqliteStore.RepositoryRow repo, RepositoryRoot root,
            List<ValidatedTarget> targets, List<String> gaps, List<String> adapters, String source,
            CancellationToken cancellation, ScanListener listener) throws ScanException {
        BeginResult begun = beginRun(store, dataDir, repo, root, targets, gaps, adapters, source,
                cancellation, listener);
        if (begun.terminal() != null) {
            return begun.terminal();
        }
        return finishRun(store, dataDir, begun.runId(), repo, root, targets, gaps, adapters, source,
                begun.snapshot(), cancellation, listener);
    }

    /**
     * Creates the run row, marks it RUNNING, and stages the private snapshot.
     * Fast and non-blocking (no scanner execution). On pre-cancellation or
     * staging failure the run is finalized immediately and returned as
     * {@code terminal}. The caller retains the repository lock until the run
     * (or {@link #finishRun}) completes.
     */
    public BeginResult beginRun(
            SqliteStore store, Path dataDir, SqliteStore.RepositoryRow repo, RepositoryRoot root,
            List<ValidatedTarget> targets, List<String> gaps, List<String> adapters, String source,
            CancellationToken cancellation, ScanListener listener) throws ScanException {
        String configJson = "{\"source\":\"" + (source == null ? "CLI" : source) + "\","
                + "\"tool_timeout_seconds\":" + config.toolTimeout().toSeconds()
                + ",\"adapters\":" + adapters.toString() + "}";
        String runId;
        try {
            runId = store.createRun(repo.id(), BuildInfo.VERSION, configJson);
            store.startRun(runId);
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot create run record: " + e.getMessage(), e);
        }
        if (cancellation != null && cancellation.isCancelled()) {
            ScanSummary terminal = finalizeRun(store, null, runId, repo, root, targets, gaps, List.of(),
                    ScanState.CANCELLED, ScanVerdict.INDETERMINATE, "CANCELLED", "cancelled before staging", runId);
            return new BeginResult(runId, null, terminal);
        }

        emit(listener, STAGE_STAGE);
        Path snapshot;
        try {
            snapshot = Staging.createSnapshot(dataDir, runId);
            for (ValidatedTarget target : targets) {
                Staging.stageFile(root.root(), target.relativePath(), snapshot);
                store.addRunFile(runId, target.relativePath(), target.sha256(), target.sizeBytes());
            }
        } catch (Exception e) {
            ScanSummary terminal = finalizeRun(store, dataDir, runId, repo, root, targets, gaps, List.of(),
                    ScanState.FAILED, ScanVerdict.INDETERMINATE,
                    "STAGING_FAILED", "Could not stage selected files: " + e.getMessage(), runId);
            return new BeginResult(runId, null, terminal);
        }
        return new BeginResult(runId, snapshot, null);
    }

    /**
     * Executes adapters against an already-staged snapshot, persists
     * outcomes, and finalizes the run. Observes both the in-memory token and
     * the durable {@code cancel_requested} flag (set by any process), so a
     * run can be cancelled cross-process.
     */
    public ScanSummary finishRun(
            SqliteStore store, Path dataDir, String runId,
            SqliteStore.RepositoryRow repo, RepositoryRoot root,
            List<ValidatedTarget> targets, List<String> gaps, List<String> adapters, String source,
            Path snapshot, CancellationToken cancellation, ScanListener listener) throws ScanException {
        List<ScannerOutcome> outcomes = new ArrayList<>();
        List<String> orderedAdapters = ADAPTER_ORDER.stream().filter(adapters::contains).toList();
        // Kubeconform default: explicit template wins, otherwise the managed
        // local cache under this data directory (never silent downloads).
        String kubeTemplate = config.kubeconform().schemaTemplate();
        if ((kubeTemplate == null || kubeTemplate.isBlank()) && dataDir != null) {
            kubeTemplate = ScanConfig.KubeconformConfig.cacheTemplate(dataDir);
        }
        boolean cancelled = false;
        for (String adapterId : orderedAdapters) {
            if (isCancelled(store, runId, cancellation)) {
                cancelled = true;
                try {
                    store.setCancelRequested(runId);
                } catch (Exception ignored) {
                    // record-only
                }
                break;
            }
            emitAdapterStarted(listener, adapterId);
            ScannerAdapter adapter = adapterFor(adapterId);
            ScannerOutcome outcome;
            try {
                outcome = adapter.execute(new ScannerAdapter.ScanContext(
                        snapshot, targetRelativePaths(targets), null, config.toolTimeout(), cancellation,
                        kubeTemplate, config.kubeconform().kubernetesVersion(),
                        config.kubeconform().strict(), config.localSchema()));
            } catch (Exception e) {
                LOG.warn("Adapter {} threw; recording FAILED", adapterId);
                outcome = ScannerOutcome.withoutGaps(adapterId, adapterId, null, ScannerStatus.FAILED, null,
                        "Adapter error: " + e.getMessage(), List.of());
            }
            if (outcome.status() == ScannerStatus.CANCELLED) {
                cancelled = true;
            }
            try {
                String scannerRunId = store.recordScannerRun(runId, outcome.adapterId(), outcome.toolName(),
                        outcome.toolVersion(), outcome.status(), outcome.exitCode(), outcome.detail());
                if (!outcome.findings().isEmpty()) {
                    store.addFindings(runId, scannerRunId, outcome.findings());
                }
            } catch (Exception e) {
                return finalizeRun(store, dataDir, runId, repo, root, targets, gaps, outcomes,
                        ScanState.FAILED, ScanVerdict.INDETERMINATE,
                        "STORAGE_ERROR", "Cannot persist scanner outcome: " + e.getMessage(), runId);
            }
            outcomes.add(outcome);
            emitAdapterFinished(listener, adapterId, outcome.status(), outcome.findings().size());
            if (cancelled) {
                break;
            }
        }

        ScanState state;
        ScanVerdict verdict;
        int findingCount = outcomes.stream().mapToInt(o -> o.findings().size()).sum();
        if (cancelled) {
            state = ScanState.CANCELLED;
        } else {
            long succeeded = outcomes.stream().filter(o -> o.status() == ScannerStatus.SUCCEEDED).count();
            // NOT_APPLICABLE is neutral: the check was correctly determined
            // out of scope, so it neither proves coverage nor breaks it.
            long bad = outcomes.stream()
                    .filter(o -> o.status() != ScannerStatus.SUCCEEDED
                            && o.status() != ScannerStatus.NOT_APPLICABLE)
                    .count();
            if (succeeded == 0) {
                state = ScanState.FAILED;
                if (outcomes.stream().allMatch(o -> o.status() == ScannerStatus.NOT_APPLICABLE)
                        && !orderedAdapters.isEmpty()) {
                    gaps.add("No selected file was applicable to any selected check");
                }
            } else if (bad == 0) {
                state = ScanState.COMPLETED;
            } else {
                state = ScanState.PARTIAL;
            }
        }
        if (findingCount > 0) {
            verdict = ScanVerdict.FINDINGS;
        } else if (state == ScanState.COMPLETED) {
            verdict = ScanVerdict.NO_FINDINGS;
        } else {
            verdict = ScanVerdict.INDETERMINATE;
        }
        for (ScannerOutcome outcome : outcomes) {
            gaps.addAll(outcome.coverageGaps());
            if (outcome.status() != ScannerStatus.SUCCEEDED
                    && outcome.status() != ScannerStatus.NOT_APPLICABLE) {
                gaps.add(adapterIdLabel(outcome.adapterId()) + ": "
                        + outcome.status().name().toLowerCase().replace('_', ' ')
                        + (outcome.detail() == null ? "" : " — " + outcome.detail()));
            }
        }
        String errorCode = state == ScanState.COMPLETED ? null : state.name();
        return finalizeRun(store, dataDir, runId, repo, root, targets, gaps, outcomes,
                state, verdict, errorCode, errorCode == null ? null : "Run ended as " + state.name(), runId);
    }

    private static String adapterIdLabel(String adapterId) {
        return switch (adapterId) {
            case YamllintAdapter.ID -> "yamllint";
            case GitleaksAdapter.ID -> "gitleaks";
            case CheckovAdapter.ID -> "checkov";
            case ActionlintAdapter.ID -> "actionlint";
            case KubeconformAdapter.ID -> "kubeconform";
            case LocalSchemaAdapter.ID -> "local-schema";
            default -> adapterId;
        };
    }

    private ScanSummary finalizeRun(SqliteStore store, Path dataDir, String runId,
            SqliteStore.RepositoryRow repo, RepositoryRoot root, List<ValidatedTarget> targets,
            List<String> gaps, List<ScannerOutcome> outcomes, ScanState state, ScanVerdict verdict,
            String errorCode, String errorMessage, String expectedRunId) throws ScanException {
        List<ScanSummary.SelectedTarget> summaryTargets = new ArrayList<>();
        for (ValidatedTarget target : targets) {
            summaryTargets.add(new ScanSummary.SelectedTarget(
                    target.relativePath(), target.sha256(), target.sizeBytes()));
        }
        int findingCount = outcomes.stream().mapToInt(o -> o.findings().size()).sum();
        StringBuilder summaryJson = new StringBuilder("{\"finding_count\":");
        summaryJson.append(findingCount).append(",\"target_count\":").append(targets.size()).append(",\"gaps\":[");
        for (String gap : gaps) {
            summaryJson.append('"').append(pwt.launcher.JsonWriter.escape(gap)).append("\",");
        }
        if (!gaps.isEmpty()) {
            summaryJson.deleteCharAt(summaryJson.length() - 1);
        }
        summaryJson.append("]}");
        try {
            store.finalizeRun(runId, state, verdict, summaryJson.toString(), errorCode, errorMessage);
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot finalize run: " + e.getMessage(), e);
        }
        ReportService.ReportPaths reports = null;
        List<String> reportGaps = new ArrayList<>(gaps);
        if (dataDir != null) {
            try {
                reports = new ReportService().generate(store, dataDir, runId);
                if (reports.jsonError() != null) {
                    reportGaps.add("JSON report export failed: " + reports.jsonError());
                }
                if (reports.markdownError() != null) {
                    reportGaps.add("Markdown report export failed: " + reports.markdownError());
                }
            } catch (Exception e) {
                reportGaps.add("Report generation failed: " + e.getMessage());
                LOG.warn("Report generation failed for run {}: {}", runId, e.toString());
            }
            Staging.deleteRunTransient(dataDir, runId);
        }
        List<String> allGaps = new ArrayList<>(reportGaps);
        SqliteStore.RunView view;
        try {
            view = store.getRun(runId);
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot reload finished run: " + e.getMessage(), e);
        }
        return new ScanSummary(runId, repo.displayName(), root.root().toString(), view.state(), view.verdict(),
                summaryTargets, outcomes, findingCount,
                reports == null ? null : reports.jsonPath(),
                reports == null ? null : reports.markdownPath(),
                view.startedAt(), view.finishedAt(), allGaps);
    }

    private static List<String> targetRelativePaths(List<ValidatedTarget> targets) {
        List<String> paths = new ArrayList<>();
        for (ValidatedTarget target : targets) {
            paths.add(target.relativePath());
        }
        return paths;
    }

    /**
     * Cancellation from either channel: the in-memory token (same process)
     * or the durable {@code cancel_requested} flag (any process). Store
     * failures here are best-effort false; finalization surfaces real
     * storage errors.
     */
    private static boolean isCancelled(SqliteStore store, String runId, CancellationToken cancellation) {
        if (cancellation != null && cancellation.isCancelled()) {
            return true;
        }
        try {
            SqliteStore.RunView view = store.getRun(runId);
            return view != null && view.cancelRequested();
        } catch (Exception ignored) {
            return false;
        }
    }

    /**
     * Reassembles a persisted run into a {@link ScanSummary} (same shape as
     * live results) for MCP responses and GUI history views. Report paths
     * come from READY artifact rows.
     */
    public static ScanSummary loadSummary(SqliteStore store, String runId) throws ScanException {
        SqliteStore.RunView view;
        SqliteStore.RepositoryRow repo;
        try {
            view = store.getRun(runId);
            if (view == null) {
                return null;
            }
            repo = store.findRepositoryById(view.repositoryId());
            if (repo == null) {
                throw new ScanException(ScanException.Code.STORAGE_ERROR,
                        "Run references unknown repository");
            }
        } catch (ScanException e) {
            throw e;
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot load run: " + e.getMessage(), e);
        }
        List<ScanSummary.SelectedTarget> targets = new ArrayList<>();
        List<ScannerOutcome> outcomes = new ArrayList<>();
        List<String> gaps = new ArrayList<>();
        String jsonReport = null;
        String markdownReport = null;
        try {
            for (SqliteStore.TargetRow row : store.listRunFiles(runId)) {
                targets.add(new ScanSummary.SelectedTarget(
                        row.relativePath(), row.sha256(), row.sizeBytes()));
            }
            List<SqliteStore.ScannerRunRow> scanners = store.listScannerRuns(runId);
            java.util.Map<String, List<Finding>> findingsByScanner = new java.util.HashMap<>();
            for (SqliteStore.FindingRow row : store.listFindings(runId)) {
                findingsByScanner.computeIfAbsent(row.scannerRunId(), key -> new ArrayList<>())
                        .add(row.finding());
            }
            for (SqliteStore.ScannerRunRow scanner : scanners) {
                outcomes.add(new ScannerOutcome(scanner.adapterId(), scanner.toolName(),
                        scanner.toolVersion(), scanner.status(), scanner.exitCode(), scanner.message(),
                        findingsByScanner.getOrDefault(scanner.id(), List.of()), List.of()));
            }
            for (SqliteStore.ArtifactRow artifact : store.listArtifacts(runId)) {
                if (!"READY".equals(artifact.status()) || artifact.relativePath() == null) {
                    continue;
                }
                if ("JSON_REPORT".equals(artifact.kind())) {
                    jsonReport = artifact.relativePath();
                } else if ("MARKDOWN_REPORT".equals(artifact.kind())) {
                    markdownReport = artifact.relativePath();
                }
            }
            gaps.addAll(extractSummaryGaps(view.summaryJson()));
        } catch (Exception e) {
            throw new ScanException(ScanException.Code.STORAGE_ERROR,
                    "Cannot load run details: " + e.getMessage(), e);
        }
        int findingCount = outcomes.stream().mapToInt(o -> o.findings().size()).sum();
        return new ScanSummary(runId, repo.displayName(), repo.canonicalRoot(), view.state(), view.verdict(),
                targets, outcomes, findingCount, jsonReport, markdownReport,
                view.startedAt(), view.finishedAt(), gaps);
    }

    public static List<String> extractSummaryGaps(String summaryJson) {
        List<String> gaps = new ArrayList<>();
        if (summaryJson == null || summaryJson.isBlank()) {
            return gaps;
        }
        try {
            com.fasterxml.jackson.databind.JsonNode root =
                    new com.fasterxml.jackson.databind.ObjectMapper().readTree(summaryJson);
            com.fasterxml.jackson.databind.JsonNode array = root.path("gaps");
            if (array.isArray()) {
                for (com.fasterxml.jackson.databind.JsonNode gap : array) {
                    if (gap.isTextual()) {
                        gaps.add(gap.asText());
                    }
                }
            }
        } catch (Exception ignored) {
            // corrupt summary JSON: gaps stay empty rather than breaking loads
        }
        return gaps;
    }

    private static void emit(ScanListener listener, String stage) {
        if (listener != null) {
            try {
                listener.stage(stage);
            } catch (Exception ignored) {
                // listener must never break a scan
            }
        }
    }

    private static void emitAdapterStarted(ScanListener listener, String adapterId) {
        if (listener != null) {
            try {
                listener.adapterStarted(adapterId);
            } catch (Exception ignored) {
                // listener must never break a scan
            }
        }
    }

    private static void emitAdapterFinished(ScanListener listener, String adapterId,
            ScannerStatus status, int findingCount) {
        if (listener != null) {
            try {
                listener.adapterFinished(adapterId, status, findingCount);
            } catch (Exception ignored) {
                // listener must never break a scan
            }
        }
    }

    /**
     * Minimal interrupted-run reconciliation: any run still marked
     * CREATED/RUNNING whose repository lock is currently acquirable belonged
     * to a dead process, so it is marked INTERRUPTED. Called at scan start.
     * Comprehensive crash recovery is Phase 6 work.
     */
    public static void reconcileInterrupted(Path dataDir) {
        SqliteStore store;
        try {
            store = SqliteStore.open(dataDir);
        } catch (Exception e) {
            LOG.warn("Startup reconcile skipped: cannot open data dir {}: {}", dataDir, e.toString());
            return;
        }
        try {
            for (SqliteStore.RunView run : store.listNonTerminalRuns()) {
                SqliteStore.RepositoryRow repo = store.findRepositoryById(run.repositoryId());
                if (repo == null) {
                    continue;
                }
                Optional<RepoLock> free;
                try {
                    free = RepoLock.tryAcquire(dataDir, repo.identityHash());
                } catch (Exception e) {
                    continue;
                }
                if (free.isEmpty()) {
                    continue;
                }
                try (RepoLock ignored = free.get()) {
                    store.finalizeRun(run.id(), ScanState.INTERRUPTED, run.verdict() == ScanVerdict.FINDINGS
                            ? ScanVerdict.FINDINGS : ScanVerdict.INDETERMINATE,
                            run.summaryJson(), "INTERRUPTED",
                            "Process ended without finishing this run; marked INTERRUPTED on startup");
                    LOG.warn("Run {} marked INTERRUPTED on startup", run.id());
                } catch (Exception e) {
                    LOG.warn("Could not reconcile run {}: {}", run.id(), e.toString());
                }
            }
        } catch (Exception e) {
            LOG.warn("Startup reconcile failed: {}", e.toString());
        } finally {
            try {
                store.close();
            } catch (Exception ignored) {
                // best effort
            }
        }
    }
}

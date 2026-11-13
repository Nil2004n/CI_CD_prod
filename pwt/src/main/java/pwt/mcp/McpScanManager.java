package pwt.mcp;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.application.CancellationToken;
import pwt.application.LocalSchemaConfig;
import pwt.application.ScanConfig;
import pwt.application.ScanException;
import pwt.application.ScanService;
import pwt.domain.ScanSummary;
import pwt.infrastructure.RepoLock;
import pwt.infrastructure.SqliteStore;

/**
 * Owns MCP-triggered scan execution. {@code start_scan} validates everything
 * synchronously, begins the durable run (row + staged snapshot) while holding
 * the repository lock, then returns the run ID promptly; a bounded background
 * pool finishes the work. {@code cancel_scan} sets the durable
 * {@code cancel_requested} flag (observed by workers in any process) and
 * cancels the in-process token when this process owns the run.
 */
public final class McpScanManager implements AutoCloseable {

    private static final Logger LOG = LoggerFactory.getLogger(McpScanManager.class);
    private static final int MAX_WORKERS = 2;

    private final Path dataDir;
    private final ExecutorService workers;
    private final Map<String, CancellationToken> liveTokens = new ConcurrentHashMap<>();

    public McpScanManager(Path dataDir) {
        this.dataDir = dataDir;
        this.workers = Executors.newFixedThreadPool(MAX_WORKERS, runnable -> {
            Thread worker = new Thread(runnable, "pwt-mcp-scan");
            worker.setDaemon(false);
            return worker;
        });
    }

    public record StartResult(String runId, String state) {
    }

    public record CancelResult(String runId, String state, boolean cancelRequested, String note) {
    }

    /**
     * Validates, begins, and background-finishes a scan. Throws
     * {@link McpErrors.McpServiceException} with a protocol error code.
     */
    public StartResult startScan(String repositoryId, List<String> relativePaths,
            Set<String> adapterIds, ScanConfig config, String source) throws McpErrors.McpServiceException {
        if (repositoryId == null || repositoryId.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, "repository_id is required");
        }
        if (relativePaths == null || relativePaths.isEmpty()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT,
                    "relative_paths must name at least one file");
        }
        if (config == null) {
            config = ScanConfig.defaults();
        }
        ScanService service = new ScanService(config);
        List<String> adapters;
        try {
            adapters = ScanService.resolveAdapters(adapterIds);
        } catch (ScanException e) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, e.getMessage());
        }
        SqliteStore store = openStore();
        try {
            SqliteStore.RepositoryRow repo;
            try {
                repo = store.findRepositoryById(repositoryId);
            } catch (Exception e) {
                throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                        "cannot look up repository: " + e.getMessage());
            }
            if (repo == null) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND,
                        "unknown repository_id: " + repositoryId);
            }
            if (!Files.isDirectory(Path.of(repo.canonicalRoot()))) {
                throw new McpErrors.McpServiceException(McpErrors.OUT_OF_SCOPE,
                        "registered repository is currently unavailable: " + repo.displayName());
            }
            ScanService.ValidatedSelection selection;
            try {
                selection = service.validateSelection(Path.of(repo.canonicalRoot()), relativePaths);
            } catch (ScanException e) {
                throw mapScanException(e);
            }
            // Local-schema without bindings can never validate anything: fail
            // fast with configuration guidance instead of a hollow SKIPPED run.
            if (adapters.contains(pwt.adapters.LocalSchemaAdapter.ID)
                    && (config.localSchema() == null || config.localSchema().isEmpty())) {
                throw new McpErrors.McpServiceException(McpErrors.CONFIGURATION_REQUIRED,
                        "local-schema selected but no schema_bindings supplied"
                                + " (pass GLOB=ABSOLUTE_SCHEMA_PATH entries)");
            }
            RepoLock lock;
            try {
                Optional<RepoLock> acquired = RepoLock.tryAcquire(dataDir, repo.identityHash());
                if (acquired.isEmpty()) {
                    throw new McpErrors.McpServiceException(McpErrors.REPOSITORY_BUSY,
                            "repository is busy: another scan of '" + repo.displayName()
                                    + "' is already running in this data directory");
                }
                lock = acquired.get();
            } catch (McpErrors.McpServiceException busy) {
                throw busy;
            } catch (Exception e) {
                throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                        "cannot acquire repository lock: " + e.getMessage());
            }
            ScanService.BeginResult begun;
            try {
                begun = service.beginRun(store, dataDir, repo, selection.root(), selection.targets(),
                        new ArrayList<>(selection.gaps()), new ArrayList<>(adapters), source,
                        CancellationToken.none(), null);
            } catch (ScanException e) {
                lock.close();
                throw mapScanException(e);
            }
            if (begun.terminal() != null) {
                lock.close();
                return new StartResult(begun.runId(), begun.terminal().state().name());
            }
            CancellationToken token = new CancellationToken();
            liveTokens.put(begun.runId(), token);
            String runId = begun.runId();
            java.nio.file.Path snapshot = begun.snapshot();
            List<ScanService.ValidatedTarget> targets = selection.targets();
            List<String> gaps = new ArrayList<>(selection.gaps());
            List<String> adapterList = new ArrayList<>(adapters);
            ScanConfig runConfig = config;
            workers.submit(() -> {
                try {
                    service.finishRun(store, dataDir, runId, repo, selection.root(), targets, gaps,
                            adapterList, source, snapshot, token, null);
                } catch (ScanException e) {
                    LOG.warn("MCP background scan {} failed: {}", runId, e.toString());
                } catch (Exception e) {
                    LOG.warn("MCP background scan {} crashed: {}", runId, e.toString());
                } finally {
                    liveTokens.remove(runId);
                    try {
                        store.close();
                    } catch (Exception ignored) {
                        // best effort
                    }
                    lock.close();
                }
            });
            return new StartResult(runId, "RUNNING");
        } catch (McpErrors.McpServiceException e) {
            try {
                store.close();
            } catch (Exception ignored) {
                // best effort
            }
            throw e;
        }
    }

    public CancelResult cancelScan(String runId) throws McpErrors.McpServiceException {
        if (runId == null || runId.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, "run_id is required");
        }
        try (SqliteStore store = openStore()) {
            SqliteStore.RunView view;
            try {
                view = store.getRun(runId);
            } catch (Exception e) {
                throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                        "cannot load run: " + e.getMessage());
            }
            if (view == null) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND, "unknown run_id: " + runId);
            }
            if (isTerminal(view.state().name())) {
                return new CancelResult(runId, view.state().name(), false,
                        "run is already terminal; nothing to cancel");
            }
            try {
                store.setCancelRequested(runId);
            } catch (Exception e) {
                throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                        "cannot record cancellation: " + e.getMessage());
            }
            CancellationToken token = liveTokens.get(runId);
            if (token != null) {
                token.cancel();
            }
            SqliteStore.RunView current;
            try {
                current = store.getRun(runId);
            } catch (Exception e) {
                throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                        "cannot reload run: " + e.getMessage());
            }
            String state = current == null ? view.state().name() : current.state().name();
            return new CancelResult(runId, state, true,
                    token != null
                            ? "cancellation requested; owning worker is signalling its tools"
                            : "cancellation requested via durable flag; the owning process observes it");
        } catch (McpErrors.McpServiceException e) {
            throw e;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
        }
    }

    /** Loads the live summary of a run (any process may have created it). */
    public ScanSummary getSummary(String runId) throws McpErrors.McpServiceException {
        if (runId == null || runId.isBlank()) {
            throw new McpErrors.McpServiceException(McpErrors.INVALID_INPUT, "run_id is required");
        }
        try (SqliteStore store = openStore()) {
            ScanSummary summary = ScanService.loadSummary(store, runId);
            if (summary == null) {
                throw new McpErrors.McpServiceException(McpErrors.NOT_FOUND, "unknown run_id: " + runId);
            }
            return summary;
        } catch (McpErrors.McpServiceException e) {
            throw e;
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR, e.getMessage());
        }
    }

    private SqliteStore openStore() throws McpErrors.McpServiceException {
        try {
            return SqliteStore.open(dataDir);
        } catch (Exception e) {
            throw new McpErrors.McpServiceException(McpErrors.INTERNAL_ERROR,
                    "cannot open application data directory: " + e.getMessage());
        }
    }

    private static McpErrors.McpServiceException mapScanException(ScanException e) {
        String code = switch (e.code()) {
            case INVALID_INPUT -> McpErrors.INVALID_INPUT;
            case REPOSITORY_BUSY -> McpErrors.REPOSITORY_BUSY;
            case STORAGE_ERROR, INTERRUPTED -> McpErrors.INTERNAL_ERROR;
        };
        // Path-escape rejections surface as OUT_OF_SCOPE for agent clarity.
        if (e.code() == ScanException.Code.INVALID_INPUT && e.getMessage() != null
                && (e.getMessage().contains("outside the registered root")
                        || e.getMessage().contains("symlinked"))) {
            return new McpErrors.McpServiceException(McpErrors.OUT_OF_SCOPE, e.getMessage());
        }
        return new McpErrors.McpServiceException(code, e.getMessage());
    }

    private static boolean isTerminal(String state) {
        return switch (state) {
            case "COMPLETED", "PARTIAL", "FAILED", "CANCELLED", "INTERRUPTED" -> true;
            default -> false;
        };
    }

    public Path dataDir() {
        return dataDir;
    }

    @Override
    public void close() {
        for (CancellationToken token : liveTokens.values()) {
            try {
                token.cancel();
            } catch (Exception ignored) {
                // best effort
            }
        }
        workers.shutdown();
        try {
            if (!workers.awaitTermination(30, TimeUnit.SECONDS)) {
                workers.shutdownNow();
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            workers.shutdownNow();
        }
    }
}

package pwt.desktop;

import java.io.File;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicLong;
import javafx.application.Application;
import javafx.application.Platform;
import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Insets;
import javafx.scene.Scene;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.Label;
import javafx.scene.control.ListView;
import javafx.scene.control.SelectionMode;
import javafx.scene.control.SplitPane;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.control.TextField;
import javafx.scene.control.cell.PropertyValueFactory;
import javafx.scene.layout.BorderPane;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.VBox;
import javafx.scene.text.Font;
import javafx.stage.DirectoryChooser;
import javafx.stage.Stage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import pwt.adapters.ActionlintAdapter;
import pwt.adapters.CheckovAdapter;
import pwt.adapters.GitleaksAdapter;
import pwt.adapters.KubeconformAdapter;
import pwt.adapters.LocalSchemaAdapter;
import pwt.adapters.ScannerCatalog;
import pwt.adapters.YamllintAdapter;
import pwt.application.BuildInfo;
import pwt.application.CancellationToken;
import pwt.application.DiscoveryException;
import pwt.application.DiscoveryService;
import pwt.application.LocalSchemaConfig;
import pwt.application.ScanConfig;
import pwt.application.ScanException;
import pwt.application.ScanService;
import pwt.domain.DiscoveryOptions;
import pwt.domain.DiscoveryResult;
import pwt.domain.DiscoveryWarning;
import pwt.domain.Finding;
import pwt.domain.RepositoryRoot;
import pwt.domain.ScanSummary;
import pwt.domain.ScannerDescriptor;
import pwt.domain.ScannerOutcome;
import pwt.domain.ScannerStatus;
import pwt.domain.ToolProbe;
import pwt.infrastructure.NioDiscoveryService;

/**
 * Terminal-inspired (not a terminal emulator) desktop shell.
 *
 * <p>Layout: repository/file navigation on the left, central workspace with
 * discovered files and selection controls, scanner catalog with live
 * availability, real scan execution with findings, report locations, a status
 * bar, and a sanitized application-event log. No shell, no terminal
 * emulation, no chat panel, no fabricated findings, no MCP "connected"
 * indicator.
 *
 * <p>Threading: discovery and scans run on background executors; every UI
 * mutation goes through {@link Platform#runLater}. A generation counter
 * discards stale discovery completions so an older task can never overwrite
 * a newer selection. Cancel signals the in-flight task cooperatively.
 */
public class PwtDesktopApp extends Application {

    private static final Logger LOG = LoggerFactory.getLogger(PwtDesktopApp.class);
    private static final int MAX_LOG_ENTRIES = 500;

    private final DiscoveryService discoveryService = new NioDiscoveryService();
    private final ExecutorService discoveryExecutor = Executors.newSingleThreadExecutor(r -> {
        Thread worker = new Thread(r, "pwt-discovery");
        worker.setDaemon(true);
        return worker;
    });
    private final ExecutorService scanExecutor = Executors.newSingleThreadExecutor(r -> {
        Thread worker = new Thread(r, "pwt-scan");
        worker.setDaemon(true);
        return worker;
    });
    private final AtomicLong generation = new AtomicLong(0);
    private volatile CancellationToken activeCancellation = CancellationToken.none();
    private volatile Future<?> activeTask;
    private volatile RepositoryRoot activeRoot;
    private volatile CancellationToken scanCancellation;
    private volatile boolean scanning;

    private TextField rootField;
    private ListView<String> fileList;
    private Label countsLabel;
    private Label stateLabel;
    private Label incompleteLabel;
    private ListView<String> logList;
    private ObservableList<String> logItems;
    private Label statusLabel;
    private Button cancelButton;
    private Button scanButton;
    private Label verdictLabel;
    private Label reportsLabel;
    private Button openReportsButton;
    private TableView<FindingRow> findingsTable;
    private ObservableList<FindingRow> findingRows;
    private ListView<String> outcomesList;
    private ObservableList<String> outcomeItems;
    private final java.util.Map<String, ScannerRow> rowByTool = new java.util.HashMap<>();
    private final java.util.Map<String, CheckBox> adapterBoxes = new java.util.LinkedHashMap<>();
    private TextField bindingsField;
    private TextField kubeVersionField;
    private CheckBox kubeStrictBox;
    private volatile int availableAdapterCount;

    @Override
    public void start(Stage stage) {
        stage.setTitle(BuildInfo.APP_NAME + " " + BuildInfo.VERSION + " (" + BuildInfo.PHASE + ")");

        BorderPane root = new BorderPane();
        root.setStyle("-fx-background-color: #0d1117;");

        root.setTop(buildToolbar(stage));
        SplitPane center = new SplitPane(buildFilePane(), buildCatalogPane());
        center.setDividerPositions(0.55);
        root.setCenter(center);
        root.setBottom(buildBottom());

        Scene scene = new Scene(root, 1100, 700);
        stage.setScene(scene);
        log("PWT desktop started. Open a directory to discover YAML files.");
        log("MCP sessions observed by this process: none (separate MCP processes do not share session state).");
        stage.show();
        LOG.warn("PWT desktop started");
        refreshAvailability();
        scanExecutor.submit(() -> ScanService.reconcileInterrupted(ScanService.defaultDataDir()));
    }

    private HBox buildToolbar(Stage stage) {
        Label title = styledLabel(" PWT // YAML WORKBENCH  [" + BuildInfo.PHASE + "] ");
        rootField = new TextField();
        rootField.setPromptText("No directory open — use Open directory…");
        rootField.setEditable(false);
        rootField.setFont(Font.font("Monospace", 12));
        rootField.setStyle("-fx-control-inner-background: #161b22; -fx-text-fill: #c9d1d9; "
                + "-fx-prompt-text-fill: #6e7681;");
        HBox.setHgrow(rootField, Priority.ALWAYS);

        Button openButton = new Button("Open directory…");
        styleButton(openButton);
        openButton.setOnAction(event -> chooseAndDiscover(stage));

        cancelButton = new Button("Cancel discovery");
        styleButton(cancelButton);
        cancelButton.setDisable(true);
        cancelButton.setOnAction(event -> cancelDiscovery());

        HBox toolbar = new HBox(8, title, rootField, openButton, cancelButton);
        toolbar.setPadding(new Insets(8));
        toolbar.setStyle("-fx-background-color: #161b22; -fx-border-color: #30363d; -fx-border-width: 0 0 1 0;");
        return toolbar;
    }

    private VBox buildFilePane() {
        Label heading = styledLabel("DISCOVERED YAML  (mouse/keyboard multi-select)");
        fileList = new ListView<>();
        fileList.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);
        fileList.setStyle("-fx-control-inner-background: #0d1117; -fx-text-fill: #c9d1d9; -fx-font-family: monospace;");
        fileList.getSelectionModel().selectedItemProperty().addListener((obs, old, current) -> updateCounts());
        VBox.setVgrow(fileList, Priority.ALWAYS);

        countsLabel = styledLabel("discovered: 0   selected: 0");
        stateLabel = styledLabel("");
        incompleteLabel = styledLabel("");
        incompleteLabel.setStyle("-fx-text-fill: #e3b341; -fx-font-family: monospace; -fx-font-size: 12;");

        Button selectAll = new Button("Select all");
        Button selectNone = new Button("Select none");
        styleButton(selectAll);
        styleButton(selectNone);
        selectAll.setOnAction(event -> fileList.getSelectionModel().selectAll());
        selectNone.setOnAction(event -> fileList.getSelectionModel().clearSelection());
        HBox selectionBar = new HBox(8, selectAll, selectNone, countsLabel);
        selectionBar.setPadding(new Insets(4, 0, 0, 0));

        VBox pane = new VBox(4, heading, fileList, selectionBar, stateLabel, incompleteLabel);
        pane.setPadding(new Insets(8));
        pane.setStyle("-fx-background-color: #0d1117;");
        return pane;
    }

    private VBox buildCatalogPane() {
        Label heading = styledLabel("SCANNER CATALOG  (availability probed locally)");
        TableView<ScannerRow> table = new TableView<>();
        table.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        table.setStyle("-fx-control-inner-background: #0d1117; -fx-text-fill: #c9d1d9;");
        table.setPlaceholder(new Label(""));

        TableColumn<ScannerRow, String> toolColumn = new TableColumn<>("Tool");
        toolColumn.setCellValueFactory(new PropertyValueFactory<>("tool"));
        TableColumn<ScannerRow, String> purposeColumn = new TableColumn<>("Intended purpose");
        purposeColumn.setCellValueFactory(new PropertyValueFactory<>("purpose"));
        TableColumn<ScannerRow, String> stateColumn = new TableColumn<>("Integration");
        stateColumn.setCellValueFactory(new PropertyValueFactory<>("state"));
        TableColumn<ScannerRow, String> availabilityColumn = new TableColumn<>("Availability");
        availabilityColumn.setCellValueFactory(new PropertyValueFactory<>("availability"));
        table.getColumns().addAll(toolColumn, purposeColumn, stateColumn, availabilityColumn);

        ObservableList<ScannerRow> rows = FXCollections.observableArrayList();
        for (ScannerDescriptor descriptor : ScannerCatalog.planned()) {
            ScannerRow row = new ScannerRow(descriptor.toolName(), descriptor.purpose(),
                    descriptor.execution().name(), "probing…");
            rows.add(row);
            rowByTool.put(descriptor.id(), row);
        }
        table.setItems(rows);

        scanButton = new Button("Run scan (selected files)");
        styleButton(scanButton);
        scanButton.setDisable(true);
        scanButton.setOnAction(event -> onScanButton());

        Label checksHeading = styledLabel("ENABLED CHECKS");
        HBox checksBar = new HBox(8);
        for (String adapterId : ScanService.ADAPTER_ORDER) {
            CheckBox box = new CheckBox(adapterId);
            box.setSelected(true);
            box.setStyle("-fx-text-fill: #c9d1d9; -fx-font-family: monospace; -fx-font-size: 12;");
            box.setOnAction(event -> updateScanButton());
            adapterBoxes.put(adapterId, box);
            checksBar.getChildren().add(box);
        }

        Label bindingsHeading = styledLabel("LOCAL SCHEMA BINDINGS  (glob=path, comma-separated; empty = not used)");
        bindingsField = new TextField();
        bindingsField.setPromptText("e.g. widgets/*.yaml=/abs/path/schema.json");
        bindingsField.setFont(Font.font("Monospace", 12));
        bindingsField.setStyle("-fx-control-inner-background: #161b22; -fx-text-fill: #c9d1d9; "
                + "-fx-prompt-text-fill: #6e7681;");
        HBox.setHgrow(bindingsField, Priority.ALWAYS);

        Label kubeHeading = styledLabel("KUBECONFORM  (local schemas only; setup: pwt kubeconform-setup)");
        kubeVersionField = new TextField("1.29.0");
        kubeVersionField.setPrefColumnCount(8);
        kubeVersionField.setFont(Font.font("Monospace", 12));
        kubeVersionField.setStyle("-fx-control-inner-background: #161b22; -fx-text-fill: #c9d1d9;");
        kubeStrictBox = new CheckBox("strict");
        kubeStrictBox.setSelected(true);
        kubeStrictBox.setStyle("-fx-text-fill: #c9d1d9; -fx-font-family: monospace; -fx-font-size: 12;");
        HBox kubeBar = new HBox(8, kubeHeading, kubeVersionField, kubeStrictBox);

        verdictLabel = styledLabel("");
        reportsLabel = styledLabel("");
        openReportsButton = new Button("Open reports folder");
        styleButton(openReportsButton);
        openReportsButton.setDisable(true);
        openReportsButton.setOnAction(event -> openReportsFolder());
        HBox reportsBar = new HBox(8, openReportsButton, reportsLabel);
        HBox.setHgrow(reportsLabel, Priority.ALWAYS);

        Label findingsHeading = styledLabel("FINDINGS (selected scope only)");
        findingsTable = new TableView<>();
        findingsTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        findingsTable.setStyle("-fx-control-inner-background: #0d1117; -fx-text-fill: #c9d1d9;");
        findingsTable.setPlaceholder(new Label("No scan yet — findings appear here."));
        TableColumn<FindingRow, String> severityColumn = new TableColumn<>("Severity");
        severityColumn.setCellValueFactory(new PropertyValueFactory<>("severity"));
        TableColumn<FindingRow, String> ruleColumn = new TableColumn<>("Rule");
        ruleColumn.setCellValueFactory(new PropertyValueFactory<>("rule"));
        TableColumn<FindingRow, String> fileColumn = new TableColumn<>("File");
        fileColumn.setCellValueFactory(new PropertyValueFactory<>("file"));
        TableColumn<FindingRow, String> lineColumn = new TableColumn<>("Line");
        lineColumn.setCellValueFactory(new PropertyValueFactory<>("line"));
        TableColumn<FindingRow, String> messageColumn = new TableColumn<>("Message");
        messageColumn.setCellValueFactory(new PropertyValueFactory<>("message"));
        findingsTable.getColumns().addAll(severityColumn, ruleColumn, fileColumn, lineColumn, messageColumn);
        findingRows = FXCollections.observableArrayList();
        findingsTable.setItems(findingRows);
        findingsTable.setPrefHeight(150);

        Label outcomesHeading = styledLabel("SCANNER OUTCOMES (all configured checks)");
        outcomeItems = FXCollections.observableArrayList();
        outcomesList = new ListView<>(outcomeItems);
        outcomesList.setPrefHeight(90);
        outcomesList.setStyle("-fx-control-inner-background: #161b22; -fx-text-fill: #8b949e; "
                + "-fx-font-family: monospace; -fx-font-size: 11;");

        Label mcpLabel = styledLabel("MCP: not observed in this process.");
        VBox pane = new VBox(6, heading, table, scanButton, checksHeading, checksBar,
                bindingsHeading, bindingsField, kubeBar, verdictLabel, outcomesHeading,
                outcomesList, findingsHeading, findingsTable, reportsBar, mcpLabel);
        pane.setPadding(new Insets(8));
        pane.setStyle("-fx-background-color: #0d1117; -fx-border-color: #30363d; -fx-border-width: 0 0 0 1;");
        return pane;
    }

    private VBox buildBottom() {
        Label heading = styledLabel("APPLICATION EVENTS (sanitized)");
        logItems = FXCollections.observableArrayList();
        logList = new ListView<>(logItems);
        logList.setPrefHeight(130);
        logList.setStyle("-fx-control-inner-background: #161b22; -fx-text-fill: #8b949e; -fx-font-family: monospace;");
        statusLabel = styledLabel("status: idle");
        VBox bottom = new VBox(4, heading, logList, statusLabel);
        bottom.setPadding(new Insets(8));
        bottom.setStyle("-fx-background-color: #0d1117; -fx-border-color: #30363d; -fx-border-width: 1 0 0 0;");
        return bottom;
    }

    private void chooseAndDiscover(Stage stage) {
        DirectoryChooser chooser = new DirectoryChooser();
        chooser.setTitle("Open repository directory");
        File chosen = chooser.showDialog(stage);
        if (chosen == null) {
            return;
        }
        startDiscovery(chosen.toPath());
    }

    private void startDiscovery(Path directory) {
        long ticket = generation.incrementAndGet();
        cancelInFlight();
        CancellationToken cancellation = new CancellationToken();
        activeCancellation = cancellation;
        Platform.runLater(() -> {
            stateLabel.setText("");
            incompleteLabel.setText("");
            statusLabel.setText("status: discovering…");
            cancelButton.setDisable(false);
        });
        log("Opening directory: " + directory);

        activeTask = discoveryExecutor.submit(() -> {
            RepositoryRoot registered;
            try {
                registered = discoveryService.registerRoot(directory);
            } catch (DiscoveryException e) {
                finishIfCurrent(ticket, () -> {
                    rootField.setText("");
                    fileList.getItems().clear();
                    updateCounts();
                    stateLabel.setText("Invalid root (" + e.code() + "): " + e.getMessage());
                    statusLabel.setText("status: invalid root");
                });
                log("Root rejected (" + e.code() + "): " + e.getMessage());
                return;
            }
            activeRoot = registered;
            DiscoveryResult result;
            try {
                result = discoveryService.discover(registered, DiscoveryOptions.defaults(), cancellation);
            } catch (DiscoveryException e) {
                DiscoveryException.Code code = e.code();
                finishIfCurrent(ticket, () -> {
                    rootField.setText(registered.root().toString());
                    fileList.getItems().clear();
                    updateCounts();
                    if (code == DiscoveryException.Code.CANCELLED) {
                        stateLabel.setText("Discovery cancelled; partial results discarded.");
                        statusLabel.setText("status: cancelled");
                    } else {
                        stateLabel.setText("Discovery failed (" + code + "): " + e.getMessage());
                        statusLabel.setText("status: discovery failed");
                    }
                });
                log("Discovery " + (code == DiscoveryException.Code.CANCELLED ? "cancelled" : "failed")
                        + " (" + code + "): " + e.getMessage());
                return;
            }
            List<String> paths = new ArrayList<>();
            for (var file : result.files()) {
                paths.add(file.relativePath());
            }
            finishIfCurrent(ticket, () -> {
                rootField.setText(registered.root().toString());
                fileList.getItems().setAll(paths);
                updateCounts();
                if (paths.isEmpty()) {
                    stateLabel.setText("No YAML files found beneath this root.");
                } else {
                    stateLabel.setText("");
                }
                if (result.incomplete()) {
                    incompleteLabel.setText("INCOMPLETE DISCOVERY — a bound or cancellation stopped "
                            + "traversal early. See events; coverage is partial.");
                }
                StringBuilder warnings = new StringBuilder();
                for (DiscoveryWarning warning : result.warnings()) {
                    String where = warning.path() == null ? "" : " [" + warning.path() + "]";
                    warnings.append(warning.code()).append(where).append("; ");
                }
                statusLabel.setText("status: discovered " + paths.size()
                        + (result.incomplete() ? " (INCOMPLETE)" : " (complete)"));
                log("Discovered " + paths.size() + " YAML file(s)"
                        + (result.incomplete() ? " — INCOMPLETE" : "")
                        + (warnings.length() == 0 ? "" : " — " + warnings));
                for (DiscoveryWarning warning : result.warnings()) {
                    log("warning " + warning.code()
                            + (warning.path() == null ? "" : " [" + warning.path() + "]")
                            + ": " + warning.message());
                }
            });
        });
    }

    private void cancelInFlight() {
        activeCancellation.cancel();
        Future<?> task = activeTask;
        if (task != null) {
            task.cancel(true);
        }
    }

    private void cancelDiscovery() {
        log("Cancellation requested by user.");
        cancelInFlight();
    }

    private void finishIfCurrent(long ticket, Runnable update) {
        if (ticket != generation.get()) {
            // Stale task: a newer discovery replaced it; never overwrite.
            return;
        }
        Platform.runLater(() -> {
            if (ticket != generation.get()) {
                return;
            }
            update.run();
            cancelButton.setDisable(true);
        });
    }

    private void updateCounts() {
        int discovered = fileList.getItems().size();
        int selected = fileList.getSelectionModel().getSelectedItems().size();
        countsLabel.setText("discovered: " + discovered + "   selected: " + selected);
        updateScanButton();
    }

    private void updateScanButton() {
        if (scanButton == null) {
            return;
        }
        if (scanning) {
            scanButton.setDisable(false);
            scanButton.setText("Cancel scan");
            return;
        }
        scanButton.setText("Run scan (selected files)");
        boolean ready = activeRoot != null
                && !fileList.getSelectionModel().getSelectedItems().isEmpty()
                && availableAdapterCount > 0;
        scanButton.setDisable(!ready);
    }

    private void refreshAvailability() {
        scanExecutor.submit(() -> {
            java.util.Map<String, ToolProbe> probes = new java.util.LinkedHashMap<>();
            probes.put("yamllint", new YamllintAdapter().probe());
            probes.put("gitleaks", new GitleaksAdapter().probe());
            probes.put("checkov", new CheckovAdapter().probe());
            probes.put("actionlint", new ActionlintAdapter().probe());
            probes.put("kubeconform", new KubeconformAdapter().probe());
            probes.put("local-schema", new LocalSchemaAdapter().probe());
            long cached = KubeconformAdapter.countCachedSchemas(
                    ScanService.defaultDataDir().resolve("schemas").resolve("kubeconform"));
            int available = 0;
            java.util.Map<String, String> texts = new java.util.LinkedHashMap<>();
            for (java.util.Map.Entry<String, ToolProbe> entry : probes.entrySet()) {
                String text = availabilityText(entry.getValue());
                if (entry.getKey().equals("kubeconform") && entry.getValue().available()) {
                    text += " (local schemas cached: " + cached + ")";
                }
                texts.put(entry.getKey(), text);
                if (entry.getValue().available()) {
                    available++;
                }
            }
            int count = available;
            Platform.runLater(() -> {
                availableAdapterCount = count;
                for (java.util.Map.Entry<String, String> entry : texts.entrySet()) {
                    setAvailability(entry.getKey(), entry.getValue());
                }
                updateScanButton();
            });
            StringBuilder summary = new StringBuilder("Scanner availability:");
            for (java.util.Map.Entry<String, String> entry : texts.entrySet()) {
                summary.append(' ').append(entry.getKey()).append(' ').append(entry.getValue()).append(';');
            }
            log(summary.toString());
        });
    }

    private static String availabilityText(ToolProbe probe) {
        if (probe.available()) {
            return probe.version() == null ? "available" : probe.version() + " — available";
        }
        return probe.reason() == null ? "unavailable" : "unavailable — " + probe.reason();
    }

    private void setAvailability(String adapterId, String text) {
        ScannerRow row = rowByTool.get(adapterId);
        if (row != null) {
            row.setAvailability(EventSanitizer.sanitize(text));
        }
    }

    private Path lastArtifactsDir;

    private void onScanButton() {
        if (scanning) {
            CancellationToken cancellation = scanCancellation;
            if (cancellation != null) {
                cancellation.cancel();
                log("Scan cancellation requested by user.");
            }
            return;
        }
        RepositoryRoot root = activeRoot;
        List<String> selected = new ArrayList<>(fileList.getSelectionModel().getSelectedItems());
        if (root == null || selected.isEmpty()) {
            return;
        }
        java.util.Set<String> enabled = new java.util.LinkedHashSet<>();
        for (java.util.Map.Entry<String, CheckBox> entry : adapterBoxes.entrySet()) {
            if (entry.getValue().isSelected()) {
                enabled.add(entry.getKey());
            }
        }
        if (enabled.isEmpty()) {
            verdictLabel.setText("No checks enabled: tick at least one adapter.");
            return;
        }
        LocalSchemaConfig bindings;
        try {
            List<String> entries = new ArrayList<>();
            for (String part : bindingsField.getText().split(",")) {
                String trimmed = part.trim();
                if (!trimmed.isEmpty()) {
                    entries.add(trimmed);
                }
            }
            bindings = LocalSchemaConfig.parse(entries, LocalSchemaConfig.DIALECT_2020_12,
                    root.root());
        } catch (IllegalArgumentException e) {
            verdictLabel.setText("Invalid schema bindings: " + e.getMessage());
            return;
        }
        String kubeVersion = kubeVersionField.getText().trim();
        if (kubeVersion.isEmpty()) {
            verdictLabel.setText("Invalid kubeconform version: empty.");
            return;
        }
        ScanConfig.KubeconformConfig kubeconform = new ScanConfig.KubeconformConfig(
                null, kubeVersion, kubeStrictBox.isSelected());
        ScanConfig scanConfig = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                kubeconform, bindings.isEmpty() ? null : bindings);
        scanning = true;
        updateScanButton();
        verdictLabel.setText("");
        reportsLabel.setText("");
        openReportsButton.setDisable(true);
        findingRows.clear();
        outcomeItems.clear();
        CancellationToken cancellation = new CancellationToken();
        scanCancellation = cancellation;
        ScanConfig configForRun = scanConfig;
        scanExecutor.submit(() -> {
            ScanSummary summary;
            try {
                ScanService guiScan = new ScanService(configForRun);
                summary = guiScan.runScan(root.root(), selected, enabled, "GUI",
                        ScanService.defaultDataDir(), cancellation, new ScanService.ScanListener() {
                            @Override
                            public void stage(String stage) {
                                Platform.runLater(
                                        () -> statusLabel.setText("status: scanning (" + stage + ")…"));
                            }

                            @Override
                            public void adapterStarted(String adapterId) {
                                log("Scan stage: running " + adapterId + "…");
                            }

                            @Override
                            public void adapterFinished(String adapterId, ScannerStatus status, int findingCount) {
                                log("Scan stage: " + adapterId + " finished (" + status.name().toLowerCase()
                                        + ", " + findingCount + " finding(s)).");
                            }
                        });
            } catch (ScanException e) {
                Platform.runLater(() -> {
                    scanning = false;
                    updateScanButton();
                    if (e.code() == ScanException.Code.REPOSITORY_BUSY) {
                        verdictLabel.setText("Repository is busy: another scan is already running.");
                        statusLabel.setText("status: repository busy");
                    } else {
                        verdictLabel.setText("Scan rejected (" + e.code() + "): " + e.getMessage());
                        statusLabel.setText("status: scan rejected");
                    }
                });
                log("Scan rejected (" + e.code() + "): " + e.getMessage());
                return;
            }
            ScanSummary finished = summary;
            Platform.runLater(() -> showScanSummary(finished));
        });
    }

    private void showScanSummary(ScanSummary summary) {
        scanning = false;
        updateScanButton();
        for (ScannerOutcome outcome : summary.scannerOutcomes()) {
            StringBuilder line = new StringBuilder(outcome.adapterId())
                    .append(": ").append(outcome.status().name().toLowerCase())
                    .append(" (").append(outcome.findings().size()).append(" finding(s))");
            if (outcome.toolVersion() != null) {
                line.append(" [").append(outcome.toolVersion()).append(']');
            }
            outcomeItems.add(line.toString());
            for (Finding finding : outcome.findings()) {
                findingRows.add(new FindingRow(finding));
            }
        }
        String verdictText;
        switch (summary.verdict()) {
            case FINDINGS -> verdictText = "Verdict: FINDINGS (" + summary.findingCount()
                    + ") — selected files only; not a repository-wide verdict.";
            case NO_FINDINGS -> verdictText = "Verdict: NO FINDINGS — applies only to "
                    + summary.targets().size() + " selected file(s) and "
                    + summary.scannerOutcomes().size() + " check(s).";
            default -> verdictText = "Verdict: INDETERMINATE (" + summary.state().name()
                    + ") — required coverage is incomplete. See events.";
        }
        verdictLabel.setText(verdictText);
        statusLabel.setText("status: scan " + summary.state().name().toLowerCase()
                + " (" + summary.findingCount() + " finding(s))");
        if (summary.jsonReportPath() != null || summary.markdownReportPath() != null) {
            StringBuilder reports = new StringBuilder("Reports: ");
            if (summary.jsonReportPath() != null) {
                reports.append(summary.jsonReportPath());
            }
            if (summary.markdownReportPath() != null) {
                reports.append("  |  ").append(summary.markdownReportPath());
            }
            reportsLabel.setText(reports.toString());
            Path dataDir = ScanService.defaultDataDir();
            lastArtifactsDir = dataDir.resolve("artifacts").resolve(summary.runId());
            openReportsButton.setDisable(false);
        }
        if (!summary.coverageGaps().isEmpty()) {
            log("Coverage gaps (" + summary.coverageGaps().size() + "):");
            for (String gap : summary.coverageGaps()) {
                log("gap: " + gap);
            }
        }
        log("Scan " + summary.runId() + " ended " + summary.state().name()
                + " with verdict " + summary.verdict().name()
                + " (" + summary.findingCount() + " finding(s)).");
    }

    private void openReportsFolder() {
        Path dir = lastArtifactsDir;
        if (dir == null) {
            return;
        }
        try {
            getHostServices().showDocument(dir.toUri().toString());
        } catch (Exception e) {
            log("Could not open reports folder: " + e.getMessage());
        }
    }

    private void log(String message) {
        String line = EventSanitizer.sanitize(message);
        LOG.warn("{}", line);
        if (Platform.isFxApplicationThread()) {
            appendLog(line);
        } else {
            Platform.runLater(() -> appendLog(line));
        }
    }

    private void appendLog(String line) {
        logItems.add(line);
        while (logItems.size() > MAX_LOG_ENTRIES) {
            logItems.remove(0);
        }
        logList.scrollTo(logItems.size() - 1);
    }

    private static Label styledLabel(String text) {
        Label label = new Label(text);
        label.setStyle("-fx-text-fill: #58a6ff; -fx-font-family: monospace; -fx-font-size: 12;");
        return label;
    }

    private static void styleButton(Button button) {
        button.setStyle("-fx-background-color: #21262d; -fx-text-fill: #c9d1d9; "
                + "-fx-border-color: #30363d; -fx-font-family: monospace;");
    }

    @Override
    public void stop() {
        cancelInFlight();
        CancellationToken cancellation = scanCancellation;
        if (cancellation != null) {
            cancellation.cancel();
        }
        discoveryExecutor.shutdownNow();
        scanExecutor.shutdownNow();
    }

    /** Row bean for the scanner catalog table. */
    public static final class ScannerRow {
        private final SimpleStringProperty tool;
        private final SimpleStringProperty purpose;
        private final SimpleStringProperty state;
        private final SimpleStringProperty availability;

        public ScannerRow(String tool, String purpose, String state, String availability) {
            this.tool = new SimpleStringProperty(tool);
            this.purpose = new SimpleStringProperty(purpose);
            this.state = new SimpleStringProperty(state);
            this.availability = new SimpleStringProperty(availability);
        }

        public String getTool() {
            return tool.get();
        }

        public String getPurpose() {
            return purpose.get();
        }

        public String getState() {
            return state.get();
        }

        public String getAvailability() {
            return availability.get();
        }

        public void setAvailability(String text) {
            availability.set(text);
        }
    }

    /** Row bean for the findings table. */
    public static final class FindingRow {
        private final SimpleStringProperty severity;
        private final SimpleStringProperty rule;
        private final SimpleStringProperty file;
        private final SimpleStringProperty line;
        private final SimpleStringProperty message;

        public FindingRow(Finding finding) {
            this.severity = new SimpleStringProperty(finding.severity().name());
            this.rule = new SimpleStringProperty(finding.ruleId());
            this.file = new SimpleStringProperty(
                    finding.relativePath() == null ? "-" : finding.relativePath());
            this.line = new SimpleStringProperty(
                    finding.lineStart() == null ? "-" : finding.lineStart().toString());
            this.message = new SimpleStringProperty(finding.message());
        }

        public String getSeverity() {
            return severity.get();
        }

        public String getRule() {
            return rule.get();
        }

        public String getFile() {
            return file.get();
        }

        public String getLine() {
            return line.get();
        }

        public String getMessage() {
            return message.get();
        }
    }
}

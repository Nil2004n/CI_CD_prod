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
import javafx.scene.control.ComboBox;
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
    private final java.util.concurrent.ScheduledExecutorService historyRefresher =
            Executors.newSingleThreadScheduledExecutor(r -> {
                Thread worker = new Thread(r, "pwt-history-refresh");
                worker.setDaemon(true);
                return worker;
            });
    private final AtomicLong generation = new AtomicLong(0);
    private volatile CancellationToken activeCancellation = CancellationToken.none();
    private volatile Future<?> activeTask;
    private volatile RepositoryRoot activeRoot;
    private volatile CancellationToken scanCancellation;
    private volatile boolean scanning;
    private volatile String displayedRunId;
    private volatile boolean displayedTerminal = true;

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
    private ListView<HistoryRow> historyList;
    private ObservableList<HistoryRow> historyItems;
    private Label analysisLabel;
    private ListView<String> analysisList;
    private ObservableList<String> analysisItems;
    private final java.util.Map<String, ScannerRow> rowByTool = new java.util.HashMap<>();
    private final java.util.Map<String, CheckBox> adapterBoxes = new java.util.LinkedHashMap<>();
    private TextField bindingsField;
    private TextField kubeVersionField;
    private CheckBox kubeStrictBox;
    private ComboBox<String> rootsComboBox;
    private Label presenceLabel;
    private ListView<String> presenceDetailsList;
    private ObservableList<String> presenceDetails;
    private Label dataDirLabel;
    private volatile int availableAdapterCount;
    private final java.util.Map<String, String> rootsByDisplay = new java.util.LinkedHashMap<>();
    private volatile boolean suppressRootEvents;

    @Override
    public void start(Stage stage) {
        stage.setTitle(BuildInfo.APP_NAME + " " + BuildInfo.VERSION + " (" + BuildInfo.PHASE + ")");

        // Presentation: build every existing control first (same nodes, same
        // listeners/handlers as before), then arrange them in the workbench shell.
        HBox toolbar = buildToolbar(stage);
        VBox filePane = buildFilePane();
        buildCatalogPane();
        VBox bottom = buildBottom();

        BorderPane root = new BorderPane();
        root.getStyleClass().add("app");
        root.setTop(toolbar);
        SplitPane center = new SplitPane(buildSidebar(filePane), buildWorkspace(), buildSidePanel());
        center.getStyleClass().add("main-split");
        center.setDividerPositions(0.24, 0.72);
        root.setCenter(center);
        root.setBottom(bottom);

        javafx.geometry.Rectangle2D screen = javafx.stage.Screen.getPrimary().getVisualBounds();
        Scene scene = new Scene(root, Math.min(1440, screen.getWidth()), Math.min(900, screen.getHeight()));
        java.net.URL theme = PwtDesktopApp.class.getResource("/pwt/desktop/pwt-theme.css");
        if (theme != null) {
            scene.getStylesheets().add(theme.toExternalForm());
        }
        stage.setMinWidth(980);
        stage.setMinHeight(620);
        stage.setScene(scene);
        log("PWT desktop started. Open a directory to discover YAML files, "
                + "or pick a registered root above. Shared data dir: "
                + ScanService.defaultDataDir());
        stage.show();
        LOG.warn("PWT desktop started");
        refreshAvailability();
        scanExecutor.submit(() -> ScanService.reconcileInterrupted(ScanService.defaultDataDir()));
        historyRefresher.scheduleWithFixedDelay(this::refreshHistory, 1, 1, java.util.concurrent.TimeUnit.SECONDS);
    }

    private HBox buildToolbar(Stage stage) {
        Label mark = new Label("PWT");
        mark.getStyleClass().add("brand-mark");
        Label title = new Label("YAML Workbench");
        title.getStyleClass().add("brand");
        Label phase = new Label(BuildInfo.VERSION);
        phase.getStyleClass().add("faint");

        rootField = new TextField();
        rootField.setPromptText("No directory open — use Open directory…");
        rootField.setEditable(false);

        Button openButton = new Button("Open directory…");
        openButton.setOnAction(event -> chooseAndDiscover(stage));

        rootsComboBox = new ComboBox<>();
        rootsComboBox.setPromptText("Registered roots…");
        rootsComboBox.setPrefWidth(240);
        rootsComboBox.getSelectionModel().selectedItemProperty().addListener((obs, old, current) -> {
            if (!suppressRootEvents && current != null) {
                openRegisteredRoot(current);
            }
        });

        cancelButton = new Button("Cancel discovery");
        cancelButton.setDisable(true);
        cancelButton.setOnAction(event -> cancelDiscovery());

        // scanButton is created in buildCatalogPane(); it is placed into this
        // toolbar by buildWorkspace() once it exists.
        toolbarSpacer = new javafx.scene.layout.Region();
        HBox.setHgrow(toolbarSpacer, Priority.ALWAYS);
        HBox toolbar = new HBox(8, mark, title, phase, separator(), rootsComboBox, openButton,
                cancelButton, toolbarSpacer);
        toolbar.getStyleClass().add("app-toolbar");
        this.toolbar = toolbar;
        return toolbar;
    }

    private HBox toolbar;
    private javafx.scene.layout.Region toolbarSpacer;
    private Label repoCountLabel;
    private Label findingsBadge;
    private Label outcomesBadge;

    private VBox buildFilePane() {
        Label heading = styledLabel("FILES");
        Label hint = new Label("mouse/keyboard multi-select");
        hint.getStyleClass().add("section-meta");
        fileList = new ListView<>();
        fileList.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);
        fileList.getStyleClass().add("mono-small");
        fileList.setPlaceholder(placeholder("Open a directory or pick a registered root."));
        fileList.getSelectionModel().selectedItemProperty().addListener((obs, old, current) -> updateCounts());
        VBox.setVgrow(fileList, Priority.ALWAYS);

        countsLabel = styledLabel("discovered: 0   selected: 0");
        countsLabel.getStyleClass().setAll("label", "value");
        stateLabel = styledLabel("");
        stateLabel.getStyleClass().setAll("label", "muted");
        stateLabel.setWrapText(true);
        incompleteLabel = styledLabel("");
        incompleteLabel.getStyleClass().setAll("label", "warning-text");
        incompleteLabel.setWrapText(true);
        for (Label label : List.of(stateLabel, incompleteLabel)) {
            label.managedProperty().bind(label.textProperty().isNotEmpty());
            label.visibleProperty().bind(label.textProperty().isNotEmpty());
        }

        Button selectAll = new Button("Select all");
        Button selectNone = new Button("Select none");
        selectNone.getStyleClass().add("link");
        selectAll.setOnAction(event -> fileList.getSelectionModel().selectAll());
        selectNone.setOnAction(event -> fileList.getSelectionModel().clearSelection());
        HBox selectionBar = new HBox(8, selectAll, selectNone);
        selectionBar.setAlignment(javafx.geometry.Pos.CENTER_LEFT);

        VBox pane = new VBox(6, row(heading, hint), fileList, countsLabel, stateLabel, incompleteLabel,
                selectionBar);
        VBox.setVgrow(pane, Priority.ALWAYS);
        return pane;
    }

    /** Left column: repository header, open root, discovered files, run history. */
    private VBox buildSidebar(VBox filePane) {
        Label heading = styledLabel("REPOSITORIES");
        repoCountLabel = new Label("");
        repoCountLabel.getStyleClass().add("section-meta");
        rootField.setMaxWidth(Double.MAX_VALUE);
        VBox sidebar = new VBox(6, row(heading, repoCountLabel), rootField, filePane,
                historyHeading, historyList);
        sidebar.getStyleClass().add("sidebar");
        sidebar.setMinWidth(240);
        return sidebar;
    }

    private Label historyHeading;

    private void buildCatalogPane() {
        Label heading = styledLabel("SCANNERS");
        Label headingMeta = new Label("availability probed locally");
        headingMeta.getStyleClass().add("section-meta");
        TableView<ScannerRow> table = new TableView<>();
        table.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        table.setPlaceholder(new Label(""));

        TableColumn<ScannerRow, String> toolColumn = new TableColumn<>("Tool");
        toolColumn.setCellValueFactory(new PropertyValueFactory<>("tool"));
        TableColumn<ScannerRow, String> purposeColumn = new TableColumn<>("Intended purpose");
        purposeColumn.setCellValueFactory(new PropertyValueFactory<>("purpose"));
        TableColumn<ScannerRow, String> stateColumn = new TableColumn<>("Integration");
        stateColumn.setCellValueFactory(new PropertyValueFactory<>("state"));
        TableColumn<ScannerRow, String> availabilityColumn = new TableColumn<>("Availability");
        availabilityColumn.setCellValueFactory(new PropertyValueFactory<>("availability"));
        availabilityColumn.setCellFactory(column -> new StatusTextCell<>());
        table.getColumns().addAll(toolColumn, purposeColumn, stateColumn, availabilityColumn);
        toolColumn.setPrefWidth(120);
        purposeColumn.setPrefWidth(260);
        stateColumn.setPrefWidth(110);
        availabilityColumn.setPrefWidth(240);

        ObservableList<ScannerRow> rows = FXCollections.observableArrayList();
        for (ScannerDescriptor descriptor : ScannerCatalog.planned()) {
            ScannerRow row = new ScannerRow(descriptor.toolName(), descriptor.purpose(),
                    descriptor.execution().name(), "probing…");
            rows.add(row);
            rowByTool.put(descriptor.id(), row);
        }
        table.setItems(rows);
        table.setPrefHeight(230);
        table.setMinHeight(160);

        scanButton = new Button("Run scan (selected files)");
        scanButton.getStyleClass().add("primary");
        scanButton.setDisable(true);
        scanButton.setOnAction(event -> onScanButton());

        Label checksHeading = styledLabel("ENABLED CHECKS");
        javafx.scene.layout.FlowPane checksBar = new javafx.scene.layout.FlowPane(14, 6);
        for (String adapterId : ScanService.ADAPTER_ORDER) {
            CheckBox box = new CheckBox(adapterId);
            box.setSelected(true);
            box.setOnAction(event -> updateScanButton());
            adapterBoxes.put(adapterId, box);
            checksBar.getChildren().add(box);
        }

        Label bindingsHeading = styledLabel("LOCAL SCHEMA BINDINGS");
        Label bindingsMeta = new Label("glob=path, comma-separated; empty = not used");
        bindingsMeta.getStyleClass().add("section-meta");
        bindingsField = new TextField();
        bindingsField.setPromptText("e.g. widgets/*.yaml=/abs/path/schema.json");
        HBox.setHgrow(bindingsField, Priority.ALWAYS);

        Label kubeHeading = styledLabel("KUBECONFORM");
        Label kubeMeta = new Label("local schemas only; setup: pwt kubeconform-setup");
        kubeMeta.getStyleClass().add("section-meta");
        kubeVersionField = new TextField("1.29.0");
        kubeVersionField.setPrefColumnCount(8);
        kubeStrictBox = new CheckBox("strict");
        kubeStrictBox.setSelected(true);
        Label kubeVersionKey = new Label("Kubernetes version");
        kubeVersionKey.getStyleClass().add("muted");
        HBox kubeBar = new HBox(10, kubeVersionKey, kubeVersionField, kubeStrictBox);
        kubeBar.setAlignment(javafx.geometry.Pos.CENTER_LEFT);

        verdictLabel = styledLabel("");
        verdictLabel.getStyleClass().setAll("label", "verdict");
        verdictLabel.setWrapText(true);
        reportsLabel = styledLabel("");
        reportsLabel.getStyleClass().setAll("label", "muted");
        reportsLabel.setWrapText(true);
        openReportsButton = new Button("Open reports folder");
        openReportsButton.setDisable(true);
        openReportsButton.setOnAction(event -> openReportsFolder());
        Button viewReportButton = new Button("View report");
        viewReportButton.setOnAction(event -> viewReport());
        Button exportReportButton = new Button("Export report");
        exportReportButton.setOnAction(event -> exportReport());
        HBox reportsBar = new HBox(8, viewReportButton, exportReportButton, openReportsButton);
        reportsBar.setAlignment(javafx.geometry.Pos.CENTER_LEFT);

        Label findingsHeading = styledLabel("FINDINGS");
        Label findingsMeta = new Label("selected scope only");
        findingsMeta.getStyleClass().add("section-meta");
        findingsTable = new TableView<>();
        findingsTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        findingsTable.setPlaceholder(placeholder("No scan yet — findings appear here."));
        TableColumn<FindingRow, String> severityColumn = new TableColumn<>("Severity");
        severityColumn.setCellValueFactory(new PropertyValueFactory<>("severity"));
        severityColumn.setCellFactory(column -> new SeverityCell());
        TableColumn<FindingRow, String> ruleColumn = new TableColumn<>("Rule");
        ruleColumn.setCellValueFactory(new PropertyValueFactory<>("rule"));
        TableColumn<FindingRow, String> fileColumn = new TableColumn<>("File");
        fileColumn.setCellValueFactory(new PropertyValueFactory<>("file"));
        fileColumn.setCellFactory(column -> new TooltipCell<>());
        TableColumn<FindingRow, String> lineColumn = new TableColumn<>("Line");
        lineColumn.setCellValueFactory(new PropertyValueFactory<>("line"));
        lineColumn.getStyleClass().add("num");
        TableColumn<FindingRow, String> messageColumn = new TableColumn<>("Message");
        messageColumn.setCellValueFactory(new PropertyValueFactory<>("message"));
        messageColumn.setCellFactory(column -> new TooltipCell<>());
        findingsTable.getColumns().addAll(severityColumn, ruleColumn, fileColumn, lineColumn, messageColumn);
        severityColumn.setPrefWidth(124);
        severityColumn.setMinWidth(100);
        ruleColumn.setPrefWidth(190);
        fileColumn.setPrefWidth(220);
        lineColumn.setPrefWidth(56);
        lineColumn.setMaxWidth(80);
        messageColumn.setPrefWidth(300);
        findingRows = FXCollections.observableArrayList();
        findingsTable.setItems(findingRows);
        findingsTable.setPrefHeight(150);

        Label outcomesHeading = styledLabel("SCANNER OUTCOMES");
        Label outcomesMeta = new Label("all configured checks for the displayed run");
        outcomesMeta.getStyleClass().add("section-meta");
        outcomeItems = FXCollections.observableArrayList();
        outcomesList = new ListView<>(outcomeItems);
        outcomesList.setPrefHeight(90);
        outcomesList.setPlaceholder(placeholder("No run displayed — scanner outcomes appear here."));
        outcomesList.setCellFactory(list -> new StatusListCell());

        presenceLabel = styledLabel("MCP: No active clients.");
        presenceLabel.getStyleClass().setAll("label", "value");
        presenceLabel.setWrapText(true);
        presenceDetails = FXCollections.observableArrayList();
        ListView<String> presenceDetailsList = new ListView<>(presenceDetails);
        presenceDetailsList.setPrefHeight(104);
        presenceDetailsList.getStyleClass().add("mono-small");
        presenceDetailsList.setPlaceholder(placeholder("No live sessions."));
        presenceDetailsList.setCellFactory(list -> new WrappingCell());
        dataDirLabel = styledLabel("Shared data: " + ScanService.defaultDataDir().toString());
        dataDirLabel.getStyleClass().setAll("label", "muted");
        dataDirLabel.setWrapText(true);
        Label dataDirHint = styledLabel("MCP must use the same --data-dir to appear here.");
        dataDirHint.getStyleClass().setAll("label", "faint");
        dataDirHint.setWrapText(true);

        // Keep references for the shell layout (presentation only).
        this.catalogTable = table;
        this.checksHeadingNode = row(checksHeading);
        this.checksBarNode = checksBar;
        this.bindingsNode = new VBox(4, row(bindingsHeading, bindingsMeta), bindingsField);
        this.kubeNode = new VBox(4, row(kubeHeading, kubeMeta), kubeBar);
        this.catalogHeadingNode = row(heading, headingMeta);
        this.reportsBarNode = reportsBar;
        this.findingsHeadingNode = row(findingsHeading, findingsMeta);
        this.outcomesHeadingNode = row(outcomesHeading, outcomesMeta);
        this.presenceDetailsNode = presenceDetailsList;
        this.dataDirHintNode = dataDirHint;
    }

    private TableView<ScannerRow> catalogTable;
    private javafx.scene.Node checksHeadingNode;
    private javafx.scene.Node checksBarNode;
    private javafx.scene.Node bindingsNode;
    private javafx.scene.Node kubeNode;
    private javafx.scene.Node catalogHeadingNode;
    private javafx.scene.Node reportsBarNode;
    private javafx.scene.Node findingsHeadingNode;
    private javafx.scene.Node outcomesHeadingNode;
    private javafx.scene.Node presenceDetailsNode;
    private javafx.scene.Node dataDirHintNode;

    /** Center column: Progress / Findings / Coverage / Activity tabs over existing controls. */
    private javafx.scene.control.TabPane buildWorkspace() {
        toolbar.getChildren().add(scanButton);

        // Progress: displayed-run verdict + scan configuration + scanner catalog.
        Label runHeading = styledLabel("RUN");
        VBox runCard = new VBox(6, runHeading, verdictLabel);
        runCard.getStyleClass().add("card");
        VBox configCard = new VBox(8, checksHeadingNode, checksBarNode, bindingsNode, kubeNode);
        configCard.getStyleClass().add("card");
        VBox.setVgrow(catalogTable, Priority.ALWAYS);
        VBox progressBody = new VBox(10, runCard, styledLabel("SCAN CONFIGURATION"), configCard,
                catalogHeadingNode, catalogTable);
        progressBody.getStyleClass().add("tab-body");
        javafx.scene.control.ScrollPane progressScroll = new javafx.scene.control.ScrollPane(progressBody);
        progressScroll.setFitToWidth(true);
        progressScroll.setFitToHeight(true);

        // Findings: table + report actions.
        VBox.setVgrow(findingsTable, Priority.ALWAYS);
        VBox findingsBody = new VBox(8, findingsHeadingNode, findingsTable, reportsBarNode, reportsLabel);
        findingsBody.getStyleClass().add("tab-body");

        // Coverage: per-check outcomes of the displayed run.
        VBox.setVgrow(outcomesList, Priority.ALWAYS);
        VBox coverageBody = new VBox(8, outcomesHeadingNode, outcomesList);
        coverageBody.getStyleClass().add("tab-body");

        // Activity: sanitized application events.
        VBox.setVgrow(logList, Priority.ALWAYS);
        Label activityMeta = new Label("sanitized · newest last");
        activityMeta.getStyleClass().add("section-meta");
        VBox activityBody = new VBox(8, row(styledLabel("APPLICATION EVENTS"), activityMeta), logList);
        activityBody.getStyleClass().add("tab-body");

        findingsBadge = badge(findingRows);
        outcomesBadge = badge(outcomeItems);
        javafx.scene.control.TabPane tabs = new javafx.scene.control.TabPane(
                tab("Progress", null, progressScroll),
                tab("Findings", findingsBadge, findingsBody),
                tab("Coverage", outcomesBadge, coverageBody),
                tab("Activity", null, activityBody));
        tabs.setTabClosingPolicy(javafx.scene.control.TabPane.TabClosingPolicy.UNAVAILABLE);
        tabs.getStyleClass().add("workspace");
        tabs.setMinWidth(480);
        return tabs;
    }

    /** Right column: saved agent analysis (read-only) and MCP session presence. */
    private VBox buildSidePanel() {
        Label meta = new Label("read-only · MCP writes only");
        meta.getStyleClass().add("section-meta");
        VBox.setVgrow(analysisList, Priority.ALWAYS);
        Label mcpHeading = styledLabel("MCP SESSIONS");
        VBox panel = new VBox(6, row(styledLabel("AGENT ANALYSIS"), meta), analysisLabel, analysisList,
                mcpHeading, presenceLabel, presenceDetailsNode, dataDirLabel, dataDirHintNode);
        panel.getStyleClass().add("side-panel");
        panel.setMinWidth(260);
        return panel;
    }

    private VBox buildBottom() {
        logItems = FXCollections.observableArrayList();
        logList = new ListView<>(logItems);
        logList.setPrefHeight(130);
        logList.getStyleClass().add("log");
        logList.setCellFactory(list -> new WrappingCell());
        statusLabel = styledLabel("status: idle");
        statusLabel.getStyleClass().setAll("label", "status-value");

        historyHeading = styledLabel("RUN HISTORY");
        historyItems = FXCollections.observableArrayList();
        historyList = new ListView<>(historyItems);
        historyList.setPrefHeight(150);
        historyList.setMinHeight(90);
        historyList.getStyleClass().add("mono-small");
        historyList.setPlaceholder(placeholder("No runs for this root yet."));
        historyList.setCellFactory(list -> new HistoryCell());
        historyList.getSelectionModel().selectedItemProperty().addListener((obs, old, current) -> {
            if (current != null) {
                loadHistoryRun(current.runId());
            }
        });

        analysisLabel = styledLabel("No saved analysis for the selected run.");
        analysisLabel.getStyleClass().setAll("label", "value");
        analysisLabel.setWrapText(true);
        analysisItems = FXCollections.observableArrayList();
        analysisList = new ListView<>(analysisItems);
        analysisList.setPrefHeight(76);
        analysisList.getStyleClass().add("mono-small");
        analysisList.setPlaceholder(placeholder("No saved assessments."));
        analysisList.setCellFactory(list -> new WrappingCell());

        Label appKey = new Label("STATUS");
        appKey.getStyleClass().add("status-key");
        javafx.scene.layout.Region spacer = new javafx.scene.layout.Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);
        Label versionKey = new Label("PWT");
        versionKey.getStyleClass().add("status-key");
        Label version = new Label(BuildInfo.VERSION);
        version.getStyleClass().add("status-value");
        HBox statusBar = new HBox(8, appKey, statusLabel, spacer, versionKey, version);
        statusBar.getStyleClass().add("status-bar");
        VBox bottom = new VBox(statusBar);
        return bottom;
    }

    // ---------- presentation helpers (no behavior) ----------

    private static HBox row(javafx.scene.Node... nodes) {
        if (nodes.length > 0 && nodes[0] instanceof javafx.scene.layout.Region first) {
            first.setMinWidth(javafx.scene.layout.Region.USE_PREF_SIZE);
        }
        HBox box = new HBox(8, nodes);
        box.setAlignment(javafx.geometry.Pos.BASELINE_LEFT);
        return box;
    }

    private static javafx.scene.layout.Region separator() {
        javafx.scene.layout.Region separator = new javafx.scene.layout.Region();
        separator.getStyleClass().add("toolbar-separator");
        return separator;
    }

    private static Label placeholder(String text) {
        Label label = new Label(text);
        label.getStyleClass().add("faint");
        return label;
    }

    private static Label badge(ObservableList<?> items) {
        Label badge = new Label();
        badge.getStyleClass().add("badge");
        badge.textProperty().bind(javafx.beans.binding.Bindings.size(items).asString());
        return badge;
    }

    private static javafx.scene.control.Tab tab(String title, Label badge, javafx.scene.Node content) {
        javafx.scene.control.Tab tab = new javafx.scene.control.Tab();
        Label titleLabel = new Label(title);
        titleLabel.getStyleClass().add("tab-title");
        HBox header = badge == null ? new HBox(titleLabel) : new HBox(6, titleLabel, badge);
        header.setAlignment(javafx.geometry.Pos.CENTER_LEFT);
        tab.setGraphic(header);
        tab.setContent(content);
        return tab;
    }

    /** Maps existing status/severity words to a style class; text is unchanged. */
    static String statusClass(String text) {
        if (text == null) {
            return "state-neutral";
        }
        String t = text.toLowerCase(java.util.Locale.ROOT);
        if (t.contains("failed") || t.contains("timed_out") || t.contains("timed out")
                || t.contains("interrupted") || t.contains("error")) {
            return "state-error";
        }
        if (t.contains("unavailable") || t.contains("partial") || t.contains("cancelled")
                || t.contains("skipped") || t.contains("indeterminate") || t.contains("not_applicable")
                || t.contains("incomplete") || t.contains("stale")) {
            return "state-warn";
        }
        if (t.contains("running") || t.contains("pending") || t.contains("probing")) {
            return "state-info";
        }
        if (t.contains("succeeded") || t.contains("completed") || t.contains("available")
                || t.contains("no_findings")) {
            return "state-ok";
        }
        return "state-neutral";
    }

    private static final List<String> STATE_CLASSES =
            List.of("state-ok", "state-warn", "state-error", "state-info", "state-neutral");

    private static void applyStateClass(javafx.scene.control.Labeled cell, String text) {
        cell.getStyleClass().removeAll(STATE_CLASSES);
        if (text != null) {
            cell.getStyleClass().add(statusClass(text));
        }
    }

    /** Table cell coloring availability text by its existing wording. */
    private static final class StatusTextCell<S> extends javafx.scene.control.TableCell<S, String> {
        @Override
        protected void updateItem(String item, boolean empty) {
            super.updateItem(item, empty);
            setText(empty ? null : item);
            setTooltip(empty || item == null ? null : new javafx.scene.control.Tooltip(item));
            applyStateClass(this, empty ? null : item);
        }
    }

    /** Table cell that keeps long values reachable through a tooltip. */
    private static final class TooltipCell<S> extends javafx.scene.control.TableCell<S, String> {
        @Override
        protected void updateItem(String item, boolean empty) {
            super.updateItem(item, empty);
            setText(empty ? null : item);
            setTooltip(empty || item == null ? null : new javafx.scene.control.Tooltip(item));
        }
    }

    /** Severity text is unchanged; a glyph and color reinforce it. */
    private static final class SeverityCell extends javafx.scene.control.TableCell<FindingRow, String> {
        private static final List<String> SEV_CLASSES =
                List.of("sev-critical", "sev-high", "sev-medium", "sev-low", "sev-info", "sev-unknown");

        @Override
        protected void updateItem(String item, boolean empty) {
            super.updateItem(item, empty);
            getStyleClass().removeAll(SEV_CLASSES);
            if (empty || item == null) {
                setText(null);
                return;
            }
            String glyph = switch (item) {
                case "CRITICAL" -> "◆ ";
                case "HIGH" -> "▲ ";
                case "MEDIUM" -> "■ ";
                case "LOW" -> "● ";
                default -> "○ ";
            };
            setText(glyph + item);
            getStyleClass().add("sev-" + item.toLowerCase(java.util.Locale.ROOT));
        }
    }

    /** Outcome lines keep their text; color follows the status word they already contain. */
    private static final class StatusListCell extends javafx.scene.control.ListCell<String> {
        @Override
        protected void updateItem(String item, boolean empty) {
            super.updateItem(item, empty);
            setText(empty ? null : item);
            applyStateClass(this, empty ? null : item);
        }
    }

    /** Run history rows keep HistoryRow.toString(); color follows the run state. */
    private static final class HistoryCell extends javafx.scene.control.ListCell<HistoryRow> {
        @Override
        protected void updateItem(HistoryRow item, boolean empty) {
            super.updateItem(item, empty);
            String text = empty || item == null ? null : item.toString();
            setText(text);
            applyStateClass(this, text == null ? null : text.split(" ", 2)[0]);
        }
    }

    /** Wrapping text cell for longer analysis lines. */
    private static final class WrappingCell extends javafx.scene.control.ListCell<String> {
        WrappingCell() {
            setWrapText(true);
            setPrefWidth(0);
        }

        @Override
        protected void updateItem(String item, boolean empty) {
            super.updateItem(item, empty);
            setText(empty ? null : item);
        }
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

    private static boolean isTerminalState(String state) {
        return switch (state) {
            case "COMPLETED", "PARTIAL", "FAILED", "CANCELLED", "INTERRUPTED" -> true;
            default -> false;
        };
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
        scanButton.getStyleClass().removeAll("primary", "danger");
        scanButton.getStyleClass().add(scanning ? "danger" : "primary");
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
        displayedRunId = summary.runId();
        displayedTerminal = isTerminalState(summary.state().name());
        findingRows.clear();
        outcomeItems.clear();
        analysisItems.clear();
        analysisLabel.setText("No saved analysis for the selected run.");
        loadAnalysis(summary.runId());
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
        } else {
            reportsLabel.setText("Reports: in database (not exported)");
            lastArtifactsDir = null;
            openReportsButton.setDisable(true);
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

    /** Renders the displayed run's Markdown report from the database (no file needed). */
    private void viewReport() {
        String runId = displayedRunId;
        if (runId == null) {
            return;
        }
        scanExecutor.submit(() -> {
            String markdown;
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(ScanService.defaultDataDir())) {
                markdown = new pwt.application.ReportService().render(store, runId).markdown();
            } catch (Exception e) {
                String message = "Could not render report: " + e.getMessage();
                Platform.runLater(() -> log(message));
                return;
            }
            String text = markdown;
            Platform.runLater(() -> {
                javafx.scene.control.TextArea area = new javafx.scene.control.TextArea(text);
                area.setEditable(false);
                area.setWrapText(true);
                area.setFont(Font.font("Monospace", 12));
                area.setPrefSize(760, 520);
                javafx.scene.Scene scene = new javafx.scene.Scene(new javafx.scene.layout.StackPane(area));
                Stage viewer = new Stage();
                viewer.setTitle("PWT report — " + runId);
                viewer.setScene(scene);
                viewer.show();
            });
        });
    }

    /** Explicit export: writes JSON + Markdown files and records the artifacts. */
    private void exportReport() {
        String runId = displayedRunId;
        if (runId == null) {
            return;
        }
        scanExecutor.submit(() -> {
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(ScanService.defaultDataDir())) {
                pwt.application.ReportService.ReportPaths paths =
                        new pwt.application.ReportService().generate(store, ScanService.defaultDataDir(), runId);
                Platform.runLater(() -> {
                    StringBuilder reports = new StringBuilder("Reports: ");
                    reports.append(paths.jsonPath() == null ? "(json failed)" : paths.jsonPath());
                    reports.append("  |  ");
                    reports.append(paths.markdownPath() == null ? "(markdown failed)" : paths.markdownPath());
                    reportsLabel.setText(reports.toString());
                    lastArtifactsDir = ScanService.defaultDataDir().resolve("artifacts").resolve(runId);
                    openReportsButton.setDisable(paths.jsonPath() == null && paths.markdownPath() == null);
                    log("Exported report for run " + runId);
                });
            } catch (Exception e) {
                Platform.runLater(() -> log("Export failed: " + e.getMessage()));
            }
        });
    }

    /**
     * Shared-store refresh, roughly once per second. Reloads the recent-run
     * list for the active root and, while the displayed run is non-terminal,
     * its summary — so MCP/CLI-created runs and background progress become
     * visible here. Store work stays off the FX thread; the list only changes
     * when run IDs change (no selection reset churn).
     */
    private void openRegisteredRoot(String displayName) {
        String canonical = rootsByDisplay.get(displayName);
        if (canonical == null) {
            return;
        }
        log("Opening registered root: " + canonical);
        startDiscovery(java.nio.file.Path.of(canonical));
    }

    private void refreshHistory() {
        RepositoryRoot root = activeRoot;
        String displayed = displayedRunId;
        boolean terminal = displayedTerminal;
        try (pwt.infrastructure.SqliteStore store =
                pwt.infrastructure.SqliteStore.open(ScanService.defaultDataDir())) {
            final java.util.List<HistoryRow> rows = new java.util.ArrayList<>();
            if (root != null) {
                pwt.infrastructure.SqliteStore.RepositoryRow repo = store.registerRepository(
                        root.root().toString(), root.displayName(), root.identityHash());
                for (pwt.infrastructure.SqliteStore.RunView view :
                        store.listRuns(repo.id(), null, 20, 0)) {
                    rows.add(new HistoryRow(view.id(), view.state().name(), view.verdict().name(),
                            store.countFindings(view.id()), view.createdAt()));
                }
            }
            // Registered roots for the picker (shared store; MCP/CLI-created).
            final java.util.Map<String, String> freshRoots = new java.util.LinkedHashMap<>();
            for (pwt.infrastructure.SqliteStore.RepositoryRow row : store.listRepositories()) {
                boolean available = java.nio.file.Files.isDirectory(java.nio.file.Path.of(row.canonicalRoot()));
                String label = row.displayName() + (available ? "" : " (unavailable)");
                if (!freshRoots.containsKey(label)) {
                    freshRoots.put(label, row.canonicalRoot());
                } else {
                    freshRoots.put(label + " — " + row.canonicalRoot(), row.canonicalRoot());
                }
            }
            // Live MCP presence for the same data directory.
            final java.util.List<pwt.mcp.McpPresence.SessionView> presence =
                    pwt.mcp.McpPresence.summarize(store.listLiveMcpSessions(), java.time.Instant.now());
            final ScanSummary reload;
            if (displayed != null && !terminal) {
                ScanSummary loaded;
                try {
                    loaded = ScanService.loadSummary(store, displayed);
                } catch (Exception e) {
                    loaded = null;
                }
                reload = loaded;
            } else {
                reload = null;
            }
            Platform.runLater(() -> {
                java.util.List<String> currentIds = new java.util.ArrayList<>();
                for (HistoryRow row : historyItems) {
                    currentIds.add(row.runId());
                }
                java.util.List<String> freshIds = new java.util.ArrayList<>();
                for (HistoryRow row : rows) {
                    freshIds.add(row.runId());
                }
                if (!currentIds.equals(freshIds)) {
                    historyItems.setAll(rows);
                }
                if (!rootsByDisplay.equals(freshRoots)) {
                    suppressRootEvents = true;
                    try {
                        rootsByDisplay.clear();
                        rootsByDisplay.putAll(freshRoots);
                        String keep = rootsComboBox.getValue();
                        rootsComboBox.getItems().setAll(freshRoots.keySet());
                        if (keep != null && freshRoots.containsKey(keep)) {
                            rootsComboBox.setValue(keep);
                        }
                    } finally {
                        suppressRootEvents = false;
                    }
                }
                repoCountLabel.setText(freshRoots.size() + " registered");
                presenceLabel.setText(pwt.mcp.McpPresence.statusLine(presence));
                java.util.List<String> details = new java.util.ArrayList<>();
                for (pwt.mcp.McpPresence.SessionView view : presence) {
                    details.add(view.clientLabel() + " · " + view.permissionMode()
                            + (view.analysisAllowed() ? " + analysis" : "")
                            + " · " + view.presence().name() + " · heartbeat "
                            + view.heartbeatAge());
                }
                if (!presenceDetails.equals(details)) {
                    presenceDetails.setAll(details);
                }
                if (reload != null) {
                    showScanSummary(reload);
                }
            });
        } catch (Exception e) {
            // Data dir may not exist yet; history simply stays empty.
        }
    }

    private void loadHistoryRun(String runId) {
        scanExecutor.submit(() -> {
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(ScanService.defaultDataDir())) {
                ScanSummary summary = ScanService.loadSummary(store, runId);
                if (summary != null) {
                    Platform.runLater(() -> showScanSummary(summary));
                    log("Loaded persisted run " + runId + " from the shared store.");
                }
            } catch (Exception e) {
                log("Could not load run " + runId + ": " + e.getMessage());
            }
        });
    }

    /** Loads the saved agent analysis for the displayed run, if any. */
    private void loadAnalysis(String runId) {
        scanExecutor.submit(() -> {
            try (pwt.infrastructure.SqliteStore store =
                    pwt.infrastructure.SqliteStore.open(ScanService.defaultDataDir())) {
                pwt.infrastructure.SqliteStore.RunView view = store.getRun(runId);
                if (view == null) {
                    return;
                }
                pwt.infrastructure.SqliteStore.AnalysisThreadView thread =
                        store.findAnalysisThread(view.repositoryId(), runId);
                final String headline;
                final java.util.List<String> rows = new java.util.ArrayList<>();
                if (thread == null || thread.currentRevision() < 1) {
                    headline = "No saved analysis for the selected run.";
                } else {
                    pwt.infrastructure.SqliteStore.AnalysisRevisionRow revision =
                            store.getAnalysisRevision(thread.threadId(), -1);
                    String summary = revision == null ? "" : revision.summary();
                    headline = "Analysis r" + thread.currentRevision() + " [" + revision.priority() + "] "
                            + (summary.length() > 160 ? summary.substring(0, 160) + "…" : summary);
                    if (revision != null) {
                        for (pwt.infrastructure.SqliteStore.AnalysisAssessmentRow assessment :
                                store.listAnalysisAssessments(revision.id())) {
                            rows.add("[" + assessment.priority() + "] "
                                    + (assessment.ruleId() == null ? "" : assessment.ruleId() + " ")
                                    + (assessment.relativePath() == null ? "" : assessment.relativePath()
                                            + (assessment.lineStart() == null ? "" : ":" + assessment.lineStart()))
                                    + " — " + assessment.message());
                        }
                    }
                    try {
                        java.util.List<pwt.infrastructure.SqliteStore.RunView> latest =
                                store.listRuns(view.repositoryId(), null, 1, 0);
                        if (!latest.isEmpty() && !latest.get(0).id().equals(runId)) {
                            rows.add(0, "STALE: a newer scan exists (" + latest.get(0).id().substring(0, 8)
                                    + ", " + latest.get(0).state().name() + "). Re-verify before trusting this.");
                        }
                    } catch (Exception ignored) {
                        // staleness check is advisory
                    }
                }
                final String finalHeadline = headline;
                Platform.runLater(() -> {
                    if (!runId.equals(displayedRunId)) {
                        return;
                    }
                    analysisLabel.setText(finalHeadline);
                    analysisItems.setAll(rows);
                });
            } catch (Exception e) {
                log("Could not load analysis: " + e.getMessage());
            }
        });
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
        label.getStyleClass().add("section-heading");
        return label;
    }

    @Override
    public void stop() {
        historyRefresher.shutdownNow();
        cancelInFlight();
        CancellationToken cancellation = scanCancellation;
        if (cancellation != null) {
            cancellation.cancel();
        }
        discoveryExecutor.shutdownNow();
        scanExecutor.shutdownNow();
    }

    /** Row bean for the shared-store run history list. */
    public static final class HistoryRow {
        private final String runId;
        private final String state;
        private final String verdict;
        private final int findings;
        private final String createdAt;

        public HistoryRow(String runId, String state, String verdict, int findings, String createdAt) {
            this.runId = runId;
            this.state = state;
            this.verdict = verdict;
            this.findings = findings;
            this.createdAt = createdAt;
        }

        public String runId() {
            return runId;
        }

        @Override
        public String toString() {
            String shortId = runId.length() > 8 ? runId.substring(0, 8) : runId;
            return state + " " + verdict + " " + shortId + " (" + findings + " findings)";
        }
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

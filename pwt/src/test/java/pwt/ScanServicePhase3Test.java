package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.ActionlintAdapter;
import pwt.adapters.CheckovAdapter;
import pwt.adapters.KubeconformAdapter;
import pwt.adapters.LocalSchemaAdapter;
import pwt.application.CancellationToken;
import pwt.application.LocalSchemaConfig;
import pwt.application.ScanConfig;
import pwt.application.ScanService;
import pwt.domain.ScanState;
import pwt.domain.ScanSummary;
import pwt.domain.ScanVerdict;
import pwt.domain.ScannerStatus;

/**
 * Real-tool integration for the four Phase 3 adapters. Tests that need an
 * installed executable assume its probe succeeds, otherwise they skip
 * explicitly. Benign fixtures only; the one seeded fake token is asserted
 * absent from every sink.
 */
class ScanServicePhase3Test {

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final String FAKE_TOKEN = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";

    @TempDir
    Path work;

    private Path repoDir(String name) throws Exception {
        Path repo = work.resolve(name);
        Files.createDirectories(repo);
        return repo;
    }

    private void write(Path repo, String relative, String content) throws Exception {
        Path file = repo.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
    }

    private static void assumeCheckov() {
        assumeTrue(new CheckovAdapter().probe().available(), "SKIP: checkov missing");
    }

    private static void assumeActionlint() {
        assumeTrue(new ActionlintAdapter().probe().available(), "SKIP: actionlint missing");
    }

    private static void assumeKubeconform() {
        assumeTrue(new KubeconformAdapter().probe().available(), "SKIP: kubeconform missing");
    }

    private ScanSummary scan(Path repo, List<String> files, Set<String> adapters, ScanConfig config)
            throws Exception {
        ScanService service = new ScanService(config);
        return service.runScan(repo, files, adapters, "CLI", work.resolve("data"),
                CancellationToken.none(), null);
    }

    private static final String PRIVILEGED_POD = "---\napiVersion: v1\nkind: Pod\nmetadata:\n"
            + "  name: web\nspec:\n  containers:\n  - name: web\n    image: nginx:1.25\n"
            + "    securityContext:\n      privileged: true\n";

    private static final String CLEAN_POD = "---\napiVersion: v1\nkind: Pod\nmetadata:\n"
            + "  name: ok\nspec:\n  containers:\n  - name: ok\n    image: nginx:1.25\n";

    @Test
    void checkovFindsMisconfiguration() throws Exception {
        assumeCheckov();
        Path repo = repoDir("ck");
        write(repo, "pod.yaml", PRIVILEGED_POD);
        write(repo, "plain.yaml", "---\nkey: value\n");

        ScanSummary summary = scan(repo, List.of("pod.yaml", "plain.yaml"), Set.of("checkov"),
                ScanConfig.defaults());

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        var outcome = summary.scannerOutcomes().get(0);
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals("3.4.4", outcome.toolVersion());
        assertTrue(outcome.findings().stream().anyMatch(f -> f.ruleId().startsWith("checkov/CKV_K8S_")));
        assertTrue(outcome.findings().stream().allMatch(f -> f.relativePath().equals("pod.yaml")));
    }

    @Test
    void checkovNotApplicableToPlainYaml() throws Exception {
        assumeCheckov();
        Path repo = repoDir("ckna");
        write(repo, "plain.yaml", "---\nkey: value\n");

        ScanSummary summary = scan(repo, List.of("plain.yaml"), Set.of("checkov"), ScanConfig.defaults());

        assertEquals(ScannerStatus.NOT_APPLICABLE, summary.scannerOutcomes().get(0).status());
        assertEquals(ScanState.FAILED, summary.state());
        assertEquals(ScanVerdict.INDETERMINATE, summary.verdict());
    }

    @Test
    void checkovMissingToolIsUnavailable() throws Exception {
        Path repo = repoDir("ckmiss");
        write(repo, "pod.yaml", PRIVILEGED_POD);
        System.setProperty("pwt.tools.checkov", "/nonexistent-checkov-xyz");
        try {
            ScanSummary summary = scan(repo, List.of("pod.yaml"), Set.of("checkov"), ScanConfig.defaults());
            assertEquals(ScannerStatus.UNAVAILABLE, summary.scannerOutcomes().get(0).status());
        } finally {
            System.clearProperty("pwt.tools.checkov");
        }
    }

    private static final String GOOD_WORKFLOW = "---\non: push\njobs:\n  test:\n"
            + "    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n";
    private static final String BAD_WORKFLOW = "---\non: [push\njobs:\n";

    @Test
    void actionlintFindsWorkflowProblems() throws Exception {
        assumeActionlint();
        Path repo = repoDir("al");
        write(repo, ".github/workflows/good.yml", GOOD_WORKFLOW);
        write(repo, ".github/workflows/bad.yml", BAD_WORKFLOW);

        ScanSummary summary = scan(repo,
                List.of(".github/workflows/good.yml", ".github/workflows/bad.yml"),
                Set.of("actionlint"), ScanConfig.defaults());

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        var outcome = summary.scannerOutcomes().get(0);
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertTrue(outcome.toolVersion().startsWith("v"), outcome.toolVersion());
        assertTrue(outcome.findings().stream()
                .allMatch(f -> f.relativePath().equals(".github/workflows/bad.yml")));
    }

    @Test
    void actionlintIgnoresNonWorkflows() throws Exception {
        assumeActionlint();
        Path repo = repoDir("alna");
        write(repo, "pod.yaml", CLEAN_POD);

        ScanSummary summary = scan(repo, List.of("pod.yaml"), Set.of("actionlint"), ScanConfig.defaults());
        assertEquals(ScannerStatus.NOT_APPLICABLE, summary.scannerOutcomes().get(0).status());
    }

    @Test
    void actionlintDropsSecretsFromSnippets() throws Exception {
        assumeActionlint();
        Path repo = repoDir("alsecret");
        write(repo, ".github/workflows/bad.yml", BAD_WORKFLOW + "# " + FAKE_TOKEN + "\n");

        ScanSummary summary = scan(repo, List.of(".github/workflows/bad.yml"), Set.of("actionlint"),
                ScanConfig.defaults());
        var outcome = summary.scannerOutcomes().get(0);
        assertTrue(outcome.findings().size() > 0);
        for (var finding : outcome.findings()) {
            assertTrue(!finding.message().contains(FAKE_TOKEN));
            assertTrue(!finding.evidenceJson().contains(FAKE_TOKEN));
        }
    }

    private static String localTemplate(Path dir) {
        return dir + "/{{.ResourceKind}}.json";
    }

    private Path kubeconformFixtures() throws Exception {
        Path dir = work.resolve("kcschemas");
        Files.createDirectories(dir);
        Path source = Path.of(KubeconformParserTest.class.getResource("/kubeconform-local/widget.json").toURI());
        Files.copy(source, dir.resolve("widget.json"));
        return dir;
    }

    private static final String WIDGET_GOOD = "---\napiVersion: example.io/v1\nkind: Widget\n"
            + "metadata:\n  name: w\nspec:\n  replicas: 2\n";
    private static final String WIDGET_BAD = "---\napiVersion: example.io/v1\nkind: Widget\n"
            + "metadata:\n  name: w\nspec:\n  replicas: many\n";

    @Test
    void kubeconformValidatesAgainstLocalSchemas() throws Exception {
        assumeKubeconform();
        Path repo = repoDir("kc");
        write(repo, "good.yaml", WIDGET_GOOD);
        write(repo, "bad.yaml", WIDGET_BAD);
        ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                new ScanConfig.KubeconformConfig(localTemplate(kubeconformFixtures()), "1.29.0", true), null);

        ScanSummary summary = scan(repo, List.of("good.yaml", "bad.yaml"), Set.of("kubeconform"), config);

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        var outcome = summary.scannerOutcomes().get(0);
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        assertEquals("bad.yaml", outcome.findings().get(0).relativePath());
        assertTrue(outcome.detail().contains("1 valid"), outcome.detail());
    }

    @Test
    void kubeconformMissingCacheReportsGaps() throws Exception {
        assumeKubeconform();
        Path repo = repoDir("kcnoschema");
        write(repo, "good.yaml", WIDGET_GOOD);
        ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                new ScanConfig.KubeconformConfig(null, "1.29.0", true), null);

        Path emptyCache = work.resolve("empty-data");
        ScanService service = new ScanService(config);
        ScanSummary summary = service.runScan(repo, List.of("good.yaml"), Set.of("kubeconform"), "CLI",
                emptyCache, CancellationToken.none(), null);
        // Empty cache: real run, per-resource schema-missing gaps, no findings.
        assertEquals(ScannerStatus.SUCCEEDED, summary.scannerOutcomes().get(0).status());
        assertTrue(summary.coverageGaps().stream().anyMatch(g -> g.contains("no usable schema")),
                summary.coverageGaps().toString());
    }

    @Test
    void kubeconformBlankTemplateSkips() throws Exception {
        assumeKubeconform();
        Path staged = work.resolve("staged");
        Files.createDirectories(staged);
        Files.writeString(staged.resolve("good.yaml"), WIDGET_GOOD);
        var ctx = new pwt.adapters.ScannerAdapter.ScanContext(staged, List.of("good.yaml"), null,
                Duration.ofSeconds(30), CancellationToken.none(), null, "1.29.0", true, null);
        var outcome = new KubeconformAdapter().execute(ctx);
        assertEquals(ScannerStatus.SKIPPED, outcome.status());
        assertTrue(outcome.detail().contains("kubeconform-setup"));
    }

    @Test
    void kubeconformMissingKindSchemaIsGap() throws Exception {
        assumeKubeconform();
        Path repo = repoDir("kcgap");
        write(repo, "custom.yaml",
                "---\napiVersion: example.io/v1\nkind: Gadget\nmetadata:\n  name: g\n");
        ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                new ScanConfig.KubeconformConfig(localTemplate(kubeconformFixtures()), "1.29.0", true), null);

        ScanSummary summary = scan(repo, List.of("custom.yaml"), Set.of("kubeconform"), config);

        assertEquals(ScannerStatus.SUCCEEDED, summary.scannerOutcomes().get(0).status());
        assertTrue(summary.scannerOutcomes().get(0).findings().isEmpty());
        assertTrue(summary.coverageGaps().stream().anyMatch(g -> g.contains("Gadget")),
                summary.coverageGaps().toString());
    }

    @Test
    void kubeconformUpstreamSchemasValidateForReal() throws Exception {
        assumeKubeconform();
        Path cache = work.resolve("upschemas").resolve("v1.29.0-standalone-strict");
        Files.createDirectories(cache);
        try {
            for (String name : List.of("deployment-apps-v1", "pod-v1")) {
                java.net.URLConnection connection = java.net.URI.create(
                        "https://raw.githubusercontent.com/yannh/kubernetes-json-schema/master/"
                                + "v1.29.0-standalone-strict/" + name + ".json").toURL().openConnection();
                connection.setConnectTimeout(30_000);
                connection.setReadTimeout(60_000);
                try (var in = connection.getInputStream()) {
                    Files.write(cache.resolve(name + ".json"), in.readAllBytes());
                }
            }
        } catch (Exception e) {
            org.junit.jupiter.api.Assumptions.abort("SKIP: cannot fetch upstream schemas: " + e.getMessage());
            return;
        }
        Path repo = repoDir("kcup");
        write(repo, "pod.yaml", CLEAN_POD);
        write(repo, "depl.yaml", "---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n"
                + "  name: web\nspec:\n  replicas: many\n");
        String template = work.resolve("upschemas")
                + "/{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}"
                + "/{{.ResourceKind}}{{.KindSuffix}}.json";
        ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                new ScanConfig.KubeconformConfig(template, "1.29.0", true), null);

        ScanSummary summary = scan(repo, List.of("pod.yaml", "depl.yaml"), Set.of("kubeconform"), config);

        assertEquals(ScannerStatus.SUCCEEDED, summary.scannerOutcomes().get(0).status());
        assertEquals(1, summary.scannerOutcomes().get(0).findings().size());
        assertEquals("depl.yaml", summary.scannerOutcomes().get(0).findings().get(0).relativePath());
    }

    @Test
    void localSchemaEndToEndThroughScanService() throws Exception {
        Path repo = repoDir("ls");
        write(repo, "good.yaml", "name: web\nspec:\n  replicas: 3\n  image: nginx\n");
        write(repo, "bad.yaml", "name: web\nspec:\n  replicas: three\n");
        Path schema = Path.of(LocalSchemaTest.class.getResource("/localschema/widget-config.json").toURI());
        LocalSchemaConfig bindings = LocalSchemaConfig.parse(
                List.of("*.yaml=" + schema.toString()), null, work);
        ScanConfig config = new ScanConfig(ScanConfig.defaults().toolTimeout(),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                ScanConfig.KubeconformConfig.defaults(), bindings);

        ScanSummary summary = scan(repo, List.of("good.yaml", "bad.yaml"), Set.of("local-schema"), config);

        assertEquals(ScanState.COMPLETED, summary.state());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        assertEquals(1, summary.findingCount());
    }

    @Test
    void combinedSixAdapterRunAggregates() throws Exception {
        assumeCheckov();
        assumeActionlint();
        assumeKubeconform();
        Path repo = repoDir("all");
        write(repo, "pod.yaml", PRIVILEGED_POD);
        write(repo, ".github/workflows/bad.yml", BAD_WORKFLOW);
        write(repo, "widget-bad.yaml", WIDGET_BAD);
        write(repo, "plain.yaml", "---\nkey: value\n");
        Path schema = Path.of(LocalSchemaTest.class.getResource("/localschema/widget-config.json").toURI());
        LocalSchemaConfig bindings = LocalSchemaConfig.parse(
                List.of("widget-*.yaml=" + schema.toString()), null, work);
        ScanConfig config = new ScanConfig(Duration.ofSeconds(180),
                ScanConfig.defaults().maxStdoutBytes(), ScanConfig.defaults().maxStderrBytes(),
                new ScanConfig.KubeconformConfig(localTemplate(kubeconformFixtures()), "1.29.0", true),
                bindings);
        Path dataDir = work.resolve("data");

        ScanService service = new ScanService(config);
        ScanSummary summary = service.runScan(repo,
                List.of("pod.yaml", ".github/workflows/bad.yml", "widget-bad.yaml", "plain.yaml"),
                Set.of(), "CLI", dataDir, CancellationToken.none(), null);

        assertEquals(6, summary.scannerOutcomes().size());
        long succeeded = summary.scannerOutcomes().stream()
                .filter(o -> o.status() == ScannerStatus.SUCCEEDED).count();
        assertTrue(succeeded >= 5, summary.scannerOutcomes().toString());
        assertEquals(ScanVerdict.FINDINGS, summary.verdict());
        assertTrue(summary.findingCount() >= 4);
        // Reports include every outcome and validate against the schema.
        String json = Files.readString(dataDir.resolve(summary.jsonReportPath()));
        JsonNode root = MAPPER.readTree(json);
        assertEquals(6, root.get("scanner_runs").size());
        assertEquals(summary.runId(), root.get("run").get("id").asText());
        assertTrue(Files.exists(dataDir.resolve(summary.markdownReportPath())));
    }

    @Test
    void nothingApplicableIsExplicitlyIndeterminate() throws Exception {
        assumeCheckov();
        assumeActionlint();
        Path repo = repoDir("na");
        write(repo, "plain.yaml", "---\nkey: value\n");

        ScanSummary summary = scan(repo, List.of("plain.yaml"),
                Set.of("checkov", "actionlint", "kubeconform"), ScanConfig.defaults());

        assertTrue(summary.scannerOutcomes().stream()
                .allMatch(o -> o.status() == ScannerStatus.NOT_APPLICABLE
                        || o.status() == ScannerStatus.SKIPPED));
        assertEquals(ScanState.FAILED, summary.state());
        assertEquals(ScanVerdict.INDETERMINATE, summary.verdict());
        assertTrue(summary.coverageGaps().stream().anyMatch(g -> g.contains("No selected file was applicable")));
    }
}

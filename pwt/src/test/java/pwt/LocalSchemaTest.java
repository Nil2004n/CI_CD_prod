package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.adapters.LocalSchemaAdapter;
import pwt.adapters.ScannerAdapter;
import pwt.application.CancellationToken;
import pwt.application.LocalSchemaConfig;
import pwt.domain.FindingCategory;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;

class LocalSchemaTest {

    @TempDir
    Path temp;

    private static Path resource(String name) throws Exception {
        Path path = Path.of(LocalSchemaTest.class.getResource("/localschema/" + name).toURI());
        assertTrue(Files.isRegularFile(path), "test resource missing: " + name);
        return path;
    }

    private Path write(String relative, String content) throws Exception {
        Path file = temp.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.writeString(file, content);
        return file;
    }

    private static ScannerAdapter.ScanContext contextFor(Path stagedDir, List<String> targets,
            LocalSchemaConfig config) {
        return new ScannerAdapter.ScanContext(stagedDir, targets, null,
                Duration.ofSeconds(60), CancellationToken.none(), null, "1.29.0", true, config);
    }

    @Test
    void bindingValidation() {
        assertThrows(IllegalArgumentException.class,
                () -> new LocalSchemaConfig.Binding("", Path.of("/s.json")));
        assertThrows(IllegalArgumentException.class,
                () -> new LocalSchemaConfig.Binding("[unclosed", Path.of("/s.json")));
        assertThrows(IllegalArgumentException.class,
                () -> new LocalSchemaConfig.Binding("*.yaml", Path.of("relative.json")));
        assertThrows(IllegalArgumentException.class,
                () -> LocalSchemaConfig.parse(List.of("no-equals-here"), null, temp));
        assertThrows(IllegalArgumentException.class,
                () -> LocalSchemaConfig.parse(List.of("*.yaml=/schema.json"), "draft-07", temp));
        LocalSchemaConfig parsed = LocalSchemaConfig.parse(
                List.of("*.yaml=widget.json", "k8s/*.yml=/abs/other.json"), null, temp);
        assertEquals(2, parsed.bindings().size());
        assertEquals("2020-12", parsed.dialect());
        assertTrue(parsed.bindings().get(0).schemaPath().isAbsolute());
        assertTrue(parsed.matching("a.yaml").size() == 1);
        assertTrue(parsed.matching("k8s/a.yml").size() == 1);
        assertTrue(parsed.matching("other.json").isEmpty());
        assertTrue(LocalSchemaConfig.none().isEmpty());
    }

    @Test
    void referenceVetting() throws Exception {
        Path dir = resource("widget-config.json").getParent();
        // Clean schema passes.
        LocalSchemaAdapter.vetReferences(
                new com.fasterxml.jackson.databind.ObjectMapper()
                        .readTree(Files.readString(resource("widget-config.json"))),
                dir);
        // Local in-root reference passes.
        LocalSchemaAdapter.vetReferences(
                new com.fasterxml.jackson.databind.ObjectMapper()
                        .readTree(Files.readString(resource("outer.json"))),
                dir);
        // Remote reference rejected.
        assertThrows(Exception.class, () -> LocalSchemaAdapter.vetReferences(
                new com.fasterxml.jackson.databind.ObjectMapper()
                        .readTree(Files.readString(resource("remote-ref-schema.json"))),
                dir));
        // Escape rejected (../outside.json resolves above dir and is missing).
        assertThrows(Exception.class, () -> LocalSchemaAdapter.vetReferences(
                new com.fasterxml.jackson.databind.ObjectMapper()
                        .readTree(Files.readString(resource("escape-ref-schema.json"))),
                dir));
        // Remote $id alone is inert, but with a relative ref it would redirect
        // resolution to the network: rejected.
        var mapper = new com.fasterxml.jackson.databind.ObjectMapper();
        assertThrows(Exception.class, () -> LocalSchemaAdapter.vetReferences(
                mapper.readTree("{\"$id\":\"https://example.invalid/x.json\","
                        + "\"properties\":{\"a\":{\"$ref\":\"other.json\"}}}"),
                dir));
        // Same shape with a local (non-remote) $id is fine when the target exists.
        LocalSchemaAdapter.vetReferences(
                mapper.readTree("{\"$id\":\"urn:local:test\","
                        + "\"properties\":{\"a\":{\"$ref\":\"inner.json#/$defs/nonEmpty\"}}}"),
                dir);
    }

    @Test
    void noBindingsMeansSkipped() throws Exception {
        Path staged = write("staged/a.yaml", "name: x\n");
        var outcome = new LocalSchemaAdapter().execute(
                contextFor(staged.getParent(), List.of("a.yaml"), LocalSchemaConfig.none()));
        assertEquals(ScannerStatus.SKIPPED, outcome.status());
    }

    @Test
    void unmatchedFilesMeanNotApplicable() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/other.json", "{}");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("widget-config.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("other.json"), config));
        assertEquals(ScannerStatus.NOT_APPLICABLE, outcome.status());
    }

    @Test
    void validAndInvalidInstances() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/good.yaml", "name: web\nspec:\n  replicas: 3\n  image: nginx\n");
        write("staged/bad.yaml", "name: web\nspec:\n  replicas: three\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("widget-config.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(
                contextFor(staged, List.of("good.yaml", "bad.yaml"), config));
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        var finding = outcome.findings().get(0);
        assertEquals("bad.yaml", finding.relativePath());
        assertEquals(Severity.MEDIUM, finding.severity());
        assertEquals(FindingCategory.SCHEMA, finding.category());
        assertTrue(finding.message().contains("$.spec.replicas") || finding.message().contains("spec"),
                finding.message());
        assertTrue(finding.evidenceJson().contains("widget-config.json"));
        assertEquals(null, finding.lineStart(), "line numbers must not be fabricated");
    }

    @Test
    void multiDocumentFilesValidatedSeparately() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/multi.yaml",
                "name: one\nspec:\n  replicas: 1\n---\nname: two\nspec:\n  replicas: many\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("widget-config.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("multi.yaml"), config));
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        assertTrue(outcome.findings().get(0).evidenceJson().contains("\"document\":1"));
    }

    @Test
    void syntaxFailureBecomesFinding() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/broken.yaml", "name: [unclosed\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("widget-config.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("broken.yaml"), config));
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        assertEquals(FindingCategory.SYNTAX, outcome.findings().get(0).category());
    }

    @Test
    void duplicateKeysRejectedExplicitly() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/dup.yaml", "name: a\nname: b\nspec:\n  replicas: 1\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("widget-config.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("dup.yaml"), config));
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        assertEquals("localschema/syntax", outcome.findings().get(0).ruleId());
    }

    @Test
    void missingSchemaFileSkips() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/a.yaml", "name: x\nspec:\n  replicas: 1\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + temp.resolve("missing.json").toString()), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("a.yaml"), config));
        assertEquals(ScannerStatus.SKIPPED, outcome.status());
        assertTrue(outcome.findings().isEmpty());
    }

    @Test
    void unsupportedDialectSkips() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/a.yaml", "name: x\nspec:\n  replicas: 1\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("draft07-schema.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("a.yaml"), config));
        assertEquals(ScannerStatus.SKIPPED, outcome.status());
        assertTrue(outcome.findings().isEmpty());
    }

    @Test
    void remoteReferenceRejected() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/a.yaml", "name: x\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("remote-ref-schema.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(contextFor(staged, List.of("a.yaml"), config));
        assertEquals(ScannerStatus.SKIPPED, outcome.status());
        assertTrue(outcome.detail().contains("remote"));
    }

    @Test
    void localReferenceWithinRootWorks() throws Exception {
        Path staged = temp.resolve("staged");
        Files.createDirectories(staged);
        write("staged/a.yaml", "name: web\n");
        write("staged/b.yaml", "name: \"\"\n");
        LocalSchemaConfig config = LocalSchemaConfig.parse(
                List.of("*.yaml=" + resource("outer.json")), null, temp);
        var outcome = new LocalSchemaAdapter().execute(
                contextFor(staged, List.of("a.yaml", "b.yaml"), config));
        assertEquals(ScannerStatus.SUCCEEDED, outcome.status());
        assertEquals(1, outcome.findings().size());
        assertEquals("b.yaml", outcome.findings().get(0).relativePath());
    }

    @Test
    void probeLoadsValidator() {
        var probe = new LocalSchemaAdapter().probe();
        assertTrue(probe.available());
        assertTrue(probe.version().contains("2020-12"));
    }
}

package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Set;
import org.junit.jupiter.api.Test;
import pwt.adapters.KubeconformAdapter;
import pwt.domain.FindingCategory;
import pwt.domain.Severity;

class KubeconformParserTest {

    private static String resource(String file, String kind, String status, String extra) {
        return "{\"filename\":\"" + file + "\",\"kind\":\"" + kind + "\",\"name\":\"web\","
                + "\"version\":\"apps/v1\",\"status\":\"" + status + "\"," + extra + "}";
    }

    private static String envelope(String... resources) {
        return "{\"resources\":[" + String.join(",", resources) + "]}";
    }

    @Test
    void validResourcesCountedWithoutFindings() throws Exception {
        var result = KubeconformAdapter.parseReport(
                envelope(resource("pod.yaml", "Pod", "statusValid", "\"msg\":\"\"")), Set.of("pod.yaml"));
        assertEquals(1, result.valid());
        assertTrue(result.findings().isEmpty());
        assertTrue(result.gaps().isEmpty());
    }

    @Test
    void invalidResourceBecomesFindingWithErrors() throws Exception {
        String invalid = resource("depl.yaml", "Deployment", "statusInvalid",
                "\"msg\":\"problem validating schema.\","
                        + "\"validationErrors\":[{\"path\":\"/spec\",\"msg\":\"missing properties 'template'\"},"
                        + "{\"path\":\"/spec/replicas\",\"msg\":\"got string, want null or integer\"}]");
        var result = KubeconformAdapter.parseReport(envelope(invalid), Set.of("depl.yaml"));
        assertEquals(1, result.findings().size());
        var finding = result.findings().get(0);
        assertEquals("kubeconform/Deployment", finding.ruleId());
        assertEquals(Severity.MEDIUM, finding.severity());
        assertEquals(FindingCategory.SCHEMA, finding.category());
        assertEquals("depl.yaml", finding.relativePath());
        assertEquals(null, finding.lineStart());
        assertTrue(finding.message().contains("/spec/replicas"));
        assertTrue(result.gaps().isEmpty());
    }

    @Test
    void missingSchemaBecomesGapNotFinding() throws Exception {
        String error = resource("custom.yaml", "Widget", "statusError",
                "\"msg\":\"could not find schema for Widget\"");
        var result = KubeconformAdapter.parseReport(envelope(error), Set.of("custom.yaml"));
        assertTrue(result.findings().isEmpty());
        assertEquals(1, result.gaps().size());
        assertTrue(result.gaps().get(0).contains("Widget"));
    }

    @Test
    void skippedAndUnknownStatusesBecomeGaps() throws Exception {
        var result = KubeconformAdapter.parseReport(envelope(
                resource("a.yaml", "Pod", "statusSkipped", "\"msg\":\"x\""),
                resource("b.yaml", "Pod", "statusMystery", "\"msg\":\"x\"")), Set.of("a.yaml", "b.yaml"));
        assertTrue(result.findings().isEmpty());
        assertEquals(2, result.gaps().size());
    }

    @Test
    void outOfScopeResourcesIgnored() throws Exception {
        var result = KubeconformAdapter.parseReport(
                envelope(resource("other.yaml", "Pod", "statusInvalid", "\"msg\":\"bad\"")),
                Set.of("pod.yaml"));
        assertTrue(result.findings().isEmpty());
        assertTrue(result.gaps().isEmpty());
    }

    @Test
    void malformedOutputRejected() {
        assertThrows(Exception.class,
                () -> KubeconformAdapter.parseReport("not json", Set.of("a.yaml")));
        assertThrows(Exception.class,
                () -> KubeconformAdapter.parseReport("{\"no\":\"resources\"}", Set.of("a.yaml")));
    }
}

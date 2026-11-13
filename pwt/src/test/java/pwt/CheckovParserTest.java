package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;
import pwt.adapters.CheckovAdapter;
import pwt.domain.FindingCategory;
import pwt.domain.Severity;

class CheckovParserTest {

    private static String envelope(String failedChecks) {
        return "{\"check_type\":\"kubernetes\","
                + "\"results\":{\"failed_checks\":" + failedChecks + ",\"skipped_checks\":[]},"
                + "\"summary\":{\"passed\":69,\"failed\":1,\"skipped\":0,"
                + "\"parsing_errors\":0,\"resource_count\":1,\"checkov_version\":\"3.4.4\"}}";
    }

    private static String failedCheck() {
        return "{\"check_id\":\"CKV_K8S_20\",\"check_name\":\"Containers should not run with allowPrivilegeEscalation\","
                + "\"check_result\":{\"result\":\"FAILED\",\"evaluated_keys\":[\"spec/containers/[0]/securityContext/allowPrivilegeEscalation\"]},"
                + "\"file_path\":\"/pod.yaml\",\"file_abs_path\":\"/stage/pod.yaml\","
                + "\"repo_file_path\":\"/pod.yaml\",\"file_line_range\":[1,12],"
                + "\"resource\":\"Pod.default.web\",\"check_class\":\"checkov.kubernetes.checks.resource.k8s.AllowPrivilegeEscalation\","
                + "\"severity\":null,\"guideline\":\"https://example.invalid/policy\"}";
    }

    @Test
    void failedCheckNormalized() throws Exception {
        CheckovAdapter.ParseResult result =
                CheckovAdapter.parseReport(envelope("[" + failedCheck() + "]"), Set.of("pod.yaml"));
        assertEquals(1, result.findings().size());
        var finding = result.findings().get(0);
        assertEquals("checkov/CKV_K8S_20", finding.ruleId());
        assertEquals("pod.yaml", finding.relativePath());
        assertEquals(1, finding.lineStart());
        assertEquals(12, finding.lineEnd());
        assertEquals(Severity.UNKNOWN, finding.severity(),
                "null tool severity must map to UNKNOWN, never invented");
        assertEquals("UNKNOWN", finding.originalSeverity() == null ? "UNKNOWN" : finding.originalSeverity());
        assertEquals(FindingCategory.MISCONFIGURATION, finding.category());
        assertTrue(finding.message().contains("Pod.default.web"));
        assertTrue(finding.evidenceJson().contains("allowPrivilegeEscalation"));
        assertEquals(69, result.passed());
    }

    @Test
    void toolSeverityPreservedWhenPresent() throws Exception {
        String withSeverity = failedCheck().replace("\"severity\":null", "\"severity\":\"HIGH\"");
        CheckovAdapter.ParseResult result =
                CheckovAdapter.parseReport(envelope("[" + withSeverity + "]"), Set.of("pod.yaml"));
        assertEquals(Severity.HIGH, result.findings().get(0).severity());
        assertEquals("HIGH", result.findings().get(0).originalSeverity());
    }

    @Test
    void outOfScopeFilesSkipped() throws Exception {
        CheckovAdapter.ParseResult result =
                CheckovAdapter.parseReport(envelope("[" + failedCheck() + "]"), Set.of("other.yaml"));
        assertTrue(result.findings().isEmpty());
    }

    @Test
    void nonKubernetesCheckTypeRejected() {
        assertThrows(Exception.class, () -> CheckovAdapter.parseReport(
                envelope("[]").replace("kubernetes", "terraform"), Set.of("pod.yaml")));
    }

    @Test
    void malformedEnvelopesRejected() {
        assertThrows(Exception.class, () -> CheckovAdapter.parseReport("not json", Set.of("pod.yaml")));
        assertThrows(Exception.class,
                () -> CheckovAdapter.parseReport("{\"no\":\"envelope\"}", Set.of("pod.yaml")));
    }

    @Test
    void repoPathNormalization() {
        assertEquals("pod.yaml", CheckovAdapter.normalizeRepoPath("/pod.yaml"));
        assertEquals("sub/a.yaml", CheckovAdapter.normalizeRepoPath("sub/a.yaml"));
        assertEquals(null, CheckovAdapter.normalizeRepoPath(null));
        assertEquals(null, CheckovAdapter.normalizeRepoPath(""));
    }

    @Test
    void severityMappingDocumented() {
        assertEquals(Severity.CRITICAL, CheckovAdapter.mapSeverity("CRITICAL"));
        assertEquals(Severity.HIGH, CheckovAdapter.mapSeverity("high"));
        assertEquals(Severity.MEDIUM, CheckovAdapter.mapSeverity("MODERATE"));
        assertEquals(Severity.LOW, CheckovAdapter.mapSeverity("low"));
        assertEquals(Severity.INFO, CheckovAdapter.mapSeverity("informational"));
        assertEquals(Severity.UNKNOWN, CheckovAdapter.mapSeverity(null));
        assertEquals(Severity.UNKNOWN, CheckovAdapter.mapSeverity("WHATEVER"));
    }
}

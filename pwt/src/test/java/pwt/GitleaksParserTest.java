package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;
import pwt.adapters.GitleaksAdapter;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.Severity;

class GitleaksParserTest {

    private static final String FAKE_SECRET = "ghp_X7sK9mQ2vB4nL8wZ1pD6fH3jR5tY0cV1eA9b";

    private static String reportJson() {
        return "[{\"RuleID\":\"github-pat\","
                + "\"Description\":\"Uncovered a GitHub Personal Access Token.\","
                + "\"StartLine\":2,\"EndLine\":2,\"StartColumn\":10,\"EndColumn\":49,"
                + "\"Match\":\"" + FAKE_SECRET + "\",\"Secret\":\"" + FAKE_SECRET + "\","
                + "\"File\":\"sub/leak.yaml\",\"Entropy\":5.17,\"Tags\":[],"
                + "\"Fingerprint\":\"sub/leak.yaml:github-pat:2\"}]";
    }

    @Test
    void cleanReportYieldsNoFindings() throws Exception {
        GitleaksAdapter.ParseResult result = GitleaksAdapter.parseReport("[]", Set.of("a.yaml"));
        assertTrue(result.findings().isEmpty());
        assertEquals(0, result.skippedOutOfScope());
    }

    @Test
    void findingNormalizedAndSecretsDropped() throws Exception {
        GitleaksAdapter.ParseResult result =
                GitleaksAdapter.parseReport(reportJson(), Set.of("sub/leak.yaml"));
        assertEquals(1, result.findings().size());
        assertEquals(0, result.skippedOutOfScope());
        Finding finding = result.findings().get(0);
        assertEquals("gitleaks/github-pat", finding.ruleId());
        assertEquals(Severity.HIGH, finding.severity());
        assertEquals(FindingCategory.SECRET, finding.category());
        assertEquals("sub/leak.yaml", finding.relativePath());
        assertEquals(2, finding.lineStart());
        assertTrue(finding.message().contains("sub/leak.yaml"));
        // Secret hygiene: neither the finding nor its evidence may carry the
        // secret, the raw match, or any raw excerpt.
        assertTrue(!finding.message().contains(FAKE_SECRET));
        assertTrue(!finding.evidenceJson().contains(FAKE_SECRET));
        assertTrue(!finding.evidenceJson().contains("Match"));
        assertTrue(!finding.evidenceJson().contains("Secret"));
        assertTrue(finding.evidenceJson().contains("5.17"));
    }

    @Test
    void outOfScopeEntriesSkippedAndCounted() throws Exception {
        GitleaksAdapter.ParseResult result =
                GitleaksAdapter.parseReport(reportJson(), Set.of("other.yaml"));
        assertTrue(result.findings().isEmpty());
        assertEquals(1, result.skippedOutOfScope());
    }

    @Test
    void malformedReportsRejected() {
        assertThrows(Exception.class,
                () -> GitleaksAdapter.parseReport("not json", Set.of("a.yaml")));
        assertThrows(Exception.class,
                () -> GitleaksAdapter.parseReport("{\"no\":\"array\"}", Set.of("a.yaml")));
    }

    @Test
    void absolutePathsBeneathStagingRelativized() throws Exception {
        String absolute = reportJson().replace("sub/leak.yaml",
                java.nio.file.Path.of("/tmp/stage-xyz/sub/leak.yaml").toString());
        GitleaksAdapter.ParseResult result = GitleaksAdapter.parseReport(absolute,
                java.nio.file.Path.of("/tmp/stage-xyz"), Set.of("sub/leak.yaml"));
        assertEquals(1, result.findings().size());
        assertEquals("sub/leak.yaml", result.findings().get(0).relativePath());
    }

    @Test
    void backslashPathsNormalized() throws Exception {
        String windowsStyle = reportJson().replace("sub/leak.yaml", "sub\\\\leak.yaml");
        GitleaksAdapter.ParseResult result =
                GitleaksAdapter.parseReport(windowsStyle, Set.of("sub/leak.yaml"));
        assertEquals(1, result.findings().size());
    }
}

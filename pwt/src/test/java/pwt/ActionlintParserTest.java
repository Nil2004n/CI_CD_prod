package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;
import pwt.adapters.ActionlintAdapter;
import pwt.domain.FindingCategory;
import pwt.domain.Severity;

class ActionlintParserTest {

    @Test
    void workflowPathsApplicableOthersNot() {
        assertEquals(List.of(".github/workflows/ci.yml", ".github/workflows/UPPER.YML"),
                ActionlintAdapter.applicableTargets(List.of(
                        ".github/workflows/ci.yml", "ci.yml", "workflows/x.yaml",
                        ".github/workflows/notes.txt", ".github/actions/a/action.yml",
                        ".github/workflows/sub/deep.yaml", "k8s/pod.yaml",
                        ".github/workflows/UPPER.YML")));
    }

    @Test
    void findingsParsedSnippetDropped() throws Exception {
        String json = "[{\"message\":\"could not parse as YAML: boom\",\"filepath\":\".github/workflows/bad.yml\","
                + "\"line\":1,\"column\":4,\"kind\":\"syntax-check\","
                + "\"snippet\":\"on: [push\\n   ^\",\"end_column\":4},"
                + "{\"message\":\"property \\\"jobz\\\" is not defined\",\"filepath\":\".github/workflows/ok.yml\","
                + "\"line\":12,\"column\":3,\"kind\":\"job-needs\","
                + "\"snippet\":\"jobz\\n   ^\",\"end_column\":3}]";
        var parsed = ActionlintAdapter.parseReport(json, Set.of(".github/workflows/bad.yml"));
        // Only the in-scope file survives.
        assertEquals(1, parsed.size());
        var finding = parsed.get(0);
        assertEquals("actionlint/syntax-check", finding.ruleId());
        assertEquals(Severity.MEDIUM, finding.severity());
        assertEquals(FindingCategory.SYNTAX, finding.category());
        assertEquals(".github/workflows/bad.yml", finding.relativePath());
        assertEquals(1, finding.lineStart());
        assertTrue(finding.message().contains(".github/workflows/bad.yml:1"));
        assertTrue(!finding.evidenceJson().contains("snippet"),
                "raw source excerpts must not survive parsing");
        assertTrue(!finding.message().contains("on: [push"),
                "raw source excerpts must not leak into messages");
    }

    @Test
    void ruleKindsMapped() throws Exception {
        String json = "[{\"message\":\"m\",\"filepath\":\".github/workflows/a.yml\","
                + "\"line\":5,\"column\":1,\"kind\":\"job-needs\",\"snippet\":\"\",\"end_column\":1}]";
        var findings = ActionlintAdapter.parseReport(json, Set.of(".github/workflows/a.yml"));
        assertEquals(1, findings.size());
        assertEquals(Severity.LOW, findings.get(0).severity());
        assertEquals(FindingCategory.LINT, findings.get(0).category());
    }

    @Test
    void malformedOutputRejected() {
        assertThrows(Exception.class,
                () -> ActionlintAdapter.parseReport("not json", Set.of("a.yml")));
        assertThrows(Exception.class,
                () -> ActionlintAdapter.parseReport("{\"x\":1}", Set.of("a.yml")));
    }

    @Test
    void cleanOutputYieldsNoFindings() throws Exception {
        assertTrue(ActionlintAdapter.parseReport("[]", Set.of(".github/workflows/a.yml")).isEmpty());
    }
}

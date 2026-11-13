package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import org.junit.jupiter.api.Test;
import pwt.adapters.YamllintAdapter;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.Severity;

class YamllintParserTest {

    @Test
    void cleanOutputYieldsNoFindings() {
        assertTrue(YamllintAdapter.parseParsable("").isEmpty());
        assertTrue(YamllintAdapter.parseParsable("   \n").isEmpty());
    }

    @Test
    void findingsParsedWithLocationRuleAndSeverity() {
        String output = "sub/a.yaml:1:1: [warning] missing document start \"---\" (document-start)\n"
                + "sub/a.yaml:3:81: [error] line too long (91 > 80 characters) (line-length)\n"
                + "bad.yaml:3:1: [error] syntax error: unexpected end (syntax)\n";
        List<Finding> findings = YamllintAdapter.parseParsable(output);
        assertEquals(3, findings.size());

        Finding warning = findings.get(0);
        assertEquals("yamllint/document-start", warning.ruleId());
        assertEquals("sub/a.yaml", warning.relativePath());
        assertEquals(1, warning.lineStart());
        assertEquals(1, warning.columnStart());
        assertEquals(Severity.INFO, warning.severity());
        assertEquals("warning", warning.originalSeverity());
        assertEquals(FindingCategory.LINT, warning.category());

        Finding error = findings.get(1);
        assertEquals(Severity.LOW, error.severity());
        assertEquals("error", error.originalSeverity());

        Finding syntax = findings.get(2);
        assertEquals(Severity.MEDIUM, syntax.severity());
        assertEquals(FindingCategory.SYNTAX, syntax.category());
    }

    @Test
    void malformedLinesSkippedWithoutFindings() {
        String output = "not a finding line\n"
                + "a.yaml:1:1: [warning] missing document start \"---\" (document-start)\n"
                + "a.yaml:nope:x: [error] broken (line-length)\n";
        List<Finding> findings = YamllintAdapter.parseParsable(output);
        assertEquals(1, findings.size());
    }

    @Test
    void fingerprintsStableAndDistinct() {
        String output = "a.yaml:1:1: [warning] missing document start \"---\" (document-start)\n";
        String first = YamllintAdapter.parseParsable(output).get(0).fingerprint();
        String second = YamllintAdapter.parseParsable(output).get(0).fingerprint();
        assertEquals(first, second);
        assertEquals(64, first.length());
        String other = YamllintAdapter.parseParsable(
                "a.yaml:2:1: [warning] missing document start \"---\" (document-start)\n")
                .get(0).fingerprint();
        assertTrue(!other.equals(first), "different locations must fingerprint differently");
    }

    @Test
    void versionParsed() {
        assertEquals("1.38.0", YamllintAdapter.parseVersion("yamllint 1.38.0\n"));
        assertEquals("unknown", YamllintAdapter.parseVersion("garbage"));
        assertEquals("unknown", YamllintAdapter.parseVersion(null));
    }
}

package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;
import pwt.application.BuildInfo;
import pwt.mcp.PwtInfo;
import pwt.mcp.PwtMcpTools;

class McpInfoTest {

    @Test
    void toolDefinitionIsHarmlessAndDocumented() {
        var spec = PwtMcpTools.pwtInfoTool();
        assertEquals("pwt_info", spec.tool().name());
        assertFalse(spec.tool().description() == null || spec.tool().description().isBlank());
        assertEquals("object", spec.tool().inputSchema().type());
    }

    @Test
    @SuppressWarnings("unchecked")
    void pwtInfoReportsIdentityCapabilitiesAndCatalog() throws Exception {
        Map<String, Object> content = PwtInfo.content();
        assertEquals(BuildInfo.APP_NAME, content.get("app"));
        assertEquals(BuildInfo.VERSION, content.get("version"));
        assertEquals(BuildInfo.PHASE, content.get("phase"));

        List<String> capabilities = (List<String>) content.get("capabilities");
        assertTrue(capabilities != null && !capabilities.isEmpty());

        List<Map<String, Object>> catalog = (List<Map<String, Object>>) content.get("scanner_catalog");
        assertEquals(6, catalog.size());
        Map<String, String> executions = new java.util.LinkedHashMap<>();
        for (Map<String, Object> entry : catalog) {
            executions.put((String) entry.get("id"), (String) entry.get("execution"));
        }
        // All six integrations are implemented; availability is probed at scan
        // time by GUI/CLI, never via MCP.
        assertEquals("INTEGRATED", executions.get("yamllint"));
        assertEquals("INTEGRATED", executions.get("gitleaks"));
        assertEquals("INTEGRATED", executions.get("checkov"));
        assertEquals("INTEGRATED", executions.get("actionlint"));
        assertEquals("INTEGRATED", executions.get("kubeconform"));
        assertEquals("INTEGRATED", executions.get("local-schema"));

        String serialized = new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(content);
        assertTrue(serialized.contains("scanner_catalog"));
        assertTrue(serialized.contains("INTEGRATED"));
        // The info payload must not offer shell access, file reads, or editing
        // affordances. (Mentioning the real scan-operation tool names is fine;
        // offering a shell *command* affordance is not.)
        assertFalse(serialized.contains("run_command"));
        assertFalse(serialized.contains("execute_scan"));
        assertFalse(serialized.contains("shell -c"));
        assertFalse(serialized.toLowerCase().contains("arbitrary file"));
    }
}

package pwt.mcp;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.modelcontextprotocol.server.McpServerFeatures;
import io.modelcontextprotocol.spec.McpSchema;
import java.util.List;
import java.util.Map;

/**
 * MCP tool wiring: exactly one tool, {@code pwt_info}.
 * Kept separate from transport startup so tests can inspect the tool
 * definition and handler without opening stdio.
 */
public final class PwtMcpTools {

    public static final String TOOL_NAME = "pwt_info";
    public static final String TOOL_DESCRIPTION =
            "Returns PWT application identity, current capabilities, and the planned "
            + "scanner catalog (yamllint and Gitleaks integrated; others NOT_IMPLEMENTED). "
            + "Harmless: performs no scans, reads no repository files, and changes nothing. "
            + "Scanner execution happens only through GUI/CLI scans, never via MCP.";

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private PwtMcpTools() {
    }

    public static McpServerFeatures.SyncToolSpecification pwtInfoTool() {
        McpSchema.JsonSchema schema = new McpSchema.JsonSchema(
                "object", Map.of(), List.of(), false);
        McpSchema.Tool tool = new McpSchema.Tool(TOOL_NAME, TOOL_DESCRIPTION, schema);
        return new McpServerFeatures.SyncToolSpecification(tool, (exchange, arguments) -> {
            String payload;
            try {
                payload = MAPPER.writeValueAsString(PwtInfo.content());
            } catch (Exception e) {
                return new McpSchema.CallToolResult(
                        List.of(new McpSchema.TextContent("pwt_info serialization failed")), true);
            }
            return new McpSchema.CallToolResult(List.of(new McpSchema.TextContent(payload)), false);
        });
    }
}

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
            + "scanner catalog (all six integrations implemented; availability is probed "
            + "per run by GUI/CLI scans, never via MCP). "
            + "Harmless: performs no scans, reads no repository files, and changes nothing. "
            + "Scanner execution happens only through GUI/CLI scans, never via MCP.";

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private PwtMcpTools() {
    }

    public static McpServerFeatures.SyncToolSpecification pwtInfoTool() {
        return pwtInfoTool(null);
    }

    /** pwt_info with an optional per-call activity hook (receives the exchange). */
    public static McpServerFeatures.SyncToolSpecification pwtInfoTool(
            java.util.function.Consumer<io.modelcontextprotocol.server.McpSyncServerExchange> activityHook) {
        McpSchema.JsonSchema schema = new McpSchema.JsonSchema(
                "object", Map.of(), List.of(), false);
        McpSchema.Tool tool = new McpSchema.Tool(TOOL_NAME, TOOL_DESCRIPTION, schema);
        return new McpServerFeatures.SyncToolSpecification(tool, (exchange, arguments) -> {
            if (activityHook != null) {
                try {
                    activityHook.accept(exchange);
                } catch (Exception ignored) {
                    // presence must never break the tool
                }
            }
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

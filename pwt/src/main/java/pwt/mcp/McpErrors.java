package pwt.mcp;

import com.fasterxml.jackson.databind.ObjectMapper;
import io.modelcontextprotocol.spec.McpSchema;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Structured MCP errors. Tool handlers never throw raw stack traces or
 * unfiltered scanner output to the client; they return {@code isError}
 * results carrying a machine-readable code, a sanitized message, and a
 * correlation ID for log matching (diagnostics stay on stderr).
 */
public final class McpErrors {

    public static final String INVALID_INPUT = "INVALID_INPUT";
    public static final String NOT_FOUND = "NOT_FOUND";
    public static final String OUT_OF_SCOPE = "OUT_OF_SCOPE";
    public static final String PERMISSION_DENIED = "PERMISSION_DENIED";
    public static final String REPOSITORY_BUSY = "REPOSITORY_BUSY";
    public static final String CONFIGURATION_REQUIRED = "CONFIGURATION_REQUIRED";
    public static final String CONFLICT = "CONFLICT";
    public static final String INTERNAL_ERROR = "INTERNAL_ERROR";
    /** Project-root-bound server: the bound directory is not inside any registered repository. */
    public static final String PROJECT_NOT_REGISTERED = "PROJECT_NOT_REGISTERED";
    /** Project-root-bound server: the bound directory is missing or not a directory. */
    public static final String PROJECT_UNAVAILABLE = "PROJECT_UNAVAILABLE";
    /** Project-root-bound server: more than one registered repository claims the same root. */
    public static final String PROJECT_AMBIGUOUS = "PROJECT_AMBIGUOUS";
    /** Project-root-bound server: the bound directory now resolves to a different repository than earlier in this session. */
    public static final String PROJECT_SCOPE_CHANGED = "PROJECT_SCOPE_CHANGED";

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final int MAX_MESSAGE = 500;

    private McpErrors() {
    }

    /** Thrown inside tool handlers; converted to an isError result. */
    public static final class McpServiceException extends Exception {
        private final String code;
        private final String correlationId;

        public McpServiceException(String code, String message) {
            super(message);
            this.code = code;
            this.correlationId = UUID.randomUUID().toString();
        }

        public String code() {
            return code;
        }

        public String correlationId() {
            return correlationId;
        }
    }

    public static McpSchema.CallToolResult error(String code, String message, String correlationId) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("code", code);
        payload.put("message", sanitize(message));
        payload.put("correlation_id", correlationId);
        String text;
        try {
            text = MAPPER.writeValueAsString(payload);
        } catch (Exception e) {
            text = "{\"code\":\"" + code + "\",\"message\":\"unserializable error\"}";
        }
        return new McpSchema.CallToolResult(List.of(new McpSchema.TextContent(text)), true);
    }

    public static McpSchema.CallToolResult error(McpServiceException failure) {
        return error(failure.code(), failure.getMessage(), failure.correlationId());
    }

    public static McpSchema.CallToolResult ok(Map<String, Object> payload) {
        String text;
        try {
            text = MAPPER.writeValueAsString(payload);
        } catch (Exception e) {
            return error(INTERNAL_ERROR, "response serialization failed: " + e.getMessage(),
                    UUID.randomUUID().toString());
        }
        return new McpSchema.CallToolResult(List.of(new McpSchema.TextContent(text)), false);
    }

    static String sanitize(String message) {
        if (message == null) {
            return "";
        }
        StringBuilder clean = new StringBuilder();
        for (int i = 0; i < message.length() && clean.length() < MAX_MESSAGE; i++) {
            char c = message.charAt(i);
            if (c == '\n' || c == '\r' || c == '\t') {
                clean.append(' ');
            } else if (c < 0x20 || c == 0x7F) {
                clean.append('?');
            } else {
                clean.append(c);
            }
        }
        return clean.toString().strip();
    }
}

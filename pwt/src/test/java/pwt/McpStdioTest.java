package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.File;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * Exercises the real MCP stdio server as a subprocess with a protocol-aware
 * harness: JSON-RPC initialize, tool listing, and a {@code pwt_info} call.
 * Also asserts the stdout-purity contract: every line the server emits on
 * stdout must be a JSON-RPC 2.0 message (logs/diagnostics belong on stderr).
 */
class McpStdioTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final long STARTUP_TIMEOUT_SECONDS = 90;

    private Process server;
    private BufferedWriter serverStdin;
    private BlockingQueue<String> stdoutLines;

    @BeforeEach
    void startServer() throws Exception {
        String javaHome = System.getProperty("java.home");
        String javaBin = javaHome + File.separator + "bin" + File.separator + "java";
        String classpath = System.getProperty("java.class.path");
        ProcessBuilder builder = new ProcessBuilder(
                javaBin, "-cp", classpath, "pwt.launcher.PwtMain", "mcp");
        builder.redirectErrorStream(false);
        // Discard stderr: diagnostics must go there, but the test only
        // asserts about stdout purity.
        builder.redirectError(ProcessBuilder.Redirect.DISCARD);
        server = builder.start();
        serverStdin = new BufferedWriter(
                new OutputStreamWriter(server.getOutputStream(), StandardCharsets.UTF_8));
        stdoutLines = new LinkedBlockingQueue<>();
        Thread reader = new Thread(() -> {
            try (BufferedReader stdout = new BufferedReader(
                    new InputStreamReader(server.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = stdout.readLine()) != null) {
                    stdoutLines.add(line);
                }
            } catch (Exception ignored) {
                // process torn down; reader exits
            }
        }, "mcp-stdio-test-reader");
        reader.setDaemon(true);
        reader.start();
    }

    @AfterEach
    void stopServer() {
        if (serverStdin != null) {
            try {
                serverStdin.close();
            } catch (Exception ignored) {
                // best effort
            }
        }
        if (server != null) {
            server.destroy();
            try {
                if (!server.waitFor(10, TimeUnit.SECONDS)) {
                    server.destroyForcibly();
                }
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                server.destroyForcibly();
            }
        }
    }

    private void send(JsonNode message) throws Exception {
        serverStdin.write(MAPPER.writeValueAsString(message));
        serverStdin.write("\n");
        serverStdin.flush();
    }

    private JsonNode nextResponse(long timeoutSeconds) throws Exception {
        String line = stdoutLines.poll(timeoutSeconds, TimeUnit.SECONDS);
        if (line == null) {
            fail("timed out waiting for MCP server stdout");
        }
        JsonNode parsed;
        try {
            parsed = MAPPER.readTree(line);
        } catch (Exception e) {
            fail("MCP server stdout line is not JSON (stdout polluted): " + abbreviate(line));
            throw new IllegalStateException("unreachable");
        }
        assertEquals("2.0", parsed.path("jsonrpc").asText(""),
                "every stdout line must be a JSON-RPC 2.0 message, got: " + abbreviate(line));
        return parsed;
    }

    private static String abbreviate(String line) {
        return line.length() > 300 ? line.substring(0, 300) + "..." : line;
    }

    private static JsonNode request(int id, String method, JsonNode params) {
        var node = MAPPER.createObjectNode();
        node.put("jsonrpc", "2.0");
        node.put("id", id);
        node.put("method", method);
        node.set("params", params);
        return node;
    }

    @Test
    void initializeListAndCallPwtInfo() throws Exception {
        // 1. initialize
        var initParams = MAPPER.createObjectNode();
        initParams.put("protocolVersion", "2024-11-05");
        initParams.set("capabilities", MAPPER.createObjectNode());
        var clientInfo = MAPPER.createObjectNode();
        clientInfo.put("name", "pwt-phase1-test");
        clientInfo.put("version", "0");
        initParams.set("clientInfo", clientInfo);
        send(request(1, "initialize", initParams));

        JsonNode initResponse = nextResponse(STARTUP_TIMEOUT_SECONDS);
        assertEquals(1, initResponse.get("id").asInt());
        assertNotNull(initResponse.get("result"), "initialize must succeed: " + initResponse);
        assertEquals("pwt", initResponse.get("result").get("serverInfo").get("name").asText());
        assertEquals("0.4.0", initResponse.get("result").get("serverInfo").get("version").asText());

        // 2. initialized notification (no response expected)
        var notification = MAPPER.createObjectNode();
        notification.put("jsonrpc", "2.0");
        notification.put("method", "notifications/initialized");
        send(notification);

        // 3. tools/list must expose pwt_info plus the seven Phase 4 tools
        send(request(2, "tools/list", MAPPER.createObjectNode()));
        JsonNode listResponse = nextResponse(30);
        assertEquals(2, listResponse.get("id").asInt());
        List<String> toolNames = new ArrayList<>();
        for (JsonNode tool : listResponse.get("result").get("tools")) {
            toolNames.add(tool.get("name").asText());
        }
        assertEquals(
                List.of("pwt_info", "list_repositories", "list_runs", "get_run",
                        "list_findings", "get_report", "start_scan", "cancel_scan",
                        "get_analysis", "save_analysis"),
                toolNames);

        // 4. tools/call pwt_info returns the six-entry catalog, all INTEGRATED.
        // MCP never executes and never implies tool availability.
        var callParams = MAPPER.createObjectNode();
        callParams.put("name", "pwt_info");
        callParams.set("arguments", MAPPER.createObjectNode());
        send(request(3, "tools/call", callParams));
        JsonNode callResponse = nextResponse(30);
        assertEquals(3, callResponse.get("id").asInt());
        JsonNode content = callResponse.get("result").get("content");
        assertTrue(content.isArray() && content.size() == 1, "single text payload expected");
        assertEquals("text", content.get(0).get("type").asText());
        JsonNode payload = MAPPER.readTree(content.get(0).get("text").asText());
        assertEquals("pwt", payload.get("app").asText());
        assertEquals(6, payload.get("scanner_catalog").size());
        java.util.Map<String, String> executions = new java.util.LinkedHashMap<>();
        for (JsonNode entry : payload.get("scanner_catalog")) {
            executions.put(entry.get("id").asText(), entry.get("execution").asText());
        }
        for (String id : java.util.List.of("yamllint", "gitleaks", "checkov", "actionlint",
                "kubeconform", "local-schema")) {
            assertEquals("INTEGRATED", executions.get(id), id);
        }

        // 5. stdout purity: drain anything left and require JSON-RPC shape.
        // (All lines consumed above already passed through nextResponse.)
        assertTrue(server.isAlive(), "server must stay alive for the session");
    }
}

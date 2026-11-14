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
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Protocol-aware harness for the real MCP stdio server subprocess.
 * Asserts the stdout contract on every line (pure JSON-RPC 2.0) and captures
 * stderr to a file for secret-hygiene assertions.
 */
class McpHarness implements AutoCloseable {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final Process server;
    private final BufferedWriter stdin;
    private final BlockingQueue<String> stdoutLines = new LinkedBlockingQueue<>();
    private final Path stderrFile;
    private int nextId = 1;

    McpHarness(Path dataDir, boolean allowScans, Map<String, String> extraEnv) throws Exception {
        String javaBin = System.getProperty("java.home") + File.separator + "bin" + File.separator + "java";
        List<String> command = new ArrayList<>();
        command.add(javaBin);
        command.add("-cp");
        command.add(System.getProperty("java.class.path"));
        command.add("pwt.launcher.PwtMain");
        command.add("mcp");
        command.add("--data-dir");
        command.add(dataDir.toString());
        if (allowScans) {
            command.add("--allow-scans");
        }
        ProcessBuilder builder = new ProcessBuilder(command);
        if (extraEnv != null) {
            builder.environment().putAll(extraEnv);
        }
        stderrFile = Files.createTempFile("pwt-mcp-stderr", ".log");
        builder.redirectError(stderrFile.toFile());
        server = builder.start();
        stdin = new BufferedWriter(new OutputStreamWriter(server.getOutputStream(), StandardCharsets.UTF_8));
        Thread reader = new Thread(() -> {
            try (BufferedReader stdout = new BufferedReader(
                    new InputStreamReader(server.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = stdout.readLine()) != null) {
                    stdoutLines.add(line);
                }
            } catch (Exception ignored) {
                // torn down
            }
        }, "mcp-harness-reader");
        reader.setDaemon(true);
        reader.start();
        initialize();
    }

    McpHarness(Path dataDir, boolean allowScans) throws Exception {
        this(dataDir, allowScans, null);
    }

    McpHarness(Path dataDir, boolean allowScans, boolean allowAnalysis, String repoId) throws Exception {
        this(dataDir, allowScans, allowAnalysis, repoId, null);
    }

    McpHarness(Path dataDir, boolean allowScans, boolean allowAnalysis, String repoId,
            Map<String, String> extraEnv) throws Exception {
        String javaBin = System.getProperty("java.home") + File.separator + "bin" + File.separator + "java";
        List<String> command = new ArrayList<>();
        command.add(javaBin);
        command.add("-cp");
        command.add(System.getProperty("java.class.path"));
        command.add("pwt.launcher.PwtMain");
        command.add("mcp");
        command.add("--data-dir");
        command.add(dataDir.toString());
        if (allowScans) {
            command.add("--allow-scans");
        }
        if (allowAnalysis) {
            command.add("--allow-analysis-writes");
        }
        if (repoId != null) {
            command.add("--repo-id");
            command.add(repoId);
        }
        ProcessBuilder builder = new ProcessBuilder(command);
        if (extraEnv != null) {
            builder.environment().putAll(extraEnv);
        }
        stderrFile = Files.createTempFile("pwt-mcp-stderr", ".log");
        builder.redirectError(stderrFile.toFile());
        server = builder.start();
        stdin = new BufferedWriter(new OutputStreamWriter(server.getOutputStream(), StandardCharsets.UTF_8));
        Thread reader = new Thread(() -> {
            try (BufferedReader stdout = new BufferedReader(
                    new InputStreamReader(server.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = stdout.readLine()) != null) {
                    stdoutLines.add(line);
                }
            } catch (Exception ignored) {
                // torn down
            }
        }, "mcp-harness-reader");
        reader.setDaemon(true);
        reader.start();
        initialize();
    }

    private void initialize() throws Exception {
        var params = MAPPER.createObjectNode();
        params.put("protocolVersion", "2024-11-05");
        params.set("capabilities", MAPPER.createObjectNode());
        var clientInfo = MAPPER.createObjectNode();
        clientInfo.put("name", "pwt-mcp-tools-test");
        clientInfo.put("version", "0");
        params.set("clientInfo", clientInfo);
        JsonNode response = request("initialize", params, 60);
        assertNotNull(response.get("result"), "initialize must succeed");
        var notification = MAPPER.createObjectNode();
        notification.put("jsonrpc", "2.0");
        notification.put("method", "notifications/initialized");
        send(notification);
    }

    synchronized JsonNode call(String tool, Map<String, Object> params) throws Exception {
        var callParams = MAPPER.createObjectNode();
        callParams.put("name", tool);
        callParams.set("arguments", MAPPER.valueToTree(params == null ? Map.of() : params));
        return request("tools/call", callParams, 60);
    }

    private synchronized void send(JsonNode message) throws Exception {
        stdin.write(MAPPER.writeValueAsString(message));
        stdin.write("\n");
        stdin.flush();
    }

    private JsonNode request(String method, JsonNode params, long timeoutSeconds) throws Exception {
        var node = MAPPER.createObjectNode();
        node.put("jsonrpc", "2.0");
        node.put("id", nextId++);
        node.put("method", method);
        node.set("params", params);
        send(node);
        return nextResponse(timeoutSeconds);
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
            fail("MCP stdout line is not JSON (protocol polluted): " + abbreviate(line));
            throw new IllegalStateException("unreachable");
        }
        assertEquals("2.0", parsed.path("jsonrpc").asText(""),
                "every stdout line must be JSON-RPC 2.0, got: " + abbreviate(line));
        return parsed;
    }

    /** Calls a tool and returns the parsed success payload (fails on isError). */
    JsonNode callResult(String tool, Map<String, Object> params) throws Exception {
        JsonNode response = call(tool, params);
        assertNotNull(response.get("result"), "expected success result, got: " + response);
        JsonNode content = response.get("result").get("content");
        assertTrue(content.isArray() && content.size() == 1);
        assertEquals("text", content.get(0).get("type").asText());
        return MAPPER.readTree(content.get(0).get("text").asText());
    }

    /** Calls a tool and returns the parsed error payload (fails on success). */
    JsonNode callError(String tool, Map<String, Object> params) throws Exception {
        JsonNode response = call(tool, params);
        assertNotNull(response.get("error"), "expected JSON-RPC error, got: " + response);
        return response.get("error");
    }

    /** Calls a tool and returns the parsed isError=true tool payload. */
    JsonNode callToolError(String tool, Map<String, Object> params) throws Exception {
        JsonNode response = call(tool, params);
        assertNotNull(response.get("result"), "expected result, got: " + response);
        JsonNode content = response.get("result").get("content");
        JsonNode first = content.get(0);
        assertTrue(response.get("result").path("isError").asBoolean(false),
                "expected isError tool result, got: " + response);
        return MAPPER.readTree(first.get("text").asText());
    }

    String stderrText() throws Exception {
        return Files.readString(stderrFile, StandardCharsets.UTF_8);
    }

    boolean isAlive() {
        return server.isAlive();
    }

    /** Abrupt kill for crash simulation: no stdin close, no graceful shutdown. */
    public void abort() {
        server.destroyForcibly();
    }

    private static String abbreviate(String line) {
        return line.length() > 300 ? line.substring(0, 300) + "..." : line;
    }

    @Override
    public void close() {
        try {
            stdin.close();
        } catch (Exception ignored) {
            // best effort
        }
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

    static Map<String, Object> params(Object... keysAndValues) {
        Map<String, Object> params = new HashMap<>();
        for (int i = 0; i + 1 < keysAndValues.length; i += 2) {
            params.put((String) keysAndValues[i], keysAndValues[i + 1]);
        }
        return params;
    }
}

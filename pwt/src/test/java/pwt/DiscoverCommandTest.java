package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.application.DiscoveryException;
import pwt.launcher.DiscoverCommand;

class DiscoverCommandTest {

    @TempDir
    Path temp;

    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Test
    void missingRootIsInvalidInput() {
        assertThrows(DiscoveryException.class, () -> DiscoverCommand.parseArgs(List.of()));
    }

    @Test
    void unknownArgumentIsInvalidInput() {
        assertThrows(DiscoveryException.class,
                () -> DiscoverCommand.parseArgs(List.of("--frobnicate")));
    }

    @Test
    void invalidMaxFilesIsInvalidInput() {
        assertThrows(DiscoveryException.class,
                () -> DiscoverCommand.parseArgs(List.of("--root", temp.toString(), "--max-files", "0")));
        assertThrows(DiscoveryException.class,
                () -> DiscoverCommand.parseArgs(List.of("--root", temp.toString(), "--max-files", "many")));
    }

    @Test
    void nonexistentRootExitsTwo() {
        int code = DiscoverCommand.execute(List.of("--root", temp.resolve("missing").toString()));
        assertEquals(2, code);
    }

    @Test
    void fileAsRootExitsTwo() throws Exception {
        Path file = temp.resolve("f.txt");
        Files.write(file, "x".getBytes(StandardCharsets.UTF_8));
        assertEquals(2, DiscoverCommand.execute(List.of("--root", file.toString())));
    }

    @Test
    void discoveryEmitsStructuredJsonOnStdout() throws Exception {
        Path root = temp.resolve("repo");
        Files.createDirectories(root.resolve("sub"));
        Files.write(root.resolve("a.yaml"), "x: 1".getBytes(StandardCharsets.UTF_8));
        Files.write(root.resolve("sub").resolve("B.YML"), "y: 2".getBytes(StandardCharsets.UTF_8));
        Files.write(root.resolve("skip.txt"), "z".getBytes(StandardCharsets.UTF_8));

        PrintStream original = System.out;
        ByteArrayOutputStream captured = new ByteArrayOutputStream();
        System.setOut(new PrintStream(captured, true, StandardCharsets.UTF_8));
        int code;
        try {
            code = DiscoverCommand.execute(List.of("--root", root.toString()));
        } finally {
            System.setOut(original);
        }
        assertEquals(0, code);
        JsonNode json = MAPPER.readTree(captured.toString(StandardCharsets.UTF_8).trim());
        assertEquals(2, json.get("discovered").asInt());
        assertEquals(false, json.get("incomplete").asBoolean());
        assertEquals(2, json.get("files").size());
        assertEquals("a.yaml", json.get("files").get(0).get("path").asText());
        assertEquals("sub/B.YML", json.get("files").get(1).get("path").asText());
        assertTrue(json.get("root").asText().endsWith("repo"));
    }

    @Test
    void exclusionFlagIsHonoured() throws Exception {
        Path root = temp.resolve("repo");
        Files.createDirectories(root.resolve("vendor"));
        Files.write(root.resolve("keep.yaml"), "x: 1".getBytes(StandardCharsets.UTF_8));
        Files.write(root.resolve("vendor").resolve("v.yaml"), "x: 1".getBytes(StandardCharsets.UTF_8));

        PrintStream original = System.out;
        ByteArrayOutputStream captured = new ByteArrayOutputStream();
        System.setOut(new PrintStream(captured, true, StandardCharsets.UTF_8));
        int code;
        try {
            code = DiscoverCommand.execute(List.of("--root", root.toString(), "--exclude", "vendor"));
        } finally {
            System.setOut(original);
        }
        assertEquals(0, code);
        JsonNode json = MAPPER.readTree(captured.toString(StandardCharsets.UTF_8).trim());
        assertEquals(1, json.get("discovered").asInt());
        assertEquals("keep.yaml", json.get("files").get(0).get("path").asText());
    }
}

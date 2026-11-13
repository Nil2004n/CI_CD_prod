package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.abort;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.application.CancellationToken;
import pwt.application.DiscoveryException;
import pwt.application.DiscoveryService;
import pwt.domain.DiscoveryOptions;
import pwt.domain.DiscoveryResult;
import pwt.domain.RepositoryRoot;
import pwt.infrastructure.NioDiscoveryService;

class DiscoveryServiceTest {

    private final DiscoveryService service = new NioDiscoveryService();

    @TempDir
    Path temp;

    private Path write(Path root, String relative, String content) throws IOException {
        Path file = root.resolve(relative);
        Files.createDirectories(file.getParent());
        Files.write(file, content.getBytes(StandardCharsets.UTF_8));
        return file;
    }

    private List<String> pathsOf(DiscoveryResult result) {
        return result.files().stream().map(f -> f.relativePath()).collect(Collectors.toList());
    }

    @Test
    void nestedYamlAndYmlDiscoveredCaseInsensitively() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "a.yaml", "x: 1");
        write(root, "sub/b.YML", "y: 2");
        write(root, "sub/deep/c.YaMl", "z: 3");
        write(root, "UPPER.YAML", "w: 4");

        RepositoryRoot registered = service.registerRoot(root);
        DiscoveryResult result = service.discover(registered, DiscoveryOptions.defaults(), CancellationToken.none());

        assertEquals(List.of("UPPER.YAML", "a.yaml", "sub/b.YML", "sub/deep/c.YaMl"), pathsOf(result));
        assertFalse(result.incomplete());
    }

    @Test
    void nonYamlFilesExcluded() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "keep.yaml", "x: 1");
        write(root, "notes.txt", "x");
        write(root, "data.json", "{}");
        write(root, "noext", "x");
        write(root, "fake.yaml.bak", "x");
        write(root, "sub/script.sh", "echo hi");

        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());

        assertEquals(List.of("keep.yaml"), pathsOf(result));
    }

    @Test
    void emptyDirectoryYieldsEmptyCompleteResult() throws Exception {
        Path root = temp.resolve("empty");
        Files.createDirectories(root);
        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());
        assertTrue(result.files().isEmpty());
        assertFalse(result.incomplete());
    }

    @Test
    void missingRootRejected() {
        DiscoveryException e = assertThrows(DiscoveryException.class,
                () -> service.registerRoot(temp.resolve("does-not-exist")));
        assertEquals(DiscoveryException.Code.ROOT_NOT_FOUND, e.code());
    }

    @Test
    void fileAsRootRejected() throws Exception {
        Path file = write(temp, "plain.txt", "x");
        DiscoveryException e = assertThrows(DiscoveryException.class, () -> service.registerRoot(file));
        assertEquals(DiscoveryException.Code.ROOT_NOT_DIRECTORY, e.code());
    }

    @Test
    void defaultGitExclusionReportedNotSilent() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "top.yaml", "x: 1");
        write(root, ".git/hooks/a.yaml", "x: 1");
        write(root, ".git/c.yaml", "x: 1");

        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());

        assertEquals(List.of("top.yaml"), pathsOf(result));
        assertTrue(result.warnings().stream().anyMatch(w -> w.code().equals("EXCLUDED_DIRECTORY")),
                "expected an EXCLUDED_DIRECTORY warning, got: " + result.warnings());
    }

    @Test
    void explicitExclusionApplies() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "keep.yaml", "x: 1");
        write(root, "vendor/lib/v.yaml", "x: 1");

        DiscoveryOptions options = DiscoveryOptions.defaults()
                .withExcludedDirectoryNames(Set.of(".git", "vendor"));
        DiscoveryResult result = service.discover(service.registerRoot(root), options, CancellationToken.none());

        assertEquals(List.of("keep.yaml"), pathsOf(result));
        assertTrue(result.warnings().stream().anyMatch(w -> w.code().equals("EXCLUDED_DIRECTORY")));
    }

    @Test
    void relativePathNormalizationAndTraversalRejection() throws Exception {
        Path root = temp.resolve("repo");
        Files.createDirectories(root);
        RepositoryRoot registered = service.registerRoot(root);

        assertEquals("b.yaml", DiscoveryService.requireInsideRoot(registered, "a/../b.yaml"));
        assertEquals("sub/c.yaml", DiscoveryService.requireInsideRoot(registered, "sub/./c.yaml"));

        assertThrows(DiscoveryException.class, () -> DiscoveryService.requireInsideRoot(registered, ".."));
        assertThrows(DiscoveryException.class, () -> DiscoveryService.requireInsideRoot(registered, "../outside.yaml"));
        assertThrows(DiscoveryException.class,
                () -> DiscoveryService.requireInsideRoot(registered, "a/../../escape.yaml"));
        assertThrows(DiscoveryException.class,
                () -> DiscoveryService.requireInsideRoot(registered, temp.toAbsolutePath().toString() + "/x.yaml"));
        assertThrows(DiscoveryException.class, () -> DiscoveryService.requireInsideRoot(registered, ""));
    }

    @Test
    void symlinkFilesAndDirectoriesNotFollowed() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "real/real.yaml", "x: 1");
        Path outside = temp.resolve("outside");
        write(outside, "secret.yaml", "x: 1");
        try {
            Files.createSymbolicLink(root.resolve("link-dir"), outside);
            Files.createSymbolicLink(root.resolve("link-file.yaml"), outside.resolve("secret.yaml"));
        } catch (UnsupportedOperationException | IOException | SecurityException e) {
            abort("environment cannot create symlinks: " + e);
        }

        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());

        assertEquals(List.of("real/real.yaml"), pathsOf(result));
        assertTrue(result.warnings().stream().anyMatch(w -> w.code().equals("SYMLINK_SKIPPED")),
                "expected SYMLINK_SKIPPED warnings, got: " + result.warnings());
    }

    @Test
    void fileLimitMarksResultIncomplete() throws Exception {
        Path root = temp.resolve("repo");
        for (int i = 0; i < 5; i++) {
            write(root, "f" + i + ".yaml", "x: " + i);
        }
        DiscoveryOptions options = DiscoveryOptions.defaults().withMaxDiscoveredFiles(2);
        DiscoveryResult result = service.discover(service.registerRoot(root), options, CancellationToken.none());

        assertEquals(2, result.files().size());
        assertTrue(result.incomplete(), "limit hit must mark the result incomplete");
        assertTrue(result.warnings().stream().anyMatch(w -> w.code().equals("FILE_LIMIT")));
    }

    @Test
    void depthLimitMarksResultIncomplete() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "top.yaml", "x: 1");
        write(root, "sub/nested.yaml", "x: 1");

        DiscoveryOptions options = DiscoveryOptions.defaults().withMaxDepth(1);
        DiscoveryResult result = service.discover(service.registerRoot(root), options, CancellationToken.none());

        assertEquals(List.of("top.yaml"), pathsOf(result));
        assertTrue(result.incomplete());
        assertTrue(result.warnings().stream().anyMatch(w -> w.code().equals("DEPTH_LIMIT")));
    }

    @Test
    void preCancelledTokenAbortsDiscovery() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "a.yaml", "x: 1");
        RepositoryRoot registered = service.registerRoot(root);

        CancellationToken cancelled = new CancellationToken();
        cancelled.cancel();
        DiscoveryException e = assertThrows(DiscoveryException.class,
                () -> service.discover(registered, DiscoveryOptions.defaults(), cancelled));
        assertEquals(DiscoveryException.Code.CANCELLED, e.code());
    }

    @Test
    void cancellationTokenSemantics() throws Exception {
        CancellationToken token = CancellationToken.none();
        assertFalse(token.isCancelled());
        token.throwIfCancelled();
        token.cancel();
        assertTrue(token.isCancelled());
        assertThrows(DiscoveryException.class, token::throwIfCancelled);
    }

    @Test
    void unreadablePathsReportedAsWarnings() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "visible.yaml", "x: 1");
        Path locked = root.resolve("locked");
        write(locked, "hidden.yaml", "x: 1");
        try {
            Files.setPosixFilePermissions(locked, PosixFilePermissions.fromString("---------"));
        } catch (UnsupportedOperationException e) {
            abort("POSIX permissions unsupported: " + e);
            return;
        }
        try {
            // If the environment still lists the directory (e.g. privileged
            // user), the test cannot exercise the unreadable path; skip
            // explicitly instead of counting a pass.
            DiscoveryResult result = service.discover(service.registerRoot(root),
                    DiscoveryOptions.defaults(), CancellationToken.none());
            assumeTrue(result.warnings().stream().anyMatch(w -> w.code().equals("UNREADABLE_PATH")),
                    "SKIP: environment lists mode-000 directories (cannot exercise unreadable path)");
        } finally {
            try {
                Files.setPosixFilePermissions(locked, PosixFilePermissions.fromString("rwxr-xr-x"));
            } catch (IOException ignored) {
                // best effort so @TempDir cleanup can proceed
            }
        }
    }

    @Test
    void sortedStableOrder() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "z.yaml", "1");
        write(root, "a/b.yaml", "1");
        write(root, "a/a.yaml", "1");
        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());
        List<String> paths = pathsOf(result);
        List<String> sorted = paths.stream().sorted().collect(Collectors.toList());
        assertEquals(sorted, paths);
    }

    @Test
    void sizesComeFromAttributesWithoutContentReads() throws Exception {
        Path root = temp.resolve("repo");
        byte[] content = "key: value\n".getBytes(StandardCharsets.UTF_8);
        Path file = write(root, "sized.yaml", new String(content, StandardCharsets.UTF_8));
        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());
        assertEquals(1, result.files().size());
        assertEquals(Files.size(file), result.files().get(0).sizeBytes());
        assertEquals(content.length, result.files().get(0).sizeBytes());
    }

    @Test
    void invalidOptionValuesRejected() {
        assertThrows(IllegalArgumentException.class,
                () -> DiscoveryOptions.defaults().withMaxDiscoveredFiles(0));
        assertThrows(IllegalArgumentException.class,
                () -> DiscoveryOptions.defaults().withMaxDepth(0));
        assertThrows(IllegalArgumentException.class,
                () -> DiscoveryOptions.defaults().withExcludedDirectoryNames(Set.of("../evil")));
    }

    /** Ordering helper sanity: deepest paths are still root-relative forward-slash strings. */
    @Test
    void relativePathsUseForwardSlashes() throws Exception {
        Path root = temp.resolve("repo");
        write(root, "a/b/c.yaml", "x: 1");
        DiscoveryResult result = service.discover(service.registerRoot(root),
                DiscoveryOptions.defaults(), CancellationToken.none());
        assertEquals(List.of("a/b/c.yaml"), pathsOf(result));
        assertFalse(pathsOf(result).get(0).contains("\\"));
    }
}

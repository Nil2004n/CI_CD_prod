package pwt.infrastructure;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.HexFormat;

/**
 * Atomic report publication: write to a temporary file inside the
 * destination directory, flush, then rename over the final name where the
 * filesystem supports it. Returns content hash and size for artifact records.
 */
public final class ReportWriter {

    public record WrittenFile(Path path, String sha256, long sizeBytes) {
    }

    private ReportWriter() {
    }

    public static WrittenFile writeAtomically(Path directory, String fileName, String content)
            throws IOException {
        Files.createDirectories(directory);
        byte[] bytes = content.getBytes(StandardCharsets.UTF_8);
        String sha;
        try {
            sha = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (java.security.NoSuchAlgorithmException impossible) {
            throw new IOException("SHA-256 unavailable", impossible);
        }
        Path tmp = directory.resolve(fileName + ".tmp-" + java.util.UUID.randomUUID());
        Files.write(tmp, bytes);
        Path target = directory.resolve(fileName);
        try {
            Files.move(tmp, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } catch (AtomicMoveNotSupportedException e) {
            Files.move(tmp, target, StandardCopyOption.REPLACE_EXISTING);
        }
        return new WrittenFile(target, sha, bytes.length);
    }
}

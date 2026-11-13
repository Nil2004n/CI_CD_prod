package pwt.infrastructure;

import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;

/**
 * Basic cross-process, per-repository lock. The GUI and the headless command
 * share a data directory; overlapping scans of the same repository in that
 * directory are rejected with a busy message instead of running concurrently.
 * This is a mutual-exclusion lock, not a scheduler or daemon.
 */
public final class RepoLock implements AutoCloseable {

    private final RandomAccessFile file;
    private final FileChannel channel;
    private final FileLock lock;

    private RepoLock(RandomAccessFile file, FileChannel channel, FileLock lock) {
        this.file = file;
        this.channel = channel;
        this.lock = lock;
    }

    public static Optional<RepoLock> tryAcquire(Path dataDir, String identityHash) throws IOException {
        Files.createDirectories(dataDir.resolve("locks"));
        Path lockFile = dataDir.resolve("locks").resolve(identityHash + ".lock");
        RandomAccessFile raf = new RandomAccessFile(lockFile.toFile(), "rw");
        FileChannel channel = raf.getChannel();
        FileLock lock;
        try {
            lock = channel.tryLock();
        } catch (java.nio.channels.OverlappingFileLockException busy) {
            // Already locked in this JVM: treat as busy, not as an error.
            closeQuietly(channel, raf);
            return Optional.empty();
        } catch (IOException e) {
            channel.close();
            raf.close();
            throw e;
        }
        if (lock == null) {
            closeQuietly(channel, raf);
            return Optional.empty();
        }
        return Optional.of(new RepoLock(raf, channel, lock));
    }

    private static Optional<RepoLock> closeQuietly(FileChannel channel, RandomAccessFile raf) throws IOException {
        try {
            channel.close();
        } finally {
            raf.close();
        }
        return Optional.empty();
    }

    @Override
    public void close() {
        try {
            lock.release();
        } catch (Exception ignored) {
            // best effort
        }
        try {
            channel.close();
        } catch (Exception ignored) {
            // best effort
        }
        try {
            file.close();
        } catch (Exception ignored) {
            // best effort
        }
    }
}

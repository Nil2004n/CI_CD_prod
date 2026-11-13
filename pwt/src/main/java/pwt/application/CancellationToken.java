package pwt.application;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Cooperative cancellation handle shared between a caller (GUI cancel button,
 * CLI interrupt) and a running discovery traversal. Thread-safe.
 */
public final class CancellationToken {

    private final AtomicBoolean cancelled = new AtomicBoolean(false);

    public void cancel() {
        cancelled.set(true);
    }

    public boolean isCancelled() {
        return cancelled.get();
    }

    /** Throws {@link DiscoveryException} with code CANCELLED when set. */
    public void throwIfCancelled() throws DiscoveryException {
        if (isCancelled()) {
            throw new DiscoveryException(DiscoveryException.Code.CANCELLED, "Discovery cancelled by caller");
        }
    }

    public static CancellationToken none() {
        return new CancellationToken();
    }
}

package pwt.domain;

import java.util.Objects;

/** Result of probing one scanner executable (no scan performed). */
public record ToolProbe(boolean available, String executablePath, String version, String reason) {

    public ToolProbe {
        if (available) {
            Objects.requireNonNull(executablePath, "executablePath");
        }
    }

    public static ToolProbe available(String executablePath, String version) {
        return new ToolProbe(true, executablePath, version, null);
    }

    public static ToolProbe unavailable(String reason) {
        return new ToolProbe(false, null, null, reason);
    }
}

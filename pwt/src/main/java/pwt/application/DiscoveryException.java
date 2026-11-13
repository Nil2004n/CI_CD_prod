package pwt.application;

/** Checked failure for discovery operations. Cancellation is a Code, not a separate type. */
public class DiscoveryException extends Exception {

    public enum Code {
        ROOT_NOT_FOUND,
        ROOT_NOT_DIRECTORY,
        ROOT_NOT_READABLE,
        CANCELLED,
        IO_ERROR,
        INVALID_PATH
    }

    private final Code code;

    public DiscoveryException(Code code, String message) {
        super(message);
        this.code = code;
    }

    public DiscoveryException(Code code, String message, Throwable cause) {
        super(message, cause);
        this.code = code;
    }

    public Code code() {
        return code;
    }
}

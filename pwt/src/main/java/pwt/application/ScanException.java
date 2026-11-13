package pwt.application;

public class ScanException extends Exception {

    public enum Code {
        INVALID_INPUT,
        REPOSITORY_BUSY,
        STORAGE_ERROR,
        INTERRUPTED
    }

    private final Code code;

    public ScanException(Code code, String message) {
        super(message);
        this.code = code;
    }

    public ScanException(Code code, String message, Throwable cause) {
        super(message, cause);
        this.code = code;
    }

    public Code code() {
        return code;
    }
}

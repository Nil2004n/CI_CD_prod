package pwt.desktop;

/**
 * Sanitizes untrusted or arbitrary text before it reaches the GUI log panel.
 * Repository paths, warning messages, and exceptions are data, never
 * interpreted: control characters (other than tab) are replaced, and overlong
 * lines are truncated with an explicit marker. No JavaFX dependency, so this
 * is unit-testable headlessly.
 */
public final class EventSanitizer {

    /** Maximum characters kept per log line before truncation. */
    public static final int MAX_LINE_LENGTH = 500;

    private EventSanitizer() {
    }

    public static String sanitize(String input) {
        if (input == null) {
            return "";
        }
        StringBuilder clean = new StringBuilder(input.length());
        for (int i = 0; i < input.length(); i++) {
            char c = input.charAt(i);
            if (c == '\n' || c == '\r') {
                clean.append(' ');
            } else if (c == '\t') {
                clean.append('\t');
            } else if (c < 0x20 || c == 0x7F) {
                clean.append('\uFFFD');
            } else {
                clean.append(c);
            }
        }
        if (clean.length() > MAX_LINE_LENGTH) {
            return clean.substring(0, MAX_LINE_LENGTH) + " [...truncated]";
        }
        return clean.toString();
    }
}

package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;
import pwt.desktop.EventSanitizer;

class EventSanitizerTest {

    @Test
    void controlCharactersNeutralized() {
        String dirty = "path\u0000with\u001b[Bell\u0007and\ttab\nnewline";
        String clean = EventSanitizer.sanitize(dirty);
        assertFalse(clean.contains("\u0000"));
        assertFalse(clean.contains("\u001b"));
        assertFalse(clean.contains("\u0007"));
        assertTrue(clean.contains("\t"));
        assertFalse(clean.contains("\n"));
    }

    @Test
    void overlongLinesTruncatedWithMarker() {
        String longLine = "x".repeat(EventSanitizer.MAX_LINE_LENGTH + 50);
        String clean = EventSanitizer.sanitize(longLine);
        assertTrue(clean.endsWith("[...truncated]"));
        assertTrue(clean.length() < longLine.length());
    }

    @Test
    void nullAndOrdinaryTextPassThrough() {
        assertEquals("", EventSanitizer.sanitize(null));
        assertEquals("plain message [ok]", EventSanitizer.sanitize("plain message [ok]"));
    }
}

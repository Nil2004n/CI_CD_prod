package pwt.launcher;

/**
 * Minimal JSON writer for headless/CLI output. Supports the small object
 * shapes PWT Phase 1 emits; not a general JSON library. Strings are escaped
 * per RFC 8259; NaN/infinity are rejected.
 */
public final class JsonWriter {

    private final StringBuilder out = new StringBuilder();

    public JsonWriter beginObject() {
        out.append('{');
        return this;
    }

    public JsonWriter endObject() {
        trimComma();
        out.append('}');
        return this;
    }

    public JsonWriter beginArray(String key) {
        fieldKey(key);
        out.append('[');
        return this;
    }

    public JsonWriter endArray() {
        trimComma();
        out.append(']');
        out.append(',');
        return this;
    }

    public JsonWriter field(String key, String value) {
        fieldKey(key);
        if (value == null) {
            out.append("null,");
        } else {
            out.append('"').append(escape(value)).append("\",");
        }
        return this;
    }

    public JsonWriter field(String key, long value) {
        fieldKey(key);
        out.append(value).append(',');
        return this;
    }

    public JsonWriter field(String key, boolean value) {
        fieldKey(key);
        out.append(value ? "true" : "false").append(',');
        return this;
    }

    public JsonWriter objectField(String key, JsonWriter nested) {
        fieldKey(key);
        out.append(nested.buildFragment()).append(',');
        return this;
    }

    public JsonWriter arrayObject(JsonWriter element) {
        out.append(element.buildFragment()).append(',');
        return this;
    }

    private void fieldKey(String key) {
        out.append('"').append(escape(key)).append("\":");
    }

    private void trimComma() {
        int n = out.length();
        if (n > 0 && out.charAt(n - 1) == ',') {
            out.deleteCharAt(n - 1);
        }
    }

    private String buildFragment() {
        StringBuilder copy = new StringBuilder(out.toString());
        int n = copy.length();
        if (n > 0 && copy.charAt(n - 1) == ',') {
            copy.deleteCharAt(n - 1);
        }
        return copy.toString();
    }

    @Override
    public String toString() {
        return buildFragment();
    }

    public static String escape(String value) {
        StringBuilder escaped = new StringBuilder(value.length() + 8);
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '"' -> escaped.append("\\\"");
                case '\\' -> escaped.append("\\\\");
                case '\n' -> escaped.append("\\n");
                case '\r' -> escaped.append("\\r");
                case '\t' -> escaped.append("\\t");
                case '\b' -> escaped.append("\\b");
                case '\f' -> escaped.append("\\f");
                default -> {
                    if (c < 0x20) {
                        escaped.append(String.format("\\u%04x", (int) c));
                    } else {
                        escaped.append(c);
                    }
                }
            }
        }
        return escaped.toString();
    }
}

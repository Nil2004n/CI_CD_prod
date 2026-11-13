package pwt.application;

import java.nio.file.FileSystems;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * Explicit bindings for local user-supplied schema validation. Each binding
 * maps a glob (matched against repository-relative forward-slash paths) to a
 * local schema file. Supported dialects: {@code 2020-12} (default) and
 * {@code 2019-09}. Anything else is rejected before any scan runs.
 *
 * <p>A file with no matching binding is "no schema configured", never
 * "schema-valid".
 */
public final class LocalSchemaConfig {

    public static final String DIALECT_2020_12 = "2020-12";
    public static final String DIALECT_2019_09 = "2019-09";

    public record Binding(String glob, Path schemaPath) {
        public Binding {
            Objects.requireNonNull(glob, "glob");
            Objects.requireNonNull(schemaPath, "schemaPath");
            if (glob.isBlank()) {
                throw new IllegalArgumentException("Binding glob must not be blank");
            }
            if (!schemaPath.isAbsolute()) {
                throw new IllegalArgumentException("Binding schema path must be absolute: " + schemaPath);
            }
            try {
                FileSystems.getDefault().getPathMatcher("glob:" + glob);
            } catch (IllegalArgumentException e) {
                throw new IllegalArgumentException("Invalid binding glob: " + glob, e);
            }
        }

        public boolean matches(String relativePath) {
            return FileSystems.getDefault().getPathMatcher("glob:" + glob)
                    .matches(Path.of(relativePath));
        }
    }

    private final List<Binding> bindings;
    private final String dialect;

    public LocalSchemaConfig(List<Binding> bindings, String dialect) {
        Objects.requireNonNull(bindings, "bindings");
        Objects.requireNonNull(dialect, "dialect");
        if (!dialect.equals(DIALECT_2020_12) && !dialect.equals(DIALECT_2019_09)) {
            throw new IllegalArgumentException(
                    "Unsupported schema dialect: " + dialect + " (supported: 2020-12, 2019-09)");
        }
        this.bindings = List.copyOf(bindings);
        this.dialect = dialect;
    }

    public static LocalSchemaConfig none() {
        return new LocalSchemaConfig(List.of(), DIALECT_2020_12);
    }

    /** Parses {@code GLOB=ABSOLUTE_SCHEMA_PATH} entries; relative schema paths resolve against {@code cwd}. */
    public static LocalSchemaConfig parse(List<String> entries, String dialect, Path cwd) {
        List<Binding> bindings = new ArrayList<>();
        if (entries != null) {
            for (String entry : entries) {
                int equals = entry.indexOf('=');
                if (equals <= 0 || equals == entry.length() - 1) {
                    throw new IllegalArgumentException(
                            "Invalid --schema-binding (expected GLOB=SCHEMA_PATH): " + entry);
                }
                String glob = entry.substring(0, equals).trim();
                Path schema = Path.of(entry.substring(equals + 1).trim());
                if (!schema.isAbsolute()) {
                    schema = cwd.toAbsolutePath().resolve(schema).normalize();
                }
                bindings.add(new Binding(glob, schema));
            }
        }
        return new LocalSchemaConfig(bindings, dialect == null ? DIALECT_2020_12 : dialect);
    }

    public List<Binding> bindings() {
        return bindings;
    }

    public String dialect() {
        return dialect;
    }

    public boolean isEmpty() {
        return bindings.isEmpty();
    }

    /** All bindings whose glob matches the repository-relative path, in order. */
    public List<Binding> matching(String relativePath) {
        List<Binding> matched = new ArrayList<>();
        for (Binding binding : bindings) {
            try {
                if (binding.matches(relativePath)) {
                    matched.add(binding);
                }
            } catch (InvalidPathException ignored) {
                // unmatchable input path: treated as no match for this binding
            }
        }
        return matched;
    }
}

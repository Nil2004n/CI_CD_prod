package pwt.adapters;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.networknt.schema.JsonSchema;
import com.networknt.schema.JsonSchemaFactory;
import com.networknt.schema.SpecVersion;
import com.networknt.schema.ValidationMessage;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Date;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import pwt.application.LocalSchemaConfig;
import pwt.domain.Finding;
import pwt.domain.FindingCategory;
import pwt.domain.Fingerprints;
import pwt.domain.ScannerOutcome;
import pwt.domain.ScannerStatus;
import pwt.domain.Severity;
import pwt.domain.ToolProbe;

/**
 * Local user-supplied schema validation, implemented in Java (no external
 * executable — availability means the validator is loaded, the dialect is
 * known, and bindings are usable).
 *
 * <p>Dialects: {@code 2020-12} (default) and {@code 2019-09}, via the
 * networknt validator; anything else declared by a schema is unsupported.
 * YAML is parsed with SnakeYAML safe construction (bounded aliases/nesting,
 * duplicate keys rejected, no arbitrary object construction).
 *
 * <p>Separation of outcomes per file:
 * <ul>
 *   <li>no matching binding → not covered by this adapter ("no schema
 *       configured", never "schema-valid");</li>
 *   <li>instance YAML syntax failure → SYNTAX finding;</li>
 *   <li>schema loading/compilation failure (missing file, bad dialect,
 *       remote/local-escape reference) → coverage gap, never a clean
 *       result;</li>
 *   <li>validation failures → one SCHEMA finding per message.</li>
 * </ul>
 *
 * <p>Each document of a multi-document file is validated separately and the
 * document index is recorded. Findings carry the binding identity, document
 * index, JSON-Pointer instance path, and validation keyword — never line or
 * column numbers (not reliably available) and never rejected instance values
 * (which may contain secrets).
 *
 * <p>Reference policy: remote {@code http(s)} references are rejected
 * without fetching; local references resolve only within the schema file's
 * own directory and must not escape it.
 */
public final class LocalSchemaAdapter implements ScannerAdapter {

    public static final String ID = "local-schema";
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    public String id() {
        return ID;
    }

    @Override
    public String toolName() {
        return "local-schema";
    }

    @Override
    public ToolProbe probe() {
        try {
            JsonSchemaFactory factory = JsonSchemaFactory.getInstance(SpecVersion.VersionFlag.V202012);
            JsonSchema smoke = factory.getSchema("{\"type\":\"object\"}");
            var messages = smoke.validate(MAPPER.readTree("{\"a\":1}"));
            if (!messages.isEmpty()) {
                return ToolProbe.unavailable("bundled validator self-check failed");
            }
            return ToolProbe.available("java-validator",
                    "networknt validator loaded (dialects 2020-12, 2019-09)");
        } catch (Exception e) {
            return ToolProbe.unavailable("bundled validator unavailable: " + e.getMessage());
        }
    }

    @Override
    public ScannerOutcome execute(ScanContext context) {
        LocalSchemaConfig config = context.localSchema();
        if (config == null || config.isEmpty()) {
            return statusOnly(ScannerStatus.SKIPPED, validatorVersion(),
                    "No schema bindings configured: pass --schema-binding GLOB=SCHEMA_PATH. "
                            + "Unbound files are not schema-validated.");
        }
        if (!"2020-12".equals(config.dialect()) && !"2019-09".equals(config.dialect())) {
            return statusOnly(ScannerStatus.SKIPPED, validatorVersion(),
                    "Unsupported schema dialect: " + config.dialect() + " (supported: 2020-12, 2019-09)");
        }
        // Group selected targets by their first matching binding.
        Map<LocalSchemaConfig.Binding, List<String>> byBinding = new LinkedHashMap<>();
        for (String relative : context.relativeTargets()) {
            List<LocalSchemaConfig.Binding> matched = config.matching(relative);
            if (matched.isEmpty()) {
                continue;
            }
            byBinding.computeIfAbsent(matched.get(0), key -> new ArrayList<>()).add(relative);
        }
        if (byBinding.isEmpty()) {
            return ScannerOutcome.withoutGaps(ID, toolName(), validatorVersion(),
                    ScannerStatus.NOT_APPLICABLE, null,
                    "No selected file matches any schema binding; local-schema validation did not run.",
                    List.of());
        }
        List<Finding> findings = new ArrayList<>();
        List<String> gaps = new ArrayList<>();
        int validated = 0;
        for (Map.Entry<LocalSchemaConfig.Binding, List<String>> entry : byBinding.entrySet()) {
            LocalSchemaConfig.Binding binding = entry.getKey();
            CompiledBinding compiled;
            try {
                compiled = compileBinding(binding, config.dialect());
            } catch (BindingException e) {
                gaps.add("Binding '" + binding.glob() + "' unusable (" + e.getMessage() + "): "
                        + entry.getValue().size() + " file(s) not validated");
                continue;
            }
            for (String relative : entry.getValue()) {
                validated++;
                validateFile(context.stagedDir().resolve(relative), relative, binding, compiled, findings, gaps);
            }
        }
        if (validated == 0) {
            return statusOnly(ScannerStatus.SKIPPED, validatorVersion(),
                    "No file could be validated: " + String.join("; ", gaps));
        }
        String detail = validated + " file(s) validated against " + byBinding.size() + " binding(s), "
                + findings.size() + " violation(s)";
        if (!gaps.isEmpty()) {
            detail += "; gaps: " + String.join("; ", gaps);
        }
        return new ScannerOutcome(ID, toolName(), validatorVersion(), ScannerStatus.SUCCEEDED,
                null, detail, findings, List.copyOf(gaps));
    }

    private static String validatorVersion() {
        return "networknt validator (dialects 2020-12, 2019-09)";
    }

    private ScannerOutcome statusOnly(ScannerStatus status, String version, String detail) {
        return ScannerOutcome.withoutGaps(ID, toolName(), version, status, null, detail, List.of());
    }

    private record CompiledBinding(JsonSchema schema, String schemaName) {
    }

    private static final class BindingException extends Exception {
        BindingException(String message) {
            super(message);
        }
    }

    /** Loads, vets, and compiles one binding's schema. */
    static CompiledBinding compileBinding(LocalSchemaConfig.Binding binding, String dialect)
            throws BindingException {
        Path schemaPath = binding.schemaPath();
        if (!Files.isRegularFile(schemaPath)) {
            throw new BindingException("schema file not found: " + schemaPath);
        }
        String schemaText;
        try {
            byte[] bytes = Files.readAllBytes(schemaPath);
            if (bytes.length > 4 * 1024 * 1024) {
                throw new BindingException("schema file too large: " + schemaPath);
            }
            schemaText = new String(bytes, StandardCharsets.UTF_8);
        } catch (BindingException e) {
            throw e;
        } catch (Exception e) {
            throw new BindingException("schema file unreadable: " + e.getMessage());
        }
        JsonNode schemaNode;
        try {
            schemaNode = MAPPER.readTree(schemaText);
        } catch (Exception e) {
            throw new BindingException("schema is not valid JSON: " + firstFragment(e.getMessage()));
        }
        if (!schemaNode.isObject()) {
            throw new BindingException("schema root must be a JSON object");
        }
        String declaredDialect = schemaNode.path("$schema").asText(null);
        String effectiveDialect = dialect;
        if (declaredDialect != null) {
            if (declaredDialect.contains("2020-12")) {
                effectiveDialect = "2020-12";
            } else if (declaredDialect.contains("2019-09")) {
                effectiveDialect = "2019-09";
            } else {
                throw new BindingException("unsupported $schema dialect: " + declaredDialect);
            }
        }
        Path schemaRoot = schemaPath.toAbsolutePath().getParent();
        try {
            vetReferences(schemaNode, schemaRoot);
        } catch (BindingException e) {
            throw e;
        } catch (Exception e) {
            throw new BindingException("reference vetting failed: " + e.getMessage());
        }
        SpecVersion.VersionFlag flag = "2019-09".equals(effectiveDialect)
                ? SpecVersion.VersionFlag.V201909
                : SpecVersion.VersionFlag.V202012;
        try {
            JsonSchemaFactory factory = JsonSchemaFactory.getInstance(flag);
            JsonSchema schema = factory.getSchema(
                    com.networknt.schema.SchemaLocation.of(schemaPath.toUri().toString()),
                    com.networknt.schema.SchemaValidatorsConfig.builder().build());
            String name = schemaPath.getFileName() == null ? schemaPath.toString()
                    : schemaPath.getFileName().toString();
            return new CompiledBinding(schema, name);
        } catch (Exception e) {
            throw new BindingException("schema compilation failed: " + firstFragment(e.getMessage()));
        }
    }

    /**
     * Reference policy enforcement before compilation: rejects remote
     * {@code http(s)} references (never fetched) and local references that
     * escape the schema root directory.
     */
    public static void vetReferences(JsonNode schemaNode, Path schemaRoot) throws BindingException {
        List<String> refs = new ArrayList<>();
        collectRefs(schemaNode, refs);
        boolean hasRelativeRef = false;
        for (String ref : refs) {
            if (!ref.startsWith("#") && !ref.isEmpty() && !ref.matches("^[a-zA-Z][a-zA-Z0-9+.-]*:.*")) {
                hasRelativeRef = true;
                break;
            }
        }
        String id = schemaNode.path("$id").asText(null);
        if (hasRelativeRef && id != null && (id.startsWith("http://") || id.startsWith("https://"))) {
            throw new BindingException(
                    "remote $id would redirect relative references to the network: " + id);
        }
        for (String ref : refs) {
            if (ref.startsWith("http://") || ref.startsWith("https://")) {
                throw new BindingException("remote $ref is not fetched: " + ref);
            }
            if (ref.startsWith("#") || ref.isEmpty() || ref.startsWith("urn:")
                    || ref.matches("^[a-zA-Z][a-zA-Z0-9+.-]*:.*")) {
                if (ref.startsWith("#") || ref.isEmpty()) {
                    continue;
                }
                throw new BindingException("unsupported $ref form (local file refs only): " + ref);
            }
            String filePart = ref.split("#", 2)[0];
            if (filePart.isEmpty()) {
                continue;
            }
            Path resolved = schemaRoot.resolve(filePart).normalize();
            if (!resolved.startsWith(schemaRoot)) {
                throw new BindingException("local $ref escapes the schema root: " + ref);
            }
            if (!Files.isRegularFile(resolved)) {
                throw new BindingException("local $ref target missing: " + ref);
            }
        }
    }

    private static void collectRefs(JsonNode node, List<String> refs) {
        if (node.isObject()) {
            node.fields().forEachRemaining(entry -> {
                if (entry.getKey().equals("$ref") && entry.getValue().isTextual()) {
                    refs.add(entry.getValue().asText());
                } else {
                    collectRefs(entry.getValue(), refs);
                }
            });
        } else if (node.isArray()) {
            for (JsonNode child : node) {
                collectRefs(child, refs);
            }
        }
    }

    private static void validateFile(Path stagedFile, String relative,
            LocalSchemaConfig.Binding binding, CompiledBinding compiled,
            List<Finding> findings, List<String> gaps) {
        List<Object> documents;
        try {
            documents = YamlSniff.loadAllDocuments(stagedFile);
        } catch (Exception e) {
            findings.add(syntaxFinding(relative, binding, "YAML syntax failure: " + firstFragment(e.getMessage())));
            return;
        }
        int index = -1;
        for (Object document : documents) {
            index++;
            if (document == null) {
                continue;
            }
            JsonNode instance;
            try {
                instance = MAPPER.valueToTree(normalize(document));
            } catch (Exception e) {
                gaps.add(relative + " document " + index + ": value conversion failed ("
                        + firstFragment(e.getMessage()) + ")");
                continue;
            }
            Set<ValidationMessage> messages;
            try {
                messages = compiled.schema().validate(instance);
            } catch (Exception e) {
                gaps.add(relative + " document " + index + ": validation error ("
                        + firstFragment(e.getMessage()) + ")");
                continue;
            }
            for (ValidationMessage message : messages) {
                findings.add(violationFinding(relative, binding, compiled, index, message));
            }
        }
    }

    private static Finding violationFinding(String relative, LocalSchemaConfig.Binding binding,
            CompiledBinding compiled, int documentIndex, ValidationMessage message) {
        String keyword = message.getType() == null ? "validation" : message.getType();
        String pointer = message.getInstanceLocation() == null ? "$"
                : message.getInstanceLocation().toString();
        String text = "Does not satisfy " + compiled.schemaName() + " [" + binding.glob() + "]"
                + " document " + documentIndex + " at " + pointer + ": "
                + firstFragment(message.getMessage(), 300);
        String fingerprint = Fingerprints.v1(ID, binding.glob() + ":" + keyword, relative, documentIndex, null);
        String evidence = "{\"binding\":\"" + escape(binding.glob()) + "\","
                + "\"schema\":\"" + escape(compiled.schemaName()) + "\","
                + "\"document\":" + documentIndex + ","
                + "\"keyword\":\"" + escape(keyword) + "\","
                + "\"instance_path\":\"" + escape(pointer) + "\"}";
        return new Finding("localschema/" + sanitizeRule(keyword),
                "Schema violation (" + keyword + ")", Severity.MEDIUM, keyword,
                FindingCategory.SCHEMA, relative, null, null, null, text, evidence, fingerprint);
    }

    private static Finding syntaxFinding(String relative, LocalSchemaConfig.Binding binding, String detail) {
        String text = "YAML syntax failure in " + relative + " [" + binding.glob() + "]: " + detail;
        String fingerprint = Fingerprints.v1(ID, binding.glob() + ":syntax", relative, null, null);
        return new Finding("localschema/syntax", "YAML syntax failure", Severity.MEDIUM, "syntax",
                FindingCategory.SYNTAX, relative, null, null, null, text,
                "{\"binding\":\"" + escape(binding.glob()) + "\"}", fingerprint);
    }

    private static String sanitizeRule(String keyword) {
        String clean = keyword.replaceAll("[^A-Za-z0-9_-]", "-");
        return clean.isEmpty() ? "validation" : clean;
    }

    /** YAML values → JSON-compatible values. Dates become UTC ISO-8601 strings; binaries Base64. */
    static Object normalize(Object value) {
        if (value instanceof Map<?, ?> map) {
            Map<String, Object> normalized = new LinkedHashMap<>();
            for (Map.Entry<?, ?> entry : map.entrySet()) {
                normalized.put(String.valueOf(entry.getKey()), normalize(entry.getValue()));
            }
            return normalized;
        }
        if (value instanceof List<?> list) {
            List<Object> normalized = new ArrayList<>(list.size());
            for (Object item : list) {
                normalized.add(normalize(item));
            }
            return normalized;
        }
        if (value instanceof Date date) {
            return DateTimeFormatter.ISO_INSTANT.format(date.toInstant().atOffset(ZoneOffset.UTC));
        }
        if (value instanceof byte[] bytes) {
            return Base64.getEncoder().encodeToString(bytes);
        }
        return value;
    }

    private static String escape(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    private static String firstFragment(String text) {
        return firstFragment(text, 200);
    }

    private static String firstFragment(String text, int max) {
        if (text == null) {
            return "";
        }
        StringBuilder clean = new StringBuilder();
        for (int i = 0; i < text.length() && clean.length() < max; i++) {
            char c = text.charAt(i);
            if (c == '\n' || c == '\r') {
                clean.append(' ');
            } else if (c < 0x20 || c == 0x7F) {
                clean.append('?');
            } else {
                clean.append(c);
            }
        }
        String result = clean.toString().strip();
        return text.length() > max ? result + " [...]" : result;
    }
}

package pwt.adapters;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.yaml.snakeyaml.LoaderOptions;
import org.yaml.snakeyaml.Yaml;
import org.yaml.snakeyaml.constructor.SafeConstructor;

/**
 * Shared, conservative YAML content sniffing for applicability checks.
 * Safe construction only (no arbitrary object creation), bounded aliases,
 * no duplicate keys, bounded input size. A file the sniffer cannot parse is
 * reported as not recognized — never as checked.
 */
public final class YamlSniff {

    /** Cap on bytes read for sniffing; larger files are not recognized. */
    public static final int MAX_SNIFF_BYTES = 512 * 1024;
    /** Cap on documents inspected per file. */
    public static final int MAX_SNIFF_DOCS = 32;

    private YamlSniff() {
    }

    public static LoaderOptions safeOptions() {
        LoaderOptions options = new LoaderOptions();
        options.setAllowDuplicateKeys(false);
        options.setMaxAliasesForCollections(50);
        options.setNestingDepthLimit(50);
        options.setCodePointLimit(3 * 1024 * 1024);
        return options;
    }

    /**
     * Returns true when at least one YAML document in the file is a mapping
     * with string {@code apiVersion} and {@code kind} keys — the scalable
     * recognition rule for Kubernetes manifests used by the Checkov and
     * kubeconform adapters.
     */
    public static boolean isKubernetesManifest(Path file) {
        try {
            if (!Files.isRegularFile(file) || Files.size(file) > MAX_SNIFF_BYTES) {
                return false;
            }
            Yaml yaml = new Yaml(new SafeConstructor(safeOptions()));
            int docs = 0;
            try (var reader = Files.newBufferedReader(file)) {
                for (Object document : yaml.loadAll(reader)) {
                    if (++docs > MAX_SNIFF_DOCS) {
                        break;
                    }
                    if (document instanceof Map<?, ?> mapping) {
                        Object apiVersion = mapping.get("apiVersion");
                        Object kind = mapping.get("kind");
                        if (apiVersion instanceof String && kind instanceof String) {
                            return true;
                        }
                    }
                }
            }
            return false;
        } catch (Exception e) {
            return false;
        }
    }

    /** Loads all documents of a file for validation; throws on any load failure. */
    public static List<Object> loadAllDocuments(Path file) throws Exception {
        if (!Files.isRegularFile(file) || Files.size(file) > 8 * 1024 * 1024) {
            throw new IllegalArgumentException("File too large or not a regular file: " + file);
        }
        Yaml yaml = new Yaml(new SafeConstructor(safeOptions()));
        List<Object> documents = new ArrayList<>();
        try (var reader = Files.newBufferedReader(file)) {
            for (Object document : yaml.loadAll(reader)) {
                documents.add(document);
            }
        }
        return documents;
    }
}

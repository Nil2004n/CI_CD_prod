package pwt.adapters;

import java.util.List;
import pwt.domain.ExecutionState;
import pwt.domain.ScannerDescriptor;

/**
 * The full intended scanner/validation scope for the PWT rebuild:
 * yamllint, Gitleaks, Checkov, actionlint, kubeconform, and local
 * user-supplied schema validation.
 *
 * <p>Phase 3 integrates Checkov, actionlint, kubeconform, and local-schema
 * validation: those entries report {@link ExecutionState#INTEGRATED}
 * (availability still determined per run by probing the local executable or,
 * for local-schema, the bundled validator). The catalog preserves all six
 * intended integrations.
 */
public final class ScannerCatalog {

    public static final String YAMLLINT = "yamllint";
    public static final String GITLEAKS = "gitleaks";
    public static final String CHECKOV = "checkov";
    public static final String ACTIONLINT = "actionlint";
    public static final String KUBECONFORM = "kubeconform";
    public static final String LOCAL_SCHEMA = "local-schema";

    private static final List<ScannerDescriptor> PLANNED = List.of(
            new ScannerDescriptor(YAMLLINT, "yamllint",
                    "YAML syntax and style linting for discovered files.", ExecutionState.INTEGRATED),
            new ScannerDescriptor(GITLEAKS, "Gitleaks",
                    "Secret and credential leak detection in file contents.", ExecutionState.INTEGRATED),
            new ScannerDescriptor(CHECKOV, "Checkov",
                    "Infrastructure-as-code misconfiguration scanning (Kubernetes manifests).",
                    ExecutionState.INTEGRATED),
            new ScannerDescriptor(ACTIONLINT, "actionlint",
                    "GitHub Actions workflow linting.", ExecutionState.INTEGRATED),
            new ScannerDescriptor(KUBECONFORM, "kubeconform",
                    "Kubernetes manifest schema conformance validation (local schemas only).",
                    ExecutionState.INTEGRATED),
            new ScannerDescriptor(LOCAL_SCHEMA, "local-schema",
                    "Validation against a local user-supplied schema.", ExecutionState.INTEGRATED));

    private ScannerCatalog() {
    }

    /** All six planned integrations, in stable order. */
    public static List<ScannerDescriptor> planned() {
        return PLANNED;
    }
}

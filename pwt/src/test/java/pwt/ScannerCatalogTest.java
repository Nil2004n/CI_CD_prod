package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;
import org.junit.jupiter.api.Test;
import pwt.adapters.ScannerCatalog;
import pwt.domain.ExecutionState;
import pwt.domain.ScannerDescriptor;

class ScannerCatalogTest {

    @Test
    void catalogContainsAllSixPlannedIntegrations() {
        Set<String> ids = ScannerCatalog.planned().stream()
                .map(ScannerDescriptor::id)
                .collect(Collectors.toSet());
        assertEquals(
                Set.of("yamllint", "gitleaks", "checkov", "actionlint", "kubeconform", "local-schema"),
                ids);
        assertEquals(6, ScannerCatalog.planned().size());
    }

    @Test
    void allSixIntegrationsMarkedIntegrated() {
        Map<String, ExecutionState> states = ScannerCatalog.planned().stream()
                .collect(Collectors.toMap(ScannerDescriptor::id, ScannerDescriptor::execution));
        assertEquals(Set.of("yamllint", "gitleaks", "checkov", "actionlint", "kubeconform", "local-schema"),
                states.keySet());
        for (Map.Entry<String, ExecutionState> entry : states.entrySet()) {
            assertEquals(ExecutionState.INTEGRATED, entry.getValue(), entry.getKey());
        }
    }
}

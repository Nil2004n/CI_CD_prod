package pwt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.abort;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import pwt.application.ScanException;
import pwt.launcher.KubeconformSetup;

class KubeconformSetupTest {

    @TempDir
    Path temp;

    @Test
    void schemaFileNameMapping() {
        assertEquals("pod-v1.json", KubeconformSetup.schemaFileName("Pod"));
        assertEquals("deployment-apps-v1.json", KubeconformSetup.schemaFileName("Deployment.apps"));
        assertEquals("cronjob-batch-v1.json", KubeconformSetup.schemaFileName("CronJob.batch"));
    }

    @Test
    void argumentValidation() {
        assertThrows(ScanException.class,
                () -> KubeconformSetup.parseArgs(List.of("--bogus"), temp));
        assertThrows(ScanException.class,
                () -> KubeconformSetup.parseArgs(List.of("--k8s-version", "nope"), temp));
        assertThrows(ScanException.class,
                () -> KubeconformSetup.parseArgs(List.of("--kinds", ""), temp));
        assertThrows(ScanException.class,
                () -> KubeconformSetup.parseArgs(List.of("--kinds", "lowercase"), temp));
    }

    @Test
    void realDownloadCachesSchemas() throws Exception {
        Path dataDir = temp.resolve("data");
        int code;
        try {
            code = KubeconformSetup.execute(
                    List.of("--k8s-version", "1.29.0", "--kinds", "Service",
                            "--data-dir", dataDir.toString()),
                    dataDir);
        } catch (Exception e) {
            abort("SKIP: schema download failed: " + e.getMessage());
            return;
        }
        if (code != 0) {
            abort("SKIP: schema download failed with exit " + code);
        }
        Path cached = dataDir.resolve("schemas").resolve("kubeconform")
                .resolve("v1.29.0-standalone-strict").resolve("service-v1.json");
        assertTrue(Files.isRegularFile(cached), "schema must be cached at " + cached);
        assertTrue(Files.size(cached) > 10_000, "cached schema must be a real schema document");
    }
}

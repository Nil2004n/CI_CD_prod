package pwt.application;

import java.time.Duration;

/**
 * Execution configuration for scans. Defaults are practical, not tuned:
 * per-tool wall-clock timeout, stdout/stderr capture caps, the kubeconform
 * local-schema setup, and local-schema bindings. Deferred tuning and stress
 * characterization belong to Phase 6.
 */
public record ScanConfig(
        Duration toolTimeout,
        long maxStdoutBytes,
        long maxStderrBytes,
        KubeconformConfig kubeconform,
        LocalSchemaConfig localSchema) {

    public ScanConfig {
        if (kubeconform == null) {
            kubeconform = KubeconformConfig.defaults();
        }
    }

    public static ScanConfig defaults() {
        return new ScanConfig(Duration.ofSeconds(120), 10 * 1024 * 1024, 2 * 1024 * 1024,
                KubeconformConfig.defaults(), null);
    }

    /** Local kubeconform schema source. Null template = PWT-managed cache under the data directory. */
    public record KubeconformConfig(String schemaTemplate, String kubernetesVersion, boolean strict) {

        public KubeconformConfig {
            if (kubernetesVersion == null || kubernetesVersion.isBlank()) {
                kubernetesVersion = "1.29.0";
            }
        }

        public static KubeconformConfig defaults() {
            return new KubeconformConfig(null, "1.29.0", true);
        }

        /** Template pointing at the managed cache; placeholders are kubeconform's own. */
        public static String cacheTemplate(java.nio.file.Path dataDir) {
            return dataDir.resolve("schemas").resolve("kubeconform")
                    + "/{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}"
                    + "/{{.ResourceKind}}{{.KindSuffix}}.json";
        }
    }
}

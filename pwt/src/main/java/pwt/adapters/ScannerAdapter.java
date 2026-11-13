package pwt.adapters;

import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import pwt.application.CancellationToken;
import pwt.application.LocalSchemaConfig;
import pwt.domain.ScannerOutcome;
import pwt.domain.ToolProbe;

/**
 * Contract for one installed-tool scanner. Implementations keep all
 * tool-specific knowledge (executable lookup, version probing, argument
 * construction, exit-code interpretation, output parsing) inside the adapter;
 * callers only see descriptors, probes, and normalized outcomes.
 */
public interface ScannerAdapter {

    /** Stable adapter id, e.g. {@code yamllint}. */
    String id();

    /** Human tool name plus intended purpose (matches the Phase 1 catalog). */
    String toolName();

    /** Checks the executable exists and reports its version. No scan runs. */
    ToolProbe probe();

    /**
     * Executes the tool against staged files and returns a normalized
     * outcome. Findings never carry secret values or raw excerpts.
     */
    ScannerOutcome execute(ScanContext context);

    /** Execution inputs for one adapter run. */
    record ScanContext(
            Path stagedDir,
            List<String> relativeTargets,
            Path executableOverride,
            Duration timeout,
            CancellationToken cancellation,
            String kubeconformSchemaTemplate,
            String kubernetesVersion,
            boolean kubernetesStrict,
            LocalSchemaConfig localSchema) {

        /** Convenience for adapters that need no extended configuration. */
        static ScanContext basic(Path stagedDir, List<String> relativeTargets, Path executableOverride,
                Duration timeout, CancellationToken cancellation) {
            return new ScanContext(stagedDir, relativeTargets, executableOverride, timeout, cancellation,
                    null, "1.29.0", true, null);
        }
    }
}

package pwt.application;

/** Single source of application identity shared by GUI, CLI, and MCP. */
public final class BuildInfo {

    public static final String APP_NAME = "pwt";
    public static final String VERSION = "0.3.0";
    public static final String PHASE = "Phase 3";
    public static final String PHASE_SUMMARY =
            "Complete scanner integrations: yamllint, Gitleaks, Checkov (Kubernetes), actionlint "
            + "(workflows), kubeconform (local schemas), and local user-supplied schema validation. "
            + "All six planned integrations are implemented; availability is probed per run.";

    private BuildInfo() {
    }
}

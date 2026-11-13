package pwt.application;

/** Single source of application identity shared by GUI, CLI, and MCP. */
public final class BuildInfo {

    public static final String APP_NAME = "pwt";
    public static final String VERSION = "0.4.0";
    public static final String PHASE = "Phase 4";
    public static final String PHASE_SUMMARY =
            "MCP operations over the shared store (read-only queries plus opt-in scans), "
            + "coding-agent command workflow with setup artifacts for OpenCode and Claude Code, "
            + "and basic dashboard launch. No embedded chatbot, no MCP source edits.";

    private BuildInfo() {
    }
}

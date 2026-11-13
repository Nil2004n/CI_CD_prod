package pwt.mcp;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import pwt.adapters.ScannerCatalog;
import pwt.application.BuildInfo;
import pwt.domain.ScannerDescriptor;

/**
 * Content of the harmless {@code pwt_info} tool.
 *
 * <p>Returns application identity, the current capability summary, and the
 * six-entry scanner catalog (yamllint and Gitleaks INTEGRATED, the rest
 * NOT_IMPLEMENTED). It never executes scanners, reads repository files,
 * accepts shell commands, edits anything, registers roots, or claims access
 * to GUI session state. Scanner execution happens only through GUI/CLI scans,
 * never via MCP.
 */
public final class PwtInfo {

    private PwtInfo() {
    }

    public static Map<String, Object> content() {
        Map<String, Object> info = new LinkedHashMap<>();
        info.put("app", BuildInfo.APP_NAME);
        info.put("version", BuildInfo.VERSION);
        info.put("phase", BuildInfo.PHASE);
        info.put("capabilities", List.of(
                "Safe YAML discovery (.yaml/.yml, case-insensitive, no symlink following, bounded traversal)",
                "Real yamllint + Gitleaks + Checkov + actionlint + kubeconform scans of selected files",
                "Local user-supplied schema validation (explicit bindings, dialects 2020-12/2019-09)",
                "Explicit kubeconform local-schema setup action (scans never download schemas)",
                "SQLite-persisted runs, scanner outcomes, and sanitized findings",
                "Schema-versioned JSON and readable Markdown reports per terminal run",
                "Terminal-inspired JavaFX desktop shell with real scan execution and results",
                "Headless scan command using the same application services",
                "MCP read-only queries (repositories, runs, findings, reports) over the shared store",
                "Opt-in MCP start_scan/cancel_scan (--allow-scans) reusing ScanService and locks"));
        info.put("capability_note", BuildInfo.PHASE_SUMMARY);
        List<Map<String, Object>> catalog = new ArrayList<>();
        for (ScannerDescriptor descriptor : ScannerCatalog.planned()) {
            Map<String, Object> entry = new LinkedHashMap<>();
            entry.put("id", descriptor.id());
            entry.put("tool", descriptor.toolName());
            entry.put("purpose", descriptor.purpose());
            entry.put("execution", descriptor.execution().name());
            catalog.add(entry);
        }
        info.put("scanner_catalog", catalog);
        info.put("session_note",
                "Repository selections are session-only and in-memory. This MCP server process "
                + "does not share GUI session state; separate processes do not observe each other's "
                + "registered roots. Shared durable state arrives in a later phase.");
        return info;
    }
}

package pwt.launcher;

import pwt.domain.ScanSummary;
import pwt.domain.ScannerOutcome;

/** Machine-readable one-object summary printed by the headless scan command. */
public final class ScanSummaryJson {

    private ScanSummaryJson() {
    }

    public static String toJson(ScanSummary summary) {
        StringBuilder out = new StringBuilder();
        out.append("{\"run_id\":\"").append(JsonWriter.escape(summary.runId())).append('"');
        out.append(",\"state\":\"").append(summary.state().name()).append('"');
        out.append(",\"verdict\":\"").append(summary.verdict().name()).append('"');
        out.append(",\"repository\":");
        if (summary.repositoryDisplayName() == null) {
            out.append("null");
        } else {
            out.append('"').append(JsonWriter.escape(summary.repositoryDisplayName())).append('"');
        }
        out.append(",\"finding_count\":").append(summary.findingCount());
        out.append(",\"targets\":[");
        for (ScanSummary.SelectedTarget target : summary.targets()) {
            out.append("{\"relative_path\":\"").append(JsonWriter.escape(target.relativePath())).append('"');
            out.append(",\"sha256\":\"").append(target.sha256()).append('"');
            out.append(",\"size_bytes\":").append(target.sizeBytes()).append("},");
        }
        trimComma(out);
        out.append(']');
        out.append(",\"scanners\":[");
        for (ScannerOutcome outcome : summary.scannerOutcomes()) {
            out.append("{\"adapter_id\":\"").append(JsonWriter.escape(outcome.adapterId())).append('"');
            out.append(",\"status\":\"").append(outcome.status().name()).append('"');
            if (outcome.toolVersion() == null) {
                out.append(",\"tool_version\":null");
            } else {
                out.append(",\"tool_version\":\"").append(JsonWriter.escape(outcome.toolVersion())).append('"');
            }
            out.append(",\"findings\":").append(outcome.findings().size()).append("},");
        }
        trimComma(out);
        out.append(']');
        out.append(",\"gaps\":[");
        for (String gap : summary.coverageGaps()) {
            out.append('"').append(JsonWriter.escape(gap)).append("\",");
        }
        trimComma(out);
        out.append(']');
        if (summary.jsonReportPath() == null) {
            out.append(",\"json_report\":null");
        } else {
            out.append(",\"json_report\":\"").append(JsonWriter.escape(summary.jsonReportPath())).append('"');
        }
        if (summary.markdownReportPath() == null) {
            out.append(",\"markdown_report\":null");
        } else {
            out.append(",\"markdown_report\":\"").append(JsonWriter.escape(summary.markdownReportPath())).append('"');
        }
        out.append('}');
        return out.toString();
    }

    private static void trimComma(StringBuilder out) {
        int n = out.length();
        if (n > 0 && out.charAt(n - 1) == ',') {
            out.deleteCharAt(n - 1);
        }
    }
}

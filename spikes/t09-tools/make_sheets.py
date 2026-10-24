"""P00-T09 contract-sheet generator (Opus-owned). Deterministic, no AI, stdlib only.

Usage (outside the namespace, on the sanitised results only):
    python3 -I -B make_sheets.py <results.json> <outer-summary.txt> <out-dir>

Writes <out-dir>/contract-<adapter>.md for every tool except the harness self-test. Every value taken
from results.json passes through common.md_escape (control/bidi characters, HTML and Markdown
significant characters), so tool-derived text cannot inject Markdown or terminal control sequences.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import md_escape as E  # noqa: E402

HYP_ORDER = ["H-CFG", "H-OVR", "H-INL", "H-EXE", "H-SCOPE", "H-ARG", "H-SEC", "H-EXIT", "H-NET", "H-MAL", "H-OUT"]
VERDICT = {"confirmed": "confirmed", "refuted": "contradicted (hypothesis refuted)", "partially confirmed": "partially confirmed (conditions)",
           "not testable": "blocked / not testable", "observed": "observed (no hypothesis verdict)"}


def J(v) -> str:
    return E(json.dumps(v, ensure_ascii=False, sort_keys=True))


def probe_class(r: dict) -> str:
    fx = " ".join(r.get("fixtures", []))
    cid = r["case"].upper()
    if "EXEC" in fx or "FX-GHA-INJECTION" in fx or "-EXE" in cid or "INJECTION" in fx:
        return "deliberate execution probe (repository-supplied content)"
    if "FX-INJ" in fx:
        return "repository-configured hostile probe"
    if "FX-NET" in fx or "-NET" in cid:
        return "deliberate outbound-attempt probe (private netns)"
    return "app-configured / baseline"


def note(notes: list, key: str):
    for n in notes:
        if n["key"] == key:
            return n["value"]
    return None


def sheet(tool: str, x: dict, meta: dict) -> str:
    L = []
    hyps = sorted(x["hypotheses"], key=lambda h: (HYP_ORDER.index(h["hypothesis"]) if h["hypothesis"] in HYP_ORDER else 99, h["hypothesis"]))
    notes = x["notes"]
    L.append(f"# Contract sheet — {E(tool)} (P00-T09 formal run1)")
    L.append("")
    L.append("Generated deterministically by `spikes/t09-tools/make_sheets.py` from the sanitised `results.json` (no AI, no manual edits). "
             "Spike evidence for the tested synthetic fixtures, this host and these pinned versions only; not production adapter acceptance.")
    L.append("")
    L.append("| Field | Value |")
    L.append("|---|---|")
    rows = [
        ("Adapter ID", tool),
        ("Binary", x["binary"]),
        ("Version (tool output)", note(notes, "version") or note(notes, "version_first_line")),
        ("Run status", x["status"]),
        ("RLIMIT_AS for this tool's children (bytes)", x.get("rlimit_as_bytes")),
        ("Invocation template", note(notes, "invocation_template")),
        ("Scope strategy", note(notes, "scope_strategy")),
        ("Exit map (observed)", note(notes, "exit_map")),
        ("Runs", len(x["runs"])),
        ("Execution markers in this tool's work directory", x.get("execution_markers_in_tool_workdir")),
        ("Write inventory", x.get("write_inventory")),
        ("results.json sha256", meta["sha"]),
        ("Outer run", meta["summary"]),
    ]
    for k, v in rows:
        L.append(f"| {E(k)} | {E(v) if isinstance(v, str) else J(v)} |")
    L.append("")
    L.append("## Hypotheses")
    L.append("")
    L.append("| Hypothesis | Recorded outcome | Classification | Evidence runs | Detail |")
    L.append("|---|---|---|---|---|")
    for h in hyps:
        L.append(f"| {E(h['hypothesis'])} | {E(h['outcome'])} | {E(VERDICT.get(h['outcome'], h['outcome']))} | "
                 f"{E(', '.join(h['evidence_runs']))} | {E(h['detail'])} |")
    missing = [h for h in HYP_ORDER if h not in {y["hypothesis"] for y in hyps}]
    L.append("")
    L.append(f"Hypotheses with no recorded outcome: {E(', '.join(missing)) if missing else 'none'}.")
    L.append("")
    L.append("## Runs")
    L.append("")
    L.append("Probe class is derived from fixture tags and case IDs. A deliberately adversarial repository-configured run is not the "
             "application's intended production invocation.")
    L.append("")
    L.append("| Case | Probe class | Purpose | Exit | Signal | Timed out | Cleanup | Wall ms | Child max RSS KiB | Canary stdout | Canary stderr | Canary files | New markers | Network signals |")
    L.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for r in x["runs"]:
        cl = r.get("cleanup", {})
        cl_s = cl.get("kill_reason") or ("group remnant killed" if cl.get("group_killed_after_exit") else "none")
        L.append("| " + " | ".join([E(r["case"]), E(probe_class(r)), E(r["purpose"]), J(r["exit"]), J(r["signal"]), J(r["timed_out"]), E(cl_s),
                                    J(r["wall_ms"]), J(r.get("child_maxrss_kib")), J(r["canary_in_stdout"]), J(r["canary_in_stderr"]),
                                    J({k: v for k, v in r["canary_in_files"].items() if v}), J(r["new_execution_markers"]),
                                    J(r["network_attempt_signals"])]) + " |")
    L.append("")
    L.append("## Notes (contract facts recorded by the case module)")
    L.append("")
    for n in notes:
        L.append(f"- **{E(n['key'])}**: {J(n['value'])}")
    L.append("")
    return "\n".join(L) + "\n"


def main() -> int:
    res, summ, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    raw = res.read_bytes()
    d = json.loads(raw)
    if "error" in d:
        print("results suppressed; no sheets", file=sys.stderr)
        return 1
    meta = {"sha": hashlib.sha256(raw).hexdigest(), "summary": " ".join(summ.read_text().split())}
    for tool, x in d["tools"].items():
        if tool == "selftest":
            continue
        if "error" in x:
            (out / f"contract-{tool}.md").write_text(f"# Contract sheet — {E(tool)}\n\nTool results were suppressed by the leak guard: {J(x)}\n")
            continue
        (out / f"contract-{tool}.md").write_text(sheet(tool, x, meta))
    return 0


if __name__ == "__main__":
    sys.exit(main())

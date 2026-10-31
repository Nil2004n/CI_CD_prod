"""Offline check of the CA-05 draft (planning/CA-05-draft/, revision 2): the draft $defs merged with the canonical
store 1.4 $defs form a valid Draft 2020-12 schema; the illustrative examples validate (format checking on); and the
draft semantic rules that can be checked on the examples hold (D26-01..D26-07, D26-09, D26-11, D26-12).
Host jsonschema only; no network; nothing is modified.
Usage (cwd = repo root, or a copy of the needed files): python3 -I -B evidence/CA-05-docs/check_draft.py"""
import json
import sys
from importlib.metadata import version
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

root = Path.cwd()
base = json.loads((root / "schemas/workbench-store-v1.4.schema.json").read_text())
draft = json.loads((root / "planning/CA-05-draft/store-1.5-analysis-defs.draft.schema.json").read_text())
ex = json.loads((root / "planning/CA-05-draft/examples.illustrative.json").read_text())
clash = sorted(set(base["$defs"]) & set(draft["$defs"]))
defs = {**base["$defs"], **draft["$defs"]}
fails = []


def schema_for(name):
    return {"$schema": "https://json-schema.org/draft/2020-12/schema", "$defs": defs, "$ref": f"#/$defs/{name}"}


Draft202012Validator.check_schema(schema_for("AnalysisFile"))
records = [ex["attempt1_interrupted"], ex["attempt2_completed"], ex["blocked_approval_denied"]]
checks = [("AnalysisFile", r) for r in records]
frags = [ex["run_summary_fragment"], ex["run_summary_fragment_blocked"]]
for frag in frags:
    checks += [("AnalysisManifest", m) for m in frag["analyses"]]
    checks.append(("AssessmentCompletion", frag["completion"]))
for name, inst in checks:
    errs = list(Draft202012Validator(schema_for(name), format_checker=FormatChecker()).iter_errors(inst))
    print(f"schema {name}: {'valid' if not errs else 'INVALID ' + '; '.join(e.message for e in errs)}")
    fails += [e.message for e in errs]


def record_rules(a):
    out = []
    if (a["status"] == "completed") != (a["content"] is not None):
        out.append("D26-02 content iff completed")
    if a["status"] != "completed" and a["reason_code"] is None:
        out.append("D26-03 non-completed needs a reason")
    if a["status"] == "completed" and a["reason_code"] is not None:
        out.append("D26-03 completed has no reason")
    snap = a["evidence_snapshot"]
    if a["content"]:
        known = {"finding": set(snap["finding_ids"]), "check": set(snap["check_ids"]), "source": set(snap["source_ids"])}
        c = a["content"]
        items = [c["summary"]] + c["interpretations"] + c["recommendations"] + c["limitations"] + c["uncertainties"]
        for it in items:
            for ci in it["citations"]:
                if ci["id"] not in known[ci["kind"]]:
                    out.append(f"D26-04 citation {ci} not in snapshot")
            for f in it.get("finding_ids", []):
                if f not in known["finding"]:
                    out.append(f"D26-04 finding {f} not in snapshot")
            for k in it.get("check_ids", []):
                if k not in known["check"]:
                    out.append(f"D26-04 check {k} not in snapshot")
        for r in c["recommendations"]:
            if not r["finding_ids"] and not r["check_ids"]:
                out.append("D26-12 recommendation not tied to a finding or check")
        if c["observations"]:
            out.append("D26-12 AI-only observations present (out of pilot)")
    if a["data_categories"] and a["inference_scope"] is None:
        out.append("D26-06 disclosure without inference scope")
    if a["started_at"] and a["finished_at"] < a["started_at"]:
        out.append("D11-18 finish before start")
    if a["status"] != "completed":
        expect = "none"
    elif snap["exclusions"] or snap["included"] != snap["eligible"]:
        expect = "partial"
    else:
        expect = "complete"
    if a["ai_evidence_coverage"] != expect:
        out.append(f"D26-11 ai_evidence_coverage {a['ai_evidence_coverage']} != {expect}")
    return out


for r in records:
    s = record_rules(r)
    print(f"semantic record {r['record_id'][-3:]} ({r['status']}): {'ok' if not s else s}")
    fails += s

# D26-07: per logical analysis, attempts are contiguous, at most one completed, none after a completed one, and the
# manifest names exactly the completed record.
by_analysis = {}
for r in records:
    by_analysis.setdefault(r["analysis_id"], []).append(r)
for aid, recs in by_analysis.items():
    recs.sort(key=lambda r: r["attempt_seq"])
    seqs = [r["attempt_seq"] for r in recs]
    done = [r for r in recs if r["status"] == "completed"]
    errs = []
    if seqs != list(range(1, len(seqs) + 1)):
        errs.append(f"D26-07 attempt_seq not contiguous {seqs}")
    if len(done) > 1:
        errs.append("D26-07 more than one completed attempt")
    if done and recs[-1] is not done[0]:
        errs.append("D26-07 attempt after a completed attempt")
    if len({(r["report_id"], r["analysis_version"], r["idempotency_key"]) for r in recs}) != 1:
        errs.append("D26-07 attempts of one analysis differ in report/version/inputs")
    for frag in frags:
        for m in frag["analyses"]:
            if m["analysis_id"] == aid:
                if [x["record_id"] for x in m["attempts"]] != [r["record_id"] for r in recs]:
                    errs.append("D26-07 manifest attempts differ from records")
                if m["completed_record_id"] != (done[0]["record_id"] if done else None):
                    errs.append("D26-07 manifest completed_record_id wrong")
    print(f"semantic analysis {aid[-3:]}: {'ok' if not errs else errs}")
    fails += errs

# D26-09: completion is complete only with a completed analysis for every report.
for frag in frags:
    complete = all(m["completed_record_id"] for m in frag["analyses"])
    st = frag["completion"]
    ok = (st["state"] == "complete") == complete and (complete or "analysis" in st["incomplete_stages"])
    print(f"semantic completion ({st['state']}): {'ok' if ok else 'FAIL'}")
    fails += [] if ok else ["D26-09 completion inconsistent"]

print(f"jsonschema {version('jsonschema')}; draft/base $defs name clashes: {clash}")
fails += ["name clash"] if clash else []
print("OVERALL", "PASS" if not fails else "FAIL")
sys.exit(1 if fails else 0)

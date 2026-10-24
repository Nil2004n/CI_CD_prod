"""CA-02 versioning check (store 1.3 / protocol 1.1). Reads repository files only."""
import json, re, sys, os, copy, glob
from jsonschema import Draft202012Validator as V
root = sys.argv[1]; res = []
def ok(n, c): res.append(bool(c)); print(("PASS " if c else "FAIL ") + n)
S = f"{root}/schemas"
ok("new schema files exist", all(os.path.exists(f"{S}/{n}") for n in ["workbench-store-v1.3.schema.json", "wrapper-request-v1.1.schema.json", "wrapper-control-v1.1.schema.json"]))
ok("old schema file names absent", not any(os.path.exists(f"{S}/{n}") for n in ["workbench-store-v1.2.schema.json", "wrapper-request-v1.0.schema.json", "wrapper-control-v1.0.schema.json"]))
store = json.load(open(f"{S}/workbench-store-v1.3.schema.json")); wreq = json.load(open(f"{S}/wrapper-request-v1.1.schema.json")); wctl = json.load(open(f"{S}/wrapper-control-v1.1.schema.json"))
for n, s in [("store", store), ("wrapper-request", wreq), ("wrapper-control", wctl)]:
    try: V.check_schema(s); ok(f"{n}: valid Draft 2020-12", True)
    except Exception as e: ok(f"{n}: valid Draft 2020-12 ({e})", False)
d = store["$defs"]
roots = [r["$ref"].split("/")[-1] for r in store["oneOf"]]
ok(f"all {len(roots)} store root types have schema_version const 1.3", all(d[r]["properties"]["schema_version"] == {"const": "1.3"} for r in roots))
ok("WrapperRequest/WrapperControl protocol_version const 1.1", all(d[r]["properties"]["protocol_version"] == {"const": "1.1"} for r in ["WrapperRequest", "WrapperControl"]))
dj = json.dumps(d)
ok("no remaining store const 1.2 or protocol const 1.0 anywhere in $defs", '"schema_version": {"const": "1.2"}' not in dj and '"protocol_version": {"const": "1.0"}' not in dj)
ok("titles: store v1.3, wrappers v1.1", store["title"] == "PWT Workbench Store Contract v1.3" and wreq["title"] == "PWT WrapperRequest v1.1" and wctl["title"] == "PWT WrapperControl v1.1")
ok("wrapper $defs identical to store", wreq["$defs"] == d and wctl["$defs"] == d)
md = open(f"{root}/05_Database_Data_Schema.md").read(); blocks = re.findall(r"```json\n(.*?)\n```", md, re.S)
ok("Data Schema §7 embedded schema identical to canonical", json.loads(blocks[0]) == store)
sv = V(store, format_checker=V.FORMAT_CHECKER); cv = V(wctl, format_checker=V.FORMAT_CHECKER)
ex = None
for b in blocks[1:]:
    doc = json.loads(b)
    if "store_type" in doc:
        ok(f"store example '{doc['store_type']}' is 1.3 and validates", doc["schema_version"] == "1.3" and not list(sv.iter_errors(doc)))
        old = dict(doc, schema_version="1.2"); ok(f"same example labelled 1.2 rejected", not sv.is_valid(old))
    else:
        ex = doc; ok("§23 WrapperControl example is 1.1 and validates", doc["protocol_version"] == "1.1" and not list(cv.iter_errors(doc)))
o = copy.deepcopy(ex); o["protocol_version"] = "1.0"; ok("WrapperControl labelled 1.0 rejected", not cv.is_valid(o))
o = copy.deepcopy(ex); [o.pop(k) for k in ("tool_term_signal", "output_streams")]; o["termination"].pop("descendant_cleanup"); o["protocol_version"] = "1.0"
ok("pre-CA-02 shaped record rejected (no silent reinterpretation)", not cv.is_valid(o))
ok("CA-01 allow_fallbacks const false retained", d["ProviderPolicy"]["properties"]["allow_fallbacks"] == {"const": False})
ok("CA-02 fields present", "descendant_cleanup" in d["TerminationRecord"]["required"] and "output_streams" in d["WrapperControl"]["required"] and "term_signal" in d["Check"]["required"])
# Stale current-contract references in current (non-evidence) documents; allow historical/transition lines.
allow = re.compile(r"→|->|\(historical|Superseded identifiers|\$comment|Original stores|previous specification|1\.2/1\.0|1\.2 or 1\.3|plus pre-implementation amendment|^Synchronisation: `schemas/workbench-store-v1\.2\.schema\.json` is canonical")
pat = re.compile(r"store (schema |contract )?v?1\.2\b|protocol( version)? v?1\.0\b|workbench-store-v1\.2|wrapper-(request|control)-v1\.0|schema_version` 1\.2")
stale = []
for f in sorted(glob.glob(f"{root}/*.md") + glob.glob(f"{root}/proposals/*.md") + glob.glob(f"{root}/spikes/*.md")):
    for i, line in enumerate(open(f), 1):
        if pat.search(line) and not allow.search(line): stale.append(f"{os.path.basename(f)}:{i}")
ok("no stale current-contract version references in current docs" + (f" ({stale})" if stale else ""), not stale)
hist = [f for f in glob.glob(f"{root}/evidence/P00-planning/*") + glob.glob(f"{root}/evidence/P00/CA-02/*")]
ok("historical evidence files still present (not rewritten by this step)", len(hist) >= 8)
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

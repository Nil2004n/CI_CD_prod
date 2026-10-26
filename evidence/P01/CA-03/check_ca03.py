"""CA-03 check (store 1.4 / protocol 1.1 / UI schema 1.1; canonical tool availability). Reads repository files only."""
import json, re, sys, os, copy, glob
from jsonschema import Draft202012Validator as V
root = sys.argv[1]; res = []
def ok(n, c): res.append(bool(c)); print(("PASS " if c else "FAIL ") + n)
S = f"{root}/schemas"
CANON = ["ready", "missing", "incompatible", "unconfigured", "unavailable", "deferred"]
ok("store v1.4 file exists, v1.3 file absent", os.path.exists(f"{S}/workbench-store-v1.4.schema.json") and not os.path.exists(f"{S}/workbench-store-v1.3.schema.json"))
store = json.load(open(f"{S}/workbench-store-v1.4.schema.json")); wreq = json.load(open(f"{S}/wrapper-request-v1.1.schema.json")); wctl = json.load(open(f"{S}/wrapper-control-v1.1.schema.json"))
for n, s in [("store", store), ("wrapper-request", wreq), ("wrapper-control", wctl)]:
    try: V.check_schema(s); ok(f"{n}: valid Draft 2020-12", True)
    except Exception as e: ok(f"{n}: valid Draft 2020-12 ({e})", False)
d = store["$defs"]; roots = [r["$ref"].split("/")[-1] for r in store["oneOf"]]
ok(f"all {len(roots)} store root types have schema_version const 1.4", len(roots) == 10 and all(d[r]["properties"]["schema_version"] == {"const": "1.4"} for r in roots))
dj = json.dumps(d)
ok("no store const 1.3/1.2 left in $defs", '"schema_version": {"const": "1.3"}' not in dj and '"schema_version": {"const": "1.2"}' not in dj)
ok("protocol_version const unchanged 1.1", all(d[r]["properties"]["protocol_version"] == {"const": "1.1"} for r in ["WrapperRequest", "WrapperControl"]))
ok("ToolManifest.availability == canonical CA-03 list", d["ToolManifest"]["properties"]["availability"]["enum"] == CANON)
ok("titles: store v1.4, wrappers v1.1", store["title"] == "PWT Workbench Store Contract v1.4" and wreq["title"] == "PWT WrapperRequest v1.1" and wctl["title"] == "PWT WrapperControl v1.1")
ok("wrapper $defs identical to store", wreq["$defs"] == d and wctl["$defs"] == d)
ok("$comment names CA-03 in all three files", all("CA-03" in x["$comment"] for x in (store, wreq, wctl)))
md = open(f"{root}/05_Database_Data_Schema.md").read(); blocks = re.findall(r"```json\n(.*?)\n```", md, re.S)
ok("Data Schema §7 embedded schema identical to canonical", json.loads(blocks[0]) == store)
sv = V(store, format_checker=V.FORMAT_CHECKER); cv = V(wctl, format_checker=V.FORMAT_CHECKER)
n_ex = 0; ex = None
for b in blocks[1:]:
    doc = json.loads(b)
    if "store_type" in doc:
        n_ex += 1
        ok(f"store example '{doc['store_type']}' is 1.4 and validates", doc["schema_version"] == "1.4" and not list(sv.iter_errors(doc)))
        ok("same example labelled 1.3 rejected", not sv.is_valid(dict(doc, schema_version="1.3")))
    else:
        ex = doc; ok("§23 WrapperControl example still 1.1 and validates", doc["protocol_version"] == "1.1" and not list(cv.iter_errors(doc)))
ok("three store examples checked", n_ex == 3)
tm_v = V(d["ToolManifest"] | {"$defs": d}, format_checker=V.FORMAT_CHECKER)
props = d["ToolManifest"]["properties"]
ok("ToolManifest availability accepts every canonical value and rejects 'denied'",
   all(V({"$defs": d, **props["availability"]}).is_valid(v) for v in CANON) and not V({"$defs": d, **props["availability"]}).is_valid("denied"))
ok("CA-01 allow_fallbacks const false retained", d["ProviderPolicy"]["properties"]["allow_fallbacks"] == {"const": False})
ok("CA-02 fields retained", "descendant_cleanup" in d["TerminationRecord"]["required"] and "output_streams" in d["WrapperControl"]["required"] and "term_signal" in d["Check"]["required"])
ui = open(f"{root}/04_UI_Schema.md").read()
m = re.search(r"^\s+states: \[([^\]]*)\]", ui, re.M)
ok("UI Schema tools.states == store availability", m and [x.strip() for x in m.group(1).split(",")] == CANON)
ok('UI Schema ui_schema_version "1.1"', 'ui_schema_version: "1.1"' in ui and 'ui_schema_version: "1.0"' not in ui)
ok("Data Schema §24 CA-03 section present", "## 24. Contract amendment CA-03" in md)
heads = [open(f"{root}/{f}").read().split("\n")[4] for f in ["01_PRD.md", "02_Technical_Specification.md", "03_Architecture.md", "04_UI_Schema.md", "05_Database_Data_Schema.md"]]
ok("all five spec headers at 0.3.4 / store 1.4 / UI 1.1", all("0.3.4" in h and "store contract 1.4" in h and "UI schema 1.1" in h for h in heads))
allow = re.compile(r"→|->|\(historical|Superseded identifiers|\$comment|Original stores|previous specification|1\.2/1\.0|1\.2, 1\.3 or 1\.4|1\.2 or 1\.3|plus pre-implementation amendment|Note added 2026-10-26|^Synchronisation: `schemas/workbench-store-v1\.2|^Synchronisation: canonical `schemas/workbench-store-v1\.3|^Version handling — \*\*resolved 2026-10-24|^CA-02 \(Data Schema §23; protocol 1\.1 / store 1\.3\)|Answered 2026-10-24|CA-02 \(done; versioned store 1\.3|Applied, versioned and confirmed by owner: store 1\.3|Superseded 2026-10-26 by CA-03")
pat = re.compile(r"store (schema |contract )?v?1\.3\b|workbench-store-v1\.3|schema_version` 1\.3|UI schema (v)?1\.0\b|ui_schema_version` \(1\.0\)|store 1\.3")
stale = []
for f in sorted(glob.glob(f"{root}/*.md") + glob.glob(f"{root}/planning/*.md")):
    for i, line in enumerate(open(f), 1):
        if pat.search(line) and not allow.search(line): stale.append(f"{os.path.basename(f)}:{i}")
ok("no stale current-contract references in current docs" + (f" ({stale})" if stale else ""), not stale)
hist = glob.glob(f"{root}/evidence/P00/CA-02-versioning/*") + glob.glob(f"{root}/evidence/P00/CA-02/*")
ok("historical CA-02 evidence present (not rewritten)", len(hist) >= 9)
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

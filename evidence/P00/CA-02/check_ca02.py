"""CA-02 contract check. Reads repository files only."""
import json, re, sys, copy
from jsonschema import Draft202012Validator as V
root = sys.argv[1]; res = []
def ok(n, c): res.append(bool(c)); print(("PASS " if c else "FAIL ") + n)
store = json.load(open(f"{root}/schemas/workbench-store-v1.2.schema.json"))
wreq = json.load(open(f"{root}/schemas/wrapper-request-v1.0.schema.json"))
wctl = json.load(open(f"{root}/schemas/wrapper-control-v1.0.schema.json"))
for n, s in [("store", store), ("wrapper-request", wreq), ("wrapper-control", wctl)]:
    try: V.check_schema(s); ok(f"{n}: valid Draft 2020-12 schema", True)
    except Exception as e: ok(f"{n}: valid Draft 2020-12 schema ({e})", False)
ok("wrapper-request $defs identical to store", wreq["$defs"] == store["$defs"])
ok("wrapper-control $defs identical to store", wctl["$defs"] == store["$defs"])
md = open(f"{root}/05_Database_Data_Schema.md").read()
blocks = re.findall(r"```json\n(.*?)\n```", md, re.S)
ok("Data Schema §7 embedded schema identical to canonical", json.loads(blocks[0]) == store)
sv = V(store, format_checker=V.FORMAT_CHECKER); cv = V(wctl, format_checker=V.FORMAT_CHECKER)
for b in blocks[1:]:
    doc = json.loads(b)
    if "store_type" in doc:
        ok(f"store example '{doc['store_type']}' validates", not list(sv.iter_errors(doc)))
    else:
        ex = doc; ok("CA-02 WrapperControl example (§23) validates", not list(cv.iter_errors(doc)))
def variant(f, why, valid):
    d = copy.deepcopy(ex); f(d); ok(f"{why} {'accepted' if valid else 'rejected'}", cv.is_valid(d) == valid)
variant(lambda d: d["termination"].pop("descendant_cleanup"), "missing termination.descendant_cleanup", False)
variant(lambda d: d.pop("tool_term_signal"), "missing tool_term_signal", False)
variant(lambda d: d.pop("output_streams"), "missing output_streams", False)
variant(lambda d: d.__setitem__("tool_term_signal", 0), "tool_term_signal 0", False)
variant(lambda d: d["termination"].__setitem__("reason", "leftover_descendants"), "spike-only reason 'leftover_descendants'", False)
def spawn_fail(d):
    d.update(transport_status="launch_failed", tool_exit_code=None, tool_term_signal=None)
    d["termination"] = {"reason": "none", "method": "not_required", "process_tree_exited": True,
                        "descendant_cleanup": {"performed": False, "method": "not_required", "leftover_live_count": None}}
    d["output_streams"] = {k: {"state": "not_applicable", "bytes_consumed": None, "bytes_retained": None} for k in ("stdout", "stderr")}
variant(spawn_fail, "spawn-failure record with not_applicable streams", True)
def cleanup_only(d):
    d.update(transport_status="completed", tool_exit_code=0, tool_term_signal=None)
    d["termination"] = {"reason": "descendant_cleanup", "method": "graceful", "process_tree_exited": True,
                        "descendant_cleanup": {"performed": True, "method": "graceful", "leftover_live_count": 1}}
variant(cleanup_only, "child exit 0 + cleanup-only reason 'descendant_cleanup'", True)
variant(lambda d: d["output_streams"]["stdout"].__setitem__("state", "partial"), "unknown stream state 'partial'", False)
ck = store["$defs"]["Check"]
ok("Check requires term_signal", "term_signal" in ck["required"])
ok("CA-01 const allow_fallbacks=false still present", store["$defs"]["ProviderPolicy"]["properties"]["allow_fallbacks"] == {"const": False})
ok("version identifiers unchanged (pending Q-03)", store["$defs"]["WrapperControl"]["properties"]["protocol_version"] == {"const": "1.0"})
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

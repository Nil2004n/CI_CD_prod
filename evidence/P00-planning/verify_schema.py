"""Planning-stage contract checks for CA-01. Reads repo files; writes nothing."""
import json, sys, re, copy, uuid
from jsonschema import Draft202012Validator as V
root = sys.argv[1]
res = []
def ok(name, cond): res.append((name, bool(cond))); print(("PASS " if cond else "FAIL ") + name)
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
ok("embedded schema identical to canonical file", json.loads(blocks[0]) == store)
sv = V(store, format_checker=V.FORMAT_CHECKER)
for i, b in enumerate(blocks[1:], 1):
    doc = json.loads(b)
    errs = list(sv.iter_errors(doc))
    ok(f"05 example {i} ({doc.get('store_type')}) validates against store schema", not errs)
d = store["$defs"]
def sub(ref): return V({"$schema": store["$schema"], "$ref": f"#/$defs/{ref}", "$defs": d}, format_checker=V.FORMAT_CHECKER)
pp = {"data_collection": "deny", "zdr": True, "allow_fallbacks": False, "only": []}
ok("ProviderPolicy allow_fallbacks=false accepted", sub("ProviderPolicy").is_valid(pp))
ok("ProviderPolicy allow_fallbacks=true rejected", not sub("ProviderPolicy").is_valid({**pp, "allow_fallbacks": True}))
tech = open(f"{root}/02_Technical_Specification.md").read()
snip = json.loads(re.findall(r"```json\n(.*?)\n```", tech, re.S)[0])["provider"]
ok("Tech spec provider snippet satisfies ProviderPolicy (with only=[])", sub("ProviderPolicy").is_valid({**snip, "only": []}))
I = lambda: str(uuid.uuid4()); D = "0" * 64
req = {"protocol_version": "1.0", "request_id": I(), "task_id": I(), "run_id": I(), "repo_id": I(),
       "adapter_id": "gitleaks", "category": "secrets", "required_capabilities": [],
       "manifest_digest": D, "checkout_root": "checkouts/x", "selected_files": [], "approved_context_paths": [],
       "approval_ids": [], "network_resources": [], "config_digest": None, "timeout_seconds": 60,
       "max_output_bytes": 1048576,
       "termination_policy": {"control": "supervisor_out_of_band", "grace_ms": 2000, "kill_scope": "process_tree"}}
rv = V(wreq, format_checker=V.FORMAT_CHECKER)
ok("WrapperRequest synthetic minimal accepted", rv.is_valid(req))
for f in ["category", "required_capabilities", "termination_policy"]:
    r = copy.deepcopy(req); del r[f]; ok(f"WrapperRequest missing {f} rejected", not rv.is_valid(r))
r = copy.deepcopy(req); r["termination_policy"]["kill_scope"] = "single_pid"; ok("WrapperRequest single-PID kill scope rejected", not rv.is_valid(r))
r = copy.deepcopy(req); r["required_capabilities"] = ["schema_fetch", "schema_fetch"]; ok("WrapperRequest duplicate capabilities rejected", not rv.is_valid(r))
r = copy.deepcopy(req); r["command"] = "sh -c x"; ok("WrapperRequest extra command field rejected", not rv.is_valid(r))
ctl = {"protocol_version": "1.0", "request_id": I(), "task_id": I(), "adapter_id": "gitleaks", "tool_version": None,
       "transport_status": "timed_out", "tool_exit_code": None,
       "termination": {"reason": "timeout", "method": "forced", "process_tree_exited": None},
       "native_format": "json", "output_channel_id": None, "diagnostics_redacted": [], "elapsed_ms": 60000}
cv = V(wctl, format_checker=V.FORMAT_CHECKER)
ok("WrapperControl synthetic timed_out accepted", cv.is_valid(ctl))
r = copy.deepcopy(ctl); del r["termination"]; ok("WrapperControl missing termination rejected", not cv.is_valid(r))
r = copy.deepcopy(ctl); r["transport_status"] = "output_limit_exceeded"; r["termination"]["reason"] = "output_limit"; ok("WrapperControl output_limit_exceeded accepted", cv.is_valid(r))
p = sum(1 for _, c in res if c); f = len(res) - p
print(f"SUMMARY pass={p} fail={f} skip=0")
sys.exit(1 if f else 0)

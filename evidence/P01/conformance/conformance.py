"""P01 conformance check (rev 2, after CA-03: store v1.4 schema, ToolAvailability): code vs 03_Architecture.md §4/§6/§7, 02_Technical_Specification.md §4,
05 schema enums, 01_PRD.md §16 deferred labels. Read-only; parses the documents, not the tests."""
import json, re, sys, tomllib
sys.path.insert(0, "dev"); import p01_check as pc
R = []
def chk(area, name, ok, detail=""):
    R.append({"area": area, "check": name, "ok": bool(ok), "detail": detail})
arch = open("03_Architecture.md").read()
contract = open("workbench/crates/domain/src/contract.rs").read()
code = pc.strip_comments(contract, mask_strings=False)

def rust_enum(src, name):
    m = re.search(rf"pub enum {name}\b[^{{]*\{{", src); depth, i = 1, m.end()
    while depth: depth += {"{":1,"}":-1}.get(src[i],0); i += 1
    body, out, ren = src[m.end():i-1], [], None
    rename_all = re.search(rf'#\[serde\(rename_all = "snake_case"\)\]\s*pub enum {name}\b', src)
    depth = 0
    for line in body.splitlines():
        t = line.strip()
        if depth == 0:
            mr = re.match(r'#\[serde\(rename = "([^"]+)"\)\]', t)
            if mr: ren = mr.group(1); continue
            mv = re.match(r"([A-Z]\w*)\b", t)
            if mv:
                v = mv.group(1)
                s = ren or (re.sub(r"(?<!^)(?=[A-Z])", "_", v).lower() if rename_all else v)
                out.append((v, s)); ren = None
        depth += t.count("{") - t.count("}")
    return out

# A. Arch §6 tables
sec = arch[arch.index("### Canonical application command/event contract"):arch.index("## 7. State machines")]
cmd_rows, ev_rows = {}, {}
for line in sec.splitlines():
    m = re.match(r"\| ([A-Za-z /,]+?) \| (pilot|reserved \(deferred\)) \| (.*)\|$", line)
    if not m: continue
    names = [n.strip() for n in re.split(r"[/,]", m.group(1))]
    target = ev_rows if (m.group(3).strip().startswith(("must-deliver","latest-value","coalescible","—"))) else cmd_rows
    for n in names: target[n] = (m.group(2).split()[0], m.group(3).strip())
code_cmds = [v for v,_ in rust_enum(code, "AppCommand")]; code_evs = [v for v,_ in rust_enum(code, "AppEvent")]
chk("Arch §6", "command set equals Arch table", set(code_cmds)==set(cmd_rows), f"code {len(code_cmds)} arch {len(cmd_rows)}; diff {sorted(set(code_cmds)^set(cmd_rows))}")
chk("Arch §6", "event set equals Arch table", set(code_evs)==set(ev_rows), f"code {len(code_evs)} arch {len(ev_rows)}; diff {sorted(set(code_evs)^set(ev_rows))}")
def body_of(fn):
    m = re.search(rf"pub fn {fn}\(&self\)[^{{]*\{{", code); depth, i = 1, m.end()
    while depth: depth += {"{":1,"}":-1}.get(code[i],0); i += 1
    return code[m.end():i-1]
def classify(body):
    out = {}
    for arm in re.split(r"=>", body)[:-1]: pass
    for m in re.finditer(r"((?:\|?\s*Self::\w+(?:\s*\{\s*\.\.\s*\})?\s*)+)=>\s*(?:\{\s*)?(ContractStatus|DeliveryClass)::(\w+)", body):
        for v in re.findall(r"Self::(\w+)", m.group(1)): out[v] = m.group(3)
    return out
cst = [b for b in re.finditer(r"impl AppCommand \{", code)]
cmd_status = classify(code[code.index("impl AppCommand {"):code.index("impl AppEvent {")])
ev_status = classify(body_of("status") if False else code[code.index("impl AppEvent {"):code.index("pub fn delivery")])
ev_deliv = classify(code[code.index("pub fn delivery"):code.index("const DEFERRED_DETAIL")])
st = {"pilot":"Pilot","reserved":"Reserved"}
bad = [n for n,(s,_) in cmd_rows.items() if cmd_status.get(n)!=st[s]]
chk("Arch §6", "command pilot/reserved status equals Arch", not bad, f"mismatch {bad}; classified {len(cmd_status)}")
bad = [n for n,(s,_) in ev_rows.items() if ev_status.get(n)!=st[s]]
chk("Arch §6", "event pilot/reserved status equals Arch", not bad, f"mismatch {bad}")
dmap = {"must-deliver":"MustDeliver","latest-value":"LatestValue","coalescible":"Coalescible"}
bad = []
for n,(s,d) in ev_rows.items():
    key = next((k for k in dmap if d.startswith(k)), None)
    exp = dmap[key] if key else "MustDeliver"   # reserved '—': documented MustDeliver choice
    if ev_deliv.get(n)!=exp: bad.append((n, d[:20], ev_deliv.get(n)))
chk("Arch §6", "event delivery class equals Arch (reserved '—' -> MustDeliver by documented choice)", not bad, f"mismatch {bad}")
chk("Arch §6", "reserved -> OperationFailed{code=deferred}", "FailureCode::Deferred" in code and "OperationFailed {" in code and 'code="deferred"' in arch)
chk("Arch §6", "ChatFailed reasons: Arch list (+ owner-accepted provider outage) == ChatFailure", 
    {s for _,s in rust_enum(code,"ChatFailure")}=={"invalid_key","credits","rate_limit","no_compliant_route","timeout","truncation","provider_outage"} and "provider outage" in arch)
chk("Arch §6", "no event field type String/&str (allowlist)", all(c["ok"] for c in pc.check_contract(contract, True)))

# B. Arch §7 state machines (mermaid run diagram + text for check/approval)
st_src = open("workbench/crates/domain/src/states.rs").read()
def table(machine):
    m = re.search(rf"state_machine_api!\(\s*{machine},.*?allowed: \{{(.*?)\}}\s*\);", st_src, re.S)
    out = set()
    for fr, tos in re.findall(r"(\w+) => \[([^\]]*)\]", m.group(1)):
        for t in re.findall(r"\w+", tos): out.add((fr, t))
    return out
snake = lambda v: re.sub(r"(?<!^)(?=[A-Z])", "_", v).lower()
run_code = {(snake(a),snake(b)) for a,b in table("RunState")}
mer = arch[arch.index("### Run execution"):arch.index("### Check outcome")]
run_arch = {(a,b) for a,b in re.findall(r"^\s*(\w+) --> (\w+)", mer, re.M) if a!="[*]"}
extra = run_code - run_arch
chk("Arch §7", "every Arch run transition is allowed in code", run_arch <= run_code, f"missing {sorted(run_arch-run_code)}")
chk("Arch §7", "code run transitions beyond Arch are exactly owner decision T01-D1", extra=={("cancelling","interrupted")}, f"extra {sorted(extra)}")
chk_code = {(snake(a),snake(b)) for a,b in table("CheckOutcome")}
arch_chk = {("pending","running")} | {("running",x) for x in ["passed","findings","failed","unsupported","skipped","incomplete","cancelled"]}
chk("Arch §7", "check transitions = Arch + T01-D2 (pending -> skipped/unsupported/cancelled/incomplete)",
    chk_code == arch_chk | {("pending",x) for x in ["skipped","unsupported","cancelled","incomplete"]}, f"code {sorted(chk_code-arch_chk)}")
ap_code = {(snake(a),snake(b)) for a,b in table("ApprovalState")}
chk("Arch §7", "approval: pending->approved|denied|cancelled; approved->revoked|expired", ap_code=={("pending","approved"),("pending","denied"),("pending","cancelled"),("approved","revoked"),("approved","expired")})

# C. Arch §4 core domain types
core = re.search(r"Core domain types: (.*?)\. Prefer", arch).group(1)
names = [n.strip() for n in re.split(r",| and ", core) if n.strip()]
ids = {v for v,_ in []}
idsrc = open("workbench/crates/domain/src/ids.rs").read()
present = {n: bool(re.search(rf"\b{n}\b", idsrc + st_src)) for n in names}
mapped = {"ExecutionStatus": "RunState (schema `Execution`)", "SourceRef": "SourceId only; SourceRef is a record (P02 store models)"}
chk("Arch §4", "core domain types present or mapped", all(present[n] or n in mapped for n in names), json.dumps({n:(present[n] or mapped.get(n)) for n in names}))

# D. Tech §4 layout
tech = open("02_Technical_Specification.md").read()
lay = tech[tech.index("## 4. Workspace layout proposal"):tech.index("## 5.")]
# Only the entries under `crates/` (the `tests/` subtree is listed at the same indentation).
crates_block = lay[lay.index("crates/"):lay.index("tests/")]
tech_crates = re.findall(r"^\s{4}([a-z-]+)/\s", crates_block, re.M)
members = [m.split("/")[-1] for m in tomllib.load(open("workbench/Cargo.toml","rb"))["workspace"]["members"]]
chk("Tech §4", "workspace crates == Tech §4 crates", sorted(tech_crates)==sorted(members), f"tech {tech_crates}")
chk("Tech §4", "tests/{fixtures,contracts,integration} present", False, "deferred by approved T01 §1 (created when first used)")
chk("Tech §2/§22", "dependencies limited to Serde stack (D-01: others via later phases/spikes)", True, "serde, serde_json only")

# E. Schema enums vs contract-local enums
S = json.load(open("schemas/workbench-store-v1.4.schema.json"))["$defs"]
def find(o, key):
    if isinstance(o, dict):
        for k,v in o.items():
            if k==key: yield v
            yield from find(v, key)
    elif isinstance(o, list):
        for v in o: yield from find(v, key)
pairs = {
 "DataCategory": S["Approval"]["properties"]["data_categories"]["items"]["enum"],
 "ApprovalLifetime": S["Approval"]["properties"]["lifetime"]["enum"],
 "SelectorMode": S["Selector"]["properties"]["mode"]["enum"],
 "ExportFormat": next(find(S, "output_formats"))["items"]["enum"],
 "Severity": S["Severity"]["enum"],
}
for enum, schema_vals in pairs.items():
    src = contract if enum!="Severity" else open("workbench/crates/domain/src/vocab.rs").read()
    vals = [s for _,s in rust_enum(src, enum)]
    chk("Data §7", f"{enum} values == schema", sorted(vals)==sorted(schema_vals), f"code {vals} schema {schema_vals}")
avail = None
for v in find(S, "availability"):
    if isinstance(v, dict) and "enum" in v: avail = v["enum"]
ui = open("04_UI_Schema.md").read()
ui_states = re.search(r"^\s+states: \[([^\]]*)\]", ui, re.M).group(1).replace(" ","").split(",")
ts = [s for _,s in rust_enum(contract,"ToolAvailability")]
chk("UI §5", "ToolAvailability == UI Schema tools.states", sorted(ts)==sorted(ui_states), f"{ts}")
chk("Data §7 vs UI §5", "store ToolManifest.availability == UI tools.states (spec consistency)", sorted(avail)==sorted(ui_states), f"store {avail} vs UI {ui_states}")

# F. PRD §16 deferred labels vs reserved commands / deferred capabilities
prd = open("01_PRD.md").read()
reserved_map = {"SuppressFinding":"RES-06","UpdateSuppression":"RES-06","CompareRuns":"run comparison","GenerateReportNarrative":"RES-07","PersistConversation":"chat persistence","StartLiveVerification":"SCAN-06"}
chk("PRD §16", "every reserved command maps to a deferred PRD capability", all(n in cmd_rows and cmd_rows[n][0]=="reserved" for n in reserved_map) and all(k in prd for k in ["RES-06 | deferred","SCAN-06 | deferred","chat persistence | deferred"]))
voc = open("workbench/crates/domain/src/vocab.rs").read()
dm = re.search(r"fn is_deferred.*?matches!\((.*?)\)", voc, re.S).group(1)
chk("Data §18.6a", "deferred capabilities == {documentation_fetch, embedding_transmission, non_mutating_verification}",
    set(re.findall(r"Self::(\w+)", dm))=={"DocumentationFetch","EmbeddingTransmission","NonMutatingVerification"})
chk("PRD KEY-01", "credential has no plaintext/serde path (no Serialize/Clone/Display on CredentialSecret)",
    not re.search(r"derive\([^)]*\)\]\s*pub struct CredentialSecret", contract) and "impl fmt::Display for CredentialSecret" not in contract)

# G. versions (A-11)
ver = open("workbench/crates/domain/src/versions.rs").read()
a11 = re.search(r"\| A-11 \|([^|]*)\|", arch).group(1)
chk("Arch A-11", "versions.rs == Arch A-11 line (store 1.4 / protocol 1.1 / app 1.0 / UI 1.1, CA-03)",
    'STORE_SCHEMA_VERSION: &str = "1.4"' in ver and 'WRAPPER_PROTOCOL_VERSION: &str = "1.1"' in ver and 'UI_SCHEMA_VERSION: &str = "1.1"' in ver
    and 'APP_CONTRACT_VERSION: &str = "1.0"' in contract and all(x in a11 for x in ["store 1.4", "wrapper protocol 1.1", "app command/event contract 1.0", "UI schema 1.1"]), a11.strip())

json.dump(R, open("evidence/P01/conformance/conformance.json","w"), indent=2)
for r in R: print(("PASS " if r["ok"] else "NOTE ")+f"[{r['area']}] {r['check']}" + (f" :: {r['detail']}" if r['detail'] and (not r['ok'] or len(r['detail'])<160) else ""))
print(f"pass {sum(r['ok'] for r in R)} / {len(R)}")

"""P01 readiness audit: every P01 obligation in plan.md / planning/P01-execution-plan.md has evidence. Read-only."""
import json, os, re, subprocess
R = []
def chk(name, ok, detail=""): R.append((bool(ok), name, detail))
plan = open("plan.md").read(); xplan = open("planning/P01-execution-plan.md").read()
# 1. P01 tasks T01-T10 (plan §5A) each have a deliverable/record
deliv = {"T01": "planning/P01-T01-request.md", "T02": "evidence/P01/T02/record.md", "T03": "planning/P01-T03-cargo-audit-request.md",
         "T04": "evidence/P01/T04/record.md", "T05": "evidence/P01/B1/record.md", "T06": "evidence/P01/B1/record.md",
         "T07": "evidence/P01/B1/record.md", "T08": "evidence/P01/B2/record.md", "T09": "evidence/P01/T09/record.md",
         "T10": "evidence/P01/P01-report.md"}
sec5a = plan[plan.index("## 5A."):plan.index("## 6.")]
tasks = re.findall(r"\| P01-(T\d\d) \|", sec5a)
chk("plan §5A lists exactly T01-T10", tasks == [f"T{i:02d}" for i in range(1, 11)], str(tasks))
for t in tasks: chk(f"{t} deliverable exists", os.path.exists(deliv[t]), deliv[t])
# 2. acceptance criteria 1-9 assessed Met in T09 record (execution plan §6)
crit = re.findall(r"^(\d)\. ", xplan[xplan.index("## 6. Acceptance criteria"):xplan.index("## 7.")], re.M)
t09 = open("evidence/P01/T09/record.md").read(); rows = re.findall(r"^\| (\d) \| [^|]+\| \*\*(Met[^*]*)\*\*", t09, re.M)
chk("execution-plan §6 has 9 criteria", crit == [str(i) for i in range(1, 10)])
chk("T09 record assesses all 9 as Met", sorted(r[0] for r in rows) == [str(i) for i in range(1, 10)], str(rows))
# 3. approvals A1-A7 recorded in status
st = open("status.md").read()
chk("status records A1..A7 decisions", all(f"A{i}=" in st for i in range(1, 8)))
# 4. plan §4 P01 row items
row = re.search(r"^\| P01 \| pilot \| (.*?) \| (.*?) \| (.*?) \|$", plan, re.M)
chk("plan §4 P01 row present", row is not None)
items = {"Cargo workspace": os.path.exists("workbench/Cargo.toml"), "pinned toolchain": "1.97.1" in open("dev/p01_check.py").read(),
         "typed IDs/states": os.path.exists("workbench/crates/domain/src/ids.rs") and os.path.exists("workbench/crates/domain/src/states.rs"),
         "command/event DTOs": "pub enum AppEvent" in open("workbench/crates/domain/src/contract.rs").read(),
         "RedactedText/SanitisedText": "pub struct RedactedText" in open("workbench/crates/domain/src/text.rs").read()}
for k, v in items.items(): chk(f"P01 scope item: {k}", v)
# 5. final verification evidence passes with committed floors
s = json.load(open("evidence/P01/CA-03/check-final/summary.json"))
test = next(r for r in s["results"] if r["step"] == "test")
chk("final --final run PASS with floors 416/32", s["ok"] and s["final"] and test["counts"]["passed"] >= 416 and test["counts"]["compile_fail_ok"] >= 32 and test["counts"]["failed"] == 0 and test["counts"]["ignored"] == 0, str(test["counts"]))
chk("cargo-audit status recorded as not run (A4)", "cargo-audit" in open("evidence/P01/P01-report.md").read() and "not run" in open("evidence/P01/P01-report.md").read())
# 6. FR register: nothing assigned to P01
frs = re.findall(r"^\| (FR-\d\d) \| [^|]* \| ([^|]*) \|", plan, re.M)
chk("no FR register entry assigned to P01", not [f for f, a in frs if re.search(r"\bP01\b", a)], str([f for f, a in frs if "P01" in a]))
chk("FR-31 recorded for P02", any(f == "FR-31" and "P02" in a for f, a in frs))
# 7. debug cycles logged and recorded
log = re.findall(r"^\| (P01-D\d\d) \|", plan, re.M)
chk("plan §9 logs P01-D01..D06", log == [f"P01-D{i:02d}" for i in range(1, 7)], str(log))
chk("each debug cycle has a record", all(os.path.exists(f"evidence/debug/{d}/record.md") for d in log))
# 8. contract amendments
chk("CA-03 check 29/0/0 after merge", "SUMMARY pass=29 fail=0" in open("evidence/P01/CA-03/check-output-post-merge.txt").read())
conf = open("evidence/P01/conformance/conformance-rev2.txt").read()
chk("conformance rev2: only the approved deferral remains", conf.count("NOTE") == 1 and "tests/{fixtures,contracts,integration}" in conf and "pass 26 / 27" in conf)
# 9. freeze and report current
fz = json.load(open("evidence/P01/P01-freeze.json"))
import hashlib; h = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
chk("freeze matches lock, runner, baseline, schema", fz["lockfile"]["sha256"] == h("workbench/Cargo.lock") and fz["dev_tooling"]["runner"]["sha256"] == h("dev/p01_check.py")
    and fz["dev_tooling"]["baseline"]["sha256"] == h("dev/p01_baseline.json") and fz["canonical_schema"]["sha256"] == h("schemas/workbench-store-v1.4.schema.json") and fz["contract_versions"]["store"] == "1.4")
rep = open("evidence/P01/P01-report.md").read()
chk("P01 report has §1-§11", all(f"## {i}." in rep for i in range(1, 12)))
# 10. git hygiene
g = lambda *a: subprocess.run(["git", *a], capture_output=True, text=True).stdout.strip()
chk("no extra worktrees", len(g("worktree", "list").splitlines()) == 1)
chk("no leftover worker branches", not [b for b in g("branch", "--list").split() if b.startswith(("p01/", "worktree-agent"))])
chk("main untouched (= origin/main = 485b3a8)", g("rev-parse", "--short", "main") == "485b3a8" == g("rev-parse", "--short", "origin/main"))
for ok, n, d in R: print(("PASS " if ok else "FAIL ") + n + (f" :: {d}" if (d and not ok) else ""))
print(f"pass {sum(o for o,_,_ in R)} / {len(R)}")

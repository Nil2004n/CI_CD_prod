"""Offline documentation consistency checks for the CA-05 change (2026-10-31). No network; reads only.
1. Markdown tables: every row has the same number of cells as its header (changed files). Rows that already
   failed at the baseline revision BASE (B2, before CA-05) are reported as BASELINE, others as NEW; any NEW row or
   any other failed check makes the result FAIL, and BASELINE rows also keep the overall result at FAIL (the full
   checker is never reported as passing while they exist).
2. Data Schema §7 embedded schema equals schemas/workbench-store-v1.4.schema.json (unchanged by CA-05).
3. Every ```json block in Data Schema §26 parses and equals the corresponding illustrative example.
4. Mermaid blocks are balanced (opening/closing fences) in changed files.
5. New requirement IDs ANL-01..12 are defined once in the PRD and used consistently (no ANL-13+).
Usage (cwd = repo root): python3 -I -B evidence/CA-05-docs/check_docs.py"""
import json, re, subprocess, sys
from pathlib import Path
BASE = "4d6c70f"
files = ["01_PRD.md", "02_Technical_Specification.md", "03_Architecture.md", "04_UI_Schema.md",
         "05_Database_Data_Schema.md", "plan.md", "README.md", "planning/CA-05-automatic-evidence-analysis.md"]
fails = []
baseline_rows = []
def cells(line):
    t = line.strip().strip("|")
    return len(re.split(r"(?<!\\)\|", re.sub(r"`[^`]*`", "x", t)))
def bad_rows(text):
    out = []; infence = False; header = None
    for i, l in enumerate(text.splitlines(), 1):
        if l.startswith("```"): infence = not infence; header = None; continue
        if infence: continue
        if l.startswith("|"):
            if header is None: header = cells(l)
            elif cells(l) != header: out.append((i, l, cells(l), header))
        else: header = None
    return out, infence
for f in files:
    rows, open_fence = bad_rows(Path(f).read_text())
    try:
        old = subprocess.run(["git", "show", f"{BASE}:{f}"], capture_output=True, text=True, check=True).stdout
        old_rows = {r[1] for r in bad_rows(old)[0]}
    except subprocess.CalledProcessError:
        old_rows = set()
    for i, line, got, want in rows:
        if line in old_rows: baseline_rows.append(f"{f}:{i} table cells {got} != {want} (BASELINE: same row fails at {BASE})")
        else: fails.append(f"{f}:{i} table cells {got} != {want} (NEW)")
    if open_fence: fails.append(f"{f}: unbalanced code fence (NEW)")
print(f"tables/fences: NEW problems {len(fails)}, BASELINE rows {len(baseline_rows)}")
data = Path("05_Database_Data_Schema.md").read_text()
s7 = data.split("## 7. Canonical JSON Schema", 1)[1].split("## 8.", 1)[0]
block = s7.split("```json", 1)[1].split("```", 1)[0]
canon = json.loads(Path("schemas/workbench-store-v1.4.schema.json").read_text())
ok7 = json.loads(block) == canon
print("§7 embedded == canonical 1.4:", ok7); fails += [] if ok7 else ["§7 copy differs"]
s26 = data.split("## 26.", 1)[1]
blocks = [json.loads(b.split("```", 1)[0]) for b in s26.split("```json")[1:]]
ex = json.loads(Path("planning/CA-05-draft/examples.illustrative.json").read_text())
ok26 = blocks == [ex["blocked_approval_denied"], ex["run_summary_fragment"]]
print("§26 examples parse and equal draft file:", ok26); fails += [] if ok26 else ["§26 examples differ"]
prd = Path("01_PRD.md").read_text()
defined = re.findall(r"^\| (ANL-\d\d) \|", prd, re.M)
allids = set(re.findall(r"ANL-\d\d", " ".join(Path(f).read_text() for f in files)))
okid = defined == [f"ANL-{n:02d}" for n in range(1, 13)] and allids <= set(defined)
print("ANL IDs defined once, 01..12, no strays:", okid, sorted(allids - set(defined)))
fails += [] if okid else ["ANL id problem"]
for f in baseline_rows: print(" - ", f)
for f in fails: print(" - ", f)
print(f"OVERALL {'FAIL' if fails or baseline_rows else 'PASS'} (CA-05 regressions: {len(fails)}; pre-existing baseline rows: {len(baseline_rows)})")
sys.exit(1 if fails or baseline_rows else 0)

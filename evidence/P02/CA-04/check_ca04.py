"""CA-04 consistency check. Exit 1 on any FAIL."""
import hashlib, pathlib, re, sys
docs = ["01_PRD.md", "02_Technical_Specification.md", "03_Architecture.md", "04_UI_Schema.md", "05_Database_Data_Schema.md"]
res = []
def check(name, ok, detail=""): res.append((ok, name, detail))
for d in docs:
    t = pathlib.Path(d).read_text()
    check(f"{d} header 0.3.5 with app contract 1.1", t.splitlines()[4].startswith("Version: 0.3.5 (amendments CA-01, CA-02, CA-03, CA-04; store contract 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1)"))
    check(f"{d} no 0.3.4 header left", "Version: 0.3.4" not in t)
# Remaining "1.0" app-contract mentions must be historical statements only.
allowed = ["app contract 1.0 unchanged as no implementation had been released", "app contract stays **1.0** because no implementation had been published",
           "No data was stored under app contract 1.0", "app contract 1.0 → **1.1**", "1.0 → 1.1 by CA-04"]
for d in docs:
    for i, l in enumerate(pathlib.Path(d).read_text().splitlines(), 1):
        if re.search(r"(app (command/event )?contract|app_contract_version)[^.;|]{0,40}\b(v?1\.0)\b", l):
            ok = any(a in l for a in allowed)
            check(f"{d}:{i} app-contract 1.0 mention is historical", ok, l[:140])
arch = pathlib.Path("03_Architecture.md").read_text()
check("Arch §6 heading v1.1", "(app contract v1.1, CA-01, CA-04)" in arch)
check("Arch §6 envelope paragraph", "Event envelope (CA-04)" in arch)
check("Arch §3 CLI paragraph", "A headless developer CLI (`pwt-workbench-cli`, D-17, CA-04)" in arch)
check("Arch §4 CLI row", "| CLI (developer, D-17) |" in arch)
check("Arch A-11 1.1", "app command/event contract 1.1 (CA-04)" in arch)
check("Tech §4 cli/", "    cli/             # headless developer CLI" in pathlib.Path("02_Technical_Specification.md").read_text())
ui = pathlib.Path("04_UI_Schema.md").read_text()
check("UI §17 parity paragraph", "CLI parity (CA-04)" in ui)
check("UI §17 app contract v1.1", "(app contract v1.1)." in ui)
data = pathlib.Path("05_Database_Data_Schema.md").read_text()
check("Data §17 v0.3.5 / app 1.1", "This document is v0.3.5;" in data and "is v1.1 (CA-04, §25)" in data)
check("Data §25 present once", data.count("## 25. Contract amendment CA-04") == 1)
before = dict(reversed(l.split()) for l in pathlib.Path("evidence/P02/CA-04/schemas-before.sha256").read_text().splitlines())
for f, h in before.items():
    check(f"{f} unchanged", hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest() == h)
check("store schema pinned hash", hashlib.sha256(pathlib.Path("schemas/workbench-store-v1.4.schema.json").read_bytes()).hexdigest()
      == "909e2a55c315ca07096f9123e6986b637a83b1bcdfc4eb6ff88d9d06daa80e2e")
for ok, n, d in res: print("PASS" if ok else "FAIL", n, ("— " + d) if d and not ok else "")
p = sum(r[0] for r in res); print(f"summary {p}/{len(res)-p}/0"); sys.exit(0 if p == len(res) else 1)

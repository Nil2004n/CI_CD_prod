"""P00-T03 document check: no guessed versions; every candidate row unresolved or not proposed. Reads only."""
import re, sys
t = open(sys.argv[1] + "/spikes/P00-T03-crate-request.md").read()
res = []
def ok(n, c): res.append(c); print(("PASS " if c else "FAIL ") + n)
rows = [l for l in t.splitlines() if re.match(r"^\| S-\d\d \|", l)]
ok(f"Tier S rows present ({len(rows)})", len(rows) == 15)
cols = [[c.strip() for c in r.strip("|").split("|")] for r in rows]
bad = [c[0] for c in cols if not re.search(r"unresolved|not proposed", c[4])]
ok("every Tier S 'Proposed version' cell is unresolved/not proposed" + (f" (bad: {bad})" if bad else ""), not bad)
guessed = [c[0] for c in cols if re.search(r"\b\d+\.\d+(\.\d+)?\b", c[4])]
ok("no version numbers in 'Proposed version' cells" + (f" (rows: {guessed})" if guessed else ""), not guessed)
lic = [c[0] for c in cols if c[6] not in ("—",) and "to confirm" not in c[6]]
ok("every stated licence marked 'to confirm'" + (f" (rows: {lic})" if lic else ""), not lic)
ok("toolchain version matches local check (1.97.1)", "rustc 1.97.1" in t)
ok("states no lookup performed", "No external lookup was made" in t)
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

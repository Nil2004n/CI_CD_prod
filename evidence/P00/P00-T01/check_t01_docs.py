"""P00-T01 document checks: fixture cross-references and secret-shaped strings. Reads only."""
import re, sys
root = sys.argv[1]
P = f"{root}/spikes/P00-T01-spike-protocols.md"; F = f"{root}/spikes/P00-T01-fixture-specifications.md"
prot, fix = open(P).read(), open(F).read()
res = []
def ok(n, c): res.append(c); print(("PASS " if c else "FAIL ") + n)
idre = re.compile(r"FX-[A-Z0-9]+(?:-[A-Z0-9]+)*(?:-\*|-n)?")
defined = set(re.findall(r"^\| (FX-[A-Z0-9-]+)", fix, re.M))
for row in re.findall(r"^\| (FX-[A-Z0-9-]+ / FX-[A-Z0-9-]+)", fix, re.M):
    defined.update(x.strip() for x in row.split("/"))
ok(f"fixture catalogue defines IDs ({len(defined)})", len(defined) > 50)
def resolves(ref):
    if ref.endswith("-*"):
        return any(d.startswith(ref[:-1]) for d in defined)
    return ref in defined
refs = sorted(set(idre.findall(prot)))
missing = [r for r in refs if not resolves(r)]
ok(f"all {len(refs)} fixture references in protocols resolve" + (f" (missing: {missing})" if missing else ""), not missing)
frefs = sorted(set(idre.findall(fix)) - defined)
fmissing = [r for r in frefs if not resolves(r) and r not in ("FX-ID",)]
ok("all fixture references inside catalogue resolve" + (f" (missing: {fmissing})" if fmissing else ""), not fmissing)
areas = re.findall(r"^\| (FX-[A-Z0-9-]+\*(?:, FX-[A-Z0-9-]+\*)*) \|", fix, re.M)
prefixes = [p.strip()[:-1] for a in areas for p in a.split(",")]
uncovered = sorted(d for d in defined if not any(d.startswith(p) for p in prefixes))
ok("every defined fixture falls under a coverage-map area" + (f" (uncovered: {uncovered})" if uncovered else ""), not uncovered)
pats = {"PEM block": r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "GitHub-style token": r"gh[pousr]_[A-Za-z0-9]{30,}",
        "AWS key id": r"\b(AKIA|ASIA)[A-Z0-9]{16}\b", "Slack token": r"xox[abprs]-[A-Za-z0-9-]{10,}",
        "long high-entropy run (letters+digits+uppercase)": r"(?=[A-Za-z0-9+/=_-]*\d)(?=[A-Za-z0-9+/=_-]*[A-Z])[A-Za-z0-9+/=_-]{40,}", "URL with credentials": r"[a-z]+://[^/\s:@]+:[^/\s@]+@"}
for name, rx in pats.items():
    hits = [(f.split('/')[-1], m.group(0)[:12] + "…") for f, t in ((P, prot), (F, fix)) for m in re.finditer(rx, t)]
    ok(f"no {name} in T01 documents" + (f" (hits: {hits})" if hits else ""), not hits)
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

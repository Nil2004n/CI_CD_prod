"""P00-T03 rev 2 check: request values traceable to research log. Reads only."""
import re, sys
r = sys.argv[1]
req = open(f"{r}/spikes/P00-T03-crate-request.md").read(); log = open(f"{r}/evidence/P00/P00-T03/research.md").read()
res = []
def ok(n, c): res.append(c); print(("PASS " if c else "FAIL ") + n)
pairs = re.findall(r"\b(libc|rustix|nix|tokio|saphyr-parser|saphyr|yaml-rust2|jsonschema|boon|serde_json|serde|keyring|pulldown-cmark|comrak|printpdf|genpdf|typst|sha2|tempfile) (\d+\.\d+\.\d+)\b", req)
missing = sorted({f"{c} {v}" for c, v in pairs if not re.search(rf"\| {re.escape(c)} \| {re.escape(v)} \|", log)})
ok(f"{len(set(pairs))} crate/version pairs in request all match research table" + (f" (missing: {missing})" if missing else ""), not missing)
ok("libc checksum prefix in request matches research", "65cd87c9" in req and "65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a" in log)
ok("no claim that transitive graphs were reviewed", "Transitive graphs are **not reviewed**" in req and "Transitive dependency graphs were **not** reviewed" in log)
ok("states nothing downloaded/resolved/compiled", "No crate archive has been downloaded, no dependency resolved, nothing compiled or executed" in req)
ok("does not claim --offline sandboxes", "not merely by `--offline`" in req and "no filesystem sandbox" in req)
msrv = [(c, v) for c, v in re.findall(r"\| (\w[\w-]*) \| \d+\.\d+\.\d+ \| [\d-]+ \| [^|]+ \| (\d+\.\d+(?:\.\d+)?) \|", log) if tuple(map(int, v.split(".")[:2])) > (1, 97)]
ok("all declared MSRVs <= 1.97" + (f" (over: {msrv})" if msrv else ""), not msrv)
p = sum(res); print(f"SUMMARY pass={p} fail={len(res)-p} skip=0"); sys.exit(0 if p == len(res) else 1)

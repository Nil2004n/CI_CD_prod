"""P01-T02 acceptance check: every registry entry of the resolved P01 lock must equal a P00 lock entry
(name, version, source, checksum); expected set and member set are fixed by planning/P01-T01-request.md §4."""
import sys, tomllib, json
p01 = tomllib.load(open(sys.argv[1], "rb"))["package"]
p00 = tomllib.load(open(sys.argv[2], "rb"))["package"]
key = lambda p: (p["name"], p["version"], p.get("source"), p.get("checksum"))
p00k = {key(p) for p in p00}
EXPECTED = {"serde", "serde_core", "serde_derive", "serde_json", "zmij", "itoa", "memchr",
            "proc-macro2", "quote", "syn", "unicode-ident"}
MEMBERS = {"domain", "application", "repositories", "scanning", "retrieval", "persistence", "terminal-ui"}
checks = []
reg = [p for p in p01 if p.get("source")]
mem = [p for p in p01 if not p.get("source")]
for p in reg:
    checks.append((f"{p['name']} {p['version']} in P00 lock with same source+checksum", key(p) in p00k))
checks.append(("registry set == expected set", {p["name"] for p in reg} == EXPECTED))
checks.append(("one version per registry crate", len(reg) == len({p["name"] for p in reg})))
checks.append(("member set == 7 workspace crates", {p["name"] for p in mem} == MEMBERS))
checks.append(("all registry sources are crates.io", all(p["source"] == "registry+https://github.com/rust-lang/crates.io-index" for p in reg)))
checks.append(("all registry entries carry checksums", all(p.get("checksum") for p in reg)))
for n, ok in checks: print(("PASS " if ok else "FAIL ") + n)
f = sum(not ok for _, ok in checks)
print(json.dumps({"pass": len(checks) - f, "fail": f, "skip": 0}))
sys.exit(1 if f else 0)

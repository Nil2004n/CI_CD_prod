"""P01-T02 acceptance check (v2): every registry entry of the resolved P01 lock must equal a P00 lock entry
(name, version, source, checksum); expected set and member set are fixed by planning/P01-T01-request.md §4."""
import sys, tomllib, json
p01 = tomllib.load(open(sys.argv[1], "rb"))["package"]
p00 = tomllib.load(open(sys.argv[2], "rb"))["package"]
key = lambda p: (p["name"], p["version"], p.get("source"), p.get("checksum"))
p00k = {key(p) for p in p00}
# v2 (2026-10-26): exact versions, after v1 (names only) let syn 3.0.9 pass unnoticed against a planning
# document that said syn 2.0.119. syn 3.0.9 is serde_derive 1.0.229's actual dependency in the P00 lock.
EXPECTED_V = {"serde": "1.0.229", "serde_core": "1.0.229", "serde_derive": "1.0.229", "serde_json": "1.0.154",
              "zmij": "1.0.23", "itoa": "1.0.18", "memchr": "2.8.3", "proc-macro2": "1.0.107", "quote": "1.0.47",
              "syn": "3.0.9", "unicode-ident": "1.0.27"}
EXPECTED = set(EXPECTED_V)
MEMBERS = {"domain", "application", "repositories", "scanning", "retrieval", "persistence", "terminal-ui"}
checks = []
reg = [p for p in p01 if p.get("source")]
mem = [p for p in p01 if not p.get("source")]
for p in reg:
    checks.append((f"{p['name']} {p['version']} in P00 lock with same source+checksum", key(p) in p00k))
checks.append(("registry set == expected set", {p["name"] for p in reg} == EXPECTED))
checks.append(("registry (name, version) == expected exact versions", {(p["name"], p["version"]) for p in reg} == set(EXPECTED_V.items())))
checks.append(("one version per registry crate", len(reg) == len({p["name"] for p in reg})))
checks.append(("member set == 7 workspace crates", {p["name"] for p in mem} == MEMBERS))
checks.append(("all registry sources are crates.io", all(p["source"] == "registry+https://github.com/rust-lang/crates.io-index" for p in reg)))
checks.append(("all registry entries carry checksums", all(p.get("checksum") for p in reg)))
for n, ok in checks: print(("PASS " if ok else "FAIL ") + n)
f = sum(not ok for _, ok in checks)
print(json.dumps({"pass": len(checks) - f, "fail": f, "skip": 0}))
sys.exit(1 if f else 0)

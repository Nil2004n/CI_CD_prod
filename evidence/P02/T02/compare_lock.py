"""P02-T02 A2 check: every registry entry of the resolved lock equals a P00 lock entry (name, version, source,
checksum); every P01 entry is unchanged. Usage: compare_lock.py <resolved> <p00 lock> <p01 lock>. Exit 1 on any FAIL."""
import sys, tomllib
def load(p):
    return {(x['name'], x['version']): x for x in tomllib.load(open(p, 'rb'))['package']}
new, p00, p01 = (load(p) for p in sys.argv[1:4])
fails = passes = 0
for k, x in sorted(new.items()):
    if 'source' not in x:
        print(f"LOCAL {k[0]} {k[1]}"); continue
    ref = p00.get(k)
    ok = ref is not None and all(x.get(f) == ref.get(f) for f in ('source', 'checksum'))
    print(("PASS" if ok else "FAIL"), k[0], k[1], x.get('checksum', '-')[:16], "" if ref else "(not in P00 lock)")
    passes += ok; fails += not ok
for k, x in sorted(p01.items()):
    ok = new.get(k) == x or ('source' not in x)
    if not ok: print("FAIL P01 entry changed:", k)
    passes += ok; fails += not ok
deps_changed = [k for k, x in p01.items() if 'source' in x and new.get(k, {}).get('dependencies') != x.get('dependencies')]
print(f"summary pass={passes} fail={fails}; P01 registry entries with changed dependency edges: {deps_changed}")
sys.exit(1 if fails else 0)

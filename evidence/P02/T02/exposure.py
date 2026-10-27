"""P02-T02 A3 check. For each registry package in the P02 Linux host graph: does it have a build script or is it a
proc-macro (read from the cached .crate's Cargo.toml without extracting), and is it, at the same version and with a
superset of features, in at least one accepted, built P00 host graph (T05, T06, T07, T08)?
Usage: exposure.py <p02 features> <cache dir> <p00 graph files...>. Exit 1 if any build script/proc-macro is new."""
import hashlib, re, sys, tarfile, tomllib, pathlib
LINE = re.compile(r"^(\S+) v(\S+)(?: \([^)]*\))? \[(.*)\]$")
def parse(p):
    out = {}
    for l in open(p):
        m = LINE.match(l.strip())
        if m: out.setdefault((m[1], m[2]), set()).update(f for f in m[3].split(',') if f)
    return out
p02 = parse(sys.argv[1]); cache = pathlib.Path(sys.argv[2])
graphs = {pathlib.Path(g).name: parse(g) for g in sys.argv[3:]}
rows, new = [], 0
for (n, v), feats in sorted(p02.items()):
    crate = cache / f"{n}-{v}.crate"
    if not crate.exists():
        rows.append(f"{n} {v}: workspace member or not cached"); continue
    with tarfile.open(crate) as t:
        names = t.getnames()
        man = tomllib.loads(t.extractfile(f"{n}-{v}/Cargo.toml").read().decode())
    pkg = man.get('package', {})
    bs = pkg.get('build') not in (None, False) or f"{n}-{v}/build.rs" in names
    pm = bool(man.get('lib', {}).get('proc-macro') or man.get('lib', {}).get('proc_macro'))
    links = pkg.get('links')
    cover = [g for g, d in graphs.items() if (n, v) in d and feats <= d[(n, v)]]
    seen = [g for g, d in graphs.items() if (n, v) in d]
    risky = bs or pm or links
    status = "ok" if cover else ("NEW-EXPOSURE" if risky else "new-plain")
    if risky and not cover: new += 1
    rows.append(f"{n} {v} build={'y' if bs else '-'} proc={'y' if pm else '-'} links={links or '-'} "
                f"features=[{','.join(sorted(feats))}] covered_by={','.join(cover) or '-'} seen_in={','.join(seen) or '-'} => {status}")
print("\n".join(rows))
print(f"summary packages={len(p02)} build_scripts={sum('build=y' in r for r in rows)} proc_macros={sum('proc=y' in r for r in rows)} "
      f"links={sum('links=-' not in r and 'build=' in r for r in rows)} new_exposure={new} new_plain={sum(r.endswith('new-plain') for r in rows)}")
sys.exit(1 if new else 0)

"""P01-T02 exposure report: per resolved registry crate, list build script / proc-macro from the cached .crate
archive (read-only; tarfile listing + Cargo.toml parse, nothing extracted to disk), and check features are a subset
of those built in accepted P00 graphs (union of the given P00 feature listings)."""
import sys, tarfile, tomllib, re, json, hashlib, pathlib
lock, p01feat, cache = sys.argv[1], sys.argv[2], pathlib.Path(sys.argv[3])
p00files = sys.argv[4:]
pk = [p for p in tomllib.load(open(lock, "rb"))["package"] if p.get("source")]
line = re.compile(r"^(\S+) v(\S+)(?: \(proc-macro\))? \[([^\]]*)\]")
def feats(path):
    out = {}
    for l in open(path):
        m = line.match(l.strip())
        if m: out.setdefault((m[1], m[2]), set()).update(f for f in m[3].split(",") if f)
    return out
p01 = feats(p01feat); p00 = {}
for f in p00files:
    for k, v in feats(f).items(): p00.setdefault(k, set()).update(v)
rows, fail = [], 0
for p in sorted(pk, key=lambda p: p["name"]):
    n, v = p["name"], p["version"]
    crate = cache / f"{n}-{v}.crate"
    sha = hashlib.sha256(crate.read_bytes()).hexdigest()
    with tarfile.open(crate) as t:
        names = t.getnames()
        man = tomllib.loads(t.extractfile(f"{n}-{v}/Cargo.toml").read().decode())
    pkg = man.get("package", {})
    b = pkg.get("build")
    has_build = (b not in (False, None)) or (b is None and f"{n}-{v}/build.rs" in names)
    build_path = (b if isinstance(b, str) else "build.rs") if has_build else None
    pm = bool(man.get("lib", {}).get("proc-macro"))
    links = pkg.get("links")
    f1 = p01.get((n, v), set()); f0 = p00.get((n, v))
    in_p00 = f0 is not None; subset = in_p00 and f1 <= f0
    ok = sha == p["checksum"] and in_p00 and subset and links is None
    fail += not ok
    rows.append(dict(crate=f"{n} {v}", archive_sha256_matches_lock=sha == p["checksum"], build_script=build_path,
                     proc_macro=pm, links=links, p01_features=sorted(f1), in_accepted_p00_graph=in_p00,
                     p01_features_subset_of_p00=subset, ok=ok))
for r in rows:
    print(("PASS " if r["ok"] else "FAIL ") + json.dumps(r))
bs = [r["crate"] for r in rows if r["build_script"]]; pm = [r["crate"] for r in rows if r["proc_macro"]]
print(json.dumps({"build_scripts": bs, "proc_macros": pm, "pass": len(rows) - fail, "fail": fail, "skip": 0}))
sys.exit(1 if fail else 0)

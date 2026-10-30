"""Lead negative controls for P03 wave 1: each mutant must make `test --package application` FAIL."""
import pathlib, subprocess, sys, time
copy = pathlib.Path(sys.argv[1]); out = pathlib.Path(sys.argv[2])
A = copy / "workbench/crates/application/src"
M = [
 ("M1 G13 expiry ignored", A/"permission/evaluate.rs", "approval.expires_at.is_none_or(|end| end > view.ctx.now)", "approval.expires_at.is_none_or(|_end| true)"),
 ("M2 inherit any host name", A/"child_env.rs", "|| !spec.inherited.contains(&name)", "|| spec.inherited.is_empty()"),
 ("M3 probe not applied to inherited", A/"child_env.rs", "bytes.contains(&0) || probe.appears_in(bytes)", "bytes.contains(&0)"),
 ("M4 appears_in always false", A/"credential.rs", "value.windows(key.len()).any(|window| window == key)", "{ let _ = (value, key); false }"),
]
res = []
for name, f, old, new in M:
    src = f.read_text(); assert src.count(old) == 1, name
    f.write_text(src.replace(old, new))
    o = out / name.split()[0]
    p = subprocess.run([sys.executable, "-I", "-B", str(copy/"dev/p03_check.py"), "test", "--slot", "lead", "--package", "application", "--repo", str(copy), "--out", str(o)], capture_output=True, text=True)
    f.write_text(src); time.sleep(1); f.touch()
    line = [l for l in p.stdout.splitlines() if l.startswith(("PASS test", "FAIL test"))]
    ok = p.returncode != 0 and line and line[0].startswith("FAIL")
    res.append(ok); print(name, "->", "CAUGHT" if ok else "SURVIVED", line[:1], flush=True)
p = subprocess.run([sys.executable, "-I", "-B", str(copy/"dev/p03_check.py"), "test", "--slot", "lead", "--package", "application", "--repo", str(copy), "--out", str(out/"restored")], capture_output=True, text=True)
print("restored copy:", [l for l in p.stdout.splitlines() if "test" in l and ("PASS" in l or "FAIL" in l)][:1])
print("NEGATIVE CONTROLS", "PASS" if all(res) and p.returncode == 0 else "FAIL")

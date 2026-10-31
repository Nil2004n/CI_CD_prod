"""Builds evidence/P03/P03-freeze.json from the repository as it stands (read-only inventory).
Usage (cwd = repo root): python3 -I -B evidence/P03/T12/make_freeze.py <pwt-workbench-cli binary>"""
import hashlib, json, subprocess, sys, tomllib
from pathlib import Path
binary = sys.argv[1]
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
ws = tomllib.loads(Path("workbench/Cargo.toml").read_text())
lock = tomllib.loads(Path("workbench/Cargo.lock").read_text())
reg = [p for p in lock["package"] if p.get("source", "").startswith("registry+")]
summary = json.loads(Path("evidence/P03/T12/final/summary.json").read_text())
static = json.loads(Path("evidence/P03/T12/final/static.json").read_text())
def run(*a):
    return [json.loads(l) for l in subprocess.run([binary, "--json", *a], capture_output=True, text=True, env={}).stdout.splitlines() if l.strip()]
helps, feats = run("help"), run("features")
steps = {r["step"]: {k: r[k] for k in ("exit", "counts") if k in r} for r in summary["results"]}
doc = {
    "phase": "P03", "date": "2026-11-01",
    "status": "inventory at P03 completion (code = D02 091224b); not adoption or build permission for later phases",
    "toolchain": {"rustc": "1.97.1 (8bab26f4f 2026-07-14)", "pin_method": "direct toolchain binaries + exact 1.97.1 gate in dev/p03_check.py"},
    "workspace": {"members": ws["workspace"]["members"], "binary": "pwt-workbench-cli (crates/cli)"},
    "direct_dependencies": ws["workspace"].get("dependencies", {}),
    "new_dependency_edges_in_p03": ["application -> sha2 (T02; lock edges only, registry set unchanged)"],
    "lockfile": {"path": "workbench/Cargo.lock", "sha256": sha("workbench/Cargo.lock"), "packages_total": len(lock["package"]), "registry_crate_count": len(reg)},
    "advisory_status": {"tool": "cargo-audit 0.22.2 (binary sha256 3a9bfe66…)", "db": "rustsec advisory-db 2295466c (2026-10-30)", "result": "94 crates, 0 vulnerabilities, 0 warnings (offline, --no-fetch, unshare -rn)", "evidence": "evidence/P03/T12/audit.json", "limitation": "yanked check not run offline; DB not refreshed since 2026-10-30"},
    "contract_versions": {"store": "1.4", "wrapper_protocol": "1.1", "app_contract": "1.1", "ui_schema": "1.1", "document": "0.3.5", "p03_contract_amendment": "none (no CA-05 for P03; T01-D2). CA-05 in planning is the unrelated, unapplied automatic-analysis draft"},
    "canonical_schema": {"path": "schemas/workbench-store-v1.4.schema.json", "sha256": sha("schemas/workbench-store-v1.4.schema.json")},
    "cli": {"commands": [h.get("command") or h.get("name") for h in helps], "command_count": len(helps), "feature_index_rows": len(feats)},
    "dev_tooling": {"runner": "dev/p03_check.py", "runner_sha256": sha("dev/p03_check.py"), "floors": json.loads(Path("dev/p03_baseline.json").read_text())},
    "final_verification": {"command": "python3 -I -B dev/p03_check.py all --final --slot lead --out evidence/P03/T12/final", "repo_head": summary["repo_head"], "dirty": summary["dirty"], "steps": steps, "overall": all(r.get("ok", True) for r in summary["results"])},
}
Path("evidence/P03/P03-freeze.json").write_text(json.dumps(doc, indent=1) + "\n")
print(json.dumps({k: doc[k] for k in ("lockfile", "cli", "final_verification")}, indent=1)[:1500])

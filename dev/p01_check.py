"""P01 development check runner (planning/P01-T01-request.md §2, §3, §6.4). Development only; not shipped.

Usage: python3 -I -B dev/p01_check.py <mode> --slot <lead|w1|w2|w3> --out <dir> [--package domain] [--final]
Modes: env, fmt, clippy, test, build, static, all (= env fmt clippy test build static).

Every Cargo command runs the 1.97.1 toolchain binaries directly, with `env -i` and the isolated
CARGO_HOME, inside `unshare -rn`, with --frozen (fmt has no network use). Exit codes are captured
from the producer process itself. Cargo.lock and every Cargo.toml are hashed before and after each step.
"""

import argparse
import hashlib
import json
import re
import subprocess
import sys
import time
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WS = REPO / "workbench"
PWT = Path("/home/kali/.local/share/pwt-dev-tools")
TC = Path("/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin")
TOOLCHAIN_VERSION = "1.97.1"
SLOTS = ("lead", "w1", "w2", "w3")
EXPECTED_REGISTRY = {
    "serde": "1.0.229", "serde_core": "1.0.229", "serde_derive": "1.0.229", "serde_json": "1.0.154",
    "zmij": "1.0.23", "itoa": "1.0.18", "memchr": "2.8.3", "proc-macro2": "1.0.107", "quote": "1.0.47",
    "syn": "3.0.9", "unicode-ident": "1.0.27",
}
MEMBERS = {"domain", "application", "repositories", "scanning", "retrieval", "persistence", "terminal-ui"}
ALLOWED_DIRECT_DEPS = {"serde", "serde_json"}
TEST_RESULT = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out")


def cargo_env(slot):
    return [
        f"PATH={TC}:/usr/bin:/bin",
        f"HOME={PWT}/_cargo-home",
        f"CARGO_HOME={PWT}/_cargo-home",
        f"CARGO_TARGET_DIR={PWT}/_cargo-target/p01-{slot}",
        f"RUSTC={TC}/rustc",
        f"RUSTDOC={TC}/rustdoc",
        "CARGO_TERM_COLOR=never",
        "LANG=C.UTF-8",
    ]


def protected_hashes():
    files = [WS / "Cargo.lock", WS / "Cargo.toml", *sorted((WS / "crates").glob("*/Cargo.toml"))]
    return {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None for p in files}


def run_cargo(name, args, slot, out):
    log = out / f"{name}.log"
    before = protected_hashes()
    cmd = ["unshare", "-rn", "env", "-i", *cargo_env(slot), str(TC / "cargo"), *args]
    t0 = time.monotonic()
    with open(log, "wb") as fh:
        rc = subprocess.run(cmd, cwd=WS, stdout=fh, stderr=subprocess.STDOUT, check=False).returncode
    after = protected_hashes()
    res = {"step": name, "command": " ".join(["cargo", *args]), "exit": rc, "log": str(log.relative_to(out)),
           "seconds": round(time.monotonic() - t0, 2), "protected_files_unchanged": before == after}
    if before != after:
        res["changed"] = [k for k in before if before[k] != after.get(k)]
    res["ok"] = rc == 0 and before == after
    return res


def step_env(slot, out):
    checks = []
    for tool in ("rustc", "cargo", "rustdoc", "rustfmt", "cargo-clippy"):
        p = subprocess.run(["env", "-i", f"PATH={TC}:/usr/bin:/bin", str(TC / tool), "--version"],
                           capture_output=True, text=True, check=False)
        ver = p.stdout.strip()
        expect = TOOLCHAIN_VERSION if tool in ("rustc", "cargo", "rustdoc") else None
        ok = p.returncode == 0 and (expect is None or ver.split()[1:2] == [expect])
        checks.append({"check": f"{tool} --version", "value": ver, "ok": ok})
    probe = ("import socket\ns=socket.socket();s.settimeout(3)\n"
             "try:\n s.connect(('1.1.1.1',443));print('REACHABLE')\nexcept OSError as e:\n print('errno',e.errno)")
    p = subprocess.run(["unshare", "-rn", sys.executable, "-I", "-c", probe], capture_output=True, text=True, check=False)
    checks.append({"check": "netns outbound connect fails", "value": p.stdout.strip(),
                   "ok": p.returncode == 0 and p.stdout.startswith("errno")})
    checks.append({"check": f"slot {slot} target dir", "value": cargo_env(slot)[3], "ok": slot in SLOTS})
    (out / "env.json").write_text(json.dumps(checks, indent=2) + "\n")
    return {"step": "env", "checks": checks, "ok": all(c["ok"] for c in checks)}


def code_lines(path):
    """Yields (lineno, line) with // comments stripped (string contents are not parsed)."""
    for i, line in enumerate(path.read_text().splitlines(), 1):
        yield i, line.split("//", 1)[0] if "//" in line and not line.strip().startswith("///") else line


def enum_block(text, name):
    m = re.search(rf"pub enum {name}\b[^{{]*\{{", text)
    if not m:
        return None
    depth, i = 1, m.end()
    while depth and i < len(text):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        i += 1
    return text[m.end():i - 1]


def step_static(final, out):
    checks = []
    rs = sorted(WS.rglob("*.rs"))
    src = [p for p in rs if "/src/" in str(p)]
    hits = [f"{p.relative_to(REPO)}:{i}" for p in rs for i, l in code_lines(p)
            if re.search(r"\bunsafe\s*(\{|fn\b|impl\b|trait\b)", l)]
    checks.append({"check": "no unsafe blocks/fns/impls", "hits": hits, "ok": not hits})
    hits = [f"{p.relative_to(REPO)}:{i}" for p in src for i, l in code_lines(p)
            if re.search(r"\b(e?println|e?print|dbg)!", l)]
    checks.append({"check": "no print/dbg macros in src", "hits": hits, "ok": not hits})
    hits = [f"{p.relative_to(REPO)}:{i}" for p in rs for i, l in code_lines(p)
            if re.search(r"\btodo!|\bunimplemented!|clippy::todo", l)]
    checks.append({"check": "no todo!/unimplemented!/allow(clippy::todo)" + ("" if final else " (informational before T09)"),
                   "hits": len(hits), "ok": (not hits) if final else True})
    contract = WS / "crates/domain/src/contract.rs"
    block = enum_block(contract.read_text(), "AppEvent") if contract.exists() else None
    if block is None:
        checks.append({"check": "AppEvent has no raw string field types", "ok": not final,
                       "note": "AppEvent not present" + (" (required at --final)" if final else " yet")})
    else:
        bad = [l.strip() for l in block.splitlines() if re.search(r":\s*(&\s*'?\w*\s*str\b|String\b|Box<str>|Cow<)", l)]
        checks.append({"check": "AppEvent has no raw string field types", "hits": bad, "ok": not bad})
        cblock = enum_block(contract.read_text(), "AppCommand") or ""
        bad = [l.strip() for l in cblock.splitlines() if re.search(r":\s*(&\s*'?\w*\s*str\b|String\b|Box<str>|Cow<)", l)
               and "// raw-input:" not in l]
        checks.append({"check": "AppCommand raw string fields are marked // raw-input:", "hits": bad, "ok": not bad})
    lock = tomllib.loads((WS / "Cargo.lock").read_text())["package"]
    reg = {(p["name"], p["version"]) for p in lock if p.get("source")}
    mem = {p["name"] for p in lock if not p.get("source")}
    checks.append({"check": "Cargo.lock registry set == approved exact versions",
                   "extra": sorted(map(list, reg - set(EXPECTED_REGISTRY.items()))),
                   "missing": sorted(map(list, set(EXPECTED_REGISTRY.items()) - reg)),
                   "ok": reg == set(EXPECTED_REGISTRY.items())})
    checks.append({"check": "Cargo.lock members == 7 workspace crates", "ok": mem == MEMBERS})
    direct = set()
    for man in [WS / "Cargo.toml", *sorted((WS / "crates").glob("*/Cargo.toml"))]:
        t = tomllib.loads(man.read_text())
        for tbl in (t.get("workspace", {}).get("dependencies", {}), t.get("dependencies", {}),
                    t.get("dev-dependencies", {}), t.get("build-dependencies", {})):
            direct |= set(tbl)
        for k in t.get("target", {}).values():
            for tbl in ("dependencies", "dev-dependencies", "build-dependencies"):
                direct |= set(k.get(tbl, {}))
    checks.append({"check": "manifest dependencies within {serde, serde_json}", "found": sorted(direct),
                   "ok": direct <= ALLOWED_DIRECT_DEPS})
    (out / "static.json").write_text(json.dumps(checks, indent=2) + "\n")
    return {"step": "static", "checks": checks, "ok": all(c["ok"] for c in checks)}


def test_counts(log):
    tot = {"suites": 0, "passed": 0, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0}
    for m in TEST_RESULT.finditer(log.read_text(errors="replace")):
        tot["suites"] += 1
        for k, v in zip(("passed", "failed", "ignored", "measured", "filtered_out"), m.groups()[1:]):
            tot[k] += int(v)
    return tot


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["env", "fmt", "clippy", "test", "build", "static", "all"])
    ap.add_argument("--slot", required=True, choices=SLOTS)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--package")
    ap.add_argument("--final", action="store_true")
    a = ap.parse_args()
    out = a.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    scope = ["-p", a.package] if a.package else ["--workspace"]
    modes = ["env", "fmt", "clippy", "test", "build", "static"] if a.mode == "all" else [a.mode]
    head = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "-C", str(REPO), "status", "--short"], capture_output=True, text=True).stdout
    results = []
    for m in modes:
        if m == "env":
            r = step_env(a.slot, out)
        elif m == "fmt":
            r = run_cargo("fmt", ["fmt", "--all", "--", "--check"], a.slot, out)
        elif m == "clippy":
            r = run_cargo("clippy", ["clippy", *scope, "--all-targets", "--frozen", "--", "-D", "warnings"], a.slot, out)
        elif m == "test":
            r = run_cargo("test", ["test", *scope, "--frozen"], a.slot, out)
            r["counts"] = test_counts(out / "test.log")
            if r["counts"]["ignored"] or r["counts"]["failed"] or not r["counts"]["suites"]:
                r["ok"] = False
        elif m == "build":
            r = run_cargo("build", ["build", *scope, "--frozen"], a.slot, out)
        else:
            r = step_static(a.final, out)
        results.append(r)
        if m == "env" and not r["ok"]:
            break
    summary = {"tool": "dev/p01_check.py", "mode": a.mode, "slot": a.slot, "package": a.package, "final": a.final,
               "repo_head": head, "dirty": dirty.splitlines(), "results": results,
               "ok": all(r["ok"] for r in results) and len(results) == len(modes)}
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for r in results:
        extra = f" counts={r['counts']}" if "counts" in r else ""
        print(f"{'PASS' if r['ok'] else 'FAIL'} {r['step']} exit={r.get('exit', '-')}{extra}")
    print("OVERALL", "PASS" if summary["ok"] else "FAIL")
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())

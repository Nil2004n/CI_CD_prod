"""Manual-pending check from evidence/P02/P02-report.md §3: run
real_storage_failure_exits_4_when_permissions_are_enforced with file permissions enforced.

Same toolchain, env -i environment, CARGO_HOME, target dir and --frozen as dev/p02_check.py; the only
difference is the namespace: `unshare -Un --map-current-user` (uid stays 1000, so permissions apply, and
the network namespace is still empty) instead of `unshare -rn` (uid maps to root).

The test returns early when permissions are not enforced, so a pass alone proves nothing. A host-side
poller records the test's temp-dir names under a private TMPDIR: `eacces-read` / `eacces-write` exist
only on the enforced branch. Control: the same run under `unshare -rn` must not show them.
Usage: python3 -I -B run.py <out dir>
"""
import hashlib, json, os, subprocess, sys, threading, time
from pathlib import Path

REPO = Path("/home/kali/Desktop/CI_CD_prod")
WS = REPO / "workbench"
PWT = Path("/home/kali/.local/share/pwt-dev-tools")
TC = Path("/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin")
TARGET = PWT / "_cargo-target" / f"p02-lead-{hashlib.sha256(str(REPO).encode()).hexdigest()[:12]}"
TEST = "real_storage_failure_exits_4_when_permissions_are_enforced"
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=True)

PROBE = r"""
import os, socket, tempfile, json
r = {"uid": os.getuid(), "euid": os.geteuid()}
d = tempfile.mkdtemp(); p = os.path.join(d, "probe"); open(p, "w").write("x"); os.chmod(p, 0)
try: open(p).close(); r["mode000_open"] = "succeeded"
except PermissionError: r["mode000_open"] = "EACCES"
os.chmod(p, 0o600); os.remove(p); os.rmdir(d)
s = socket.socket(); s.settimeout(2)
try: s.connect(("1.1.1.1", 443)); r["net"] = "connected"
except OSError as e: r["net"] = f"errno {e.errno}"
print(json.dumps(r))
"""


def env(tmpdir):
    return [f"PATH={TC}:/usr/bin:/bin", f"HOME={PWT}/_cargo-home", f"CARGO_HOME={PWT}/_cargo-home",
            f"CARGO_TARGET_DIR={TARGET}", f"RUSTC={TC}/rustc", f"RUSTDOC={TC}/rustdoc",
            "CARGO_TERM_COLOR=never", "LANG=C.UTF-8", f"TMPDIR={tmpdir}"]


def protected():
    files = [WS / "Cargo.lock", WS / "Cargo.toml", *sorted((WS / "crates").glob("*/Cargo.toml"))]
    return {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}


def run(label, ns):
    tmpdir = OUT / f"tmp-{label}"
    tmpdir.mkdir(mode=0o700, exist_ok=True)
    probe = subprocess.run([*ns, "env", "-i", *env(tmpdir), sys.executable, "-I", "-c", PROBE],
                           capture_output=True, text=True, check=False)
    seen, stop = set(), threading.Event()

    def poll():
        while not stop.is_set():
            try:
                seen.update(n.rsplit("-", 2)[0] for n in os.listdir(tmpdir))
            except OSError:
                pass
            time.sleep(0.0005)

    t = threading.Thread(target=poll); t.start()
    before = protected()
    cmd = [*ns, "env", "-i", *env(tmpdir), str(TC / "cargo"), "test", "-p", "cli", "--frozen",
           "--test", "p02_failures", "--", TEST, "--exact", "--test-threads=1"]
    with open(OUT / f"{label}.log", "wb") as fh:
        rc = subprocess.run(cmd, cwd=WS, stdout=fh, stderr=subprocess.STDOUT, check=False).returncode
    stop.set(); t.join()
    log = (OUT / f"{label}.log").read_text(errors="replace")
    left = sorted(os.listdir(tmpdir))
    return {"label": label, "namespace": " ".join(ns), "probe": probe.stdout.strip() or probe.stderr.strip(),
            "command": " ".join(cmd[cmd.index(str(TC / "cargo")):]), "exit": rc,
            "result_line": [l for l in log.splitlines() if l.startswith("test result:")],
            "tmp_dirs_seen": sorted(seen), "tmp_left_after": left, "protected_files_unchanged": before == protected()}


res = [run("enforced", ["unshare", "-Un", "--map-current-user"]), run("control-root-mapped", ["unshare", "-rn"])]
e, c = res
verdict = {
    "enforced_uid_1000": json.loads(e["probe"]).get("euid") == 1000,
    "enforced_mode000_EACCES": json.loads(e["probe"]).get("mode000_open") == "EACCES",
    "enforced_no_network": json.loads(e["probe"]).get("net", "").startswith("errno"),
    "enforced_test_passed": e["exit"] == 0 and any("1 passed; 0 failed" in l for l in e["result_line"]),
    "enforced_branch_ran": {"pwt-cli-p02f-eacces-read", "pwt-cli-p02f-eacces-write"} <= set(e["tmp_dirs_seen"]),
    "control_root_mapped": json.loads(c["probe"]).get("mode000_open") == "succeeded",
    "control_branch_skipped": not any("eacces-read" in n or "eacces-write" in n for n in c["tmp_dirs_seen"]),
    "control_test_passed": c["exit"] == 0,
    "no_tmp_left": not e["tmp_left_after"] and not c["tmp_left_after"],
    "protected_files_unchanged": e["protected_files_unchanged"] and c["protected_files_unchanged"],
}
verdict["overall"] = all(verdict.values())
json.dump({"runs": res, "verdict": verdict}, open(OUT / "result.json", "w"), indent=2)
print(json.dumps(verdict, indent=2))
sys.exit(0 if verdict["overall"] else 1)

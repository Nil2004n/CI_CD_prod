"""Lead seeded-key sweep (P03-T11) against the real pwt-workbench-cli binary built by the lead check.
Runs every P03 command as separate processes with a synthetic key on stdin, in the environment (innocent and
key-like names, inside a longer value, in TZ) and as a permission-check argument value; then searches every
stdout/stderr, the argv used and every file under the data dir for the key and an 8-byte core fragment.
resource_constraints is deliberately not seeded (recorded limitation: stored verbatim, sanitised).
Usage: unshare -rn python3 -I -B evidence/P03/B3/lead_sweep.py <binary> <scratch-dir>"""
import json, os, subprocess, sys, uuid
from pathlib import Path

binary, scratch = sys.argv[1], Path(sys.argv[2])
KEY = "pwt-test-FAKE-" + uuid.uuid4().hex + "-SWEEPKEY"
CORE = KEY[14:22]
data = scratch / "data"; data.mkdir(parents=True, mode=0o700); os.chmod(data, 0o700)
env = {"PATH": "/usr/bin:/bin", "LANG": "C", "OPENROUTER_API_KEY": KEY, "INNOCENT": KEY,
       "BUILD_NOTE": "prefix-" + KEY + "-suffix", "MY_TOKEN": KEY, "TZ": KEY}
outs, argvs, codes = [], [], []

def run(args, stdin=b""):
    argv = [binary, *args]; argvs.append(" ".join(argv))
    p = subprocess.run(argv, input=stdin, capture_output=True, env=env, timeout=60)
    outs.append(p.stdout + p.stderr); codes.append((" ".join(args[:2]), p.returncode)); return p

D = ["--data-dir", str(data)]
run(["store", "init", *D])
run(["credential", "set", "--session"], (KEY + "\n").encode())
run(["credential", "status"]); run(["credential", "delete"])
for tool in ["git", "yamllint", "gitleaks", "checkov", "actionlint", "kubeconform", "internal"]:
    run(["dev", "env-preview", tool, *D]); run(["--json", "dev", "env-preview", tool, *D])
req = {"capability": "git_acquire", "operation": "git.clone", "arguments": [{"name": "token", "value": KEY}],
       "destinations": ["https://example.com/org/repo"], "data": ["repository_metadata"], "lifetime": "once"}
p = run(["--json", "dev", "permission-check", *D], json.dumps(req).encode())
aid = next((json.loads(l).get("approval_id") for l in p.stdout.decode().splitlines() if "approval_id" in l), None)
run(["approvals", "list", *D]); run(["--json", "approvals", "list", *D])
if aid:
    run(["approvals", "approve", aid, *D]); run(["dev", "permission-check", *D], json.dumps(req).encode())
    run(["approvals", "list", *D])
hits = []
for i, o in enumerate(outs):
    if KEY.encode() in o or CORE.encode() in o: hits.append(f"output #{i}")
for a in argvs:
    if KEY in a or CORE in a: hits.append("argv")
files = [f for f in data.rglob("*") if f.is_file()]
for f in files:
    b = f.read_bytes()
    if KEY.encode() in b or CORE.encode() in b or KEY in f.name: hits.append(f"file {f.relative_to(data)}")
print("exit codes:", codes)
print(f"processes {len(outs)}, files scanned {len(files)}, approval recorded {bool(aid)}")
print("hits:", hits or "none")
# non-vacuous: the key really was offered (stdin accepted, env present)
print("credential set exit:", codes[1][1])
sys.exit(1 if hits or not files or not aid else 0)

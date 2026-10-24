"""P00-T09 harness self-test (Opus-owned). Runs inside the spike namespace before any tool.

Children are `/usr/bin/python3 -I -B -c <probe>` only (enumerated helper). Every probe is spike-owned
and writes only inside its case directory. Checks:
- S2: process-group cleanup (lingering same-group descendant, timeout with SIGTERM, SIGTERM-ignoring
  escalation to SIGKILL, a deliberately escaping descendant — bounded harness return, NOT containment);
- per-child RLIMIT_AS (default 4 GiB; the 16 GiB value used for gitleaks);
- S3: redaction before excerpt truncation, partial canary at the excerpt and raw-capture boundaries,
  C03's full value and partials, partial fragments anywhere;
- S4: control/bidi escaping in excerpts and notes, Markdown escaping for contract sheets;
- loopback server: lo up only for its lifetime, controlled directory only, request log, shutdown,
  no listener/thread left, lo down after.
"""

from __future__ import annotations

import http.client
import os
import signal
import time
from pathlib import Path

import common

PY = ["/usr/bin/python3", "-I", "-B", "-c"]

LINGER = """
import os, sys, time
pid = os.fork()
if pid == 0:
    open(sys.argv[1], "w").write(str(os.getpid()))
    time.sleep(30)
    os._exit(0)
time.sleep(0.3)
print("parent done", flush=True)
"""

HANG = """
import os, sys, time, signal
if sys.argv[2] == "ignore-term":
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
pid = os.fork()
if pid == 0:
    open(sys.argv[1], "w").write(str(os.getpid()))
    time.sleep(60)
    os._exit(0)
time.sleep(60)
"""

ESCAPE = """
import os, sys, time
pid = os.fork()
if pid == 0:
    os.setsid()
    open(sys.argv[1], "w").write(str(os.getpid()))
    time.sleep(20)
    os._exit(0)
time.sleep(0.3)
"""

LIMITS = """
for l in open("/proc/self/limits"):
    if l.startswith("Max address space"):
        print(l.split()[3], l.split()[4])
"""

EMIT = """
import sys
data = open(sys.argv[1], "rb").read()
sys.stdout.buffer.write(data)
"""


def _alive(pid: int) -> bool:
    p = Path(f"/proc/{pid}/stat")
    if not p.exists():
        return False
    s = p.read_text()
    return s[s.rfind(")") + 2] != "Z"


def cases(ctx):
    checks = ctx.checks = []
    red = ctx.red
    c = ctx.canaries

    def ck(name, ok, detail=""):
        checks.append({"check": name, "ok": bool(ok), "detail": ctx.clean(detail)})

    def run(cid, code, args, **kw):
        d = ctx.case_dir(cid)
        return d, ctx.run(cid, [ctx.bin] + PY[1:] + [code] + [str(a) for a in args], d / "repo",
                          purpose="harness self-test", fixtures=[], **kw)

    bound = lambda extra: int((extra + 1.5) * 1000)

    # ---------------------------------------------------------------- S2
    d, r = run("st-normal", "print('ok')", [])
    ck("s2_normal_exit_no_kill", r["exit"] == 0 and not r["timed_out"] and not r["cleanup"]["kill_sent"]
       and not r["cleanup"]["group_killed_after_exit"], r["cleanup"])

    d = ctx.case_dir("st-linger")
    pf = d / "gc.pid"
    r = ctx.run("st-linger", [ctx.bin, "-I", "-B", "-c", LINGER, str(pf), "pwt-t09-selftest-linger"], d / "repo",
                purpose="same-group grandchild keeps stdout open after the child exits", fixtures=[])
    gpid = int(pf.read_text())
    ck("s2_linger_bounded_and_group_killed",
       r["exit"] == 0 and not r["timed_out"] and r["cleanup"]["kill_reason"] == "pipes held after child exit"
       and r["wall_ms"] < bound(common.LINGER_S + common.DRAIN_S) and not _alive(gpid),
       {"wall_ms": r["wall_ms"], "cleanup": r["cleanup"], "grandchild_alive": _alive(gpid)})

    for cid, mode, want_sig, reason in (("st-timeout-term", "plain", signal.SIGTERM, None),
                                        ("st-timeout-kill", "ignore-term", signal.SIGKILL, "timeout escalation")):
        d = ctx.case_dir(cid)
        pf = d / "gc.pid"
        r = ctx.run(cid, [ctx.bin, "-I", "-B", "-c", HANG, str(pf), mode], d / "repo",
                    purpose=f"timeout, child+grandchild hang ({mode})", fixtures=[], timeout=2)
        gpid = int(pf.read_text())
        ok = (r["timed_out"] and r["signal"] == want_sig and not _alive(gpid)
              and r["wall_ms"] < bound(2 + common.TERM_GRACE_S + common.DRAIN_S)
              and (reason is None or r["cleanup"]["kill_reason"] == reason))
        ck(f"s2_{cid.split('-', 1)[1].replace('-', '_')}", ok,
           {"signal": r["signal"], "wall_ms": r["wall_ms"], "cleanup": r["cleanup"], "grandchild_alive": _alive(gpid)})

    d = ctx.case_dir("st-escape")
    pf = d / "gc.pid"
    r = ctx.run("st-escape", [ctx.bin, "-I", "-B", "-c", ESCAPE, str(pf), "pwt-t09-selftest-escape"], d / "repo",
                purpose="descendant leaves the process group (setsid) and keeps stdout open", fixtures=[])
    gpid = int(pf.read_text())
    alive_after = _alive(gpid)
    cleaned = False
    if alive_after:
        cmd = Path(f"/proc/{gpid}/cmdline").read_bytes()
        if b"pwt-t09-selftest-escape" in cmd:          # exact spike-owned PID, identity checked
            os.kill(gpid, signal.SIGKILL)
            for _ in range(50):
                common.reap_orphans()
                if not _alive(gpid):
                    cleaned = True
                    break
                time.sleep(0.1)
    ck("s2_escaped_descendant_harness_bounded",
       r["wall_ms"] < bound(common.LINGER_S + common.DRAIN_S) and r["cleanup"]["pipes_force_closed"] and (cleaned or not alive_after),
       {"wall_ms": r["wall_ms"], "cleanup": r["cleanup"], "escaped_alive_after_run": alive_after,
        "contained_by_process_group": not alive_after, "removed_by_exact_pid": cleaned})

    # ---------------------------------------------------------------- RLIMIT_AS
    d, r = run("st-rlimit-default", LIMITS, [])
    ck("rlimit_default_4GiB", r["_stdout"].split() == [b"4294967296", b"4294967296"], r["_stdout"].decode())
    d, r = run("st-rlimit-gitleaks-value", LIMITS, [], rlimit_as=common.RLIMIT_AS_BY_TOOL["gitleaks"])
    ck("rlimit_gitleaks_value_16GiB", r["_stdout"].split() == [b"17179869184", b"17179869184"]
       and common.RLIMIT_AS_BY_TOOL == {"gitleaks": 16 * common.GIB}, r["_stdout"].decode())

    # ---------------------------------------------------------------- S3
    def emit(cid, data: bytes):
        d = ctx.case_dir(cid)
        (d / "payload").write_bytes(data)
        return ctx.run(cid, [ctx.bin, "-I", "-B", "-c", EMIT, str(d / "payload")], d / "repo",
                       purpose="emit synthetic payload", fixtures=[])

    def clean_text(s: str) -> bool:
        if "PWTFAKE" in s or red.hits(s.encode()) or red._tail_prefix_len(s):
            return False
        for cid, v in c.items():
            rnd = v[v.index(ctx.red.markers[cid]) + len(ctx.red.markers[cid]):]
            if any(rnd[i:i + 6] in s for i in range(0, len(rnd) - 5)):
                return False
        return True

    for off in (5, 20, 40):
        r = emit(f"st-excerpt-{off}", b"x" * (common.EXCERPT - off) + c["C01"].encode() + b"y" * 200 + b"\n")
        ck(f"s3_excerpt_boundary_{off}", "C01" in r["canary_in_stdout"] and r["excerpt_truncated"]["stdout"]
           and clean_text(r["stdout_excerpt"]), {"tail": r["stdout_excerpt"][-24:]})

    for cid_, off in (("C02", 5), ("C03", 8), ("C04", 12)):
        r = emit(f"st-capture-{cid_}", b"x" * (common.OUTPUT_CAP - off) + c[cid_].encode() + b"\n")
        tail = r["_stdout"][-80:].decode()
        ck(f"s3_capture_boundary_{cid_}", cid_ in r["canary_in_stdout"] and r["retained_trailing_partial_removed"]["stdout"]
           and r["stdout_truncated_for_analysis"] and clean_text(tail) and clean_text(r["stdout_excerpt"]),
           {"retained_tail": tail[-20:], "stdout_bytes": r["stdout_bytes"]})

    r = emit("st-c03", (c["C03"] + "\n" + c["C03"][:15] + "\n" + c["C04"][:20] + "\n" + c["C05"][:10] + "zz\n").encode())
    ex = r["stdout_excerpt"]
    ck("s3_c03_full_and_partials", r["canary_in_stdout"] == ["C03", "C04", "C05"] and "<canary:C03>" in ex
       and "<canary-partial:C03>" in ex and "<canary-partial:C04>" in ex and clean_text(ex), {"excerpt": ex})

    r = emit("st-b64", ("data: " + __import__("base64").b64encode(c["C09"].encode()).decode() + "\n").encode())
    ck("s3_base64_form_detected_and_redacted", "C09" in r["canary_in_stdout"] and "<canary-b64:C09>" in r["stdout_excerpt"],
       {"excerpt": r["stdout_excerpt"]})

    # ---------------------------------------------------------------- S4
    r = emit("st-control", b"\x1b[31mRED\x1b[0m\x07\xe2\x80\xaeabc\r\n")
    ex = r["stdout_excerpt"]
    ck("s4_control_chars_escaped_in_excerpt", all(t in ex for t in ("<U+001B>", "<U+0007>", "<U+202E>", "<U+000D>"))
       and not any(ch in ex for ch in "\x1b\x07‮\r"), {"excerpt": ex})

    ctx.note("selftest_note_probe", {"key\x1b": "v\x1b " + c["C05"][:24], "list": ["‮" + c["C06"]]})
    v = ctx.notes[-1]["value"]
    flat = repr(v)
    ck("s4_note_escaped_and_redacted", "\x1b" not in flat and "‮" not in flat and "<canary-partial:C05>" in flat
       and "<canary:C06>" in flat and clean_text(flat), v)

    md = common.md_escape("| a | `b` [x](http://e) <script>*_# \n- item\x1b")
    ck("s4_markdown_escaped", "\n" not in md and "<" not in md and ">" not in md and "\x1b" not in md
       and all(md[i - 1] == "\\" for i, ch in enumerate(md) if ch in "|`[]()*_#"), {"escaped": md})

    # ---------------------------------------------------------------- loopback server
    d = ctx.case_dir("st-loopback")
    (d / "srv").mkdir()
    (d / "srv" / "x.json").write_text('{"type": "object"}')
    (d / "outside.txt").write_text("not served\n")
    obs = {}
    try:
        with common.LoopbackSchemaServer(d / "srv") as srv:
            conn = http.client.HTTPConnection("127.0.0.1", srv.port, timeout=3)
            conn.request("GET", "/x.json")
            obs["get_status"] = conn.getresponse().status
            conn.close()
            conn = http.client.HTTPConnection("127.0.0.1", srv.port, timeout=3)
            conn.request("GET", "/../outside.txt")
            obs["traversal_status"] = conn.getresponse().status
            conn.close()
            srv.stop_server()
            obs["connect_after_stop"] = common.connect_ok(srv.port, timeout=1)
        obs.update(srv.obs)
        obs["requests"] = srv.requests
    except OSError as e:
        obs["error"] = f"errno {e.errno}"
    ck("loopback_server_lifecycle",
       obs.get("get_status") == 200 and obs.get("traversal_status") == 404 and obs.get("lo_up_before") is False
       and obs.get("lo_up_after_enable") is True and obs.get("thread_alive_after_stop") is False
       and obs.get("listeners_after_stop") == [] and str(obs.get("connect_after_stop", "")).startswith("failed")
       and obs.get("lo_up_after_exit") is False and obs.get("listeners_after_exit") == []
       and obs.get("requests") == ["GET /x.json 200", "GET /../outside.txt 404"], obs)

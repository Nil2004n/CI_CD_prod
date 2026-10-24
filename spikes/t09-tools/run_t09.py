"""P00-T09 orchestrator (Opus-owned). Must run as PID 1 of a dedicated user+net+PID namespace.

Usage: python3 -I -B -X pycache_prefix=<root>/pycache run_t09.py <spike-root> <out-dir> <tool>[,<tool>...] [preflight-only]

Writes <out-dir>/results.json (sanitised). Raw outputs stay under <spike-root>, which the outer
script removes. The harness self-test (`cases_selftest`) always runs after preflight. Exit 1 if
preflight or the self-test fails (no tool runs then).
"""

from __future__ import annotations

import sys

sys.dont_write_bytecode = True  # belt and braces next to -B

import hashlib  # noqa: E402
import importlib  # noqa: E402
import json  # noqa: E402
import os  # noqa: E402
import resource  # noqa: E402
import secrets  # noqa: E402
import traceback  # noqa: E402
from pathlib import Path  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import common  # noqa: E402
import fixtures  # noqa: E402

TRUSTED = {  # evidence/P00/P00-T02/install-record.md "Trusted hashes"
    "gitleaks": "88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509",
    "actionlint": "c872d6db8c6bf83a8eaa704fc93999f027d55dffbc63b8a6abdccb47df5f4cd4",
    "kubeconform": "457722b36e98d7bdbc91525e594951ef5d7ea3a29da2d13b9b28a2fb8a5097b7",
    "python3.12": "58fe899223901d346e9d3a0184bdd04d82a276ff1f99be0e68d5873ea71c1b68",
}
PY312 = common.USER_HOME / ".local/share/pwt-dev-tools/python/3.12.15+20261021/python/bin/python3.12"
# Host-side name-resolution / proxy IPC endpoints that would let a child bypass the network namespace
# (filesystem sockets are not network-namespaced). Any present one fails preflight.
DNS_IPC_SOCKETS = ["/run/nscd/socket", "/var/run/nscd/socket", "/run/systemd/resolve/io.systemd.Resolve",
                   "/run/avahi-daemon/socket", "/var/run/avahi-daemon/socket"]
NSS_ALLOWED_HOST_SOURCES = {"files", "myhostname", "mdns4_minimal", "[NOTFOUND=return]", "dns"}


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else ""


def _routes() -> dict:
    v4 = [l.split() for l in Path("/proc/net/route").read_text().splitlines()[1:] if l.strip()]
    v6 = [l.split() for l in Path("/proc/net/ipv6_route").read_text().splitlines() if l.strip()] if Path("/proc/net/ipv6_route").exists() else []
    return {"ipv4_entries": len(v4), "ipv4_default": any(r[1] == "00000000" for r in v4),
            "ipv6_non_lo": sum(1 for r in v6 if r[-1] != "lo"),
            "ipv6_default": any(r[0] == "0" * 32 and r[1] == "00" and r[-1] != "lo" for r in v6)}


def preflight(root: Path, tools: list[str]) -> tuple[bool, list[dict]]:
    checks = []

    def ck(name, ok, detail=""):
        checks.append({"check": name, "ok": bool(ok), "detail": str(detail)})

    ck("pid1", os.getpid() == 1, os.getpid())
    host = dict(l.split("=", 1) for l in (root / "host-ns.txt").read_text().split())
    mine = {k: os.readlink(f"/proc/self/ns/{k}") for k in ("net", "user", "pid")}
    for k in ("net", "user", "pid"):
        ck(f"namespace_{k}_differs_from_host", host.get(k) and mine[k] != host[k], f"host {host.get(k)} / here {mine[k]}")
    ifaces = [l.split(":")[0].strip() for l in Path("/proc/net/dev").read_text().splitlines()[2:]]
    ck("netns_interfaces_loopback_only", ifaces == ["lo"], ifaces)
    ck("loopback_down_at_start", not common.lo_is_up(), f"flags=0x{common.lo_flags():x}")
    r = _routes()
    ck("no_routes_no_default_route", r["ipv4_entries"] == 0 and not r["ipv4_default"] and r["ipv6_non_lo"] == 0 and not r["ipv6_default"], r)
    ck("inherited_env_exactly_LANG_PATH", sorted(os.environ) == ["LANG", "PATH"], sorted(os.environ))
    ck("child_env_allowlist_has_no_proxy", not any("proxy" in n.lower() for n in common.CHILD_ENV_NAMES), common.CHILD_ENV_NAMES)
    present = [s for s in DNS_IPC_SOCKETS if os.path.exists(s)]
    ck("no_host_dns_ipc_sockets", not present, present or "none present")
    hosts = next((l.split()[1:] for l in Path("/etc/nsswitch.conf").read_text().splitlines() if l.startswith("hosts:")), [])
    ck("nss_hosts_sources_known", set(hosts) <= NSS_ALLOWED_HOST_SOURCES, hosts)
    # Loopback toggle (in-namespace ioctl, no extra executable): up -> listen/connect -> down -> connect fails.
    try:
        common.set_lo(True)
        import socket
        with socket.socket() as srv:
            srv.bind(("127.0.0.1", 0))
            srv.listen(1)
            port = srv.getsockname()[1]
            up_res = common.connect_ok(port)
        common.set_lo(False)
        down_res = common.connect_ok(port, timeout=1)
        ck("loopback_toggle_works", up_res == "connected" and down_res != "connected" and not common.lo_is_up(),
           f"up:{up_res} down:{down_res}")
    except OSError as e:
        ck("loopback_toggle_works", False, f"errno {e.errno}")
    ck("no_tcp_listeners", not common.tcp_listeners(), common.tcp_listeners())
    ck("spike_root_prefix", str(root).startswith(str(common.USER_HOME / ".local/share/pwt-dev-tools/_spikes/t09.")), root)
    ck("spike_root_0700", (root.stat().st_mode & 0o777) == 0o700, oct(root.stat().st_mode & 0o777))
    ck("python_isolated_and_no_bytecode", sys.flags.isolated == 1 and sys.dont_write_bytecode and sys.flags.dont_write_bytecode == 1,
       f"isolated={sys.flags.isolated} -B={sys.flags.dont_write_bytecode}")
    ck("pycache_prefix_inside_spike_root", bool(sys.pycache_prefix) and str(sys.pycache_prefix).startswith(str(root)), sys.pycache_prefix)
    ck("no_repo_pycache_dir", not (HERE / "__pycache__").exists(), HERE / "__pycache__")
    ck("sys_path_excludes_spike_root", not any(str(p).startswith(str(root)) for p in sys.path), "")
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    ck("orchestrator_rlimit_as_soft4_hard16_GiB", soft == 4 * common.GIB and hard == 16 * common.GIB, f"soft={soft} hard={hard}")
    for t in tools:
        if t == "selftest":
            continue
        b = common.TOOLS[t]
        ck(f"tool {t} present", b.is_file(), b)
        ck(f"tool {t} child rlimit_as", True, common.RLIMIT_AS_BY_TOOL.get(t, common.RLIMIT_AS_DEFAULT))
        if t in TRUSTED:
            ck(f"tool {t} sha256 trusted", b.is_file() and sha256(b) == TRUSTED[t], sha256(b) if b.is_file() else "")
        if t in ("yamllint", "checkov", "checkov_r2"):
            ck("python3.12 sha256 trusted", sha256(PY312) == TRUSTED["python3.12"], sha256(PY312))
    for helper in ("shellcheck", "pyflakes"):
        ck(f"helper {helper} absent from PATH", not any((Path(d) / helper).exists() for d in "/usr/bin:/bin".split(":")), "")
    return all(c["ok"] for c in checks), checks


def _snapshot(paths: list[Path], skip: set[Path]) -> dict[str, tuple]:
    snap = {}
    for base in paths:
        if not base.exists():
            continue
        it = [base] + list(base.rglob("*")) if base.is_dir() else [base]
        for p in it:
            if any(p == s or s in p.parents for s in skip):
                continue
            try:
                st = p.lstat()
            except OSError:
                continue
            snap[str(p)] = (st.st_size, st.st_mtime_ns)
    return snap


def _top_names(d: Path) -> set[str]:
    try:
        return {p.name for p in d.iterdir()}
    except OSError:
        return set()


OBSERVED_OUTSIDE = [Path("/tmp"), common.USER_HOME / ".cache", common.USER_HOME / ".config", common.USER_HOME]


def tool_guard(text: str, red: common.Redactor) -> list[str]:
    hits = set(red.hits(text.encode()))
    if "PWTFAKE" in text:
        hits.add("PWTFAKE-substring")
    return sorted(hits)


def main() -> int:
    root, out, tools = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3].split(",")
    preflight_only = len(sys.argv) > 4 and sys.argv[4] == "preflight-only"
    if "selftest" not in tools:
        tools = ["selftest"] + tools
    pf_ok, pf = preflight(root, tools)
    result = {"preflight_ok": pf_ok, "preflight": pf, "rlimit_as": {"by_tool": common.RLIMIT_AS_BY_TOOL, "default": common.RLIMIT_AS_DEFAULT},
              "cleanup_bounds_s": {"linger": common.LINGER_S, "term_grace": common.TERM_GRACE_S, "drain": common.DRAIN_S},
              "tools": {}}
    canaries = fixtures.gen_canaries()
    markers = fixtures.canary_markers(canaries)
    components = fixtures.synthetic_components(canaries)
    red = common.Redactor(canaries, markers, components)
    result["registered_synthetic_components"] = sorted(components)   # IDs only, never values
    sentinel = root / "sentinel" / "SENTINEL.txt"
    sentinel.parent.mkdir(mode=0o700)
    sentinel.write_text("pwt-t09 synthetic sentinel " + secrets.token_hex(8) + "\n")
    sentinel_hash = sha256(sentinel)
    run_tools = [] if not pf_ok else (["selftest"] if preflight_only else tools)
    for t in run_tools:
        ctx = common.Ctx(root, t, canaries, markers, components)
        skip = {ctx.work, root / "out"}
        snap0 = _snapshot([root], skip)
        outside0 = {str(d): _top_names(d) for d in OBSERVED_OUTSIDE}
        try:
            mod = importlib.import_module(f"cases_{t}")
            mod.cases(ctx)
            status = "completed"
        except Exception:
            status = "harness_error: " + ctx.sanitise(traceback.format_exc()[-1500:])
        common.reap_orphans()
        snap1 = _snapshot([root], skip)
        outside1 = {str(d): _top_names(d) for d in OBSERVED_OUTSIDE}
        exported = ctx.export()
        exported["status"] = status
        if hasattr(ctx, "checks"):
            exported["checks"] = ctx.checks
        exported["execution_markers_in_tool_workdir"] = ctx.markers()
        exported["execution_markers_in_spike_root_so_far"] = sorted(str(m.relative_to(root)) for m in root.rglob("PWT_EXECUTED_*"))
        exported["write_inventory"] = {
            "spike_root_changes_outside_tool_workdir": sorted(ctx.sanitise(k.replace(str(root), "<root>")) for k in set(snap1) ^ set(snap0) | {k for k in snap0.keys() & snap1.keys() if snap0[k] != snap1[k]}),
            "sentinel_unchanged": sha256(sentinel) == sentinel_hash,
            "new_top_level_names_outside_root": {d.replace(str(common.USER_HOME), "~"): sorted(ctx.sanitise(n) for n in outside1[d] - outside0[d]) for d in outside1},
            "scope": "tracking only (no filesystem confinement); paths outside the spike root are observed only as new top-level names in the listed directories",
        }
        leaked = tool_guard(json.dumps(exported, ensure_ascii=False), red)
        if leaked:
            exported = {"tool": t, "status": status, "error": "tool results suppressed: contained canary material", "canary_ids": leaked}
        result["tools"][t] = exported
        if t == "selftest" and (status != "completed" or not getattr(ctx, "checks", None) or not all(c["ok"] for c in ctx.checks)):
            result["selftest_failed"] = True
            break
    text = json.dumps(result, indent=1, ensure_ascii=False)
    leaked = tool_guard(text, red)  # final fail-closed guard over everything persisted
    if leaked:
        text = json.dumps({"error": "results suppressed: contained canary material", "canary_ids": leaked})
    (out / "results.json").write_text(text)
    print("t09: done")
    return 0 if pf_ok and not result.get("selftest_failed") else 1


if __name__ == "__main__":
    sys.exit(main())

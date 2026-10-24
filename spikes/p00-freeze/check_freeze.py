"""P00-T11 freeze consistency checker (Opus-owned; stdlib only, offline, read-only).

Usage: python3 -I -B check_freeze.py <repo-root>

Prints one PASS/FAIL line per check and exits 1 on any FAIL. Checks (request spikes/P00-T11-request.md §4):
- tools.json / crates.json shape, fixed vocabularies, required tool ids, base commit;
- tool hashes with basis "install-record trusted hashes" equal the install record's Trusted-hashes table, and the
  installed binary's current SHA-256 (read-only hashing; nothing is executed);
- every direct spike dependency (spikes/rust/*/Cargo.toml) frozen with the same version and default-features, and
  present in Cargo.lock at that version; lockfile hash; K-A advisory ids recorded;
- every referenced repository path exists; every FR id exists in plan.md §5.2;
- the P00 report covers P00-T01..T11 and FR-01..FR-30 and states that P00 acceptance is pending.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path

ROLES = {"baseline", "framework", "interpreter", "install-helper", "verification-helper", "deferred"}
TOOL_STATUS = {"accepted-qualified", "accepted-limitations", "pending", "deferred"}
HASH_BASIS = {"install-record trusted hashes", "observed (not trusted)", "none"}
SHEET_KIND = {"generated", "opus-authored"}
SHEET_STATUS = {"current", "superseded-for-invocation"}
CRATE_STATUS = {"accepted-qualified", "provisional", "deferred", "excluded", "fallback"}
REQUIRED_TOOLS = {"yamllint", "gitleaks", "checkov", "actionlint", "kubeconform", "python3.12", "uv", "cosign", "cargo-audit"}
TOOL_KEYS = {"id", "role", "version", "path", "sha256", "hash_basis", "status", "contract", "invocation", "rejected_invocations",
             "coverage", "limitations", "requirements", "evidence"}
CRATE_KEYS = {"name", "version", "default_features", "features", "spike_package", "purpose", "status", "advisory_exceptions",
              "evidence", "notes"}
KA_IDS = {"RUSTSEC-2026-0370", "RUSTSEC-2026-0371", "RUSTSEC-2026-0372"}
HOME = Path("/home/kali")


def main() -> int:
    root = Path(sys.argv[1])
    res = []

    def ck(name, ok, detail=""):
        res.append(bool(ok))
        print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""))

    def exists(p):
        return isinstance(p, str) and not p.startswith(("/", "~")) and ".." not in p and (root / p).exists()

    plan = (root / "plan.md").read_text()
    fr_ids = set(re.findall(r"^\| (FR-\d+) \|", plan, re.M))
    install = (root / "evidence/P00/P00-T02/install-record.md").read_text()
    trusted = {m.group(1): m.group(2) for m in re.finditer(r"^\| `([^`]+)` \| `([0-9a-f]{64})` \|", install, re.M)}

    # ------------------------------------------------------------------ tools.json
    t = json.loads((root / "spikes/p00-freeze/tools.json").read_text())
    ck("tools: freeze_version/base_commit", t.get("freeze_version") == "1" and t.get("base_commit") == "3decec1", (t.get("freeze_version"), t.get("base_commit")))
    ids = [x.get("id") for x in t.get("tools", [])]
    ck("tools: required ids present exactly once", set(ids) == REQUIRED_TOOLS and len(ids) == len(set(ids)), sorted(set(ids) ^ REQUIRED_TOOLS))
    for x in t.get("tools", []):
        i = x.get("id")
        ck(f"tools[{i}]: exact key set", set(x) == TOOL_KEYS, sorted(set(x) ^ TOOL_KEYS))
        ck(f"tools[{i}]: vocabularies", x.get("role") in ROLES and x.get("status") in TOOL_STATUS and x.get("hash_basis") in HASH_BASIS,
           (x.get("role"), x.get("status"), x.get("hash_basis")))
        sha = x.get("sha256")
        ck(f"tools[{i}]: sha256 format consistent with basis",
           (sha is None and x.get("hash_basis") == "none") or (isinstance(sha, str) and re.fullmatch(r"[0-9a-f]{64}", sha) and x.get("hash_basis") != "none"))
        if x.get("hash_basis") == "install-record trusted hashes":
            ck(f"tools[{i}]: hash equals install-record trusted table", trusted.get(x.get("path")) == sha, x.get("path"))
            p = Path(str(x.get("path")).replace("~", str(HOME), 1))
            cur = hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None
            ck(f"tools[{i}]: installed binary currently matches", cur == sha, f"current {cur}")
        refs = list(x.get("contract") or []) + list(x.get("evidence") or [])
        missing = [r for r in refs if not exists(r)]
        ck(f"tools[{i}]: referenced paths exist", not missing, missing)
        bad = [r for r in x.get("requirements") or [] if r not in fr_ids]
        ck(f"tools[{i}]: FR ids exist in plan.md", not bad, bad)
    ck_t = next((x for x in t.get("tools", []) if x.get("id") == "checkov"), {})
    inv = ck_t.get("invocation") or ""
    ck("tools[checkov]: invocation is revision 2 (private cwd, staged -f, kubernetes, no downloads)",
       all(s in inv for s in ("--config-file", "--framework kubernetes", "--skip-download", "--skip-results-upload", "-f")) and " -d " not in f" {inv} ")
    ck("tools[checkov]: R1 rejected recorded", any("R1" in r for r in ck_t.get("rejected_invocations") or []))
    kc = next((x for x in t.get("tools", []) if x.get("id") == "kubeconform"), {})
    ck("tools[kubeconform]: FR-30 linked", "FR-30" in (kc.get("requirements") or []))
    sheets = t.get("contract_sheets", [])
    ck("tools: contract sheets valid and existing",
       sheets and all(s.get("kind") in SHEET_KIND and s.get("status") in SHEET_STATUS and exists(s.get("path")) for s in sheets),
       [s.get("path") for s in sheets if not exists(s.get("path"))])
    ck("tools: run1 checkov sheet marked superseded-for-invocation",
       any(s.get("path") == "evidence/P00/P00-T09/contract-checkov.md" and s.get("status") == "superseded-for-invocation" for s in sheets))

    # ------------------------------------------------------------------ crates.json
    c = json.loads((root / "spikes/p00-freeze/crates.json").read_text())
    ck("crates: freeze_version/base_commit", c.get("freeze_version") == "1" and c.get("base_commit") == "3decec1")
    lock_bytes = (root / "spikes/rust/Cargo.lock").read_bytes()
    ck("crates: lockfile sha256", (c.get("lockfile") or {}).get("sha256") == hashlib.sha256(lock_bytes).hexdigest())
    lock = tomllib.loads(lock_bytes.decode())
    lock_versions = {}
    for pkg in lock.get("package", []):
        lock_versions.setdefault(pkg["name"], set()).add(pkg["version"])
    ck("crates: toolchain 1.97.1", (c.get("toolchain") or {}).get("rustc") == "1.97.1" and (c.get("toolchain") or {}).get("cargo") == "1.97.1")
    by_name = {}
    for x in c.get("crates", []):
        by_name.setdefault(x.get("name"), []).append(x)
        n = x.get("name")
        ck(f"crates[{n}]: exact key set and status", set(x) == CRATE_KEYS and x.get("status") in CRATE_STATUS, (sorted(set(x) ^ CRATE_KEYS), x.get("status")))
        missing = [r for r in x.get("evidence") or [] if not exists(r)]
        ck(f"crates[{n}]: evidence paths exist", not missing, missing)
    for toml in sorted((root / "spikes/rust").glob("*/Cargo.toml")):
        pkg = toml.parent.name
        deps = tomllib.loads(toml.read_text()).get("dependencies", {})
        for name, spec in deps.items():
            ver = (spec if isinstance(spec, str) else spec.get("version", "")).lstrip("=")
            dflt = True if isinstance(spec, str) else spec.get("default-features", True)
            feats = [] if isinstance(spec, str) else spec.get("features", [])
            entries = [e for e in by_name.get(name, []) if e.get("spike_package") == pkg]
            ok = len(entries) == 1 and entries[0].get("version") == ver and entries[0].get("default_features") == dflt \
                and sorted(entries[0].get("features") or []) == sorted(feats)
            ck(f"crates: {pkg} dependency {name} ={ver} frozen as pinned", ok, entries)
            ck(f"crates: {name} {ver} present in Cargo.lock", ver in lock_versions.get(name, set()))
            ck(f"crates: {name} status is not 'fallback'/'excluded' for a built spike dependency",
               bool(entries) and entries[0].get("status") in {"accepted-qualified", "provisional", "deferred"})
    all_adv = {a for x in c.get("crates", []) for a in x.get("advisory_exceptions") or []}
    ck("crates: K-A advisory ids recorded", KA_IDS <= all_adv, sorted(KA_IDS - all_adv))
    ck("crates: build rule per package (-p) recorded", any("-p" in r and "--workspace" in r for r in c.get("build_rules") or []))

    # ------------------------------------------------------------------ report
    rep = (root / "evidence/P00/P00-report.md").read_text()
    miss_t = [f"P00-T{n:02d}" for n in range(1, 12) if f"P00-T{n:02d}" not in rep and f"T{n:02d}" not in rep]
    ck("report: covers T01..T11", not miss_t, miss_t)
    miss_fr = [f"FR-{n:02d}" for n in range(1, 31) if f"FR-{n:02d}" not in rep]
    ck("report: covers FR-01..FR-30", not miss_fr, miss_fr)
    # Revised at P00 closure (2026-10-24): the T11-time expectation "P00 acceptance pending" became untrue by the owner's
    # approved closure. The report must state EITHER pending OR the closure state, and the closure state requires all of:
    # "accepted ... with qualifications", a "P00 closure" section, and "ready for planning, not executing" for P01.
    pending = re.search(r"P00 acceptance[^.\n]*pending|pending[^.\n]*P00 acceptance", rep, re.I) is not None
    closed = (re.search(r"P00 accepted[^.\n]*with qualifications", rep, re.I) is not None and "## 11. P00 closure" in rep
              and "ready for planning, not executing" in rep)
    ck("report: P00 acceptance state stated (pending, or closed with qualifications + closure section + P01 planning-only)", pending or closed,
       f"pending={pending} closed={closed}")
    paths = sorted({p for p in re.findall(r"`((?:spikes|evidence|schemas)/[^`\s]+)`", rep)})
    bad_paths = [p for p in paths if not (root / p.rstrip("/")).exists() and "*" not in p and "{" not in p and "<" not in p]
    ck("report: cited repository paths exist", not bad_paths, bad_paths)
    bad_fr = sorted(set(re.findall(r"FR-\d+", rep)) - fr_ids)
    ck("report: FR ids exist in plan.md", not bad_fr, bad_fr)

    print(f"{sum(res)}/{len(res)} passed")
    return 0 if all(res) else 1


if __name__ == "__main__":
    sys.exit(main())

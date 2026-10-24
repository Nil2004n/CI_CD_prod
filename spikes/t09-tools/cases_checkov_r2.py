"""P00-T09-D01: checkov replacement application contract R2 (Opus-owned; spike code, not an adapter).

Contract R1 (run1: app flags or app --config-file with cwd = repository) is REJECTED: repository
`.checkov.yaml` discovered at cwd was merged into the configuration and its `external-checks-dir`
imported repository Python (run1 CK-EXE-CFG-F-RUN, CK-EXE-APPCFG-RUN).

Contract R2 evaluated here:
- cwd: a private, empty, app-owned directory (never the repository, never the staging directory);
- HOME/XDG/TMPDIR: private per case (harness), containing no checkov configuration;
- environment: harness allowlist + CKV_SKIP_PACKAGE_UPDATE_CHECK=True;
- input: an allowlisted STAGED COPY of the selected files only (regular *.yaml/*.yml, no dot-path
  components, no symlinks anywhere in the path, no path escape, size-bounded), with a mapping and
  sha256 back to the original;
- argv: --config-file <app-owned file outside the input> --framework kubernetes --skip-download
  --skip-results-upload -o json -f <absolute staged file> [-f ...]; never -d, never
  --external-checks-*;
- results are accepted only after structure validation against the staged input (an empty or
  zero-resource result is "not assessed", never "clean").

Runs labelled `control:` deliberately reproduce rejected or unsafe set-ups to prove that the hostile
fixtures are potent in this same run; they are not application invocations and are expected to
execute the marker module. Only `R2-` runs are application-contract runs.
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import stat
from pathlib import Path, PurePosixPath

import fixtures as fx
from cases_checkov import evil_module, failed_ids, files_in, parse_errors, passed_ids, reports, skipped_ids, summary
from fixtures import write_tree

T = 170
MARK = "PWT_EXECUTED_FX-INJ-EXEC-CHECKOV-R2"
ALLOWED_SUFFIX = {".yaml", ".yml"}
MAX_STAGE_BYTES = 1024 * 1024
APP_CFG = "framework:\n  - kubernetes\nskip-download: true\n"
R2_FLAGS = ["--framework", "kubernetes", "--skip-download", "--skip-results-upload", "-o", "json"]
R2_ENV = {"CKV_SKIP_PACKAGE_UPDATE_CHECK": "True"}

K8S_MALFORMED = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: broken\nspec:\n  template: [unclosed\n containers: x\n"
K8S_NO_RESOURCES = "---\nsettings:\n  a: 1\n"


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def tree_hash(root: Path) -> str:
    h = hashlib.sha256()
    for p in sorted(root.rglob("*")):
        h.update(str(p.relative_to(root)).encode() + b"\0")
        if p.is_symlink():
            h.update(b"L" + os.readlink(p).encode())
        elif p.is_file():
            h.update(b"F" + p.read_bytes())
    return h.hexdigest()


def expected_resources(data: bytes) -> int:
    return len(re.findall(rb"(?m)^kind:\s*\S", data))


def stage(repo: Path, selected: list[str], dest: Path) -> tuple[dict, dict]:
    """Copy only allowlisted selected data files into `dest`. Returns (mapping, rejected)."""
    mapping, rejected = {}, {}
    repo_real = repo.resolve()
    for rel in selected:
        p = PurePosixPath(rel)
        if not rel or p.is_absolute() or ".." in p.parts:
            rejected[rel] = "absolute path or path escape"
            continue
        if any(part.startswith(".") for part in p.parts):
            rejected[rel] = "dot-path component (scanner config / repository metadata)"
            continue
        if p.suffix not in ALLOWED_SUFFIX:
            rejected[rel] = "not an allowlisted data type (*.yaml, *.yml)"
            continue
        cur, link = repo, None
        for part in p.parts:
            cur = cur / part
            if cur.is_symlink():
                link = str(PurePosixPath(*cur.relative_to(repo).parts))
                break
        if link:
            rejected[rel] = f"symlink in path ({link})"
            continue
        src = repo / rel
        if not src.exists():
            rejected[rel] = "missing"
            continue
        try:
            fd = os.open(src, os.O_RDONLY | os.O_NOFOLLOW)
        except OSError as e:
            rejected[rel] = f"open refused (errno {e.errno})"
            continue
        with os.fdopen(fd, "rb") as f:
            st = os.fstat(f.fileno())
            if not stat.S_ISREG(st.st_mode):
                rejected[rel] = "not a regular file"
                continue
            if st.st_size > MAX_STAGE_BYTES:
                rejected[rel] = "too large"
                continue
            data = f.read()
        if repo_real not in src.resolve().parents:
            rejected[rel] = "resolves outside the repository"
            continue
        out = dest / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(data)
        h = hashlib.sha256(data).hexdigest()
        if sha(out) != h:
            rejected[rel] = "staged copy hash mismatch"
            out.unlink()
            continue
        mapping[rel] = {"staged": str(out), "sha256": h, "expected_resources": expected_resources(data)}
    return mapping, rejected


def assess(r: dict, mapping: dict) -> dict:
    """Structure validation of an R2 result against the staged input (spike version)."""
    out = {"json": r.get("json_parse"), "exit": r["exit"]}
    reps = reports(r)
    if r.get("json_parse") != "ok" or reps is None:
        return {**out, "verdict": "not assessed: output not parseable JSON"}
    k8s = [x for x in reps if x.get("check_type") == "kubernetes"]
    want = sum(m["expected_resources"] for m in mapping.values())
    if not k8s:
        return {**out, "verdict": "not assessed: no kubernetes report" + (" (input has no kubernetes resources)" if want == 0 else ""),
                "expected_resources": want}
    sm = k8s[0].get("summary") or {}
    rc, pe = sm.get("resource_count"), parse_errors(r)
    reported = files_in(r)
    unmatched = [rel for rel, m in mapping.items() if m["expected_resources"] and not any(f.endswith("/" + rel) or f == rel for f in reported)]
    out.update({"expected_resources": want, "resource_count": rc, "parsing_errors": len(pe), "files_without_results": unmatched})
    if pe:
        return {**out, "verdict": "not assessed: parsing errors"}
    if rc is None or rc < want:
        return {**out, "verdict": f"not assessed: resource_count {rc} < expected {want}"}
    if unmatched:
        return {**out, "verdict": "not assessed: staged file(s) absent from results"}
    return {**out, "verdict": "assessed"}


def cases(ctx):
    c = ctx.canaries

    def new_case(cid):
        d = ctx.case_dir(cid)
        for sub in ("run", "stage", "app"):
            (d / sub).mkdir()
        (d / "app" / "checkov-app.yaml").write_text(APP_CFG)
        return d

    def hostile(d: Path, skip_ids: list[str]) -> dict:
        """Root and near-input hostile content (absolute external-checks-dir so the config fires whenever it is read)."""
        rp = d / "repo"
        skip = "".join(f"  - {i}\n" for i in skip_ids) or "  - CKV_K8S_1\n"
        cfg = lambda mod_dir: f"external-checks-dir:\n  - {mod_dir}\nskip-check:\n{skip}soft-fail: true\n"
        files = {
            "k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/valid.yaml": fx.K8S_VALID,
            ".checkov.yaml": cfg(rp / "custom_checks"), ".checkov.yml": cfg(rp / "custom_checks"),
            "custom_checks/__init__.py": "", "custom_checks/pwt_spike_check.py": evil_module(d / f"{MARK}-ROOT"),
            "k8s/.checkov.yaml": cfg(rp / "k8s" / "custom_checks"),
            "k8s/custom_checks/__init__.py": "", "k8s/custom_checks/pwt_spike_check.py": evil_module(d / f"{MARK}-NEAR"),
            "k8s/evil.py": f"import pathlib\npathlib.Path({str(d / (MARK + '-EVILPY'))!r}).touch()\n",
            "k8s/helper.sh": f"#!/bin/sh\ntouch {d / (MARK + '-HELPER')}\n",
            "k8s/kustomization.yaml": "resources:\n  - insecure.yaml\n",
        }
        write_tree(rp, files)
        os.chmod(rp / "k8s" / "helper.sh", 0o755)
        return files

    def markers_in(d: Path) -> list[str]:
        return sorted(p.name for p in d.glob(MARK + "*"))

    def R2(cid, d, mapping, purpose, fixtures_, extra=None):
        argv = [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), *R2_FLAGS]
        for m in mapping.values():
            argv += ["-f", m["staged"]]
        return ctx.run(cid, argv + (extra or []), d / "run", purpose="R2 application contract: " + purpose, fixtures=fixtures_,
                       extra_env=R2_ENV, parse_json="--show-config" not in (extra or []), timeout=T)

    obs = {}
    ctx.note("contract_R2_argv_template", ["<checkov>", "--config-file", "<app-owned file outside input>", *R2_FLAGS, "-f", "<absolute staged file>", "..."])
    ctx.note("contract_R2_env_additions", R2_ENV)
    ctx.note("contract_R2_app_config_contents", APP_CFG)
    ctx.note("contract_R1_rejected", ["app flags (-f, --framework) with cwd = repository (run1 CK-EXE-CFG-F-RUN)",
                                      "app --config-file + -f with cwd = repository (run1 CK-EXE-APPCFG-RUN)",
                                      "any -d on a repository or on an unfiltered copy"])

    # ------------------------------------------------------------ reference (clean repo)
    d = new_case("R2-REF")
    write_tree(d / "repo", {"k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/valid.yaml": fx.K8S_VALID})
    m_ref, rej = stage(d / "repo", ["k8s/insecure.yaml"], d / "stage")
    ref = R2("R2-REF", d, m_ref, "clean repository, staged insecure.yaml", ["FX-K8S-INSECURE"])
    ref_failed = sorted(failed_ids(ref))
    obs["ref"] = {"assess": assess(ref, m_ref), "failed": len(ref_failed), "passed": len(passed_ids(ref)), "staging_rejected": rej,
                  "reported_files": files_in(ref), "markers": markers_in(d)}
    m_ref2, _ = stage(d / "repo", ["k8s/insecure.yaml", "k8s/valid.yaml"], d / "stage")
    ref2 = R2("R2-REF-MULTI", d, m_ref2, "clean repository, staged insecure.yaml + valid.yaml", ["FX-K8S-INSECURE", "FX-K8S-VALID"])
    rel_of = lambda f: f.split("/stage/", 1)[1] if "/stage/" in f else f.lstrip("/")
    per_file = lambda r: {rel_of(f): sorted(failed_ids_by_file(r, f)) for f in files_in(r)}

    def failed_ids_by_file(r, f):
        from cases_checkov import entries
        return {e["id"] for e in entries(r) if e["status"] == "failed" and e["file"] == f}

    obs["ref_multi"] = {"assess": assess(ref2, m_ref2), "failed_per_file": {k: len(v) for k, v in per_file(ref2).items()}}

    # ------------------------------------------------------------ controls (rejected / unsafe set-ups; markers EXPECTED)
    ctrl = {}
    d = new_case("CTRL-R1-FLAGS-CWD-REPO")
    hostile(d, ref_failed)
    r = ctx.run("CTRL-R1-FLAGS-CWD-REPO", [ctx.bin, "-f", "k8s/insecure.yaml", "--framework", "kubernetes", "-o", "json"], d / "repo",
                purpose="control: REJECTED contract R1 (app flags, cwd = repository), hostile root+near content",
                fixtures=["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"], parse_json=True, timeout=T)
    ctrl["R1 flags, cwd=repo"] = {"markers": markers_in(d), "failed": len(failed_ids(r)), "exit": r["exit"]}
    d = new_case("CTRL-R1-APPCFG-CWD-REPO")
    hostile(d, ref_failed)
    r = ctx.run("CTRL-R1-APPCFG-CWD-REPO", [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), "-f", "k8s/insecure.yaml", "-o", "json"],
                d / "repo", purpose="control: REJECTED contract R1 (app --config-file + -f, cwd = repository)",
                fixtures=["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"], parse_json=True, timeout=T)
    ctrl["R1 app --config-file, cwd=repo"] = {"markers": markers_in(d), "failed": len(failed_ids(r)), "exit": r["exit"]}
    d = new_case("CTRL-HOME-CONFIG")
    hostile(d, ref_failed)
    (d / "home" / ".checkov.yaml").write_text(f"external-checks-dir:\n  - {d / 'repo' / 'custom_checks'}\n")
    m, _ = stage(d / "repo", ["k8s/insecure.yaml"], d / "stage")
    r = ctx.run("CTRL-HOME-CONFIG", [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), *R2_FLAGS, "-f", m["k8s/insecure.yaml"]["staged"]],
                d / "run", purpose="control: R2 argv but a hostile ~/.checkov.yaml in HOME (shows HOME must be app-owned)",
                fixtures=["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"], extra_env=R2_ENV, parse_json=True, timeout=T)
    ctrl["R2 argv, hostile HOME config"] = {"markers": markers_in(d), "failed": len(failed_ids(r)), "exit": r["exit"]}
    d = new_case("CTRL-UNFILTERED-COPY-D")
    hostile(d, ref_failed)
    shutil.copytree(d / "repo", d / "stage" / "copy", symlinks=True)
    r = ctx.run("CTRL-UNFILTERED-COPY-D", [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), *R2_FLAGS, "-d", str(d / "stage" / "copy")],
                d / "run", purpose="control: unfiltered whole-tree copy scanned with -d (shows staging must filter and -d is not allowed)",
                fixtures=["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"], extra_env=R2_ENV, parse_json=True, timeout=T)
    ctrl["unfiltered copy, -d"] = {"markers": markers_in(d), "failed": len(failed_ids(r)), "exit": r["exit"]}
    ctx.note("controls_rejected_or_unsafe_setups", ctrl)
    potent = all(v["markers"] for v in ctrl.values())

    # ------------------------------------------------------------ R2 against hostile variants
    r2 = {}
    d = new_case("R2-HOSTILE")
    hostile(d, ref_failed)
    before = tree_hash(d / "repo")
    m, rej = stage(d / "repo", ["k8s/insecure.yaml"], d / "stage")
    r = R2("R2-HOSTILE-SINGLE", d, m, "hostile root + near-input config/modules/scripts left in the original repository", ["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV", "FX-K8S-INSECURE"])
    r2["single"] = {"assess": assess(r, m), "failed_equals_ref": sorted(failed_ids(r)) == ref_failed, "skipped": sorted(skipped_ids(r)),
                    "exit": r["exit"], "markers": markers_in(d), "staging_rejected": rej}
    m2, _ = stage(d / "repo", ["k8s/insecure.yaml", "k8s/valid.yaml"], d / "stage")
    r = R2("R2-HOSTILE-MULTI", d, m2, "same hostile repository, two staged files", ["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV", "FX-K8S-INSECURE", "FX-K8S-VALID"])
    r2["multi"] = {"assess": assess(r, m2), "failed_per_file_equals_ref": per_file(r) == per_file(ref2), "markers": markers_in(d)}
    sc = R2("R2-HOSTILE-SHOW-CONFIG", d, m, "same, --show-config: which configuration sources checkov reports", ["FX-INJ-CHECKOV-CONFIG"], extra=["--show-config"])
    src_lines = sorted({l.strip()[:200] for l in sc["_stdout"].decode("utf-8", "replace").splitlines() if re.search(r"config file|\.ya?ml", l, re.I)})
    r2["show_config"] = {"exit": sc["exit"], "source_lines": [ctx.sanitise(l.replace(str(ctx.root), "<root>")) for l in src_lines][:12],
                         "mentions_repo_config": ".checkov.y" in sc["_stdout"].decode("utf-8", "replace"), "markers": markers_in(d)}
    r2["original_repo_unchanged"] = tree_hash(d / "repo") == before
    r2["stage_contents"] = sorted(str(p.relative_to(d / "stage")) for p in (d / "stage").rglob("*") if p.is_file())
    r2["run_dir_contents_after"] = sorted(str(p.relative_to(d / "run")) for p in (d / "run").rglob("*"))

    # ------------------------------------------------------------ staging rules against hostile selections
    d = new_case("R2-STAGING")
    hostile(d, ref_failed)
    (d / "outside.yaml").write_text(fx.K8S_VALID)
    os.symlink(d / "outside.yaml", d / "repo" / "k8s" / "link-out.yaml")
    os.symlink("insecure.yaml", d / "repo" / "k8s" / "link-in.yaml")
    (d / "realdir").mkdir()
    (d / "realdir" / "x.yaml").write_text(fx.K8S_VALID)
    os.symlink(d / "realdir", d / "repo" / "k8s" / "linkdir")
    sel = ["k8s/insecure.yaml", ".checkov.yaml", ".checkov.yml", "k8s/.checkov.yaml", "custom_checks/pwt_spike_check.py",
           "k8s/custom_checks/pwt_spike_check.py", "k8s/evil.py", "k8s/helper.sh", "k8s/link-out.yaml", "k8s/link-in.yaml",
           "k8s/linkdir/x.yaml", "../outside.yaml", "/etc/hostname", "k8s/missing.yaml", "k8s"]
    m, rej = stage(d / "repo", sel, d / "stage")
    r = R2("R2-STAGING-RUN", d, m, "hostile selection list; only accepted staged files scanned", ["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV", "FX-LINK-OUTSIDE"])
    r2["staging"] = {"accepted": {k: {"sha256_prefix": v["sha256"][:12], "expected_resources": v["expected_resources"]} for k, v in m.items()},
                     "rejected": rej, "assess": assess(r, m), "failed_equals_ref": sorted(failed_ids(r)) == ref_failed, "markers": markers_in(d)}

    # ------------------------------------------------------------ coverage: missing / malformed / no resources / suppression
    cov = {}
    d = new_case("R2-MALFORMED")
    write_tree(d / "repo", {"k8s/broken.yaml": K8S_MALFORMED})
    m, _ = stage(d / "repo", ["k8s/broken.yaml"], d / "stage")
    r = R2("R2-MALFORMED", d, m, "staged malformed kubernetes YAML", ["FX-YAML-MALFORMED"])
    cov["malformed"] = {"assess": assess(r, m), "exit": r["exit"], "summary": summary(r)}
    d = new_case("R2-MISSING")
    write_tree(d / "repo", {"k8s/insecure.yaml": fx.K8S_INSECURE})
    m, rej = stage(d / "repo", ["k8s/missing.yaml"], d / "stage")
    cov["missing_staging"] = {"accepted": list(m), "rejected": rej, "invocation": "none (nothing staged)"}
    ghost = {"k8s/missing.yaml": {"staged": str(d / "stage" / "k8s" / "missing.yaml"), "sha256": "", "expected_resources": 1}}
    r = R2("R2-MISSING-DIRECT", d, ghost, "contract argv pointed at a nonexistent staged path (simulated staging bypass)", ["FX-SCOPE-MISSING"])
    cov["missing_direct"] = {"assess": assess(r, ghost), "exit": r["exit"]}
    d = new_case("R2-NO-RESOURCES")
    write_tree(d / "repo", {"cfg/settings.yaml": K8S_NO_RESOURCES})
    m, _ = stage(d / "repo", ["cfg/settings.yaml"], d / "stage")
    r = R2("R2-NO-RESOURCES", d, m, "staged YAML without kubernetes resources", ["FX-YAML-VALID"])
    cov["no_resources"] = {"assess": assess(r, m), "exit": r["exit"]}
    d = new_case("R2-INLINE")
    skip_two = ref_failed[:2]
    ann = "".join(f"    checkov.io/skip{i + 1}: {cid}=pwt synthetic suppression\n" for i, cid in enumerate(skip_two))
    write_tree(d / "repo", {"k8s/insecure.yaml": fx.K8S_INSECURE.replace("metadata:\n", "metadata:\n  annotations:\n" + ann, 1)})
    m, _ = stage(d / "repo", ["k8s/insecure.yaml"], d / "stage")
    r = R2("R2-INLINE", d, m, "in-file checkov.io/skip annotations inside the staged data", ["FX-INJ-CHECKOV-INLINE"])
    cov["inline_suppression"] = {"annotated": skip_two, "skipped": sorted(skipped_ids(r)), "failed_missing_vs_ref": sorted(set(ref_failed) - failed_ids(r)),
                                 "assess": assess(r, m)}
    d = new_case("R2-CONTEXT")
    write_tree(d / "repo", {"k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/valid.yaml": fx.K8S_VALID})
    full = ctx.run("R2-CONTEXT-FULL-CLEAN-D", [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), *R2_FLAGS, "-d", str(d / "repo")],
                   d / "run", purpose="comparison only: -d over a CLEAN full repository (not an allowed invocation)", fixtures=["FX-K8S-INSECURE", "FX-K8S-VALID"],
                   extra_env=R2_ENV, parse_json=True, timeout=T)
    full_rel = ctx.run("R2-CONTEXT-FULL-CLEAN-D-REL", [ctx.bin, "--config-file", str(d / "app" / "checkov-app.yaml"), *R2_FLAGS, "-d", "repo"],
                       d, purpose="comparison only: -d repo (relative, cwd = clean case directory; not an allowed invocation)",
                       fixtures=["FX-K8S-INSECURE", "FX-K8S-VALID"], extra_env=R2_ENV, parse_json=True, timeout=T)
    ins = [f for f in files_in(full_rel) if f.endswith("k8s/insecure.yaml")]
    cov["single_file_vs_full_clean_dir"] = {
        "absolute_d_path_with_dot_component": {"files": files_in(full), "summary": summary(full), "exit": full["exit"],
                                               "note": "an absolute -d path through a dot-directory (~/.local/...) yielded zero resources and exit 0"},
        "relative_d": {"files": files_in(full_rel), "exit": full_rel["exit"]},
        "insecure_failed_equal": bool(ins) and sorted(failed_ids_by_file(full_rel, ins[0])) == ref_failed}
    ctx.note("r2_reference", obs)
    ctx.note("r2_hostile", r2)
    ctx.note("r2_coverage", cov)

    # ------------------------------------------------------------ outcomes
    r2_runs = [x for x in ctx.runs if x["case"].startswith("R2-") and "CONTEXT-FULL" not in x["case"]]
    r2_markers = {x["case"]: x["new_execution_markers"] for x in r2_runs if x["new_execution_markers"]}
    ref_ok = obs["ref"]["assess"]["verdict"] == "assessed" and len(ref_failed) > 0
    if not potent:
        ctx.hyp("R2-NO-EXEC", "not testable", f"Controls did not all execute the marker module ({ctrl}); fixture potency unproven.", [x["case"] for x in ctx.runs if x["case"].startswith("CTRL")])
    else:
        ctx.hyp("R2-NO-EXEC", "confirmed" if not r2_markers else "refuted",
                f"Under R2 no execution marker appeared in {len(r2_runs)} contract runs (markers: {r2_markers}) while every control in the same run "
                f"executed the hostile module ({ {k: v['markers'] for k, v in ctrl.items()} }). Observed for the tested variants only: hostile "
                "root/near .checkov.yaml/.yml with absolute external-checks-dir, Python modules, evil.py, executable helper, kustomization.yaml.",
                [x["case"] for x in r2_runs] + [x["case"] for x in ctx.runs if x["case"].startswith("CTRL")])
    ctx.hyp("R2-NO-DISCOVERY", "confirmed" if not r2["show_config"]["mentions_repo_config"] else "refuted",
            f"--show-config under R2 named no repository .checkov.* source (lines: {r2['show_config']['source_lines']}).", ["R2-HOSTILE-SHOW-CONFIG"])
    ctx.hyp("R2-FINDINGS", "confirmed" if ref_ok and r2["single"]["failed_equals_ref"] and r2["multi"]["failed_per_file_equals_ref"]
            and r2["staging"]["failed_equals_ref"] else "refuted",
            f"Reference R2 run on a clean repository: {len(ref_failed)} failed built-in checks ({obs['ref']['assess']}). With the hostile repository "
            f"(which also carried skip-check of every failing id and soft-fail): single file equal={r2['single']['failed_equals_ref']}, "
            f"multi-file per-file equal={r2['multi']['failed_per_file_equals_ref']}, hostile selection equal={r2['staging']['failed_equals_ref']}; "
            f"R2 exit {r2['single']['exit']} (soft-fail from the repo config not applied).", ["R2-REF", "R2-REF-MULTI", "R2-HOSTILE-SINGLE", "R2-HOSTILE-MULTI", "R2-STAGING-RUN"])
    want_rej = {".checkov.yaml", ".checkov.yml", "k8s/.checkov.yaml", "custom_checks/pwt_spike_check.py", "k8s/custom_checks/pwt_spike_check.py",
                "k8s/evil.py", "k8s/helper.sh", "k8s/link-out.yaml", "k8s/link-in.yaml", "k8s/linkdir/x.yaml", "../outside.yaml", "/etc/hostname",
                "k8s/missing.yaml", "k8s"}
    ctx.hyp("R2-STAGING", "confirmed" if set(r2["staging"]["rejected"]) == want_rej and set(r2["staging"]["accepted"]) == {"k8s/insecure.yaml"} else "refuted",
            f"Accepted {sorted(r2['staging']['accepted'])}; rejected {r2['staging']['rejected']}. Stage directory of R2-HOSTILE held only "
            f"{r2['stage_contents']}; original repository unchanged: {r2['original_repo_unchanged']}.", ["R2-STAGING-RUN", "R2-HOSTILE-SINGLE"])
    cov_ok = (cov["malformed"]["assess"]["verdict"].startswith("not assessed") and cov["missing_direct"]["assess"]["verdict"].startswith("not assessed")
              and cov["no_resources"]["assess"]["verdict"].startswith("not assessed") and not cov["missing_staging"]["accepted"])
    ctx.hyp("R2-COVERAGE-VALIDATION", "confirmed" if cov_ok else "refuted",
            f"Malformed input: exit {cov['malformed']['exit']}, {cov['malformed']['assess']}; missing file rejected at staging ({cov['missing_staging']['rejected']}); "
            f"contract argv on a nonexistent path: exit {cov['missing_direct']['exit']}, {cov['missing_direct']['assess']}; YAML without resources: "
            f"{cov['no_resources']['assess']}. Exit codes alone would have reported several of these as clean; the structure check does not.",
            ["R2-MALFORMED", "R2-MISSING-DIRECT", "R2-NO-RESOURCES"])
    ctx.hyp("R2-INLINE-SUPPRESSION", "partially confirmed" if cov["inline_suppression"]["skipped"] else "refuted",
            f"Staging copies data verbatim, so in-file checkov.io/skip annotations still suppress: annotated {skip_two}, skipped {cov['inline_suppression']['skipped']}, "
            f"missing from failed vs reference {cov['inline_suppression']['failed_missing_vs_ref']}. Condition: suppressions are reported via skipped_checks "
            "and must be surfaced as repository-controlled suppression, not as a pass. No flag to ignore them was found.", ["R2-INLINE"])
    ctx.hyp("R2-CONTEXT", "partially confirmed",
            f"For this fixture the single staged file gave the same failed ids as a -d scan of the clean full repository: "
            f"{cov['single_file_vs_full_clean_dir']['insecure_failed_equal']}. Observation: the same -d with an absolute path through a "
            f"dot-directory returned zero resources and exit 0 ({cov['single_file_vs_full_clean_dir']['absolute_d_path_with_dot_component']['summary']}). "
            "Not a general equivalence claim: staging only selected files drops "
            "cross-file context (kustomize/helm structures, other files' resources) and other frameworks; R2 covers the kubernetes framework only.",
            ["R2-REF", "R2-CONTEXT-FULL-CLEAN-D-REL", "R2-CONTEXT-FULL-CLEAN-D"])
    sig = {x["case"]: x["network_attempt_signals"] for x in r2_runs if x["network_attempt_signals"]}
    ctx.hyp("R2-NET", "confirmed" if not sig else "refuted",
            f"No network-attempt signal in R2 runs ({sig}) with --skip-download, --skip-results-upload and CKV_SKIP_PACKAGE_UPDATE_CHECK=True "
            "(heuristic evidence, private netns).", [x["case"] for x in r2_runs])

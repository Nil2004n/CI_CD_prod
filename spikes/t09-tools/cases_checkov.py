"""P00-T09 checkov spike cases (worker W2; spike code, not application code).

Highest-priority hypothesis is H-EXE: a custom check module inside the repository tree that writes an
execution marker at import time. Marker names are made unique per configuration (suffix) so that
the harness' by-name "new marker" detection stays meaningful; the primary configuration (repo
`.checkov.yaml` with an external checks dir) uses the exact name PWT_EXECUTED_FX-INJ-EXEC-CHECKOV.

Outcomes are derived from comparing observations (hostile vs clean twin), never from documentation.
Flags were taken from the pinned binary's own --help output (case CK-HELP) and are used only if
that output lists them.
"""

from __future__ import annotations

import base64
import json
import re
from pathlib import Path

import fixtures as fx
from fixtures import write_tree

T = 170                                   # per-run timeout (s)
APP = ["-o", "json"]                      # machine output; frameworks are chosen per case
EXEC_MARK = "PWT_EXECUTED_FX-INJ-EXEC-CHECKOV"


# --------------------------------------------------------------------------- result helpers
def reports(r):
    j = r.get("_json")
    if isinstance(j, dict):
        return [j]
    if isinstance(j, list):
        return [x for x in j if isinstance(x, dict)]
    return None


def entries(r):
    out = []
    for rep in reports(r) or []:
        res = rep.get("results") or {}
        for st, key in (("passed", "passed_checks"), ("failed", "failed_checks"), ("skipped", "skipped_checks")):
            for c in res.get(key) or []:
                out.append({"fw": rep.get("check_type"), "status": st, "id": c.get("check_id"),
                            "file": c.get("file_path"), "resource": c.get("resource"),
                            "lines": c.get("file_line_range"), "keys": sorted(c.keys()),
                            "suppress_comment": bool((c.get("check_result") or {}).get("suppress_comment"))})
    return out


def failed_ids(r, fw=None):
    return {e["id"] for e in entries(r) if e["status"] == "failed" and (fw is None or e["fw"] == fw)}


def passed_ids(r):
    return {e["id"] for e in entries(r) if e["status"] == "passed"}


def skipped_ids(r):
    return {e["id"] for e in entries(r) if e["status"] == "skipped"}


def files_in(r):
    return sorted({e["file"] for e in entries(r) if e["file"]})


def summary(r):
    s = {}
    for rep in reports(r) or []:
        sm = rep.get("summary") or {}
        s[rep.get("check_type") or "<top-level summary only>"] = {k: sm.get(k) for k in ("passed", "failed", "skipped", "parsing_errors", "resource_count")}
    return s


def parse_errors(r):
    out = []
    for rep in reports(r) or []:
        pe = (rep.get("results") or {}).get("parsing_errors") or []
        out += [str(x) for x in pe]
    return out


def fws(r):
    return sorted({rep.get("check_type") for rep in reports(r) or []})


def has_marker(case_dir: Path, name: str) -> bool:
    return (case_dir / name).exists()


def evil_module(marker: Path) -> str:
    return (
        "import pathlib\n"
        f"pathlib.Path({str(marker)!r}).write_text('executed\\n')\n"
        "from checkov.common.models.enums import CheckCategories, CheckResult\n"
        "from checkov.kubernetes.checks.resource.base_spec_check import BaseK8sSpecCheck\n\n\n"
        "class PwtSpikeCheck(BaseK8sSpecCheck):\n"
        "    def __init__(self):\n"
        "        super().__init__(name='pwt spike custom check', id='CKV_PWT_9', categories=[CheckCategories.KUBERNETES],\n"
        "                         supported_entities=['Deployment'])\n\n"
        "    def scan_spec_conf(self, conf):\n"
        "        return CheckResult.PASSED\n\n\n"
        "check = PwtSpikeCheck()\n"
    )


def insecure_env(name: str, secret: str) -> str:
    return fx.K8S_INSECURE.replace("name: insecure", f"name: {name}").replace(
        "          image: nginx:latest\n",
        f"          image: nginx:latest\n          env:\n            - name: TOKEN\n              value: \"{secret}\"\n")


def with_k8s_annotation_skips(text: str, ids) -> str:
    ann = "".join(f"    checkov.io/skip{n}: {i}=pwt-spike-inline\n" for n, i in enumerate(sorted(ids), 1))
    return text.replace("  name: insecure\n", "  name: insecure\n  annotations:\n" + ann, 1)


def with_skips_everywhere(text: str, ids) -> str:
    out = []
    for line in text.splitlines(keepends=True):
        out.append(line)
        out += [f"        # checkov:skip={i}:pwt-spike-inline\n" for i in sorted(ids)]
    return "".join(out)


def with_skips(text: str, after: str, ids, indent: int) -> str:
    """Insert `# checkov:skip=ID:reason` comment lines (one per id) right after the first line equal to `after`."""
    pad = " " * indent
    block = "".join(f"{pad}# checkov:skip={i}:pwt-spike-inline\n" for i in sorted(ids))
    out, done = [], False
    for line in text.splitlines(keepends=True):
        out.append(line)
        if not done and line.rstrip("\n") == after:
            out.append(block)
            done = True
    return "".join(out)


# --------------------------------------------------------------------------- cases
def cases(ctx):
    c = ctx.canaries
    all_runs = []
    exit_map = {}
    exec_obs = {}                 # configuration label -> {"marker": bool, "custom_check_ran": bool, "exit": int}

    def R(cid, argv, cwd, purpose, fixtures, *, extra_env=None, scan_files=None, parse=True, timeout=T):
        r = ctx.run(cid, [ctx.bin] + argv, cwd, purpose=purpose, fixtures=fixtures, extra_env=extra_env,
                    scan_files=scan_files, parse_json=parse, timeout=timeout)
        all_runs.append(r)
        return r

    # ------------------------------------------------------------------ version, help, usage error
    d0 = ctx.case_dir("CK-META")
    r_ver = R("CK-VERSION", ["--version"], d0 / "repo", "version", [], parse=False, timeout=60)
    ctx.note("version", r_ver["_stdout"].decode("utf-8", "replace").strip()[:40])
    ctx.note("version_run_files_created", r_ver["files_created"])
    r_help = R("CK-HELP", ["--help"], d0 / "repo", "help: discover real flags", [], parse=False, timeout=60)
    help_text = r_help["_stdout"].decode("utf-8", "replace")
    flags = sorted(set(re.findall(r"(?<![\w-])(--[a-z][\w-]*)", help_text)))
    flags = [f for f in flags if not (f.endswith("-") or any(g != f and g.startswith(f) and f.endswith("-") for g in flags))]
    ctx.note("flags_from_help", flags)
    m = re.search(r"Possible values: all, ([a-z0-9_]+(?:, [a-z0-9_]+)+)", re.sub(r"\s+", " ", help_text))
    frameworks = [x.strip() for x in m.group(1).split(",")] if m else []
    ctx.note("frameworks_from_help", frameworks)
    ctx.note("framework_support", {k: k in frameworks for k in ("kubernetes", "github_actions", "secrets", "yaml", "json", "sca_package")})
    outs = re.search(r"-o \{([^}]*)\}", help_text.replace("\n", "").replace("  ", ""))
    ctx.note("output_formats", outs.group(1).split(",") if outs else [])
    want = ("--config-file", "--external-checks-dir", "--external-checks-git", "--external-checks-public-key", "--framework",
            "--skip-framework", "--skip-download", "--skip-results-upload", "--soft-fail", "--skip-check", "--check", "--file", "--directory",
            "--compact", "--quiet", "--mask", "--no-fail-on-crash", "--show-config", "--create-config", "--baseline", "--bc-api-key",
            "--enable-secret-scan-all-files", "--skip-path")
    ctx.note("flag_support", {k: (k in flags) for k in want})
    r_bad = R("CK-BADFLAG", ["--bogus-flag"], d0 / "repo", "usage error", [], parse=False, timeout=60)
    exit_map["bad flag"] = r_bad["exit"]
    r_nofile = R("CK-MISSING-FILE", ["-f", "no-such-file.yaml", "--framework", "kubernetes"] + APP, d0 / "repo", "nonexistent file argument", [])
    exit_map["nonexistent file argument"] = r_nofile["exit"]

    # ------------------------------------------------------------------ baseline tree
    d, repo = ctx.case_dir("CK-BASE"), None
    repo = d / "repo"
    write_tree(repo, {
        "k8s/valid.yaml": fx.K8S_VALID, "k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/invalid.yaml": fx.K8S_INVALID,
        "k8s/multidoc.yaml": fx.K8S_MULTIDOC, "k8s/crd.yaml": fx.K8S_CRD, "k8s/template.yaml": fx.K8S_TEMPLATE,
        "k8s/sibling.yaml": insecure_env("sibling", c["C11"]),
        ".github/workflows/valid.yaml": fx.GHA_VALID, ".github/workflows/invalid.yaml": fx.GHA_INVALID,
        ".github/workflows/ref.yaml": fx.GHA_REF, ".github/workflows/inj.yaml": fx.gha_injection(d),
        "notes.txt": f"token={c['C12']}\n",
    })
    K = ["--framework", "kubernetes"]
    G = ["--framework", "github_actions"]
    k_f = R("CK-K8S-F", ["-f", "k8s/insecure.yaml"] + K + APP, repo, "baseline FX-K8S-INSECURE, single file (-f)", ["FX-K8S-INSECURE", "FX-SCOPE-SIBLINGS"])
    k_d = R("CK-K8S-D", ["-d", "k8s"] + K + APP, repo, "baseline k8s directory scan (-d)", ["FX-K8S-VALID", "FX-K8S-INSECURE", "FX-K8S-INVALID", "FX-K8S-MULTIDOC", "FX-K8S-CRD-MISSING", "FX-K8S-TEMPLATE", "FX-SCOPE-SIBLINGS"])
    g_f = R("CK-GHA-F", ["-f", ".github/workflows/inj.yaml"] + G + APP, repo, "baseline FX-GHA-INJECTION, single file", ["FX-GHA-INJECTION"])
    g_d = R("CK-GHA-D", ["-d", "."] + G + APP, repo, "github_actions framework over the repo directory", ["FX-GHA-VALID", "FX-GHA-INVALID", "FX-GHA-INJECTION", "FX-GHA-REF"])
    exit_map["findings (k8s insecure, -f)"] = k_f["exit"]
    exit_map["findings (gha, -f)"] = g_f["exit"]
    nfw = R("CK-F-NO-FRAMEWORK", ["-f", "k8s/insecure.yaml"] + APP, repo, "-f with NO --framework: which frameworks run", ["FX-K8S-INSECURE"])
    ctx.note("python_version_hint_from_traceback_paths", sorted(set(re.findall(r"python(3\.\d+)", k_f["stderr_excerpt"])))[:2])
    ctx.note("baseline_json_shape", {
        "k8s_f_top_level": type(k_f.get("_json")).__name__, "no_framework_top_level": type(nfw.get("_json")).__name__,
        "top_keys": sorted((reports(k_f) or [{}])[0].keys()), "results_keys": sorted(((reports(k_f) or [{}])[0].get("results") or {}).keys()),
        "check_entry_keys": sorted(set(k for e in entries(k_f) for k in e["keys"])),
        "summary_keys": sorted(((reports(k_f) or [{}])[0].get("summary") or {}).keys()),
        "k8s_f_summary": summary(k_f), "k8s_d_summary": summary(k_d), "gha_f_summary": summary(g_f), "gha_d_summary": summary(g_d),
    })
    ctx.note("baseline_k8s_d_files", files_in(k_d))
    ctx.note("baseline_gha_d_files", files_in(g_d))
    ctx.note("baseline_failed_ids", {"k8s_f": sorted(failed_ids(k_f)), "gha_f": sorted(failed_ids(g_f))})
    ctx.note("hidden_dir_github_scanned_with_d", any(".github" in (f or "") for f in files_in(g_d)))
    ctx.note("no_framework_frameworks_run", fws(nfw))
    ctx.note("no_framework_canary_in_stdout", nfw["canary_in_stdout"])
    ctx.note("fx_gha_injection_findings", {"failed_ids": sorted(failed_ids(g_f)), "marker_present": "PWT_EXECUTED_FX-GHA-INJECTION" in ctx.markers()})
    ctx.note("fx_k8s_framework_checks_on_valid_vs_insecure", {
        "valid_failed": sorted({e["id"] for e in entries(k_d) if e["status"] == "failed" and (e["file"] or "").endswith("valid.yaml") and not (e["file"] or "").endswith("invalid.yaml")}),
        "insecure_failed": sorted(failed_ids(k_f))})
    ctx.note("k8s_d_per_file_status", {f: sorted({e["status"] for e in entries(k_d) if e["file"] == f}) for f in files_in(k_d)})

    # ------------------------------------------------------------------ framework selection hypothesis support
    # (recorded as note; kubernetes + github_actions must exist and produce results on the fixtures)
    ctx.note("framework_results", {"kubernetes_failed_on_insecure": bool(failed_ids(k_f)), "github_actions_failed_on_injection": bool(failed_ids(g_f)),
                                   "kubernetes_multidoc_template_crd_seen": [f for f in files_in(k_d) if any(x in f for x in ("multidoc", "template", "crd"))]})

    mfw = R("CK-MULTI-FW", ["-d", ".", "--framework", "kubernetes", "--framework", "github_actions"] + APP, repo,
            "two frameworks in one run (JSON top-level shape)", ["FX-K8S-INSECURE", "FX-GHA-INJECTION"])
    ctx.note("multi_framework_json", {"top_level": type(mfw.get("_json")).__name__, "check_types": fws(mfw), "exit": mfw["exit"],
                                      "sum_equals_single_runs": all(summary(mfw).get(k, {}).get("failed") is not None for k in ("kubernetes", "github_actions"))})

    # ------------------------------------------------------------------ H-SCOPE
    sib_in_f = any((f or "").endswith("sibling.yaml") for f in files_in(k_f))
    sib_in_d = any((f or "").endswith("sibling.yaml") for f in files_in(k_d))
    only_selected = files_in(k_f) and all((f or "").endswith("insecure.yaml") for f in files_in(k_f))
    ctx.note("scope_observations", {"f_files": files_in(k_f), "d_files_count": len(files_in(k_d)), "sibling_in_f": sib_in_f, "sibling_in_d": sib_in_d,
                                    "C11_in_f_stdout": "C11" in k_f["canary_in_stdout"], "C11_in_d_stdout": "C11" in k_d["canary_in_stdout"],
                                    "canaries_in_d_gha_nofw_stdout": sorted(set(k_d["canary_in_stdout"] + g_d["canary_in_stdout"] + nfw["canary_in_stdout"]))})
    if only_selected and not sib_in_f and "C11" not in k_f["canary_in_stdout"] + k_f["canary_in_stderr"] and sib_in_d:
        ctx.hyp("H-SCOPE", "confirmed",
                f"-f k8s/insecure.yaml reported only the selected file (files: {files_in(k_f)}) and sibling canary C11 is absent from its output; "
                f"-d k8s walks the directory and reports the sibling (sibling reported {sib_in_d}, C11 echoed in -d output: {'C11' in k_d['canary_in_stdout']}). "
                f"Non-YAML notes.txt (C12) echoed by any run: {'C12' in (k_d['canary_in_stdout'] + g_d['canary_in_stdout'] + nfw['canary_in_stdout'])}. "
                "Limitation: reads inferred from output. Config files are read from -d dir, cwd and HOME (see H-CFG) even with -f.",
                [k_f["case"], k_d["case"], nfw["case"]])
    else:
        ctx.hyp("H-SCOPE", "partially confirmed" if only_selected else "refuted",
                f"-f files={files_in(k_f)} sibling_in_f={sib_in_f} C11_in_f={'C11' in k_f['canary_in_stdout']} sibling_in_d={sib_in_d}",
                [k_f["case"], k_d["case"]])

    # ------------------------------------------------------------------ H-INL (inline skip annotations)
    twin_k = failed_ids(k_f)
    twin_g = failed_ids(g_f)
    write_tree(repo, {
        "k8s/inline-comment.yaml": with_skips(fx.K8S_INSECURE, "metadata:", twin_k, 2),
        "k8s/inline-annot.yaml": with_k8s_annotation_skips(fx.K8S_INSECURE, twin_k),
        ".github/workflows/inline-jobs.yaml": with_skips(fx.gha_injection(d), "jobs:", twin_g, 2),
        ".github/workflows/inline-all.yaml": with_skips_everywhere(fx.gha_injection(d), twin_g),
    })
    inl = {}
    for key, fw_args, path, twin in (
            ("k8s-comment-in-resource", K, "k8s/inline-comment.yaml", twin_k), ("k8s-annotation", K, "k8s/inline-annot.yaml", twin_k),
            ("gha-comment-under-jobs", G, ".github/workflows/inline-jobs.yaml", twin_g), ("gha-comment-after-every-line", G, ".github/workflows/inline-all.yaml", twin_g)):
        rr = R("CK-INL-" + key.upper(), ["-f", path] + fw_args + APP, repo, f"inline suppression form {key} for every id failing on the twin", ["FX-INJ-INLINE-CHECKOV"])
        inl[key] = {"twin_failed": len(twin), "failed": len(failed_ids(rr)), "suppressed": len(twin - failed_ids(rr)), "skipped_entries": len(skipped_ids(rr)),
                    "skipped_carry_suppress_comment": any(e["suppress_comment"] for e in entries(rr) if e["status"] == "skipped"), "exit": rr["exit"], "case": rr["case"]}
    ctx.note("inline_observations", {k: {kk: vv for kk, vv in v.items() if kk != "case"} for k, v in inl.items()})
    inl_hit = {k: v["suppressed"] for k, v in inl.items() if v["suppressed"]}
    inl_cases = [k_f["case"], g_f["case"]] + [v["case"] for v in inl.values()]
    if twin_k and inl_hit:
        ctx.hyp("H-INL", "confirmed",
                f"In-file suppressions are honoured: suppressed failing checks relative to the clean twin per form: {inl_hit} of "
                f"{ {k: v['twin_failed'] for k, v in inl.items()} } (skipped_checks entries and whether they carry the reason: "
                f"{ {k: (v['skipped_entries'], v['skipped_carry_suppress_comment']) for k, v in inl.items()} }). Forms with no effect: "
                f"{[k for k, v in inl.items() if not v['suppressed']]}. Kubernetes uses metadata.annotations 'checkov.io/skipN', comment form works for github_actions only where observed. "
                "No flag in --help disables inline suppressions (see flag_support); results are repository-suppressible unless the product counts skipped_checks "
                "(which are listed with their reason) and surfaces them as a limitation.",
                inl_cases)
    elif twin_k:
        ctx.hyp("H-INL", "refuted", f"None of the inline forms changed failed ids: {inl}", inl_cases)
    else:
        ctx.hyp("H-INL", "not testable", "Clean twin had no failed checks, so suppression could not be observed.", inl_cases)

    # ------------------------------------------------------------------ H-SEC
    write_tree(repo, {"k8s/secretenv.yaml": insecure_env("secretenv", c["C02"])})
    s_k8s = R("CK-SEC-K8S", ["-f", "k8s/secretenv.yaml"] + K + APP, repo, "secret in a failing resource's env value, kubernetes framework", ["FX-SEC-TOKEN-PREFIXED"])
    s_cmp = R("CK-SEC-K8S-COMPACT", ["-f", "k8s/secretenv.yaml", "--compact"] + K + APP, repo, "same with --compact", ["FX-SEC-TOKEN-PREFIXED"])
    s_quiet = R("CK-SEC-K8S-QUIET", ["-f", "k8s/secretenv.yaml", "--quiet"] + K + APP, repo, "same with --quiet", ["FX-SEC-TOKEN-PREFIXED"])
    write_tree(repo, fx.secrets_tree(c))
    s_nofw = R("CK-SEC-F-NOFW", ["-f", "k8s/secretenv.yaml"] + APP, repo, "secret file, -f WITHOUT --framework (default frameworks for the file type)", ["FX-SEC-TOKEN-PREFIXED"])
    s_dnofw = R("CK-SEC-D-NOFW", ["-d", "sec"] + APP, repo, "FX-SEC-* directory, NO --framework (default frameworks)", ["FX-SEC-TOKEN-PREFIXED", "FX-SEC-KEYPAIR", "FX-SEC-PRIVATE-KEY", "FX-SEC-URL-CRED", "FX-SEC-GENERIC"])
    fld = {}
    for rep_ in reports(s_k8s) or []:
        for st in ("passed_checks", "failed_checks"):
            for ch in (rep_.get("results") or {}).get(st) or []:
                for k_, v_ in ch.items():
                    if ctx.scan(json.dumps(v_).encode()):
                        fld.setdefault(k_, set()).update(ctx.scan(json.dumps(v_).encode()))
    ctx.note("secret_echo_by_json_field", {k_: sorted(v_) for k_, v_ in fld.items()})
    ctx.note("default_frameworks_without_flag", {"f_k8s_yaml": fws(s_nofw), "d_sec_dir": fws(s_dnofw), "d_sec_canary_in_stdout": sorted(set(s_dnofw["canary_in_stdout"])),
                                                   "f_k8s_canary_in_stdout": sorted(set(s_nofw["canary_in_stdout"]))})
    s_sf = R("CK-SEC-SECRETS-FW", ["-d", "sec", "--framework", "secrets"] + APP, repo, "secrets framework over FX-SEC-* (product will not use it)", [
        "FX-SEC-TOKEN-PREFIXED", "FX-SEC-KEYPAIR", "FX-SEC-PRIVATE-KEY", "FX-SEC-URL-CRED", "FX-SEC-GENERIC", "FX-SEC-MULTILINE", "FX-SEC-BASE64", "FX-SEC-NEAR-MISS"])
    sec_find = [e for e in entries(s_sf) if e["status"] == "failed"]
    ctx.note("secrets_framework", {"exit": s_sf["exit"], "failed_findings": len(sec_find), "check_ids": sorted({e["id"] for e in sec_find}),
                                   "files_with_findings": sorted({(e["file"] or "").rsplit("/", 1)[-1] for e in sec_find}),
                                   "canaries_in_stdout": sorted(set(s_sf["canary_in_stdout"])), "canaries_in_stderr": sorted(set(s_sf["canary_in_stderr"]))})
    ctx.note("secret_echo", {"k8s": sorted(set(s_k8s["canary_in_stdout"])), "k8s_compact": sorted(set(s_cmp["canary_in_stdout"])),
                             "k8s_quiet": sorted(set(s_quiet["canary_in_stdout"])), "secrets_fw": sorted(set(s_sf["canary_in_stdout"])),
                             "stderr_any": sorted(set(s_k8s["canary_in_stderr"] + s_sf["canary_in_stderr"] + s_cmp["canary_in_stderr"])),
                             "f_no_framework": sorted(set(s_nofw["canary_in_stdout"])), "d_no_framework": sorted(set(s_dnofw["canary_in_stdout"]))})
    leaks = bool(s_k8s["canary_in_stdout"] or s_sf["canary_in_stdout"] or s_nofw["canary_in_stdout"] or s_dnofw["canary_in_stdout"])
    ctx.hyp("H-SEC", "partially confirmed" if leaks else "refuted",
            f"Canaries in stdout: kubernetes framework {sorted(set(s_k8s['canary_in_stdout']))}, with --compact {sorted(set(s_cmp['canary_in_stdout']))}, "
            f"--quiet {sorted(set(s_quiet['canary_in_stdout']))}, secrets framework {sorted(set(s_sf['canary_in_stdout']))}, -f without --framework "
            f"{sorted(set(s_nofw['canary_in_stdout']))} (frameworks run: {fws(s_nofw)}), -d without --framework {sorted(set(s_dnofw['canary_in_stdout']))} "
            f"(frameworks run: {fws(s_dnofw)}); fields carrying the canary in the kubernetes JSON: {sorted(fld)}; stderr any: {sorted(set(s_k8s['canary_in_stderr'] + s_sf['canary_in_stderr']))}. "
            "No report file is written by default. The code_block field echoes raw source lines; no redaction flag was verified (see flag_support; "
            "--mask is for resource attributes and was not exercised). The product must use an explicit framework allowlist and redact/strip code_block "
            "before persistence. --compact removed the echo in this fixture: " + str(not s_cmp["canary_in_stdout"]) + " (verify per field before relying on it).",
            [s_k8s["case"], s_cmp["case"], s_quiet["case"], s_sf["case"], s_nofw["case"], s_dnofw["case"]])

    # ------------------------------------------------------------------ H-MAL / parse errors
    write_tree(repo, {"mal/k8s/malformed.yaml": fx.YAML_MALFORMED, "mal/k8s/malformed-secret.yaml": fx.yaml_malformed_secret(c),
                      "mal/.github/workflows/malformed.yaml": fx.YAML_MALFORMED})
    m_run = R("CK-MAL", ["-d", "mal", "--framework", "kubernetes", "--framework", "github_actions", "--framework", "secrets"] + APP, repo,
              "malformed YAML (+ malformed with secret) through k8s, gha and secrets frameworks", ["FX-YAML-MALFORMED", "FX-YAML-MALFORMED-SECRET"])
    m_k = R("CK-MAL-K8S-ONLY", ["-d", "mal", "--framework", "kubernetes"] + APP, repo, "malformed YAML, kubernetes framework only", ["FX-YAML-MALFORMED"])
    mal_secret_found = any(e["status"] == "failed" and e["fw"] == "secrets" and (e["file"] or "").endswith("malformed-secret.yaml") for e in entries(m_run))
    exit_map["malformed YAML only (kubernetes fw)"] = m_k["exit"]
    exit_map["malformed YAML + secrets fw"] = m_run["exit"]
    ctx.note("zero_result_runs", {
        "malformed_k8s_top_keys": sorted((reports(m_k) or [{}])[0].keys()), "malformed_k8s_stderr_head": m_k["stderr_excerpt"][:240],
        "missing_file_top_keys": sorted((reports(r_nofile) or [{}])[0].keys()), "missing_file_stderr_head": r_nofile["stderr_excerpt"][:240],
        "missing_file_exit": r_nofile["exit"], "malformed_exit": m_k["exit"]})
    ctx.note("malformed_observations", {
        "kubernetes_only_exit": m_k["exit"], "kubernetes_only_summary": summary(m_k), "kubernetes_only_parsing_errors_count": len(parse_errors(m_k)),
        "all_fw_exit": m_run["exit"], "all_fw_summary": summary(m_run), "frameworks_reported": fws(m_run),
        "secrets_finding_on_malformed_secret_file": mal_secret_found, "C01_echoed_stdout": "C01" in m_run["canary_in_stdout"],
        "C01_echoed_stderr": "C01" in m_run["canary_in_stderr"] or "C01" in m_k["canary_in_stderr"]})
    ctx.hyp("H-MAL", "observed",
            f"kubernetes framework on malformed YAML: exit {m_k['exit']}, parsing_errors={len(parse_errors(m_k))}, summary {summary(m_k)}. With the secrets framework also enabled: "
            f"secret finding on the malformed-with-secret file = {mal_secret_found} (secrets scanning is line-based and independent of YAML parsing), "
            f"C01 echoed in stdout = {'C01' in m_run['canary_in_stdout']}. The product uses gitleaks for SCAN-08; this only records checkov's behaviour.",
            [m_k["case"], m_run["case"]])

    # ------------------------------------------------------------------ H-ARG / H-OUT : hostile names
    dn = ctx.case_dir("CK-NAME")
    nr = dn / "repo"
    write_tree(nr, {f"names/{n}": fx.K8S_INSECURE for n in fx.HOSTILE_NAMES.values()})
    write_tree(nr, {"-rf.yaml": fx.K8S_INSECURE, "--config=evil.yaml": fx.K8S_INSECURE})   # root copies so cwd can stay = repo
    argv_names = []
    for n in fx.HOSTILE_NAMES.values():
        argv_names.append(f"names/{n}")
    n_all = R("CK-NAME-ALL", ["-f"] + argv_names + K + APP, nr, "all hostile names as '-f names/<name>' (never option-like: dir prefix)", list(fx.HOSTILE_NAMES))
    got = {e["file"] for e in entries(n_all)}
    name_round = {fid: any(e["file"] and e["file"].endswith(nm) for e in entries(n_all)) for fid, nm in fx.HOSTILE_NAMES.items()}
    esc_raw = b"\x1b" in n_all["_stdout"]
    n_dash = R("CK-NAME-DASH-BARE", ["-f", "-rf.yaml"] + K + APP, nr, "dash-leading name as bare -f argument (file in cwd)", ["FX-NAME-DASH-1"])
    n_dash_eq = R("CK-NAME-DASH-EQ", ["--file=-rf.yaml"] + K + APP, nr, "dash-leading name via --file=<name>", ["FX-NAME-DASH-1"])
    n_dash2_eq = R("CK-NAME-DASH2-EQ", ["--file=--config=evil.yaml"] + K + APP, nr, "option-like name '--config=evil.yaml' via --file=<name>", ["FX-NAME-DASH-2"])
    ctx.note("hostile_name_observations", {
        "n_all_exit": n_all["exit"], "json_ok": n_all.get("json_parse") == "ok", "round_trip_by_fixture": name_round,
        "raw_ESC_in_json_stdout": esc_raw, "raw_newline_in_file_path_field": any("\n" in (f or "") for f in got),
        "dash_bare": {"exit": n_dash["exit"], "analysed": bool(entries(n_dash)), "stderr_head": n_dash["stderr_excerpt"][:140]},
        "dash_equals": {"exit": n_dash_eq["exit"], "analysed": bool(entries(n_dash_eq))},
        "dash2_equals_analysed": bool(entries(n_dash2_eq)), "dash2_equals_exit": n_dash2_eq["exit"],
        "dash2_was_not_taken_as_config": "config" not in n_dash2_eq["stderr_excerpt"].lower() or bool(entries(n_dash2_eq))})
    arg_ok = all(name_round.values()) and n_all.get("json_parse") == "ok"
    ctx.hyp("H-ARG", "confirmed" if arg_ok and bool(entries(n_dash_eq)) and bool(entries(n_dash2_eq)) else "partially confirmed",
            f"Hostile names given as '-f names/<name>' (argv array, no shell): round-trip in file_path for all fixtures={arg_ok} ({name_round}). "
            f"Dash-leading names: bare '-f -rf.yaml' exit {n_dash['exit']} analysed={bool(entries(n_dash))}; '--file=-rf.yaml' exit {n_dash_eq['exit']} "
            f"analysed={bool(entries(n_dash_eq))}; '--file=--config=evil.yaml' analysed={bool(entries(n_dash2_eq))}. argparse nargs='+' means '--' is not a "
            "reliable terminator: the product should always pass a directory-prefixed path or the --file=<path> form.",
            [n_all["case"], n_dash["case"], n_dash_eq["case"], n_dash2_eq["case"]])

    # determinism (H-OUT)
    k_f2 = R("CK-K8S-F-AGAIN", ["-f", "k8s/insecure.yaml"] + K + APP, repo, "determinism re-run of CK-K8S-F", ["FX-K8S-INSECURE"])
    same_json = k_f.get("_json") == k_f2.get("_json")
    ctx.hyp("H-OUT", "confirmed" if (same_json and k_f.get("json_parse") == "ok" and n_all.get("json_parse") == "ok") else "partially confirmed",
            f"-o json emits parseable JSON on stdout (logs/warnings go to stderr): baseline ok={k_f.get('json_parse')}, hostile-name run ok={n_all.get('json_parse')}; "
            f"re-run parsed JSON identical={same_json}; stdout bytes identical={k_f['_stdout'] == k_f2['_stdout']}. Top level is "
            f"{type(k_f.get('_json')).__name__} for one framework and {type(mfw.get('_json')).__name__} when two frameworks run (parser must accept both). "
            f"Raw ESC bytes in JSON stdout for the ANSI-named file: {esc_raw}; newline in file_path field: {any(chr(10) in (f or '') for f in got)}. "
            f"Parse errors are reported in results.parsing_errors / summary.parsing_errors (see malformed_observations).",
            [k_f["case"], k_f2["case"], n_all["case"], mfw["case"]])

    # ------------------------------------------------------------------ H-CFG / H-OVR / H-EXE
    mkdir = lambda cid: ctx.case_dir(cid)

    def cfgtree(cid, config_text, config_rel=".checkov.yaml", exec_marker=None, extra=None):
        dd = mkdir(cid)
        rp = dd / "repo"
        files = {"k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/valid.yaml": fx.K8S_VALID}
        if config_text is not None:
            files[config_rel] = config_text
        if exec_marker:
            files["custom_checks/pwt_spike_check.py"] = evil_module(dd / exec_marker)
            files["custom_checks/__init__.py"] = ""
        files.update(extra or {})
        write_tree(rp, files)
        return dd, rp

    # twin: custom check module present but nothing references it, no config (discovery of checks with NO explicit config)
    tw_d, tw = cfgtree("CK-CFG-TWIN", None, exec_marker=EXEC_MARK + "-NOCFG")
    t_run = R("CK-CFG-TWIN-RUN", ["-d", "."] + K + APP, tw, "clean twin: custom check module present in tree, no config, no flags referencing it", ["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"])
    twin_failed = failed_ids(t_run)
    exec_obs["no config, module present in tree, -d ."] = {"marker": has_marker(tw_d, EXEC_MARK + "-NOCFG"), "custom_check_ran": "CKV_PWT_9" in (passed_ids(t_run) | failed_ids(t_run)), "exit": t_run["exit"]}
    ctx.note("twin_summary", {"failed": len(twin_failed), "exit": t_run["exit"], "summary": summary(t_run)})

    skip_cfg = "skip-check:\n" + "".join(f"  - {i}\n" for i in sorted(twin_failed)) if twin_failed else "skip-check:\n  - CKV_K8S_1\n"
    sk_d, sk = cfgtree("CK-CFG-SKIP", skip_cfg)
    sk_run = R("CK-CFG-SKIP-RUN", ["-d", "."] + K + APP, sk, "repo .checkov.yaml skip-check listing every failing id; -d . cwd=repo", ["FX-INJ-CHECKOV-CONFIG"])
    sf_d, sf = cfgtree("CK-CFG-SOFT", "soft-fail: true\n")
    sf_run = R("CK-CFG-SOFT-RUN", ["-d", "."] + K + APP, sf, "repo .checkov.yaml soft-fail: true (exit-code suppression)", ["FX-INJ-CHECKOV-CONFIG"])
    fw_d, fwp = cfgtree("CK-CFG-FW", "framework:\n  - terraform\n")
    fw_run = R("CK-CFG-FW-RUN", ["-d", "."] + APP, fwp, "repo .checkov.yaml framework: [terraform]; app passes no --framework", ["FX-INJ-CHECKOV-CONFIG"])
    fw2_run = R("CK-CFG-FW-APPFLAG", ["-d", ".", "--framework", "kubernetes"] + APP, fwp, "same repo config, app passes --framework kubernetes (CLI vs config precedence)", ["FX-INJ-CHECKOV-CONFIG"])
    yml_d, ymlp = cfgtree("CK-CFG-YML", skip_cfg, config_rel=".checkov.yml")
    yml_run = R("CK-CFG-YML-RUN", ["-d", "."] + K + APP, ymlp, "repo .checkov.yml (yml suffix) skip-check", ["FX-INJ-CHECKOV-CONFIG"])
    # config only in the -d directory (not cwd)
    dd_d = mkdir("CK-CFG-DDIR")
    write_tree(dd_d / "repo", {"k8s/insecure.yaml": fx.K8S_INSECURE, "k8s/.checkov.yaml": skip_cfg})
    dd_run = R("CK-CFG-DDIR-RUN", ["-d", "k8s"] + K + APP, dd_d / "repo", "config only inside the -d directory; cwd = repo root (no config there)", ["FX-INJ-CHECKOV-CONFIG"])
    # -f with cwd = repo (positive control for the neutralisation by cwd)
    cf_run = R("CK-CFG-F-CWDREPO", ["-f", "k8s/insecure.yaml"] + K + APP, sk, "-f single file, cwd = repo root with hostile skip config (positive control)", ["FX-INJ-CHECKOV-CONFIG"])
    # -f, cwd = child directory of the repo (config at the parent)
    par_d, par = cfgtree("CK-CFG-PARENT", None)
    (par_d / ".checkov.yaml").write_text(skip_cfg)          # config in the PARENT of cwd (cwd = <case>/repo)
    cp_run = R("CK-CFG-F-PARENT", ["-f", "k8s/insecure.yaml"] + K + APP, par, "-f, cwd = repo, hostile config only in the PARENT directory of cwd", ["FX-INJ-CHECKOV-CONFIG"])
    # -f, cwd outside the repo (app-owned scratch directory)
    co_run = R("CK-CFG-F-CWDOUT", ["-f", "repo/k8s/insecure.yaml"] + K + APP, sk_d, "-f, cwd = app-owned directory outside the repo; hostile config in repo root", ["FX-INJ-CHECKOV-CONFIG"])
    # explicit --config-file with hostile config at cwd
    app_cfg = sk_d / "app-config.yaml"
    app_cfg.write_text("framework:\n  - kubernetes\n")
    oc_run = R("CK-OVR-APPCFG-CWDREPO", ["--config-file", str(app_cfg), "-f", "k8s/insecure.yaml"] + APP, sk, "explicit --config-file (app) + hostile repo config at cwd", ["FX-INJ-CHECKOV-CONFIG"])
    oo_run = R("CK-OVR-APPCFG-CWDOUT", ["--config-file", str(app_cfg), "-f", "repo/k8s/insecure.yaml"] + APP, sk_d, "explicit --config-file (app), cwd outside the repo", ["FX-INJ-CHECKOV-CONFIG"])
    # show-config: which sources are loaded
    sc_run = R("CK-OVR-SHOWCONFIG", ["--show-config", "--config-file", str(app_cfg), "-f", "k8s/insecure.yaml"] + K, sk, "--show-config: which config sources are active", ["FX-INJ-CHECKOV-CONFIG"], parse=False)
    show = (sc_run["_stdout"] + sc_run["_stderr"]).decode("utf-8", "replace")
    srcs = sorted(set(re.findall(r"Config File \(([^)]*)\)", show)))
    ctx.note("show_config_sources", [re.sub(r".*/work/checkov/", "<work>/", x) for x in srcs][:6])
    ctx.note("show_config_exit", sc_run["exit"])

    twin_equiv = lambda r_: failed_ids(r_) == twin_failed and r_["exit"] == t_run["exit"]
    skipped_all = lambda r_: not failed_ids(r_) and bool(twin_failed)
    cfg_loaded = {
        "skip_check_dir_cwd": skipped_all(sk_run), "yml_suffix": skipped_all(yml_run), "d_dir_only": skipped_all(dd_run),
        "f_with_cwd_repo": skipped_all(cf_run), "f_cwd_in_subdir_parent_config": skipped_all(cp_run), "f_cwd_outside": skipped_all(co_run),
        "soft_fail_exit": sf_run["exit"], "framework_override_results": len(entries(fw_run)), "cli_framework_vs_config": len(entries(fw2_run)),
        "explicit_config_file_does_not_stop_cwd_discovery": skipped_all(oc_run),
    }
    ctx.note("config_discovery", cfg_loaded)
    ctx.note("neutralisation_equivalence", {"f_cwd_outside_equals_twin_findings": failed_ids(co_run) == twin_failed, "app_cfg_cwd_outside_equals_twin": failed_ids(oo_run) == twin_failed})
    exit_map["clean twin (insecure+valid, no config)"] = t_run["exit"]
    exit_map["repo soft-fail:true config"] = sf_run["exit"]
    h_cfg_hits = [k for k in ("skip_check_dir_cwd", "yml_suffix", "d_dir_only", "f_with_cwd_repo") if cfg_loaded[k]]
    if twin_failed and h_cfg_hits:
        ctx.hyp("H-CFG", "confirmed",
                f"Repo-controlled .checkov.yaml/.checkov.yml is auto-loaded and changes results: skip-check config removed all {len(twin_failed)} failing ids "
                f"(loaded via {h_cfg_hits}); soft-fail:true config turned exit {t_run['exit']} into {sf_run['exit']}; framework:[terraform] config yielded "
                f"{len(entries(fw_run))} entries vs {len(entries(t_run))}. Search locations observed: -d directory ({cfg_loaded['d_dir_only']}), cwd with -d ({cfg_loaded['skip_check_dir_cwd']}), "
                f"cwd with -f ({cfg_loaded['f_with_cwd_repo']}); config only in the parent of cwd was loaded: {cfg_loaded['f_cwd_in_subdir_parent_config']}; config loaded when cwd is outside the repo and "
                f"-f is used: {cfg_loaded['f_cwd_outside']}. User-level config: HOME is a scratch directory so HOME discovery was not exercised with content.",
                [t_run["case"], sk_run["case"], yml_run["case"], dd_run["case"], cf_run["case"], cp_run["case"], co_run["case"], sf_run["case"], fw_run["case"]])
    else:
        ctx.hyp("H-CFG", "refuted" if twin_failed else "not testable", f"No repo-config effect observed (hits={h_cfg_hits}, twin_failed={len(twin_failed)}).",
                [t_run["case"], sk_run["case"]])
    appcfg_neutral = failed_ids(oo_run) == twin_failed
    ctx.hyp("H-OVR", "refuted" if cfg_loaded["explicit_config_file_does_not_stop_cwd_discovery"] else "confirmed",
            f"Explicit --config-file does {'NOT ' if cfg_loaded['explicit_config_file_does_not_stop_cwd_discovery'] else ''}stop discovery of the repo config at cwd "
            f"(hostile skip still applied with --config-file, cwd=repo: {cfg_loaded['explicit_config_file_does_not_stop_cwd_discovery']}; app-config + cwd outside repo equals "
            f"twin: {appcfg_neutral}). Config sources are merged, not replaced; the effective neutralisation is the working directory (cwd outside the repo, absolute/relative "
            "paths into the repo via -f, never -d), see CK-CFG-F-CWDOUT. --show-config source list is in note show_config_sources.",
            [sk_run["case"], oc_run["case"], oo_run["case"], co_run["case"], sc_run["case"]])

    # ---- H-EXE : one run per configuration; `kind` says who set up the configuration
    HOSTILE_CFG = "external-checks-dir:\n  - ./custom_checks\nframework:\n  - kubernetes\n"
    exec_obs.clear()
    exec_obs["no config, module present in tree, -d ."] = {"kind": "control: nothing references the module", "marker": has_marker(tw_d, EXEC_MARK + "-NOCFG"),
                                                         "custom_check_ran": "CKV_PWT_9" in (passed_ids(t_run) | failed_ids(t_run)), "exit": t_run["exit"], "case": t_run["case"]}

    def exec_case(cid, marker_suffix, argv, cwd_sel, purpose, label, kind, *, extra=None, pre=None):
        dd_, rp_ = cfgtree(cid, HOSTILE_CFG, exec_marker=EXEC_MARK + marker_suffix)
        if pre:
            pre(dd_)
        rr = R(cid + "-RUN", [a.replace("<CASE>", str(dd_)) for a in argv], dd_ if cwd_sel == "case" else rp_, purpose, ["FX-INJ-CHECKOV-CONFIG", "FX-INJ-EXEC-CHECKOV"])
        exec_obs[label] = {"kind": kind, "marker": has_marker(dd_, EXEC_MARK + marker_suffix),
                           "custom_check_ran": "CKV_PWT_9" in (passed_ids(rr) | failed_ids(rr)), "exit": rr["exit"], "case": rr["case"],
                           "stderr_loader_lines": [l[:160] for l in rr["stderr_excerpt"].splitlines() if re.search(r"pwt|custom|external|signature|digest|import", l, re.I)][:4]}
        return rr, dd_

    e1, ex_d = exec_case("CK-EXE-CFG-D", "", ["-d", "."] + APP, "repo", "PRIMARY: repo .checkov.yaml external-checks-dir -> in-repo custom check module; -d .; cwd=repo",
                         "repo .checkov.yaml external-checks-dir, -d ., cwd=repo", "unprotected: repo config reachable")
    e2, _ = exec_case("CK-EXE-CFG-F", "-F", ["-f", "k8s/insecure.yaml"] + K + APP, "repo", "same hostile config; app passes -f + explicit --framework; cwd=repo",
                      "app flags (-f, --framework), cwd=repo", "app-configured: flags only, cwd=repo")

    def mk_app_cfg(dd_):
        (dd_ / "app-config.yaml").write_text("framework:\n  - kubernetes\n")

    e3, _ = exec_case("CK-EXE-APPCFG", "-APPCFG", ["--config-file", "<CASE>/app-config.yaml", "-f", "k8s/insecure.yaml"] + APP, "repo",
                      "explicit app --config-file, hostile config at cwd=repo", "app --config-file + -f, cwd=repo", "app-configured: explicit config file, cwd=repo", pre=mk_app_cfg)
    e4, _ = exec_case("CK-EXE-CWDOUT", "-CWDOUT", ["-f", "repo/k8s/insecure.yaml", "--skip-download"] + K + APP, "case",
                      "NEGATIVE: app-owned cwd outside the repo, -f only", "APP: cwd outside repo, -f, --framework, --skip-download", "app-configured: cwd outside repo")
    e5, _ = exec_case("CK-EXE-CWDOUT-APPCFG", "-CWDOUT-APPCFG", ["--config-file", "<CASE>/app-config.yaml", "-f", "repo/k8s/insecure.yaml", "--skip-download"] + APP, "case",
                      "NEGATIVE: explicit app --config-file AND cwd outside repo", "APP: explicit --config-file, cwd outside repo, -f", "app-configured: explicit config file + cwd outside repo", pre=mk_app_cfg)
    e7, _ = exec_case("CK-EXE-CWDOUT-D", "-CWDOUT-D", ["-d", "repo", "--skip-download"] + K + APP, "case",
                      "cwd outside repo but -d points at the repo (config in the -d directory)", "app: cwd outside repo, -d <repo>", "app-configured: cwd outside repo but -d used")
    # signature verification option as an additional gate when hostile config is reachable
    spki = bytes.fromhex("3059301306072a8648ce3d020106082a8648ce3d030107034200") + bytes.fromhex(
        "04" "60fed4ba255a9d31c961eb74c6356d68c049b8923b61fa6ce669622e60f29fb6" "7903fe1008b8bc99a41ae9e95628bc64f2f1b20c2d7e9f5177a3c294d4462299")
    pem = "-----BEGIN PUBLIC KEY-----\n" + base64.encodebytes(spki).decode() + "-----END PUBLIC KEY-----\n"   # public RFC 6979 test-vector key, never a secret

    def mk_pem(dd_):
        (dd_ / "test-pub.pem").write_text(pem)

    e6, _ = exec_case("CK-EXE-SIGKEY", "-SIGKEY", ["--external-checks-public-key", "<CASE>/test-pub.pem", "-f", "k8s/insecure.yaml"] + K + APP, "repo",
                      "hostile config at cwd=repo + --external-checks-public-key (signature gate)", "app: --external-checks-public-key, cwd=repo",
                      "app-configured: signature gate, cwd=repo", pre=mk_pem)
    ctx.note("exec_observations", {k: {kk: vv for kk, vv in v.items() if kk != "case"} for k, v in exec_obs.items()})
    exit_map["signature gate with unsigned external check (cwd=repo, hostile config)"] = e6["exit"]
    ctx.note("runs_with_new_execution_markers", {c_["case"]: c_["new_execution_markers"] for c_ in all_runs if c_["new_execution_markers"]})
    triggered = [k for k, v in exec_obs.items() if v["marker"]]
    held = [k for k, v in exec_obs.items() if not v["marker"]]
    app_def = [k for k in triggered if exec_obs[k]["kind"].startswith("app-configured")]
    unprot = [k for k in triggered if exec_obs[k]["kind"].startswith("unprotected")]
    ctx.note("exec_summary", {
        "marker_configurations_defects": app_def, "unprotected_repo_config_executes": bool(unprot),
        "no_config_autoloads_checks": exec_obs["no config, module present in tree, -d ."]["marker"],
        "verified_neutralisations_no_marker": held,
        "custom_check_registered_when_marker": {k: exec_obs[k]["custom_check_ran"] for k in triggered}})
    ev = [v["case"] for v in exec_obs.values()]
    if triggered:
        ctx.hyp("H-EXE", "confirmed",
                f"A repository-supplied Python module referenced from .checkov.yaml (external-checks-dir) is imported and executed: marker created in {len(triggered)} configuration(s): {triggered}. "
                f"Of these, app-configured configurations that are DEFECTS (marker despite app-owned flags/config): {app_def}; the unprotected repo-config configuration: {unprot}. "
                f"Configurations where NO marker appeared (verified negatives): {held}. Without any config the module in the tree was not auto-loaded: "
                f"{not exec_obs['no config, module present in tree, -d .']['marker']}. Explicit --config-file does not stop discovery of the repo config at cwd (H-OVR); "
                "what neutralised it was the working directory (app cwd outside the repo with -f), and the signature gate option when the config stays reachable. "
                "Per-configuration detail in note exec_observations.", ev)
    else:
        ctx.hyp("H-EXE", "refuted", f"No marker in any configuration: {sorted(exec_obs)}", ev)

    # ------------------------------------------------------------------ H-NET / DIST-01
    def net_view(r_):
        err = r_["stderr_excerpt"] + r_["stdout_excerpt"]
        hosts = sorted(set(re.findall(r"(api0\.prismacloud\.io|[a-z0-9.-]*bridgecrew\.[a-z]+|pypi\.org|[a-z0-9.-]*prismacloud\.io)", err)))
        sigs = [s for s in r_["network_attempt_signals"] if not s.startswith("http")]
        return {"hosts_in_stderr": hosts, "signals": sigs, "wall_ms": r_["wall_ms"], "timed_out": r_["timed_out"], "stderr_bytes": r_["stderr_bytes"]}

    n_def = k_f
    n_skip = R("CK-NET-SKIPDL", ["-f", "k8s/insecure.yaml", "--skip-download"] + K + APP, repo, "--skip-download (no guideline/mapping fetch)", ["FX-NET-DEFAULT"])
    n_env = R("CK-NET-SKIPDL-ENV", ["-f", "k8s/insecure.yaml", "--skip-download", "--skip-results-upload"] + K + APP, repo,
              "--skip-download + --skip-results-upload + CKV_SKIP_PACKAGE_UPDATE_CHECK=True", ["FX-NET-DEFAULT"],
              extra_env={"CKV_SKIP_PACKAGE_UPDATE_CHECK": "True"})
    n_ver_cache = [f for f in r_ver["files_created"] if "update_checker" in f]
    nv = {"default": net_view(n_def), "skip_download": net_view(n_skip), "skip_download_and_env": net_view(n_env), "version_run": net_view(r_ver)}
    ctx.note("network_views", nv)
    ctx.note("network_options", {"flags": {k: (k in flags) for k in ("--skip-download", "--skip-results-upload", "--bc-api-key", "--no-cert-verify", "--external-checks-git", "--support")},
                                 "update_check_cache_file_written_by_version_run": bool(n_ver_cache),
                                 "same_findings_with_skip_download": failed_ids(n_skip) == failed_ids(n_def),
                                 "bc_ids_differ_note": "see check_entry_keys"})
    attempt_default = bool(nv["default"]["hosts_in_stderr"] or nv["default"]["signals"])
    attempt_skip = bool(nv["skip_download"]["hosts_in_stderr"] or nv["skip_download"]["signals"])
    attempt_both = bool(nv["skip_download_and_env"]["hosts_in_stderr"] or nv["skip_download_and_env"]["signals"])
    ctx.hyp("H-NET", "confirmed" if attempt_default else "refuted",
            f"Default run in a loopback-only netns: network attempt visible = {attempt_default} (hosts named in stderr: {nv['default']['hosts_in_stderr']}; "
            f"signals {nv['default']['signals']}; exit/results unaffected: findings identical to skip-download run = {failed_ids(n_skip) == failed_ids(n_def)}; wall {nv['default']['wall_ms']} ms, "
            f"timed_out {nv['default']['timed_out']}). With --skip-download: attempt visible = {attempt_skip} (hosts {nv['skip_download']['hosts_in_stderr']}); "
            f"with --skip-download --skip-results-upload + CKV_SKIP_PACKAGE_UPDATE_CHECK=True: attempt visible = {attempt_both} (hosts {nv['skip_download_and_env']['hosts_in_stderr']}, "
            f"signals {nv['skip_download_and_env']['signals']}). A version run writes an update-check cache file: {bool(n_ver_cache)}. "
            "Failure is non-fatal and bounded here (name resolution fails immediately in a netns); behaviour on a black-holed network (long timeouts) was not tested.",
            [n_def["case"], n_skip["case"], n_env["case"], r_ver["case"]])
    ctx.hyp("DIST-01", "observed",
            f"Pinned checkov performs a guidelines/mappings download (api0.prismacloud.io) and an update check without an API key unless disabled; --skip-download "
            f"{'removed' if attempt_default and not attempt_skip else 'did not remove'} the visible attempt; env/flags combination {'removed' if attempt_default and not attempt_both else 'did not remove'} "
            f"all visible attempts. Product consequence: run with --skip-download and CKV_SKIP_PACKAGE_UPDATE_CHECK=True (child env allowlist) inside the network-denied runner; "
            "BC_* ids are then unavailable.", [n_def["case"], n_skip["case"], n_env["case"]])

    # ------------------------------------------------------------------ clean / exit map completion
    clean_ids = sorted(passed_ids(k_f))[:1]
    if clean_ids:
        cl = R("CK-EXIT-CLEAN", ["-f", "k8s/valid.yaml", "--check", clean_ids[0]] + K + APP, repo, "clean run: valid fixture restricted to one passing check", ["FX-K8S-VALID"])
        exit_map["clean (only passing checks selected)"] = cl["exit"]
    sf = R("CK-EXIT-SOFTFAIL-FLAG", ["-f", "k8s/insecure.yaml", "--soft-fail"] + K + APP, repo, "--soft-fail with failing checks", ["FX-K8S-INSECURE"])
    exit_map["--soft-fail with failures"] = sf["exit"]
    ctx.note("exit_map", exit_map)
    silent0 = [n for n, r_ in (("malformed YAML (kubernetes fw)", m_k), ("nonexistent -f path", r_nofile)) if r_["exit"] == 0]
    soft = [n for n, v in (("--soft-fail flag", exit_map["--soft-fail with failures"]), ("repo soft-fail:true config", exit_map["repo soft-fail:true config"])) if v == 0]
    ok = exit_map.get("clean (only passing checks selected)") == 0 and exit_map["findings (k8s insecure, -f)"] == 1 and exit_map["bad flag"] == 2
    ctx.hyp("H-EXIT", "partially confirmed" if (ok and (silent0 or soft)) else ("confirmed" if ok else "refuted"),
            f"Observed map: {exit_map}. clean -> 0, findings (failed checks) -> 1, usage error (argparse) -> 2. Conditions: exit 0 despite failures with {soft} "
            f"(exit code is repository-suppressible: derive findings from JSON, not exit); exit 0 with zero results for {silent0} "
            "(unparseable or missing input is NOT an error exit; the product must treat summary-only/zero-resource JSON as 'not assessed', not as a pass); "
            "crash exit code not observed in these runs; a signature-gate rejection exits 2 (same code as a usage error).",
            [k_f["case"], sf["case"], r_bad["case"], r_nofile["case"], m_k["case"], sf_run["case"], e6["case"]] + ([cl["case"]] if clean_ids else []))
    ctx.note("max_wall_ms", max(r_["wall_ms"] for r_ in all_runs))
    ctx.note("run_count", len(all_runs))

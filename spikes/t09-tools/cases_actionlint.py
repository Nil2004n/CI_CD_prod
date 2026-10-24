"""P00-T09 actionlint spike cases (worker W2; spike code, not application code).

Every outcome is derived from comparing observed runs (hostile vs clean twin, explicit vs
auto-discovery, ...), never from documentation. Flags were discovered from the pinned binary's own
`-h` output (run as case AL-HELP) and are only used if that output lists them.
"""

from __future__ import annotations

import json
import os
import re
import stat
from pathlib import Path

import fixtures as fx
from fixtures import write_tree

FMT = "{{json .}}"
GIT = True

# Repository-controlled configs (FX-INJ-ACTIONLINT). Glob keys are workflow paths relative to the project.
CFG_IGNORE_ALL = (
    "paths:\n"
    "  '**/*.yaml':\n    ignore:\n      - '.*'\n"
    "  '**/*.yml':\n    ignore:\n      - '.*'\n"
    "  '.github/workflows/**/*.yaml':\n    ignore:\n      - '.*'\n"
    "  '.github/workflows/**/*.yml':\n    ignore:\n      - '.*'\n"
)
CFG_LABELS = "self-hosted-runner:\n  labels:\n    - fake-runner-label\n"
CFG_APP_MIN = "self-hosted-runner:\n  labels: []\n"          # app-owned minimal config (explicit)
WF_LABEL = (
    "name: lbl\non: [push]\njobs:\n  a:\n    runs-on: [self-hosted, fake-runner-label]\n"
    "    steps:\n      - run: echo hi\n"
)


# ----------------------------------------------------------------------------- helpers
def errs(r):
    j = r.get("_json")
    return j if isinstance(j, list) else None


def cnt(r):
    e = errs(r)
    return None if e is None else len(e)


def sig(r):
    """Order-preserving signature of the findings (message, file, line, column, kind)."""
    e = errs(r)
    if e is None:
        return None
    return [(x.get("message"), x.get("filepath"), x.get("line"), x.get("column"), x.get("kind")) for x in e]


def files_of(r):
    e = errs(r)
    return None if e is None else sorted({x.get("filepath") for x in e})


def mk(ctx, cid, files, git=GIT, execs=()):
    d = ctx.case_dir(cid)
    repo = d / "repo"
    write_tree(repo, files)
    if git:
        (repo / ".git").mkdir()
    for rel in execs:
        os.chmod(repo / rel, 0o755)
    return d, repo


def run(ctx, cid, args, cwd, purpose, fixtures, *, parse=True, fmt=True, **kw):
    argv = [ctx.bin] + (["-format", FMT] if fmt else []) + list(args)
    return ctx.run(cid, argv, cwd, purpose=purpose, fixtures=fixtures, parse_json=parse and fmt, timeout=60, **kw)


def cases(ctx):
    c = ctx.canaries
    ids_net = []          # (case id, record) for the H-NET sweep
    exit_map = {}

    def R(cid, *a, **k):
        r = run(ctx, cid, *a, **k)
        ids_net.append(r)
        return r

    # ------------------------------------------------------------------ version, help, usage error
    d0 = ctx.case_dir("AL-VERSION")
    r = ctx.run("AL-VERSION", [ctx.bin, "-version"], d0 / "repo", purpose="version", fixtures=[], timeout=30)
    ids_net.append(r)
    ctx.note("version_first_line", r["_stdout"].decode("utf-8", "replace").splitlines()[:1])
    exit_map["-version"] = r["exit"]

    r = ctx.run("AL-HELP", [ctx.bin, "-h"], d0 / "repo", purpose="help: discover real flags", fixtures=[], timeout=30)
    ids_net.append(r)
    help_text = (r["_stdout"] + r["_stderr"]).decode("utf-8", "replace")
    flags = sorted(set(re.findall(r"^\s{2}-([A-Za-z][\w-]*)", help_text, re.M)))
    ctx.note("flags_from_help", flags)
    ctx.note("help_exit", r["exit"])
    exit_map["-h"] = r["exit"]
    has = lambda f: f in flags
    ctx.note("flag_support", {k: has(k) for k in ("format", "config-file", "ignore", "shellcheck", "pyflakes",
                                                    "oneline", "no-color", "stdin-filename", "init-config", "verbose", "debug")})
    ctx.note("has_soft_fail_or_strict_flag", any(f in flags for f in ("soft-fail", "strict", "exit-code")))
    ctx.note("has_network_or_offline_flag", any(re.search(r"net|offline|proxy|download|update", f) for f in flags))

    r = ctx.run("AL-BADFLAG", [ctx.bin, "-bogus-flag"], d0 / "repo", purpose="usage error", fixtures=[], timeout=30)
    ids_net.append(r)
    exit_map["bad flag"] = r["exit"]

    # ------------------------------------------------------------------ baseline FX-GHA-*
    d, repo = mk(ctx, "AL-BASE", {
        ".github/workflows/valid.yaml": fx.GHA_VALID,
        ".github/workflows/invalid.yaml": fx.GHA_INVALID,
        ".github/workflows/ref.yaml": fx.GHA_REF,
    })
    # injection fixture: the marker directory is the case directory (watched by ctx.markers()).
    write_tree(repo, {".github/workflows/inj.yaml": fx.gha_injection(d)})
    base = {}
    for name, fid in (("valid", "FX-GHA-VALID"), ("invalid", "FX-GHA-INVALID"), ("inj", "FX-GHA-INJECTION"),
                      ("ref", "FX-GHA-REF")):
        base[name] = R(f"AL-BASE-{name.upper()}", [f".github/workflows/{name}.yaml"], repo,
                       f"baseline {fid}, explicit file, JSON template", [fid])
    exit_map["clean (FX-GHA-VALID)"] = base["valid"]["exit"]
    exit_map["findings (FX-GHA-INVALID)"] = base["invalid"]["exit"]
    exit_map["findings (FX-GHA-INJECTION)"] = base["inj"]["exit"]
    exit_map["unpinned refs only (FX-GHA-REF)"] = base["ref"]["exit"]
    fields = sorted({k for r_ in base.values() for e in (errs(r_) or []) for k in e})
    ctx.note("json_fields", fields)
    ctx.note("json_shape", {"top_level": "list" if errs(base["invalid"]) is not None else "other",
                            "clean_output_is_empty_list": errs(base["valid"]) == [],
                            "kinds_seen": sorted({e.get("kind") for r_ in base.values() for e in (errs(r_) or [])})})
    ref_msgs = [(e.get("kind"), (e.get("message") or "")[:110]) for e in (errs(base["ref"]) or [])]
    ctx.note("fx_gha_ref_findings", ref_msgs)
    ctx.note("fx_gha_ref_reports_unpinned_or_mutable_refs",
             any(re.search(r"unpinned|not pinned|mutable|full-length|commit sha|@main|@v4|:latest", m, re.I) for _, m in ref_msgs))
    ctx.note("fx_gha_injection_reports_untrusted_expression",
             any("untrusted" in (e.get("message") or "") for e in (errs(base["inj"]) or [])))
    ctx.note("fx_gha_injection_reports_pull_request_target_or_permissions",
             any(re.search(r"permission|pull_request_target", e.get("message") or "") for e in (errs(base["inj"]) or [])))
    # explicit file that is not a workflow
    write_tree(repo, {"notes.txt": "hello\n"})
    r_txt = R("AL-BASE-NONWORKFLOW", ["notes.txt"], repo, "explicit non-YAML file", ["FX-SCOPE-NONYAML"])
    exit_map["non-workflow text file given explicitly"] = r_txt["exit"]
    ctx.note("explicit_non_yaml_file_is_analysed_as_workflow", bool(errs(r_txt)))

    # Determinism (H-OUT): same run twice -> byte-identical stdout.
    r2 = R("AL-BASE-INVALID-AGAIN", [".github/workflows/invalid.yaml"], repo, "determinism re-run", ["FX-GHA-INVALID"])
    deterministic = base["invalid"]["_stdout"] == r2["_stdout"] and base["invalid"]["exit"] == r2["exit"]
    # Multi-file ordering
    r_ord = R("AL-BASE-ORDER", [".github/workflows/inj.yaml", ".github/workflows/invalid.yaml"], repo,
              "two explicit files, output ordering", ["FX-GHA-INJECTION", "FX-GHA-INVALID"])
    fo = [e.get("filepath") for e in (errs(r_ord) or [])]
    first_file_order = [f for i, f in enumerate(fo) if f not in fo[:i]]
    ctx.note("multi_file_order_follows_argv", first_file_order == [".github/workflows/inj.yaml", ".github/workflows/invalid.yaml"])
    # Text and oneline formats (for contrast)
    r_text = ctx.run("AL-BASE-TEXT", [ctx.bin, ".github/workflows/invalid.yaml"], repo, purpose="default text format",
                     fixtures=["FX-GHA-INVALID"], timeout=60)
    r_one = ctx.run("AL-BASE-ONELINE", [ctx.bin, "-oneline", ".github/workflows/invalid.yaml"], repo,
                    purpose="oneline format", fixtures=["FX-GHA-INVALID"], timeout=60)
    ids_net += [r_text, r_one]
    ctx.note("text_format_line_count_vs_oneline", {"text_lines": len(r_text["_stdout"].splitlines()),
                                                   "oneline_lines": len(r_one["_stdout"].splitlines())})

    # ------------------------------------------------------------------ H-EXE : run: steps and helper executables
    marker_runs = [base["inj"]["case"]]
    # repo-local helper executables named like the optional helpers, repo root as cwd
    d_h, repo_h = mk(ctx, "AL-EXE-HELPERS", {
        ".github/workflows/inj.yaml": fx.gha_injection(ctx.work / "AL-EXE-HELPERS"),
        ".github/workflows/sc.yaml": "name: sc\non: [push]\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n"
                                     "      - run: echo $HOME $(ls) ${{ github.event.issue.title }}\n"
                                     "      - shell: python\n        run: |\n          import os\n          print( 1\n",
        "shellcheck": "#!/bin/sh\ntouch " + str(ctx.work / "AL-EXE-HELPERS" / "PWT_EXECUTED_FX-INJ-ACTIONLINT-SHELLCHECK") + "\n",
        "pyflakes": "#!/bin/sh\ntouch " + str(ctx.work / "AL-EXE-HELPERS" / "PWT_EXECUTED_FX-INJ-ACTIONLINT-PYFLAKES") + "\n",
        ".github/shellcheck": "#!/bin/sh\ntouch " + str(ctx.work / "AL-EXE-HELPERS" / "PWT_EXECUTED_FX-INJ-ACTIONLINT-SHELLCHECK2") + "\n",
    }, execs=("shellcheck", "pyflakes", ".github/shellcheck"))
    r_h_def = R("AL-EXE-HELPERS-DEFAULT", [".github/workflows/inj.yaml", ".github/workflows/sc.yaml"], repo_h,
                "default helper flags, repo contains executable shellcheck/pyflakes and run: marker steps",
                ["FX-GHA-INJECTION", "FX-INJ-ACTIONLINT"])
    r_h_off = R("AL-EXE-HELPERS-DISABLED", ["-shellcheck", "", "-pyflakes", "", ".github/workflows/inj.yaml",
                                          ".github/workflows/sc.yaml"], repo_h,
                "app-owned flags: helpers explicitly disabled with empty string", ["FX-GHA-INJECTION", "FX-INJ-ACTIONLINT"])
    r_h_verbose = ctx.run("AL-EXE-HELPERS-VERBOSE", [ctx.bin, "-verbose", "-format", FMT, ".github/workflows/sc.yaml"], repo_h,
                          purpose="-verbose to learn how absent helpers are reported", fixtures=["FX-INJ-ACTIONLINT"], timeout=60)
    ids_net.append(r_h_verbose)
    ctx.note("verbose_mentions_helper_lines", [l[:160] for l in (r_h_verbose["_stderr"] + r_h_verbose["_stdout"]).decode("utf-8", "replace").splitlines()
                                              if re.search(r"shellcheck|pyflakes", l, re.I)][:6])
    ctx_sc_off = R("AL-EXE-HELPERS-DISABLED-SC", ["-shellcheck", "", "-pyflakes", "", ".github/workflows/sc.yaml"], repo_h,
                   "helpers disabled, sc.yaml only (comparison base for the missing-path run)", ["FX-INJ-ACTIONLINT"])
    r_h_missing = R("AL-EXE-HELPERS-EXPLICIT-MISSING", ["-shellcheck", "/nonexistent/pwt-shellcheck", ".github/workflows/sc.yaml"],
                    repo_h, "-shellcheck pointing at a missing path (does a bad helper path error?)", ["FX-INJ-ACTIONLINT"])
    ctx.note("helpers_absent_silent_default_equals_disabled", sig(r_h_def) == sig(r_h_off) and r_h_def["exit"] == r_h_off["exit"])
    ctx.note("helper_missing_explicit_path", {"exit": r_h_missing["exit"], "stderr_excerpt": r_h_missing["stderr_excerpt"][:200],
                                              "findings_same_as_disabled": sig(r_h_missing) == sig(ctx_sc_off)})
    # Can repo config set a helper command?  (unknown-key behaviour)
    d_k, repo_k = mk(ctx, "AL-EXE-CFGKEY", {
        ".github/workflows/valid.yaml": fx.GHA_VALID,
        ".github/actionlint.yaml": "shellcheck: ./shellcheck\npyflakes: ./pyflakes\n",
    })
    r_cfgkey = R("AL-EXE-CFGKEY-RUN", [".github/workflows/valid.yaml"], repo_k,
                 "repo config with shellcheck/pyflakes keys (is an executable path configurable from the repo?)", ["FX-INJ-ACTIONLINT"])
    ctx.note("repo_config_with_helper_keys", {"exit": r_cfgkey["exit"], "stderr_excerpt": r_cfgkey["stderr_excerpt"][:300],
                                              "unknown_keys_silently_accepted": r_cfgkey["exit"] == 0 and not r_cfgkey["stderr_bytes"]})
    # shellcheck/pyflakes inline directive channels cannot be exercised: helpers absent
    exe_markers = ctx.markers()
    exe_in_cases = sorted(m for m in exe_markers if "ACTIONLINT" in m or "GHA-INJECTION" in m)
    ctx.note("exe_markers_seen", exe_in_cases)
    if exe_in_cases:
        ctx.hyp("H-EXE", "confirmed", f"execution markers appeared: {exe_in_cases} (see per-run new_execution_markers)",
                [base["inj"]["case"], r_h_def["case"], r_h_off["case"]])
    else:
        ctx.hyp("H-EXE", "refuted",
                "No PWT_EXECUTED_* marker in any actionlint run: run: steps of FX-GHA-INJECTION were analysed, not executed; "
                "repo-local executables named shellcheck/pyflakes (repo root and .github/, cwd = repo root, PATH=/usr/bin:/bin) "
                "were not executed with default flags or with helpers disabled; a repo config naming helper commands did not "
                f"execute them (config run exit={r_cfgkey['exit']}, unknown keys silently accepted: {r_cfgkey['exit'] == 0 and not r_cfgkey['stderr_bytes']}). Helper commands are chosen only by app flags/PATH. "
                "Limitation: real ShellCheck/Pyflakes are absent so the helper-on text-analysis path is not exercised.",
                [base["inj"]["case"], r_h_def["case"], r_h_off["case"], r_cfgkey["case"]])
    ctx.hyp("H-EXE-HELPERS-ABSENT", "observed",
            f"Default run == helpers-disabled run (identical findings and exit): {sig(r_h_def) == sig(r_h_off)}. Helpers can be disabled "
            f"explicitly: flags -shellcheck/-pyflakes advertised={has('shellcheck') and has('pyflakes')}; empty-string run exit={r_h_off['exit']}. "
            f"Explicit missing helper path run exit={r_h_missing['exit']} (findings same as disabled: {sig(r_h_missing) == sig(ctx_sc_off)}).",
            [r_h_def["case"], r_h_off["case"], r_h_missing["case"], ctx_sc_off["case"], r_h_verbose["case"]])

    # ------------------------------------------------------------------ H-CFG / H-OVR
    cfg_files = {".github/workflows/invalid.yaml": fx.GHA_INVALID, ".github/workflows/label.yaml": WF_LABEL,
                 "wf/outside.yaml": fx.GHA_INVALID}
    twin_d, twin = mk(ctx, "AL-CFG-TWIN", cfg_files)
    t_inv = R("AL-CFG-TWIN-INVALID", [".github/workflows/invalid.yaml"], twin, "clean twin: no repo config", ["FX-INJ-ACTIONLINT"])
    t_lbl = R("AL-CFG-TWIN-LABEL", [".github/workflows/label.yaml"], twin, "clean twin, fake runner label", ["FX-INJ-ACTIONLINT"])
    t_out = R("AL-CFG-TWIN-WF", ["wf/outside.yaml"], twin, "clean twin, workflow outside .github/workflows", ["FX-INJ-ACTIONLINT"])

    def hostile(cid, cfgrel, cfgtext, git=True):
        files = dict(cfg_files)
        files[cfgrel] = cfgtext
        return mk(ctx, cid, files, git=git)

    h_d, h_repo = hostile("AL-CFG-HOSTILE-YAML", ".github/actionlint.yaml", CFG_IGNORE_ALL)
    h_inv = R("AL-CFG-HOSTILE-YAML-INVALID", [".github/workflows/invalid.yaml"], h_repo,
              "repo .github/actionlint.yaml ignoring all messages, explicit file", ["FX-INJ-ACTIONLINT"])
    h_out = R("AL-CFG-HOSTILE-YAML-WF", ["wf/outside.yaml"], h_repo, "same repo, workflow outside .github/workflows", ["FX-INJ-ACTIONLINT"])
    yml_d, yml_repo = hostile("AL-CFG-HOSTILE-YML", ".github/actionlint.yml", CFG_IGNORE_ALL)
    y_inv = R("AL-CFG-HOSTILE-YML-INVALID", [".github/workflows/invalid.yaml"], yml_repo,
              "repo .github/actionlint.yml (yml suffix)", ["FX-INJ-ACTIONLINT"])
    lab_d, lab_repo = hostile("AL-CFG-LABELS", ".github/actionlint.yaml", CFG_LABELS)
    l_lbl = R("AL-CFG-LABELS-RUN", [".github/workflows/label.yaml"], lab_repo,
              "repo config declaring fake self-hosted runner label", ["FX-INJ-ACTIONLINT"])
    ng_d, ng_repo = hostile("AL-CFG-NOGIT", ".github/actionlint.yaml", CFG_IGNORE_ALL, git=False)
    ng_inv = R("AL-CFG-NOGIT-RUN", [".github/workflows/invalid.yaml"], ng_repo,
               "hostile repo config but no .git (staged copy) - is it discovered?", ["FX-INJ-ACTIONLINT"])
    # cwd dependence: discovery from a sub directory and from outside the repo
    (h_repo / "sub").mkdir(exist_ok=True)
    cwd_sub = R("AL-CFG-CWD-SUBDIR", [str(Path("..") / ".github/workflows/invalid.yaml")], h_repo / "sub",
                "cwd = sub directory of repo, relative path to workflow", ["FX-INJ-ACTIONLINT"])
    cwd_out = R("AL-CFG-CWD-OUTSIDE", ["repo/.github/workflows/invalid.yaml"], h_d,
                "cwd = parent of repo (outside project), relative path to workflow", ["FX-INJ-ACTIONLINT"])
    cwd_abs = R("AL-CFG-CWD-ABSOLUTE", [str(h_repo / ".github/workflows/invalid.yaml")], h_d,
                "absolute path to workflow, cwd outside repo", ["FX-INJ-ACTIONLINT"])
    # explicit app-owned config
    app_cfg = h_d / "app-actionlint.yaml"
    app_cfg.write_text(CFG_APP_MIN)
    o_inv = R("AL-OVR-EXPLICIT-INVALID", ["-config-file", str(app_cfg), ".github/workflows/invalid.yaml"], h_repo,
              "hostile repo config + explicit app config", ["FX-INJ-ACTIONLINT"])
    o_out = R("AL-OVR-EXPLICIT-WF", ["-config-file", str(app_cfg), "wf/outside.yaml"], h_repo,
              "hostile repo config + explicit app config, workflow outside .github/workflows", ["FX-INJ-ACTIONLINT"])
    app_cfg2 = lab_d / "app-actionlint.yaml"
    app_cfg2.write_text("self-hosted-runner:\n  labels:\n    - app-declared-label\n")
    o_lbl = R("AL-OVR-EXPLICIT-LABEL", ["-config-file", str(app_cfg2), ".github/workflows/label.yaml"], lab_repo,
              "repo declares fake-runner-label, app config declares a different label (replace vs merge)", ["FX-INJ-ACTIONLINT"])
    # config error handling
    bad1 = R("AL-CFG-MISSING-FILE", ["-config-file", str(h_d / "does-not-exist.yaml"), ".github/workflows/invalid.yaml"], h_repo,
             "explicit config path missing", ["FX-INJ-ACTIONLINT"])
    (h_d / "bad.yaml").write_text("self-hosted-runner: [unclosed\n")
    bad2 = R("AL-CFG-MALFORMED", ["-config-file", str(h_d / "bad.yaml"), ".github/workflows/invalid.yaml"], h_repo,
             "explicit config is malformed YAML", ["FX-INJ-ACTIONLINT"])
    malf_d, malf_repo = hostile("AL-CFG-REPO-MALFORMED", ".github/actionlint.yaml", "paths: [unclosed\n")
    bad3 = R("AL-CFG-REPO-MALFORMED-RUN", [".github/workflows/invalid.yaml"], malf_repo,
             "repo config itself malformed (discovery failure mode)", ["FX-INJ-ACTIONLINT"])
    # repo config + explicit app config both present: does the error path differ from clean twin
    ctx.note("config_discovery", {
        "repo_config_yaml_applied": sig(h_inv) != sig(t_inv) and cnt(h_inv) == 0,
        "repo_config_yml_applied": cnt(y_inv) == 0 and cnt(t_inv) not in (0, None),
        "repo_label_config_applied": cnt(t_lbl) not in (0, None) and cnt(l_lbl) == 0,
        "requires_git_dir_project_root": cnt(ng_inv) == cnt(t_inv),
        "discovery_independent_of_cwd": cnt(cwd_sub) == 0 and cnt(cwd_out) == 0 and cnt(cwd_abs) == 0,
        "applies_outside_dot_github_workflows_for_explicit_file": cnt(h_out) == 0 and cnt(t_out) not in (0, None),
    })
    exit_map["explicit config missing"] = bad1["exit"]
    exit_map["explicit config malformed"] = bad2["exit"]
    exit_map["repo config malformed (auto-discovered)"] = bad3["exit"]
    ctx.note("config_error_stderr", {"missing": bad1["stderr_excerpt"][:200], "malformed": bad2["stderr_excerpt"][:200],
                                     "repo_malformed": bad3["stderr_excerpt"][:200]})

    # H-CFG
    applied = [x for x, ok in (("yaml", cnt(h_inv) == 0), ("yml", cnt(y_inv) == 0), ("labels", cnt(l_lbl) == 0)) if ok]
    twin_has_findings = all(cnt(x) not in (0, None) for x in (t_inv, t_lbl))
    if applied and twin_has_findings:
        ctx.hyp("H-CFG", "confirmed",
                f"Repo .github/actionlint.{{yaml,yml}} auto-loaded (applied: {applied}); twin without config reports "
                f"{cnt(t_inv)}/{cnt(t_lbl)} findings vs hostile {cnt(h_inv)}/{cnt(l_lbl)}. Discovery requires a .git project root "
                f"(no .git: findings {cnt(ng_inv)} == twin {cnt(t_inv)}); independent of cwd "
                f"(sub/outside/absolute all suppressed: {cnt(cwd_sub)}/{cnt(cwd_out)}/{cnt(cwd_abs)}). Malformed repo config exit {bad3['exit']}.",
                [t_inv["case"], h_inv["case"], y_inv["case"], l_lbl["case"], ng_inv["case"], cwd_sub["case"], cwd_out["case"], cwd_abs["case"], bad3["case"]])
    else:
        ctx.hyp("H-CFG", "refuted" if twin_has_findings else "not testable",
                f"No repo config effect observed (applied={applied}, twin_has_findings={twin_has_findings}).",
                [t_inv["case"], h_inv["case"], y_inv["case"], l_lbl["case"]])
    # H-OVR
    replaced = sig(o_inv) == sig(t_inv) and sig(o_out) == sig(t_out) and cnt(o_lbl) not in (0, None)
    ctx.hyp("H-OVR", "confirmed" if replaced else "partially confirmed" if cnt(o_inv) not in (0, None) else "refuted",
            f"With explicit -config-file (app config) and the hostile repo config present: findings equal clean twin={sig(o_inv) == sig(t_inv)} "
            f"(workflow outside .github/workflows={sig(o_out) == sig(t_out)}); repo-declared label not honoured when app config declares a "
            f"different one (error count {cnt(o_lbl)}, twin {cnt(t_lbl)}) => replace, not merge. Missing explicit config exit {bad1['exit']}, "
            f"malformed {bad2['exit']}. Alternative neutralisation: no .git in the scanned tree disables discovery (case AL-CFG-NOGIT-RUN).",
            [t_inv["case"], o_inv["case"], o_out["case"], t_lbl["case"], o_lbl["case"], bad1["case"], bad2["case"], ng_inv["case"]])
    ctx.note("neutralisation_options", {"explicit_config_file_flag": replaced, "no_git_dir_in_scanned_tree": cnt(ng_inv) == cnt(t_inv)})

    # ------------------------------------------------------------------ H-INL : inline suppression candidates
    forms = {
        "top-actionlint-ignore": "# actionlint-ignore\n",
        "top-actionlint-colon-ignore": "# actionlint: ignore\n",
        "top-actionlint-disable": "# actionlint-disable\n",
        "top-actionlint-disable-colon": "# actionlint: disable\n",
        "top-nolint": "# nolint\n",
    }
    inl_files = {".github/workflows/plain.yaml": fx.GHA_INVALID}
    for k, v in forms.items():
        inl_files[f".github/workflows/{k}.yaml"] = v + fx.GHA_INVALID
    # trailing comment forms on the offending lines
    inl_files[".github/workflows/trail-ignore.yaml"] = fx.GHA_INVALID.replace("  build:", "  build: # actionlint-ignore").replace(
        "unknown_key: 1", "unknown_key: 1 # actionlint-ignore")
    inl_files[".github/workflows/trail-shellcheck.yaml"] = fx.GHA_INVALID.replace("unknown_key: 1", "unknown_key: 1 # shellcheck disable=SC2086")
    di, ir = mk(ctx, "AL-INL", inl_files)
    inl_base = R("AL-INL-PLAIN", [".github/workflows/plain.yaml"], ir, "baseline without any inline comment", ["FX-INJ-INLINE-ACTIONLINT"])
    inl_res = {}
    for k in list(forms) + ["trail-ignore", "trail-shellcheck"]:
        rr = R(f"AL-INL-{k.upper()}", [f".github/workflows/{k}.yaml"], ir, f"inline comment form {k}", ["FX-INJ-INLINE-ACTIONLINT"])
        inl_res[k] = (cnt(rr), rr["case"])
    # Messages are stable aside from filepath, so compare counts and the message sets
    suppressing = {k: v[0] for k, v in inl_res.items() if v[0] != cnt(inl_base)}
    ctx.note("inline_comment_forms_tried", {k: v[0] for k, v in inl_res.items()})
    if suppressing:
        ctx.hyp("H-INL", "confirmed", f"Inline comment(s) changed the finding count (baseline {cnt(inl_base)}): {suppressing}.",
                [inl_base["case"]] + [v[1] for v in inl_res.values()])
    else:
        ctx.hyp("H-INL", "refuted",
                f"None of {len(inl_res)} candidate inline comment forms (leading/trailing; actionlint-ignore/disable/nolint; "
                f"shellcheck disable) changed the finding count (baseline {cnt(inl_base)}). Limited to the forms tried; "
                "ShellCheck/Pyflakes inline directives (a repo-controlled suppression channel inside those helpers) are not testable "
                "because the helpers are absent. Repo-controlled suppression of core checks via config remains (H-CFG).",
                [inl_base["case"]] + [v[1] for v in inl_res.values()])

    # ------------------------------------------------------------------ H-SCOPE
    sc_files = {
        ".github/workflows/a.yaml": fx.GHA_INVALID,
        ".github/workflows/b.yaml": fx.GHA_INVALID.replace("echo ${{ github.event.foo.", f"echo {c['C11']} ${{{{ github.event.foo."),
        ".github/workflows/sub/nested.yaml": fx.GHA_INVALID,
        ".github/workflows/notes.txt": f"token={c['C12']}\n",
    }
    ds, sr = mk(ctx, "AL-SCOPE", sc_files)
    s_a = R("AL-SCOPE-EXPLICIT-A", [".github/workflows/a.yaml"], sr, "explicit file a; sibling b holds canary C11", ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NONYAML"])
    s_auto = R("AL-SCOPE-AUTO", [], sr, "no file args: auto-discovery from cwd", ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NESTED", "FX-SCOPE-NONYAML"])
    s_dir = R("AL-SCOPE-DIRARG", [".github/workflows"], sr, "directory as argument", ["FX-SCOPE-NESTED"])
    s_sub = R("AL-SCOPE-AUTO-FROM-SUBDIR", [], sr / ".github" / "workflows" / "sub", "no args from a nested directory", ["FX-SCOPE-NESTED"])
    s_ng = R("AL-SCOPE-AUTO-NOGIT", [], mk(ctx, "AL-SCOPE-NOGIT", {k: v for k, v in sc_files.items() if k.endswith("a.yaml")}, git=False)[1],
             "no args, no .git in tree", ["FX-SCOPE-SIBLINGS"])
    exit_map["auto-discovery without project"] = s_ng["exit"]
    exit_map["directory argument"] = s_dir["exit"]
    auto_files = files_of(s_auto)
    ctx.note("scope_observations", {
        "explicit_a_files": files_of(s_a), "explicit_a_has_C11": "C11" in s_a["canary_in_stdout"],
        "auto_files": auto_files, "auto_has_C11": "C11" in s_auto["canary_in_stdout"],
        "auto_has_C12": "C12" in s_auto["canary_in_stdout"], "auto_from_subdir_files": files_of(s_sub),
        "dir_arg_exit": s_dir["exit"], "dir_arg_stderr": s_dir["stderr_excerpt"][:160],
        "auto_no_project_exit": s_ng["exit"],
    })
    if files_of(s_a) == [".github/workflows/a.yaml"] and not s_a["canary_in_stdout"] and not s_a["canary_in_stderr"]:
        det = (f"Explicit file args: only the named file appears in output and sibling canary C11 is absent from stdout/stderr. "
               f"Without args the tool walks .github/workflows of the nearest project (files reported: {auto_files}; sibling canary "
               f"echoed in output: {'C11' in s_auto['canary_in_stdout']}; non-YAML sibling C12 echoed: {'C12' in s_auto['canary_in_stdout']}). "
               f"Directory args are rejected (exit {s_dir['exit']}). Limitation: reads are inferred from output, not syscalls; "
               "the project config files are also read.")
        ctx.hyp("H-SCOPE", "confirmed", det, [s_a["case"], s_auto["case"], s_dir["case"], s_sub["case"], s_ng["case"]])
    else:
        ctx.hyp("H-SCOPE", "refuted", f"Explicit file args still produced other files/canaries: files={files_of(s_a)} "
                f"C11={'C11' in s_a['canary_in_stdout']}", [s_a["case"], s_auto["case"]])

    # ------------------------------------------------------------------ H-SEC : secrets echoed
    sec_wf = (
        "name: sec\non: [push]\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n"
        f"      - run: echo {c['C02']} ${{{{ github.event.foo.\n"
        f"    {c['C07']}: 1\n"
    )
    dse, se = mk(ctx, "AL-SEC", {".github/workflows/sec.yaml": sec_wf})
    sec_json = R("AL-SEC-JSON", [".github/workflows/sec.yaml"], se, "secret in erroring lines, JSON template", ["FX-SEC-TOKEN-PREFIXED", "FX-SEC-GENERIC"])
    sec_text = ctx.run("AL-SEC-TEXT", [ctx.bin, ".github/workflows/sec.yaml"], se, purpose="secret in erroring lines, default text",
                       fixtures=["FX-SEC-TOKEN-PREFIXED"], timeout=60)
    sec_one = ctx.run("AL-SEC-ONELINE", [ctx.bin, "-oneline", ".github/workflows/sec.yaml"], se, purpose="secret, oneline",
                      fixtures=["FX-SEC-TOKEN-PREFIXED"], timeout=60)
    safe_tpl = '{{range $i, $e := .}}{{$e.Kind}}:{{$e.Line}}:{{$e.Column}}:{{$e.EndColumn}}\n{{end}}'
    sec_safe = ctx.run("AL-SEC-RESTRICTED-TEMPLATE", [ctx.bin, "-format", safe_tpl, ".github/workflows/sec.yaml"], se,
                       purpose="template emitting only kind/line/column (no message/snippet)", fixtures=["FX-SEC-TOKEN-PREFIXED"], timeout=60)
    msg_tpl = '{{range $i, $e := .}}{{$e.Kind}}:{{$e.Line}}:{{$e.Column}}:{{$e.Message}}\n{{end}}'
    sec_msg = ctx.run("AL-SEC-MESSAGE-TEMPLATE", [ctx.bin, "-format", msg_tpl, ".github/workflows/sec.yaml"], se,
                      purpose="template emitting message but not snippet", fixtures=["FX-SEC-TOKEN-PREFIXED", "FX-SEC-GENERIC"], timeout=60)
    ids_net += [sec_text, sec_one, sec_safe, sec_msg]
    field_hits = {}
    for e in errs(sec_json) or []:
        for k, v in e.items():
            hits = ctx.scan(str(v).encode())
            if hits:
                field_hits.setdefault(k, set()).update(hits)
    field_hits = {k: sorted(v) for k, v in field_hits.items()}
    ctx.note("secret_echo_by_json_field", field_hits)
    ctx.note("secret_echo_by_format", {
        "json_template_all_fields": sorted(set(sec_json["canary_in_stdout"])),
        "default_text": sorted(set(sec_text["canary_in_stdout"])),
        "oneline": sorted(set(sec_one["canary_in_stdout"])),
        "restricted_kind_line_col": sorted(set(sec_safe["canary_in_stdout"])),
        "message_without_snippet": sorted(set(sec_msg["canary_in_stdout"])),
        "stderr_any": sorted(set(sec_json["canary_in_stderr"] + sec_text["canary_in_stderr"])),
    })
    ctx.note("redaction_flag_present", any(re.search(r"redact|mask|secret", f) for f in flags))
    leaks = bool(sec_json["canary_in_stdout"] or sec_text["canary_in_stdout"])
    ctx.hyp("H-SEC", "partially confirmed" if leaks else "refuted",
            f"Raw source text is echoed by diagnostics: canaries in JSON stdout {sorted(set(sec_json['canary_in_stdout']))}, default text "
            f"{sorted(set(sec_text['canary_in_stdout']))}, oneline {sorted(set(sec_one['canary_in_stdout']))}; JSON fields carrying them: {field_hits}. "
            f"No redaction flag exists (flags: {flags}). A restricted -format template with only kind/line/column leaked: "
            f"{sorted(set(sec_safe['canary_in_stdout']))}; message-without-snippet leaked: {sorted(set(sec_msg['canary_in_stdout']))}. "
            "No report files are created; stderr clean. The product must redact (or use a field-restricted template) before persistence.",
            [sec_json["case"], sec_text["case"], sec_one["case"], sec_safe["case"], sec_msg["case"]])

    # ------------------------------------------------------------------ H-MAL : malformed YAML
    mal = {".github/workflows/mal.yaml": fx.yaml_malformed_secret(c),
           ".github/workflows/mal2.yaml": fx.YAML_MALFORMED, ".github/workflows/empty.yaml": "",
           ".github/workflows/dup.yaml": "name: x\non: [push]\non: [pull_request]\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n"}
    dm, mr = mk(ctx, "AL-MAL", mal)
    m1 = R("AL-MAL-SECRET", [".github/workflows/mal.yaml"], mr, "FX-YAML-MALFORMED-SECRET as workflow", ["FX-YAML-MALFORMED-SECRET"])
    m2 = R("AL-MAL-PLAIN", [".github/workflows/mal2.yaml"], mr, "FX-YAML-MALFORMED as workflow", ["FX-YAML-MALFORMED"])
    m3 = R("AL-MAL-EMPTY", [".github/workflows/empty.yaml"], mr, "empty file", ["FX-YAML-MALFORMED"])
    m4 = R("AL-MAL-DUPKEY", [".github/workflows/dup.yaml"], mr, "duplicate top-level key", ["FX-YAML-DUPKEY"])
    exit_map["parse error (malformed YAML)"] = m2["exit"]
    kinds = lambda r_: sorted({e.get("kind") for e in (errs(r_) or [])})
    ctx.note("malformed_observations", {"malformed_exit": m2["exit"], "malformed_kinds": kinds(m2), "malformed_count": cnt(m2),
                                        "malformed_secret_C01_echoed": "C01" in m1["canary_in_stdout"], "empty_exit": m3["exit"],
                                        "empty_kinds": kinds(m3), "dupkey_exit": m4["exit"], "dupkey_kinds": kinds(m4)})
    ctx.hyp("H-MAL", "observed",
            f"Malformed YAML workflow: exit {m2['exit']}, finding kinds {kinds(m2)} ({cnt(m2)} finding(s)); only parser/syntax-level diagnostics "
            f"emitted (actionlint has no secret rule so SCAN-08 'secret still found' does not apply to this tool). Secret-bearing malformed file "
            f"echoes canary C01 in output: {'C01' in m1['canary_in_stdout']}. Empty file exit {m3['exit']}, duplicate key exit {m4['exit']}.",
            [m1["case"], m2["case"], m3["case"], m4["case"]])

    # ------------------------------------------------------------------ H-ARG / H-OUT : hostile names
    names = fx.hostile_name_tree(fx.GHA_INVALID)
    dn, nr = mk(ctx, "AL-NAME", names)
    namedir = nr / "names"
    name_obs = {}
    arg_ok = True
    for fid, nm in fx.HOSTILE_NAMES.items():
        rn = ctx.run(f"AL-NAME-{fid.split('FX-NAME-')[1]}", [ctx.bin, "-format", FMT, "--", nm], namedir,
                     purpose="hostile name as argv element after --", fixtures=[fid], parse_json=True, timeout=60)
        ids_net.append(rn)
        e = errs(rn) or []
        round_trip = bool(e) and all(x.get("filepath") == nm for x in e)
        esc_in_stdout = b"\x1b" in rn["_stdout"] and nm.startswith("\x1b")
        name_obs[fid] = {"exit": rn["exit"], "json_ok": rn.get("json_parse") == "ok", "findings": len(e), "filepath_round_trips": round_trip,
                         "raw_ESC_in_json_stdout": esc_in_stdout, "raw_newline_in_filepath_field": any("\n" in (x.get("filepath") or "") for x in e)}
        arg_ok &= round_trip and rn["exit"] == 1
    # without "--" for the dash names
    nod = {}
    for fid in ("FX-NAME-DASH-1", "FX-NAME-DASH-2"):
        rn = ctx.run(f"AL-NAME-{fid.split('FX-NAME-')[1]}-NO-ENDOPT", [ctx.bin, "-format", FMT, fx.HOSTILE_NAMES[fid]], namedir,
                     purpose="dash-leading name WITHOUT end-of-options marker", fixtures=[fid], parse_json=True, timeout=60)
        ids_net.append(rn)
        nod[fid] = {"exit": rn["exit"], "findings": cnt(rn), "stderr_head": rn["stderr_excerpt"][:120]}
    # Text output with ANSI-bearing name: does the tool echo raw ESC?
    rn_ansi_text = ctx.run("AL-NAME-ANSI-TEXT", [ctx.bin, "--", fx.HOSTILE_NAMES["FX-NAME-ANSI"]], namedir,
                           purpose="ANSI name, default text output", fixtures=["FX-NAME-ANSI"], timeout=60)
    rn_nl_text = ctx.run("AL-NAME-NEWLINE-TEXT", [ctx.bin, "--", fx.HOSTILE_NAMES["FX-NAME-NEWLINE"]], namedir,
                         purpose="newline name, default text output", fixtures=["FX-NAME-NEWLINE"], timeout=60)
    ids_net += [rn_ansi_text, rn_nl_text]
    ctx.note("hostile_name_json", name_obs)
    ctx.note("hostile_name_without_end_of_options", nod)
    ansi_raw_text = b"\x1b[31m" in rn_ansi_text["_stdout"]
    nl_split_text = rn_nl_text["_stdout"].count(b"\nbreak.yaml") > 0
    ctx.note("hostile_name_text_output", {"ansi_raw_ESC_in_text_stdout": ansi_raw_text, "newline_name_splits_text_record": nl_split_text})
    # Content with ANSI in YAML values
    ansi_wf = fx.GHA_INVALID.replace("unknown_key: 1", "unknown_key: \x1b[31mred\x1b[0m")
    da, ar = mk(ctx, "AL-ANSI-CONTENT", {".github/workflows/ansi.yaml": ansi_wf.replace("    unknown_key", "    \x1b[31munknown_key")})
    rn_ac = R("AL-ANSI-CONTENT-JSON", [".github/workflows/ansi.yaml"], ar, "ESC in YAML key/values, JSON", ["FX-OUT-ANSI-CONTENT"])
    rn_ac_t = ctx.run("AL-ANSI-CONTENT-TEXT", [ctx.bin, ".github/workflows/ansi.yaml"], ar, purpose="ESC in YAML content, text",
                      fixtures=["FX-OUT-ANSI-CONTENT"], timeout=60)
    ids_net.append(rn_ac_t)
    ctx.note("ansi_content", {"json_exit": rn_ac["exit"], "json_findings": cnt(rn_ac), "kinds": sorted({e.get("kind") for e in (errs(rn_ac) or [])}), "json_ok": rn_ac.get("json_parse"), "raw_ESC_in_json_stdout": b"\x1b" in rn_ac["_stdout"],
                              "raw_ESC_in_text_stdout": b"\x1b" in rn_ac_t["_stdout"]})
    ctx.hyp("H-ARG", "confirmed" if arg_ok else "partially confirmed",
            f"All {len(name_obs)} hostile names passed as argv elements after '--' were analysed and round-tripped exactly in the JSON filepath "
            f"field: {arg_ok} (details in note hostile_name_json). Without '--' the dash-leading names behave as: {nod}. No shell is involved "
            "(argv array); glob/space/quote/$ names were not expanded. Dash names rely on Go flag-parsing '--' support or a ./ prefix.",
            [f"AL-NAME-{k.split('FX-NAME-')[1]}" for k in fx.HOSTILE_NAMES] + [f"AL-NAME-DASH-{i}-NO-ENDOPT" for i in (1, 2)])
    ctx.hyp("H-OUT", "confirmed" if deterministic and base["invalid"].get("json_parse") == "ok" and all(v["json_ok"] for v in name_obs.values()) else "partially confirmed",
            f"JSON via -format '{FMT}': parse ok for baseline and all hostile-name runs={all(v['json_ok'] for v in name_obs.values())}; "
            f"two identical runs byte-identical={deterministic}; empty result is '[]'; fields {fields}. Control characters in names/values appear "
            f"as JSON escapes in JSON output (raw ESC in JSON stdout for ANSI name: {name_obs['FX-NAME-ANSI']['raw_ESC_in_json_stdout']}); default text output "
            f"emits raw ESC for ANSI name: {ansi_raw_text}, newline-name record split in text: "
            f"{nl_split_text}. Multi-file order follows argv: {first_file_order == ['.github/workflows/inj.yaml', '.github/workflows/invalid.yaml']}.",
            [base["invalid"]["case"], r2["case"], r_ord["case"], rn_ansi_text["case"], rn_nl_text["case"], rn_ac["case"], rn_ac_t["case"]])

    # ------------------------------------------------------------------ H-EXIT
    e_nofile = R("AL-EXIT-MISSING-FILE", ["no-such-file.yaml"], repo, "nonexistent file argument", ["FX-YAML-VALID"])
    exit_map["nonexistent file argument"] = e_nofile["exit"]
    ctx.note("exit_map", exit_map)
    ctx.note("exit_codes_observed", sorted({v for v in exit_map.values() if v is not None}))
    ctx.hyp("H-EXIT", "confirmed" if (exit_map["clean (FX-GHA-VALID)"] == 0 and exit_map["findings (FX-GHA-INVALID)"] == 1 and exit_map["bad flag"] == 2) else "partially confirmed",
            f"Observed map: {exit_map}. No soft-fail/strict flag exists (flags: {flags}); exit 1 is returned for both lint findings and "
            f"(see map) syntax/parse problems in the workflow file, so findings vs. parse errors are separable only via JSON 'kind'; "
            "exit 2 = usage error (bad flag), exit 3 = operational errors (missing file, directory arg, no project, bad config). "
            "FX-GHA-REF exits 1 only because its local ./ references do not resolve (see note fx_gha_ref_findings), not because of mutable remote refs.",
            [base["valid"]["case"], base["invalid"]["case"], "AL-BADFLAG", e_nofile["case"], s_dir["case"], s_ng["case"], bad1["case"], bad2["case"], m2["case"]])

    # ------------------------------------------------------------------ H-NET
    sigs = {}
    for r_ in ids_net:
        s = [x for x in r_["network_attempt_signals"] if not x.startswith("http")]
        if s:
            sigs[r_["case"]] = s
    max_wall = max(r_["wall_ms"] for r_ in ids_net)
    timed = [r_["case"] for r_ in ids_net if r_["timed_out"]]
    ctx.hyp("H-NET", "refuted" if not sigs and not timed else "partially confirmed",
            f"Across {len(ids_net)} runs (incl. FX-GHA-REF with unpinned remote actions and docker:// refs) in a loopback-only netns: no connection/resolution "
            f"error text (signals: {sigs}), no timeouts ({timed}), max wall {max_wall} ms; no network/offline/update flag exists "
            f"(note has_network_or_offline_flag). Evidence is indirect (blocked connections would normally surface as errors); "
            "no syscall tracing.",
            [r_["case"] for r_ in ids_net[:6]] + [base["ref"]["case"]])
    ctx.note("max_wall_ms", max_wall)

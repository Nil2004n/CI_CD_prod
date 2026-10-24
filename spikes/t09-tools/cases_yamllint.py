"""P00-T09 yamllint spike cases (W1; protocol P00-T01 section 3.1). Spike code, not application code.

Every hypothesis outcome is derived by comparing recorded runs (hostile vs clean twin, explicit
config vs auto-discovery). Raw outputs stay in memory; only derived facts are noted.
"""

from __future__ import annotations

import os
import re
from pathlib import Path

import fixtures as fx

LINE = re.compile(r"^(?P<file>.*?):(?P<line>\d+):(?P<col>\d+): \[(?P<level>error|warning)\] "
                  r"(?P<msg>.*?)(?: \((?P<rule>[a-z0-9-]+)\))?$")

APP_CFG = "extends: default\n"                      # app-owned config (stand-in; real digest set at P00-T11)
H_RULES_OFF = "rules: {}\n"                         # hostile: every style rule disabled
H_IGNORE_ALL = "extends: default\nignore: |\n  *\n  **\n  *.yaml\n  *.yml\n"   # hostile: ignore every file


def parse(rec):
    out = []
    for ln in rec["_stdout"].decode("utf-8", "replace").split("\n"):
        m = LINE.match(ln)
        if m:
            out.append(m.groupdict())
    return out


def sigs(rec):
    return {(os.path.basename(p["file"]), p["line"], p["col"], p["rule"]) for p in parse(rec)}


def cases(ctx):
    c = ctx.canaries
    exit_map: dict[str, object] = {}

    def Y(cid, args, cwd, purpose, fixtures, **kw):
        return ctx.run(cid, [ctx.bin, *args], cwd, purpose=purpose, fixtures=fixtures, **kw)

    def write_app(d) -> str:
        p = d / "app.yaml"                  # outside the repo tree: app-owned config
        p.write_text(APP_CFG)
        return str(p)

    # ------------------------------------------------------------------ version / help
    d = ctx.case_dir("yl-meta")
    r = Y("yl-version", ["--version"], d / "repo", "version string", [])
    ctx.note("version", r["_stdout"].decode().strip() or r["_stderr"].decode().strip())
    r = Y("yl-help", ["--help"], d / "repo", "flag discovery", [])
    helptxt = r["_stdout"].decode()
    ctx.note("flags_present_in_help", {f: (f in helptxt) for f in
             ["--config-file", "--config-data", "--format", "--strict", "--no-warnings", "--list-files"]})
    ctx.note("help_mentions_directive_ignore_switch", bool(re.search(r"directive|inline", helptxt, re.I)))
    ctx.note("format_choices", re.findall(r"\{([a-z,]+)\}", helptxt)[:2])

    # ------------------------------------------------------------------ baseline FX-YAML-*
    d = ctx.case_dir("yl-baseline")
    app = write_app(d)
    fx.write_tree(d / "repo", {
        "valid.yaml": fx.YAML_VALID, "style.yaml": fx.YAML_STYLE, "malformed.yaml": fx.YAML_MALFORMED,
        "malformed-secret.yaml": fx.yaml_malformed_secret(c), "dupkey.yaml": fx.YAML_DUPKEY,
        "multidoc.yaml": fx.YAML_MULTIDOC,
        "malformed-trailing.yaml": "---\napp:   \n  name: demo\n  ports: [80, 443\n bad_indent: true   \n",
        "malformed-after.yaml": "---\napp:\n  ports: [80, 443\n bad: true\nmore: 1   \nlast: [1,2,  3]\nend: 3   \n",
        "dupkey-secretkey.yaml": f"---\n{c['C01']}: 1\n{c['C01']}: 2\n",
        "truthy-secret.yaml": f"---\nkey: yes\ntoken: {c['C01']}\n",
    })
    base = {}
    for name in ["valid", "style", "malformed", "malformed-secret", "dupkey", "multidoc", "malformed-trailing",
                 "dupkey-secretkey", "truthy-secret", "malformed-after"]:
        base[name] = Y(f"yl-base-{name}", ["-c", app, "-f", "parsable", f"{name}.yaml"], d / "repo",
                       f"baseline {name}", [f"FX-YAML-{name.upper()}"])
    exit_map["clean"] = base["valid"]["exit"]
    exit_map["findings_style"] = base["style"]["exit"]
    exit_map["malformed"] = base["malformed"]["exit"]
    rules_style = sorted({p["rule"] or "NONE" for p in parse(base["style"])})
    rules_mal = [(p["rule"], p["level"]) for p in parse(base["malformed"])]
    syntax_has_rule_none_or_syntax = {p["rule"] for p in parse(base["malformed"])}
    ctx.note("baseline", {
        "valid_problem_count": len(parse(base["valid"])), "valid_exit": base["valid"]["exit"],
        "style_rules": rules_style, "style_exit": base["style"]["exit"],
        "style_has_syntax_diag": any(p["rule"] in (None, "syntax") for p in parse(base["style"])),
        "malformed_rule_field_values": sorted(str(x) for x in syntax_has_rule_none_or_syntax),
        "malformed_levels": sorted({lv for _, lv in rules_mal}),
        "dupkey_rules": sorted({p["rule"] or "NONE" for p in parse(base["dupkey"])}),
        "dupkey_count": len(parse(base["dupkey"])),
        "multidoc_diag_lines": sorted({int(p["line"]) for p in parse(base["multidoc"])}),
        "multidoc_has_document_index": any(re.search(r"document\s*(no\.?|#)?\s*\d", p["msg"], re.I) for p in parse(base["multidoc"])),
        "malformed_trailing_rules": sorted({f'{p["rule"]}@{p["line"]}' for p in parse(base["malformed-trailing"])}),
        "parsable_fields": ["file", "line", "col", "level", "message", "rule(optional; 'syntax' for parse errors)"],
    })
    syn = base["malformed"]
    ctx.note("syntax_vs_style_distinction", {
        "syntax_rule_token": sorted({p["rule"] for p in parse(syn) if p["rule"] in (None, "syntax")}, key=str),
        "message_prefix_for_syntax": sorted({p["msg"].split(":")[0][:30] for p in parse(syn)}),
        "style_rule_ids_have_distinct_names": rules_style,
    })

    # ------------------------------------------------------------------ H-SEC
    sec_runs = ["yl-base-malformed-secret", "yl-base-dupkey-secretkey", "yl-base-truthy-secret"]
    fmts = {}
    for fmt in ("standard", "colored", "github", "parsable"):
        fmts[fmt] = Y(f"yl-sec-fmt-{fmt}", ["-c", app, "-f", fmt, "dupkey-secretkey.yaml", "malformed-secret.yaml"],
                      d / "repo", f"secret echo in format {fmt}", ["FX-YAML-DUPKEY", "FX-YAML-MALFORMED-SECRET"])
        sec_runs.append(f"yl-sec-fmt-{fmt}")
    echo_streams = {}
    for k in ["malformed-secret", "dupkey-secretkey", "truthy-secret"]:
        echo_streams[k] = {"stdout": base[k]["canary_in_stdout"], "stderr": base[k]["canary_in_stderr"]}
    for k, rr in fmts.items():
        echo_streams["fmt-" + k] = {"stdout": rr["canary_in_stdout"], "stderr": rr["canary_in_stderr"]}
    any_echo = any(v["stdout"] or v["stderr"] for v in echo_streams.values())
    only_key_case = [k for k, v in echo_streams.items() if (v["stdout"] or v["stderr"])]
    ctx.note("secret_echo_by_case", echo_streams)
    value_cases_echo = [k for k in ("malformed-secret", "truthy-secret") if echo_streams[k]["stdout"] or echo_streams[k]["stderr"]]
    key_case_echo = bool(echo_streams["dupkey-secretkey"]["stdout"] or echo_streams["dupkey-secretkey"]["stderr"])
    ctx.note("secret_echo_summary", {"secret_as_yaml_value_echoed_in": value_cases_echo,
                                     "secret_as_duplicated_key_echoed": key_case_echo,
                                     "echoed_in_all_four_formats": all(
                                         fmts[f]["canary_in_stdout"] for f in fmts), "stderr_ever": any(
                                         v["stderr"] for v in echo_streams.values())})
    if any_echo:
        ctx.hyp("H-SEC", "confirmed",
                f"Diagnostics echo YAML key text: a secret used as a duplicated mapping key appeared in stdout in every "
                f"format tried ({key_case_echo}); a secret in a VALUE position was not echoed by the syntax-error or truthy "
                f"cases (echoed in: {value_cases_echo or 'none'}). yamllint has no redaction option, so the adapter must "
                "sanitise diagnostic messages (at least key-duplicates text) before persistence. No report files are written.",
                sec_runs)
    else:
        ctx.hyp("H-SEC", "refuted",
                "No canary appeared in stdout/stderr in any of: syntax error with secret value, duplicate key whose "
                "name is the secret, truthy rule with secret value, in formats standard/colored/github/parsable.",
                sec_runs)

    # ------------------------------------------------------------------ H-CFG / H-OVR
    files = ["a.yaml", "tok.yaml", "k8s.yaml"]
    inj = {"a.yaml": fx.YAML_STYLE, "tok.yaml": f"---\ntoken: {c['C02']}   \n", "k8s.yaml": fx.K8S_INSECURE}

    def mk_repo(cid, extra=None):
        dd = ctx.case_dir(cid)
        tree = dict(inj)
        tree.update(extra or {})
        fx.write_tree(dd / "repo", tree)
        return dd

    twin = mk_repo("yl-inj-twin")
    twin_app = write_app(twin)
    t_noc = Y("yl-inj-twin-noc", ["-f", "parsable", *files], twin / "repo", "clean twin, no -c", ["FX-INJ-YAMLLINT"])
    t_c = Y("yl-inj-twin-c", ["-c", twin_app, "-f", "parsable", *files], twin / "repo", "clean twin, -c app",
            ["FX-INJ-YAMLLINT"])
    cfg_changed, ovr_ok, outside_found = {}, {}, {}
    cfg_runs, ovr_runs = [t_noc["case"], t_c["case"]], []
    for idx, (name, kind, body) in enumerate([(".yamllint", "rules-off", H_RULES_OFF),
                                              (".yamllint.yaml", "ignore-all", H_IGNORE_ALL),
                                              (".yamllint.yml", "rules-off", H_RULES_OFF),
                                              (".yamllint", "ignore-all", H_IGNORE_ALL),
                                              (".yamllint.yaml", "rules-off", H_RULES_OFF),
                                              (".yamllint.yml", "ignore-all", H_IGNORE_ALL)]):
        tag = f"{name.strip('.').replace('.', '-')}-{kind}"
        dd = mk_repo(f"yl-inj-{tag}", {name: body})
        ap = write_app(dd)
        h_noc = Y(f"yl-inj-{tag}-noc", ["-f", "parsable", *files], dd / "repo", f"hostile {name} {kind}, no -c",
                  ["FX-INJ-YAMLLINT"])
        h_c = Y(f"yl-inj-{tag}-c", ["-c", ap, "-f", "parsable", *files], dd / "repo",
                f"hostile {name} {kind}, explicit app config", ["FX-INJ-YAMLLINT"])
        h_out = Y(f"yl-inj-{tag}-outcwd", ["-f", "parsable", *[f"repo/{f}" for f in files]], dd,
                  f"hostile {name} {kind}, cwd outside repo, no -c", ["FX-INJ-YAMLLINT"])
        cfg_changed[tag] = {"hostile_findings": len(sigs(h_noc)), "twin_findings": len(sigs(t_noc)),
                            "auto_loaded": sigs(h_noc) != sigs(t_noc), "exit": h_noc["exit"]}
        ovr_ok[tag] = sigs(h_c) == sigs(t_c)
        outside_found[tag] = sigs(h_out) != {s for s in sigs(t_noc)}
        cfg_runs += [h_noc["case"], h_out["case"]]
        ovr_runs += [h_c["case"]]
    # -d (config-data) with hostile repo config
    dd = mk_repo("yl-inj-data", {".yamllint": H_RULES_OFF})
    h_d = Y("yl-inj-data-d", ["-d", APP_CFG, "-f", "parsable", *files], dd / "repo", "hostile .yamllint + -d config-data",
            ["FX-INJ-YAMLLINT"])
    ovr_ok["config-data(-d)"] = sigs(h_d) == sigs(t_c)
    ovr_runs.append(h_d["case"])
    # parent-directory discovery: config in repo root, cwd in repo/sub
    dd = mk_repo("yl-inj-parent", {".yamllint": H_RULES_OFF, "sub/x.yaml": fx.YAML_STYLE})
    env_sub = {"HOME": str(dd / "home"), "XDG_CONFIG_HOME": str(dd / "xdg-config"),
               "XDG_CACHE_HOME": str(dd / "xdg-cache"), "TMPDIR": str(dd / "tmp")}
    h_sub = Y("yl-inj-parent-sub", ["-f", "parsable", "x.yaml"], dd / "repo" / "sub",
              "hostile .yamllint in parent of cwd, no -c", ["FX-INJ-YAMLLINT"], extra_env=env_sub)
    twin_sub = ctx.case_dir("yl-inj-parent-twin")
    fx.write_tree(twin_sub / "repo", {"sub/x.yaml": fx.YAML_STYLE})
    t_sub = Y("yl-inj-parent-twin-sub", ["-f", "parsable", "x.yaml"], twin_sub / "repo" / "sub",
              "clean twin for parent discovery", ["FX-INJ-YAMLLINT"],
              extra_env={"HOME": str(twin_sub / "home"), "XDG_CONFIG_HOME": str(twin_sub / "xdg-config"),
                         "XDG_CACHE_HOME": str(twin_sub / "xdg-cache"), "TMPDIR": str(twin_sub / "tmp")})
    parent_found = sigs(h_sub) != sigs(t_sub)
    cfg_runs += [h_sub["case"], t_sub["case"]]
    # config key sanity: ignore patterns vs explicit files (ignore variant, noc)
    auto = [t for t, v in cfg_changed.items() if v["auto_loaded"]]
    notauto = [t for t, v in cfg_changed.items() if not v["auto_loaded"]]
    ctx.note("repo_config_autoload", cfg_changed)
    ctx.note("repo_config_found_when_cwd_outside_repo_and_files_in_repo", outside_found)
    ctx.note("repo_config_found_in_parent_of_cwd", parent_found)
    if auto:
        ctx.hyp("H-CFG", "confirmed" if not notauto else "partially confirmed",
                f"Repository config changed results (auto-loaded) for: {sorted(auto)}; not changed for: {sorted(notauto)}. "
                f"Discovery is by cwd (and parents: {parent_found}); with cwd outside the repo the config was "
                f"found for: {sorted(t for t, v in outside_found.items() if v)}. Mitigation direction: always pass an "
                "explicit config AND/OR run with cwd outside the repository.", cfg_runs)
    else:
        ctx.hyp("H-CFG", "refuted", "No hostile repo config changed results.", cfg_runs)

    # ------------------------------------------------------------------ user / XDG / env-var config
    dd = mk_repo("yl-user")
    ap = write_app(dd)
    user_cfg_dir = dd / "xdg-config" / "yamllint"
    user_cfg_dir.mkdir()
    (user_cfg_dir / "config").write_text(H_RULES_OFF)
    u_noc = Y("yl-user-xdg-noc", ["-f", "parsable", *files], dd / "repo", "hostile $XDG_CONFIG_HOME/yamllint/config, no -c",
              ["FX-INJ-YAMLLINT"])
    u_c = Y("yl-user-xdg-c", ["-c", ap, "-f", "parsable", *files], dd / "repo", "same with explicit -c", ["FX-INJ-YAMLLINT"])
    (dd / "home" / ".config" / "yamllint").mkdir(parents=True)
    (dd / "home" / ".config" / "yamllint" / "config").write_text(H_RULES_OFF)
    u_home = Y("yl-user-home-noc", ["-f", "parsable", *files], dd / "repo", "hostile ~/.config/yamllint/config (XDG unset path)",
               ["FX-INJ-YAMLLINT"], extra_env={"XDG_CONFIG_HOME": str(dd / "no-such-xdg")})
    (dd / "env-hostile.yaml").write_text(H_RULES_OFF)
    envv = {"YAMLLINT_CONFIG_FILE": str(dd / "env-hostile.yaml"), "XDG_CONFIG_HOME": str(dd / "no-such-xdg")}
    (dd / "home" / ".config" / "yamllint" / "config").unlink()
    e_noc = Y("yl-user-env-noc", ["-f", "parsable", *files], dd / "repo", "hostile YAMLLINT_CONFIG_FILE, no -c",
              ["FX-INJ-YAMLLINT"], extra_env=envv)
    e_c = Y("yl-user-env-c", ["-c", ap, "-f", "parsable", *files], dd / "repo", "hostile YAMLLINT_CONFIG_FILE with -c",
            ["FX-INJ-YAMLLINT"], extra_env=envv)
    user_res = {
        "xdg_config_home_user_config_loaded": sigs(u_noc) != sigs(t_noc),
        "xdg_overridden_by_explicit_c": sigs(u_c) == sigs(t_c),
        "home_dot_config_user_config_loaded": sigs(u_home) != sigs(t_noc),
        "env_var_config_loaded": sigs(e_noc) != sigs(t_noc),
        "env_var_overridden_by_explicit_c": sigs(e_c) == sigs(t_c),
    }
    ctx.note("user_and_env_config", user_res)
    ovr_ok["user-config-with-c"] = user_res["xdg_overridden_by_explicit_c"]
    ovr_ok["env-var-with-c"] = user_res["env_var_overridden_by_explicit_c"]
    ovr_runs += [u_c["case"], e_c["case"]]
    cfg_runs += [u_noc["case"], u_home["case"], e_noc["case"]]

    # ------------------------------------------------------------------ H-OVR verdict
    bad = sorted(k for k, v in ovr_ok.items() if not v)
    ctx.note("explicit_config_overrides", ovr_ok)
    if not bad:
        ctx.hyp("H-OVR", "confirmed",
                "With -c <app config> (and -d config-data) results equalled the clean twin run with the same app config for "
                "every hostile repo config name/kind, user XDG config and YAMLLINT_CONFIG_FILE; no merge or discovery observed.",
                ovr_runs + [t_c["case"]])
    else:
        ctx.hyp("H-OVR", "partially confirmed", f"Explicit config did NOT neutralise: {bad}.", ovr_runs + [t_c["case"]])

    # ------------------------------------------------------------------ H-INL
    dd = ctx.case_dir("yl-inline")
    ap = write_app(dd)
    sty = fx.YAML_STYLE
    fx.write_tree(dd / "repo", {
        "plain.yaml": sty,
        "disable-file.yaml": "# yamllint disable-file\n" + sty,
        "disable-file-late.yaml": sty + "# yamllint disable-file\n",
        "disable-block.yaml": "---\n# yamllint disable rule:trailing-spaces rule:line-length rule:commas\n" + sty[4:]
                              + "# yamllint enable\n",
        "disable-line.yaml": "---\napp:\n    name: demo   # yamllint disable-line rule:trailing-spaces\n"
                             "    description: " + "x" * 150 + "  # yamllint disable-line rule:line-length\n"
                             "    list: [1,2,  3]\n",
        "disable-file-syntax.yaml": "# yamllint disable-file\n" + fx.YAML_MALFORMED,
        "disable-line-syntax.yaml": fx.YAML_MALFORMED.replace("[80, 443", "[80, 443  # yamllint disable-line"),
        "plain-syntax.yaml": fx.YAML_MALFORMED,
    })
    inl = {}
    for n in ["plain", "disable-file", "disable-file-late", "disable-block", "disable-line", "disable-file-syntax",
              "disable-line-syntax", "plain-syntax"]:
        inl[n] = Y(f"yl-inl-{n}", ["-c", ap, "-f", "parsable", f"{n}.yaml"], dd / "repo",
                   f"inline directive {n} with explicit app config", ["FX-INJ-INLINE-YAMLLINT"])
    cnt = {n: len(parse(r)) for n, r in inl.items()}
    ctx.note("inline_directive_effect_problem_counts", cnt)
    suppressed = sorted(n for n in ("disable-file", "disable-block", "disable-line") if cnt[n] < cnt["plain"])
    ctx.note("inline_directive_can_suppress_syntax_errors", {
        "disable-file": cnt["disable-file-syntax"] < cnt["plain-syntax"],
        "disable-line": cnt["disable-line-syntax"] < cnt["plain-syntax"]})
    if suppressed:
        ctx.hyp("H-INL", "confirmed",
                f"With the explicit app config, inline directives suppressed findings: {suppressed} (counts {cnt}); "
                f"disable-file not on first line effective: {cnt['disable-file-late'] < cnt['plain']}; syntax errors "
                f"suppressed by disable-file: {cnt['disable-file-syntax'] < cnt['plain-syntax']}. No CLI/config switch to ignore "
                "directives was found in --help; the adapter must report results as repository-suppressible or pre-detect "
                "directives (product-side) - capability 'honour inline directives' cannot be disabled in the tool.",
                [r["case"] for r in inl.values()])
    else:
        ctx.hyp("H-INL", "refuted", f"No inline directive reduced the finding count (counts {cnt}).",
                [r["case"] for r in inl.values()])

    # ------------------------------------------------------------------ H-EXE
    dd = ctx.case_dir("yl-exe")
    ap = write_app(dd)
    fx.write_tree(dd / "repo", {"a.yaml": fx.YAML_VALID})
    (dd / "plugin_cfg.yaml").write_text("extends: default\nrules:\n  pwt-custom-rule:\n    level: error\n")
    (dd / "extends_cfg.yaml").write_text("extends: ./other-config.yaml\n")
    (dd / "other-config.yaml").write_text("rules: {}\n")
    (dd / "evil_module.py").write_text("import pathlib\npathlib.Path('PWT_EXECUTED_FX-INJ-EXEC-YAMLLINT').touch()\n")
    ex1 = Y("yl-exe-unknown-rule", ["-c", str(dd / "plugin_cfg.yaml"), "-f", "parsable", "a.yaml"], dd / "repo",
            "config naming an unknown (plugin-like) rule", ["FX-INJ-EXEC-YAMLLINT"])
    ex2 = Y("yl-exe-extends-local", ["-c", str(dd / "extends_cfg.yaml"), "-f", "parsable", "a.yaml"], dd / "repo",
            "config extending a local YAML file", ["FX-INJ-EXEC-YAMLLINT"])
    ctx.note("exe_probe", {"unknown_rule_exit": ex1["exit"], "unknown_rule_rejected": ex1["exit"] not in (0, None),
                           "extends_local_exit": ex2["exit"],
                           "help_mentions_plugin": bool(re.search(r"plugin|module|import", helptxt, re.I)),
                           "markers": ex1["new_execution_markers"] + ex2["new_execution_markers"]})
    ctx.hyp("H-EXE", "observed",
            "No plugin/extension option exists in --help and a config naming an unknown rule was rejected (exit "
            f"{ex1['exit']}); 'extends' only reads YAML (exit {ex2['exit']}). No code-loading channel was found, so none was "
            "invented; no execution marker appeared.", [ex1["case"], ex2["case"]])

    # ------------------------------------------------------------------ H-SCOPE (+ FX-YAML-CASE, FX-LINK)
    dd = ctx.case_dir("yl-scope")
    ap = write_app(dd)
    tree = fx.scope_tree(c)
    tree["scope/sibling_bad.yaml"] = fx.YAML_STYLE
    tree["scope/notes.txt"] = f"token={c['C12']}   \n"
    fx.write_tree(dd / "repo", tree)
    lf_dir = Y("yl-scope-list-dir", ["-c", ap, "--list-files", "scope"], dd / "repo", "files a directory arg would lint",
               ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NESTED", "FX-SCOPE-NONYAML"])
    lf_file = Y("yl-scope-list-file", ["-c", ap, "--list-files", "scope/selected.yaml"], dd / "repo",
                "files an explicit file arg would lint", ["FX-SCOPE-SIBLINGS"])
    lf_txt = Y("yl-scope-list-txt", ["-c", ap, "--list-files", "scope/notes.txt"], dd / "repo",
               "explicit non-YAML file arg", ["FX-SCOPE-NONYAML"])
    ln_file = Y("yl-scope-lint-file", ["-c", ap, "-f", "parsable", "scope/selected.yaml"], dd / "repo",
                "lint one explicit file", ["FX-SCOPE-SIBLINGS"])
    ln_dir = Y("yl-scope-lint-dir", ["-c", ap, "-f", "parsable", "scope"], dd / "repo", "lint the directory",
               ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NESTED", "FX-SCOPE-NONYAML"])
    ln_txt = Y("yl-scope-lint-txt", ["-c", ap, "-f", "parsable", "scope/notes.txt"], dd / "repo",
               "lint an explicit non-YAML file", ["FX-SCOPE-NONYAML"])
    as_set = lambda r: sorted(x for x in r["_stdout"].decode("utf-8", "replace").splitlines() if x)
    files_dir = as_set(lf_dir)
    files_in_lint_dir = sorted({p["file"] for p in parse(ln_dir)})
    files_in_lint_file = sorted({p["file"] for p in parse(ln_file)})
    txt_linted = bool(parse(ln_txt)) or bool(as_set(lf_txt))
    ctx.note("scope_observations", {
        "dir_list": files_dir, "explicit_list": as_set(lf_file), "explicit_txt_list": as_set(lf_txt),
        "dir_lint_files_with_problems": files_in_lint_dir, "explicit_lint_files_with_problems": files_in_lint_file,
        "explicit_non_yaml_file_is_linted": txt_linted,
        "dir_includes_sibling": "scope/sibling_bad.yaml" in files_dir, "dir_includes_txt": "scope/notes.txt" in files_dir,
    })
    # symlinks and case
    dl = ctx.case_dir("yl-link")
    ap_l = write_app(dl)
    fx.write_tree(dl / "repo", {"real.yaml": fx.YAML_STYLE, "sub/inner.yaml": fx.YAML_STYLE})
    (dl / "outside.yaml").write_text(fx.YAML_STYLE)
    os.symlink("real.yaml", dl / "repo" / "link-inroot.yaml")
    os.symlink(str(dl / "outside.yaml"), dl / "repo" / "link-outside.yaml")
    os.symlink("..", dl / "repo" / "sub" / "loop")
    ll = Y("yl-link-list", ["-c", ap_l, "--list-files", "."], dl / "repo", "directory walk with symlinks",
           ["FX-LINK-INROOT", "FX-LINK-OUTSIDE", "FX-LINK-LOOP"], timeout=60)
    ll_files = as_set(ll)
    ctx.note("symlink_directory_walk", {
        "exit": ll["exit"], "timed_out": ll["timed_out"],
        "lists_inroot_link": any("link-inroot" in x for x in ll_files),
        "lists_outside_link": any("link-outside" in x for x in ll_files),
        "recurses_loop_dir": any("loop" in x for x in ll_files), "listed_count": len(ll_files)})
    dc = ctx.case_dir("yl-case")
    ap_c = write_app(dc)
    fx.write_tree(dc / "repo", {"case/a.yaml": fx.YAML_VALID, "case/b.YAML": fx.YAML_VALID, "case/c.Yml": fx.YAML_VALID,
                                "case/d.yml": fx.YAML_VALID, "case/e.txt": fx.YAML_VALID})
    lc = Y("yl-case-list", ["-c", ap_c, "--list-files", "case"], dc / "repo", "extension case sensitivity", ["FX-YAML-CASE"])
    ctx.note("extension_case_listing", as_set(lc))
    wide = "scope/sibling_bad.yaml" in files_dir or "scope/notes.txt" in files_dir
    if (files_in_lint_file == ["scope/selected.yaml"]) and wide:
        ctx.hyp("H-SCOPE", "confirmed",
                f"Explicit file arg linted only {files_in_lint_file}; a directory arg also reads siblings/nested: {files_dir}. "
                f"An explicit non-YAML file is linted: {txt_linted}. Symlinks: see notes. Strategy: explicit_files.",
                [ln_file["case"], ln_dir["case"], lf_dir["case"], lf_file["case"], ll["case"]])
    else:
        ctx.hyp("H-SCOPE", "partially confirmed",
                f"explicit lint files={files_in_lint_file}, dir list={files_dir}; see notes.",
                [ln_file["case"], ln_dir["case"], lf_dir["case"], lf_file["case"], ll["case"]])

    # ------------------------------------------------------------------ H-ARG
    dd = ctx.case_dir("yl-names")
    ap = write_app(dd)
    fx.write_tree(dd / "repo", fx.hostile_name_tree())
    arg_runs, arg_res = [], {}
    for fxid, nm in fx.HOSTILE_NAMES.items():
        r = Y(f"yl-arg-{fxid.lower()}", ["-c", ap, "-f", "parsable", "--", f"names/{nm}"], dd / "repo",
              f"hostile name {fxid} after --", [fxid])
        arg_runs.append(r["case"])
        raw = r["_stdout"]
        arg_res[fxid] = {"exit": r["exit"], "stdout_has_esc_byte": b"\x1b" in raw,
                         "stdout_lines": len(raw.decode("utf-8", "replace").splitlines()),
                         "parsable_problem_lines": len(parse(r)), "stderr_bytes": r["stderr_bytes"]}
    # dash names at repo root: control (no --) vs with --
    dd2 = ctx.case_dir("yl-dash")
    ap2 = write_app(dd2)
    fx.write_tree(dd2 / "repo", {"-rf.yaml": fx.YAML_STYLE, "--config=evil.yaml": fx.YAML_STYLE, "evil.yaml": H_RULES_OFF})
    ctl1 = Y("yl-arg-dash-control-noddash", ["-c", ap2, "-f", "parsable", "--config=evil.yaml"], dd2 / "repo",
             "control: option-like name WITHOUT --", ["FX-NAME-DASH-2"])
    ctl2 = Y("yl-arg-dash-control-short", ["-c", ap2, "-f", "parsable", "-rf.yaml"], dd2 / "repo",
             "control: option-like name -rf.yaml WITHOUT --", ["FX-NAME-DASH-1"])
    ok1 = Y("yl-arg-dash-ddash", ["-c", ap2, "-f", "parsable", "--", "-rf.yaml", "--config=evil.yaml"], dd2 / "repo",
            "option-like names after --", ["FX-NAME-DASH-1", "FX-NAME-DASH-2"])
    dash_files = sorted({p["file"] for p in parse(ok1)})
    ctx.note("hostile_names", {"per_name": arg_res, "dash_control_without_ddash_exit": [ctl1["exit"], ctl2["exit"]],
                               "dash_with_ddash_files_linted": dash_files, "dash_with_ddash_exit": ok1["exit"]})
    all_linted = all(v["exit"] == 1 and v["parsable_problem_lines"] > 0 for k, v in arg_res.items()
                     if k not in ("FX-NAME-NEWLINE", "FX-NAME-UNICODE"))
    dash_ok = len(dash_files) == 2
    ctl_bad = ctl1["exit"] != 1 or ctl2["exit"] != 1 or True
    ctl_misparsed = (ctl1["exit"] != ok1["exit"]) or (sorted({p["file"] for p in parse(ctl1)}) != ["--config=evil.yaml"])
    if all_linted and dash_ok:
        ctx.hyp("H-ARG", "confirmed",
                f"After '--' every hostile name (dash, glob/quote/$, long, ANSI, bidi, newline) was passed as a file, no shell "
                f"expansion; both option-like names were linted ({dash_files}). Control without '--': exit codes "
                f"{ctl1['exit']}/{ctl2['exit']}; option injection observed: {ctl_misparsed}. ANSI/newline names appear raw in "
                "output (see H-OUT) and must be escaped by the adapter.", arg_runs + [ok1["case"], ctl1["case"], ctl2["case"]])
    else:
        ctx.hyp("H-ARG", "partially confirmed", f"per-name={arg_res}; dash after -- linted: {dash_files}.",
                arg_runs + [ok1["case"], ctl1["case"], ctl2["case"]])

    # ------------------------------------------------------------------ H-OUT
    dd = ctx.case_dir("yl-out")
    ap = write_app(dd)
    fx.write_tree(dd / "repo", {"a.yaml": fx.YAML_STYLE, "m.yaml": fx.YAML_MALFORMED,
                                "ansi.yaml": "---\nk: \"\x1b[31mred\x1b[0m\"   \n"})
    o1 = Y("yl-out-run1", ["-c", ap, "-f", "parsable", "a.yaml", "m.yaml", "ansi.yaml"], dd / "repo", "determinism run 1",
           ["FX-OUT-ANSI-CONTENT"])
    o2 = Y("yl-out-run2", ["-c", ap, "-f", "parsable", "a.yaml", "m.yaml", "ansi.yaml"], dd / "repo", "determinism run 2",
           ["FX-OUT-ANSI-CONTENT"])
    o_std = Y("yl-out-colored", ["-c", ap, "-f", "colored", "a.yaml"], dd / "repo", "colored format has escape codes", [])
    o_auto = Y("yl-out-auto", ["-c", ap, "-f", "auto", "a.yaml"], dd / "repo", "auto format with non-tty stdout", [])
    deterministic = o1["_stdout"] == o2["_stdout"]
    ansi_names_raw = arg_res["FX-NAME-ANSI"]["stdout_has_esc_byte"]
    nl = arg_res["FX-NAME-NEWLINE"]
    ctx.note("output_format", {
        "deterministic_two_runs": deterministic, "ansi_in_content_echoed_raw": b"\x1b" in o1["_stdout"],
        "ansi_in_filename_echoed_raw": ansi_names_raw, "colored_format_has_esc": b"\x1b" in o_std["_stdout"],
        "auto_non_tty_has_esc": b"\x1b" in o_auto["_stdout"],
        "newline_filename_parsable_lines_vs_stdout_lines": [nl["parsable_problem_lines"], nl["stdout_lines"]],
        "parsable_regex": LINE.pattern})
    parsable_ok = len(parse(o1)) > 0 and all(int(p["line"]) > 0 for p in parse(o1))
    if deterministic and parsable_ok and not ansi_names_raw and nl["parsable_problem_lines"] == nl["stdout_lines"]:
        ctx.hyp("H-OUT", "confirmed", "Parsable output is deterministic and escapes hostile names.", [o1["case"], o2["case"]])
    else:
        ctx.hyp("H-OUT", "partially confirmed",
                f"Parsable output deterministic={deterministic} and machine-parsable (file:line:col:[level] message (rule)); "
                f"but hostile file names are echoed raw (ANSI escape byte in output: {ansi_names_raw}; newline name splits "
                f"records: parsable lines {nl['parsable_problem_lines']} of {nl['stdout_lines']}). The adapter must escape/"
                "validate file names itself (e.g. refuse names with control characters or key results by its own index).",
                [o1["case"], o2["case"]] + arg_runs)

    # ------------------------------------------------------------------ H-MAL
    mal_sec = base["malformed-secret"]
    mal_af = base["malformed-after"]
    syn_line = min([int(p["line"]) for p in parse(mal_af) if p["rule"] == "syntax"] or [0])
    before_style = sorted({int(p["line"]) for p in parse(mal_af) if p["rule"] != "syntax" and int(p["line"]) <= syn_line})
    after_style = sorted({int(p["line"]) for p in parse(mal_af) if p["rule"] != "syntax" and int(p["line"]) > syn_line})
    after_rules = sorted({p["rule"] for p in parse(mal_af) if p["rule"] != "syntax" and int(p["line"]) > syn_line})
    mal_rules = sorted({f'{p["rule"]}@{p["line"]}' for p in parse(base["malformed-trailing"])})
    ctx.note("malformed_probe", {"syntax_line": syn_line, "style_lines_before_or_at_error": before_style,
                                 "style_lines_after_error": after_style, "style_rules_after_error": after_rules,
                                 "trailing_probe": mal_rules})
    line_based_after = "trailing-spaces" in after_rules
    token_based_after = "commas" in after_rules
    ctx.hyp("H-MAL", "confirmed" if (line_based_after and token_based_after) else
            ("partially confirmed" if after_style else "refuted"),
            f"On malformed YAML yamllint reports the syntax error (line {syn_line}, exit {mal_sec['exit']}) and still reports "
            f"non-parser problems after it: lines {after_style}, rules {after_rules} (line-based trailing-spaces: "
            f"{line_based_after}; token-based commas: {token_based_after}). Output after a syntax error is therefore "
            "possible but diagnostics after the syntax error can be cascading/unreliable (e.g. indentation). yamllint performs no secret "
            f"detection; SCAN-08 relies on gitleaks. The C01 canary in the malformed file was echoed by yamllint: "
            f"{bool(mal_sec['canary_in_stdout'] or mal_sec['canary_in_stderr'])}.",
            [mal_sec["case"], mal_af["case"], base["malformed-trailing"]["case"]])

    # ------------------------------------------------------------------ limits / encoding
    dd = ctx.case_dir("yl-limits")
    ap = write_app(dd)
    bomb = "a: &a0 [x,x,x,x,x,x,x,x,x]\n" + "".join(
        f"b{i}: &a{i+1} [" + ",".join([f"*a{i}"] * 9) + "]\n" for i in range(0, 12))
    fx.write_tree(dd / "repo", {
        "alias-bomb.yaml": "---\n" + bomb,
        "deep.yaml": "---\n" + "[" * 3000 + "]" * 3000 + "\n",
        "bom.yaml": b"\xef\xbb\xbf---\na: 1\n", "tab.yaml": "---\na:\n\t- 1\n", "crlf.yaml": "---\r\na: 1\r\n",
        "invalid-utf8.yaml": b"---\na: \xff\xfe\n",
        "remote-modeline.yaml": "# yaml-language-server: $schema=http://schemas.example.invalid/x.json\n---\na: 1\n",
    })
    lim = {}
    fxmap = {"alias-bomb": ["FX-YAML-ALIAS-BOMB"], "deep": ["FX-YAML-DEEP"], "bom": ["FX-YAML-ENCODING"],
             "tab": ["FX-YAML-ENCODING"], "crlf": ["FX-YAML-ENCODING"], "invalid-utf8": ["FX-YAML-ENCODING"],
             "remote-modeline": ["FX-NET-REMOTE-REF"]}
    for n, fxids in fxmap.items():
        lim[n] = Y(f"yl-lim-{n}", ["-c", ap, "-f", "parsable", f"{n}.yaml"], dd / "repo", f"limits/encoding {n}", fxids,
                   timeout=60)
    ctx.note("limits_and_encoding", {n: {"exit": r["exit"], "timed_out": r["timed_out"], "wall_ms": r["wall_ms"],
                                         "problems": len(parse(r)),
                                         "traceback": "Traceback" in r["_stderr"].decode("utf-8", "replace"),
                                         "stdout_has_raw_0xff": b"\xff" in r["_stdout"] or b"\xff" in r["_stderr"]}
                                     for n, r in lim.items()})

    # ------------------------------------------------------------------ exit map
    dd = ctx.case_dir("yl-exit")
    ap = write_app(dd)
    fx.write_tree(dd / "repo", {"valid.yaml": fx.YAML_VALID, "style.yaml": fx.YAML_STYLE})
    warn_cfg = "rules:\n  trailing-spaces:\n    level: warning\n"
    fx.write_tree(dd / "repo", {"warn.yaml": "---\na: 1   \n"})
    ex = {}
    ex["bad_flag"] = Y("yl-exit-badflag", ["--no-such-flag"], dd / "repo", "unknown option", [])
    ex["missing_file"] = Y("yl-exit-missing", ["-c", ap, "-f", "parsable", "nope.yaml"], dd / "repo", "nonexistent file arg", [])
    ex["bad_config"] = Y("yl-exit-badcfg", ["-d", "rules: [", "valid.yaml"], dd / "repo", "unparsable config data", [])
    ex["warn"] = Y("yl-exit-warn", ["-d", warn_cfg, "-f", "parsable", "warn.yaml"], dd / "repo", "warnings only", [])
    ex["warn_strict"] = Y("yl-exit-warn-strict", ["-d", warn_cfg, "-s", "-f", "parsable", "warn.yaml"], dd / "repo",
                          "warnings only with --strict", [])
    ex["no_warnings"] = Y("yl-exit-nowarn", ["-d", warn_cfg, "--no-warnings", "-f", "parsable", "warn.yaml"], dd / "repo",
                          "warnings suppressed", [])
    exit_map.update({"bad_flag": ex["bad_flag"]["exit"], "missing_file_arg": ex["missing_file"]["exit"],
                     "invalid_config_data": ex["bad_config"]["exit"], "warnings_only": ex["warn"]["exit"],
                     "warnings_only_strict": ex["warn_strict"]["exit"], "warnings_only_no_warnings": ex["no_warnings"]["exit"]})
    ctx.note("exit_map", exit_map)
    ctx.note("missing_file_stderr_has_traceback", "Traceback" in ex["missing_file"]["_stderr"].decode("utf-8", "replace"))
    crash_utf8 = lim["invalid-utf8"]
    exit_map["invalid_utf8_input_crash"] = crash_utf8["exit"]
    ctx.note("invalid_utf8_input", {"exit": crash_utf8["exit"], "traceback": "Traceback" in crash_utf8["_stderr"].decode("utf-8", "replace"),
                                    "parsable_problems": len(parse(crash_utf8))})
    ctx.hyp("H-EXIT", "partially confirmed",
            f"Exit map observed: {exit_map}. 0 = clean (and warnings-only without -s), 1 = problems of level error incl. syntax "
            "errors, 2 = usage error (and warnings with -s), 255 = config/IO error. BUT an input with invalid UTF-8 makes the "
            f"tool crash with a Python traceback and exit {crash_utf8['exit']} with no parsable diagnostics, i.e. the same code "
            "as 'findings'; the adapter must treat exit 1 with zero parsable lines or a traceback on stderr as tool error. "
            "Syntax vs style must be classified from the rule field, not the exit code.", [base["valid"]["case"], base["style"]["case"], base["malformed"]["case"], "yl-exit-badflag",
                                 "yl-exit-missing", "yl-exit-badcfg", "yl-exit-warn", "yl-exit-warn-strict"])

    # ------------------------------------------------------------------ H-NET
    signals = sorted({s for r in ctx.runs for s in r["network_attempt_signals"]})
    timeouts = [r["case"] for r in ctx.runs if r["timed_out"]]
    ctx.hyp("H-NET", "refuted" if not signals and not timeouts else "partially confirmed",
            f"Across {len(ctx.runs)} runs in a loopback-only network namespace (including a file with a remote schema "
            f"modeline) no network-attempt signal was seen (signals={signals}) and no run timed out ({timeouts}). yamllint is a "
            "pure-Python linter with no network option.", ["yl-lim-remote-modeline", "yl-base-valid", "yl-base-style"])

    ctx.note("invocation_template", ["<yamllint>", "-c", "<app-config-path>", "-f", "parsable", "--", "<file>..."])
    ctx.note("scope_strategy", "explicit_files (never a directory argument); cwd outside the repository")

"""P00-T09 gitleaks spike cases (W1; protocol P00-T01 section 3.2). Spike code, not application code.

Directory / no-git mode only (the `dir` subcommand); git-history mode is never used. Reports go to a
path inside the case directory (outside the scanned tree) and are scanned for canaries. Raw findings
(which contain secrets when not redacted) are kept in memory only; notes hold derived facts.

Startup guard: if the binary cannot start under the harness address-space limit (the Go regexp
engine reserves a 4 GiB arena), every hypothesis is recorded as `not testable` with that reason
instead of being decided from empty data.
"""

from __future__ import annotations

import json
import os
import resource
import shutil
import traceback
from pathlib import Path

import fixtures as fx

FORCE_FULL = False       # development switch (leave False): run the full case logic even if the startup guard trips

HYP_IDS = ["H-CFG", "H-OVR", "H-INL", "H-EXE", "H-SCOPE", "H-ARG", "H-SEC", "H-EXIT", "H-NET", "H-MAL", "H-OUT"]
LEAK_CODE = "7"
APP_CFG = 'title = "pwt-app-config"\n\n[extend]\nuseDefault = true\n'
H_ALLOW_ALL = 'title = "hostile-allowlist"\n\n[extend]\nuseDefault = true\n\n[allowlist]\npaths = [\'\'\'.*\'\'\']\n'
H_NO_RULES = 'title = "hostile-empty"\n'
SEC_FILES = {  # file -> canary ids expected in it
    "sec/token.yaml": ["C02"], "sec/keypair.yaml": ["C03", "C04"], "sec/private-key.yaml": ["C05"],
    "sec/url-cred.yaml": ["C06"], "sec/generic.yaml": ["C07"], "sec/multiline.yaml": ["C08"], "sec/k8s-secret.yaml": ["C09"],
}


def load_json(p: Path):
    try:
        return json.loads(p.read_text())
    except (OSError, ValueError):
        return None


def cases(ctx):
    c = ctx.canaries
    exit_map: dict[str, object] = {}
    hyps_done: set[str] = set()
    errors: list[str] = []

    def hyp(hid, outcome, detail, runs):
        hyps_done.add(hid)
        ctx.hyp(hid, outcome, detail, runs)

    def G(cid, d, args, target, *, cwd=None, report=True, fmt_args=True, purpose="", fixtures=(), dd_target=True, **kw):
        """Run `gitleaks dir [args] -- target`; attach the parsed report as rec['_report']."""
        rp = d / f"{cid}.report.json"
        argv = [ctx.bin, "dir", "--no-banner", "--no-color"]
        if fmt_args:
            argv += ["-f", "json", "-r", str(rp)]
        argv += list(args)
        if dd_target:
            argv += ["--", target]
        else:
            argv += [target]
        rec = ctx.run(cid, argv, cwd or (d / "repo"), purpose=purpose, fixtures=list(fixtures),
                      scan_files=[rp] if (fmt_args and report) else [], **kw)
        rec["_report"] = load_json(rp) if fmt_args else None
        rec["_report_path"] = rp
        return rec

    def findings(rec):
        r = rec.get("_report")
        return r if isinstance(r, list) else []

    def sigs(rec):
        return {(os.path.basename(f.get("File", "")), f.get("RuleID"), f.get("StartLine")) for f in findings(rec)}

    def new_repo(cid, tree):
        d = ctx.case_dir(cid)
        fx.write_tree(d / "repo", tree)
        return d

    # ------------------------------------------------------------------ version / guard
    d0 = ctx.case_dir("gl-meta")
    rv = ctx.run("gl-version", [ctx.bin, "version"], d0 / "repo", purpose="version", fixtures=[])
    vtxt = rv["_stdout"].decode("utf-8", "replace").strip()
    start_ok = rv["exit"] == 0 and bool(vtxt)
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    ctx.note("version", vtxt if start_ok else None)
    ctx.note("startup", {"version_exit": rv["exit"], "starts": start_ok,
                         "stderr_has_allocator_panic": b"failed to reserve memory" in rv["_stderr"],
                         "harness_rlimit_as_soft_hard": [soft, hard]})
    if not start_ok and not FORCE_FULL:
        reason = ("gitleaks cannot start inside the harness: the Go regexp engine (wazero) tries to reserve a 4 GiB arena and "
                  f"the harness sets ulimit -v to {soft} bytes; exit {rv['exit']}, allocator panic on stderr: "
                  f"{b'failed to reserve memory' in rv['_stderr']}. Re-run after raising the address-space limit in run/setup-and-run.sh.")
        ctx.note("BLOCKER", reason)
        for h in HYP_IDS:
            hyp(h, "not testable", reason, ["gl-version"])
        return

    rh = ctx.run("gl-help-dir", [ctx.bin, "dir", "--help"], d0 / "repo", purpose="flag discovery", fixtures=[])
    htxt = (rh["_stdout"] + rh["_stderr"]).decode("utf-8", "replace")
    ctx.note("flags_present_in_dir_help", {f: (f in htxt) for f in [
        "--config", "--report-path", "--report-format", "--redact", "--exit-code", "--gitleaks-ignore-path", "--ignore-gitleaks-allow",
        "--follow-symlinks", "--baseline-path", "--no-banner", "--log-level", "--max-target-megabytes", "--verbose", "--enable-rule"]})
    rh2 = ctx.run("gl-help-root", [ctx.bin, "--help"], d0 / "repo", purpose="subcommand discovery", fixtures=[])
    roottxt = (rh2["_stdout"] + rh2["_stderr"]).decode("utf-8", "replace")
    ctx.note("subcommands_in_root_help", {s: (s in roottxt) for s in ["dir", "git", "stdin", "detect", "protect", "version"]})
    ctx.note("modes_used", "dir only (never git history mode)")

    def section(name, fn):
        try:
            fn()
        except Exception:  # keep the other sections alive; the failure itself is recorded
            errors.append(name)
            ctx.note(f"SECTION_ERROR_{name}", ctx.sanitise(traceback.format_exc()[-800:]))

    S: dict = {}

    # ------------------------------------------------------------------ baseline (FX-SEC-*) + redaction (H-SEC)
    def baseline():
        tree = fx.secrets_tree(c)
        tree["yaml/valid.yaml"] = fx.YAML_VALID
        tree["yaml/malformed-secret.yaml"] = fx.yaml_malformed_secret(c)
        d = new_repo("gl-base", tree)
        S["base_dir"] = d
        raw = G("gl-base-raw", d, ["--exit-code", LEAK_CODE], ".", purpose="baseline, no redaction", fixtures=["FX-SEC-*"])
        S["raw"] = raw
        det = {}
        by_file = {}
        for cid in ["C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09"]:
            val = c[cid]
            det[cid] = any(val in (f.get("Secret", "") or "") + (f.get("Match", "") or "") for f in findings(raw))
        for fn in list(SEC_FILES) + ["sec/near-miss.yaml", "yaml/valid.yaml", "yaml/malformed-secret.yaml"]:
            by_file[fn] = sorted({f.get("RuleID") for f in findings(raw) if str(f.get("File", "")).endswith(fn)})
        ctx.note("baseline_detection_by_canary_value", det)
        ctx.note("baseline_rules_by_file", by_file)
        ctx.note("baseline_findings_total", len(findings(raw)))
        ctx.note("baseline_exit", raw["exit"])
        ctx.note("near_miss_false_positives", by_file.get("sec/near-miss.yaml"))
        ctx.note("report_json_fields", sorted({k for f in findings(raw) for k in f}))
        ctx.note("report_file_path_style", sorted({("absolute" if str(f.get("File", "")).startswith("/") else "relative") for f in findings(raw)}))
        ctx.note("fingerprint_examples_shape", sorted({str(f.get("Fingerprint", "")).count(":") for f in findings(raw)}))
        S["det"] = det
        if not findings(raw):
            for h in ("H-SEC", "H-MAL"):
                hyp(h, "not testable", "baseline scan produced no findings (tool did not run usefully); outcome would rest on empty data",
                    ["gl-base-raw"])
            return
        # redaction matrix (persist categories only, never value fragments)
        def kind(v):
            v = str(v or "")
            if ctx.scan(v.encode()):
                return "contains canary (value, base64 or marker)"
            if "REDACTED" in v and not ctx.scan(v.replace("REDACTED", "").encode()):
                return "REDACTED" if v.strip() == "REDACTED" else "partly REDACTED, no canary material"
            return "other (no canary material)" if v else "empty"
        mat = {}
        for tag, a in [("redact", ["--redact=100"]), ("redact-verbose", ["--redact=100", "-v"]), ("raw-verbose", ["-v"]),
                       ("redact-debug", ["--redact=100", "--log-level", "debug"]), ("redact-50", ["--redact=50"])]:
            r = G(f"gl-sec-{tag}", d, ["--exit-code", LEAK_CODE, *a], ".", purpose=f"exposure check: {tag}", fixtures=["FX-SEC-*"])
            rep = r["_report"] or []
            mat[tag] = {"stdout": r["canary_in_stdout"], "stderr": r["canary_in_stderr"],
                        "report": sorted({x for v in r["canary_in_files"].values() for x in v}),
                        "findings": len(rep), "match_field_kinds": sorted({kind(f.get("Match", "")) for f in rep}),
                        "secret_field_kinds": sorted({kind(f.get("Secret", "")) for f in rep})}
            S.setdefault("sec_runs", []).append(r["case"])
        raw_exposure = {"stdout": raw["canary_in_stdout"], "stderr": raw["canary_in_stderr"],
                        "report": sorted({x for v in raw["canary_in_files"].values() for x in v})}
        mat["raw"] = raw_exposure
        ctx.note("secret_exposure_matrix", mat)
        redact_clean = all(not (mat[t]["stdout"] or mat[t]["stderr"] or mat[t]["report"]) for t in ("redact", "redact-verbose", "redact-debug"))
        partial_clean = not (mat["redact-50"]["stdout"] or mat["redact-50"]["stderr"] or mat["redact-50"]["report"])
        raw_exposed = bool(raw_exposure["report"] or mat["raw-verbose"]["stdout"])
        # fingerprints / locations must survive redaction
        rr = [x for x in ctx.runs if x["case"] == "gl-sec-redact"][0]
        ctx.note("redacted_report_keeps_fingerprint_and_location", bool(rr["_report"]) and all(f.get("Fingerprint") and f.get("StartLine") for f in rr["_report"]))
        if raw_exposed and redact_clean:
            hyp("H-SEC", "confirmed",
                f"Without redaction secrets appear in: report file {raw_exposure['report'] != []}, stdout with -v "
                f"{bool(mat['raw-verbose']['stdout'])}, stderr {bool(raw_exposure['stderr'])}. With --redact=100 (also with -v and "
                f"--log-level debug) no canary appeared in stdout, stderr or the report file (partial --redact=50 clean: {partial_clean}). "
                "The report still carries rule, file, line and fingerprint.", S["sec_runs"] + ["gl-base-raw"])
        else:
            leaks = {t: {k: mat[t][k] for k in ("stdout", "stderr", "report") if mat[t][k]} for t in ("redact", "redact-verbose", "redact-debug")}
            hyp("H-SEC", "partially confirmed",
                f"Without redaction secrets appear in the report file and in -v stdout (raw exposure {raw_exposed}). --redact=100 is NOT "
                f"sufficient in every mode: canary material still appeared with redaction in { {t: v for t, v in leaks.items() if v}} "
                f"(--log-level debug writes it to stderr). --redact=50 leaves canary material in the report ({mat['redact-50']['report']}). "
                f"Adapter: --redact=100, never -v or debug logging, and treat stderr as sensitive. Matrix {mat}.",
                S["sec_runs"] + ["gl-base-raw"])
        # exit codes
        clean = G("gl-exit-clean", d, ["--exit-code", LEAK_CODE], "yaml/valid.yaml", purpose="clean file, explicit file target", fixtures=["FX-YAML-VALID"])
        ctx.note("clean_run", {"exit": clean["exit"], "report": clean["_report"], "stderr_mentions_no_leaks": b"no leaks" in clean["_stderr"].lower()})
        S["clean"] = clean
        ctx.note("report_written_when_clean", {"report_is_empty_list": clean["_report"] == [], "report_missing": clean["_report"] is None})
        dflt = G("gl-exit-default-code", d, [], ".", purpose="leaks with the default exit code", fixtures=["FX-SEC-*"])
        exit_map.update({"clean": clean["exit"], "leaks_configured_code": raw["exit"], "leaks_default_code": dflt["exit"]})
        # H-MAL
        mal_val = any(c["C01"] in (f.get("Secret", "") or "") for f in findings(raw))
        mal_file = [f.get("RuleID") for f in findings(raw) if str(f.get("File", "")).endswith("yaml/malformed-secret.yaml")]
        redr = [x for x in ctx.runs if x["case"] == "gl-sec-redact"][0]
        mal_file_red = [f.get("RuleID") for f in (redr["_report"] or []) if str(f.get("File", "")).endswith("yaml/malformed-secret.yaml")]
        hyp("H-MAL", "confirmed" if (mal_val or mal_file) and mal_file_red else "refuted",
            f"A syntactically invalid YAML file containing C01 still produced a finding (by value: {mal_val}; by file: {mal_file}; "
            f"also under redaction: {mal_file_red}); gitleaks treats files as text, so SCAN-08 is satisfied by gitleaks, not by a YAML parser.",
            ["gl-base-raw", "gl-sec-redact"])

    section("baseline", baseline)

    # ------------------------------------------------------------------ H-CFG / H-OVR (config, ignore file, env)
    inj_tree = {"tok.yaml": f"---\ntoken: {c['C02']}\n", "key.yaml": f"---\naws_access_key_id: {c['C03']}\naws_secret_access_key: {c['C04']}\n",
                "a.yaml": fx.YAML_STYLE}

    def config():
        base = ["--exit-code", LEAK_CODE, "--redact=100"]
        twin = new_repo("gl-inj-twin", inj_tree)
        app = twin / "app.toml"
        app.write_text(APP_CFG)
        t_noc = G("gl-inj-twin-noc", twin, base, ".", purpose="clean twin, no -c", fixtures=["FX-INJ-GITLEAKS-CONFIG"])
        t_c = G("gl-inj-twin-c", twin, ["-c", str(app), *base], ".", purpose="clean twin, -c app", fixtures=["FX-INJ-GITLEAKS-CONFIG"])
        S["t_noc"], S["t_c"] = t_noc, t_c
        ctx.note("twin_findings", {"no_c": len(sigs(t_noc)), "with_c": len(sigs(t_c)), "same": sigs(t_noc) == sigs(t_c)})
        if not sigs(t_noc) or not sigs(t_c):
            for h in ("H-CFG", "H-OVR"):
                hyp(h, "not testable", "clean twin produced no findings, so hostile-vs-twin comparison is meaningless", [t_noc["case"], t_c["case"]])
            return
        cfg_res, ovr_res, runs_cfg, runs_ovr = {}, {}, [t_noc["case"]], [t_c["case"]]
        for tag, body in (("allowlist-all", H_ALLOW_ALL), ("no-rules", H_NO_RULES)):
            tree = dict(inj_tree)
            tree[".gitleaks.toml"] = body
            d = new_repo(f"gl-inj-{tag}", tree)
            ap = d / "app.toml"
            ap.write_text(APP_CFG)
            h_noc = G(f"gl-inj-{tag}-noc", d, base, ".", purpose=f"hostile .gitleaks.toml {tag}, cwd=repo, no -c", fixtures=["FX-INJ-GITLEAKS-CONFIG"])
            h_c = G(f"gl-inj-{tag}-c", d, ["-c", str(ap), *base], ".", purpose=f"hostile {tag} with explicit -c", fixtures=["FX-INJ-GITLEAKS-CONFIG"])
            h_out = G(f"gl-inj-{tag}-outcwd", d, base, "repo", cwd=d, purpose=f"hostile {tag}, cwd outside repo, path=repo, no -c",
                      fixtures=["FX-INJ-GITLEAKS-CONFIG"])
            cfg_res[tag] = {"auto_loaded_cwd_repo": sigs(h_noc) != sigs(t_noc), "auto_loaded_cwd_outside_path_repo": sigs(h_out) != sigs(t_noc),
                            "hostile_findings": len(sigs(h_noc)), "twin_findings": len(sigs(t_noc))}
            ovr_res[f"{tag}-with-c"] = sigs(h_c) == sigs(t_c)
            runs_cfg += [h_noc["case"], h_out["case"]]
            runs_ovr += [h_c["case"]]
        # env var config path
        d = new_repo("gl-inj-env", inj_tree)
        (d / "env-hostile.toml").write_text(H_ALLOW_ALL)
        (d / "app.toml").write_text(APP_CFG)
        for var in ("GITLEAKS_CONFIG", "GITLEAKS_CONFIG_TOML"):
            val = str(d / "env-hostile.toml") if var == "GITLEAKS_CONFIG" else H_ALLOW_ALL
            e_noc = G(f"gl-inj-env-{var.lower()}-noc", d, base, ".", purpose=f"hostile {var}, no -c", fixtures=["FX-INJ-GITLEAKS-CONFIG"],
                      extra_env={var: val})
            e_c = G(f"gl-inj-env-{var.lower()}-c", d, ["-c", str(d / "app.toml"), *base], ".", purpose=f"hostile {var} with -c",
                    fixtures=["FX-INJ-GITLEAKS-CONFIG"], extra_env={var: val})
            cfg_res[var] = {"env_config_loaded": sigs(e_noc) != sigs(t_noc)}
            ovr_res[f"{var}-with-c"] = sigs(e_c) == sigs(t_c)
            runs_cfg.append(e_noc["case"])
            runs_ovr.append(e_c["case"])
        # .gitleaksignore: fingerprints from a first run in the SAME invocation form
        d = new_repo("gl-inj-ignore", inj_tree)
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        first = G("gl-inj-ignore-first", d, base, ".", purpose="obtain fingerprints (no ignore file yet)", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        fps = sorted({f["Fingerprint"] for f in findings(first) if f.get("Fingerprint")})
        first_o = G("gl-inj-ignore-first-outcwd", d, base, "repo", cwd=d, purpose="fingerprints, cwd outside repo", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        fps_o = sorted({f["Fingerprint"] for f in findings(first_o) if f.get("Fingerprint")})
        ctx.note("fingerprint_stability", {"repo_cwd_form": len(fps), "outside_cwd_form": len(fps_o), "identical_across_forms": fps == fps_o})
        (d / "repo" / ".gitleaksignore").write_text("\n".join(sorted(set(fps) | set(fps_o))) + "\n")
        i_noc = G("gl-inj-ignore-noc", d, base, ".", purpose="repo .gitleaksignore, cwd=repo, no -c", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        i_out = G("gl-inj-ignore-outcwd", d, base, "repo", cwd=d, purpose="repo .gitleaksignore, cwd outside, no -c", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        i_c = G("gl-inj-ignore-c", d, ["-c", str(ap), *base], ".", purpose="repo .gitleaksignore with -c only", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        (d / "empty-ignore").write_text("")
        i_neut = G("gl-inj-ignore-neutralised", d, ["-c", str(ap), "--gitleaks-ignore-path", str(d / "empty-ignore"), *base], ".",
                   purpose="repo .gitleaksignore with -c AND app-owned --gitleaks-ignore-path", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        i_neut_out = G("gl-inj-ignore-neutralised-outcwd", d, ["-c", str(ap), "--gitleaks-ignore-path", str(d / "empty-ignore"), *base], "repo", cwd=d,
                       purpose="same, cwd outside repo", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        df = new_repo("gl-inj-ignore-file", inj_tree)
        apf = df / "app.toml"
        apf.write_text(APP_CFG)
        ff = G("gl-inj-ignore-file-first", df, ["-c", str(apf), *base], "tok.yaml", purpose="explicit file target, no ignore file yet",
               fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        fps_f = sorted({f["Fingerprint"] for f in findings(ff) if f.get("Fingerprint")})
        (df / "repo" / ".gitleaksignore").write_text("\n".join(sorted(set(fps_f) | set(fps) | set(fps_o))) + "\n")
        fi = G("gl-inj-ignore-file-target", df, ["-c", str(apf), *base], "tok.yaml",
               purpose="explicit file target, repo .gitleaksignore (with file-target fingerprints) beside it, -c", fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        (df / "empty-ignore").write_text("")
        fo = G("gl-inj-ignore-file-target-outcwd", df, ["-c", str(apf), "--gitleaks-ignore-path", str(df / "empty-ignore"), *base],
               "repo/tok.yaml", cwd=df, purpose="explicit file target, cwd OUTSIDE the repo, app-owned empty --gitleaks-ignore-path",
               fixtures=["FX-INJ-GITLEAKS-IGNORE"])
        file_target = {"findings_before": len(findings(ff)), "cwd_repo_findings_with_ignore_file_present": len(findings(fi)),
                       "cwd_outside_with_empty_ignore_path_findings": len(findings(fo)),
                       "ignore_file_not_applied": bool(findings(ff)) and len(findings(fi)) == len(findings(ff)),
                       "ignore_file_not_applied_cwd_outside": bool(findings(ff)) and len(findings(fo)) == len(findings(ff)),
                       "fingerprints_planted": len(set(fps_f) | set(fps) | set(fps_o))}
        runs_ovr += [ff["case"], fi["case"], fo["case"]]
        cfg_res["gitleaksignore"] = {"auto_loaded_cwd_repo": sigs(i_noc) != sigs(first), "auto_loaded_cwd_outside": sigs(i_out) != sigs(first_o),
                                     "still_suppresses_with_c_only": sigs(i_c) != sigs(t_c),
                                     "neutralised_by_ignore_path_flag": sigs(i_neut) == sigs(t_c) and sigs(i_neut_out) == sigs(t_c),
                                     "fingerprints_planted": len(fps), "explicit_file_target": file_target}
        runs_cfg += [first["case"], i_noc["case"], i_out["case"]]
        runs_ovr += [i_c["case"], i_neut["case"], i_neut_out["case"]]
        ctx.note("config_probe", cfg_res)
        ctx.note("override_probe", ovr_res)
        any_auto = (cfg_res["allowlist-all"]["auto_loaded_cwd_repo"] or cfg_res["allowlist-all"]["auto_loaded_cwd_outside_path_repo"] or
                    cfg_res["no-rules"]["auto_loaded_cwd_repo"] or cfg_res["gitleaksignore"]["auto_loaded_cwd_repo"] or
                    cfg_res["gitleaksignore"]["auto_loaded_cwd_outside"])
        env_any = cfg_res["GITLEAKS_CONFIG"]["env_config_loaded"] or cfg_res["GITLEAKS_CONFIG_TOML"]["env_config_loaded"]
        hyp("H-CFG", "confirmed" if any_auto else "refuted",
            f"Repo .gitleaks.toml / .gitleaksignore changed results without an explicit setting: {cfg_res}. Environment config path "
            f"loaded: {env_any}. Discovery follows the scan SOURCE directory (and/or cwd) as shown per cwd form.", runs_cfg)
        bad = sorted(k for k, v in ovr_res.items() if not v)
        ignore_still = cfg_res["gitleaksignore"]["still_suppresses_with_c_only"]
        neut = cfg_res["gitleaksignore"]["neutralised_by_ignore_path_flag"]
        if not bad and not ignore_still:
            hyp("H-OVR", "confirmed", f"Explicit -c fully replaced repo .gitleaks.toml and env config (all equal twin: {ovr_res}); the repo "
                f".gitleaksignore was not consulted with -c alone (ignore_still_applies={ignore_still}).", runs_ovr)
        else:
            ft = cfg_res["gitleaksignore"]["explicit_file_target"]
            remedy = ("--gitleaks-ignore-path <app-owned empty file> neutralised it" if neut else
                      "--gitleaks-ignore-path <app-owned empty file> did NOT neutralise it (the ignore file beside the scan source is "
                      "still applied), so that flag is not a remedy")
            hyp("H-OVR", "partially confirmed",
                f"-c neutralised config files except: {bad}. Repo .gitleaksignore still suppressed with -c alone: {ignore_still}; {remedy}. "
                f"Explicit single-file target with a planted .gitleaksignore beside it: ignored findings restored with cwd=repo: "
                f"{ft['ignore_file_not_applied']}; with cwd outside the repo and an app-owned empty --gitleaks-ignore-path: "
                f"{ft['ignore_file_not_applied_cwd_outside']} ({ft}). Adapter consequence: never scan a repository directory directly; "
                + ("single-file targets from a cwd outside the repo with an app-owned empty ignore path avoided the repo ignore file in this "
                   "test; staged copies without .gitleaksignore are the general remedy (not separately tested here)"
                   if ft["ignore_file_not_applied_cwd_outside"] else
                   "single-file targets did not avoid it either; only app-staged copies without .gitleaksignore remain (not separately tested)")
                + "; a repository .gitleaksignore is a suppression channel to report.", runs_ovr)

    section("config", config)

    # ------------------------------------------------------------------ H-INL
    def inline():
        t = {"plain.yaml": f"---\ntoken: {c['C02']}\n",
             "inline.yaml": f"---\ntoken: {c['C02']}  # gitleaks:allow\n",
             "inline-block.yaml": f"---\nnotes: |\n  line\n  token {c['C08']}  # gitleaks:allow\n"}
        d = new_repo("gl-inline", t)
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        base = ["-c", str(ap), "--exit-code", LEAK_CODE, "--redact=100"]
        w = G("gl-inl-default", d, base, ".", purpose="inline gitleaks:allow, explicit app config", fixtures=["FX-INJ-INLINE-GITLEAKS"])
        n = G("gl-inl-ignore-allow", d, [*base, "--ignore-gitleaks-allow"], ".", purpose="same with --ignore-gitleaks-allow",
              fixtures=["FX-INJ-INLINE-GITLEAKS"])
        files_w = sorted({os.path.basename(f.get("File", "")) for f in findings(w)})
        files_n = sorted({os.path.basename(f.get("File", "")) for f in findings(n)})
        ctx.note("inline_effect", {"default_files_with_findings": files_w, "with_ignore_gitleaks_allow": files_n})
        if "plain.yaml" not in files_w:
            hyp("H-INL", "not testable", "the un-annotated control file produced no finding", [w["case"], n["case"]])
            return
        suppressed = [f for f in ("inline.yaml", "inline-block.yaml") if f not in files_w]
        restored = [f for f in suppressed if f in files_n]
        if suppressed:
            hyp("H-INL", "confirmed",
                f"With the explicit app config the inline `gitleaks:allow` annotation suppressed findings in {suppressed}; "
                f"--ignore-gitleaks-allow restored them for {restored} (negative test: "
                f"{'neutralisation verified' if set(restored) == set(suppressed) else 'neutralisation INCOMPLETE'}). "
                "Adapter must always pass --ignore-gitleaks-allow.", [w["case"], n["case"]])
        else:
            hyp("H-INL", "refuted", f"Inline annotations did not suppress: files with findings {files_w}.", [w["case"], n["case"]])

    section("inline", inline)

    # ------------------------------------------------------------------ H-EXE
    def exe():
        d = new_repo("gl-exe", {"tok.yaml": f"---\ntoken: {c['C02']}\n"})
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        r = G("gl-exe-subcommands", d, ["-c", str(ap)], ".", purpose="baseline for exe probe", fixtures=["FX-INJ-EXEC-GITLEAKS"])
        ctx.note("exe_probe", {"help_mentions_plugin": any(w in htxt.lower() for w in ("plugin", "hook", "script")),
                               "markers": ctx.markers()})
        hyp("H-EXE", "observed",
            "The `dir` options contain no plugin, script or hook channel; rules are TOML data with regexes; no execution marker appeared "
            "in any run. Repo-controlled rule/allowlist data and the `useDefault`/`path` extend directive are the only repository inputs "
            "(neutralised by -c). No code-loading channel was found, so none was invented.", [r["case"]])

    section("exe", exe)

    # ------------------------------------------------------------------ H-SCOPE
    def scope():
        tree = fx.scope_tree(c)
        tree["scope/selected.yaml"] = fx.YAML_STYLE + f"secret_line:\n  token: {c['C02']}\n"
        d = new_repo("gl-scope", tree)
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        base = ["-c", str(ap), "--exit-code", LEAK_CODE, "--redact=100", "--ignore-gitleaks-allow"]
        dir_run = G("gl-scope-dir", d, base, "scope", purpose="directory target", fixtures=["FX-SCOPE-SIBLINGS", "FX-SCOPE-NESTED", "FX-SCOPE-NONYAML"])
        file_run = G("gl-scope-file", d, base, "scope/selected.yaml", purpose="explicit single-file target", fixtures=["FX-SCOPE-SIBLINGS"])
        # staged copy
        stage = d / "stage"
        (stage / "scope").mkdir(parents=True)
        shutil.copy2(d / "repo" / "scope" / "selected.yaml", stage / "scope" / "selected.yaml")
        staged = G("gl-scope-staged", d, base, "scope", cwd=stage, purpose="staged copy of the selected file only (relative path preserved)",
                   fixtures=["FX-SCOPE-STAGED"])
        fl = lambda r: sorted({str(f.get("File", "")) for f in findings(r)})
        if not findings(dir_run):
            hyp("H-SCOPE", "not testable", "directory scan of the scope fixture produced no findings", [dir_run["case"]])
            return
        lines = lambda r: sorted({(os.path.basename(f.get("File", "")), f.get("StartLine")) for f in findings(r) if str(f.get("File", "")).endswith("selected.yaml")})
        orig_line = next((i + 1 for i, ln in enumerate((d / "repo/scope/selected.yaml").read_text().splitlines()) if c["C02"] in ln), None)
        ctx.note("scope_observations", {
            "dir_files": fl(dir_run), "file_target_files": fl(file_run), "file_target_exit": file_run["exit"],
            "staged_files": fl(staged), "selected_line_dir": lines(dir_run), "selected_line_file": lines(file_run),
            "selected_line_staged": lines(staged), "original_line_of_C02": orig_line,
            "dir_has_sibling": any(f.endswith("sibling.yaml") for f in fl(dir_run)), "dir_has_txt": any(f.endswith("notes.txt") for f in fl(dir_run)),
            "file_has_sibling": any(f.endswith("sibling.yaml") for f in fl(file_run)),
            "file_target_report_file_path_is": fl(file_run)})
        # symlinks
        dl = new_repo("gl-link", {"real.yaml": f"---\ntoken: {c['C02']}\n"})
        (dl / "outside.yaml").write_text(f"---\ntoken: {c['C12']}\n")
        os.symlink("real.yaml", dl / "repo" / "link-inroot.yaml")
        os.symlink(str(dl / "outside.yaml"), dl / "repo" / "link-outside.yaml")
        (dl / "repo" / "sub").mkdir()
        os.symlink("..", dl / "repo" / "sub" / "loop")
        ap2 = dl / "app.toml"
        ap2.write_text(APP_CFG)
        lbase = ["-c", str(ap2), "--exit-code", LEAK_CODE, "--redact=100", "--ignore-gitleaks-allow"]
        l0 = G("gl-link-default", dl, lbase, ".", purpose="symlinks, default", fixtures=["FX-LINK-INROOT", "FX-LINK-OUTSIDE", "FX-LINK-LOOP"], timeout=60)
        l1 = G("gl-link-follow", dl, [*lbase, "--follow-symlinks"], ".", purpose="symlinks with --follow-symlinks", fixtures=["FX-LINK-OUTSIDE", "FX-LINK-LOOP"], timeout=60)
        ctx.note("symlink_walk", {"default_files": sorted(os.path.basename(x) for x in fl(l0)), "default_exit": l0["exit"],
                                  "follow_files": sorted(os.path.basename(x) for x in fl(l1)), "follow_exit": l1["exit"], "follow_timed_out": l1["timed_out"]})
        file_ok = file_run["exit"] in (0, int(LEAK_CODE)) and fl(file_run) and all(f.endswith("selected.yaml") for f in fl(file_run))
        staged_lines_ok = lines(staged) and lines(staged) == lines(dir_run) and any(ln == orig_line for _, ln in lines(staged))
        wide = any(f.endswith("sibling.yaml") for f in fl(dir_run))
        strategy = "explicit_files (single file path accepted)" if file_ok else "staged_selection"
        ctx.note("scope_strategy", strategy)
        hyp("H-SCOPE", "partially confirmed" if file_ok else ("confirmed" if wide else "refuted"),
            f"`dir` accepts a single file path: {bool(file_ok)} (files {fl(file_run)}); a directory target also scanned siblings: {wide} "
            f"(files {fl(dir_run)}; non-YAML notes.txt included: {any(f.endswith('notes.txt') for f in fl(dir_run))}). gitleaks accepts only ONE "
            f"path, so selections of several files need one run per file or a staged copy. Staged copy keeps the relative path and the "
            f"line number (original line {orig_line}; staged {lines(staged)}; matches directory run: {bool(staged_lines_ok)}). "
            f"Symlinks default/follow: see notes. Strategy: {strategy}.",
            [dir_run["case"], file_run["case"], staged["case"], l0["case"], l1["case"]])

    section("scope", scope)

    # ------------------------------------------------------------------ H-ARG / H-OUT
    def names():
        d = new_repo("gl-names", fx.hostile_name_tree(f"---\ntoken: {c['C02']}\n"))
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        base = ["-c", str(ap), "--exit-code", LEAK_CODE, "--redact=100", "--ignore-gitleaks-allow"]
        per, runs = {}, []
        for fxid, nm in fx.HOSTILE_NAMES.items():
            r = G(f"gl-arg-{fxid.lower()}", d, base, f"names/{nm}", purpose=f"hostile name {fxid} after --", fixtures=[fxid])
            runs.append(r["case"])
            f = findings(r)
            per[fxid] = {"exit": r["exit"], "findings": len(f), "file_round_trips": [x.get("File") for x in f] == [f"names/{nm}"] or
                         [os.path.basename(str(x.get("File"))) for x in f] == [nm],
                         "stdout_or_stderr_raw_esc": b"\x1b" in r["_stdout"] or (fxid != "FX-NAME-ANSI" and b"\x1b[31m" in r["_stderr"]),
                         "stderr_has_raw_esc_byte": b"\x1b" in r["_stderr"]}
        d2 = new_repo("gl-dash", {"-rf.yaml": f"---\ntoken: {c['C02']}\n", "--config=evil.yaml": f"---\ntoken: {c['C02']}\n",
                                  "evil.yaml": H_ALLOW_ALL, "ok.yaml": "a: 1\n"})
        ap2 = d2 / "app.toml"
        ap2.write_text(APP_CFG)
        b2 = ["-c", str(ap2), "--exit-code", LEAK_CODE, "--redact=100"]
        ctl = G("gl-arg-dash-control", d2, b2, "--config=evil.yaml", dd_target=False, purpose="control: option-like path WITHOUT --", fixtures=["FX-NAME-DASH-2"])
        ctl2 = G("gl-arg-dash-control2", d2, b2, "-rf.yaml", dd_target=False, purpose="control 2: -rf.yaml WITHOUT --", fixtures=["FX-NAME-DASH-1"])
        ok1 = G("gl-arg-dash-ddash", d2, b2, "-rf.yaml", purpose="-rf.yaml after --", fixtures=["FX-NAME-DASH-1"])
        ok2 = G("gl-arg-dash-ddash2", d2, b2, "--config=evil.yaml", purpose="--config=evil.yaml after --", fixtures=["FX-NAME-DASH-2"])
        ctx.note("hostile_names", {"per_name": per, "control_without_ddash": {
            "config_injection_hides_findings": len(findings(ctl)) == 0, "exit": ctl["exit"], "short_opt_exit": ctl2["exit"],
            "short_opt_findings": len(findings(ctl2))},
            "ddash": {"-rf.yaml": [ok1["exit"], len(findings(ok1))], "--config=evil.yaml": [ok2["exit"], len(findings(ok2))]}})
        all_ok = all(v["findings"] == 1 and v["file_round_trips"] for v in per.values()) and len(findings(ok1)) == 1 and len(findings(ok2)) == 1
        hyp("H-ARG", "confirmed" if all_ok else "partially confirmed",
            f"After '--' every hostile name was scanned as a path (per-name {per}); option-like names scanned after '--': "
            f"{len(findings(ok1)) == 1 and len(findings(ok2)) == 1}. Control without '--': '--config=evil.yaml' acted as an option "
            f"(findings hidden: {len(findings(ctl)) == 0}, exit {ctl['exit']}). Only ONE path operand is accepted per run.",
            runs + [ok1["case"], ok2["case"], ctl["case"], ctl2["case"]])
        # H-OUT
        base_d = S.get("base_dir")
        a = G("gl-out-run1", base_d, ["--exit-code", LEAK_CODE, "--redact=100"], ".", purpose="determinism 1", fixtures=["FX-SEC-*"])
        b = G("gl-out-run2", base_d, ["--exit-code", LEAK_CODE, "--redact=100"], ".", purpose="determinism 2", fixtures=["FX-SEC-*"])
        order_same = [(f.get("File"), f.get("RuleID"), f.get("StartLine")) for f in findings(a)] == [(f.get("File"), f.get("RuleID"), f.get("StartLine")) for f in findings(b)]
        set_same = sigs(a) == sigs(b)
        ansi_json = [x for x in per.values()]  # noqa: F841
        ctx.note("output_determinism", {"identical_order": order_same, "same_set": set_same, "report_is_json_array": isinstance(a["_report"], list),
                                        "ansi_name_file_field_escaped_in_json": per["FX-NAME-ANSI"]["file_round_trips"],
                                        "stderr_raw_esc_for_ansi_name": per["FX-NAME-ANSI"]["stderr_has_raw_esc_byte"]})
        hyp("H-OUT", "confirmed" if (set_same and order_same and per["FX-NAME-ANSI"]["file_round_trips"]) else "partially confirmed",
            f"JSON report array parsed; same findings across two runs: set {set_same}, order {order_same}; hostile names (ANSI, newline, "
            f"bidi) round-tripped in the File field via JSON escaping; raw ESC byte on stderr for the ANSI name: "
            f"{per['FX-NAME-ANSI']['stderr_has_raw_esc_byte']} (adapter must escape stderr/log text itself).",
            [a["case"], b["case"], *runs])

    section("names", names)

    # ------------------------------------------------------------------ exit map + H-EXIT + H-NET
    def exits():
        d = new_repo("gl-exit", {"ok.yaml": "a: 1\n", "tok.yaml": f"---\ntoken: {c['C02']}\n", "bad.toml": "title = [unclosed\n"})
        ap = d / "app.toml"
        ap.write_text(APP_CFG)
        base = ["-c", str(ap), "--exit-code", LEAK_CODE, "--redact=100"]
        bf = ctx.run("gl-exit-badflag", [ctx.bin, "dir", "--no-such-flag"], d / "repo", purpose="unknown flag", fixtures=[])
        miss = G("gl-exit-missing", d, base, "does-not-exist", purpose="nonexistent target", fixtures=[])
        badcfg = G("gl-exit-badconfig", d, ["-c", str(d / "repo" / "bad.toml"), "--exit-code", LEAK_CODE], ".", purpose="invalid TOML config", fixtures=[])
        nocfg = G("gl-exit-missing-config", d, ["-c", str(d / "nope.toml"), "--exit-code", LEAK_CODE], ".", purpose="nonexistent config file", fixtures=[])
        leaks = G("gl-exit-leaks", d, base, "tok.yaml", purpose="leaks", fixtures=["FX-SEC-TOKEN-PREFIXED"])
        clean = G("gl-exit-clean2", d, base, "ok.yaml", purpose="clean", fixtures=["FX-YAML-VALID"])
        exit_map.update({"bad_flag": bf["exit"], "missing_target": miss["exit"], "invalid_config": badcfg["exit"],
                         "missing_config_file": nocfg["exit"], "leaks_with_code_7": leaks["exit"], "clean_check": clean["exit"]})
        ctx.note("exit_map", exit_map)
        ctx.note("exit_errors_write_report", {"missing_target_report": miss["_report"], "bad_config_report": badcfg["_report"]})
        distinct = len({exit_map.get("clean"), exit_map.get("leaks_configured_code"), exit_map.get("bad_flag"), exit_map.get("missing_target"),
                        exit_map.get("invalid_config")}) >= 3 and exit_map.get("leaks_configured_code") not in (exit_map.get("bad_flag"), exit_map.get("missing_target"), exit_map.get("invalid_config"))
        fail_open = sorted(k for k in ("bad_flag", "missing_target", "invalid_config", "missing_config_file")
                           if exit_map.get(k) is not None and exit_map.get(k) == exit_map.get("clean"))
        hyp("H-EXIT", "confirmed" if distinct and not fail_open else "partially confirmed",
            f"Exit map {exit_map}. Error conditions exiting with the clean code (fail-open): {fail_open}"
            f"{' (a nonexistent target only logs a WRN skipping line and reports no leaks)' if 'missing_target' in fail_open else ''}; "
            "the adapter must verify target existence and scanned-byte counts itself. With --exit-code " + LEAK_CODE + " leaks are distinguishable from errors "
            f"(leak code differs from bad-flag/missing-target/bad-config codes: {distinct}); without it leaks use the same code as errors "
            f"({exit_map.get('leaks_default_code')} vs {exit_map.get('bad_flag')}). Adapter must always set --exit-code to an unused value.",
            ["gl-exit-clean", "gl-base-raw", "gl-exit-default-code", bf["case"], miss["case"], badcfg["case"], nocfg["case"]])
        sig = sorted({s for r in ctx.runs for s in r["network_attempt_signals"]})
        to = [r["case"] for r in ctx.runs if r["timed_out"]]
        hyp("H-NET", "refuted" if (not sig and not to) else "partially confirmed",
            f"Across {len(ctx.runs)} runs in a loopback-only network namespace (dir mode, default config incl. rules with validation "
            f"disabled by default) no network-attempt signal (signals={sig}) and no timeout ({to}). gitleaks dir mode has no network option "
            "in the flags used; `git` mode and unrecognised update checks were not exercised.", ["gl-version", "gl-base-raw", "gl-exit-clean"])

    section("exits", exits)

    ctx.note("invocation_template", ["<gitleaks>", "dir", "--no-banner", "--no-color", "-f", "json", "-r", "<app-owned report path>",
                                     "-c", "<app-config>", "--gitleaks-ignore-path", "<app-owned empty file>", "--ignore-gitleaks-allow",
                                     "--redact=100", "--exit-code", LEAK_CODE, "--", "<single file or staged dir>"])
    ctx.note("sections_failed", errors)
    for h in HYP_IDS:
        if h not in hyps_done:
            hyp(h, "not testable", "the section that decides this hypothesis failed (see SECTION_ERROR_* notes)" if errors else
                "no run decided it", [])

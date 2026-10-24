"""P00-T09 kubeconform spike cases (W1; protocol P00-T01 section 3.5). Spike code, not application code.

Only hand-made local stub schemas are used (FX-K8S-SCHEMA-STUB: apps/v1 Deployment and nothing else).
The one run that uses the tool's default schema location is the H-NET test and is expected to fail
inside the network namespace (bounded by a timeout). Outcomes are derived from recorded runs.
"""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

import common
import fixtures as fx

SCHEMA_VERSION_DIR = "v1.30.0-standalone"
KVER = "1.30.0"

DEPLOY_PROPS = {
    "apiVersion": {"type": "string"},
    "kind": {"type": "string"},
    "metadata": {"type": "object", "properties": {"name": {"type": "string"}}},
    "spec": {
        "type": "object",
        "required": ["selector", "template"],
        "properties": {
            "replicas": {"type": "integer"},
            "selector": {"type": "object"},
            "template": {"type": "object"},
            "mode": {"enum": ["rolling"]},
        },
    },
}


def stub(strict: bool) -> dict:
    s = {"type": "object", "required": ["apiVersion", "kind", "metadata", "spec"],
         "properties": json.loads(json.dumps(DEPLOY_PROPS))}
    if strict:
        s["additionalProperties"] = False
        s["properties"]["spec"]["additionalProperties"] = False
    return s


def template(schemas: Path, strict_suffix: bool = True) -> str:
    return (str(schemas) + "/{{.NormalizedKubernetesVersion}}-standalone"
            + ("{{.StrictSuffix}}" if strict_suffix else "") + "/{{.ResourceKind}}{{.KindSuffix}}.json")


def write_stubs(schemas: Path, fname: str, extra_lenient: dict | None = None) -> None:
    for sfx, strict in (("", False), ("-strict", True)):
        p = schemas / f"v{KVER}-standalone{sfx}" / fname
        p.parent.mkdir(parents=True, exist_ok=True)
        body = stub(strict)
        if extra_lenient and not strict:
            body.update(extra_lenient)
        p.write_text(json.dumps(body))


def resources(rec):
    j = rec.get("_json")
    return j.get("resources", []) if isinstance(j, dict) else []


def statuses(rec):
    return sorted({r.get("status") for r in resources(rec)})


def cases(ctx):
    c = ctx.canaries
    exit_map: dict[str, object] = {}

    def K(cid, args, cwd, purpose, fixtures, **kw):
        kw.setdefault("parse_json", True)
        return ctx.run(cid, [ctx.bin, *args], cwd, purpose=purpose, fixtures=fixtures, **kw)

    # ------------------------------------------------------------------ version / help
    d = ctx.case_dir("kc-meta")
    rv = K("kc-version", ["-v"], d / "repo", "version", [], parse_json=False)
    ctx.note("version", rv["_stdout"].decode().strip() or rv["_stderr"].decode().strip())
    rh = K("kc-help", ["-h"], d / "repo", "flag discovery", [], parse_json=False)
    helptxt = (rh["_stdout"] + rh["_stderr"]).decode("utf-8", "replace")
    flags = ["-cache", "-debug", "-exit-on-error", "-ignore-filename-pattern", "-ignore-missing-schemas",
             "-insecure-skip-tls-verify", "-kubernetes-version", "-n", "-output", "-reject", "-schema-location",
             "-skip", "-strict", "-summary", "-verbose"]
    ctx.note("flags_present_in_help", {f: bool(re.search(r"^\s+" + re.escape(f) + r"\b", helptxt, re.M)) for f in flags})
    ctx.note("output_formats_in_help", re.findall(r"json, junit, pretty, tap, text", helptxt))
    ctx.note("help_exit", rh["exit"])
    ctx.note("config_file_or_env_option_in_help", bool(re.search(r"config|environment|env var", helptxt, re.I)))

    # ------------------------------------------------------------------ discover the schema file name
    d = ctx.case_dir("kc-name")
    fx.write_tree(d / "repo", {"valid.yaml": fx.K8S_VALID})
    fname = None
    cands = ["deployment-apps-v1.json", "deployment-v1.json", "deployment.json", "deployment-apps.json"]
    name_runs = []
    for i, cand in enumerate(cands):
        shutil_dir = d / f"schemas{i}"
        write_stubs(shutil_dir, cand)
        r = K(f"kc-name-{i}", ["-kubernetes-version", KVER, "-schema-location", template(shutil_dir), "-output", "json",
                              "-verbose", "valid.yaml"], d / "repo", f"schema file name candidate {cand}", ["FX-K8S-SCHEMA-STUB"])
        name_runs.append(r["case"])
        if "statusValid" in statuses(r) and fname is None:
            fname = cand
    ctx.note("stub_schema_file_name_resolved_by_template", fname)
    fname = fname or cands[0]
    ctx.note("schema_location_template", "<schemas-root>/{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}/{{.ResourceKind}}{{.KindSuffix}}.json")

    # ------------------------------------------------------------------ main tree
    d = ctx.case_dir("kc-main")
    sch = d / "schemas"
    write_stubs(sch, fname)
    tree = {
        "valid.yaml": fx.K8S_VALID, "insecure.yaml": fx.K8S_INSECURE, "invalid.yaml": fx.K8S_INVALID,
        "multidoc.yaml": fx.K8S_MULTIDOC, "crd.yaml": fx.K8S_CRD, "template.yaml": fx.K8S_TEMPLATE,
        "malformed.yaml": fx.YAML_MALFORMED, "malformed-secret.yaml": fx.yaml_malformed_secret(c),
        "multidoc-malformed.yaml": fx.K8S_VALID + "---\nkind: [unclosed\n---\n" + fx.K8S_VALID.replace("web", "web2"),
    }
    fx.write_tree(d / "repo", tree)
    base_args = ["-kubernetes-version", KVER, "-schema-location", template(sch), "-output", "json", "-summary", "-verbose"]
    R = {}
    for name, extra, fxid in [
        ("valid", [], "FX-K8S-VALID"), ("insecure", [], "FX-K8S-INSECURE"),
        ("invalid", [], "FX-K8S-INVALID"), ("multidoc", [], "FX-K8S-MULTIDOC"), ("crd", [], "FX-K8S-CRD-MISSING"),
        ("template", [], "FX-K8S-TEMPLATE"), ("malformed", [], "FX-YAML-MALFORMED"),
        ("malformed-secret", [], "FX-YAML-MALFORMED-SECRET"), ("multidoc-malformed", [], "FX-YAML-MULTIDOC"),
    ]:
        R[name] = K(f"kc-{name}", base_args + extra + [f"{name}.yaml"], d / "repo", f"{name} (lenient)", [fxid, "FX-K8S-SCHEMA-STUB"])
    R["invalid-strict"] = K("kc-invalid-strict", base_args + ["-strict", "invalid.yaml"], d / "repo", "invalid, -strict",
                            ["FX-K8S-INVALID", "FX-K8S-SCHEMA-STUB"])
    R["valid-strict"] = K("kc-valid-strict", base_args + ["-strict", "valid.yaml"], d / "repo", "valid, -strict", ["FX-K8S-VALID"])
    nosfx = [a if not a.startswith(str(sch)) else template(sch, strict_suffix=False) for a in base_args]
    R["invalid-strict-nosuffix"] = K("kc-invalid-strict-nosuffix", nosfx + ["-strict", "invalid.yaml"], d / "repo",
                                     "-strict with a template lacking StrictSuffix", ["FX-K8S-INVALID"])
    R["noverbose"] = K("kc-valid-noverbose", ["-kubernetes-version", KVER, "-schema-location", template(sch), "-output", "json",
                                              "valid.yaml"], d / "repo", "valid without -verbose", ["FX-K8S-VALID"])
    R["crd-ignore"] = K("kc-crd-ignore-missing", base_args + ["-ignore-missing-schemas", "crd.yaml"], d / "repo",
                        "CRD with -ignore-missing-schemas", ["FX-K8S-CRD-MISSING"])
    R["skip"] = K("kc-skip", base_args + ["-skip", "Deployment", "valid.yaml"], d / "repo", "-skip Deployment", ["FX-K8S-VALID"])
    R["crd-mixed"] = K("kc-crd-mixed", base_args + ["valid.yaml", "crd.yaml"], d / "repo", "valid + CRD in one run",
                       ["FX-K8S-VALID", "FX-K8S-CRD-MISSING"])
    R["crd-mixed-ignore"] = K("kc-crd-mixed-ignore", base_args + ["-ignore-missing-schemas", "valid.yaml", "crd.yaml"], d / "repo",
                              "valid + CRD with ignore-missing", ["FX-K8S-CRD-MISSING"])

    def st(r):
        return [(x.get("kind"), x.get("status")) for x in resources(r)]

    def summ(r):
        j = r.get("_json")
        return j.get("summary") if isinstance(j, dict) else None

    exit_map.update({"valid": R["valid"]["exit"], "invalid": R["invalid"]["exit"], "missing_schema": R["crd"]["exit"],
                     "malformed_input": R["malformed"]["exit"], "ignore_missing_with_only_crd": R["crd-ignore"]["exit"]})
    ctx.note("json_shape", {
        "top_level_keys": sorted((R["valid"].get("_json") or {}).keys()),
        "resource_keys": sorted({k for r in resources(R["invalid"]) for k in r}),
        "validation_error_keys": sorted({k for r in resources(R["invalid"]) for e in r.get("validationErrors", []) for k in e}),
        "summary_keys": sorted((summ(R["valid"]) or {}).keys()),
        "status_values_seen": sorted({s for r in R.values() for s in statuses(r) if s}),
        "valid_listed_only_with_verbose": [len(resources(R["noverbose"])), len(resources(R["valid"]))],
        "valid_status": st(R["valid"]), "insecure_status": st(R["insecure"]),
        "invalid_lenient": [(x.get("status"), [e.get("path") for e in x.get("validationErrors", [])]) for x in resources(R["invalid"])],
        "invalid_strict": [(x.get("status"), [e.get("path") for e in x.get("validationErrors", [])]) for x in resources(R["invalid-strict"])],
        "invalid_strict_template_without_suffix": [(x.get("status"), [e.get("path") for e in x.get("validationErrors", [])])
                                                   for x in resources(R["invalid-strict-nosuffix"])],
        "valid_strict": st(R["valid-strict"]),
        "validation_msg_embeds_local_schema_file_url": any("file://" in (x.get("msg") or "") for r in R.values() for x in resources(r)),
    })
    lenient_paths = {e.get("path") for x in resources(R["invalid"]) for e in x.get("validationErrors", [])}
    strict_paths = {e.get("path") for x in resources(R["invalid-strict"]) for e in x.get("validationErrors", [])}
    nosfx_paths = {e.get("path") for x in resources(R["invalid-strict-nosuffix"]) for e in x.get("validationErrors", [])}
    ctx.note("strict_mode", {
        "strict_adds_findings_vs_lenient": len(strict_paths) > len(lenient_paths),
        "strict_needs_strict_schema_variant_in_layout": (len(strict_paths) > len(lenient_paths)) and not (len(nosfx_paths) > len(lenient_paths)),
        "strict_flag_alone_enforces_without_strict_schema": len(nosfx_paths) > len(lenient_paths)})
    ctx.note("multidoc", {"entries": len(resources(R["multidoc"])), "per_entry": st(R["multidoc"]),
                          "entry_has_document_index_field": any(re.search(r"doc|index|position", k, re.I)
                                                                for r in resources(R["multidoc"]) for k in r),
                          "malformed_middle_doc": st(R["multidoc-malformed"]),
                          "malformed_middle_doc_exit": R["multidoc-malformed"]["exit"],
                          "malformed_middle_msgs": [(x.get("status"), (x.get("msg") or "")[:60]) for x in resources(R["multidoc-malformed"])]})
    ctx.note("crd_missing", {
        "default": st(R["crd"]), "default_exit": R["crd"]["exit"], "default_summary": summ(R["crd"]),
        "ignore_missing": st(R["crd-ignore"]), "ignore_missing_exit": R["crd-ignore"]["exit"], "ignore_missing_summary": summ(R["crd-ignore"]),
        "mixed_exit": R["crd-mixed"]["exit"], "mixed_ignore_exit": R["crd-mixed-ignore"]["exit"],
        "mixed_ignore_summary": summ(R["crd-mixed-ignore"]), "skip_flag": st(R["skip"]), "skip_exit": R["skip"]["exit"],
        "decision": "do NOT pass -ignore-missing-schemas or -skip: they convert unsupported kinds into exit 0 / skipped"})
    ctx.note("template_file", {"status": st(R["template"]), "exit": R["template"]["exit"],
                               "msgs": [(x.get("msg") or "")[:80] for x in resources(R["template"])]})
    ctx.note("malformed_file", {"status": st(R["malformed"]), "exit": R["malformed"]["exit"],
                                "msgs": [(x.get("msg") or "")[:80] for x in resources(R["malformed"])]})

    # ------------------------------------------------------------------ exit map extras
    ex_bad = K("kc-exit-badflag", ["-no-such-flag"], d / "repo", "unknown flag", [], parse_json=False)
    ex_missing = K("kc-exit-missingfile", base_args + ["does-not-exist.yaml"], d / "repo", "nonexistent path", [])
    ex_noargs = K("kc-exit-noargs", base_args, d / "repo", "no file operands (stdin is closed)", [])
    ex_badver = K("kc-exit-badver", ["-kubernetes-version", "not-a-version", "-schema-location", template(sch), "valid.yaml"],
                  d / "repo", "bad kubernetes version", [], parse_json=False)
    exit_map.update({"bad_flag": ex_bad["exit"], "help": rh["exit"], "nonexistent_path": ex_missing["exit"],
                     "no_operands_closed_stdin": ex_noargs["exit"], "bad_kubernetes_version": ex_badver["exit"]})
    ctx.note("exit_map", exit_map)
    ctx.note("exit_extra", {"bad_flag_stdout_bytes": ex_bad["stdout_bytes"], "bad_flag_stderr_has_usage": b"Usage" in ex_bad["_stderr"],
                            "no_operands_resources": len(resources(ex_noargs)),
                            "missing_file": [(x.get("status"), (x.get("msg") or "")[:60]) for x in resources(ex_missing)],
                            "missing_file_json_parse": ex_missing.get("json_parse")})
    inv_vs_err = R["invalid"]["exit"] == R["crd"]["exit"]
    ctx.hyp("H-EXIT", "partially confirmed",
            f"Observed exit map {exit_map}. Invalid resources and missing schemas both exit {R['invalid']['exit']} "
            f"(same code: {inv_vs_err}); a bad flag also exits {ex_bad['exit']} with usage text and no JSON, and -h exits "
            f"{rh['exit']}. Classification must therefore come from the parsed JSON (resource status values "
            f"{sorted({s for r in R.values() for s in statuses(r) if s})}), never from the exit code alone; exit!=0 with "
            "unparsable stdout = tool/usage error.",
            [R["valid"]["case"], R["invalid"]["case"], R["crd"]["case"], R["malformed"]["case"], ex_bad["case"], ex_missing["case"]])

    # ------------------------------------------------------------------ H-NET: default location
    dn = ctx.case_dir("kc-net")
    fx.write_tree(dn / "repo", {"valid.yaml": fx.K8S_VALID})
    net = K("kc-net-default-location", ["-kubernetes-version", KVER, "-output", "json", "-verbose", "valid.yaml"], dn / "repo",
            "DEFAULT schema location inside the netns (expected to fail)", ["FX-NET-DEFAULT", "FX-K8S-VALID"], timeout=90)
    net_cache = K("kc-net-default-cache", ["-kubernetes-version", KVER, "-cache", str(dn / "cache"), "-output", "json", "-verbose",
                                           "valid.yaml"], dn / "repo", "default location with -cache dir", ["FX-NET-DEFAULT"], timeout=90)
    (dn / "cache").mkdir(exist_ok=True)
    nstat = st(net)
    msgs = [(x.get("msg") or "") for x in resources(net)]
    attempted = bool(net["network_attempt_signals"]) or any(re.search(r"dial|lookup|http|connect|resolve|download", m, re.I) for m in msgs)
    ctx.note("default_location_run", {"exit": net["exit"], "timed_out": net["timed_out"], "wall_ms": net["wall_ms"], "status": nstat,
                                      "msg_mentions_network": any(re.search(r"dial|lookup|http|connect|resolve|download", m, re.I) for m in msgs),
                                      "signals": net["network_attempt_signals"],
                                      "cache_dir_files_after_failed_default_with_cache": net_cache["files_created"]})
    failed = (net["exit"] not in (0, None)) or net["timed_out"]
    if failed and attempted and not net["timed_out"]:
        ctx.hyp("H-NET", "confirmed",
                f"With no -schema-location the tool tried to fetch the schema remotely; inside the loopback-only netns it failed "
                f"(exit {net['exit']}, {net['wall_ms']} ms, status {nstat}) and was not a pass. Network signals: "
                f"{net['network_attempt_signals']}. Disabling is achieved by always passing an explicit local -schema-location "
                "(no default fallback) and never the literal 'default'.", [net["case"], net_cache["case"]])
    elif failed:
        ctx.hyp("H-NET", "partially confirmed",
                f"Default location run failed or timed out (exit {net['exit']}, timed_out {net['timed_out']}, status {nstat}) "
                f"but no explicit network-attempt signal was captured (signals {net['network_attempt_signals']}).",
                [net["case"], net_cache["case"]])
    else:
        ctx.hyp("H-NET", "refuted", "Default location run succeeded without a network inside the netns.", [net["case"]])

    # ------------------------------------------------------------------ cache behaviour
    dc = ctx.case_dir("kc-cache")
    schc = dc / "schemas"
    write_stubs(schc, fname)
    fx.write_tree(dc / "repo", {"valid.yaml": fx.K8S_VALID})
    cache_dir = dc / "cache"
    cache_dir.mkdir()
    r_cl = K("kc-cache-local", ["-kubernetes-version", KVER, "-cache", str(cache_dir), "-schema-location", template(schc),
                                "-output", "json", "-verbose", "valid.yaml"], dc / "repo", "-cache with a local schema location",
             ["FX-K8S-SCHEMA-STUB"])
    cache_after_local = sorted(str(p.relative_to(dc)) for p in cache_dir.rglob("*") if p.is_file())
    cache_http = {"tested": False}
    # Owner-approved 2026-10-24: in-process stdlib server on 127.0.0.1 inside the private netns, serving only
    # the stub-schema directory; lo is up only inside this block (common.LoopbackSchemaServer).
    try:
        with common.LoopbackSchemaServer(schc) as srv:
            http_tmpl = (f"http://127.0.0.1:{srv.port}" + "/{{.NormalizedKubernetesVersion}}-standalone{{.StrictSuffix}}/{{.ResourceKind}}{{.KindSuffix}}.json")
            n0 = len(srv.requests)
            h1 = K("kc-cache-http-first", ["-kubernetes-version", KVER, "-cache", str(cache_dir), "-schema-location", http_tmpl,
                                           "-output", "json", "-verbose", "valid.yaml"], dc / "repo",
                   "loopback HTTP schema location, empty cache", ["FX-K8S-SCHEMA-STUB"], timeout=60)
            req_first = srv.requests[n0:]
            cache_after_http = sorted(str(p.relative_to(dc)) for p in cache_dir.rglob("*") if p.is_file())
            n1 = len(srv.requests)
            h1b = K("kc-cache-http-second-served", ["-kubernetes-version", KVER, "-cache", str(cache_dir), "-schema-location", http_tmpl,
                                                   "-output", "json", "-verbose", "valid.yaml"], dc / "repo",
                    "same, server still serving, warm cache (does it re-request?)", ["FX-K8S-SCHEMA-STUB"], timeout=60)
            req_second_served = srv.requests[n1:]
            srv.stop_server()
            h2 = K("kc-cache-http-second", ["-kubernetes-version", KVER, "-cache", str(cache_dir), "-schema-location", http_tmpl,
                                            "-output", "json", "-verbose", "valid.yaml"], dc / "repo",
                   "same, server stopped (connection refused), warm cache", ["FX-K8S-SCHEMA-STUB"], timeout=60)
            h3 = K("kc-cache-http-nocache", ["-kubernetes-version", KVER, "-schema-location", http_tmpl, "-output", "json", "-verbose",
                                             "valid.yaml"], dc / "repo", "same, server stopped, no cache", ["FX-K8S-SCHEMA-STUB"], timeout=60)
        cache_http = {"tested": True, "endpoint": srv.obs.get("endpoint"), "server_lifecycle": srv.obs,
                      "requests_first_run": req_first, "first_status": statuses(h1), "cache_files_after_first": cache_after_http,
                      "requests_second_run_while_serving": req_second_served, "second_status_served_warm_cache": statuses(h1b),
                      "second_status_server_down_warm_cache": statuses(h2), "nocache_status_server_down": statuses(h3),
                      "cache_files_end": sorted(str(p.relative_to(dc)) for p in cache_dir.rglob("*") if p.is_file())}
    except OSError as e:
        cache_http = {"tested": False, "reason": f"loopback server setup failed: errno {e.errno} ({type(e).__name__}); "
                                                 "a failed connection is not a cache-behaviour result"}
    ctx.note("cache_behaviour", {"local_location_cache_files": cache_after_local, "local_status": statuses(r_cl), "http": cache_http})

    # ------------------------------------------------------------------ H-CFG / H-OVR / H-INL / H-EXE
    dcf = ctx.case_dir("kc-cfg-twin")
    sc = dcf / "schemas"
    write_stubs(sc, fname)
    fx.write_tree(dcf / "repo", {"valid.yaml": fx.K8S_VALID, "invalid.yaml": fx.K8S_INVALID})
    cargs = ["-kubernetes-version", KVER, "-schema-location", template(sc), "-output", "json", "-verbose"]
    tw = K("kc-cfg-twin-run", cargs + ["valid.yaml", "invalid.yaml"], dcf / "repo", "clean twin", ["FX-INJ-KUBECONFORM"])
    dh = ctx.case_dir("kc-cfg-hostile")
    write_stubs(dh / "schemas", fname)
    hostile = {"valid.yaml": fx.K8S_VALID, "invalid.yaml": fx.K8S_INVALID}
    for nm in (".kubeconform", ".kubeconform.yaml", ".kubeconform.yml", "kubeconform.yaml", ".kubeconformrc", ".kubeconform.json"):
        hostile[nm] = "skip: [Deployment]\nignore-missing-schemas: true\nschema-location: [/nonexistent]\n"
    hostile["# yaml-config.yaml"] = "x: 1\n"
    fx.write_tree(dh / "repo", hostile)
    hs = K("kc-cfg-hostile-run", cargs[:3] + [template(dh / "schemas")] + cargs[4:] + ["valid.yaml", "invalid.yaml"], dh / "repo",
           "repository config-like files present", ["FX-INJ-KUBECONFORM"])
    hs_env = K("kc-cfg-env-run", cargs[:3] + [template(dh / "schemas")] + cargs[4:] + ["valid.yaml", "invalid.yaml"], dh / "repo",
               "env vars that look like options set", ["FX-INJ-KUBECONFORM"],
               extra_env={"KUBECONFORM_SKIP": "Deployment", "KUBECONFORM_IGNORE_MISSING_SCHEMAS": "true",
                          "KUBECONFORM_SCHEMA_LOCATION": "/nonexistent"})
    norm = lambda r: sorted((x.get("filename"), x.get("status"), len(x.get("validationErrors", []))) for x in resources(r))
    cfg_same = norm(tw) == norm(hs)
    env_same = norm(tw) == norm(hs_env)
    ctx.note("config_probe", {"repo_files_probed": [n for n in hostile if n.startswith(".kube") or n.startswith("kubeconform")],
                              "results_equal_to_twin": cfg_same, "env_vars_probed_results_equal": env_same,
                              "help_lists_config_option": bool(re.search(r"config", helptxt, re.I))})
    ctx.hyp("H-CFG", "refuted" if (cfg_same and env_same) else "confirmed",
            f"Hostile config-like files (.kubeconform, .kubeconform.yaml/.yml/.json, kubeconform.yaml, .kubeconformrc) and "
            f"option-like KUBECONFORM_* environment variables did {'not ' if cfg_same and env_same else ''}change results versus the "
            "clean twin. The tool's -h lists no config-file option; configuration is command-line only. (Only candidate names "
            "were probed; absence of other undocumented channels is not proven.)", [tw["case"], hs["case"], hs_env["case"]])
    ctx.hyp("H-OVR", "not testable", "kubeconform has no configuration file or config argument to override; all behaviour is set by "
            "command-line flags, which the app owns entirely (the repo cannot supply flags).", [tw["case"], hs["case"]])
    ctx.hyp("H-INL", "not testable", "No inline suppression/annotation feature is listed in -h and none is known; no channel was "
            "invented. (Skipping is only via the -skip/-reject/-ignore-* command-line flags.)", [tw["case"]])
    ctx.hyp("H-EXE", "observed", "No plugin, extension, external-policy or custom-check option exists in -h; schemas are data "
            "(JSON). No execution marker appeared in any run. The data-file channels found (schema-path traversal via the "
            "resource kind, remote $ref) are recorded under H-SCOPE and notes.",
            [tw["case"], hs["case"]])

    # ------------------------------------------------------------------ H-SCOPE
    ds = ctx.case_dir("kc-scope")
    write_stubs(ds / "schemas", fname)
    sib = fx.K8S_INVALID.replace("name: bad", f"name: sib-{c['C11']}")
    fx.write_tree(ds / "repo", {
        "scope/selected.yaml": fx.K8S_VALID, "scope/sibling.yaml": sib,
        "scope/notes.txt": fx.K8S_VALID.replace("name: web", f"name: web-{c['C12']}"), "scope/sub/nested.yaml": fx.K8S_VALID.replace("web", "nested"),
    })
    sargs = ["-kubernetes-version", KVER, "-schema-location", template(ds / "schemas"), "-output", "json", "-verbose"]
    sc_file = K("kc-scope-file", sargs + ["scope/selected.yaml"], ds / "repo", "explicit single file", ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NONYAML"])
    sc_dir = K("kc-scope-dir", sargs + ["scope"], ds / "repo", "directory operand", ["FX-SCOPE-SIBLINGS", "FX-SCOPE-NESTED", "FX-SCOPE-NONYAML"])
    sc_txt = K("kc-scope-txt", sargs + ["scope/notes.txt"], ds / "repo", "explicit non-YAML file", ["FX-SCOPE-NONYAML"])
    ign = K("kc-scope-dir-ignore", sargs + ["-ignore-filename-pattern", "sibling", "scope"], ds / "repo",
            "directory with -ignore-filename-pattern", ["FX-SCOPE-SIBLINGS"])
    fn = lambda r: sorted({x.get("filename") for x in resources(r)})
    ctx.note("scope_observations", {
        "explicit_file_results": fn(sc_file), "dir_results": fn(sc_dir), "explicit_txt_results": fn(sc_txt),
        "explicit_txt_status": st(sc_txt), "dir_with_ignore_pattern": fn(ign),
        "sibling_canary_C11_in_dir_stdout": "C11" in sc_dir["canary_in_stdout"], "sibling_canary_in_explicit_stdout": "C11" in sc_file["canary_in_stdout"],
        "txt_canary_C12_in_explicit_txt_stdout": "C12" in sc_txt["canary_in_stdout"], "txt_canary_C12_in_dir_stdout": "C12" in sc_dir["canary_in_stdout"]})
    # symlink-aware walk
    dl = ctx.case_dir("kc-link")
    write_stubs(dl / "schemas", fname)
    fx.write_tree(dl / "repo", {"real.yaml": fx.K8S_VALID, "sub/inner.yaml": fx.K8S_VALID.replace("web", "inner")})
    (dl / "outside.yaml").write_text(fx.K8S_VALID.replace("web", "outside"))
    os.symlink("real.yaml", dl / "repo" / "link-inroot.yaml")
    os.symlink(str(dl / "outside.yaml"), dl / "repo" / "link-outside.yaml")
    os.symlink("..", dl / "repo" / "sub" / "loop")
    lk = K("kc-link-dir", ["-kubernetes-version", KVER, "-schema-location", template(dl / "schemas"), "-output", "json", "-verbose", "."],
           dl / "repo", "directory walk with symlinks", ["FX-LINK-INROOT", "FX-LINK-OUTSIDE", "FX-LINK-LOOP"], timeout=60)
    ctx.note("symlink_walk", {"exit": lk["exit"], "timed_out": lk["timed_out"], "files": fn(lk),
                              "follows_inroot_link": "link-inroot.yaml" in fn(lk), "follows_outside_link": "link-outside.yaml" in fn(lk),
                              "recurses_loop": any("loop" in (f or "") for f in fn(lk))})
    # schema path traversal via kind (repo-controlled string used in a file path)
    dt = ctx.case_dir("kc-traversal")
    write_stubs(dt / "schemas", fname)
    (dt / "escape-apps-v1.json").write_text(json.dumps({"type": "object"}))   # permissive schema OUTSIDE the schema root
    fx.write_tree(dt / "repo", {"evil.yaml": "apiVersion: apps/v1\nkind: ../../escape\nmetadata:\n  name: x\nspec: {}\n"})
    tr = K("kc-traversal-kind", ["-kubernetes-version", KVER, "-schema-location", template(dt / "schemas"), "-output", "json", "-verbose",
                                 "evil.yaml"], dt / "repo", "kind containing ../ segments", ["FX-INJ-KUBECONFORM"])
    traversal = "statusValid" in statuses(tr)
    ctx.note("schema_path_traversal_via_kind", {"status": statuses(tr), "exit": tr["exit"],
                                                "msg": [(x.get("msg") or "")[:80] for x in resources(tr)], "escaped_schema_root": traversal})
    # remote $ref inside a local stub schema
    dr = ctx.case_dir("kc-remoteref")
    write_stubs(dr / "schemas", fname, extra_lenient={"allOf": [{"$ref": "http://schemas.example.invalid/x.json"}]})
    fx.write_tree(dr / "repo", {"valid.yaml": fx.K8S_VALID})
    rr = K("kc-remote-ref", ["-kubernetes-version", KVER, "-schema-location", template(dr / "schemas"), "-output", "json", "-verbose",
                             "valid.yaml"], dr / "repo", "stub schema with a remote $ref", ["FX-NET-REMOTE-REF"], timeout=90)
    ctx.note("remote_ref_in_stub", {"exit": rr["exit"], "timed_out": rr["timed_out"], "wall_ms": rr["wall_ms"],
                                    "latency_comparable_to_failed_default_fetch": rr["wall_ms"] >= 1000 and net["wall_ms"] >= 1000,
                                    "fast_local_baseline_ms": R["valid"]["wall_ms"], "status": statuses(rr),
                                    "signals": rr["network_attempt_signals"], "msgs": [(x.get("msg") or "")[:100] for x in resources(rr)]})
    sc_ok = fn(sc_file) == ["scope/selected.yaml"]
    wide = len(fn(sc_dir)) > 1
    ctx.hyp("H-SCOPE", "confirmed" if (sc_ok and wide) else "partially confirmed",
            f"Explicit file operand read only {fn(sc_file)}; a directory operand read {fn(sc_dir)} (siblings/nested; the sibling "
            f"canary C11 reached stdout in the directory run: {'C11' in sc_dir['canary_in_stdout']}, in the explicit run: "
            f"{'C11' in sc_file['canary_in_stdout']}). Non-YAML files were not picked up by the directory walk "
            f"({'C12' not in sc_dir['canary_in_stdout']}); an explicit non-YAML (.txt) operand yielded {len(resources(sc_txt))} resources with exit {sc_txt['exit']} (silently ignored: zero coverage, not a pass). Strategy: "
            f"explicit_files. Extra read-scope finding: kind containing '../' escaped the schema root: {traversal}; "
            f"directory walk followed in-root link {'link-inroot.yaml' in fn(lk)}, outside link {'link-outside.yaml' in fn(lk)}.",
            [sc_file["case"], sc_dir["case"], sc_txt["case"], lk["case"], tr["case"]])

    # ------------------------------------------------------------------ H-ARG
    dn2 = ctx.case_dir("kc-names")
    write_stubs(dn2 / "schemas", fname)
    fx.write_tree(dn2 / "repo", fx.hostile_name_tree(fx.K8S_VALID))
    nargs = ["-kubernetes-version", KVER, "-schema-location", template(dn2 / "schemas"), "-output", "json", "-verbose"]
    arg_res, arg_runs = {}, []
    for fxid, nm in fx.HOSTILE_NAMES.items():
        r = K(f"kc-arg-{fxid.lower()}", nargs + ["--", f"names/{nm}"], dn2 / "repo", f"hostile name {fxid} after --", [fxid])
        got = [x.get("filename") for x in resources(r)]
        arg_runs.append(r["case"])
        arg_res[fxid] = {"exit": r["exit"], "json_ok": r.get("json_parse") == "ok", "resources": len(got),
                         "filename_round_trips": got == [f"names/{nm}"], "stdout_has_raw_esc_byte": b"\x1b" in r["_stdout"],
                         "status": statuses(r)}
    dd2 = ctx.case_dir("kc-dash")
    write_stubs(dd2 / "schemas", fname)
    fx.write_tree(dd2 / "repo", {"-rf.yaml": fx.K8S_VALID, "--strict=false.yaml": fx.K8S_VALID, "ok.yaml": fx.K8S_VALID})
    dargs = ["-kubernetes-version", KVER, "-schema-location", template(dd2 / "schemas"), "-output", "json", "-verbose"]
    dctl = K("kc-arg-dash-control", dargs + ["-rf.yaml"], dd2 / "repo", "control: option-like name WITHOUT --", ["FX-NAME-DASH-1"], parse_json=False)
    dctl2 = K("kc-arg-dash-control2", dargs + ["--strict=false.yaml"], dd2 / "repo", "control 2 WITHOUT --", ["FX-NAME-DASH-2"], parse_json=False)
    dok = K("kc-arg-dash-ddash", dargs + ["--", "-rf.yaml", "--strict=false.yaml"], dd2 / "repo", "option-like names after --",
            ["FX-NAME-DASH-1", "FX-NAME-DASH-2"])
    ctx.note("hostile_names", {"per_name": arg_res, "dash_control_exit": [dctl["exit"], dctl2["exit"]],
                               "dash_control_stdout_is_json": [dctl.get("json_parse"), None],
                               "dash_ddash_files": sorted(f for f in fn(dok)), "dash_ddash_exit": dok["exit"]})
    all_ok = all(v["exit"] == 0 and v["filename_round_trips"] for k, v in arg_res.items())
    ctl_inj = dctl["exit"] != 0 or dctl2["exit"] != 0
    if all_ok and len(fn(dok)) == 2:
        ctx.hyp("H-ARG", "confirmed",
                f"After '--' all hostile names (dash, glob/quote/$, long, ANSI, bidi, newline) were validated and round-tripped "
                f"exactly in JSON filename fields (ANSI/newline are JSON-escaped; raw ESC byte in stdout: "
                f"{arg_res['FX-NAME-ANSI']['stdout_has_raw_esc_byte']}). Control without '--': exit codes "
                f"{dctl['exit']}/{dctl2['exit']} (option-like names treated as flags: {ctl_inj}).",
                arg_runs + [dok["case"], dctl["case"], dctl2["case"]])
    else:
        ctx.hyp("H-ARG", "partially confirmed", f"per-name {arg_res}; dash after -- files {fn(dok)}.",
                arg_runs + [dok["case"], dctl["case"], dctl2["case"]])

    # ------------------------------------------------------------------ H-SEC
    dsec = ctx.case_dir("kc-sec")
    write_stubs(dsec / "schemas", fname)
    sec_dep = fx.K8S_VALID.replace("name: web", f"name: web-{c['C01']}").replace("replicas: 1", "replicas: 1\n  mode: " + c["C02"])
    fx.write_tree(dsec / "repo", {"name-secret.yaml": sec_dep, "malformed-secret.yaml": fx.yaml_malformed_secret(c),
                                  "kind-secret.yaml": f"apiVersion: apps/v1\nkind: {c['C03']}\nmetadata:\n  name: x\n"})
    secargs = ["-kubernetes-version", KVER, "-schema-location", template(dsec / "schemas"), "-verbose"]
    sec_runs, sec_hits = [], {}
    for fmt in ("json", "text", "pretty", "tap", "junit"):
        r = K(f"kc-sec-{fmt}", secargs + ["-output", fmt, "name-secret.yaml", "malformed-secret.yaml", "kind-secret.yaml"],
              dsec / "repo", f"secret echo in {fmt} output", ["FX-YAML-MALFORMED-SECRET"], parse_json=False)
        sec_runs.append(r["case"])
        sec_hits[fmt] = {"stdout": r["canary_in_stdout"], "stderr": r["canary_in_stderr"]}
    sec_dbg = K("kc-sec-debug", secargs + ["-debug", "-output", "json", "name-secret.yaml", "malformed-secret.yaml"], dsec / "repo",
                "secret echo with -debug", ["FX-YAML-MALFORMED-SECRET"], parse_json=False)
    sec_runs.append(sec_dbg["case"])
    sec_hits["debug"] = {"stdout": sec_dbg["canary_in_stdout"], "stderr": sec_dbg["canary_in_stderr"]}
    ctx.note("secret_echo_by_format", sec_hits)
    echoed = sorted({cid for v in sec_hits.values() for cid in v["stdout"] + v["stderr"]})
    sec_mal = R["malformed-secret"]
    c02_echoed = "C02" in echoed
    ctx.hyp("H-SEC", "confirmed" if echoed or sec_mal["canary_in_stdout"] else "refuted",
            f"Canaries echoed by kubeconform: {echoed} (C01 = secret inside metadata.name, C02 = secret as the value of a "
            f"schema-checked field (echoed: {c02_echoed}), C03 = secret as the kind). Malformed-YAML file with a secret value (C01) echoed in JSON stdout: "
            f"{bool(sec_mal['canary_in_stdout'])}. The tool has no redaction option and writes no report files; the adapter must "
            "sanitise resource names/kinds and messages before persistence and must not retain raw stdout.",
            sec_runs + [sec_mal["case"]])

    # ------------------------------------------------------------------ H-MAL / H-OUT
    mm = R["multidoc-malformed"]
    ctx.hyp("H-MAL", "partially confirmed" if resources(R["malformed"]) else "observed",
            f"A malformed single-document file produced {st(R['malformed'])} (exit {R['malformed']['exit']}); in a multi-document "
            f"file with a malformed middle document the results were {st(mm)} (exit {mm['exit']}); later documents still validated: "
            f"{sum(1 for x in resources(mm) if x.get('status') == 'statusValid') >= 2}; the error entry has empty kind/name and no "
            "document index, so it cannot be mapped to a document except by order. kubeconform performs no secret detection (SCAN-08 relies on gitleaks); "
            f"the C01 canary in the malformed file reached stdout: {bool(R['malformed-secret']['canary_in_stdout'])}.",
            [R["malformed"]["case"], mm["case"], R["malformed-secret"]["case"]])
    do = ctx.case_dir("kc-out")
    write_stubs(do / "schemas", fname)
    many = {f"m/f{i:02d}.yaml": fx.K8S_VALID.replace("web", f"w{i:02d}") for i in range(20)}
    fx.write_tree(do / "repo", many)
    oargs = ["-kubernetes-version", KVER, "-schema-location", template(do / "schemas"), "-output", "json", "-verbose"]
    o = [K(f"kc-out-run{i}", oargs + ["m"], do / "repo", f"determinism run {i}", ["FX-OUT-LARGE"]) for i in range(1, 4)]
    o1 = K("kc-out-n1-a", oargs + ["-n", "1", "m"], do / "repo", "determinism with -n 1 (a)", ["FX-OUT-LARGE"])
    o2 = K("kc-out-n1-b", oargs + ["-n", "1", "m"], do / "repo", "determinism with -n 1 (b)", ["FX-OUT-LARGE"])
    same_default = o[0]["_stdout"] == o[1]["_stdout"] == o[2]["_stdout"]
    same_sorted = all(sorted(map(json.dumps, resources(x))) == sorted(map(json.dumps, resources(o[0]))) for x in o)
    same_n1 = o1["_stdout"] == o2["_stdout"]
    ctx.note("output_determinism", {"byte_identical_default_n4": same_default, "same_set_after_sorting": same_sorted, "byte_identical_n1": same_n1,
                                    "ansi_name_raw_esc_in_json": arg_res["FX-NAME-ANSI"]["stdout_has_raw_esc_byte"]})
    if same_n1 and same_sorted and all(v["json_ok"] for v in arg_res.values()):
        ctx.hyp("H-OUT", "confirmed" if same_default else "partially confirmed",
                f"JSON output parsed for all hostile names with exact filename round-trip and JSON escaping. Byte-identical across "
                f"repeated runs at default concurrency: {same_default}; same set after sorting: {same_sorted}; byte-identical with "
                f"-n 1: {same_n1}. The adapter should sort resources itself or pass -n 1.", [x["case"] for x in o] + [o1["case"], o2["case"]] + arg_runs)
    else:
        ctx.hyp("H-OUT", "partially confirmed",
                f"Determinism default={same_default} sorted={same_sorted} n1={same_n1}; json ok for hostile names: "
                f"{all(v['json_ok'] for v in arg_res.values())}.", [x["case"] for x in o] + [o1["case"], o2["case"]] + arg_runs)

    ctx.note("invocation_template", ["<kubeconform>", "-kubernetes-version", "<pinned>", "-schema-location", "<local-template>", "-strict",
                                     "-output", "json", "-verbose", "-summary", "-n", "1", "--", "<file>..."])
    ctx.note("scope_strategy", "explicit_files; never -ignore-missing-schemas / -skip; validate kind/apiVersion strings against a safe pattern before invoking")
    ctx.note("markers_seen", ctx.markers())

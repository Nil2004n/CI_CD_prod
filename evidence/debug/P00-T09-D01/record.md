# P00-T09-D01 — Checkov-only diagnostic / correction cycle: record

Written by Opus, 2026-10-24. Owner-approved: one bounded Checkov-only cycle and, conditionally, one Checkov-only formal run. T09 remains pending acceptance. T09 run1 (`evidence/P00/P00-T09/run1/`) is preserved unchanged. Its two app-owned invocations that executed repository code remain **failed application-invocation candidates**: evidence that contract revision 1 (R1) was unsafe. They are not relabelled.

## 1. Original finding (T09 run1, unchanged)
| Run | cwd | argv (after the binary) | Environment | Observed |
|---|---|---|---|---|
| `CK-EXE-CFG-F-RUN` | `<case>/repo` (the synthetic repository) | `-f k8s/insecure.yaml --framework kubernetes -o json` | harness allowlist: `PATH`, `LANG`, private `HOME`/`XDG_CONFIG_HOME`/`XDG_CACHE_HOME`/`TMPDIR` | marker `PWT_EXECUTED_FX-INJ-EXEC-CHECKOV-F` created; exit 1 |
| `CK-EXE-APPCFG-RUN` | `<case>/repo` | `--config-file <case>/app-config.yaml -f k8s/insecure.yaml -o json`; app config `framework: [kubernetes]` | same | marker `PWT_EXECUTED_FX-INJ-EXEC-CHECKOV-APPCFG` created; exit 1 |

- **Repository config in both runs:** `.checkov.yaml` containing `external-checks-dir: [./custom_checks]` and `framework: [kubernetes]`. `custom_checks/__init__.py` plus `pwt_spike_check.py` write the marker at import time.
- **Executable:** `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv/bin/checkov` (Checkov 3.3.30 on the standalone CPython 3.12.15, whose binary hash matches the install record; the venv itself is not hash-verified, as covered by the PyPI provenance exception).

## 2. Established cause
**Observed (run1):** a marker file was written in exactly these configurations, and in none with cwd outside the repository. In run1, `--show-config` named the repository `.checkov.yaml` as a source when cwd = repo. The custom check was not registered in the results; the module ran at import.

**Mechanism (inferred from the pinned source; consistent with the observations, not separately traced at run time):**

| Step | Source (venv `site-packages/`, sha256) | Behaviour |
|---|---|---|
| Default config list | `checkov/common/util/config_utils.py:12` `get_default_config_paths` (`eb06ad4b…`) | `.checkov.yaml`/`.yml` in every `-d` directory, then **`Path.cwd()`**, then **`Path.home()`**. Not the `-f` file's directory. |
| Parser setup | `checkov/main.py:170–190` `parse_config` (`4cddb628…`) | `ExtArgumentParser(default_config_files=…, YAMLConfigFileParser)`. A nonexistent `--config-file` is silently cleared only when `ENABLE_CONFIG_FILE_VALIDATION` is set (default off; otherwise an open error). |
| Merge | `configargparse.py:1856–1965` `_open_config_files`, `:1310–1365` (`e162e90a…`) | Opens **all** existing default config files **plus** the user `--config-file`. Every key from every file is added unless that option is already on the command line. An explicit `--config-file` is therefore merged, not exclusive. |
| Option type | `checkov/common/util/ext_argument_parser.py:164–165` | `--external-checks-dir` is `action="append"`. A repository value is added, and only a command-line value of the same option would displace it. |
| Import | `checkov/common/checks/base_check_registry.py:205ff` (`07cd8e25…`) | `os.walk` of each external-checks dir, `sys.path` insert, `importlib.util.spec_from_file_location` → `exec_module` for every `.py` in directories with `__init__.py`. Unverified disk load unless `--external-checks-public-key` is set. |

So with cwd = repository, the repository `.checkov.yaml` is a default config file. Its `external-checks-dir` is merged in, and the module is imported and executed. App flags and the app's `--config-file` do not prevent this. **No CVE is declared and nothing is published**: this is documented configuration behaviour used in an untrusted-input setting.

## 3. Replacement contract R2 (spike evaluation only; no production adapter or helper written)
- **cwd:** private, empty, app-owned `<case>/run`. Never the repository or the staging directory.
- **Environment:** harness allowlist (`PATH=/usr/bin:/bin`, `LANG`, private `HOME`/`XDG_*`/`TMPDIR`) plus `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`. HOME holds no Checkov config: HOME is a discovery path (control `CTRL-HOME-CONFIG`).
- **App config:** `<case>/app/checkov-app.yaml` (outside the input) containing `framework: [kubernetes]`, `skip-download: true`.
- **argv:** `--config-file <app config> --framework kubernetes --skip-download --skip-results-upload -o json -f <absolute staged file> [-f …]`. Never `-d`, never `--external-checks-*`.
- **Staging (`cases_checkov_r2.stage`):**
  - copies only the selected files that are regular `*.yaml`/`*.yml`, ≤1 MiB, with no dot-path component, no symlink anywhere in the path, and no absolute or `..` path;
  - opens with `O_NOFOLLOW` and checks the target resolves inside the repository;
  - records a mapping `rel → staged path, sha256, expected resource count` and verifies the copy's hash.
  
  The repository tree is never copied.
- **Result acceptance:** parsed JSON with a `kubernetes` report, no parsing errors, `resource_count` ≥ the expected count from the staged input, and every staged file with resources present in the results. Anything else is **not assessed**, never "clean".
- **Rejected / retired:**
  - **R1a:** app flags with cwd = repository;
  - **R1b:** app `--config-file` with cwd = repository;
  - any `-d` on a repository or on an unfiltered copy;
  - any HOME/cwd/XDG location not owned by the app.

## 4. Development validation (session scratchpad; not evidence)
- **`r2-1`:** all R2 outcomes as intended, except the R2-CONTEXT comparison. An absolute `-d` path through `~/.local` (a dot-directory component) returned **zero resources with exit 0**.
- **Correction (the one cycle):** added a relative `-d repo` comparison from the clean case directory, and kept the absolute-path run as a recorded observation.
- **`r2-2`:** all R2 outcomes as intended.
- **Shared-code regression (`regress1`):** `common.py` gained one `TOOLS` key (`checkov_r2`); `run_t09.py` gained `checkov_r2` in the interpreter-hash preflight condition. Both changes are additive. A development re-run of the five original suites matched T09 run1 exactly: hypothesis outcomes, case lists, exit codes and markers, for yamllint, gitleaks, checkov, actionlint and kubeconform. On that basis run1's results for those tools are considered unaffected; they were not formally re-run.

## 5. Formal run (`evidence/debug/P00-T09-D01/`)
| Item | Result |
|---|---|
| Harness hashes | `run1/harness-sha256.txt` (HEAD `a2d8488`) |
| Fresh preflight + self-test | `preflight/`: 25/25, self-test 19/19 |
| Formal run | `run1/`: 2026-10-24T13:26:42Z–13:27:51Z, outer exit 0, root removed; preflight 25/25; self-test 19/19; 16 Checkov invocations; 0 timeouts; results not suppressed; no `PWTFAKE`, no high-entropy 64-character runs, no PEM material |
| `results.json` sha256 | `70f35be5ede2f989ae2d89d4e987a27d1901e309928bb36b074e5dd721f7e25b` |
| Sheet | `contract-checkov_r2.md` (generated) |

| Hypothesis | Outcome | Evidence |
|---|---|---|
| R2-NO-EXEC | confirmed for the tested variants | Path-level listing of the whole spike root: exactly 4 marker files, all in `CTRL-*` case directories. Per-case globs of every hostile R2 case are empty. All four controls executed the hostile module in the same run: R1a, R1b, R2 argv with a hostile `~/.checkov.yaml`, and an unfiltered copy scanned with `-d`. |
| R2-NO-DISCOVERY | confirmed | `--show-config` under R2 names only the command line, including the app `--config-file`; no repository `.checkov.*`. |
| R2-FINDINGS | confirmed | Clean reference: 21 failed built-in checks, verdict assessed. With the hostile repository (root and near-input configs carrying `skip-check` of all 21 ids, `soft-fail: true`, absolute `external-checks-dir`, Python modules, `evil.py`, an executable helper, `kustomization.yaml`), failed ids were equal for a single file, per file for two files, and for the filtered hostile selection. Exit 1: repository soft-fail not applied. |
| R2-STAGING | confirmed | Of 15 selected entries only `k8s/insecure.yaml` was accepted. Rejected: three `.checkov.*` (dot-path), two modules, `evil.py`, `helper.sh`, a directory (type), three symlink paths (outside, inside, symlinked directory), `../outside.yaml`, `/etc/hostname`, a missing file. Original repository tree hash unchanged. |
| R2-COVERAGE-VALIDATION | confirmed | Malformed input: exit 0, no kubernetes report → not assessed. Missing input: rejected at staging; with the contract argv forced at a nonexistent path, exit 0 → not assessed. YAML without resources → not assessed (no kubernetes resources). |
| R2-INLINE-SUPPRESSION | partially confirmed (limitation) | In-file `checkov.io/skipN` annotations still suppress (2 of 2 ids moved to `skipped_checks`). Staging copies data verbatim and no flag ignores them, so they must be surfaced as repository-controlled suppression. |
| R2-CONTEXT | partially confirmed (limitation) | For this fixture, single staged file = relative `-d` clean full scan (same 21 ids). Not a general equivalence: selected-file staging drops cross-file context (kustomize/helm, resources in other files) and covers the kubernetes framework only. |
| R2-NET | confirmed (heuristic) | No network-attempt signal in R2 runs. One control, R1a without `--skip-download`, showed the `api0.prismacloud.io` attempt failing at resolution. |

**Formal stop conditions:**
- **Execution marker in an application-contract (R2) run:** not fired.
- **Markers in the four controls:** expected and pre-declared in the module before the run. The controls reproduce rejected or unsafe set-ups to prove fixture potency and are not application invocations.
- **Canary in persisted results:** not fired.
- **Unbounded network attempt:** not fired.
- **Preflight failure:** not fired.

## 6. Defects and disclosures
- **D01 module detection weakness:** the per-run `new_execution_markers` field is a by-name diff. The controls share the marker name `…-R2-ROOT`, so only the first control shows a new marker, and a same-named marker in a later case directory would not appear in that field. The R2-NO-EXEC text's "markers: {}" came from that field. The verdict instead rests on the path-level marker listing and the per-case globs above, which are complete. Not repaired after the formal run; any future use should give marker names a per-case suffix.
- **Absolute `-d` with a dot-directory component** silently yields zero resources and exit 0. This is another reason structure validation is mandatory.
- **Reported `file_path` values are relative to cwd** (e.g. `/../stage/k8s/insecure.yaml`). The adapter must map results through its own staging map, never by trusting the path text.
- **Scope of the evidence:**
  - spike-only evidence for synthetic fixtures, this host, Checkov 3.3.30 and the kubernetes framework;
  - no production staging helper or adapter exists;
  - the signature gate was not used as a boundary;
  - H-NET evidence is heuristic.
- **Implementation:** by Opus (no worker spawned in this cycle).

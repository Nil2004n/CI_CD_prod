# Checkov contract — revision 2 (P00-T09-D01)

Opus-authored contract revision; supersedes the invocation implied by revision 1. The generated sheets `contract-checkov.md` (T09 run1) and `evidence/debug/P00-T09-D01/contract-checkov_r2.md` are unchanged. Owner status 2026-10-24: **provisionally accepted as the candidate for the tested Kubernetes selected-file workflow**; T09 final acceptance pending. Spike-level contract only; no production adapter or staging helper exists.

Document history:
- 2026-10-24 (D01): first version.
- 2026-10-24 (D02 clarification; contract unchanged):
  - values re-taken from the canonical code (`spikes/t09-tools/cases_checkov_r2.py`, `common.py`, sha256 in `evidence/debug/P00-T09-D01/run1/harness-sha256.txt`) and evidence;
  - the earlier statement that the app config's existence "is checked before invocation" was **not implemented in the spike** and is now listed as an untested requirement;
  - suppression semantics made explicit;
  - attribution basis added.

## 1. Executable identity
| Item | Value | Basis |
|---|---|---|
| Entry point | `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv/bin/checkov`, sha256 `f0683221fa1e6c3be2bd19921c0650e83bb1c833f78535af0b96e02487f639ec` | observed 2026-10-24 (D02); not in the install record's trusted list |
| Interpreter | shebang `…/venv/bin/python` → `~/.local/share/pwt-dev-tools/python/3.12.15+20261021/python/bin/python3.12` | that interpreter's hash matches the install record (preflight) |
| Version | 3.3.30 | `checkov --version` (T09 run1 note `version`); `"checkov_version": "3.3.30"` in D01 JSON output |
| Packages | venv contents not hash-verified | PyPI provenance exception (P00-T02) |

## 2. Allowed invocation (R2)
- **argv:**

  ```
  checkov --config-file <APP_CONFIG> --framework kubernetes --skip-download --skip-results-upload -o json -f <STAGED_FILE_1> [-f <STAGED_FILE_2> …]
  ```

  `<STAGED_FILE_n>` are absolute paths of staged copies. `-f` may repeat (`action="append"`).
- **Never:** `-d`; `--external-checks-dir`, `--external-checks-git` or `--external-checks-public-key` as the boundary; `--create-config`/`--baseline`; verbose or debug options.
- **Working directory:** a private, empty, app-owned directory. Never the repository and never the staging directory.
- **Environment (exactly):**
  - `PATH=/usr/bin:/bin`
  - `LANG=C.UTF-8`
  - `HOME=<private dir>`
  - `XDG_CONFIG_HOME=<private dir>`
  - `XDG_CACHE_HOME=<private dir>`
  - `TMPDIR=<private dir>`
  - `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`
  
  No other variables (in particular no `ENABLE_CONFIG_FILE_VALIDATION`, which would silently drop a missing `--config-file`; no `BC_*`/`CKV_*` API settings; no proxy variables).
- **App config:** an app-owned file outside all input, content exactly:

  ```
  framework:
    - kubernetes
  skip-download: true
  ```

## 3. Discovery behaviour that R2 relies on
- Checkov reads default config files from every `-d` directory, the cwd and HOME (`config_utils.py:12`). configargparse merges them with `--config-file` (`configargparse.py:1856–1965`, `:1310–1365`).
- R2 passes no `-d` and uses a private cwd and HOME that contain no `.checkov.yaml`/`.yml`.
- Evidence (D01 formal run):
  - `--show-config` listed only the command line, including the app `--config-file`;
  - control `CTRL-HOME-CONFIG` (R2 argv, hostile `~/.checkov.yaml`) **did** execute repository code, so HOME being app-owned is essential.

## 4. Selected-file staging rules (spike implementation `cases_checkov_r2.stage`)
- **Selection:** only explicitly selected relative paths. The repository tree is never copied.
- **Accepted only if:**
  - relative, no `..`, not absolute;
  - no path component starting with `.`;
  - suffix `.yaml` or `.yml`;
  - no symlink in any path component;
  - exists; opened with `O_NOFOLLOW`; a regular file of at most 1 MiB;
  - resolves inside the repository.
- **Copy:** bytes copied verbatim to `<stage>/<same relative path>`; the copy's sha256 is verified.
- **Mapping:** per selected file, `relative path → staged absolute path, sha256 of the original bytes, expected resource count` (lines matching `^kind:`).
  - **Evidence limitation:** the spike kept the full mapping in memory. Persisted evidence holds only the accepted/rejected lists, a 12-hex-character sha256 prefix and the expected resource count (D01 note `r2_hostile.staging`).
- **Rejected in D01:** scanner configs (`.checkov.yaml`/`.yml`, including near the input), Python modules, `evil.py`, an executable helper, a directory, symlinked files (in-root and outside), a symlinked directory component, `../` and absolute paths, a missing file.

## 5. Evaluated coverage
- Kubernetes framework only, on the selected staged files.
- Not evaluated under R2: GitHub Actions, the secrets framework, kustomize/helm, cross-file context. Selected-file staging is **not equivalent to a full repository scan**.

## 6. Result acceptance (coverage and structure checks)
A result counts as **assessed** only if all of the following hold. Otherwise it is **not assessed**, never "clean":
- stdout parses as JSON;
- a report with `check_type == "kubernetes"` exists;
- `results.parsing_errors` is empty;
- `summary.resource_count` ≥ the expected resource count of the staged input;
- every staged file with resources appears in the results.

Exit code and empty output are never evidence of a clean scan. In D01, malformed input, a nonexistent path and YAML without resources all exited 0.

Results are mapped back through the app's staging map, not from Checkov's `file_path` text (cwd-relative, e.g. `/../stage/k8s/…`).

## 7. Suppression semantics
- In-file `checkov.io/skip*` annotations in the staged data still suppress checks. Staging copies data verbatim, and no flag ignores them (D01 `R2-INLINE`).
- Each `skipped_checks` entry is **repository-controlled suppression**. It is neither a passed check nor an assessed-clean result. It must be exposed as suppression/coverage information (check id, file, line range, suppression comment) and must never contribute to a clean verdict.
- Under R2 there is no other skip source: no repository config is read; the app config sets none.

## 8. Rejected / retired invocations
| ID | Invocation | Why |
|---|---|---|
| R1a | `-f k8s/insecure.yaml --framework kubernetes -o json`, cwd = repository | T09 run1 `CK-EXE-CFG-F-RUN`: repository `.checkov.yaml` merged; `external-checks-dir` executed repository Python. Failed application-invocation candidate. |
| R1b | `--config-file <app config> -f k8s/insecure.yaml -o json`, cwd = repository | T09 run1 `CK-EXE-APPCFG-RUN`: the same; `--config-file` merges, it is not exclusive. Failed application-invocation candidate. |
| — | any `-d` on a repository or on an unfiltered copy | D01 `CTRL-UNFILTERED-COPY-D` executed repository code. An absolute `-d` through a dot-directory scanned nothing with exit 0. |
| — | HOME not owned by the app | D01 `CTRL-HOME-CONFIG` executed repository code |
| — | external checks, repository policies, platform downloads | code execution and network |

## 9. Evidence and attribution
- **D01 formal run:** `evidence/debug/P00-T09-D01/run1/results.json` (sha256 `70f35be5…`).
- **No execution under R2:** in each invocation's path-level before/after file diff of its own case directory (`files_created`), exactly the four control invocations created a marker. Each control's directory was used by that single invocation. None of the 12 R2 or comparison invocations created one.
- **Why overwriting without a new file cannot hide an execution:** every hostile module writes to an absolute path inside its own case directory, and R2 invocations' cwd, HOME and inputs stay inside their own directories. A marker would need to already exist there, and no R2 directory ever held one (the path-level listing at the end of the run shows none).
- **Unreliable field:** the per-run `new_execution_markers` field is a name-only diff. It is unreliable when marker names repeat across cases and is not used.

## 10. Untested production behaviour (P09 / FR-27)
- **Not yet built or verified:**
  - the production staging helper and adapter;
  - check-to-copy races (TOCTOU) and concurrent repository changes;
  - an existence and permission check of the app config before invocation;
  - mapping persistence and line mapping in the product;
  - other frameworks and cross-file context;
  - behaviour on other hosts and versions.
- **Confinement and network:** no filesystem confinement (write tracking only). The signature gate is not relied on. Network evidence is heuristic (private netns, `--skip-download`, `--skip-results-upload`, update check disabled).

# Checkov contract — revision 2 (P00-T09-D01)

Opus-authored contract revision; supersedes the invocation implied by revision 1. The generated run1 sheet `contract-checkov.md` is unchanged and remains the record of revision 1. Spike-level contract for Checkov 3.3.30 (venv on CPython 3.12.15) on this host. Not production adapter acceptance and not T09 acceptance.

## Allowed invocation (R2)
| Element | Value |
|---|---|
| Executable | `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv/bin/checkov` |
| Working directory | private, empty, app-owned directory, never the repository or the staging directory |
| Environment | allowlist only: `PATH`, `LANG`, private `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `TMPDIR`; plus `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`. HOME must contain no `.checkov.yaml`/`.yml`. |
| App config | app-owned file outside all input, containing `framework: [kubernetes]` and `skip-download: true`; its existence is checked before invocation |
| argv | `--config-file <app config> --framework kubernetes --skip-download --skip-results-upload -o json -f <absolute staged file> [-f <absolute staged file> …]` |
| Input | allowlisted staged copies only: regular `*.yaml`/`*.yml` ≤1 MiB, no dot-path component, no symlink in the path, no absolute or `..` path, `O_NOFOLLOW` read, resolved inside the repository, sha256 mapping back to the original |
| Result acceptance | parsed JSON; a `kubernetes` report; no parsing errors; `resource_count` ≥ resources expected from the staged input; every staged file with resources present in the results. Otherwise **not assessed**. Never derive "clean" from exit 0 or empty output. |
| Result mapping | through the app's staging map, not from Checkov's `file_path` text (which is cwd-relative, e.g. `/../stage/…`) |
| Suppression | in-file `checkov.io/skip*` annotations remain effective; report every `skipped_checks` entry as repository-controlled suppression |

## Rejected / retired invocations
| ID | Invocation | Why |
|---|---|---|
| R1a | app flags (`-f …`, `--framework`) with cwd = repository | run1 `CK-EXE-CFG-F-RUN`: repository `.checkov.yaml` merged; `external-checks-dir` imported repository Python (marker) |
| R1b | app `--config-file` + `-f` with cwd = repository | run1 `CK-EXE-APPCFG-RUN`: same; `--config-file` merges, it is not exclusive |
| — | any `-d` on a repository or an unfiltered copy | default config discovery in the `-d` directory (D01 control `CTRL-UNFILTERED-COPY-D`); `-d` with an absolute dot-path silently scans nothing |
| — | app-uncontrolled HOME | `~/.checkov.yaml` is a discovery path (D01 control `CTRL-HOME-CONFIG`) |
| — | `--external-checks-dir`/`-git`, repository-supplied policies, platform downloads | code execution and network |

## Evidence
- **Revision 1 failure:** `evidence/P00/P00-T09/record.md` §5; `run1/results.pem-redacted.json` (cases `CK-EXE-CFG-F-RUN`, `CK-EXE-APPCFG-RUN`).
- **Revision 2:** `evidence/debug/P00-T09-D01/record.md`; `run1/results.json` (sha256 `70f35be5…`); `contract-checkov_r2.md`. Confirmed there:
  - no execution under R2 against root and near-input hostile configs and modules, while the potency controls executed in the same run;
  - no repository config source in `--show-config`;
  - built-in findings unchanged under hostile configs;
  - staging rejections;
  - not-assessed classification of malformed, missing and no-resource input.

## Remaining limitations
- Selected-file staging is **not equivalent to a full repository scan**: cross-file context (kustomize/helm, resources in other files) and other frameworks are not covered. Coverage is reported per staged file.
- R2 covers the kubernetes framework only. GitHub Actions and the secrets framework under R2 were not evaluated.
- In-file suppressions cannot be disabled.
- The staging rules are spike code. The production staging helper and the adapter (P09) still need implementation and their own verification, including races between check and copy.
- No filesystem confinement: write tracking only. The signature gate is not relied on.
- H-NET evidence is heuristic (private netns, `--skip-download`, `--skip-results-upload`, update check disabled).

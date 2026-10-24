# P00-T11 — Tool freeze (human-readable view of `tools.json`)

Frozen at 2026-10-24, base commit `3decec1`. Machine-readable source: `spikes/p00-freeze/tools.json` (this file mirrors it and adds no new facts; if they differ, `tools.json` and the cited records win).

Freeze principle: this records only what accepted records establish. Nothing was upgraded, re-resolved, installed, downloaded or run for the freeze. A freeze entry never upgrades an earlier acceptance qualification. Hashes are copied from the install record's "Trusted hashes" table and must be re-hashed before execution. All acceptances are spike acceptances for the tested synthetic fixtures, this host and these versions; none is production adapter acceptance.

## 1. Summary table

| id | Role | Version | Status | sha256 (basis) | Contract | FR links |
|---|---|---|---|---|---|---|
| yamllint | baseline | 1.38.0 | accepted-qualified | none (venv entry script not hash-pinned) | `contract-yamllint.md` | FR-29 |
| gitleaks | baseline | 8.30.1 | accepted-qualified | `88f91962…e5509` (install-record trusted hashes) | `contract-gitleaks.md` | FR-25, FR-29 |
| checkov | baseline | 3.3.30 | accepted-qualified (revision 2 only, Kubernetes selected-file workflow) | `f0683221…639ec` (observed, not trusted) | `contract-checkov-rev2.md`, `contract-checkov_r2.md` | FR-27, FR-29 |
| actionlint | framework | 1.7.12 | accepted-qualified | `c872d6db…f4cd4` (install-record trusted hashes) | `contract-actionlint.md` (no invocation template) | FR-28, FR-29 |
| kubeconform | framework | 0.8.0 | accepted-qualified (schema cache not approved) | `457722b3…097b7` (install-record trusted hashes) | `contract-kubeconform.md`, `P00-T10-schema-cache-plan.md` | FR-26, FR-29, FR-30 |
| python3.12 | interpreter | 3.12.15+20261021 | accepted-limitations | `58fe8992…71c1b68` (install-record trusted hashes) | none | — |
| uv | install-helper | 0.13.5 | accepted-limitations | `8a70db72…67e52` (install-record trusted hashes) | none | — |
| cosign | verification-helper | 3.1.2 | accepted-limitations | `f7622ed3…1e7cf` (install-record trusted hashes) | none | — |
| cargo-audit | deferred | 0.22.2 (requested, not installed) | deferred | null (none) | none | — |

Full 64-character hashes are in `tools.json`. Paths (user-owned prefix `~/.local/share/pwt-dev-tools/`, no sudo, no PATH or profile changes):

| id | Path |
|---|---|
| yamllint | `~/.local/share/pwt-dev-tools/yamllint/1.38.0/venv/bin/yamllint` |
| gitleaks | `~/.local/bin/gitleaks` (reused existing binary) |
| checkov | `~/.local/share/pwt-dev-tools/checkov/3.3.30/venv/bin/checkov` |
| actionlint | `~/.local/share/pwt-dev-tools/actionlint/1.7.12/actionlint` |
| kubeconform | `~/.local/share/pwt-dev-tools/kubeconform/0.8.0/kubeconform` |
| python3.12 | `~/.local/share/pwt-dev-tools/python/3.12.15+20261021/python/bin/python3.12` |
| uv | `~/.local/share/pwt-dev-tools/uv/0.13.5/uv-x86_64-unknown-linux-gnu/uv` |
| cosign | `/usr/local/bin/cosign` |
| cargo-audit | none (not installed) |

## 2. Contract sheet index

| Tool | Path | Kind | Status |
|---|---|---|---|
| yamllint | `evidence/P00/P00-T09/contract-yamllint.md` | generated | current |
| gitleaks | `evidence/P00/P00-T09/contract-gitleaks.md` | generated | current |
| checkov | `evidence/P00/P00-T09/contract-checkov.md` | generated | **superseded-for-invocation** (run1, revision 1; R1a/R1b rejected) |
| checkov | `evidence/P00/P00-T09/contract-checkov-rev2.md` | opus-authored | current (the only allowed Checkov invocation) |
| checkov | `evidence/debug/P00-T09-D01/contract-checkov_r2.md` | generated | current |
| actionlint | `evidence/P00/P00-T09/contract-actionlint.md` | generated | current |
| kubeconform | `evidence/P00/P00-T09/contract-kubeconform.md` | generated | current |

## 3. Per-tool notes

FR-29 (process-group cleanup does not contain descendants that leave the group) is linked on every scanner entry because the scanners ran as child processes under the T09 harness; it is a harness/process requirement, not a tool defect.

### yamllint 1.38.0 — baseline, accepted-qualified
- **Invocation (contract sheet template):** `["<yamllint>", "-c", "<app-config-path>", "-f", "parsable", "--", "<file>..."]`; explicit files only (never a directory argument); cwd outside the repository.
- **Rejected:** directory argument; operands without `--`; running without an explicit app config or with cwd inside the repository (repository `.yamllint*` config is auto-loaded by cwd and parents).
- **Coverage:** YAML lint of explicitly named files with an app-owned config (89 T09 invocations). Not a secret scanner.
- **Limitations:** venv entry script not hash-pinned (venv covered only by the install-time hash lock `evidence/P00/P00-T02/locks/yamllint.lock.txt`); PyPI provenance exception; inline `disable-*` directives suppress findings and cannot be switched off; invalid UTF-8 input crashes with exit 1 (same code as findings), so exit 1 with no parsable lines is a tool error; diagnostics echo YAML key text and raw hostile file names, so sanitise before persistence; production adapter not built.

### gitleaks 8.30.1 — baseline, accepted-qualified
- **Invocation (contract sheet template):** `["<gitleaks>", "dir", "--no-banner", "--no-color", "-f", "json", "-r", "<app-owned report path>", "-c", "<app-config>", "--gitleaks-ignore-path", "<app-owned empty file>", "--ignore-gitleaks-allow", "--redact=100", "--exit-code", "7", "--", "<single file or staged dir>"]`; explicit files (single file path accepted), one path operand per run, dir mode only.
- **Rejected:** scanning a repository directory directly; omitting `-c`, `--ignore-gitleaks-allow` or `--exit-code`; `-v`, `--log-level debug`, `--redact=50`; treating `--gitleaks-ignore-path` as a remedy for a repository `.gitleaksignore`; operands without `--`.
- **Coverage:** text-based secret detection of a single file or staged copy (60 T09 invocations plus 60 in the D02 fresh synthetic run). Git history mode not exercised.
- **Limitations:** `~/.local/bin/gitleaks` is user-writable with unestablished origin (content byte-identical to the verified official binary on 2026-10-24; re-check the hash before every use); no upstream signature (same-origin checksum only); nonexistent target exits 0 (fail-open); repository `.gitleaksignore` and `gitleaks:allow` are suppression channels (FR-25); `--redact=100` does not make stderr safe; report order varies; 16 GiB RLIMIT_AS in T09; production adapter not built.

### checkov 3.3.30 — baseline, accepted-qualified (revision 2 only)
- **Invocation (contract revision 2, section 2):** `checkov --config-file <APP_CONFIG> --framework kubernetes --skip-download --skip-results-upload -o json -f <STAGED_FILE_1> [-f <STAGED_FILE_2> ...]`. Working directory: a private, empty, app-owned directory (never the repository or the staging directory). Environment exactly `PATH=/usr/bin:/bin`, `LANG=C.UTF-8`, `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `TMPDIR` set to the private directory, `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`; no other variables. App config outside all input, content exactly `framework: [kubernetes]` (as a YAML list) and `skip-download: true`. Never `-d`, `--external-checks-*` as the boundary, `--create-config`/`--baseline`, verbose or debug options.
- **Rejected:** R1a (app flags, cwd = repository), R1b (app `--config-file`, cwd = repository), any `-d` on a repository or unfiltered copy, HOME not owned by the app, external checks / repository policies / platform downloads. The run1 sheet `contract-checkov.md` is superseded for invocation.
- **Coverage:** Kubernetes framework on explicitly selected, app-staged `.yaml`/`.yml` copies only; not GitHub Actions, secrets framework, kustomize/helm or cross-file context; not equivalent to a full repository scan.
- **Limitations:** entry point hash is observed, not trusted, and the venv (96 packages) is not hash-verified at run time (install-time hash lock only; PyPI provenance exception); empty output, zero checks, malformed input and a nonexistent path all exit 0, so a result is assessed only if it passes the structure and coverage checks of revision 2 section 6 (otherwise not assessed, never clean; FR-27); in-file `checkov.io/skip*` annotations still suppress and must be surfaced as repository-controlled suppression; production staging helper, TOCTOU handling, app-config existence check and mapping persistence are untested; no filesystem confinement; network evidence heuristic.

### actionlint 1.7.12 — framework, accepted-qualified
- **Invocation:** `null`. The generated sheet records no invocation template or scope strategy; none is frozen and no flags were reconstructed. Sheet facts (not a template): explicit file arguments only; `-config-file <app config>` replaces repository config; JSON output via `-format '{{json .}}'`; helpers disabled explicitly via `-shellcheck` / `-pyflakes` set to an empty string; no soft-fail, strict or redaction flag.
- **Rejected / not established:** directory argument (exit 3); no file arguments (auto-discovery walks `.github/workflows`); dash-leading names without `--` (exit 2); relying on a repository `.github/actionlint.yaml` / `.yml` (auto-loaded, needs a `.git` root, suppresses findings).
- **Coverage:** GitHub Actions workflow linting of explicitly named files (71 T09 invocations). ShellCheck and Pyflakes integration not exercised.
- **Limitations:** ShellCheck/Pyflakes absent and untested; disable explicitly, do not install or claim tested without separate approval (FR-28); exit 1 covers both lint findings and syntax/parse problems (separate via JSON `kind`); diagnostics echo raw source (snippet, message) with no redaction flag; default text output emits raw ESC; SLSA provenance verified, GitHub release attestation not; extracted and version-run before the attestation lookup (disclosed); network evidence indirect; production adapter not built.

### kubeconform 0.8.0 — framework, accepted-qualified (schema cache not approved)
- **Invocation (contract sheet template):** `["<kubeconform>", "-kubernetes-version", "<pinned>", "-schema-location", "<local-template>", "-strict", "-output", "json", "-verbose", "-summary", "-n", "1", "--", "<file>..."]`; explicit files; never `-ignore-missing-schemas` / `-skip`; validate kind/apiVersion against a safe pattern before invoking. `<pinned>` and `<local-template>` are placeholders with no frozen value.
- **Rejected:** no `-schema-location` / default schema location (network attempt); the literal `default`; `-ignore-missing-schemas`; `-skip`; directory operand; operands without `--`.
- **Coverage:** schema validation of explicitly named manifest files against a local schema location (65 T09 invocations, stub schema and controlled loopback server). No real schema set exists.
- **Limitations:**
  - **Schema cache not approved or populated.** P00-T10 was accepted as planning and an offline reference prototype only. Owner decisions O-1 to O-6 are open; no schema source, Kubernetes version policy or schema download is approved.
  - **FR-30 (R-1):** the T10 name mapping lets other API groups collide with core schemas (synthetic demonstration: `apps.example.com/v1` Deployment, `networking.istio.io/v1` Ingress). Unknown or non-built-in API groups must not be routed through the T10 filename mapping until FR-30 is met in P13; the prototype does not fix it.
  - kind/apiVersion-derived schema paths are untrusted (`../` in `kind` escaped the schema root; FR-26).
  - Exit codes alone do not establish coverage; classify from parsed per-resource status. Explicit non-YAML operand: 0 resources, exit 0.
  - Output not byte-identical at default concurrency (`-n 1` is); names/kinds are echoed without redaction; no upstream signature; production adapter not built.

### python3.12 3.12.15+20261021 — interpreter, accepted-limitations
CPython from python-build-standalone release 20261021, extracted from a verified archive; base of the yamllint and Checkov venvs. SLSA provenance verified, GitHub release attestation not; built from a `refs/heads/main` workflow. System Python (`/usr/bin/python3` → 3.14) untouched. Not a scanner; no contract sheet; accepted with the installation checkpoint, not as scanner or application acceptance.

### uv 0.13.5 — install-helper, accepted-limitations
Used once at install time to resolve and install hash-locked wheels (procedure recorded in `tools.json` `invocation`, copied from the install record). No contract sheet; no further use approved by this freeze. SLSA provenance verified, release attestation not. Compile-step exit codes were `tail`'s (correction RC-3), success inferred from complete lock files (indirect evidence). Rejected: `--only-binary` with `--no-build` (uv usage error, exit 2).

### cosign 3.1.2 — verification-helper, accepted-limitations
Used once (V1) to verify SLSA provenance attestations of the actionlint, uv and CPython archives (three verifications exit 0; two negative controls exit 1). `/usr/local/bin/cosign` is user-writable and shown byte-identical to the official `sigstore/cosign` v3.1.2 release asset; re-hash before reuse. GitHub release attestations and PyPI provenance were not verified. Attestations establish origin/build identity, not code safety. Needed network access for the Sigstore trust root.

### cargo-audit — deferred
Not installed (R-4: deferred to P01, a pending dependency, not a check). The version 0.22.2 is the one named in installation Request C, not an installed or verified version. Installing builds from source (dependency build scripts run on the host); a later `cargo audit` downloads the RustSec advisory database. Both need separate owner approval. No advisory scan result exists.

## 4. Not frozen / open

- **Schema cache decisions O-1 to O-6 (open):** schema origin; target Kubernetes version(s); staleness threshold and warn-or-block; CRD catalogue use; redistribution/bundling and NOTICE review; approval of the first `schema_fetch` and revision pin. kubeconform cannot be exercised against real schemas until these are decided and a cache is approved.
- **FR-30 (R-1)** is a mandatory P13 acceptance requirement and is not fixed by the T10 prototype.
- **ShellCheck and Pyflakes:** absent and untested; actionlint's helper-on path and the helpers' inline directives are unverified (FR-28).
- **No filesystem confinement:** T09 write tracking observed new top-level names and spike-root changes only; it is not confinement.
- **Process-group escape not contained:** descendants that leave the group (`setsid`) are not contained by process groups; a bounded harness return is not proof of containment (FR-29).
- **H-3 guard scope:** the T09 output guard (after the P00-T09-D02 fix) covers registered synthetic fixtures only; it does not detect arbitrary real secrets, interior fragments shorter than 15 characters, short trailing boundary prefixes or other transformed encodings. The committed T09 run1 results are the PEM-redacted derivative; the raw file is held privately outside the repository.
- **Production adapters and the Checkov staging helper are not built;** Checkov revision 2 untested items (TOCTOU, app-config existence check, mapping persistence, other frameworks, other hosts/versions) remain.
- **Provenance:** the yamllint and Checkov wheels and their dependency chain are not provenance-verified (PyPI provenance exception, controlled development only; revisit before customer distribution or sensitive client use). gitleaks and kubeconform have no upstream signature (same-origin checksum only). GitHub release attestations are unverified for actionlint, uv and CPython.
- **Trusted-hash gaps:** the Checkov entry point hash is observed only; the yamllint entry script has no hash; a venv-level re-hash method was left to be defined and is not defined in the records used for this freeze.
- **User-writable paths:** `~/.local/bin/gitleaks` and `/usr/local/bin/cosign` live in user-writable locations; re-hash before every use.
- **cargo-audit:** deferred; no Rust advisory scan has been run.
- **No blanket "all green = secure":** manual checks and unsupported coverage remain explicit; Checkov covers only the Kubernetes selected-file workflow, actionlint's invocation is not frozen, and kubeconform has no approved schema set.

## P00 closure clarifications and later-phase gates (2026-10-24)
Recorded at P00 closure. Null, provisional and unverified fields are preserved unchanged. **The frozen inventory is not permission to build, install or adopt anything.**

| Deferred / open item | Clarification | Later-phase gate |
|---|---|---|
| cargo-audit | version `0.22.2` is the **requested** version (Request C); **not installed, not verified**; no advisory scan result exists | P01 (R-4): install request, verification, first run |
| yamllint identity | no trusted entry-script or venv hash (`sha256: null`); identity rests on the install-record version check and install-time hash lock — a **recorded limitation**, not a frozen identity | P08 adapter preflight (venv re-hash method to be defined) |
| actionlint invocation | `invocation: null` — **no actionlint invocation is frozen**; sheet facts are observations, not a template; ShellCheck/Pyflakes untested | P13 adapter contract (FR-28) |
| gitleaks adapter | run1 contract qualifications | P08 (FR-25) |
| Checkov adapter + staging helper | revision 2 only; staging helper not built | P09 (FR-27) |
| kubeconform + schema cache | no cache approved or populated; R-1 collision | P13 (FR-26, FR-30 mandatory); O-1–O-6 owner decisions; separate `schema_fetch` approval |
| Process containment | escaped descendants not contained; bounded return ≠ containment | P06 (FR-01–FR-03, FR-29) |
| YAML/JSON-Schema production pipeline | spike suitability only | P02/P05/P07/P08 per FR-05–FR-17 |
| Keyring / OS credential store | K-A exception historical/inactive (see crates); no further build authorised | P03 (FR-18–FR-24; new decision + re-screen) |
| Unexercised crates (provisional / fallback / excluded) | **no production acceptance and no test evidence implied** | P01+ approved dependency requests |
| PDF renderer | featureless printpdf, visual correctness unverified | P12 |

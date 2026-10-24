# P00-T11 crate freeze (draft for Opus review and owner acceptance)

Frozen at 2026-10-24, base `3decec1`. Machine-readable twin: `spikes/p00-freeze/crates.json` (authoritative for versions, features and statuses).

**Freeze principle.** This file records only what accepted repository records establish. Nothing was re-resolved, upgraded, fetched, installed or built for it. A freeze status never upgrades an earlier acceptance qualification. The spike workspace is development-only; the freeze does **not** approve any dependency for application use.

## 1. Toolchain and lockfile

| Item | Frozen value | Source |
|---|---|---|
| rustc / cargo | 1.97.1 / 1.97.1 (stable, `x86_64-unknown-linux-gnu`) | `spikes/P00-T03-crate-request.md` (local toolchain); `evidence/P00/P00-T05-D3-1/record.md` (rustc 1.97.1 8bab26f4f, LLVM 22.1.6; cargo 1.97.1 c980f4866) |
| Invocation | Toolchain binaries invoked directly under `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/`, no rustup proxy | `evidence/P00/P00-T05-D3-1/record.md` |
| Lockfile | `spikes/rust/Cargo.lock`, SHA-256 in `crates.json` (computed with `sha256sum` at freeze time; 235 packages after T08 stage A) | `evidence/P00/P00-T08/record.md` |

## 2. Direct dependencies pinned in spike manifests

All pins are exact (`=`). "Accepted-qualified" means the owning spike task was accepted by the owner with the qualifications listed; it is **not** acceptance as an application dependency.

| Crate | Version | Default features | Features | Spike package | Status | Key qualification |
|---|---|---|---|---|---|---|
| libc | 0.2.191 | on | none | t05-process | accepted-qualified | T05 feasibility spike with findings; production supervisor (output limits, descendant cleanup, escaped descendants) unverified (FR-01 to FR-03) |
| saphyr-parser | 0.2.2 | on | none | t06-yaml | accepted-qualified | Tested combination with spike-owned composer and `jsonschema`; parser reports character offsets as "bytes", keeps BOM, no structured error kind (FR-05 to FR-14); spike-validated candidate, not an accepted application dependency |
| jsonschema | 0.60.1 | **off** | none | t06-yaml | accepted-qualified | Reviewed defaults-off setup only; `$ref` retrieval compiled out; `format` assertion explicit (FR-15); suite conformance not run |
| serde_json | 1.0.154 | on | none | t06-yaml | accepted-qualified | Part of the T06 pipeline; `arbitrary_precision` not active |
| keyring | 4.2.0 | on (`v1`) | none | t07-keyring | accepted-qualified | **Tested isolated setup only.** Not accepted: direct synchronous use in the app, persistent storage as pilot or product feature, native cancellation, real prompter, other implementations or login sessions (FR-18 to FR-24). Spike-only advisory exception K-A |
| pulldown-cmark | 0.13.5 | **off** | none | t08-pdf | accepted-qualified | Tested cases, debug build; run1 failed, D01 9 of 10 cases written (Korean out of scope, D-15) |
| printpdf | 0.12.8 | **off** | none | t08-pdf | accepted-qualified | **Featureless only**; whole-font embedding, no subsetting, no shaping, no bidi or RTL; empty `/Annots` array permitted; visual correctness unverified |

## 3. Provisional, fallback and excluded (T03 statements; no spike exercised these)

| Crate | Version (T03) | Status | Role |
|---|---|---|---|
| tokio | 1.54.1 | provisional | Async supervision primary; features `rt`, `process`, `time`, `io-util`, `sync` (no `full`/`macros`); present in the lockfile only on a wasm-only path, never downloaded or verified |
| saphyr | 0.2.2 | provisional | Paired with `saphyr-parser` in the T03 primary; only `saphyr-parser` was exercised |
| serde | 1.0.229 | provisional | `derive`; transitive only in the spikes |
| sha2 | 0.11.1 | provisional | Harness hashing; transitive only in the spikes |
| tempfile | 3.27.0 | provisional | Harness temp dirs; in the lockfile but in no host graph |
| rustix | 1.1.5 | fallback | Process-group signalling (`process` feature) if libc is unsuitable |
| yaml-rust2 | 0.13.0 | fallback | YAML parser; T06 states no fallback was required |
| boon | 0.6.1 | fallback | JSON Schema; MSRV undeclared, needs a build test |
| typst | 0.15.1 | fallback | PDF generation; large |
| nix | 0.31.3 | excluded | Overlaps libc |
| serde_yaml | 0.9.34+deprecated | excluded | Deprecated; RUSTSEC-2018-0005 |
| serde_yml | 0.0.13 | excluded | Unmaintained; RUSTSEC-2025-0068 |
| yaml-rust | not stated | excluded | RUSTSEC-2024-0320 unmaintained |
| comrak | 0.57.0 | excluded | Native default features; XSS advisories |
| genpdf | 0.2.0 | excluded | No release since 2021-06 |

`default_features` is `null` in `crates.json` where T03 states none (no spike manifest exists for these entries).

## 4. Advisory exception (K-A, spike-only)

| Advisory | Locked crate | Kind |
|---|---|---|
| RUSTSEC-2026-0370 | zvariant 5.15.1 | informational: unmaintained |
| RUSTSEC-2026-0371 | zbus_names 4.3.5 | informational: unmaintained |
| RUSTSEC-2026-0372 | zvariant_derive 5.15.1 (proc-macro) | informational: unmaintained |

- Attached to the `keyring` entry (dependency path `keyring` -> `zbus-secret-service-keyring-store` -> `zbus` 5.19.0 -> the three crates; `evidence/P00/P00-T07/advisory-exception.md`).
- Not an advisory-ignore configuration; scope is this spike, these exact versions, checksums and paths. It does not extend to another advisory, version, graph, P01+ workspace, or any production or release build.
- Mandatory review points: before P03 or backend adoption, and again before any release (FR-24).
- **Open point (not decided by any record):** the exception names the lockfile hash `eeca61f5…099c`; the current lockfile differs because the T08 stage A lock-preserving resolution added packages without changing any existing version (`evidence/P00/P00-T08/record.md`). The exception text says a re-resolution voids it. Whether it continues to cover the current lockfile is an owner/Opus decision.
- T08 A2 found no applicable advisory for its new crates, so no further exception exists (`evidence/P00/P00-T08/record.md`).

## 5. Build rules (from the records)

1. Build per package (`-p`), never `--workspace`, never several spike packages in one invocation (feature unification).
2. Exact `=` pins; no extra features and no upgrades without owner approval and re-verification.
3. Toolchain binaries invoked directly; `env -i` with only `PATH`, `HOME`, `CARGO_HOME`, `CARGO_TARGET_DIR`, `RUSTC`; isolated `CARGO_HOME` and `CARGO_TARGET_DIR` under `~/.local/share/pwt-dev-tools/` (cache separation, not a sandbox); no `.cargo/config*` in ancestors.
4. Lock-preserving resolution (`cargo update --workspace`), not `generate-lockfile`; verify that no existing version changed.
5. `cargo fetch --locked --target x86_64-unknown-linux-gnu`, independent `.crate` SHA-256 checks, lockfile and manifest hashes compared before and after each step.
6. Offline locked builds inside `unshare -rn` (`cargo build --frozen -p <package>`). No filesystem sandbox: build scripts and proc-macros run with the user's file permissions; pattern review is not an audit.
7. Spike binaries run only through the recorded namespace commands with the verified environment check first.
8. RustSec screening of the locked graph before build; cargo-audit deferred to P01.
9. Bash arrays for Cargo option lists (recurring zsh word-splitting trap).

## 6. Limitations carried by this freeze

- Transitive graphs and build-time code were reviewed by pattern only; no filesystem sandbox existed for build code; crates.io integrity rests on TLS plus index checksums (no per-crate signature).
- RustSec screening covered published advisories only; yanked status was not queried everywhere; T05 and T06 crates were not re-screened at later stages.
- Spike passes are not production evidence; each FR in `plan.md` §5.2 must be implemented and re-verified in its phase.
- Combined (unified) builds of different spike packages were not tested.

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

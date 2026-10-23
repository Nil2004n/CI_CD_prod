# P00-T03 — Crate / dependency request (revision 2)

Task: P00-T03 (owner-approved 2026-10-24, documentation only). Revision 1 (`f1bd80d`) had all versions unresolved; revision 2 fills them from the C-1 read-only research (`evidence/P00/P00-T03/research.md`) under owner decisions C-1–C-6 of 2026-10-24.

Status: **request only.** No crate archive has been downloaded, no dependency resolved, nothing compiled or executed. Versions below are crates.io metadata at lookup time; licences are crates.io-declared SPDX expressions (upstream licence files not inspected unless noted). Transitive graphs are **not reviewed**.

Local toolchain: rustc/cargo 1.97.1 (stable, x86_64-unknown-linux-gnu). Every selected version's declared `rust_version` is ≤ 1.97.1 unless marked unknown.

## 1. Decisions applied (owner, 2026-10-24)
C-1 research done (this revision). C-2 RustSec screening for direct candidates; cargo-audit stays in P01. C-3 isolated `CARGO_HOME`/`CARGO_TARGET_DIR` under `~/.local/share/pwt-dev-tools/` (cache/output separation only — **not a sandbox**; dependency code still runs with the user's file access). C-4 `spikes/rust/` with a small shared workspace; lockfiles only from real resolution. C-5 build-code execution acknowledged, **not** blanket approval. C-6 one primary + at most one fallback per capability; start with the minimum set for the first spike.

## 2. Selection per capability

| Capability (spike) | Primary | Fallback | Excluded / not selected | Basis |
|---|---|---|---|---|
| Process-group signalling (T05) | **libc 0.2.191** (MIT OR Apache-2.0, MSRV 1.65) | rustix 1.1.5 (`process` feature; Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, MSRV 1.65) | nix 0.31.3 (overlaps; not needed) | No advisories for libc/rustix; nix advisory RUSTSEC-2021-0119 does not affect 0.31.3. `std::os::unix::process::CommandExt::process_group` + `libc::kill(-pgid, …)` is the smallest surface |
| Async supervision (T05 second stage / P06) | **tokio 1.54.1** (MIT, MSRV 1.85), features `rt`, `process`, `time`, `io-util`, `sync` (no `full`, no `macros` initially) | — | — | All 7 tokio advisories have patched ranges that include 1.54.1; 1.54.0 was yanked for RUSTSEC-2026-0384. Release is 3 days old (no age rule, R-3) |
| YAML parsing (T06) | **saphyr 0.2.2** / **saphyr-parser 0.2.2** (MIT OR Apache-2.0, MSRV 1.85.0) | yaml-rust2 0.13.0 (MIT OR Apache-2.0, MSRV 1.85.0) | serde_yaml (`0.9.34+deprecated`; RUSTSEC-2018-0005); serde_yml (self-declared deprecated/unmaintained; RUSTSEC-2025-0068 unsound+unmaintained); yaml-rust (RUSTSEC-2024-0320 unmaintained) | Duplicate-key detection, alias bounds and span quality **cannot be established from metadata → requires approved build test (T06)** |
| JSON Schema 2020-12 (T06) | **jsonschema 0.60.1** (MIT, MSRV 1.85.0) with `default-features = false` | boon 0.6.1 (MIT OR Apache-2.0, **MSRV not declared**, last release 2025-01-07) | — | Defaults pull `resolve-http`, `resolve-file`, `tls-aws-lc-rs` (remote `$ref` fetch + native TLS) — must be off; whether core Draft 2020-12 + `format` checking work with defaults off **requires build test**. 0.60.1 is 1 day old. boon compatibility with 1.97.1 requires build test |
| Serde (T06 harness, P01) | **serde 1.0.229** (`derive`), **serde_json 1.0.154** | — | — | No advisories; `derive` brings the `serde_derive` proc-macro |
| OS credential store (T07) | **keyring 4.2.0** (MIT OR Apache-2.0, MSRV 1.88.0), default feature `v1` → Linux `zbus-secret-service-keyring-store` (`crypto-rust`) | none researched (unresolved; e.g. a direct Secret Service crate would need a new lookup) | `cli` feature (pulls `dbus-secret-service-keyring-store` with `vendored`) | v4 is a restructured "all-in-one" crate over `keyring-core`; headless behaviour is the T07 question |
| Markdown model (T08, Rust-native path) | **pulldown-cmark 0.13.5** (MIT, MSRV 1.71.1), `default-features = false` (default enables `getopts`, `html`) | — | comrak 0.57.0 (BSD-2-Clause; default `syntect-onig` native lib; XSS advisories RUSTSEC-2021-0063/-0026 with ranges not retrieved) | Decide only after the T08 renderer shortlist |
| PDF generation (T08) | **printpdf 0.12.8** (MIT, MSRV 1.88), `default-features = false` (default `html`); `text_layout` to evaluate | typst 0.15.1 (Apache-2.0, MSRV 1.92, no features; large) | genpdf 0.2.0 (no release since 2021-06) | Unicode/RTL/CJK and "no remote resources" behaviour **require build test**; not part of the first request |
| Hashing / temp dirs (harness) | **sha2 0.11.1** (MSRV 1.88; RUSTSEC-2021-0100 affects only 0.9.7), **tempfile 3.27.0** (MSRV 1.63) | — | — | Only when a spike needs them |

## 3. Build-code exposure known so far (not exhaustive)

| Crate | Build script | Proc-macro | Native library | Evidence |
|---|---|---|---|---|
| libc 0.2.191 | **yes** (`build.rs` at crate root) | no | no (`lib_links` null) | docs.rs source listing; crates.io |
| tokio 1.54.1 | not established | `tokio-macros` only with `macros` feature | no (Linux) | crates.io dependencies |
| serde 1.0.229 | not established | `serde_derive` with `derive` | no | crates.io features |
| jsonschema 0.60.1 | not established | not established | `aws-lc` only if `tls-aws-lc-rs` enabled (to be disabled) | crates.io features |
| others | not established | not established | not established | — |

"Not established" means unknown, not "absent". The resolved lockfile review (§5 step 3) is where the full graph is first seen.

## 4. Proposed first minimal request — T05 stage 1 (libc only)

Purpose: give P00-T05 (process-tree supervision spike) the single crate needed for process-group signalling; Tokio is a later, separate request.

| Item | Proposal |
|---|---|
| Direct dependency | `libc = "=0.2.191"` (default features: `std`) |
| Expected graph | libc only (its sole dependency is optional and unused here) — **to be confirmed by resolution**, not assumed |
| Workspace | `spikes/rust/Cargo.toml` (workspace, `resolver = "2"`), member `spikes/rust/t05-process/` (binary crate). Created at the resolution step, not before |
| Toolchain | invoke `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo` and `rustc` directly (no rustup proxy, so no toolchain update); record `rustc -vV` |
| Environment | `env -i PATH=<toolchain bin>:/usr/bin:/bin HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_TARGET_DIR=~/.local/share/pwt-dev-tools/_cargo-target RUSTC=<toolchain rustc>` — no API keys, tokens or unrelated variables; global `~/.cargo` not used |
| Step A — resolve (network) | `cargo generate-lockfile` in `spikes/rust/` → expect only `index.crates.io`. Exit captured directly. Review `Cargo.lock`: every package, version, `source = registry+https://github.com/rust-lang/crates.io-index` (sparse), checksum equals research value `65cd87c9…240a` |
| Step B — fetch (network) | `cargo fetch --locked` → `static.crates.io`; Cargo verifies the `.crate` checksum against `Cargo.lock` |
| Step C — offline build | `unshare -rn <env> cargo build --offline --locked` — network is **enforced absent** by a new network namespace (only a down loopback), not merely by `--offline`. libc's `build.rs` runs here with the user's filesystem access |
| Step D — spike execution | **Not part of this request**; P00-T05 execution needs its own approval |
| Paths written | `spikes/rust/**` (repo), `~/.local/share/pwt-dev-tools/_cargo-home/`, `~/.local/share/pwt-dev-tools/_cargo-target/` |
| Commit | `spikes/rust/Cargo.toml`, member manifest and the **resolved** `Cargo.lock` after review (target dir is outside the repo) |
| Rollback | delete `_cargo-home/`, `_cargo-target/`, and `spikes/rust/` (uncommitted) |
| Isolation limits | `env -i` strips credentials from the environment; `unshare -rn` blocks network during build; there is **no filesystem sandbox** — build scripts can read files the user can read. Cache separation is not sandboxing |
| Integrity limits | crates.io: TLS + index checksums; no verifiable per-crate signature/attestation in this plan (checksum-only, like the PyPI exception). Direct-candidate RustSec screening done; resolved-graph advisory review = manual check of each locked package against RustSec (cargo-audit deferred to P01) |

## 5. Generic procedure for later requests
1. Metadata research (done for current candidates). 2. Resolution (network, index only) → lockfile review incl. sources, checksums, new packages, RustSec check per package. 3. Fetch (network, crate files). 4. Offline locked build in a network namespace. 5. Spike execution (separate approval). Each step is approved and recorded separately; exit codes captured directly.

## 6. Unknowns needing a later approved build test (not further research)
saphyr vs yaml-rust2 duplicate-key/alias/span behaviour; jsonschema with defaults off (Draft 2020-12, `format`, no remote `$ref`); boon MSRV; keyring headless Secret Service behaviour; printpdf/typst Unicode shaping and resource isolation; build scripts/proc-macros/native deps of every transitive dependency.

## 7. Remaining decisions
| ID | Decision | Recommendation |
|---|---|---|
| D3-1 | Approve the §4 libc-only request: Step A (resolve), B (fetch), C (offline build) — no spike execution | Approve, together with or just before P00-T05 |
| D3-2 | Accept primaries/fallbacks in §2 as the candidate set for later per-spike requests | Accept |
| D3-3 | T06/T07/T08 dependency requests to be presented when those spikes are proposed | Yes |

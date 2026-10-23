# P00-T03 — Crate / dependency request

Task: P00-T03 (owner-approved 2026-10-24, documentation only). Base revision `008e80c` (= `origin/main`).

Status: **request only.** No crate has been downloaded, resolved, built or tested. No external lookup was made for this task (none authorised). Every version, MSRV and licence below is therefore **unresolved** unless it is derived from existing project evidence; licence notes from prior knowledge are marked "to confirm" and must not be relied on. §6 lists the exact lookups needed to resolve them.

Existing evidence used: `plan.md` §4–§5, Technical Specification §2 (candidate stack) and §14, Data Schema §15, `spikes/P00-T01-spike-protocols.md` §4.2–§4.5, local non-mutating checks below.

Local toolchain (checked 2026-10-24, `rustc --version`, `cargo --version`, `rustup show active-toolchain`): rustc 1.97.1 (`8bab26f4f`, 2026-07-14), cargo 1.97.1, default `stable-x86_64-unknown-linux-gnu`. A pre-existing `~/.cargo/registry` (index + cache) of unknown origin exists; this request avoids depending on it (§4).

## 1. Scope and staging

Two tiers. **Tier S** is what the P00 spikes need now. **Tier P** is the P01 workspace baseline, listed so P01 does not invent dependencies, but it is requested for approval only when P01 is approved.

| Tier | Needed by | Why now |
|---|---|---|
| S | P00-T05 process supervision, P00-T06 YAML/schema, P00-T07 keyring, P00-T08 PDF (Rust candidates) | Spikes decide crate selections that P01 freezes; without them T05–T08 cannot run |
| P | P01 scaffolding (D-01 stack) | Information only; separate approval at P01 |

## 2. Tier S — spike crates

| ID | Crate (candidate) | Purpose | Spike | Proposed version | Features of interest | Licence (to confirm) | Notes |
|---|---|---|---|---|---|---|---|
| S-01 | `libc` | `kill(-pgid, sig)`, signal constants for process-group termination; `std` already provides `CommandExt::process_group` | T05 | **unresolved** | default | MIT OR Apache-2.0 (to confirm) | Preferred minimal option; alternative S-02 |
| S-02 | `rustix` *or* `nix` | Safe wrappers for process/signal syscalls instead of raw `libc` | T05 | **unresolved** | process/signal features only | to confirm | Choose one; compare unsafe surface vs dependency weight |
| S-03 | `tokio` | Async child supervision, timers, cancellation (D-01 confirmed stack) | T05 | **unresolved** | `rt`, `process`, `time`, `io-util`, `macros`, `sync` (no `full`) | MIT (to confirm) | Same crate as P01; spike confirms kill/timeout semantics under Tokio |
| S-04 | `saphyr` / `saphyr-parser` | YAML 1.2 parser with event/marker spans; candidate for duplicate-key detection, alias bounds, source mapping | T06 | **unresolved** | default | to confirm | Candidate A |
| S-05 | `yaml-rust2` | Alternative maintained YAML parser | T06 | **unresolved** | default | to confirm | Candidate B |
| S-06 | `serde_yaml` | — | T06 | **not proposed** | — | — | Prior knowledge: upstream archived/unmaintained; excluded unless a lookup shows otherwise |
| S-07 | `serde_yml` | — | T06 | **not proposed pending advisory lookup** | — | — | Prior knowledge: maintenance/soundness concerns reported; include only if RustSec lookup is clean |
| S-08 | `jsonschema` | Draft 2020-12 validation, explicit `format` checking, controllable `$ref` resolution (Tech §2 candidate) | T06 | **unresolved** | disable default remote resolution features if present | MIT (to confirm) | Validate against CA-01 schemas and Data Schema examples |
| S-09 | `boon` | Alternative Draft 2020-12 validator | T06 | **unresolved** | default | to confirm | Comparison only |
| S-10 | `serde`, `serde_json` | Data model / JSON handling for T06 harness | T06 | **unresolved** | `serde/derive` | MIT OR Apache-2.0 (to confirm) | Same as P01 |
| S-11 | `keyring` | OS credential store abstraction; Linux Secret Service backend | T07 | **unresolved** | Linux Secret Service backend only (exact feature names unresolved; they changed across major versions); no keyutils-only or mock backend in the decision run | MIT OR Apache-2.0 (to confirm) | Decides KEY-01 OS-store mode; dummy credential only |
| S-12 | `pulldown-cmark` *or* `comrak` | Markdown → document model for Rust-native PDF candidates; raw HTML can be disabled | T08 | **unresolved** | HTML rendering off | to confirm | Needed only for in-process candidates |
| S-13 | `printpdf` *or* `genpdf` | In-process PDF generation (no browser engine, no network) | T08 | **unresolved** | font embedding | to confirm | Unicode/RTL/CJK shaping capability unknown — key spike question |
| S-14 | `typst` (library crates) | Typst-based rendering as an in-process option | T08 | **unresolved** | no package/network download feature | to confirm | Large dependency tree; requires a "no remote packages/fonts" configuration to be verified |
| S-15 | `sha2`, `tempfile` | Hashing fixture outputs; scratch directories in harnesses | T05–T08 | **unresolved** | default | to confirm | Small utilities |

External-renderer candidates for T08 (e.g. restricted WeasyPrint, pandoc, typst CLI) are **not** crates and are outside this request; they need the T08 installation request.

## 3. Tier P — P01 baseline (information only)

`ratatui`, `crossterm` (D-01); `tokio`, `serde`, `serde_json` (S-03, S-10); `uuid` (v4 IDs), `time` (UTC RFC 3339), `thiserror`, `walkdir`, `globset`, `sha2`, `tempfile`; test-only: `insta` (snapshots), `proptest`. Later phases (not P01): `reqwest` with rustls (P16, OpenRouter), mock HTTP server crate (P16). Versions **unresolved**; selections from T05–T08 replace candidates above. cargo-audit (deferred to P01 by R-4) is the advisory gate for this tier.

## 4. Proposed operations (for a later approval; none executed)

Isolation:
- `CARGO_HOME=~/.local/share/pwt-dev-tools/_cargo-home` for all spike Cargo operations, so the pre-existing `~/.cargo/registry` is neither used nor modified and rollback is one directory.
- Spike crates live in the repository under `spikes/rust/<spike-id>/` (one small binary crate each), each with exact `=x.y.z` requirements in `Cargo.toml` and a committed `Cargo.lock`. `target/` directories are git-ignored and placed under `CARGO_TARGET_DIR=~/.local/share/pwt-dev-tools/_cargo-target`.

Steps per spike crate:
1. Write `Cargo.toml` with exact pins (after §6 lookups resolve versions).
2. **Resolve (network):** `cargo generate-lockfile` → review the full transitive list in `Cargo.lock` (names, versions, sources = crates.io only, no git dependencies) before any download of crate contents.
3. **Fetch (network):** `cargo fetch --locked` — downloads `.crate` files; Cargo verifies each against the checksum recorded in `Cargo.lock` (from the crates.io index).
4. **Build/test (offline):** `cargo build --offline --locked` and the spike's own test commands. **Build scripts and proc-macros of dependencies execute on the host** during this step; this is normal Cargo behaviour and the main code-execution exposure.
5. Record per check: command, exit (captured per `evidence/README.md`), lockfile digest, `rustc -vV`.

Network endpoints: `index.crates.io` (sparse index), `static.crates.io` (crate files). No git dependencies, no alternative registries, no `build.rs`-driven downloads accepted (a crate that downloads at build time is rejected or isolated with network off and reported).

Affected paths: `spikes/rust/**` (repo), `~/.local/share/pwt-dev-tools/_cargo-home/`, `~/.local/share/pwt-dev-tools/_cargo-target/`. Rollback: delete those two directories and the spike crate directories.

## 5. Verification plan and limitations

- Integrity: Cargo checksum verification against the crates.io index (TLS). crates.io does not provide per-crate signatures/attestations that this plan can verify; this is a **checksum/TLS-only** limitation like the PyPI exception, and is not provenance verification.
- Advisories: before fetch, check each resolved crate+version against the RustSec advisory database. Options: (a) read-only lookup of RustSec pages per crate (needs approval, §6); (b) bring cargo-audit forward from P01 (changes R-4; see §7); (c) defer advisories to P01 for spike-only code (weakest).
- Licences: confirm SPDX per crate+version from crates.io metadata; spike code is not distributed, but selections must be compatible for P01/P20.
- MSRV: each pinned version's `rust_version` must be ≤ 1.97.1 (local toolchain).

## 6. Lookups required to resolve this request (not performed)

For each crate in §2 (S-01–S-05, S-07–S-15) and, when P01 is approved, §3:
1. `https://crates.io/api/v1/crates/<crate>` — `max_stable_version`, `repository`, last update; yanked status.
2. `https://crates.io/api/v1/crates/<crate>/<version>` — `license`, `rust_version`, `checksum`, `features`, `yanked`.
3. `https://rustsec.org/packages/<crate>.html` (or the advisory-db GitHub repository listing for that crate) — open advisories/unmaintained notices.

These are read-only metadata lookups of the same kind as Q-06; they are requested as a **new, separately approved** lookup scope for P00-T03.

## 7. Changes to existing approvals (requested decisions)

| ID | Decision | Recommendation |
|---|---|---|
| C-1 | Approve the §6 read-only lookups for Tier S so versions/licences/MSRV/advisories can be filled in (still no downloads) | Approve |
| C-2 | Advisory check method for spike crates: (a) RustSec page lookups, (b) move cargo-audit from P01 into P00 (amends R-4), (c) defer | (a) now; cargo-audit stays in P01 |
| C-3 | Isolated `CARGO_HOME`/`CARGO_TARGET_DIR` under the tool prefix (§4) | Approve |
| C-4 | Spike crates committed under `spikes/rust/` with `Cargo.lock`; `target/` ignored | Approve |
| C-5 | Accept build-script/proc-macro execution during offline spike builds as an explicit exposure, limited to the reviewed lockfile | Your call; required for any Rust spike |
| C-6 | Candidate set: keep or drop S-02, S-05, S-09, S-12–S-14 alternatives (fewer candidates = fewer downloads) | Keep T06 alternatives; decide T08 crates after the T08 renderer shortlist |

Download/build approval for the resolved lockfiles is a later, separate request (after C-1 lookups), per spike.

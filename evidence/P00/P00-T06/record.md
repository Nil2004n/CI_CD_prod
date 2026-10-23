# P00-T06 — dependency resolution, advisory screen, fetch and build-code review (paused before build)

Date: 2026-10-24. Authority: owner approval of the bounded T06 request (A → A-review → A2 → B → inspection → C → D), 2026-10-24. Executed by Opus on `phase/P00-decisions-spikes` (base `f0d9361`). Environment for all Cargo commands: toolchain binaries invoked directly (rustc/cargo 1.97.1), `env -i`, isolated `CARGO_HOME`/`CARGO_TARGET_DIR` under `~/.local/share/pwt-dev-tools/`.

**Status: PAUSED before step C (build)** under the owner's rule "pause before building if inspection reveals materially different execution exposure from the approved request". No spike code was written, nothing was compiled, nothing was executed. Steps A, A2 and B completed with all stop conditions satisfied.

## A — resolution
- Workspace member `t06-yaml` added with exactly `saphyr-parser =0.2.2`, `jsonschema =0.60.1` (`default-features = false`), `serde_json =1.0.154`; `src/main.rs` is a placeholder. `cargo generate-lockfile` → exit 0, "Locking 83 packages" (`A-resolve.log`).
- `A-review.txt`: 85 packages (83 registry + 2 local); sources: crates.io registry only; `libc 0.2.191` unchanged (checksum `65cd87c9…`); no TLS/HTTP/native crates (`reqwest`, `hyper`, `rustls`, `aws-lc*`, `ring`, `openssl`, `url`, `idna` absent); direct deps exactly the three approved.
- Name-pattern hits (`wasm-bindgen*`, `js-sys`, `wit-bindgen`, `wasip2`, `r-efi`, `windows-link`, `redox_syscall`) are other-target dependencies of `getrandom`/`parking_lot_core`; confirmed absent from the host build graph (below).

## A2 — RustSec screen (`A2-advisories.txt`)
Advisory-db `crates/` listing via GitHub contents API (HTTP 200; 966 entries < 1000 cap). 83 locked crate/versions checked; 7 have advisory directories. Each advisory file's TOML `[versions]` was fetched from the official advisory-db repository and evaluated against the locked version, then checked by hand:

| Crate (locked) | Advisories | Result |
|---|---|---|
| bumpalo 3.20.3 | RUSTSEC-2020-0006, RUSTSEC-2022-0078 (unsound) | patched (≥3.2.1, ≥3.11.1) |
| hashbrown 0.17.1 | RUSTSEC-2024-0402 | patched (≥0.15.1) |
| lock_api 0.4.14 | RUSTSEC-2020-0070 (unsound) | patched (≥0.4.2) |
| once_cell 1.21.4 | RUSTSEC-2019-0017 | patched (≥1.0.1) |
| regex 1.13.1 | RUSTSEC-2022-0013 | patched (≥1.5.5) |
| smallvec 1.16.4 | 5 advisories (2018–2021) | all patched (≥1.6.1 for the latest) |
| zerocopy 0.8.63 | RUSTSEC-2023-0074 | patched (≥0.7.31) |

The other 76 locked crates have no advisory directory. Scope: published advisories only; absence of an advisory is not evidence of code safety; yanked status was not separately queried (Cargo does not select yanked versions during resolution).

## B — fetch (`B-fetch.log`, `B-checksums.txt`)
`cargo fetch --locked --target x86_64-unknown-linux-gnu` → exit 0 (68 downloads + cached libc). Independent SHA-256 of every fetched `.crate` vs `Cargo.lock`: 69 verified, 0 mismatches; 14 not fetched (other-target crates and `zerocopy-derive`, not enabled on the host). `Cargo.lock` unchanged by fetch (`lock-after-A.sha256` = `lock-after-B.sha256`).

## Inspection — host build graph, features, build code
- `cargo tree --frozen --target x86_64-unknown-linux-gnu -p t06-yaml -e normal,build` → 70 packages (`B-host-packages.txt`, features in `B-tree-features.txt`). A first attempt returned exit 127 because zsh did not word-split a command stored in a variable; its "none found" output was discarded and the command re-run explicitly (exit 0).
- Features: `jsonschema` **[] (no features)**; `referencing` `[default]` where `default = []`; no `resolve-http`, `resolve-file`, `tls-*`, `idna` or `reqwest` anywhere; `serde_json` `[default, float_roundtrip, std]` (no `arbitrary_precision`); `serde` `[derive, …]` — requested by `jsonschema` itself and by `bit-set`/`bit-vec`, not by the spike.
- Source-level reference handling (`B-api-review.txt`): with the features off, `jsonschema`'s `DefaultRetriever` returns fixed errors ("`resolve-http` feature or a custom resolver is required…", "`resolve-file` feature or a custom resolver is required…") with the HTTP client and `File::open` compiled out. `with_retriever` allows an instrumented retriever; `should_validate_formats`, `with_draft`, `with_registry` exist; saphyr-parser `Marker` exposes `index()`, `line()`, `col()` (units unverified).
- **Build-code exposure (`B-build-exposure.txt`)** in the host graph: **16 build scripts** — ahash, getrandom, jsonschema-value, libc, num-traits, parking_lot_core, proc-macro2, quote, ref-cast, serde_core, serde_json, serde, thiserror, unicode-general-category, zerocopy, zmij; **4 procedural-macro crates** — ref-cast-impl, serde_derive (8,975 lines), strum_macros, thiserror-impl (plus `syn`/`quote`/`proc-macro2` as their compile-time support); **0 native `links`**.
- Pattern review (`B-pattern-review.txt`; grep-based, not an audit): build-script process spawns are `rustc --version`/`-vV` probes or probe compilations (`proc-macro2`, `thiserror`), and libc's target-gated FreeBSD/Emscripten probes; file writes/creates/removals are confined to `OUT_DIR` (generated `private.rs`, `category.rs`, an `OUT_DIR/probe` directory). No network, home-directory or `set_var` patterns. The four procedural-macro crates had no process/filesystem/network pattern matches (one hit is a comment).

## Why paused
The approved request anticipated build-code exposure as "likely at least libc and serde_json". Actual exposure is 16 build scripts and 4 procedural-macro crates (compile-time code execution with the user's filesystem permissions; the build runs in a network namespace but there is no filesystem sandbox). `serde_derive` executes despite the request excluding a direct serde-derive dependency, because `jsonschema` requires it. This is treated as materially different exposure; step C waits for an explicit owner decision.

## Not done
Spike composer/projection/harness, fixtures, build, run, J16 comparison.

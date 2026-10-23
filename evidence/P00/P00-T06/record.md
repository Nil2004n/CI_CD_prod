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

## Resumed after owner risk acceptance (2026-10-24) — implementation, build, run

Owner accepted the disclosed build-time exposure (16 build scripts, 4 proc-macro crates incl. transitive `serde_derive`) for this exact locked graph only; no filesystem sandbox; pattern review is not an audit; advisory screening covers published advisories only.

**Result: formal verification FAILED — 101 rows: 96 pass, 5 fail, 6 observations (harness exit 1). J16: 35 agree / 0 disagree. P00-T06 is not complete; no repair applied.**

### Integrity before build
`Cargo.lock` SHA-256 equals the recorded post-fetch value; manifests/lockfile unchanged since `2931774`; 69 fetched `.crate` files re-verified (0 mismatches). No re-resolution or re-fetch.

### Spike code (`spikes/rust/t06-yaml/src/`)
- `compose.rs`: consumes saphyr-parser events; limits per **input** (1 MiB input, 100 documents, depth 64, 1,000 alias events, 100,000 expanded nodes, 64 KiB scalar) so multiple documents cannot bypass them; expanded-node budget is checked on each alias/node attach using the anchored subtree's precomputed size, **before** any expansion is materialised; alias cycles (alias to a still-open anchor) rejected; duplicate keys detected on the lossless entry list (resolved type + value) before any map exists; YAML **1.2 core schema** scalar resolution for untagged plain scalars (quoted/block → string; `yes/no/on/off`, `1_000`, date-like → string; `True/TRUE` → bool; `~/null/empty` → null; `0o`/`0x`/decimal ints; core floats incl. `.inf/.nan`; finite literals overflowing f64 kept as text); core tags `!!str/int/float/bool/null/seq/map` applied (value mismatch rejected); any other tag recorded, never instantiated.
- `project.rs`: lossless projection to `serde_json::Value` (no `arbitrary_precision`); unsupported with precise reasons: `non_finite_float(inf|nan)`, `integer_outside_lossless_range(i64/u64)`, `float_literal_overflows_f64`, `non_string_key`, `complex_key`, `merge_key(<<)`, `tag:<name>`, `key_collision_after_projection`.
- `main.rs`: harness (refuses unless PID 1), Y/J cases with expected outcomes, instrumented `Retrieve` implementation recording requested URIs, J16 export, scratch cleanup and `/proc` check.
- Fixtures: 36 text files under `t06-yaml/fixtures/` (YAML + schemas); bombs, size/depth/document/alias-budget inputs, invalid UTF-8, NUL, BOM/CRLF and the `file:` canary schema are generated at run time in the scratch directory and removed.
- Design correction disclosed: the T06 design expected Y07 (`1:` and `"1":`) to end as `key_collision_after_projection`, which contradicts the approved Y19 policy (non-string keys unsupported). Implemented per Y19: the composer does **not** treat `1` and `"1"` as duplicates; projection reports `non_string_key`.

### Build (C)
`unshare -rn env -i … cargo build --frozen -p t06-yaml`: attempt 1 exit 101 (one borrow error in spike code, `C-build-attempt-1.log`; all dependency build scripts and proc-macros ran during this attempt, inside the network namespace); fixed; attempt 2 exit 0, no warnings (`C-build.log`). `Cargo.lock` unchanged. Artifact `~/.local/share/pwt-dev-tools/_cargo-target/debug/t06-yaml`, SHA-256 `163bbee141506449f67b8ae36f259a7cbb6d0ee0c886935ebd006e148727e33e` (not committed).

### Run (D)
Setup check (`D-env-check.log`): inner shell PID 1; `ulimit -v` 1,048,576 KiB also in the exec'd child (still PID 1); only own PIDs; `lo` DOWN, 0 routes, `curl` exit 7; watchdog exit 137, 0 host leftovers; J16 interpreter `/usr/bin/python3` 3.14.7, jsonschema 4.26.0, rfc3339-validator 0.1.4.
Command: `timeout -s KILL 120 unshare -rn --pid --fork --mount-proc --kill-child env -i PATH=/usr/bin:/bin LANG=C.UTF-8 bash -c 'ulimit -v 1048576 && exec "$0" "$@"' <artifact> <fixtures> <repo> <scratch> results.json j16-cases.json` → exit 1 (`D-run.log`, `results.json`). Scratch removed; no other processes in the namespace at the end.

### Failures (confirmed vs expectation errors)
| Case | Observed | Diagnosis |
|---|---|---|
| Y16a unicode (LF) | target reported at `index 35, line 4, col 7`; UTF-8 byte offset is 58 | **Confirmed parser behaviour:** saphyr-parser 0.2.2 `Marker::index()` is a **character** offset (35 = character count), contradicting its doc comment "index (in bytes)"; `col` is a **0-based character** column; `line` 1-based. Positions are sufficient (convertible), so the parser failure criterion is not met; the composer must convert |
| Y16b unicode (BOM + CRLF) | same units; first key `\u{feff}名前` | **Confirmed:** a leading UTF-8 BOM is **not** stripped by `Parser::new_from_str`; it becomes part of the first key. CRLF line counting correct |
| Y14b depth 10,000 | `syntax_error: recursion limit exceeded` at character 258 | **Confirmed:** saphyr-parser has its own nesting limit and its scanner runs ahead of emitted events, so it fails before the composer sees the 65th collection start. Input rejected boundedly (no stack overflow), but classified as a syntax error rather than a limit. Y14a (depth 65) hit the composer limit as designed |
| Y18c/Y18d tag names | `tag:tag:yaml.org,2002:python/object:os.system`, `…:binary` | **Expectation error (mine):** the parser expands `!!` to `tag:yaml.org,2002:`; both tags were correctly unsupported and not instantiated |

### Other evidence
- Duplicate keys (Y06a–c) rejected with both positions; `"a"` and `a` treated as the same key; `1` vs `"1"` not a YAML duplicate (Y07).
- Alias budget: Y11 bomb stopped at `expanded_nodes > 100000` after 38 alias events, before materialisation, 0 ms; Y15d (25 alias events × 50 documents) stopped at the per-input alias limit.
- Cycles (Y12a/b) → `policy_rejected:alias_cycle` (composer); undefined alias (Y13) → parser `syntax_error: found unknown anchor`; NUL (Y17b) → parser `found unprintable character`; invalid UTF-8 → input gate.
- Syntax error positions (Y03a–c) on the declared lines.
- Format assertion: `date-time`/`uuid` invalid values rejected with assertion on and accepted with it off (J04/J05).
- References: with the default retriever, both `https:` and `file:` refs fail with the compiled-in "`resolve-http`/`resolve-file` feature or a custom resolver is required" messages (disabled path, not a network failure); with the instrumented retriever the requested URI is recorded and refused; canary file content absent from all output (J08/J09). Valid YAML + unavailable schema → `schema_unavailable` (J15).
- Draft handling (J10, observation): with `with_draft(Draft202012)`, a draft-07 schema using array-form `items` fails to build; a 2019-09 schema is applied as 2020-12.
- J16 (`j16_compare.py`, `J16-compare.log`, run in the same namespace setup): 35/35 agree on J01–J07, J11–J13 (Python `Draft202012Validator`, `FormatChecker` when formats on, empty `Registry`). Agreement is not a conformance proof.

### Proposed bounded debug cycle P00-T06-D01 (not started)
1. Composer: convert parser character offsets to UTF-8 byte offsets from the source text; report byte offset, 1-based line, 1-based character column; record the saphyr-parser doc/behaviour discrepancy.
2. Input gate: strip one leading UTF-8 BOM before parsing (recorded), positions reported relative to the original bytes.
3. Classify the parser's "recursion limit exceeded" error as `limit_exceeded:depth(parser)`; keep the composer limit of 64; record that the effective first limit for deeply nested flow input is the parser's (matching on error text is fragile and noted).
4. Correct the Y18c/d expected names to the expanded form `tag:yaml.org,2002:…` (expectation correction, not a behaviour change).
5. Retest: full harness and J16 under the same verified setup; no dependency or limit changes.

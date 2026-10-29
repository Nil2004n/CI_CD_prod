# P02-T02 — offline dependency resolution and build-exposure report

Date: 2026-10-27. Executed by Opus (lead-owned: manifests and lockfile are shared contracts; no worker spawned).
Authority: P02 A2 = 2a, A3 = 3a (owner, 2026-10-26); `planning/P02-T01-request.md` §4 (approved with T01-D1–D12, 2026-10-27).
Repository: `phase/P02-store` at `f454f92`, dirty with uncommitted T01 deliverables and status/handover edits.

**Result: PASS. The A2 and A3 conditions hold; no stop condition fired.** Nothing was downloaded, compiled or executed; the only cargo commands were `update` and `tree` (neither builds, so no build script ran).

## 1. Environment
- Toolchain: rustc/cargo 1.97.1, called directly (version gate as P01).
- Environment: `env -i` with the P01 variables; `CARGO_TARGET_DIR=…/_cargo-target/p02-t02` (unused, since nothing was built). Helper `t02_cargo.sh`.
- Network: every cargo command ran in `unshare -rn`. The probe connect to 1.1.1.1:443 failed with errno 101 (`netns-probe.txt`).
- Scratch workspace: `~/.local/share/pwt-dev-tools/_p02/t02-resolve/` (0700, outside the repo), holding the current `workbench/` manifests and lock (`fb8b7d07…`) plus the T01 §4 changes:
  - new member `crates/cli`, with `[[bin]] pwt-workbench-cli`;
  - workspace deps `jsonschema =0.60.1` (default features off), `sha2 =0.11.1` (default features off), and path deps;
  - `persistence` → domain, serde, serde_json, jsonschema, sha2; `application` → domain, persistence, serde, serde_json; `cli` → domain, application, serde, serde_json.
  - Stub `lib.rs`/`main.rs` only. Copies in `manifests/`.

## 2. Commands and exits
| Step | Command | Exit | Evidence |
|---|---|---|---|
| Resolve (lock-preserving) | `cargo update --workspace --offline` | 0 ("Locking 76 packages") | `update.log`, `update.result` |
| Cache unchanged | SHA-256 of the registry cache listing + index file count, before/after | equal (`664eca4e…`) | `update.result` |
| Host graph | `cargo tree --offline --locked --workspace -e normal,build --target x86_64-unknown-linux-gnu --format '{p} [{f}]'` | 0 | `p02-host-features.txt` (80 packages incl. 8 members) |
| Full tree | same with `-e normal,build,dev` | 0 | `tree-workspace.txt` |
| A2 lock check | `python3 -I -B compare_lock.py <resolved> spikes/rust/Cargo.lock workbench/Cargo.lock` | 1 → see §3 (one diagnosed P01-edge notation change; 86/86 registry entries equal P00) | `compare-lock.txt` |
| A2 negative control | same against a copy with every checksum altered | 1 (97 FAIL), as expected | `compare-lock-negative-control.txt` |
| A3 exposure | `python3 -I -B exposure.py` against the accepted built P00 host graphs T05, T06, T07, T08 | 0 (new_exposure 0) | `exposure.txt` |
| A3 negative control | same against the T05 graph only | 1 (17 new exposures), as expected | `exposure-negative-control.txt` |
| Archive checksums | SHA-256 of each cached `.crate` vs lock checksum | 74 verified / 0 mismatch / 12 not cached (non-Linux targets only) | `crate-checksums.txt` |

**Resolved lock:** `Cargo.lock.resolved`, 94 packages, SHA-256 `b34cb529a79905546621573fef5de55b2be29884b2028715ef67e68bf0ef1c26`. **T04 must reproduce this file byte for byte.**

## 3. A2: resolved set
- **86 registry entries, each equal to its `spikes/rust/Cargo.lock` entry** in (name, version, source, checksum). All come from crates.io, and no version is newer than P00.
- **The 18 P01 entries:** 17 are unchanged. `serde_derive 1.0.229` changed in its dependency list only: `"syn"` → `"syn 3.0.9"`.
  - **Diagnosis:** syn 2.0.119 entered the graph (through `strum_macros`/`ref-cast-impl`/jsonschema), so Cargo must qualify the edge by version. The target crate is the same (syn 3.0.9, already in the P01 lock), and the new text is byte-identical to the P00 lock's `serde_derive` entry.
  - **Classification:** lock notation, not a dependency change. `compare_lock.py` reports it as FAIL (exit 1) because it compares whole entries, and that output is preserved unchanged.
- The 12 entries that are not cached are only reachable on other targets: `wasm-bindgen*`, `js-sys`, `bumpalo` (wasm), `wasip2`/`wit-bindgen` (WASI), `r-efi` (UEFI), `redox_syscall`, `windows-link`. The twelfth, `zerocopy-derive`, is a disabled feature (`derive`), absent from the Linux host graph. A `--frozen` Linux build does not need them (same situation as the P00-T08 wasm-only crates).

## 4. A3: build-code exposure
- **Linux host graph:** 72 registry crates, **15 build scripts, 3 proc-macros, no `links`.**
- **Build scripts:** ahash, getrandom 0.3.4, jsonschema-value, libc, num-traits, parking_lot_core, proc-macro2, quote, ref-cast, serde, serde_core, serde_json, unicode-general-category, zerocopy, zmij.
- **Proc-macros:** ref-cast-impl, serde_derive, strum_macros.
- **Every one of the 72** is present at the same version, with a superset of features, in at least one accepted, built P00 host graph. Build scripts and proc-macros are covered by T06, which built the jsonschema graph; the sha2 chain is covered by T07/T08. No new build-script or proc-macro code is introduced relative to P00.
- New relative to the P01 workspace graph: 9 build scripts and 2 proc-macros, all from the accepted, built T06 graph.
- **Limitation (unchanged from P00/P01):** pattern-level review only, no filesystem sandbox for build code. This T02 executed nothing.

## 5. Planning-fact correction
`plan.md` §5B.2 and `spikes/p00-freeze/crates.md` call sha2 0.11.1 "provisional (no spike exercised them)". That is only partly correct:
- sha2 0.11.1, with digest 0.11.3, block-buffer, crypto-common, hybrid-array, cpufeatures 0.3.1 and typenum 1.20.1, was **built** as a dependency in the accepted T07 and T08 host graphs;
- no spike **called** a sha2 API. Its behaviour in our use (`Sha256` over report bytes) is first tested in P02-T10.

## 6. Output for T04
- Manifests: as in `manifests/` (plus lints and `cli` `[[bin]]`).
- `Cargo.lock` must equal `Cargo.lock.resolved` (`b34cb529…`).
- Runner allowlist = these 86 registry entries.
- The first compile in T04 runs the 15 build scripts and 3 proc-macros above, in the network namespace.

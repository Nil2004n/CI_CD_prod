# D3-1 — libc-only resolve, fetch and offline build (for P00-T05)

Date: 2026-10-24. Authority: owner approval D3-1 (A → B → C), 2026-10-24. Executed by Opus. Branch `phase/P00-decisions-spikes`, base `ccb0471`.

**Outcome: dependency build succeeded. The built binary was NOT executed. No `cargo run`, `cargo test`, doctest, process-signalling experiment or P00-T05 spike was performed.**

## Setup
- Workspace created: `spikes/rust/Cargo.toml` (workspace, resolver 2, member `t05-process`), `spikes/rust/t05-process/Cargo.toml` (`libc = "=0.2.191"`), `spikes/rust/t05-process/src/main.rs` (`use libc as _; fn main() {}` — compile target only).
- Toolchain invoked directly (no rustup proxy): `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/{cargo,rustc}`; rustc 1.97.1 (8bab26f4f, LLVM 22.1.6), cargo 1.97.1 (c980f4866).
- Environment for every Cargo command: `env -i PATH=<toolchain bin>:/usr/bin:/bin HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_TARGET_DIR=~/.local/share/pwt-dev-tools/_cargo-target RUSTC=<toolchain rustc> CARGO_TERM_COLOR=never`. No API keys, tokens or other variables passed.
- Pre-check: no `.cargo/config` or `.cargo/config.toml` in any ancestor of `spikes/rust/` (Cargo reads those regardless of `CARGO_HOME`). Global `~/.cargo` not used or modified.

## Commands and exit codes (captured directly, no pipes)

| Step | Command (cwd `spikes/rust/`) | Network | Exit | Log |
|---|---|---|---|---|
| A | `cargo generate-lockfile` | crates.io sparse index | 0 | `A-resolve.log` ("Locking 1 package") |
| A-review | Python `tomllib` comparison of `Cargo.lock` with research record | none | 0 | in this record |
| B | `cargo fetch --locked` | static.crates.io | 0 | `B-fetch.log` ("Downloaded libc v0.2.191") |
| C-netns | `unshare -rn sh -c '…ip -brief link; ip route; curl …'` | none | 0 | `C-netns-check.log` |
| C | `unshare -rn env -i … cargo build --frozen` | **none (network namespace)** | 0 | `C-build.log` |

## A — resolution review
- Resolved set: registry `libc 0.2.191`; local `t05-process 0.0.0`. Nothing else.
- Source: `registry+https://github.com/rust-lang/crates.io-index` (crates.io); no git or path dependencies besides the local member.
- Checksum in `Cargo.lock`: `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` = research record (`evidence/P00/P00-T03/research.md`).
- `Cargo.lock` SHA-256: `68050af59647de679193657afcd21a30964ca552a2896a4ff3e79419983e7e63`.

## B — fetch
- `.crate` file: `~/.local/share/pwt-dev-tools/_cargo-home/registry/cache/index.crates.io-1949cf8c6b5b557f/libc-0.2.191.crate`; independently computed SHA-256 `65cd87c9…240a` = lockfile = research.
- `Cargo.lock` unchanged by fetch (`lock-after-A.sha256` = `lock-after-B.sha256`).

## C — pre-build review (limited; not a code audit)
- Packaged `Cargo.toml`: `build = "build.rs"`; license `MIT OR Apache-2.0`; no `links` key; only dependency `rustc-std-workspace-core` (optional, feature `rustc-dep-of-std`, not enabled); default feature `std`.
- `build.rs` (630 lines, SHA-256 prefix `e70593206df4d2a3`): pattern review for process, filesystem, environment and network use.
  - Spawns `$RUSTC --version` (or `$RUSTC_WRAPPER`, unset here) to detect the compiler version.
  - Spawns `freebsd-version` only when target OS is FreeBSD, `emcc` only when target OS is Emscripten — not on this x86_64 Linux build.
  - Reads `CARGO_CFG_*`, `CARGO_FEATURE_RUSTC_DEP_OF_STD`, `RUST_LIBC_UNSTABLE_*`, `WIND_RELEASE_ID` environment variables.
  - No matches for filesystem writes/removal, `File`/`OpenOptions`, `std::net`, sockets, `include!`/`include_str!`/`include_bytes!`, `set_var`, `home_dir`.
  - Limitation: grep-based and partial reading; does not exclude behaviour hidden from these patterns.

## C — network isolation
- Established with `unshare -rn` (new user + network namespace; no sudo, no host policy change).
- Verified in an identically created namespace immediately before the build: only interface `lo` (DOWN), 0 routes, `curl https://static.crates.io/` exit 6 (DNS resolution failed), `curl https://1.1.1.1/` exit 7 (connection failed).
- The build ran in its own fresh namespace created the same way; `--frozen` (= `--locked --offline`) additionally prevented Cargo network use. Isolation was enforced by the namespace, not by `--offline`.

## C — build result
- `Compiling libc v0.2.191`, `Compiling t05-process v0.0.0`, finished dev profile, exit 0.
- Artifact (not executed): `~/.local/share/pwt-dev-tools/_cargo-target/debug/t05-process`, 4,353,416 B, SHA-256 `ff34633cae57337bfdf94a427f4746eaeb66feea392cc6881ade581c80c94054`. Build-script outputs under `_cargo-target/debug/build/libc-*`.
- `Cargo.lock` unchanged after build (`lock-after-C.sha256` = `lock-after-A.sha256`).

## Advisory check (actual scope)
- Direct dependency libc: no RustSec advisory directory at the time of the P00-T03 research lookup (2026-10-24). Not re-queried at build time.
- Transitive dependencies: none in the resolved graph, so no further packages to check.
- This is not a complete safety audit; cargo-audit remains deferred to P01 (R-4).

## Limitations (unchanged)
- **No filesystem sandbox.** libc's build script ran with the user's filesystem permissions. `env -i` and the changed `HOME` remove credentials from the environment but do not make other user files inaccessible.
- crates.io integrity: TLS + index checksum only; no per-crate signature or attestation verified.
- Isolated `CARGO_HOME`/`CARGO_TARGET_DIR` separate caches and outputs; they are not sandboxing.

## Not done
P00-T05 spike implementation or execution; running the binary; any test; other candidate crates (T06–T08); push or merge.

## Rollback
Delete `~/.local/share/pwt-dev-tools/_cargo-home/` and `_cargo-target/`; revert/delete `spikes/rust/` in the repository.

# P01-T02 — offline dependency resolution and build-exposure report

Date: 2026-10-26 (UTC 05:07 onward). Executed by Opus (lead-owned task: lockfile and manifests are lead-owned, so no worker was spawned).

**Authority:**
- A2 = 2a and A3 = 3a, approved 2026-10-26;
- `planning/P01-T01-request.md` (approved with D1–D6, 2026-10-26), §2 and §4.

**Repository:** `phase/P01-foundation` at `2667a31`. Dirty with the uncommitted T01 deliverables (`CLAUDE.md`, `plan.md`, `status.md`, `planning/P01-T01-request.md`) plus this directory (`env.txt`).

**Result: PASS. The A2/A3 conditions hold, so there is no stop condition.** One planning-fact error was found and corrected (§5). Nothing was fetched, downloaded, compiled or executed apart from `cargo` itself (`generate-lockfile`, `tree`). Build scripts ran **not at all**: neither command builds anything.

## 1. Environment
| Item | Value |
|---|---|
| Toolchain | rustc / cargo / rustdoc 1.97.1, called directly from `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin` (version gate per T01 §2) |
| Environment | `env -i` with `PATH=<tc>/bin:/usr/bin:/bin`, `HOME`=`CARGO_HOME`=`~/.local/share/pwt-dev-tools/_cargo-home`, `CARGO_TARGET_DIR=…/_cargo-target/p01-lead`, `RUSTC`, `RUSTDOC`, `CARGO_TERM_COLOR=never`, `LANG=C.UTF-8`. No other variables |
| Network | Every cargo command ran inside `unshare -rn`. Probe (`netns-probe.txt`): TCP connect to 1.1.1.1:443 fails with errno 101 (ENETUNREACH); only `lo`, which is DOWN |
| Scratch workspace | `~/.local/share/pwt-dev-tools/_p01/t02-resolve/` (0700, outside the repo): the T01 §1/§4 manifests (7 members; `domain` depends on `serde`, with `serde_json` as a dev-dependency), with one-line stub `lib.rs` files. Copies in `manifests/` |
| Helper | `t02_cargo.sh` (bash arrays, exit captured without pipes) |

## 2. Commands and exits
| Step | Command | Exit | Evidence |
|---|---|---|---|
| Resolve | `cargo generate-lockfile --offline` | 0 ("Locking 11 packages") | `generate-lockfile.log`, `.result` |
| Cache unchanged | SHA-256 of the registry `.crate` listing plus index file count, before and after | equal (`655996a4…`) | `generate-lockfile.result` |
| P00 lock unchanged | `sha256sum -c` | OK | `p00-lock.sha256` |
| Feature tree | `cargo tree --offline --locked -p domain -e features,normal,build,dev --target all` | 0 | `tree-features.txt` |
| Workspace tree | `cargo tree --offline --locked --workspace -e normal,build,dev --target all` | 0 | `tree-workspace.txt` |
| Flat features | `cargo tree … --prefix none --format '{p} [{f}]'` | 0 | `p01-host-features.txt` |
| Lock check v1 | `python3 -I -B compare_lock.py` | 0 (16/0/0) | `compare-lock.txt` |
| Lock check v2 | `compare_lock_v2.py` (exact versions) | 0 (17/0/0) | `compare-lock-v2.txt` |
| v2 negative control | v2 against a copy with `syn` changed to 2.0.119 | 1 (as expected; 2 FAIL lines) | `compare-lock-v2-negative-control.txt` |
| Exposure | `python3 -I -B exposure.py` | 0 (11/0/0) | `exposure.txt` |
| Build-script review | pattern scan plus context lines, read from the `.crate` archives | 0 | `build-script-review.txt`, `build-script-context.txt` |

**Resolved lock:** `Cargo.lock.resolved`, SHA-256 `fb8b7d076ccf7f79c94128c24d84e88bee763dc7fabfbf95befb5af361a71258`. **T04 must reproduce this file byte for byte.** Lint tables in the manifests do not affect the lock.

## 3. Resolved set (A2 condition)
- **11 registry crates, each byte-equal to its `spikes/rust/Cargo.lock` entry** (name, version, source, checksum):
  - itoa 1.0.18, memchr 2.8.3, proc-macro2 1.0.107, quote 1.0.47;
  - serde, serde_core, serde_derive 1.0.229;
  - serde_json 1.0.154;
  - **syn 3.0.9**;
  - unicode-ident 1.0.27, zmij 1.0.23.
- Plus the 7 workspace members.
- All registry sources are crates.io, all entries carry checksums, and each crate appears in one version only.
- Each `.crate` archive's SHA-256 equals its lock checksum (`exposure.txt`).
- **Shape:** `serde_json` is reached only through `[dev-dependencies]`. The normal graph of `domain` is serde → serde_core / serde_derive (proc-macro) → proc-macro2 / quote / syn 3.0.9 / unicode-ident. The edge serde_core → serde_derive shown under `--target all` is serde's version-lock dependency on an empty target cfg; cargo does not build it for this target.

## 4. Build-code exposure (A3 condition)
| Crate | Build script | Proc-macro | `links` | P01 features | Built in accepted P00 graph; features ⊆ P00 |
|---|---|---|---|---|---|
| proc-macro2 1.0.107 | yes | – | none | proc-macro | yes |
| quote 1.0.47 | yes | – | none | proc-macro | yes |
| serde 1.0.229 | yes | – | none | derive, serde_derive, std | yes (T06: also alloc, default) |
| serde_core 1.0.229 | yes | – | none | result, std | yes |
| serde_derive 1.0.229 | – | **yes** | none | default | yes |
| serde_json 1.0.154 | yes | – | none | default, std | yes |
| zmij 1.0.23 | yes | – | none | – | yes |
| syn 3.0.9, itoa, memchr, unicode-ident | – | – | none | see `exposure.txt` | yes |

- **Exposure:** 6 build scripts and 1 proc-macro. The reference graphs are the T06 host graph and the T08 host graph, both built in accepted P00 stages (`evidence/P00/P00-T08/B-t06-host-features-now.txt`, `B-t08-host-features.txt`).
- **Not new:** every item and every P01 feature set is contained in them. The P01 feature sets are equal to or smaller than P00's, so no new code paths are enabled.
- **Limited build-script review** (patterns plus context, `build-script-context.txt`):
  - every `https://` hit is in a comment;
  - every process spawn is the `rustc` compiler, either `--version` or proc-macro2's probe compile into an `OUT_DIR` subdirectory that it creates and then removes;
  - file writes go only under `OUT_DIR` (serde and serde_core write `private.rs`);
  - environment reads are compiler and cargo variables only (`RUSTC*`, `TARGET`, `OUT_DIR`, `CARGO_*`, `OPT_LEVEL`);
  - serde_json's script only emits cfgs.
  - This is consistent with the P00-T08 stage-B exposure finding ("rustc probes / OUT_DIR only").
  - It is not a full audit, and nothing was executed here.

## 5. Anomaly found and handled
- **What happened:** `planning/P01-execution-plan.md` §2 lists `syn 2.0.119` as part of the serde chain. That fact was wrong. The P00 lock contains both syn 2.0.119 and syn 3.0.9, and `serde_derive 1.0.229` depends on **`syn 3.0.9`** in the P00 lock (`dependencies = [..., "syn 3.0.9"]`), the same edge as in the accepted T06/T08 graphs.
- **Why the check missed it:** lock check v1 compared crate names, not versions, so the mismatch was not caught. The lead found it by reading the output.
- **Classification:** a confirmed planning-document error, not a resolution defect. The resolved crate equals a P00 entry, as A2 requires.
- **Handling (lead-owned, not a worker defect):**
  - v1 and its output are preserved unchanged;
  - `compare_lock_v2.py` adds an exact-version assertion (17/0/0), and a negative control shows it now fails on syn 2.0.119;
  - the plan §2 row is annotated with the correction;
  - T01 §4 lists crate names only, so it needs no correction.
- No debug cycle was opened: no worker task was involved and no repair to code was needed.

## 6. Limitations (recorded, not repaired)
- No fresh RustSec screen; the P00 screen dates from 2026-10-24 (A2 = 2a). cargo-audit was not run (A4 = 4a).
- No filesystem sandbox for build code when T04 builds. The build-script review is pattern-based and limited.
- Feature-subset reasoning assumes cargo feature unification within the P01 workspace only. The P01 workspace is separate from `spikes/rust`, so P00 spike features do not unify into it.
- The scratch workspace remains under `~/.local/share/pwt-dev-tools/_p01/` until T04 has reproduced the lock. Removing it is a lead clean-up step after T04.

## 7. Next
T04 (B0 scaffold): create `workbench/` with these manifests plus the T01 lint tables. Then `cargo generate-lockfile --offline` must reproduce `fb8b7d07…` exactly, and `p01_check.py` gets its first verified run. T03 (the cargo-audit request, documents only) is independent. **Stopped here for owner approval.**

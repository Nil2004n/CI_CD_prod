# P04-T02 — dependency and lockfile check

Owner authorized T02. Base revision `140f61c013531d82afe4da46c3d94e9cd3c676f4`, branch `phase/P04-git-acquisition`. Scratch workspace: `/tmp/p04-t02-V8iJku/workbench`, copied from `workbench` excluding `target`; no product manifests or lockfile changed. Host Cargo 1.99.1 and Rust 1.99.1; normal local Cargo configuration/cache, offline mode. Scratch data is temporary.

Scratch manifests added the `repositories → domain` and `application → repositories` path dependencies and `fixture-transport` as a non-default feature enabled by a repositories dev self-dependency. The exact tested manifest delta is preserved in `scratch-manifests.diff`. This checks manifest resolution only, not feature isolation in production builds.

| Command (scratch cwd) | Exit | Result |
|---|---:|---|
| `cargo metadata --offline --format-version 1 --no-deps` | 0 | Manifest metadata parsed; `--no-deps` does not resolve the complete dependency graph. |
| `cargo generate-lockfile --offline` | 0 | Resolved 86 packages, but regenerated unrelated `js-sys` and `wasm-bindgen` versions; this does not meet the edge-only requirement and was discarded. |
| Restore original lockfile; `cargo metadata --offline --format-version 1` | 101 | Cargo updated the scratch lockfile but could not download missing cached `bitflags v2.13.2` in offline mode. Full metadata and complete offline dependency availability are not verified. |
| `diff -u` original versus scratch lockfile | 1 | Differences are only the three local dependency edges in `scratch-lock.diff`; registry package entries and versions were unchanged in that file. A diff exit of 1 means differences found. |
| `cargo tree --offline --locked --workspace` | 0 | Locked dependency tree resolves for the host target; this does not establish availability for every target. |
| `cargo tree --offline --locked --workspace --target all` | 101 | Full target tree requires the missing `bitflags v2.13.2` archive. |

The initial result was partial: full offline metadata failed due to missing cached packages. The regenerated lockfile with unrelated upgrades was discarded.

Following the owner's request for a branch-session worker, a read-only worker was assigned `task/P04-T02-dependencies` at base `140f61c` in `/tmp/p04-t02-worker`, configured with requested model `gpt-6.1-terra`; its resolved identity is not independently observable. The primary session independently ran the tree checks above. The worker also reported 14 absent locked archives in the default cache; that count is not independently verified and is not used as acceptance evidence. The worker made no product changes. Its worktree is retained for inspection.

## Cache restoration and final checks

After the owner requested a solution and approved the network/cache operation, the primary ran `cargo fetch --locked --manifest-path /tmp/p04-t02-V8iJku/workbench/Cargo.toml` from the repository root, exit 0. Cargo downloaded 14 locked package archives into the default local Cargo cache; no product files were changed. Then, in the scratch workspace with Cargo/Rust 1.99.1:

| Command | Exit | Result |
|---|---:|---|
| `cargo metadata --offline --locked --format-version 1 > /tmp/p04-t02-V8iJku/metadata-complete.json` | 0 | Full metadata resolved offline against the scratch lockfile. |
| `cargo tree --offline --locked --workspace --target all > /tmp/p04-t02-V8iJku/tree-all.txt` | 0 | All-target dependency tree resolved offline. |
| TOML comparison of original and scratch registry entries | 0 | 86 registry identities, versions, sources and checksums unchanged. |
| `diff -u` original versus scratch lockfile | 1 | Only the local edges shown in `scratch-lock.diff`; exit 1 means differences found. |

T02 dependency and lockfile check passed in scratch. The product manifests and lockfile are unchanged; applying changes and establishing B0 belong to subsequent authorized work. The self-dependency feature isolation in production remains to be proved in T04/T11; no product build, test or security acceptance is claimed here. Temporary metadata and tree output are not durable evidence. The owner subsequently authorized committing and pushing the takeover documentation to the phase branch; T03 requires separate authorization.

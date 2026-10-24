# P00-T07 — keyring resolution and advisory screen (stopped before fetch)

Date: 2026-10-24. Authority: owner approval of T07 stages A, A2, B (resolution, graph/feature review, RustSec screen, fetch + checksums, build-exposure inspection). Executed by Opus on `phase/P00-decisions-spikes` at base `f076c6c` (clean before T07). Environment for every Cargo command: toolchain binaries invoked directly (rustc/cargo 1.97.1), `env -i`, isolated `CARGO_HOME`/`CARGO_TARGET_DIR` under `~/.local/share/pwt-dev-tools/` (same as `evidence/P00/P00-T05-D3-1/record.md`). No `.cargo/config*` in any ancestor of `spikes/rust/` (checked up to `/`).

**Status (A/A2 section below, unchanged): STOPPED before stage B (fetch)** under the owner's rule "stop before fetching if the graph introduces … an applicable unresolved advisory". Nothing was fetched, built or executed; no D-Bus daemon or keyring operation; no credentials.

## A — resolution
- New workspace member `t07-keyring`: `keyring = "=4.2.0"` with default features (`v1`); `src/main.rs` is an empty placeholder. Workspace `members` extended to `["t05-process", "t06-yaml", "t07-keyring"]`.
- Lock-preserving resolution: `cargo update --workspace --dry-run` (exit 0, `A-resolve-dryrun.log`), then `cargo update --workspace` (exit 0, `A-resolve.log`): "Adding" 94 packages, no "Updating"/"Downgrading"/"Removing". `cargo generate-lockfile` (used in T06) was deliberately **not** used because it re-resolves every package and could silently change T05/T06 versions.
- Lock: 85 → 179 packages; sources crates.io + local only (`A-lock-diff.txt`; before/after copies and SHA-256 kept). **No existing package version changed or was removed.** Six existing entries changed only in their dependency lists:
  - `ahash`, `getrandom 0.3.4`, `jsonschema`, `jsonschema-value`, `referencing`: the dependency reference became version-qualified (`getrandom` → `getrandom 0.3.4`, `r-efi` → `r-efi 5.3.0`) because a second version now exists. Checksums are unchanged.
  - `js-sys 0.3.106` (wasm32-only, not in the host graph) gains `futures-util`, because that crate is now present in the lock and activates `js-sys`'s optional dependency. This has no effect on the T06 host graph. It cannot be confirmed with `cargo tree` until the sources are fetched.
- New second versions: `getrandom 0.4.3` (by `secret-service`), `r-efi 6.0.0` (UEFI-only). `sha2 0.11.1` and `tempfile 3.27.0` match the P00-T03 provisional selections.
- `cargo tree --offline` for the host graph needs the crate sources (exit 101, "failed to download `aes v0.9.3`"; the one-line output file `A-t07-host-tree.txt` was deleted during stage B and is superseded by `B-t07-host-tree.txt`); deferred to after B. A first attempt exited 127 because zsh did not word-split a command stored in a variable (same issue as T06); its output was discarded and a wrapper script used.

### Feature review (cached sparse-index metadata, `A-index-features.txt`; no crate sources)
- `keyring 4.2.0`: `default = ["v1"]`; `v1` = apple keychain (macOS/iOS-gated), windows store (Windows-gated), `zbus-secret-service-keyring-store` with `crypto-rust` (unix, non-Apple, non-Android). `dbus-secret-service-keyring-store` (`crypto-rust`, **`vendored`**, libdbus) is only reachable through the `cli` feature, which is not enabled.
- `zbus-secret-service-keyring-store 1.0.1` → `secret-service 5.2.0` with `crypto-rust` = `aes`, `cbc`, `sha2`, `hkdf` (pure Rust); `crypto-openssl` (`dep:openssl`) **not** enabled.
- `zbus 5.19.0`: default features `async-io` + `blocking-api` (via the keyring store; `secret-service` itself asks for `default-features = false`, `blocking-api`). Tokio is not enabled. `async-io` pulls `async-process`; whether zbus can spawn a process (e.g. autolaunch) on Linux is a source question for after B.
- Name screen of the whole lock (a superset of the host graph): no `openssl`, `native-tls`, `rustls`, `ring`, `aws-lc*`, `cc`, `pkg-config`, `cmake`, `libdbus`/`dbus`, `*-src`, `hyper`, `reqwest`, `url`, `idna`. `-sys` crates: `core-foundation-sys`, `security-framework-sys` (Apple-gated), `windows-sys` (Windows-gated), `js-sys` (wasm, pre-existing), `linux-raw-sys` (pure-Rust syscall bindings, no C). `wasm-bindgen*`/`wit-bindgen` name hits are pre-existing wasm-only crates.
- The native/C, vendored and TLS stop conditions are not triggered for the Linux host path, as far as the index metadata shows. This is to be confirmed by `cargo tree` after B.

## A2 — RustSec screen (`A2-advisories.txt`, raw files in `A2-advisories/`)
Advisory-db `crates/` listing via the GitHub contents API (HTTP 200, 967 entries, below the 1000 cap; names and raw-response SHA-256 in `A2-crates-listing-names.txt`). 93 new registry crate/versions were checked, and 15 of them have advisory directories. Each advisory's TOML `[versions]` was fetched from the official repository (all HTTP 200), then evaluated against the locked version and checked by hand.

| Crate (locked) | Advisory | Kind | Result |
|---|---|---|---|
| cmov 0.5.4 | RUSTSEC-2026-0003 | vulnerability | patched (≥0.4.4) |
| crossbeam-utils 0.8.23 | RUSTSEC-2022-0041 | unsound | patched (≥0.8.7) |
| ctutils 0.4.5 | RUSTSEC-2026-0385 | vulnerability | patched (≥0.4.4) |
| enumflags2 0.7.12 | RUSTSEC-2023-0035 | unsound | patched (≥0.7.7) |
| event-listener 5.4.2 | RUSTSEC-2026-0221 | unsound | patched — **exactly at the bound** `>= 5.4.2` (hand-checked) |
| futures-task 0.3.34 | RUSTSEC-2020-0060, -0061 | vulnerability | patched |
| futures-util 0.3.34 | RUSTSEC-2020-0059, -0062 | vulnerability | patched |
| memoffset 0.9.1 | RUSTSEC-2019-0011, RUSTSEC-2023-0045 | unsound | patched |
| security-framework 3.7.0 | RUSTSEC-2017-0003 | vulnerability (Apple-only crate) | patched |
| sha2 0.11.1 | RUSTSEC-2021-0100 | vulnerability | not affected (only 0.9.7) |
| slab 0.4.12 | RUSTSEC-2025-0047 | vulnerability | patched (≥0.4.11) |
| tracing 0.1.44 | RUSTSEC-2023-0078 | unsound | patched (≥0.1.40) |
| **zbus_names 4.3.5** | **RUSTSEC-2026-0371** (2026-10-16) | **unmaintained** | **applicable, no patched version** |
| **zvariant 5.15.1** | **RUSTSEC-2026-0370** (2026-10-16) | **unmaintained** | **applicable, no patched version** |
| **zvariant_derive 5.15.1** | **RUSTSEC-2026-0372** (2026-10-16) | **unmaintained** | **applicable, no patched version** |

The three `unmaintained` notices say that the crates were merged into `zbus` 6.0.0 (as the `names`/`wire` modules and derive macros). They are informational, not vulnerability reports. They are nonetheless applicable and unresolved for this graph.

**Why it cannot be resolved within the approved request.** In the sparse index cached at resolution time (2026-10-24 11:07 +0530), `zbus` 6.0.0 and 6.1.0 exist. However, every release of `secret-service` (latest 5.2.0) and `zbus-secret-service-keyring-store` (latest 1.0.1) requires `zbus ^5`, and `keyring` 4.2.0 is the latest `keyring`. No version of the approved backend path uses `zbus` 6.

Scope: published advisories only; absence of an advisory is not evidence of code safety; yanked status not separately queried; transitive code not reviewed.

## Not done (blocked by the stop)
B (fetch, checksum verification, `Cargo.lock` stability across fetch), host-graph `cargo tree`, feature-unification confirmation, build-script/proc-macro exposure inspection, zbus bus-address/autolaunch source review.

## Working-tree state (as of the A/A2 stop; see the checkpoint commit for the current state)
Uncommitted at that time: `spikes/rust/Cargo.toml` (member added), `spikes/rust/Cargo.lock` (94 added), new `spikes/rust/t07-keyring/`, this evidence directory, `spikes/P00-T07-request-and-isolation.md`. Revert path if the owner declines: restore `Cargo.toml` member list and `Cargo.lock` from `Cargo.lock.before` (SHA-256 `03671ce1…` = committed `2931774` version), and remove `t07-keyring/`.

## Owner decision K-A and stage B (2026-10-24) — appended; A/A2 above unchanged

Owner chose **K-A**: a spike-only exception for RUSTSEC-2026-0370/-0371/-0372, bound to the exact versions, checksums and paths in this lockfile. Full scope: `advisory-exception.md`. Stage B approved: locked fetch of the reviewed graph, checksums, inspection. No re-resolution, no build, no execution.

### Fetch and checksums
- Before: `sha256sum -c lock-after-A.sha256` → OK. `cargo fetch --locked --target x86_64-unknown-linux-gnu` → exit 0, 80 downloads (`B-fetch.log`).
- `Cargo.lock` after B = after A (`eeca61f5…`, `lock-after-B.sha256`). **Lockfile stable.**
- Independent SHA-256 of every cached `.crate` vs `Cargo.lock` (`B-checksums.txt`): 176 registry entries; **149 verified, 0 mismatches**; 27 not fetched. None of the 27 is in the host graph: Apple/Windows/wasm/UEFI/hermit/redox crates, plus `tempfile`, `zeroize`, `memoffset`, `byteorder`, `rustversion`, `bumpalo`, `zerocopy-derive` (optional or dev-only for this host).

### Host graph and features (`B-t07-host-features.txt`, `B-t07-host-packages.txt`, `B-t07-host-tree.txt`)
- `t07-keyring` host graph: 102 packages (unique), 80 not in the T06 host graph.
- Active features: `keyring [default, v1, …]`; `zbus-secret-service-keyring-store [crypto-rust]`; `secret-service [crypto-rust]` (aes/cbc/hkdf/sha2); `zbus [default = async-io + blocking-api, async-process, …]`; no `tokio`. No `openssl`, `native-tls`, `rustls`, `ring`, `aws-lc`, `reqwest`, `hyper`, `cc`, `pkg-config`, `libdbus` or `dbus-secret-service-keyring-store` in the graph.
- Native link exposure: **no package declares `links`**.

### T06 feature-unification check (`B-t06-feature-delta.txt`)
- `cargo tree -p t06-yaml` now = the accepted `evidence/P00/P00-T06/B-tree-features.txt` after removing cargo's `(*)` de-duplication markers. Same 70 lines, same versions, same features. **T06 built alone (`-p t06-yaml`, as every recorded T06 command was) is unchanged.**
- 20 crates are shared between the T06 and T07 host graphs. Only if both are built in **one** Cargo invocation (`-p t06-yaml -p t07-keyring` or `--workspace`; resolver 2 unifies features across the selected packages) do 4 T06 crates gain features:
  - `num 0.4.3 +default`;
  - `once_cell 1.21.4 +default,std`;
  - `syn 2.0.119 +extra-traits,full,visit-mut` and `syn 3.0.9 +extra-traits,fold,full,visit-mut` (compile-time only).
- `jsonschema []`, `referencing [default=[]]`, `serde_json [default,float_roundtrip,std]` are unchanged in the unified graph. **No `resolve-http`, `resolve-file`, TLS, HTTP client or `arbitrary_precision` becomes active.**
- T06 assumptions remain valid for the recorded `-p t06-yaml` builds.
- Targeted retest would be needed only if T06 is ever built together with T07 (or in a future unified product workspace). That retest is the D01 Y/J suite plus J16, rebuilt with the unified features, focusing on `num` (fraction/bigint arithmetic in jsonschema numeric checks) and `once_cell`. The `syn` changes affect proc-macro compilation only. Rule carried forward: build spikes per package (`-p`), never `--workspace`.

### Build-time execution exposure (`B-build-exposure.txt`, `B-pattern-review.txt`; grep-based, not an audit)
- **10 build scripts** in the T07 host graph, 6 already accepted in T06 (`libc`, `num-traits`, `proc-macro2`, `quote`, `serde`, `serde_core`). The 4 new ones are:
  - `async-io 2.6.0`: a rustc version probe only;
  - `crossbeam-utils 0.8.23`: reads `TARGET`/`CARGO_CFG_SANITIZE`, emits cfgs;
  - `getrandom 0.4.3`: reads `CARGO_CFG_SANITIZE`, emits a cfg;
  - `rustix 1.1.5`: runs `$RUSTC` (or `RUSTC_WRAPPER`) compile probes, writes and removes a probe file in `OUT_DIR`, and checks one path via `fs::metadata`.
- **9 proc-macro crates**, 1 already accepted (`serde_derive`). The 8 new ones total 9,472 lines: `async-recursion` 416, `async-trait` 1,213, `enumflags2_derive` 355, `futures-macro` 698, `serde_repr` 272, `tracing-attributes` 2,072, `zbus_macros` 3,745, `zvariant_derive` 701. A pattern grep of their `src/` found no process spawning, file I/O, env access or networking.
- **Additional compile-time exposure:** `proc-macro-crate 3.5.0`, used inside `zbus_macros`/`zvariant_derive` during compilation, reads `CARGO_MANIFEST_DIR/Cargo.toml` and, if `CARGO` is set (Cargo sets it), **spawns `$CARGO locate-project --workspace`** to read the workspace manifest. This is a local Cargo invocation inside the build. It runs inside the build's network namespace; there is no filesystem sandbox.
- No network, home-directory or `set_var` patterns in the new build code. Absence of a pattern is not proof of safety.

### Backend source findings (observed in code; nothing executed)
| Question | Observation (file) | Status |
|---|---|---|
| Session bus address | `zbus-5.19.0/src/address/mod.rs` `Address::session()`: `DBUS_SESSION_BUS_ADDRESS` if set (parsed as given; unparsable → error, not fallback); else `unix:path=$XDG_RUNTIME_DIR/bus`; if `XDG_RUNTIME_DIR` unset → `/run/user/<geteuid()>/bus` | observed |
| Runtime-dir default | `/run/user/<euid>` — inside `unshare -r` the euid is **0**, so the fallback is `/run/user/0/bus`, not `/run/user/1000/bus` | observed (implication for isolation) |
| Autolaunch | `autolaunch:` connect is implemented only under `cfg(windows)`; `launchd:` only macOS; no `dbus-launch` call on Linux | observed |
| Process spawning from an address | **`unixexec:` transport spawns the program named in the address** (`address/transport/unixexec.rs`); `ibus:` transport runs `ibus address` (`address/transport/ibus.rs`). So a `DBUS_SESSION_BUS_ADDRESS` value can make the client execute a program | observed — product finding (P03: the app must validate or construct the address; only `unix:path=`/`unix:abstract=` accepted) |
| System bus | `DBUS_SYSTEM_BUS_ADDRESS` else `/var/run/dbus/system_bus_socket`; not used by the secret-service path | observed |
| Secret Service activation | `secret-service` connects with `zbus::blocking::Connection::session()` and calls the service without `NoAutoStart` (no occurrence in `secret-service`, the keyring store or `keyring`), so the bus daemon may **activate** `org.freedesktop.secrets` from its service directories. Host `/usr/share/dbus-1/services/org.freedesktop.secrets.service` exists | observed in client code; daemon activation behaviour is a runtime question |
| Store initialisation | `keyring-4.2.0/src/v1.rs`: the store is created once per process in a `LazyLock` on first `Entry::new`/`store_status`, and **the result (including failure) is cached for the process lifetime**. `zbus-secret-service-keyring-store` `Store::new()` connects immediately (`SecretService::connect(EncryptionType::Dh)`); `new_with_configuration` rejects any configuration ("Secret Service configuration is not supported") | observed — each D case must run in its own process |
| Timeouts | zbus supports `Builder::method_timeout`, default **None** (`connection/builder.rs`), so calls wait indefinitely. Neither `secret-service`, the keyring store nor `keyring` sets it, and `keyring` v1 offers no way to pass a connection or timeout. `secret-service` has `connect_with_existing(conn)` (a different API path; using it would need a new direct dependency on `secret-service`/`zbus` — not approved) | observed |
| Prompts | Locked items trigger `ensure_unlocked` → `unlock_all` → `exec_prompt_blocking`, which calls `Prompt.Prompt("")` and then **blocks on the `Completed` signal with no timeout**. It returns `PromptDisconnected` only if the connection closes, and `Prompt` if dismissed | observed — K04 is expected to hang unless the service refuses; runtime question |
| gnome-keyring 50.0 flags | man page documents `--foreground`, `--components=`, `--control-directory=`, `--unlock` (password from stdin; unlocks or **creates** the login keyring) | documented only; not run |

### Remaining runtime unknowns
- Whether gnome-keyring 50.0 honours `XDG_DATA_HOME` for keyring storage and `--control-directory` under the spike environment.
- What a locked or absent default collection does without a prompter: a typed error, or an indefinite prompt wait.
- Whether `dbus-daemon` 1.16.2 with a custom config and no standard service dirs refuses activation as expected.
- Whether unprivileged tmpfs mounts over `/run/user`, `/run/dbus`, `~/.local/share/keyrings`, `/tmp/.X11-unix` and `/usr/share/dbus-1/*` succeed in the user namespace.
- The exact typed errors returned by `keyring-core` for each case.

### Limitations
Grep-based review only; transitive code not audited; no filesystem sandbox for build scripts or proc-macros; results are for the exact locked graph.

## Stage C and D (owner-approved 2026-10-24) — appended; A/A2/B above unchanged

Authority: owner approval of the d5f5687 push, stage C (harness + package-scoped frozen offline build, with acceptance of the reported build-time exposure for this locked spike build only), stage D env check, and cases K01a–K06. Executed by Opus. The `d5f5687` push was a normal fast-forward from `f076c6c`; the remote was verified as `d5f5687` and `main` was unchanged.

### C — implementation and build (`C/`)
- Harness `spikes/rust/t07-keyring/src/main.rs` (std + `keyring` only); run scripts `spikes/rust/t07-keyring/run/{outer.sh,setup-and-run.sh}`. Manifests and `Cargo.lock` unchanged: SHA-256 before = after both builds (`C/hashes-before.txt`, `C/hashes-after.txt`). No new dependencies, features, refetch or installation. No `.cargo/config*` in any ancestor.
- Build commands: `unshare -rn <isolated cargo> build --frozen --offline -p t07-keyring`.
  - Build 1 (`C-build-1.log`): exit 0, 0 warnings, 99 packages compiled, all in the reviewed host graph (`C/compiled.txt`). Artifact `4ee636ee…5def0`.
  - Build 2 (`C-build-2.log`, after adding the K04b sub-case): exit 0, 0 warnings, only `t07-keyring` recompiled. Artifact `3b8665cf…6e73b40f`.
- **Build-time code has no filesystem sandbox.** The 10 build scripts, 9 proc-macros and `proc-macro-crate`'s `cargo locate-project` ran with the user's file permissions inside a network namespace only. Pattern review and a stripped environment do not prove that they did nothing else.

### D — isolation and runs (`D/`)
- **Layers:** `outer.sh` (outside: `mktemp` root, metadata-only before/after snapshot, 240 s `timeout -s KILL`, sanitised copy, removal of the exact root only) → `unshare -r -n -m --pid --fork --mount-proc --kill-child env -i PATH LANG` → `setup-and-run.sh` (PID 1; artifact SHA-256 check; tmpfs masks; `exec env -i` of the harness as PID 1).
- **Preflight** (`D/preflight/`, artifact 1; `D/preflight-2/`, artifact 2): **20/20 checks passed** before any daemon or library call:
  - PID 1; euid 0 in the namespace;
  - masks are tmpfs, private propagation and empty for `/run/user`, `/run/dbus`, `~/.local/share/keyrings`, `/tmp/.X11-unix`, `/usr/share/dbus-1/services` and `/etc/dbus-1/session.d` (`/usr/share/dbus-1/session.d` does not exist; `/var/run` → `/run`);
  - root mount private;
  - `/run/user/0/bus`, `/run/user/1000/bus` and `/run/dbus/system_bus_socket` absent;
  - network namespace has loopback only;
  - inherited environment exactly `{LANG, PATH}`;
  - spike root 0700 with the expected prefix;
  - daemon and tool binaries present.
- **Per bus:** the harness-written `bus.conf` was re-verified (1 servicedir, empty; no standard servicedirs, `<include>`, systemd or `<user>`). The daemon's printed address equals the private socket, and `ListActivatableNames` = `["org.freedesktop.DBus"]` only. The last check is done with `dbus-send` against the **private** bus only; this tool was added for the check and is disclosed.
- **Per case process:** environment is exactly the §4.3 set; `DBUS_SESSION_BUS_ADDRESS` equals exactly the expected `unix:path=` socket, or is absent for K01a/K01b; host fallback paths absent. All passed.
- **Binding limitation preserved:** binding is shown by construction and these assertions, not by a library-level bus-ID check (the `keyring` API exposes no connection).
- **Real paths:** `stat` of `/run/user/1000/bus` plus a hash of names/mtime/size of `~/.local/share/keyrings`, and the PID lists of host `dbus-daemon`/`gnome-keyring-daemon`, were identical before and after every run (`outer-*.txt`; `real_path_snapshot=unchanged`). Nothing real was opened or connected to.
- **Runs:**
  - `D/run1/` (artifact 1): K01a–K04, K06, K05. It did **not** include the K04b sub-case planned in request §4.4, which I had omitted from the first implementation.
  - `D/run2/` (artifact 2): adds K04b; this is the formal result.
  - run1 is preserved unchanged. Outcomes for the cases common to both runs are identical.
  - Each run took 13 s; the spike root was removed after each.

### Case outcomes (run2)
| Case | Setup | R1 library call | R2 prompt | R3 app deadline (10 s) | R4 worker | R5 forced kill | Verdict |
|---|---|---|---|---|---|---|---|
| K01a | no address; `XDG_RUNTIME_DIR=<case>/run` | returned in 0 ms: `PlatformFailure` ("Failed to connect to `unix:path=<case>/run/bus`: No such file") | no | no | finished | no | typed, bounded failure ✔ |
| K01b | no address; `XDG_RUNTIME_DIR` unset | returned in 0 ms: `PlatformFailure` (connect to `unix:path=/run/user/0/bus`: No such file) | no | no | finished | no | typed ✔; confirms the euid-0 fallback (masked) |
| K02 | private bus, empty servicedir | returned in 20 ms: `PlatformFailure` ("ServiceUnknown: org.freedesktop.secrets was not provided by any .service files") | no | no | finished | no | typed ✔; **the bus attempted activation and the private config refused it** |
| K03 | private bus + `gnome-keyring-daemon --foreground --components=secrets --control-directory=<case>/keyring-ctl --unlock` (password via stdin) | all 8 ops returned, all ok (set, get = C, replace, get = C2, delete, get → `NoEntry`); each ≤ 41 ms | no | no | finished | no | round trip ✔ on a unique synthetic entry; control socket in the private dir; daemon owned `org.freedesktop.secrets` on the private bus |
| K04 | as K03 without `--unlock` (no login/default collection) | `store_status`/`new` ok; `set_password` returned in 1 ms: `NoStorageAccess` ("result not returned from SS API": `ReadAlias("default")` = `/`, before any prompt); get/delete → `NoEntry` | no (prompt path not reached; per source) | no | finished | no | typed ✔ for "no default collection" |
| K04b | unlocked, then default collection `/org/freedesktop/secrets/collection/login` locked via `Service.Lock` on the private bus (`Locked` = true) | `set_password` returned in 6 ms: `NoStorageAccess` ("**prompt dismissed**"); get/delete → `NoEntry` | **yes**: the daemon logged "couldn't create system prompt: … ServiceUnknown: org.gnome.keyring.SystemPrompter" | no | finished | no | typed ✔ **only because no prompter can be activated**, so gnome-keyring dismisses the prompt immediately |
| K06 | harness listener accepts and never replies | **did not return**: the connect inside `store_status` blocked | n/a | **yes**, at op 0 | **still blocked at exit → leaked worker, not cancelled** | no orchestrator kill; the case process ended itself at 11.0 s via `exit`, which ended the blocked thread | **containment only; no library-level timeout** |
| K05 | after all cases | — | — | — | — | — | ✔ C, C2 and U (hex) absent from all 24 app-owned/captured files. Private service storage (4 files under `cases/*/data/keyrings/`) is reported separately: no plaintext occurrence (encrypted by gnome-keyring). Only spike-owned paths were scanned. Results guard (synthetic value in results) not triggered |

- After every case and at the end there were no processes left in the namespace, and all daemons stopped with SIGTERM.
- The host process lists were unchanged.
- Committed evidence contains only `results.json`, per-case harness stdout/stderr and daemon logs (all scanned by K05), the preflight results and the outer summaries. No keyring databases, runtime directories or secrets.

### Suitability conclusion (spike evidence; not a product decision)
- **Direct in-process use of `keyring` v1 is not suitable as-is.** An unresponsive bus (K06) blocks the store initialisation indefinitely. There is no library timeout, and the blocked thread cannot be cancelled; only process exit ends it. Store initialisation is also cached per process, so a failure at first use stays a failure for the life of that process.
- **Workable shape (for P03 to decide):** perform OS-store operations in a short-lived, application-owned helper process with a hard deadline and kill. The helper gets a validated `unix:path=`-only session address (because of the `unixexec:`/`ibus:` finding), and failure maps to "OS store unavailable". The fallback is session-only, or disabled AI.
- The alternative is an app-owned zbus connection with `method_timeout` via `secret-service::connect_with_existing`. That would need new direct dependencies and approval, and it still would not bound prompt waits.
- Absent bus, absent service, no default collection, a locked collection without a prompter, and the normal round trip all yield typed errors or success within milliseconds on this host.
- **Untested:** a locked collection **with** a real desktop prompter, where the source shows an unbounded wait for the user; PAM-unlocked login sessions; KWallet/KeePassXC; other gnome-keyring versions.
- Persistent OS storage remains **not accepted** for pilot or product. K-D is not chosen.

### Remaining risks
- The advisory exception (unmaintained zvariant/zbus_names/zvariant_derive) still applies to this path.
- Binding is shown by construction plus assertions; there is no independent peer-credential or bus-ID check.
- No filesystem sandbox for build-time code or for the daemons beyond the masks.
- `dbus-send` was used for private-bus verification and locking (an existing host tool, not in the original command list).
- gnome-keyring ran as namespace-root (uid 0 mapped to the user), which a real session does not.
- Leaked-worker containment relies on process exit.

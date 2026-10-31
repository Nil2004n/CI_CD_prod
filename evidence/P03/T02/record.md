# P03-T02 — dependency and lock check (C3 = 3a)

Date: 2026-10-30. Executed by Opus (lead-owned: manifests and lockfile are shared contracts; no worker).
Authority: owner approval of P03-T01–T04 (2026-10-30); C3 = 3a; `planning/P03-T01-request.md` §4.
Base: `phase/P03-permissions-credentials` at `5775437`.

**Result: PASS.** Nothing was downloaded, built or executed except `cargo update` (no build scripts run).

| Step | Command / check | Result |
|---|---|---|
| Manifest change | `workbench/crates/application/Cargo.toml`: `sha2 = { workspace = true }` | only change to any manifest |
| Resolve | scratch copy `~/.local/share/pwt-dev-tools/_p03/t02-resolve/`; `unshare -rn env -i PATH=<1.97.1 bin>:/usr/bin:/bin HOME/CARGO_HOME=<_cargo-home> CARGO_TARGET_DIR=<…/p03-t02> RUSTC=<1.97.1 rustc> cargo update --workspace --offline` | exit 0, "Locking 0 packages" (`update.log`) |
| Registry cache | sha256 of `ls -R _cargo-home/registry` before/after | equal |
| Lock delta | `diff` old/new | one added line: `"sha2",` in the `application` entry |
| Registry set | (name, version, source, checksum) of the 86 registry entries, old vs new | equal |
| Package count | 94 → 94 | unchanged |
| Lock sha256 | `6c9d4998…7545` → **`d4b2efad605c88fc2870ea007475772c0bbb736305b1bf03919a0cb25f470751`** | new accepted value for `dev/p03_check.py` |

`sha2 0.11.1` was already built in the P02 graph (persistence), so no new build script or proc-macro is exposed.

FR-24 re-screen: **not applicable** while C2 = 2a (session-only credentials, no keyring dependency). cargo-audit: still stage C-A only; C-A2–C-E not approved; no new crate, so no new advisory exposure.

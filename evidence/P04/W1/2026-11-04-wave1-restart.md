# P04 wave 1 restart evidence

Owner approved T05–T07 work, requested `gpt-6.1-terra` workers, primary verification, no commits/pushes, and a stop for review. Base revision `f71749c`; phase and worker worktrees were dirty. Worker model overrides were requested; actual resolved identities and worker self-reports are not independently verifiable.

Primary reviewed the worker changes. Integrated one remote regression for `+` in canonical paths. The proposed remote character allowlist was withdrawn because the approved intake grammar does not authorize it. The runner worker added only a refusal regression; it was not integrated as a test of the existing stub, and the production runner remains refused. The T07 registry and lock changes remain in `worktrees/p04-authority` and are not integrated or independently tested: the registry change needs reconciliation with the identity contract; the lock changes need their own meaningful race/fault coverage. Checkout deletion remains refused.

The T01 request §3 preserves path case and `.git`. The existing store semantic rule `d11_06` in `workbench/crates/persistence/src/semantic/ids.rs` compares ASCII-case-insensitively and `canonical_of` strips a `.git` suffix. This is a contract conflict with practical implications for validated registry writes; owner choice was requested before changing a shared contract. SSH argument handling for path punctuation remains unverified.

Primary checks, cwd `workbench`, offline, locked, Rust 1.99.1 / Cargo 1.99.1, dirty phase tree at `f71749c`:

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test --offline --locked -p repositories --test p04_remote --test p04_lock` (baseline) | 0 | 18 passed, 0 failed, 0 ignored |
| `CARGO_TARGET_DIR=<phase>/workbench/target cargo test --offline --locked -p repositories --test p04_remote canonical_identity_preserves_graphic_path_punctuation` (worker worktree) | 0 | 1 passed, 0 failed, 8 filtered |
| `cargo test --offline --locked -p repositories --test p04_remote --test p04_lock` (integrated tree) | 0 | 19 passed, 0 failed, 0 ignored |
| `cargo clippy --offline --locked -p repositories --tests -- -D warnings` | 0 | passed |
| `cargo fmt --all -- --check` | 0 | passed |
| `git diff --check` (phase root) | 0 | passed for tracked diff |

`cargo fmt --all` also exited 0 before final checks. No full phase check, privileged setup, real network/credentials, production authority integration, commit, push or phase acceptance occurred. The filesystem had approximately 967 MiB free at the last check; generated artifacts and worktrees were preserved.

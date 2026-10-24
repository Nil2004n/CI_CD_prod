---
name: project-verification
description: Run this repository's real, already-verified checks for an owner-approved task or stage and record the evidence under evidence/. Use when asked to verify, build, re-run a spike, or produce a check record. Not for unapproved builds, fetches or network use.
allowed-tools: Bash, Read, Write, Grep, Glob
---

# Project verification (PWT Workbench)

You run checks and record what actually happened. You never declare a stage accepted; the owner does.

## Before running anything
1. Confirm the current approval covers the exact operation (build, run, fetch, network). Approvals live in `status.md` (Approvals row) and the latest owner message. If the operation is not covered, stop and say what approval is missing.
2. Record the state: `git rev-parse HEAD`, `git status --short`, `git branch --show-current`.
3. Re-hash protected tool inputs against `evidence/P00/P00-T02/install-record.md` ("Trusted hashes") before executing them. On mismatch, do not execute.

## Verified commands (discovered from evidence records; use only these)
- **Toolchain:** `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/{cargo,rustc}` (1.97.1), called directly, not through a rustup proxy.
- **Isolated Cargo environment** (`evidence/P00/P00-T05-D3-1/record.md`):
  `env -i PATH=<toolchain bin>:/usr/bin:/bin HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_TARGET_DIR=~/.local/share/pwt-dev-tools/_cargo-target RUSTC=<toolchain>/rustc CARGO_TERM_COLOR=never <toolchain>/cargo …`
  - First check that no `.cargo/config*` exists in any ancestor of `spikes/rust/`.
  - In zsh, put the command in a bash script or a bash array; zsh does not word-split variables (exit-127 trap).
- **Build (spikes):** from `spikes/rust/`, `unshare -rn <isolated cargo> build --frozen --offline -p <package>`. **Always `-p` per package, never `--workspace`**: combined builds change features (`evidence/P00/P00-T08/B-earlier-spike-comparison.txt`). Check `Cargo.lock` and manifest hashes before and after.
- **Graph:** `<isolated cargo> tree --frozen --target x86_64-unknown-linux-gnu -p <package> -e normal,build`.
- **Spike runners** (run only the exact recorded procedure, env check first):
  - T04: `spikes/t04-git/run_t04.sh`; see `evidence/P00/P00-T04/record.md`.
  - T05, T06: the namespace commands in `evidence/P00/P00-T05/record.md` and `evidence/P00/P00-T06/record.md`.
  - T07: `spikes/rust/t07-keyring/run/outer.sh <artifact> <evidence-dir> [preflight-only]`.
  - T08: `spikes/rust/t08-pdf/run/outer.sh <artifact> <evidence-dir> <artifact-dir> [preflight-only]`. Keep the artifact dir outside the repo.

## Present but NOT yet verified in this repo (Tech spec §22)
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. There is no application workspace yet (P01), and the spikes have never been run under these commands. Run them only if the stage approves it, and report the first run as establishing the command, not as a passing baseline.

## Unavailable (prerequisite missing — report as unavailable, never as passed)
`cargo audit` (not installed; deferred to P01), `shellcheck`, `bats` (not installed; required only for implemented `.sh` wrappers, extension X6). There is no CI in this repository.

## Recording (`evidence/README.md`)
- Capture the producer's own exit code with no pipe: `cmd > "$LOG" 2>&1; rc=$?; echo "exit=$rc"`. Never report a pipeline's exit status.
- Write logs and the check record **only under `evidence/<phase-or-cycle>/`** (e.g. `evidence/P00/P00-T08/`, `evidence/debug/Pxx-Dnn/`). Never edit other files.
- A record states: command, cwd, environment (tool versions, offline/network), revision and dirty state, exit code, pass/fail/skip counts, evidence paths, manual-pending items and known limitations.
- Keep failed runs; never overwrite or delete earlier records. A re-run goes in a new subdirectory.
- No secrets, keys, canary values or client content in evidence.

## Report
List each check as **passed / failed / skipped / unavailable / not run**, with the exit code and the evidence path. Say "not run" when a check did not run. Never infer a pass.

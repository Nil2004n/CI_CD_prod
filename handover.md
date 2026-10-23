# Handover

Checkpoint: 2026-10-24, P00-T06 spike built and run on `phase/P00-decisions-spikes` (local only): formal run failed (5/101), D01 proposed. Written by Opus directly; no Haiku documentation agent is configured, none ran.

## Resume steps
1. `git status --short`, `git branch --show-current`, `git log --oneline -3`. Expect branch `phase/P00-decisions-spikes` with the T06 failed-run commit on top of `2931774`, clean tree, not pushed; `main` = `origin/main` = `008e80c`.
2. Read `status.md`, then `plan.md` §1 (decisions, Q-01–Q-06) and §5 (P00 tasks).
3. For P00 spike work, use `spikes/P00-T01-spike-protocols.md` (common protocol §1, hypotheses §2, per-tool §3) and `spikes/P00-T01-fixture-specifications.md` (rules §1, catalogue §2).

## Facts
- No application code, tests, fixtures, generators, Cargo workspace or CI exist.
- P00-T01 deliverables are documentation. Tool flags, file names and exit codes in them are unverified candidates for the spikes to confirm.
- Main design point from T01: inline suppression directives (H-INL) and repository-loaded custom checks (H-EXE) are repository-controlled channels in addition to config files. Fake secrets are generated at build time and never committed (to keep them out of Git history and avoid host push-protection blocks).
- Evidence: `evidence/P00/P00-T01/check.md` (10/0/0; one checker pattern narrowed after a confirmed prose false positive, disclosed there).
- P00-T02: `spikes/P00-T02-installation-request.md` (Requests A Go binaries, B Python tools with interpreter options B1/B2/B3, C cargo-audit; decisions R-1–R-5). Values from read-only lookups logged in `evidence/P00/P00-T02/lookups.md`. Key facts: Go tools publish no signatures (same-origin checksums only); Checkov 3.3.30 declares Python 3.9–3.12, host has only 3.14; Checkov, uv and the CPython build are days old (cooldown option R-3).
- Owner approvals: planning stage and checkpoint commit/push (2026-10-23); P00-T01, its acceptance and commit/push; P00-T02 and Q-06 read-only lookups (2026-10-24). P00-T02 accepted as planning; R-1 (Request A), R-2 (B1), R-3 (listed versions), R-4 (cargo-audit deferred to P01), R-5 (upstream verification where available) decided; scoped local commit approved, no push. Then V1, resume of B1 and a local commit approved (no push).
- Not approved: cargo-audit (P01), crate lookups/downloads/builds, P00-T04/T05/T09, real API, cloning, scanner spikes, further pushes.
- Worker models unverified; agent definitions are only proposals.

- Installed dev tools (absolute paths only; nothing on PATH changed): see `evidence/P00/P00-T02/install-record.md`. Re-hash `~/.local/bin/gitleaks` and `/usr/local/bin/cosign` before reuse (user-writable, outside the prefix). Python tools: `~/.local/share/pwt-dev-tools/{yamllint/1.38.0,checkov/3.3.30}/venv/bin/`.

- P00-T03: `spikes/P00-T03-crate-request.md` rev 2 (selections §2, build exposure §3, first libc-only request §4, unknowns needing build tests §6); evidence `evidence/P00/P00-T03/research.md`. Only direct candidates were RustSec-screened; no transitive review; nothing resolved/fetched/built. Branch policy: phase branches; merges/pushes need approval.
- Conventions: capture exit codes without pipes (`evidence/README.md`); re-hash protected inputs against `install-record.md` "Trusted hashes" before execution (not complete tamper protection). PyPI provenance exception accepted for development only.

- D3-1: `spikes/rust/` workspace (`t05-process`, `libc =0.2.191`, committed `Cargo.lock`). Cargo always via toolchain binaries with `env -i`, isolated `CARGO_HOME`/`CARGO_TARGET_DIR` under `~/.local/share/pwt-dev-tools/`, builds inside `unshare -rn`. Built binary at `_cargo-target/debug/t05-process` never executed. Record `evidence/P00/P00-T05-D3-1/record.md`.

- P00-T05: harness `spikes/rust/t05-process/src/main.rs`, fixtures `spikes/rust/t05-process/fixtures/`. Run only via `timeout -s KILL 300 unshare -rn --pid --fork --mount-proc --kill-child env -i …` (harness refuses unless PID 1). Findings F-1–F-4 in `evidence/P00/P00-T05/record.md`.

- P00-T04: harness `spikes/t04-git/run_t04.sh` (D01 version); run only via the verified namespace setup (env check first). Run 1 failed, D01 re-run passed; both in `evidence/P00/P00-T04/record.md`.
- CA-02: Data Schema §23, versioned store 1.3 / protocol 1.1; canonical `schemas/workbench-store-v1.3.schema.json`; follow-ups FR-01–FR-04 in `plan.md` §5.2.

- P00-T06: `spikes/P00-T06-request-and-fixtures.md` — layers (parser vs spike composer vs projection vs validator), limits, cases Y01–Y21 / J01–J16, request §5, failure criteria §6, open questions §7. Nothing resolved/fetched/built yet.

- P00-T06: spike `spikes/rust/t06-yaml/` (composer, projection, harness, fixtures). Build/run only via the recorded namespace commands (env check first, `ulimit -v 1048576`). Failed run and proposed D01 in `evidence/P00/P00-T06/record.md`. Do not fix without approval.

## Next
Await approval of debug cycle P00-T06-D01. No repair, dependency or limit change without it.

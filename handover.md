# Handover

Checkpoint: 2026-10-24, installs accepted (with limitations) and pushed (`008e80c`); P00-T03 crate request completed, awaiting decisions C-1–C-6. Written by Opus directly; no Haiku documentation agent is configured, none ran.

## Resume steps
1. `git status --short`, `git log --oneline -3`. Expect `main` = `origin/main` = `008e80c` with uncommitted P00-T03 and documentation changes, unless a commit was since approved.
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

- P00-T03: `spikes/P00-T03-crate-request.md`. Versions are deliberately unresolved (no lookups authorised); §6 lists the exact lookups; §7 decisions C-1–C-6.
- Conventions: capture exit codes without pipes (`evidence/README.md`); re-hash protected inputs against `install-record.md` "Trusted hashes" before execution (not complete tamper protection). PyPI provenance exception accepted for development only.

## Next
Await owner decisions C-1–C-6 and the branch-policy question. Do not start P00-T04/T05/T09.

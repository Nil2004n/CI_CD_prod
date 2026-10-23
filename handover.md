# Handover

Checkpoint: 2026-10-24, P00-T02 installation request completed and awaiting owner decisions (P00-T01 accepted and pushed as `db402ca`). Written by Opus directly (no Haiku agent).

## Resume steps
1. `git status --short`, `git log --oneline -3`. Expect `main` at `db402ca` (= `origin/main`) with uncommitted P00-T02 files, unless a commit was since approved.
2. Read `status.md`, then `plan.md` §1 (decisions, Q-01–Q-06) and §5 (P00 tasks).
3. For P00 spike work, use `spikes/P00-T01-spike-protocols.md` (common protocol §1, hypotheses §2, per-tool §3) and `spikes/P00-T01-fixture-specifications.md` (rules §1, catalogue §2).

## Facts
- No application code, tests, fixtures, generators, Cargo workspace or CI exist.
- P00-T01 deliverables are documentation. Tool flags, file names and exit codes in them are unverified candidates for the spikes to confirm.
- Main design point from T01: inline suppression directives (H-INL) and repository-loaded custom checks (H-EXE) are repository-controlled channels in addition to config files. Fake secrets are generated at build time and never committed (to keep them out of Git history and avoid host push-protection blocks).
- Evidence: `evidence/P00/P00-T01/check.md` (10/0/0; one checker pattern narrowed after a confirmed prose false positive, disclosed there).
- P00-T02: `spikes/P00-T02-installation-request.md` (Requests A Go binaries, B Python tools with interpreter options B1/B2/B3, C cargo-audit; decisions R-1–R-5). Values from read-only lookups logged in `evidence/P00/P00-T02/lookups.md`. Key facts: Go tools publish no signatures (same-origin checksums only); Checkov 3.3.30 declares Python 3.9–3.12, host has only 3.14; Checkov, uv and the CPython build are days old (cooldown option R-3).
- Owner approvals: planning stage and checkpoint commit/push (2026-10-23); P00-T01, its acceptance and commit/push; P00-T02 and Q-06 read-only lookups (2026-10-24). P00-T02 accepted as planning; R-1 (Request A), R-2 (B1), R-3 (listed versions), R-4 (cargo-audit deferred to P01), R-5 (upstream verification where available) decided; scoped local commit approved, no push.
- Not approved: any install or download (Requests A–C), P00-T03+, crates, real API, cloning, further commits/pushes.
- Worker models unverified; agent definitions are only proposals.

## Next
Await owner decisions R-1–R-5; installs happen only as a separately approved step, recorded in `evidence/P00/P00-T02/install-record.md`.

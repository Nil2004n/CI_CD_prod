# Handover

Checkpoint: 2026-10-24, P00-T01 accepted, committed and pushed. Written by Opus directly (no Haiku agent).

## Resume steps
1. `git status --short`, `git log --oneline -3`. Expect the P00-T01 commit on `main` (parent `1e4a5b3`), equal to `origin/main`, clean tree.
2. Read `status.md`, then `plan.md` §1 (decisions, Q-01–Q-06) and §5 (P00 tasks).
3. For P00 spike work, use `spikes/P00-T01-spike-protocols.md` (common protocol §1, hypotheses §2, per-tool §3) and `spikes/P00-T01-fixture-specifications.md` (rules §1, catalogue §2).

## Facts
- No application code, tests, fixtures, generators, Cargo workspace or CI exist.
- P00-T01 deliverables are documentation. Tool flags, file names and exit codes in them are unverified candidates for the spikes to confirm.
- Main design point from T01: inline suppression directives (H-INL) and repository-loaded custom checks (H-EXE) are repository-controlled channels in addition to config files. Fake secrets are generated at build time and never committed (to keep them out of Git history and avoid host push-protection blocks).
- Evidence: `evidence/P00/P00-T01/check.md` (10/0/0; one checker pattern narrowed after a confirmed prose false positive, disclosed there).
- Owner approvals: planning stage and checkpoint commit/push (2026-10-23); P00-T01, its acceptance and commit/push (2026-10-24).
- Not approved: P00-T02+, installs, crates, downloads, network lookups, real API, cloning, further commits/pushes.
- Worker models unverified; agent definitions are only proposals.

## Next
Await approval of P00-T02 and Q-06 (read-only lookups of official pages).

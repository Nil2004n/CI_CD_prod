# Handover

Checkpoint: 2026-10-29, **P02 closed.** Accepted by the owner, merged into `main` (`4747214`, `--no-ff`, tree = phase tip `84fedd4`) and pushed. No phase is active. The previous handover, verbatim (includes all P00–P02 checkpoints and lessons): `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`).

## Resume here
1. `git status -sb` on `main`: expect `main...origin/main` and only `?? .commandcode/` (local, never commit).
2. Read `status.md` (open decisions) and `plan.md` §4 (P03 row) and §5.2 (FR-18–FR-24, FR-33).
3. Wait for the owner. **P03 planning is not approved.** When it is, write plan §5C as P01/P02 did (`plan.md` §5A/§5B): scope, facts, tasks/waves, acceptance criteria, risks, owner decisions; then stop for approval and create `phase/P03-…` from `main`.

## Verified state
- `main` check: `python3 -I -B dev/p02_check.py all --slot lead --final --out <new dir>` → PASS 866/0/0, 39 compile_fail, cli 48, static 28/28 (`evidence/P02/merge/check-main`). The lead Cargo target dir was deleted on 2026-10-29, so the next run rebuilds offline (slower first run; no network).
- Cargo only through the runner (pinned 1.97.1, `env -i`, isolated `CARGO_HOME`, `unshare -rn`, `--frozen`). Worker trees: add `--repo <worktree> --slot w<N>`.
- Differential B: `python3 -I -B dev/p02_differential.py --out <new dir>` (host jsonschema 4.26.0).
- Manual exit-4 check closed PASS 2026-10-29: `python3 -I -B evidence/P02/manual-exit4/run.py <out dir>` (uid-mapped user namespace + temp-dir poller + root-mapped control; `evidence/P02/manual-exit4/record.md`).
- P02 evidence map: `evidence/P02/P02-report.md` §2 (task → record), debug cycles `evidence/debug/P02-D01..D08/`, packets `planning/P02-packets/`.

## Lessons to carry into P03 (full list in the archived handover)
- Spawn workers with a short prompt pointing to a packet file; inline-prompt spawns hit classifier no-verdict errors.
- Do not pass harness `isolation: "worktree"` with a lead-prepared worktree; confine by packet and check `git status` of the main tree afterwards.
- Lead runs negative controls for anything a worker cannot mutate; check worker test arithmetic against suite counts.
- Keep a finding test `#[ignore]`d only until its debug cycle lands (the runner fails on ignored tests).
- Tests holding a `flock` in-process race with sibling `fork`→`exec`; use a bounded wait or a separate process.
- Each Cargo slot target dir grows to 2.5–4 GB; delete it and archive/remove the worktree once the work is committed.
- `--no-fail-fast` for test steps; negative-control scripts must restore files with a fresh mtime.

## Not approved
P03 planning; cargo-audit C-A2+; network; new dependencies; installs. Open items are listed in `status.md`.

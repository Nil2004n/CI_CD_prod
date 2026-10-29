# Handover

Checkpoint: 2026-10-29, **P02 closed.** Accepted by the owner, merged into `main` (`4747214`, `--no-ff`, tree = phase tip `84fedd4`) and pushed. **P03 planned (plan §5C), not approved;** planning commit on `phase/P03-permissions-credentials` (from `main` `01a3cbd`), pushed. The previous handover, verbatim (includes all P00–P02 checkpoints and lessons): `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`).

## Resume here
1. `git status -sb` on `phase/P03-permissions-credentials`: expect up to date with origin and only `?? .commandcode/` (local, never commit).
2. Read `plan.md` §5C (scope, facts, tasks T01–T12, criteria, risks, decisions C1–C10) and `status.md`.
3. Wait for the owner's P03 decision. On approval: T01 request (lead; layout, frozen interfaces, scope-match table spec, env profiles, packets like `planning/P02-packets/`), then T02–T04 and baseline B0 with a new `dev/p03_check.py` copied from `dev/p02_check.py` (floors start at 866/39).

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
P03 execution (any task); cargo-audit C-A2+; network; new dependencies; installs. Open items are listed in `status.md`.

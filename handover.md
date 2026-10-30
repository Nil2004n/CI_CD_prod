# Handover

Checkpoint: 2026-10-30, **P03-T01–T04 done and owner-accepted; B0 `5e29908` PASS; stopped before wave 1.** Earlier: 2026-10-29, **P02 closed.** Accepted by the owner, merged into `main` (`4747214`, `--no-ff`, tree = phase tip `84fedd4`) and pushed. **P03 planned (plan §5C), C1–C10 = recommended recorded 2026-10-29, no task started;** planning commit on `phase/P03-permissions-credentials` (from `main` `01a3cbd`), pushed. The previous handover, verbatim (includes all P00–P02 checkpoints and lessons): `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`).

## Resume here
1. `git status -sb` on `phase/P03-permissions-credentials`: expect up to date with origin and only `?? .commandcode/` (local, never commit).
2. Read `plan.md` §5C (scope, facts, tasks T01–T12, criteria, risks, decisions C1–C10) and `status.md`.
3. Read `planning/P03-T01-request.md` (frozen interfaces §5, table §6, env policy §7, tests §8, ownership §3) and `evidence/P03/B0/record.md`.
4. On the owner's go for wave 1: write packets `planning/P03-packets/T05-W1.md`, `T06-W2.md`, `T07-W3.md` (template `T04b-W1.md`), create three worktrees from `5e29908`, spawn three `sonnet-implementer` workers with short prompts, then integrate, verify with `dev/p03_check.py all`, raise floors and commit B1.

## Verified state
- **P03 runner:** `python3 -I -B dev/p03_check.py all --slot lead --out <new dir>` → B0 PASS 866/39, cli 48, static self-test 167. Use `--final` only at T11 (B0 stubs fail it by design). No Cargo target dir exists after the 2026-10-30 clean-up; the first run rebuilds offline (slower, no network).
- `main` check (P02 runner): `python3 -I -B dev/p02_check.py all --slot lead --final --out <new dir>` → PASS 866/0/0, 39 compile_fail, cli 48, static 28/28 (`evidence/P02/merge/check-main`). The lead Cargo target dir was deleted on 2026-10-29, so the next run rebuilds offline (slower first run; no network).
- Cargo only through the runner (pinned 1.97.1, `env -i`, isolated `CARGO_HOME`, `unshare -rn`, `--frozen`). Worker trees: add `--repo <worktree> --slot w<N>`.
- Differential B: `python3 -I -B dev/p02_differential.py --out <new dir>` (host jsonschema 4.26.0).
- Manual exit-4 check closed PASS 2026-10-29: `python3 -I -B evidence/P02/manual-exit4/run.py <out dir>` (uid-mapped user namespace + temp-dir poller + root-mapped control; `evidence/P02/manual-exit4/record.md`).
- P02 evidence map: `evidence/P02/P02-report.md` §2 (task → record), debug cycles `evidence/debug/P02-D01..D08/`, packets `planning/P02-packets/`.

## Lessons to carry into P03 (full list in the archived handover)
- P03 B0: the P02 `redact_static` rule cuts its argument at the first `)`, so fixed texts must not contain parentheses.
- `dev/p02_check.py static` now fails on the lock hash (expected since P03-T02); use `dev/p03_check.py` on this branch.
- Spawn workers with a short prompt pointing to a packet file; inline-prompt spawns hit classifier no-verdict errors.
- Do not pass harness `isolation: "worktree"` with a lead-prepared worktree; confine by packet and check `git status` of the main tree afterwards.
- Lead runs negative controls for anything a worker cannot mutate; check worker test arithmetic against suite counts.
- Keep a finding test `#[ignore]`d only until its debug cycle lands (the runner fails on ignored tests).
- Tests holding a `flock` in-process race with sibling `fork`→`exec`; use a bounded wait or a separate process.
- Each Cargo slot target dir grows to 2.5–4 GB; delete it and archive/remove the worktree once the work is committed.
- `--no-fail-fast` for test steps; negative-control scripts must restore files with a fresh mtime.

## Not approved
P03 wave 1 and later tasks (T05–T12); cargo-audit C-A2+; network; new dependencies; installs. Open items are listed in `status.md`.

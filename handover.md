# Handover

Checkpoint: 2026-10-31, **P03 wave 1 (T05–T07) integrated as B1 `b1ca4cb`; debug cycle P03-D01 closed; lead check PASS 1005/41; stopped before wave 2.** cargo-audit 0.22.2 installed (owner install grant), workbench lock 0 advisories. Earlier: 2026-10-30 T01–T04 accepted (B0 `5e29908`); 2026-10-29 P02 closed and merged. The previous handover, verbatim (includes all P00–P02 checkpoints and lessons): `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`).
## Resume here
1. `git status -sb` on `phase/P03-permissions-credentials`: expect up to date with origin and only `?? .commandcode/`.
2. Read `status.md`, `plan.md` §5C, `planning/P03-T01-request.md` (§5.4 approvals, §5.6 FR-33, §7 as amended by D01) and `evidence/debug/P03-D01/record.md` (deferred items that bind T08).
3. On the owner's go for wave 2: packets `planning/P03-packets/T08-W1.md` and `T09-W3.md` (template `_wave1-common.md` with base = the D01 commit, floors 1005/41). T08 must carry: workflow-scope grants stored with `run_id = null`; user-facing destinations via `canonical()`; a static rule that only `approvals.rs` acts on `Permitted`; run-grant end-of-run handling; deferred capabilities rejected. Reuse worktree paths `.claude/worktrees/p03-t05-w1` (slot w1) and a new `p03-t07-w3` (slot w3).

## Verified state
- **P03 runner:** `python3 -I -B dev/p03_check.py all --slot lead --out <new dir>` → B0 PASS 866/39, cli 48, static self-test 167. Use `--final` only at T11 (B0 stubs fail it by design). No Cargo target dir exists after the 2026-10-30 clean-up; the first run rebuilds offline (slower, no network).
- `main` check (P02 runner): `python3 -I -B dev/p02_check.py all --slot lead --final --out <new dir>` → PASS 866/0/0, 39 compile_fail, cli 48, static 28/28 (`evidence/P02/merge/check-main`). The lead Cargo target dir was deleted on 2026-10-29, so the next run rebuilds offline (slower first run; no network).
- Cargo only through the runner (pinned 1.97.1, `env -i`, isolated `CARGO_HOME`, `unshare -rn`, `--frozen`). Worker trees: add `--repo <worktree> --slot w<N>`.
- Differential B: `python3 -I -B dev/p02_differential.py --out <new dir>` (host jsonschema 4.26.0).
- Manual exit-4 check closed PASS 2026-10-29: `python3 -I -B evidence/P02/manual-exit4/run.py <out dir>` (uid-mapped user namespace + temp-dir poller + root-mapped control; `evidence/P02/manual-exit4/record.md`).
- P02 evidence map: `evidence/P02/P02-report.md` §2 (task → record), debug cycles `evidence/debug/P02-D01..D08/`, packets `planning/P02-packets/`.

## Lessons to carry into P03 (full list in the archived handover)
- Wave 1: restoring a file with `cp -a` keeps the old mtime, so Cargo reuses a stale (mutant) build; always `touch` after a restore.
- Worker transcripts record the API-reported model (`message.model`); use them as B-04 evidence after each spawn.
- Reviewers find frozen-contract issues too (D01 M1 was lead-owned B0 code): review lead stubs with the same scrutiny.
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
P03 wave 2 and later tasks (T08–T12); network; new dependencies. Installs beyond cargo-audit need a concrete request. Open items are listed in `status.md`.

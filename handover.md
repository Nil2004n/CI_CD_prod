# Handover

Checkpoint: 2026-10-31, **P03 wave 1 (T05–T07) integrated as B1 `b1ca4cb`; debug cycle P03-D01 closed; lead check PASS 1005/41; stopped before wave 2.** cargo-audit 0.22.2 installed (owner install grant), workbench lock 0 advisories. Earlier: 2026-10-30 T01–T04 accepted (B0 `5e29908`); 2026-10-29 P02 closed and merged. The previous handover, verbatim (includes all P00–P02 checkpoints and lessons): `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`).
## New developer orientation (added 2026-10-31)
- Project: PWT YAML Security Workbench, a Rust terminal app that assesses CI/CD YAML repositories with pinned local scanners. Specs `01_PRD.md` … `05_Database_Data_Schema.md`; working rules `CLAUDE.md`; schedule `plan.md`; live state `status.md`; this file for resuming.
- Way of working: the owner approves each phase, wave, debug cycle, install, network use, commit/push/merge. Nothing proceeds on its own; "Not approved" below is binding. Worker reports are not evidence — only the lead runner output under `evidence/` is.
- Branches: `main` (P00–P02 merged), `phase/P0N-*` per phase; active `phase/P03-permissions-credentials`. Commit messages name the task (`P03 B1: …`, `P03-D01: …`, `docs: …`).
- Host specifics: toolchain 1.97.1 under `~/.rustup`, isolated tools/caches under `~/.local/share/pwt-dev-tools/` (runner paths are hard-coded to user `kali`); `unshare` user/network namespaces must be available. `.commandcode/` is a local untracked folder: never commit it.
- Where things are: code `workbench/crates/`, runners `dev/`, task requests `planning/`, worker packets `planning/P0N-packets/`, evidence `evidence/P0N/` and `evidence/debug/Pxx-Dnn/`, phase reports `evidence/P0N/P0N-report.md`.

## Resume here
1. `git status -sb` on `phase/P03-permissions-credentials`: expect up to date with origin and only `?? .commandcode/`.
2. Read `status.md`, `plan.md` §5C, `planning/P03-T01-request.md` (§5.4 approvals, §5.6 FR-33, §7 as amended by D01) and `evidence/debug/P03-D01/record.md` (deferred items that bind T08).
3. On the owner's go for wave 2: **first write** packets `planning/P03-packets/T08-W1.md` and `T09-W3.md` (they do not exist yet; checked 2026-10-31) from the template `_wave1-common.md` with base = the D01 commit `b9fdc0e` and floors 1005/41. T08 must carry: workflow-scope grants stored with `run_id = null`; user-facing destinations via `canonical()`; a static rule that only `approvals.rs` acts on `Permitted`; run-grant end-of-run handling; deferred capabilities rejected. Create new worktrees (none exist after the 2026-10-31 clean-up), e.g. `.claude/worktrees/p03-t08-w1` (slot w1) and `p03-t09-w3` (slot w3). No Cargo target dir exists; the first runner call rebuilds offline (≈10 min).

## Verified state
- **P03 runner:** `python3 -I -B dev/p03_check.py all --slot lead --out <new dir>` → latest PASS at D01 1005/0/0, 41 compile_fail, cli 48 (`evidence/debug/P03-D01/check-lead`; B0 was 866/39, static self-test 167). Use `--final` only at T11 (T08–T10 stubs fail it by design). No Cargo target dir exists after the 2026-10-31 clean-up; the first run rebuilds offline (slower, no network).
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

# Wave 3 — rules common to T10 (W2) and T11 fixtures (W1)

Issued by Opus 2026-10-31 under the owner's approval of wave 3 ("I approve wave 3, start T10 and T11 … the sub agents should communicate with each other to avoid conflict"). Base: `phase/P03-permissions-credentials` at the commit that adds this file (code = B2 `4d6c70f`, accepted 2026-10-31). Floors: **1053 passed / 41 compile_fail**. Quality bar: production.

Everything in `planning/P03-packets/_wave1-common.md` applies (Read first, Frozen, Engineering rules, Permitted commands, Stop condition, Report), with the base and floors above and the additions below. The main checkout (`/home/kali/Desktop/CI_CD_prod`) is the lead's integration, testing and debug-orchestration tree: never edit, build or run anything in it. In parallel with you the lead edits `dev/p03_check.py` there (T11 static rule); your worktree keeps the base version of the runner.

## Step 0
`cd` into your worktree. `git rev-parse HEAD` equals the base commit named in your spawn message and `git status --short` is empty. Otherwise STOP.

## Coordination with the other worker (owner requirement)
Shared append-only log: `/home/kali/Desktop/CI_CD_prod/.claude/worktrees/_wave3-coord.md` (outside both worktrees; not in Git).

- **Append only**, with Bash: `printf '%s\n' '<line>' >> /home/kali/Desktop/CI_CD_prod/.claude/worktrees/_wave3-coord.md`. Never Write/Edit it; never rewrite or delete lines.
- Line format: `<UTC time> <W1|W2> <TOPIC>: <message>` (`date -u +%H:%MZ`).
- **Read it** at start, before each runner `test`/`all` run, and before your final report.
- **Post** `START`, `DONE`, and whenever your work could affect the other worker: `INFO` (behaviour another worker's tests may rely on — e.g. W2: the exact CLI wording/exit for approval and credential commands; W1: a hostile case that shows an evaluator or approvals behaviour W2's CLI scenarios will hit), `QUESTION`/`ANSWER`, `REQUEST` (need a file you do not own: do not touch it, the lead decides), `BLOCKED` (then stop and report).
- If W1's hostile fixtures expose a **defect** in already-accepted code (`permission/*`, `approvals.rs`, `credential.rs`, `child_env.rs`, persistence), post `FINDING: <one line>` and keep the failing test `#[ignore = "P03-D02 finding: …"]` in your file; do not fix accepted code. The lead decides on a debug cycle.
- No secrets, key values or long output in the log. One line per entry. Lead lines (`LEAD`) and lead messages win.

## Ownership (disjoint; paths relative to `workbench/crates/`)
| Worker | Task | Owned |
|---|---|---|
| W2 | T10 CLI | `cli/src/args.rs`, `cli/src/commands.rs`, `cli/src/output.rs`, `cli/src/features.rs`; `cli/src/lib.rs` (body of `run_session` only — signatures of `Session`, `run`, `run_session` frozen); `application/src/dev_permission.rs`; new `cli/tests/p03_*.rs`; in `cli/tests/p02_scenarios.rs` only the feature-index staleness test (`feature_index_rows_name_an_existing_command_and_test` and its helpers) so that it also scans `p03_*.rs` |
| W1 | T11 hostile fixtures | new `application/tests/p03_hostile.rs` |

Everything else is frozen, including `permission/*`, `approvals.rs`, `credential.rs`, `child_env.rs`, `dev_store.rs`, `cli/src/main.rs`, `persistence`, `domain`, manifests, `Cargo.lock`, `dev/`, `planning/`, docs.

## Stop condition
`python3 -I -B dev/p03_check.py all --slot <slot> --out <scratch>/all-final` = OVERALL PASS, passed ≥ 1053 + your added tests, compile_fail ≥ 41, `DONE` posted. W2 additionally: `all --final` PASS (no `todo!` left anywhere once T10 lands). Ignored tests make the runner fail: a W1 `FINDING` test kept `#[ignore]` is the one allowed exception — then report the runner failure honestly instead of claiming PASS. Report per common Report items 1–6, plus every coordination line you posted or acted on.

# Wave 2 — rules common to T08 (W1) and T09 (W3)

Issued by Opus 2026-10-31 under the owner's approval of wave 2 ("I approve wave 2, start T08 and T09 … the sub agents should communicate with each other to avoid conflict"). Base: `phase/P03-permissions-credentials` at the commit that adds this file (its parent `09fc256` has the same code as D01 `b9fdc0e`). Floors: **1005 passed / 41 compile_fail**. Quality bar: production.

Everything in `planning/P03-packets/_wave1-common.md` applies (Read first, Frozen, Engineering rules, Permitted commands, Stop condition, Report), with the base and floors above and the additions below. The main checkout (`/home/kali/Desktop/CI_CD_prod`) is the lead's integration, testing and debug-orchestration tree: never edit, build or run anything in it.

## Step 0
`cd` into your worktree. `git rev-parse HEAD` equals the base commit named in your spawn message and `git status --short` is empty. Otherwise STOP.

## Coordination with the other worker (owner requirement)
Workers run in parallel and cannot message each other directly, so you coordinate through one shared log:

`/home/kali/Desktop/CI_CD_prod/.claude/worktrees/_wave2-coord.md` (outside both worktrees; not in Git)

- **Append only**, with Bash: `printf '%s\n' '<line>' >> /home/kali/Desktop/CI_CD_prod/.claude/worktrees/_wave2-coord.md`. Never use Write/Edit on it, never rewrite or delete lines.
- Line format: `<UTC time> <W1|W3> <TOPIC>: <message>`. `date -u +%H:%MZ` gives the time.
- **Read it** (`cat`) at start, before each runner `all` or `test` run, and before your final report.
- **Post** when you start (`START`), when you finish (`DONE`), and whenever something you do could affect the other worker:
  - a behaviour change in `persistence` that the other's code or tests rely on (e.g. W3: when `DataDir::new` starts refusing a directory; W1: anything about how test data dirs are created);
  - a frozen item that blocks you (`BLOCKED`, then stop and report to the lead);
  - a need to touch a file you do not own (`REQUEST`): do not touch it; the lead decides at integration.
- Answer a `QUESTION` addressed to you with an `ANSWER` line. The lead may also post (`LEAD`) and may send you messages; lead instructions win.
- No secrets, key values or long output in the log. One line per entry.

## Ownership (disjoint)
| Worker | Task | Owned |
|---|---|---|
| W1 | T08 | `application/src/approvals.rs`; new `application/tests/p03_approvals*.rs`; `persistence/src/semantic/rules.rs` (rows D11-12, D11-13, D11-16, D11-17 only); `persistence/tests/semantic.rs` (status-count assertions only) |
| W3 | T09 | `persistence/src/fs.rs` (bodies of `process_effective_uid`, `parse_status_euid`, private helpers); `persistence/src/paths.rs`; new `persistence/tests/p03_fr33.rs`; new `application/tests/p03_fr33.rs` |

Paths are relative to `workbench/crates/`. Everything else is frozen, including `persistence/src/error.rs`, `application/src/dev_store.rs`, `permission/*`, `domain`, `cli`, manifests, `Cargo.lock`, `dev/`, `planning/` and docs. The runner static rule "only `approvals.rs` acts on `Permitted`" is lead work at T11.

## Stop condition
`python3 -I -B dev/p03_check.py all --slot <slot> --out <scratch>/all-final` = OVERALL PASS (non-`--final`: the T10 stub still holds `todo!`), passed ≥ 1005 + your added tests, compile_fail ≥ 41, `grep -n 'todo!' <your owned src files>` empty, `DONE` posted. Then report (common Report items 1–6, plus every coordination line you posted or acted on).

# P02-D08 — intermittent post-release failure in `lock_held_by_another_holder_exits_3` (closed PASS, attempt 1 of 2)

Date: 2026-10-28. Authority: P02 A9 within the T14 approval. Packet `planning/P02-packets/D08-W2.md`. Worker W2 `sonnet-implementer` (configured `claude-sonnet-5-5`; self-report unverified), worktree `.claude/worktrees/p02-d08-w2` (base `bf6130b` + lead-staged T14 + D07 state, staged sha `87cbf0e0…`).

**Anomaly:** reported by the D07 worker: after `drop(held)` the immediate `store validate` exited 3 instead of 0 in 2 of 6 `cli` runs (`cli/tests/p02_scenarios.rs:796`). Lead: 0 of 6 (`evidence/debug/P02-D07/cli-flake-{1..6}/`). Not reproduced deterministically.

**Diagnosis (hypothesis, consistent with POSIX `flock` semantics; not directly confirmed):** the test holds the lock through an open file description of the test process; sibling tests in the same binary spawn the CLI in parallel; between `fork` and `exec` each child holds a copy of the descriptor, so the lock briefly outlives `drop(held)`. Test-only; the product never spawns while holding the lock.

**Fix (test-only, +25/−2):** helper `store_after_release` re-runs the post-release command only while it exits 3, every 20 ms, for at most 10 s, and the assertion reports the try count; held-phase assertions unchanged. Other lock tests reviewed: `in_process_run_reports_a_held_lock_as_exit_3` makes no post-release assertion; `lock_held_by_a_second_process_exits_3_then_succeeds_after_release` holds the lock in a separate process (no inheritance) — unchanged.

**Verification:** worker: `all` PASS 866/0/0 and `cli` 10/10 PASS 48/0. Lead (main tree): `all --final` PASS 866/0/0, 39, cli 48 (`evidence/P02/T14/final-check/`); `cli` 5/5 PASS (`evidence/P02/T14/cli-repeat-{1..5}/`). Combined: 16 consecutive clean `cli` runs after the fix.

**Limitations:** the hypothesis is unconfirmed (no failure observed after the fix, so the try count was never shown); the retry would mask a genuinely delayed release for up to 10 s before failing.

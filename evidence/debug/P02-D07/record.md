# P02-D07 — F-1 stray `.prev` temp after a failed Replace (closed PASS, attempt 1 of 2)

Date: 2026-10-28. Authority: P02 A9 (pre-authorised debug cycle) within the T14 approval. Packet `planning/P02-packets/D07-W1.md`. Worker W1 `sonnet-implementer` (configured `claude-sonnet-5-5`; self-report unverified), worktree `.claude/worktrees/p02-d07-w1` (base `bf6130b` + lead-staged T14 state, staged sha `f2f65bbd…`).

**Defect (confirmed by the lead from code + POSIX semantics; reproduced by the ignored T14 test):** `persistence/src/atomic.rs::replace_steps` with `keep_previous`: after a failure at `Replace`, `<name>.prev` already links the target inode. The next rewrite links the target to `.<name>.prev.tmp-<hex>` and renames it over `<name>.prev`; `rename(2)` between two links of one inode succeeds without effect, so the temp name remained and `temps.forget` dropped it from cleanup. Severity low (no data loss; next `recover` removed it), but it broke "no stray temp after a successful write".

**Fix (bounded, 12 lines in `replace_steps`):** after the `ReplacePrevious` rename, if `prev_tmp` still exists remove it; a removal or non-NotFound metadata error is returned as a storage error at `ReplacePrevious` before the main `Replace` (target untouched); `prev_tmp` stays registered for cleanup until gone.

**Tests:** `persistence/tests/fs_atomic.rs` + `a_rewrite_after_a_failed_replace_leaves_no_stray_prev_temp`, `failing_to_remove_the_prev_temp_is_reported_before_the_main_replace`; `application/tests/dev_store_faults.rs`: F-1 test un-ignored; F-1 tolerances removed (strict zero strays in the fault and crash matrices).

**Lead verification:** integrated `all --min-passed 866 --min-compile-fail 39` PASS 866/0/0, 39, cli 48 (`integrated-check/`). Lead negative control — fix block removed in the main tree: 5 tests FAIL (2 new persistence tests, the F-1 test, `recover_rewriting_the_log_survives_every_fault`, `recover_that_dies_mid_rewrite_leaves_temps_that_the_next_recover_removes`); restored byte-identical (`cmp` vs worktree) (`nc-lead-fix-reverted/`). The lead's 6 `cli` runs before D08 (`cli-flake-{1..6}/`) all passed.

**Limitations:** the `remove_file` failure path is exercised only through a test `FsOps` wrapper (`FaultFs` never injects `remove_file`); non-NotFound `symlink_metadata` errors in the new block have no dedicated test.

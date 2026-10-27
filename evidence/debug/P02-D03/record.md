# P02-D03 — owner-approved review findings O1–O10 and Q-T04-1

Opened 2026-10-27 by Opus. Authority: owner, 2026-10-27: "I approve all recommended options, run P02-D03, spawn all necessary sonnet5.5 sub-agents and start the work in parallel branch session, in the main session merge them and test them. give me the report". This approves the recommended option of every row in `evidence/P02/wave1a/record.md` §5 / `status.md` "Open owner decisions": O1–O10, Q-T04-1 (rename) and wave 1b after P02-D03. cargo-audit C-A2 had no recommended option and is **not** run. Attempt 1 of 2 for every item.

## Split
| Item | Owner | Files |
|---|---|---|
| O8 escape set + `values` forbidden set shared | lead | `domain/src/text.rs`, `domain/src/store/values.rs` (one function), `domain/tests/text.rs`, `domain/tests/store_values.rs` (count) |
| Q-T04-1 `RunStarted.at` → `started_at` | lead | `domain/src/contract.rs`, `domain/tests/contract.rs` |
| O3 dependency | lead | `Cargo.toml`, `persistence/Cargo.toml`, `Cargo.lock` (+`"libc"` edge only), `dev/p02_check.py` (`ALLOWED_DIRECT_DEPS`, `EXPECTED_LOCK_SHA256`) |
| O1, O4, O5, O6, O7, O9, O10 | W1 (`sonnet-implementer`, worktree) | packet `planning/P02-packets/D03-W1.md` |
| O2, O3 | W3 (`sonnet-implementer`, worktree) | packet `planning/P02-packets/D03-W3.md` |
| Request §5.2/§5.5 text, records | lead | `planning/P02-T01-request.md` §5.10 |

W1 and W3 own disjoint files and branch from the same lead base, so they run in parallel (owner rule: up to three mutating workers, disjoint files, own worktree, common baseline).

## Lead part
- **O8.** `text::needs_escape` gains U+00AD, U+034F, U+115F, U+1160, U+180E, U+2061–2064, U+3164, U+FFA0, U+FFF9–FFFB and U+E0000–E007F (142 code points; set 82 → 224). It becomes `pub(crate)` and `values::forbidden_char` is now `TAB || LF || needs_escape`, so the two sets cannot diverge. The independent oracle and range table in `tests/text.rs` and the store-values count (85 → 227) are updated; 16 neighbour code points added. This amends the P01-frozen escape set (P01 D5 / `planning/P01-T01-request.md` §5.3). Visible effect: emoji tag sequences (e.g. subdivision flags) are now escaped, like ZWJ sequences already were.
- **Q-T04-1.** `AppEvent::RunStarted { run_id, started_at }`; golden JSON updated. App contract stays 1.1 (no 1.1 consumer yet).
- **O3 dependency.** `libc =0.2.191`, default features off, direct `persistence` dependency. Registry set unchanged (libc was already in the accepted graph); `Cargo.lock` changes by one line (`"libc"` in `persistence` dependencies), new sha256 `6c9d4998…`, pinned in the runner. `--frozen` build/clippy/test accept the lock.
- **Lead run 1** (`lead-run1/`, uncommitted lead part on `d73d807`): `all` PASS, 26 suites, 497 / 0 / 0, 32 compile_fail; static PASS with the new lock hash.

## New finding (not in this cycle — owner decision)
- **Q-D03-1:** `AppEvent::TriggerFired { workflow_id, at }` and `AppEvent::TriggerSkipped { workflow_id, at, reason }` have the same flattened-envelope `at` collision as `RunStarted` (missed by the T04 finding). Recommended: rename to `fired_at` / `skipped_at` within app contract 1.1, same as Q-T04-1. Needed before wave 3 (T11).

## Retest plan (named)
1. Lead `all` on the lead part (done, run 1).
2. Lead `all` on each worker branch.
3. Lead `all` on the integrated branch; floors raised to the new count.
4. Read-only `reviewer` + `security-reviewer` over the D03 diff.

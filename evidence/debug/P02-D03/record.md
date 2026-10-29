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

## Result: closed, PASS on attempt 1 of 2 (2026-10-27)
**Workers** (both `sonnet-implementer`, configured `model: claude-sonnet-5-5`, `isolation: worktree`, base `b50f64c`; actual model resolution not observable, no substitution notice; self-reports unverified, lead re-ran everything):

| Worker | Commit | Files (verified `git diff --stat`) | Tests added |
|---|---|---|---|
| W3 (O2, O3) | `b7a9174` | `persistence/src/{fs,paths,lock}.rs`, `tests/fs_atomic.rs` (`fault.rs` unchanged) | 16 |
| W1 (O1, O4–O7, O9, O10) | `2311014` | `domain/src/store/values.rs`, `tests/store_values.rs` | 10 |

**W1 stop (handled per packet).** W1 stopped with `test` FAIL: the `store_plan.rs` provider fixture `keychain:pwt/openrouter` (not W1-owned) is invalid under approved O1 (`/` → `Character`). The lead changed it to `keychain:pwt.openrouter` (`ece49d3`); the two bad-value rows next to it (`sk-…`, 40×`a`) stay rejected. W1 then committed unchanged owned files. W1's interim result from a scratch copy outside the repo is not used as evidence.

**Removed test lines (lead check, `git diff d73d807 9e311ee -- …/store_values.rs`):** 6 lines, each replaced under an approved rule: the locator fill helper (64-byte segment cap), sample `pwt/openrouter` → `pwt.openrouter`, count 85 → 227 (O8), and the `{run31}:{run31}/{run31}` accept assertion → `/` is now `Character` (plus a `:`-separated accept). No regression case was dropped.

**Retests (lead runs, all `python3 -I -B dev/p02_check.py`):**

| # | Run | Tree | Result |
|---|---|---|---|
| 1 | `lead-run1/` | lead part | PASS 497/0/0, 32 compile_fail |
| 2a | `lead-verify-w3/` | W3 `b7a9174` | PASS 513/0/0, 32 compile_fail |
| 2b | — | W1 `2311014` alone | not run separately (W1 branch lacks `ece49d3`, so `store_plan` fails by construction); covered by run 3 |
| 3 | **`integrated-check/`** | integrated `9e311ee` | **PASS** fmt, clippy `-D warnings`, test 26 suites **523 / 0 / 0**, 32 compile_fail, build, cli (0 scenarios yet), static incl. lock `6c9d4998…` |
| 3b | `floor-check/` / `floor-control/` | same, floors 523/32 | exit 0 / exit 1 with `--min-passed 524` (expected) |
| 4 | read-only reviews | `d73d807..9e311ee` | `reviewer`: 0 high, 1 medium, 5 low; `security-reviewer`: 0 high, 0 medium, 6 low, 2 info |

Arithmetic: 497 + 16 (W3) + 10 (W1) = 523. Floors in `dev/p02_baseline.json` raised to **523 / 32**.

## Review findings (none fixed in this cycle; outside approved scope or new policy)
| ID | Sev | Source | Finding | Recommended |
|---|---|---|---|---|
| R-D03-1 | medium | reviewer | O6 cap 32 + O1 run check: a 32-char plain username (`[A-Za-z0-9_-]`) is `Forbidden`; effective limit is 31 plain chars | Exempt the ssh/scp user from the run check (cap 32 stays) |
| R-D03-2 | low | reviewer | `acquire`: non-NotFound metadata error on `rag/`/`backup/` → `Io{Lock}`, not `PathRejected` (fails closed either way) | Record as accepted deviation |
| R-D03-3 | low | reviewer | uid-spoof test misses `backup/` | Add the case (test only) |
| R-D03-4 | low | reviewer | `PRIVATE_DIR_MODE` unused; duplicates `atomic::DIR_MODE` | Single constant at T10 |
| R-D03-5 | low | security | `IanaTimezone` accepts leading `-` | Reject (`Syntax`) |
| R-D03-6 | low | security | Slash-permitting tokens accept `../x`, `a/../b` | Rule: token types never used as path components (P04/P05 acceptance check), or reject `..` segments |
| R-D03-7 | low | security | Not fully canonical: IPv6 forms, port leading zeros (`00443`), unreserved `%XX` aliases | Canonicalise at P03/P16 matching; disclose |
| R-D03-8 | low | security | uid proxy fails for euid 0; ancestors of data dir unchecked; T10 must use `check_private_dir` | P06 acceptance check: refuse euid 0; T10 packet requirement |
| R-D03-9 | low | security | Escape set still hand-picked (variation selectors, U+180B–D, U+2065, Arabic format controls, space look-alikes) | P06: derive from `Default_Ignorable_Code_Point` + `Cf` |
| R-D03-10 | info | security | 16-wildcard cap is not ReDoS safety; `O_NOCTTY` not set; libc not RustSec-screened (C-A2 not run) | P05 linear matcher (already an acceptance check); disclose |

Plus **Q-D03-1** (above): `TriggerFired.at` / `TriggerSkipped.at` envelope collision.

# P02-D05 — owner decisions E-1, E-2, R-D03-1, R-D03-5, Q-D03-1 (and E-3 as a T08 requirement)

Opened 2026-10-27 by Opus. Authority: owner, 2026-10-27: "I approve all the recommendations. Spawn the required sonnet5.5 sub-agents … stop before commit and push, main session is there for the tests, orachastration and initiation of the debug cycle. I want pure production grade work". This approves the recommended option of every row of `status.md` "Open owner decisions (after wave 1b + P02-D04)". Interface amendment: `planning/P02-T01-request.md` §5.11. Attempt 1 of 2 for every item.

**B1 (commit + push) is not executed:** it conflicts with the same message's "stop before commit and push"; pushing is outward-facing, so the lead keeps everything uncommitted and asks. B1's cleanup part was executed for the six merged, clean worktrees (preservation: `git status --porcelain` empty, branch ancestor of `HEAD`; `git worktree remove` non-force, `git branch -d`). The two wave-1b worktrees hold uncommitted work and are kept.

## Base
`9537c67` + uncommitted wave 1b + P02-D04 (integrated PASS 603/0/0, 32 compile_fail; `evidence/debug/P02-D04/record.md`).

## Split
| Item | Change | Owner |
|---|---|---|
| E-1 (lead part) | `closed_enum!` moved from `store/evidence.rs` to `store/mod.rs` (`$crate::store::values::deserialize_parsed`; textual scope, defined before the `mod` declarations); W2 imports removed | lead (done) |
| Q-D03-1 | `TriggerFired.fired_at`, `TriggerSkipped.skipped_at` (`contract.rs`, golden JSON in `tests/contract.rs`); new guard `no_event_payload_uses_an_envelope_field_name` | lead (done) |
| E-1 (worker part) | Convert the 19 derive-based store enums (`plan` 6, `provider` 1, `workflow` 3, `repository` 1, `tools` 8) to `closed_enum!`; no-echo planted-marker tests for every store-enum field | W1 (`planning/P02-packets/D05-W1.md`) |
| E-2 | `Identifier`/`PolicyId` charset without `/`, `@`; reject `..` | W1 |
| R-D03-1 | ssh/scp user exempt from the key-run check | W1 |
| R-D03-5 | `IanaTimezone` segment starting with `-` rejected | W1 |
| E-1 guard | Source test: no `#[derive(…Deserialize…)]` on an enum in `domain/src/store/**` | lead, after integration |
| E-3 | Read-boundary no-echo requirement | T08 packet (wave 2) |

## Lead part — verification
- Run1 FAIL (lead): fmt diff in `tests/contract.rs`; clippy `unused_imports` (the `use super::closed_enum;` imports are redundant under textual scope). Fixed (imports and re-export removed, rationale documented in `mod.rs`; `fmt-fix` on the lead file). A vacuous length assert in the first draft of the guard test was removed before the run (`all_events()` is derived from `golden_cases()`).
- Run2 PASS 604/0/0, 32 compile_fail, all steps (session scratchpad `lead/d05/lead2/`).

## Worker part
- `sonnet-implementer` (configured `claude-sonnet-5-5`; resolution not observable; self-report unverified) in lead-prepared worktree `.claude/worktrees/p02-d05-w1` (branch `p02/d05-w1`, base `9537c67` + uncommitted state applied staged, patch sha256 `34954976…`, verified identical to the main tree). Increment unstaged, sha256 `b17651d4…`: 9 owned files, +802/−376.
- All 19 enums converted to `closed_enum!` (wire format unchanged: existing schema-enum, round-trip and Data §8/§9 tests pass unchanged); no-echo tests for all 20 enum-typed fields (string, wire±marker, numbers, array, object; from `Value` and text) with a source scan that fails on an unplanted enum field and a self-check proving the helper catches an echoing derive; E-2, R-D03-1, R-D03-5 with positive/negative tests; worker mutation check (4 tests failed as intended, file restored, `cmp` verified).
- Updated expectations (approved rule changes, not weakening): `Identifier` sample without `/`/`@`; `a..b` now `Syntax` for `Identifier`/`PolicyId` only; the D03 O6 test asserted a 32-byte plain user is `Forbidden` — reversed by R-D03-1 and renamed `d03_o6_ssh_and_scp_user_is_at_most_32_bytes_and_may_be_a_plain_run`.
- **Disclosed worker deviation:** one `fmt-fix` runner call (not in the permitted list); lead confirmed from the diff that only owned files changed.
- Lead verification of the worktree (`all`): PASS 611/0/0, 32 compile_fail.

## Integration and lead guard
- Increment applied to the main tree (`git apply --index`, no conflict). Lead E-1 guard in `tests/store_any.rs`: `no_store_enum_derives_deserialize` (source scan of all 16 store modules), `the_enum_scanner_detects_a_derived_deserialize` (positive control), `every_store_module_is_scanned` (module list equals `pub mod` declarations).
- Integrated run `integrated-check/` (before any mutation): PASS 627/0/0, 32 compile_fail. 627 = 604 (lead D05 part) + 13 (`persistence/tests/read.rs`, wave-2 prep, request §5.12) + 7 (worker) + 3 (guard).

## Negative controls (lead, `negative-controls/nc2.py`)
| ID | Mutation | Failing test(s) |
|---|---|---|
| NC4 | add a `#[derive(Deserialize)]` enum to `tools.rs` | `no_store_enum_derives_deserialize` |
| NC5 | `#[serde(rename = "at")]` on `TriggerFired.fired_at` | `no_event_payload_uses_an_envelope_field_name`, `every_event_variant_serialises_to_its_golden_json` |
| NC6 | `"1.3"` removed from the older-version list | `version_classes_follow_request_6_1` |
| NC7 | `Problems::push` cap `<` → `<=` | `problems_are_capped_and_truncation_carries_over` |
All four restored byte-identical (sha256).

**Lead harness defect found and fixed (this cycle):** the control scripts restored files with `shutil.copy2`, which also restores the old mtime; Cargo then treated the artifacts built from the mutated source as fresh. The first floor control after the mutations (`floor-control-628-stale-run1.txt`) therefore ran stale NC7 code and failed one test (101 vs 100). Source on disk was correct (hash-verified). Fix: restored files touched; both scripts (`P02-D04/negative-controls/nc.py`, this `nc2.py`) now use `copyfile` + `os.utime`. Re-run on a rebuilt tree `integrated-check-final/`: **PASS 627/0/0, 32 compile_fail**; floor control at 628 fails on the floor only (`floor-control-628.txt`). The P02-D04 post-control run (`P02-D04/integrated-check-final/`) is superseded by this fresh run, which contains all D04 code.

## Result — closed PASS (attempt 1 for every item), 2026-10-27, uncommitted
Floors raised to **627/32** (`dev/p02_baseline.json`). Nothing committed or pushed.

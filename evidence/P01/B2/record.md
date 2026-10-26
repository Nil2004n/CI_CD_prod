# P01-T08 and baseline B2

Date: 2026-10-26.
- **Authority:** owner approval of B1, of the four B1 decisions and of T08 (2026-10-26).
- **Base:** B1 `d95090c`.
- **Result: B2 integrated. The lead `--final` run passes.** Baseline B2 is the commit containing this record.

## Worker
- W1′ (`sonnet-implementer`, configured `claude-sonnet-5-5`, actual resolution not observable, self-report unverified) worked on branch `p01/w1-contract`, commit `67b9bf9`, in a worktree created from B1 by step-0 fast-forward.
- Changed files: `contract.rs` (bodies of `AppCommand::status`, `AppEvent::status`, `AppEvent::delivery` and `dispatch_guard`, a private `DEFERRED_DETAIL` constant, and 16 doctests) and the new `tests/contract.rs`.

## Lead review
- **Non-doc diff:** only the four bodies and the constant. No type, field, derive, serde attribute or visibility changed.
- **Classification and delivery:** exhaustive matches with no wildcard arm.
  - Commands: 23 pilot, 6 reserved.
  - Events: 32 pilot, 2 reserved.
  - Delivery: `SelectionChanged` is LatestValue; `TaskProgress` and `ChatResponseChunk` are Coalescible; everything else is MustDeliver. The reserved events are also MustDeliver, by documented choice, because Arch §6 gives them no rule.
- **`dispatch_guard`:** derived from `status()`. Reserved commands get `OperationFailed{Deferred, redact_static(DEFERRED_DETAIL)}`. The literal contains no braces and no interpolation.
- **compile_fail pairs:** all 8 new pairs differ from their positive control in exactly one line, the offending line (mechanical diff).
- **Golden JSON:** `golden_cases()` covers all 34 event types, and each case is asserted.
- **Test count:** the suite has 22 tests. The report claimed 33, and **P01-D03** corrected that (no code change).

## Checks
| Run | Command | Result |
|---|---|---|
| Worker branch | `p01_check.py all --slot lead --repo <wt> --min-passed 379 --min-compile-fail 17` | PASS (`evidence/P01/T08/lead-verify`) |
| Worker branch | `p01_check.py static --slot lead --repo <wt> --final` | PASS |
| **Integrated B2** | `p01_check.py all --slot lead --final --min-passed 379 --min-compile-fail 17 --out evidence/P01/B2/check-run1` | **PASS**: env, fmt, clippy (0 warnings), test (19 suites, **379 passed / 0 failed / 0 ignored, 17 compile_fail**), build, static `--final` (no `todo!`/allow anywhere; both contract enums allowlisted; checker self-test) |

Per-suite counts: unit 3, contract 22, ids_time 279, states 23, text 18, doctests 34 (17 run + 17 compile_fail: text 8, ids 1, contract 8).

## Limitations
- Golden JSON pins the current serialisation. There is no read-back for events (T01-D3).
- The `DEFERRED_DETAIL` wording was chosen by the worker. The owner may change it, but any change is a contract-visible text change.
- `redact_static` still carries the `cfg_attr(not(test), allow(dead_code))` attribute, which is now redundant. Removing it is a lead item for T09.

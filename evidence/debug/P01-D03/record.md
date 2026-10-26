# P01-D03 — test count misreported in the P01-T08 hand-back (W1′)

Date: 2026-10-26. Authority: owner pre-authorisation (`planning/P01-T01-request.md` §9). Responsible worker: W1′ (`sonnet-implementer`, configured `claude-sonnet-5-5`, unverified), resumed read-only.

**Confirmed anomaly (lead):**
- The T08 report said `tests/contract.rs` was "new, 33 tests".
- The lead counted 22 `#[test]` functions. A 23rd textual match is a comment on line 9.
- The lead's run shows exactly 22 executed tests in that suite, with names matching the declarations one for one (`evidence/P01/T08/lead-verify/test.log`). No declared test is skipped.

**Worker answer:**
- The correct count is 22.
- "33" was written from memory without counting, and its source cannot be traced. The worker gives no explanation beyond that.
- The worker's corrected per-suite counts equal the lead's: unit 3, contract 22, ids_time 279, states 23, text 18, doctests 34 (17 + 17 compile_fail), total 379.
- **Lead correction of the worker's breakdown:** the 9 wave-1 compile_fail doctests are 8 in `text.rs` plus 1 in `ids.rs`, not 9 in `text.rs`.

**Outcome:**
- No code change. The cycle is closed.
- Lesson recorded: worker test counts are cross-checked against per-suite log lines, not only against totals.

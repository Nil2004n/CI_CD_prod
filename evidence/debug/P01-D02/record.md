# P01-D02 — review-driven hardening of `text.rs` (P01-T06, W2)

Date: 2026-10-26.
- **Authority:** owner pre-authorisation (`planning/P01-T01-request.md` §9).
- **Source:** lead-triaged findings of the read-only `security-reviewer` (L1/L2) and `reviewer` (#5, #11) on the B1 candidate.
- **Responsible worker:** W2 (`sonnet-implementer`, configured `claude-sonnet-5-5`, unverified), resumed on `p01/w2-text`.

## Confirmed findings
1. **(L1/L2, reviewer #5)** T08 must build fixed, application-authored `OperationFailed`/`StorageFailure` details as `RedactedText`. The only constructor was `redact(raw, &dyn Redactor)`. An identity `Redactor` would make any runtime string look redacted.
2. **(reviewer #11)** The positive control of the "deserialising" compile_fail doctest did not use `serde_json`, so the compile_fail case could pass for the wrong reason.

## Fix (W2 commit `3b1547e`, parent `622f989`)
- **New constructor:** `pub(crate) fn redact_static(text: &'static str) -> RedactedText`.
  - It applies the same `escape`. The public surface is unchanged.
  - Its doc comment states the literal-only discipline and the `Box::leak` caveat.
  - `#[cfg_attr(not(test), allow(dead_code))]` stays until T08 calls it.
  - Three unit tests in `#[cfg(test)] mod tests`.
- **Doctest control:** the deserialising control now calls `serde_json::from_str::<String>`. The compile_fail case is unchanged.
- **Diff check (lead, `git diff 622f989 3b1547e`):** only `text.rs` changed, +45 / −0. The one new item is `pub(crate)`.

## Lead verification
- **Integrated run** (`evidence/P01/B1/check-run4`): fmt, clippy, test (341 passed, 9 compile_fail) and build all PASS. **static FAIL** on the new lead check "no Redactor impl in src": it hit W2's test-only identity redactor inside `#[cfg(test)] mod tests` (`text.rs:328`).
  - That is a **lead checker defect**, not a worker defect: the packet asked for exactly that unit test.
  - The checker was fixed to blank `#[cfg(test)] mod` blocks for production-code checks, with self-test cases in both directions.
- **Re-run** (`evidence/P01/B1/check-run5`): **PASS**. 341/0/0, 9 compile_fail, static 11/11 checks, checker self-test 21/21.

Cycle count: 1. Closed.

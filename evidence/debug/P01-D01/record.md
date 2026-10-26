# P01-D01 — suspected doctest heading mislabel in `text.rs` (P01-T06, W2)

Date: 2026-10-26.
- **Authority:** owner pre-authorisation of debug cycles for anomalies the lead finds after a worker task (`planning/P01-T01-request.md` §9).
- **Responsible worker:** W2 (`sonnet-implementer`, configured `claude-sonnet-5-5`, unverified), resumed in its own worktree on branch `p01/w2-text`.

## Lead diagnosis (wrong)
While verifying commit `2495d85`, the lead read `text.rs` lines 78–135 and reported the doctest prose headings as "shifted by one". The lead had assumed each compile_fail case is followed by its control.

## Worker finding (confirmed by the lead)
- W2 could not reproduce the defect. In `2495d85`, each positive control sits **directly before** its compile_fail case and carries the same heading: line 70 → line 82 (`From<String>`), line 94 → line 106 (`Into`), line 118 → line 130 (`From<&str>`).
- The lead re-read the file at `2495d85` (`git show 2495d85:…/text.rs | grep -n …`) and confirmed this.
- **Result: no defect. The lead's diagnosis was a misreading.**

## Change made in the cycle
- **Commit:** `622f989`, W2. It rewords all 16 doctest headings so the pairing is explicit: "Positive control for the next case (compiles) – …" and "Compile-fail case: … (does not compile)".
- **Verified by the lead:** `git diff 2495d85 622f989` touches only `text.rs` (16 insertions, 16 deletions), and **0 changed lines that are not `///` lines**.
- **Worker retest:** `p01_check.py all --slot w2 --package domain` reported 34/0/0, 8 compile_fail. This is not evidence on its own; the lead's integrated run is (`evidence/P01/B1/check-run1`).

**Cycle count for this "defect": 1. It is closed with no defect.** It does not count towards the two-cycle limit of any real defect.

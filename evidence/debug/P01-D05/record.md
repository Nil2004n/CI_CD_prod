# P01-D05 — test-file `#[allow]` attributes without reasons (after the lint tightening)

Date: 2026-10-26.
- **Authority:** owner approval of the recommended lint tightening (T09 report) and debug-cycle pre-authorisation (`planning/P01-T01-request.md` §9).
- **Worker:** `sonnet-implementer` (configured `claude-sonnet-5-5`, unverified), fresh instance, branch `p01/d05-allow-reasons`, base `4d82f94`.

## Cause and failure
- **Cause:** lead commit `4d82f94` set the workspace clippy lints `dbg_macro`, `print_stdout`, `print_stderr`, `todo`, `unimplemented`, `unwrap_used`, `expect_used`, `panic`, `exit` and `mem_forget` to `deny`, and added `allow_attributes_without_reason = "deny"`.
- **Failure:** the lead clippy run (`evidence/P01/T10/lint-tighten-run1`) failed at four test-file allows that had a comment but no `reason`: `tests/{versions,contract,states}.rs` (file-level) and `tests/ids_time.rs:276` (helper-level).

## Fix
- **Commit:** `38851f3`.
- **Change:** each of the four attributes gained `reason = "…"`, with the lint lists unchanged. The three duplicate `//` comment lines were removed. The lead read the diff: the 4 test files only, 22 insertions and 7 deletions, all inside those attributes.

## Verification (lead)
| Run | Evidence | Result |
|---|---|---|
| Worker branch | `evidence/P01/T10/D05-lead-verify` (`--final`) | PASS, 415/0/0, 32 compile_fail |
| Integrated `cf54227` | `evidence/P01/T10/check-final` (`--final`) | **PASS**, 415/0/0, 32 compile_fail, clippy clean |

**Report inaccuracy (minor, recorded, no cycle):** the worker described the domain doctests as "32 passed (the compile_fail doctests)". The two 32-lines are 32 ordinary doctests and 32 compile_fail doctests (64 in total).

Cycle count: 1. Closed.

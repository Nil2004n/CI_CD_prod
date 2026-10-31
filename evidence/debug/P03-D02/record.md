# P03-D02 — wave-3 review findings (attempt 1 of 2; open)

Authority: pre-authorised debug cycles, C10 = 10a (`planning/P03-T01-request.md` §9). Base B3 `f63c39a`. Findings from the lead-spawned `reviewer` and `security-reviewer` (Sonnet, read-only) on B1–B3; lead confirmed the code facts before assigning.

| Item | Finding | Fix | Owner |
|---|---|---|---|
| A1 (sec M1, medium, confirmed: `domain/src/text.rs` escape set keeps U+000A/U+0009; `cli/src/output.rs` pushes a constraint as one line) | Multi-line constraint forges lines in human `approvals list` | CLI renderer escapes LF/CR/TAB in free-text fields; `dev permission-check` rejects control/escapable characters in constraints, bounds 16 × 512 B | W2′ |
| A2 (rev M1, medium) | No CLI test for run-lifetime / model-transmission positive paths or approve/deny/revoke JSON shapes | Tests | W2′ |
| A3 (rev M2, medium) | `approvals list` hides inference scope | Show provider, model, policy, source count, redaction version | W2′ |
| A4 (rev M3, medium) | FR-33 feature row overstates CLI coverage | Reword row; gap recorded | W2′ |
| A5 (rev L3–L5, sec I2, low) | Vacuous harness-argv assertion; stale comments; wrong usage text; overstated doc wording | Fix | W2′ |
| A6 (rev L9, low) | Closed stdout not tested for permission-check | Test | W2′ |
| B1 (rev L1, low, confirmed) | `approvals list` classifies oversize/non-regular store file as exit 4, writer path as 5 | Align classification | W1′ |
| B2 (sec L5 / O3, low, confirmed) | `ToolDirs` derived `Debug` prints paths | Path-free `Debug`; sweep includes it | W1′ |
| C1 (sec L2 / rev L8, low) | `RequestOutcome::Permitted` (unconsumed) could be treated as a go-ahead by later callers | Lead static rule 13: `RequestOutcome::Permitted` matched only in `application/src/approvals.rs` (src) | Lead |

**Recorded, not fixed (owner or later phase):** R7/R8 ignore inference scope (owner question; `p03_hostile.rs` pins it); store has no integrity binding / same-user CLI approve (P06 entry gate); no cap on pending records; post-write emit failure message wording (frozen `ApprovalError`); stale `--scope once` note (UX, P15); stdin std-buffer residue (best effort, D01 H5); identical-looking approvals apart from scope (TUI); `resource_constraints` verbatim (now bounded and control-free after A1); FR-33 has no CLI-level process test; §5.7 "6 after a revision change" read as R4 (5 for the old revision, 6 for a request at the new one) — owner to confirm.

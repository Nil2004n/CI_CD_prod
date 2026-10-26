# P01-D06 — domain migration to CA-03 (canonical tool availability)

Date: 2026-10-26.
- **Authority:** owner approval of CA-03 before the P01 merge, and debug-cycle pre-authorisation.
- **Worker:** `sonnet-implementer` (configured `claude-sonnet-5-5`, unverified), fresh instance, branch `p01/d06-ca03`, commit `4b4b3fe`, base `cb4910c`.
  - The base is the lead CA-03 commit: schemas, specs, plan and `versions.rs`. Its tests deliberately did not compile.

## Change
| File | Change |
|---|---|
| `contract.rs` | `ToolStatus` → `ToolAvailability`, variants `ready, missing, incompatible, unconfigured, unavailable, deferred` (`denied` removed, `unconfigured` added); `DependencyStatusChanged.status` → `availability` |
| `vocab.rs` | doc path updated to v1.4 |
| `tests/contract.rs` | golden case and enum-value test; new schema cross-check that `ToolAvailability` equals `ToolManifest.availability` in order, `denied` absent, with an exhaustive-match guard |
| `tests/states.rs`, `tests/versions.rs` | v1.4 schema path; expected versions store 1.4 and UI 1.1; no check removed |

**Lead diff review:** the non-doc changes are exactly the rename, the variant swap, the field rename, the paths and the version literals.

## Verification (lead)
| Run | Evidence | Result |
|---|---|---|
| Worker branch | `evidence/P01/CA-03/D06-lead-verify` | PASS, 416/0/0, 32 compile_fail |
| Integrated, baseline floor raised to 416 | `evidence/P01/CA-03/check-final` | **PASS**, 416/0/0, 32 compile_fail |
| CA-03 contract check after the merge | `evidence/P01/CA-03/check-output-post-merge.txt` | 29/0/0 |
| Conformance rev 2 | `evidence/P01/conformance/conformance-rev2.txt` | 26 PASS / 1 NOTE |

- **Conformance rev 2 detail:** the spec inconsistency is resolved, and the only NOTE left is the deferred test directories.
- **Script fix:** the first rev-2 run also flagged A-11, because the conformance script still hard-coded the pre-CA-03 versions. The lead fixed the script to read the Arch A-11 line, and re-ran it.

Cycle count: 1. Closed.

# P02-D04 — wave 1b review findings (test and documentation gaps)

Opened 2026-10-27 by Opus. Authority: P02 A9 pre-authorised debug cycles for anomalies the lead finds after a worker task (bounds `planning/P02-T01-request.md` §11: no change to frozen interfaces, acceptance criteria, dependencies, lockfile, scope or permissions; never weaken assertions; two attempts per defect, then owner). Owner 2026-10-27: "I approve wave 1b … main session is there for the tests, orchestration and initiation of the debug cycle … stop before commit and push". Attempt 1 of 2 for every item.

## Baseline
- Base `9537c67`; wave 1b uncommitted. W1 (T05b) worktree `.claude/worktrees/agent-a4ba8a936ec281447`, W2 (T06) worktree `.claude/worktrees/agent-abb89aad73dcb2554`; worker work staged there, not committed.
- Lead verification per branch (`--slot w1|w2 --repo <worktree>`, `all`): W1 PASS 552/0/0, W2 PASS 566/0/0, both 32 compile_fail.
- Integrated in the main tree (patches sha256 `9494c53c…` W1, `5b94540c…` W2, applied with `git apply --index`, no conflict) + lead `AnyStore`/`StoreTypeMarker` (`store/mod.rs`) + `tests/store_any.rs`: run1 FAIL (lead: clippy `large_enum_variant` on `AnyStore`), fixed by boxing `Report` (deviation from request §5.3, serialised form unchanged); run2 PASS 598/0/0, 32 compile_fail, all steps. Logs: session scratchpad `lead/b1/all2/` (copied to `evidence/` at B1).
- W1 report arithmetic error: "470 plus the 32 doc-tests" — suites sum to 438; 438 + 64 doctests = 502 (total correct, breakdown wrong). Report correction only, no defect (as P01-D03).

## Items in this cycle (confirmed from code by reviewer, re-read by lead)
| ID | Source | Defect | Fix | Owner |
|---|---|---|---|---|
| D04-1 | reviewer L1 | Field order is never tested (`serde_json` without `preserve_order`; "byte-stable" tests compare a serialisation with itself) | Per def: serialised key order of the maximal instance (from `to_string`, not `Value`) equals the schema `properties` order; Data §8/§9/§10 examples compared as text after re-serialisation where the example is in schema order (report any example whose order differs) | W1 (13 defs), W2 (20 defs) |
| D04-2 | reviewer L2 | `assert_eq!(names.len(), N)` on a literal array cannot fail | Remove, or drive per-def checks from the list | W1, W2 |
| D04-3 | reviewer L3 | `EventType` sync test misses non-literal/multi-line arms; `>= 30` guard loose | Also count the `AppEvent` variants / `Self::` arms in the extracted body and require equality with the extracted names | W2 |
| D04-4 | reviewer L6 | W1 `nullable` lists not asserted against the schema | Assert each list equals the def's `anyOf … null` properties (as W2's `def_nullable`) | W1 |
| D04-5 | security I-2 | Config-side R fields not covered by a source scan | Source scan of W1's five files: every `RedactedText` field uses a `store_read` helper; field set equals Appendix A class R for those defs | W1 |
| D04-6 | reviewer M1 (doc) | `evidence.rs` header implies uniform no-echo | State the scope exactly: `closed_enum!` types are no-echo; reused derive enums, `SanitisedText`, IDs, timestamps, primitives and unknown-field names rely on the read boundary (`persistence::read`, T08) | W2 |
| D04-7 | reviewer L7, security I-7 | `AnyStore` variants / `StoreTypeMarker` undocumented; stale `dead_code` reason in `text.rs` | Lead: rustdoc added; reason updated (`redacted_vec` still unused) | lead (done) |

## Held for the owner (not in this cycle: frozen interfaces / new policy)
See `status.md` "Open owner decisions": enum mechanism (reviewer M1/M2, security M-1), `Identifier`/`PolicyId` charset (security M-2), and requirements recorded for T08/T09/T10 (security L-1–L-4, reviewer L4/L5).

## Result — closed PASS (attempt 1 for every item), 2026-10-27
- Workers: two fresh `sonnet-implementer` instances (configured `claude-sonnet-5-5`; resolution not observable; self-reports unverified), run in the existing W1/W2 worktrees, increments left unstaged. W1: `store_models_a.rs` only (+309/−21, 29 → 33 tests). W2: `store_models_b.rs` (+1 test, 43 → 44) and the `evidence.rs` module header (documentation only; lead read the diff).
- Lead verification per worktree (`all`): W1 PASS 556/0/0, W2 PASS 567/0/0, both 32 compile_fail.
- Increments taken with `git diff --binary` (sha256 `b6df6e5c…` W1, `fb8b03a3…` W2) and applied to the main tree with `git apply --index`; no conflict.
- **Integrated check PASS 603/0/0, 32 compile_fail**, all steps (`integrated-check/`, explicit floors 523/32; `integrated-check-final/`, repeat run). 603 = 523 (D03) + 33 (`store_models_a`) + 44 (`store_models_b`) + 3 (`store_any`).
- **Negative controls (lead, `negative-controls/`, script `nc.py`):** each mutates one source file, runs the domain tests, restores the file and checks its sha256 (all restored OK):
  - NC1 swap the first two `Engine` fields (W2) → `engine` FAILS;
  - NC2 swap `Schedule.timezone`/`next_due_at` (W1) → `schedule` and `data_schema_section_nine_workflows_json_is_byte_stable` FAIL;
  - NC3 add a variant to `EventType` → `event_type_equals_the_app_event_names` FAILS.
- Floors raised in `dev/p02_baseline.json` to **603/32**; control at 604 fails as expected (`floor-control-604.txt`). Note: the runner reads the baseline only with `--final` (which fails until T14 by design on B0 stub markers); non-final runs take `--min-passed`/`--min-compile-fail` explicitly.
- Both workers confirmed the Data §8/§9/§10 examples are already in schema key order (no example/schema difference).
- Limitations disclosed by workers: the order and source scanners are text-based and depend on rustfmt layout (they fail loudly on a layout change, not silently); `AppEvent` variant count is a layout heuristic.
- Nothing committed or pushed (owner: stop before commit and push).

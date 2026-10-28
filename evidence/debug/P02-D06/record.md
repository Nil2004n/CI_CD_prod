# P02-D06 — T08 fixtures violate T09 semantic rules after the wave-2 merge

Opened 2026-10-27 by Opus. Authority: P02 A9 pre-authorised debug cycles (anomaly found by the lead after worker tasks; bounds request §11). Attempt 1 of 2.

## Trigger
Wave 2 verified per worktree by the lead (rebuilt trees): T08 PASS 660/0/0, T09 PASS 642/0/0, T10 PASS 680/0/0 (all 32 compile_fail). Integration in the main tree: increments taken with `git add -N` + `git diff --binary` (sha256 prefixes T08 `c3630372…`, T09 `856f6a9d…`, T10 `563d6cb0…`). First apply attempt refused (`does not match index`: lead wave-2-prep files were unstaged in the main tree; nothing applied; that run re-tested the unchanged 627 baseline). Main tree staged (`git add workbench dev`; index verified equal to the wave-2 base patch `9048d7a5…`), then all three applied with `git apply --index`, no conflict.

Integrated run (`int/r2`): fmt, clippy, build, cli, static PASS; **test FAIL: 606 passed, 9 failed** — all nine in `persistence/tests/schema.rs` (T08): `an_absent_required_nullable_key_fails_at_the_schema_layer`, `concurrent_first_use_agrees`, `every_keyword_the_schema_uses_is_enforced_with_its_location`, `every_fixture_has_the_verdict_the_manifest_expects`, `format_negatives_fail_at_the_schema_layer`, `j11_every_valid_example_passes_schema_and_typed_decode`, `planted_markers_are_never_echoed`, `schema_minimums_are_enforced_by_the_schema_layer`, `the_boundary_values_are_accepted`. All T09 and T10 tests pass.

## Diagnosis — confirmed
Every failure is a `Layer::Semantic` problem on a fixture the T08 tests expect to be `Valid`. Twelve distinct (rule, pointer) pairs (packet `planning/P02-packets/D06-W1.md`). Each was checked against the fixture value and the spec:
- D11-24 `chat_persistence_enabled = true`: PRD §16 "chat persistence deferred; `chat_persistence_enabled=false`" — rule correct (also settles T09 question Q1).
- D11-25 `max_active_repositories = u64::MAX`: above the development cap (packet T09, pending owner confirmation) — rule follows the packet.
- D18-07 eligible file without digest; D11-07 `patterns` with `paths`; D11-09 `daily` with `interval_seconds`; D11-03 check file not selected; D11-05 `repository_file` source without repo/path; D18-04 adapter mismatch and `exit_code` + `term_signal`; D18-07 `rendered` artifact without sources; D18-12 `completed` enrichment with a reason — each a genuine cross-field inconsistency in the fixture.
Cause: T08 built maximal fixtures field-by-field while the semantic registry was empty (T08 disclosed the risk). No rule defect found. Hypotheses not pursued: none.

## Fix (bounded)
Fresh `sonnet-implementer` on T08's test files only (`tests/schema.rs`, `tests/fixtures/store/**`), in lead-prepared worktree `.claude/worktrees/p02-d06-w1` holding the integrated state (patch `6538eadf…`, verified identical): fixtures made semantically consistent while staying maximal; plus a test that every valid fixture passes all layers. Retest: full integrated run.

## Worker part
Fresh `sonnet-implementer` (configured `claude-sonnet-5-5`; resolution not observable; self-report unverified). Increment unstaged, sha256 prefix `f6b73997…`: 9 fixtures + `tests/schema.rs` (+282/−31). The worker's new test `every_valid_fixture_passes_the_semantic_rules_too` found violations beyond the nine failing tests (also `approvals.max`, `log.max`, `rag_index.max`, `report.min`, `tool_manifest.max`); all fixed with consistent values, maximality kept where rules allow (a second workflow carries the daily/explicit maxima). New test `values_the_schema_admits_and_a_semantic_cap_rejects` keeps the "schema has no upper bound" intent at the semantic caps. No assertion removed; no rule judged wrong. Lead verification of the worktree: PASS 730/0/0.
Worker note kept open: `report.max` `selected_ref` (`release/2026.10`) differs from its plan snapshot ref (`main`), so the D11-21 selected/missing-count comparison does not apply to that fixture — fixture coherence item for the wave-2 review.

## Integration (main tree)
- D06 increment applied (`git apply --index`), plus lead B2 integration tests `persistence/tests/store_integration.rs` (5 tests: `StoreWriter` with the real `SchemaValidator` and all semantic rules over the T08 fixtures — init, replace/reopen of every writable fixture, `write_report`, reconciliation of all four in-flight states, `restore_previous`).
- Run `int1`: FAIL 1 — **lead test defect**: `every_writable_fixture_round_trips_through_replace_and_read` expected `log.max` unchanged after reopen, but it holds a `queued` run that reconciliation correctly interrupts. Fixed: unchanged unless runs were interrupted; then exactly those runs differ, only in `execution` and `finished_at`. Plus a fmt diff in the lead file.
- **Lead runner defect found by negative control NC8** (reconciliation does not set `finished_at`): only T10's own test failed; the integration binary never ran because `cargo test` stops at the first failing test binary. Passing runs are unaffected (every binary runs when nothing fails); earlier controls each hit their intended test, so no recorded verdict changes, but failure lists in failing runs were incomplete. Fix: `--no-fail-fast` in the runner's `test` and `cli` modes (`dev/p02_check.py`). NC8 repeated: 8 binaries ran; `each_in_flight_state_becomes_interrupted_and_terminal_runs_are_untouched`, `every_writable_fixture_round_trips_through_replace_and_read` and `reconciliation_output_is_accepted_by_the_real_validator` fail; file restored byte-identical (fresh mtime).
- **Final integrated run (`integrated-check/`): PASS 735/0/0, 32 compile_fail**, all steps. 735 = 627 + 33 (T08) + 15 (T09) + 53 (T10, incl. 1 doctest) + 2 (D06) + 5 (lead integration). Floors raised to 735/32; control at 736 fails on the floor only (`floor-control-736.txt`). First failing integration run kept in `integrated-check-first-fail/`.

## Result — closed PASS (attempt 1), 2026-10-27, uncommitted

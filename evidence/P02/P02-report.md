# P02 report — JSON store v1.4, validation, writer, recovery and CLI skeleton

Date: 2026-10-29.
- **Phase branch:** `phase/P02-store`; base `main` `4a300cb`. Code baseline **B4 `0cc562e`**; HEAD `6b3fd98` differs from B4 only in `status.md`, `handover.md`, `plan.md` (`git diff --stat 0cc562e HEAD`).
- **Prepared by:** Opus (lead). No Haiku documentation agent is installed.
- **Status:** **accepted by the owner on 2026-10-29** ("I accept p02, commit and push to the phase branch, merge to the main branch"), with the limitations in §8. `evidence/P02/T15/final-check/` is the aborted 2026-10-28 start and is kept only as history, not evidence.

This report states what was done and what was verified. It is not a claim that the code is secure or production-ready.

## 1. Scope delivered (plan §5B.1)
- **Store models (`domain::store`):** typed models for all ten roots of `schemas/workbench-store-v1.4.schema.json` and the 57 `$defs` reachable from them (the six wrapper-protocol defs are P06; T01-D2). FR-31 safe-text types on all 105 plain-string properties (Appendix A rev 2: E 6 / N 66 / R 27 / S 6).
- **Validation (`persistence`):** embedded schema pinned by SHA-256, FR-15 settings (format on, external `$ref` refused, unavailable schema → `schema_unavailable`), version gate, `store_type` discrimination, typed errors with JSON pointers and no value echo; semantic registry of 44 rule IDs over Data §11 / §18.
- **Persistence:** `.app.lock`, hardened opens (`O_NOFOLLOW|O_NONBLOCK`, regular file, `nlink == 1`), data-dir mode/ownership check, temp → flush → replace → directory sync with recovery copy, `FsOps` fault seam, serialised `StoreWriter`, startup reconciliation, report naming + collision check, export manifests (JSON only; Markdown/PDF are P10–P12).
- **Emission (`application::emit`):** envelope `event_id` (UUID v4) / `at` / `level` behind injectable clock and ID source; `AppEvent` → store `Event` mapping; emission-time validation.
- **CLI (D-17):** `crates/cli`, binary `pwt-workbench-cli`: `--version`, `help`, `features` (27-row feature index + staleness test), `--json` JSON Lines, required `--data-dir`, exit-code table, `store init|validate|show|recover` (labelled `dev`); scenarios `cli/tests/p02_{scenarios,failures}.rs`.
- **Dev tooling:** `dev/p02_check.py` (P01 runner + `cli` mode, 28 static checks with 77-case self-test, floors in `dev/p02_baseline.json`), `dev/p02_differential.py` (differential B), `dev/p02_fr31_appendix.py`, `dev/p02_registry.json`.
- **Dependencies added:** `jsonschema =0.60.1` (defaults off), `sha2 =0.11.1` (defaults off), `libc =0.2.191` (open-flag constants only; P02-D03 O3). No `unsafe`.

**Not in P02** (per plan): permission scope matching and grant consumption (P03), credentials (P03), Git/inventory (P04/P05), process supervision and real redactor (P06), Markdown/PDF (P10–P12), RAG content (P17), migrations from 1.0–1.3, retention/compaction, TUI.

## 2. Tasks
| Task | Result | Record |
|---|---|---|
| T01 request + FR-31 Appendix A | Approved, T01-D1–D12 | `planning/P02-T01-request.md`, `planning/P02-T01-fr31-classification.md` |
| cargo-audit C-A | PASS (fetch + packaged-lock inspection; nothing built) | `evidence/P02/cargo-audit-C-A/record.md` |
| T02 offline resolution + exposure | PASS, 94 packages / 86 registry entries = P00 | `evidence/P02/T02/record.md` |
| T03 / CA-04 | Approved and applied (app contract 1.1, specs 0.3.5) | `planning/P02-T03-CA-04-draft.md`, `evidence/P02/CA-04/` |
| T04 scaffold, B0 `a9ea9ce` | PASS 416/32 | `evidence/P02/T04/record.md` |
| Wave 1a (T05a, T07), B1a `d73d807` | PASS 497/32; D01, D02 | `evidence/P02/wave1a/record.md` |
| D03 (O1–O10, Q-T04-1) | PASS 523/32 | `evidence/debug/P02-D03/record.md` |
| Wave 1b (T05b, T06) + D04, D05 | PASS 603/32, 627/32 | `evidence/debug/P02-D04/`, `P02-D05/` |
| Wave 2 (T08, T09, T10) + D06, B2 `2448abf` | PASS 735/32 | `evidence/P02/wave2/record.md` |
| Wave 3 T11 + T12, B3 `49b8cba` | PASS 805/32, cli 34 | `evidence/P02/T11/`, `T12/record.md` |
| Wave 3 T13, B3b `bf6130b` | PASS 840/39 | `evidence/P02/T13/record.md` |
| T14 integration + D07, D08, B4 `0cc562e` | PASS 866/39, cli 48; reviews 0 high | `evidence/P02/T14/record.md` |
| T15 final check, freeze, report | PASS (§3) | this report, `evidence/P02/P02-freeze.json`, `evidence/P02/T15/` |

## 3. Final verification (lead, 2026-10-29)
- **Command:** `python3 -I -B dev/p02_check.py all --slot lead --final --out evidence/P02/T15/final` (cwd repo root).
- **Tree:** HEAD `6b3fd98` (code = B4); dirty only `?? .commandcode/` (local), `?? evidence/P02/T15/`.
- **Environment:** rustc/cargo/rustdoc 1.97.1 called directly, `env -i`, isolated `CARGO_HOME` (offline crate copy), lead target dir rebuilt from scratch after the 2026-10-28 clean-up, every Cargo command in `unshare -rn` (probe errno 101), `--frozen`; protected files (lock, manifests) hash-checked unchanged around every step.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --frozen -- -D warnings` | exit 0 |
| `cargo test --workspace --frozen --no-fail-fast` | exit 0. **41 suites, 866 passed, 0 failed, 0 ignored, 39 compile_fail**; floors 866/39 from the committed baseline |
| `cargo build --workspace --frozen` | exit 0 |
| `cli` (`cargo test -p cli --frozen --tests --no-fail-fast`) | exit 0, 4 suites, **48 passed** |
| static | **28/28** (incl. self-test, lock registry set, lock SHA `6c9d4998…`, 8 workspace members, manifest allowlist) |
| differential B (`dev/p02_differential.py`, `evidence/P02/T15/differential-b/`, run 2026-10-28 on the B4 code tree) | PASS: host jsonschema 4.26.0, schema SHA = pin, 31/31 fixtures agree, 5 negative controls |
| cargo-audit | **not run** (§4 criterion 11) |

Evidence: `evidence/P02/T15/final/` (`summary.json`, `env.json`, `static.json`, per-step logs), `evidence/P02/T15/final.stdout`. An earlier `all --final` start (`evidence/P02/T15/final-check/`, empty `final-check.stdout`) was stopped by the lead at the owner's stop on 2026-10-28; it has no verdict and is **not** evidence.

**Negative controls** (the checks can fail) are in the task records: floor controls at every baseline (e.g. 867 in T14), NC1–NC8, static planted violations (`evidence/P02/T14/nc-static-planted/`), D07 revert control, T13 model-file control, T11/T12 behaviour controls.

**Manual-pending → closed 2026-10-29 (after acceptance):** PASS with permissions enforced, branch execution observed, root-mapped control skipped as expected (`evidence/P02/manual-exit4/record.md`). Original note: `real_storage_failure_exits_4_when_permissions_are_enforced` returns early inside the runner (the user namespace maps to root) and counts as passed there; run it once as an unprivileged user outside `unshare`. Exit 4 is otherwise pinned by the application fault matrix, `application_storage_failures_map_to_exit_4` and the `Exit::for_failure` unit test.

## 4. Acceptance criteria (plan §5B.4)
| # | Criterion | Status and evidence |
|---|---|---|
| 1 | fmt/clippy/test/build PASS via `--final`; P01 floors not lowered; new floors committed | **Met.** §3; floors 866/39 committed in B4 (P01 was 416/32) |
| 2 | Every root: valid examples pass schema **and** typed round trip; ≥1 schema-negative and ≥1 version-negative per root | **Met.** `persistence/tests/schema.rs` J11 (Data §8–§10 examples + min/max fixture per root, schema + typed decode), `store_integration.rs` replace/reopen round trip; schema-negatives per root through the generated single-edit mutants of every valid fixture (differential A) and keyword/format cases; version-negatives J12 for every root (1.3, 1.2, 1.1, 1.0, 1.5, 2.0, 9.9, number, null, missing). File-level `invalid-*` fixtures exist for 5 roots only; the per-root negatives are generated in code |
| 3 | FR-15, FR-16 (J11–J13, J16); external `$ref` never fetched, never valid | **Met.** J11–J13 tests; `external_references_never_validate_and_never_reach_the_retriever`, `the_production_validator_never_calls_the_retriever`; pin mismatch / non-compiling schema → `schema_unavailable`; differential B (J16) 31/31 |
| 4 | Every Data §11/§18 rule in the rule table; implemented rules have positive + negative tests; deferred parts name their phase | **Met.** 44 IDs, each once in order (`registry_lists_every_rule_id_once_in_order_and_nothing_else`); 17 implemented, 16 partial (each `rest_in` names a phase, e.g. P03, P06/P13, P07, P10/P17, P14), 9 producer, 2 policy (`registry_status_counts_are_as_reviewed`); negative cases assert the exact (rule, pointer) set |
| 5 | Recovery: interruption at each write step keeps the last validated copy; malformed/foreign files reported, not overwritten; ENOSPC/EACCES → `StorageFailure`, further writes stop; abandoned runs → `interrupted`; pending approvals stay pending | **Met** at persistence and application level (T10 writer/recovery tests, T14 fault matrix over every reached write step × {StorageFull, PermissionDenied, Interrupted}, D07). Limits: ENOSPC/EACCES are injected through `FsOps`; the real-permission CLI case is manual-pending (§3); CLI-level `restore_previous` matrix not covered (L-T14-1, low) |
| 6 | Same-second reports for one repo get distinct names; slugs bounded, never raw paths | **Met.** `persistence/tests/report_name.rs` (run ID in stem; bounded ASCII slug; same run → `ReportNameCollision`, first file unchanged). The CLI same-second scenario is weaker than its name (L-T14-2, low) |
| 7 | FR-31 (1)–(3), incl. schema-completeness test and negative control | **Met.** `domain/src/store/classes.rs` (105 rows), `domain/tests/fr31.rs` (completeness both directions, row-removal control), sealed `text::store_read`, 7 compile_fail guards + controls, static rules for store-model fields and `store_read` use |
| 8 | Every pilot `AppEvent` maps to a schema-valid `Event`; non-terminal `RunFinished` etc. rejected at emission | **Met as specified in the T11 packet:** 25 persisted events map to schema-valid `Event` records (checked with the real validator); 7 pilot events are deliberately not persisted (5 `Chat*`, `SelectionChanged`, `TaskProgress`), plus `SuppressionChanged` and reserved `RunComparisonReady`, pinned by the exact-set test. Non-terminal `RunFinished`/`CheckCompleted`, illegal run-state changes and reserved events are rejected without consuming an ID |
| 9 | CLI: §4.1 definition of done; runner `cli` mode headless; no real app-data dir; no secret in CLI output fixtures | **Met.** 48 scenarios headless; `--data-dir` required; static rule "no environment/home/temp/cwd lookups in cli, application, persistence"; feature index 27 rows + staleness test; no-echo tests over seeded markers |
| 10 | reviewer + security-reviewer: no unresolved high; mediums fixed or owner-accepted | **Met.** All passes 0 high. Mediums: R-D03-1 fixed (D05); T14 R-M1 recorded as manual-pending limitation, R-M2 partly covered with residual low — both recorded in `evidence/P02/T14/record.md` §4 and presented here for acceptance |
| 11 | cargo-audit result recorded, or "not run" with reason | **Met (not run).** Stage C-A PASS; C-A2–C-E (native C build exposure `aws-lc-sys`, `ring`; install; advisory-DB fetch) not approved. Last RustSec screen: P00 2026-10-24; all 86 registry entries equal their P00 entries |

## 5. Debug cycles
| Cycle | What | Outcome |
|---|---|---|
| P02-D01 | FR-31 Appendix A rev 1 missed string maps | Generator rev 2, 105 rows |
| P02-D02 | Wave-1a review findings (serde echo, pinned type, multibyte bounds) | Fixed |
| P02-D03 | Owner O1–O10, Q-T04-1 (hardening, `libc` edge, escape set 82 → 224) | Fixed, 523/32 |
| P02-D04 | Wave-1b review findings (field order, vacuous asserts, `EventType` sync) | Fixed, NC1–NC3 |
| P02-D05 | E-1, E-2, Q-D03-1, R-D03-1/-5, wave-2 prep | Fixed, NC4–NC7 |
| P02-D06 | T08 max fixtures vs T09 rules; lead test defect; runner hid failures without `--no-fail-fast` | Fixtures and runner fixed, NC8 |
| P02-D07 | F-1 stray `.prev` temp after rewrite | `atomic.rs` fixed, revert control |
| P02-D08 | Flaky post-release lock assertion (test-only) | Bounded wait; 16 clean runs |

All on attempt 1; none reached the two-cycle limit. Lead-side defects (differential B comparator, control-script mtimes, lead test/fmt/clippy slips) are recorded in the wave/task records.

## 6. Agents (CLAUDE.md model-recording rule)
- **`sonnet-implementer`:** configured `model: claude-sonnet-5-5` in every spawn; actual resolution not observable; no substitution notice. Up to three parallel workers per wave in lead-prepared worktrees (plan §3.8). Self-reports were never used as evidence; the lead re-ran every check on the integrated tree.
- **`reviewer`, `security-reviewer`:** read-only passes after wave 1a, wave 1b, D03, wave 2 and T14.
- All 13 worktrees were archived (`~/.local/share/pwt-dev-tools/_archive/p02-worktrees-2026-10-28.tar.gz`, checksums verified) and removed on 2026-10-28; every worktree HEAD is an ancestor of B4.
- T15 used no worker (lead-owned per plan §5B.3).

## 7. Owner decisions recorded during P02
A1–A10 (recommended); T01-D1–D12; T04 + CA-04; O1–O10, Q-T04-1; E-1, E-2, Q-D03-1, R-D03-1/-5; Q-T12-1..5 (accepted as implemented); Q-T14-1 (keep exit 4 for an over-long path under an accepted dir), Q-T14-2 → FR-32 (P06, P19 at the latest), Q-T14-3 → FR-33 (P03); commits B2–B4 and push of `phase/P02-store` (2026-10-28); T15 approved 2026-10-28, resumed 2026-10-29.

## 8. Known limitations and carry-forward
- **Manual-pending (closed 2026-10-29):** the unprivileged exit-4 test passed (`evidence/P02/manual-exit4/record.md`); the runner still skips that branch inside `unshare -rn`.
- **Fault injection:** ENOSPC/EACCES/interruption are simulated through `FsOps`; rename/directory-sync durability is tested on this Linux host and filesystem only. No crash-proof or multi-file ACID claim (Data §12).
- **Build-code exposure:** 15 build scripts, 3 proc-macros (72-crate host graph), all from accepted built P00 graphs; pattern-level review only, no filesystem sandbox. No advisory scan in P02.
- **Static checks:** regex/parser accident guards, not adversarial or type-aware (S-I7); the FR-31 model parser is textual (L-T13-2).
- **Semantic rules:** 16 partial + 9 producer rules depend on later phases; tested on synthetic records only.
- **Text safety:** escape-set gaps for some default-ignorable code points → FR-32; `.app.lock` uid proxy when run as root on a foreign dir → FR-33. `RedactedText` still proves only that a `Redactor` ran (real redactor P06).
- **Development defaults:** T09 resource caps (timeout ≤ 86 400 s, output ≤ 64 MiB, ≤ 64 scanner processes, ≤ 1 024 active repositories) pending FR-11.
- **Recorded lows, no change:** L-T11-1..4, L-T12-1..3 (`validate`/`show` create `.app.lock`; lock-held scenarios use an in-process holder), L-T13-1..4, L-T14-1..4; `report.max` selected-ref coherence for P07.
- **Info:** `dev/p02_registry.json` carries a `lock_sha256` field with the T02 value `b34cb529…`; the runner does not read it (it pins `6c9d4998…` in `EXPECTED_LOCK_SHA256`). Stale metadata only; correct it on the next lead-owned runner change if wanted.
- **Build cache:** the lead Cargo target dir was rebuilt by T15 (3.0 GB under `_cargo-target/`).
- **Open elsewhere:** O-1–O-6 (P13), Q-01/Q-02/Q-04/Q-05, B-01, B-04, B-08.

## 9. Recommended next steps (owner decisions)
1. Accept P02 with the limitations above.
2. Approve committing T15 (this report, `P02-freeze.json`, `evidence/P02/T15/` minus the aborted `final-check/` or with it labelled, status/handover, plan §5B.3 T15 row) and a non-force push of `phase/P02-store`.
3. Approve merging `phase/P02-store` into `main` (`--no-ff`) and pushing `main`.
4. Run the manual exit-4 check when convenient.
5. Approve P03 planning (credentials, permissions; includes FR-33).

# P02-T13 — FR-31 enforcement: integration record (uncommitted)

Date: 2026-10-28. Lead: Opus. Authority: owner 2026-10-28 "commit B3 locally, no push, and approve t13, spawn the required sonnet subagents … branch sessions do the work, main session for test, orchestration and debug cycle initiation". B3 was committed first (`49b8cba` = B2 + T11 + T12); T13 is integrated on top and **not committed** (no approval for a further commit).

Base: B3 `49b8cba` (local commit; `--final` PASS 805/0/0, 32, cli 34; `evidence/P02/T12/final-check/`).

## 1. Worker

| Item | Value |
|---|---|
| Packet | `planning/P02-packets/T13-W3.md` |
| Agent | `sonnet-implementer`, configured `model: claude-sonnet-5-5`, no harness isolation (packet-confined to the lead-prepared worktree); resolution not observable, no substitution notice; self-report unverified, lead re-ran everything |
| Worktree | `.claude/worktrees/p02-t13-w3`, branch `p02/t13-w3` @ `49b8cba`, no commits; increment unstaged |
| Owned files changed | `domain/src/store/classes.rs` (+663/−2; 664 lines), new `domain/tests/fr31.rs` (1344 lines, 21 tests); no other file, no dependency change |

Worker-reported: no Appendix A / model mismatch (105 rows: E 6, N 66, R 27, S 6; 57 reachable defs; 24 class-N newtypes; 49 store structs parsed); final `all --min-passed 840 --min-compile-fail 39` PASS; worker mutation (class swap `Workflow.name`, type swap `Repository.slug` in the table) failed 5 fr31 tests.

## 2. Lead verification (main tree = B3 + the two files copied byte-identical; hashes in `sha256.txt`)

| Run | Result | Evidence |
|---|---|---|
| `all --slot lead --min-passed 840 --min-compile-fail 39` | **OVERALL PASS** — env, fmt, clippy, build, static PASS; test 39 suites **840/0/0, 39 compile_fail**; cli 34/0/0 | `integrated-check/`, `.stdout` |
| floor control `--min-passed 841` | FAIL on the floor only | `control-841/`, `.stdout` |
| compile_fail floor control `--min-compile-fail 40` | FAIL on the floor only | `control-cf40/`, `.stdout` |
| lead negative control **in a model file** (independent of the worker's table mutations): `Workflow.description` changed from `RedactedText` + `store_read::redacted` to `SanitisedText` | FAIL as intended — `fr31::every_row_matches_the_rust_type_and_helper_of_its_field`, `fr31::every_text_field_of_every_model_has_its_row`, and the pre-existing `store_models_a::every_redacted_text_field_uses_the_store_read_helper_and_matches_the_classification` (596/3). Restored (`git diff` clean), fresh mtime | `nc-lead-description-as-S/`, `.stdout` |
| **`all --final`** (floors 840/39 from `dev/p02_baseline.json`) | **OVERALL PASS** 840/0/0, 39; cli 34 | `final-check/`, `.stdout` |

Arithmetic: 840 = 805 + 21 (`tests/fr31.rs`) + 14 doctests in `classes.rs` (7 compiling controls counted as passed + 7 `compile_fail`; the runner counts compile_fail doctests in both `passed` and `compile_fail_ok`). compile_fail 39 = 32 + 7. Floors raised to **840/39** (uncommitted).

## 3. Lead review (inline; formal reviewer/security-reviewer pass in T14)

Checked against request §5.1, §6.6, §9 row T13, T01-D1/D2 and Appendix A rev. 2: `TABLE` has 105 rows in Appendix A order; `Class` E/N/R/S; module rustdoc states the rule of thumb, the FR-31 (2) trust assumption and the §6.6 limitations; no I/O; the completeness walk includes arrays and string-map values and has negative controls in both directions (synthetic extra property; each of the 105 rows removed in turn); class/type agreement covers all 27 R rows incl. the exact `store_read` helper; seal tests cover every R shape present (plain, nullable, map) and one S shape.

Lead decisions / observations (no code change, no debug cycle):
- D-T13-1 (Appendix coupling): **kept** `the_table_equals_appendix_a_row_for_row`, which `include_str!`s the lead-owned Appendix A. It pins table = approved appendix; a future appendix revision must update both in one lead change.
- L-T13-1: `Row` is a tuple alias rather than a struct because the runner's static "no bare String/str in store-model fields" rule scans every struct in `store/*.rs`; acceptable, documented in the worker report.
- L-T13-2: the model-source parser in `fr31.rs` is textual (self-tested; no generic/tuple/macro-generated store structs exist). A future macro-generated model would need the parser or a const table.
- L-T13-3: `compile_fail` doctests pass for any compile error; each has a compiling control. Control 3b differs in two places (helper path and field type), a minor deviation from "differs only in the forbidden part". Error codes in the attributes are not enforced on stable.
- L-T13-4: no `Vec<RedactedText>` field exists, so `store_read::redacted_vec` stays unused (existing `#[expect(dead_code)]` reason in `text.rs`); `the_redacted_shapes_present_are_the_ones_the_seal_tests_cover` fails when one appears.
- Plan wording: `plan.md` T13 says "sealed `RedactedText` read-back path in `persistence`"; superseded by T01-D1 (read-back is `domain::text::store_read`, `pub(crate)`, already in place). No persistence path was added.

## 4. Not done
T13 not committed (owner decision). T14 (runner static-allowlist extension with negative controls, CLI-level failure fixtures, reviewer + security-reviewer, debug cycles) and T15 (report) not started/approved. No network, no dependency change, no push.

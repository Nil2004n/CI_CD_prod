# P02-T14 — integration record (committed in B4)

Date: 2026-10-28. Lead: Opus. Authority: owner 2026-10-28 "commit T13, accept Q-T12-1..5, approve T14, spawn the required sonnet subagents … branch sessions do the work, main session for test, orchestration and debug cycle initiation". Debug cycles D07, D08 under P02 A9 (pre-authorised; two attempts per defect). B3b `bf6130b` committed before T14 started; T14 + D07 + D08 are **not committed**.

Base: B3b `bf6130b` (`--final` PASS 840/0/0, 39, cli 34).

## 1. Work items

| Item | Who | Files | Result |
|---|---|---|---|
| Failure fixtures and end-to-end failure scenarios | W1 `sonnet-implementer` (packet `planning/P02-packets/T14-W1.md`, worktree `p02-t14-w1`) | `application/src/dev_store.rs` (four `*_with` fault seams made `pub`, no behaviour change), new `application/tests/dev_store_faults.rs` (10 tests), new `cli/tests/p02_failures.rs` (14 tests incl. one helper), `cli/src/features.rs` (+14 rows, 27 total) | 863 passed + 1 ignored = finding **F-1** (§3) |
| Runner static extension (§8; wave 3) | lead | `dev/p02_check.py` | 6 new rules + negative controls; review hardening (§4) |
| Differential B (request §7, FR-16/J16) | lead | new `dev/p02_differential.py` | host `jsonschema` 4.26.0 agrees on **31/31** fixtures |
| Runner git hardening (security L-4) | lead | `dev/p02_check.py` | `git` for `--repo` ignores repo-local fsmonitor/hooks and global/system config |
| D07 — F-1 fix | W1 (packet `D07-W1.md`, worktree `p02-d07-w1`) | `persistence/src/atomic.rs`, `persistence/tests/fs_atomic.rs`, `application/tests/dev_store_faults.rs` | `evidence/debug/P02-D07/record.md` |
| D08 — flaky lock test | W2 (packet `D08-W2.md`, worktree `p02-d08-w2`) | `cli/tests/p02_scenarios.rs` | `evidence/debug/P02-D08/record.md` |
| Reviews | `reviewer`, `security-reviewer` (read-only; configured sonnet; resolution not observable) | — | §4 |

All workers: configured `model: claude-sonnet-5-5`, no harness isolation (packet-confined to lead-prepared worktrees), self-reports unverified; the lead re-ran everything; every increment was copied byte-identical into the main tree (`w1-sha256.txt`; `cmp` for D07/D08).

## 2. Lead verification (main tree = `bf6130b` + T14 + D07 + D08 + lead runner/differential)

| Run | Result | Evidence |
|---|---|---|
| T14 W1 integrated (before D07) | test FAIL only because 1 ignored (F-1): 863/0/1, 39; every other step PASS | `integrated-check-with-f1/` |
| new static rules on planted violations (layer, exit, urandom, store Event, R field helper) | all five caught; source restored (`git diff` clean) | `nc-static-planted/` |
| static after review hardening | PASS, 77 self-test cases | `static-ext3/` |
| differential B (final) | PASS: version 4.26.0, SHA = pin read from `persistence/src/schema.rs`, 31/31 agree, 5 negative controls (extra key, format on, remote/file/relative `$ref` refused by the counted retriever) | `differential-b/` |
| differential B run 1 | FAIL — **comparator defect** (expected the full root `oneOf` to reject a store_type_mismatch fixture, which is a valid document of another root); fixed, kept as evidence | `differential-b-run1-comparator-defect/` |
| D07 integrated | PASS 866/0/0, 39; cli 48 | `evidence/debug/P02-D07/integrated-check/` |
| **`all --final`** after D08 (floors 866/39) | **OVERALL PASS** — 866/0/0, 0 ignored, 39 compile_fail; cli 48/0/0; static PASS | `final-check/` |
| floor control `--min-passed 867` | FAIL on the floor only | `control-867/` |
| `cli` repeated 5× after D08 | 5/5 PASS 48/0 | `cli-repeat-{1..5}/` |

Arithmetic: 866 = 840 + 10 (`application/tests/dev_store_faults.rs`) + 14 (`cli/tests/p02_failures.rs`) + 2 (D07 persistence regressions) = 866. `cli` step 48 = 34 + 14. Floors raised to **866/39** (uncommitted).

## 3. Findings
- **F-1 (confirmed, low; fixed in D07):** `atomic::replace_steps` left a stray `.<name>.prev.tmp-<hex>` after a successful rewrite when `<name>.prev` already linked the target inode (POSIX no-op rename of two links). See D07 record.
- **F-2 (intermittent, test-only; fixed in D08):** `lock_held_by_another_holder_exits_3` post-release assertion raced with sibling-test `fork`→`exec` descriptor inheritance. See D08 record.

## 4. Review adjudication (reviewer: 0 high, 2 medium, 8 low, info; security-reviewer: 0 high, 0 medium, 6 low, 6 info)

| ID | Finding | Decision |
|---|---|---|
| R-M1 / S-L6 | `real_storage_failure_exits_4_when_permissions_are_enforced` returns early in the runner (user namespace maps to root), yet counts as passed | **Explicit limitation: SKIPPED in the runner, manual-pending** (run once as an unprivileged user outside `unshare`). Exit 4 is pinned by the application fault matrix + `application_storage_failures_map_to_exit_4` + `Exit::for_failure` unit test. No code change. |
| R-M2 | no application/CLI-level fault matrix over `restore_previous`; no undeletable-temp case in `recover` | Partly covered at the persistence level (`writer.rs:908` `restore_previous_halts_on_a_storage_error_and_can_be_completed_after_reopening` + 7 restore/quarantine tests). CLI-level gap kept open as low (L-T14-1). |
| R-L3 | F-1 tolerance in the matrices | **Fixed in D07** (strict, zero strays). |
| R-L4 | 4090-byte `--data-dir` → exit 4 (ENAMETOOLONG on `.app.lock`), 5000-byte → exit 5 | **Owner Q-T14-1 (2026-10-28): keep** as implemented (OS refusal of a path under an accepted directory is a storage failure). |
| R-L5 | same-second tests weak (run ID in the stem makes names distinct by construction); CLI half hand-builds fixtures | The collision case (same run → `ReportNameCollision`, first file unchanged) is pinned in T10 `persistence/tests/{writer,report_name}.rs`. No change; scenario names over-claim slightly (L-T14-2). |
| R-L6 | static regexes narrow | **Fixed (lead):** brace `std::{fs,..}` imports and bare `fs::<item>`; `use std::process::{exit/abort}`; `#[expect]` and `cfg_attr(.., allow)` counted as allowances (equality with `SRC_ALLOWS`); self-tests added. Regexes remain accident guards, not adversarial (S-I7). |
| R-L7 | `fr31.rs` textual parser edge cases (string `//`, enum struct variants, `type: [..]`) | Low; no such constructs today; recorded (L-T13-2 extended). |
| R-L8 | `ALL_STEPS` not tied to `WriteStep` | Low; recorded (L-T14-3). |
| R-L9 / S-L5 | differential: one-level `context`, hard-coded SHA, any-exception `$ref` control | **Fixed (lead):** recursive keywords, pin read from Rust, counted-retriever `Unresolvable` for remote/file/relative refs. |
| R-L10 | `StoreError::Invalid` → `PreviousInvalid` text for non-restore callers | Cannot occur today; recorded (L-T14-4). |
| S-L1 | public `*_with` seams | **Mitigated (lead):** static rule — `*_with(` called only in `dev_store.rs` and tests. |
| S-L2 | `sanitise` escape set lacks some default-ignorable code points (U+206A–206F, U+180B–180D, VS1–VS256, U+17B4/5, U+FFFC) | P01 `domain/src/text.rs`; visual disguise only (no terminal control). **Owner Q-T14-2 (2026-10-28): accepted as recommended** → plan §5.2 FR-32 (P06, P19 at the latest). |
| S-L3 | `.app.lock` uid proxy when the CLI runs as root against a foreign directory | Disclosed T07 limitation; **Owner Q-T14-3 (2026-10-28): accepted as recommended** → plan §5.2 FR-33 (P03: refuse euid 0 on non-root-owned data dirs). |
| S-L4 | runner `git` trusted repo-local config | **Fixed (lead).** |
| Info | circular transition assertion in `emit.rs` tests; structural no-echo test; no-op helper `#[test]` counted in floors (+1); duplicated exit-4 mapping; L-T11-1 silent drop pinned only by the exact-set test; S-I8–I11 (redactor dependency, `/dev/urandom` not fstat-checked, 64 MiB memory, S-class sources unchecked) | Recorded, no change. |
| Record | T13 L-T13-4 said `#[expect]`, code has `#[allow]` | **Corrected** in `evidence/P02/T13/record.md`. |

## 5. Owner decisions and commit
Owner 2026-10-28: "commit B4, approve T15, accept Q-T14-1..3 as recommended …". Q-T14-1 keep; Q-T14-2 → FR-32; Q-T14-3 → FR-33. T14 + D07 + D08 committed locally as baseline **B4** (not pushed). Manual-pending: the unprivileged exit-4 test (R-M1).

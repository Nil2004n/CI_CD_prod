# P02-T04 — scaffold, runner, baseline B0

Date: 2026-10-27. Executed by Opus (lead-owned: manifests, lock, shared contracts, runner; no worker spawned).
Authority: owner approval "I approve T04 and CA-04, proceed" (2026-10-27); `planning/P02-T01-request.md` §3–§5, §8 (approved with T01-D1–D12).

**Result: PASS (run 3).** B0 = the commit containing this record.

## 1. What was created or changed
| Area | Change |
|---|---|
| Manifests / lock | `workbench/Cargo.toml` (member `crates/cli`; workspace deps jsonschema =0.60.1 and sha2 =0.11.1, both default-features off; path deps), `persistence`/`application`/`cli` manifests exactly as accepted in T02 (`evidence/P02/T02/manifests/`). `Cargo.lock` = T02 `Cargo.lock.resolved`, sha256 `b34cb529…`; every `--frozen` cargo step accepted it unchanged (protected-file hashes equal before/after each step) |
| `domain` (§5.7, CA-04) | `APP_CONTRACT_VERSION` "1.1"; `Deserialize` added to the 16 fieldless contract enums (wire format unchanged); `AppEvent::event_type()` and `AppEvent::store_message()` (detail text, or a fixed summary through `redact_static` with literals); `from_random_bytes` on every ID type (v4 + variant bits); `text::store_read::{redacted, redacted_opt, redacted_vec}` (`pub(crate)`, re-escaping); `pub mod store` |
| `domain::store` | `mod.rs` (lead): `StoreKind` (store_type, file_name, writable, from_store_type), `SchemaVersion`, `ConstFalse`, ten `*StoreType` markers; `values.rs` frozen error types (`ValueKind` 26, `ValueReason`, `ValueError`); 14 doc-only module stubs with owner/wave |
| `persistence` | `lib.rs`, `error.rs` (`WriteStep`, `StoreError`), `validate.rs` (`Layer`, `Verdict`, `Problems`, `Problem`, `ProblemCode`, `VersionClass`, `SchemaKeyword`, `RuleId`, `DocumentValidator`, caps 100/256), `semantic/mod.rs` (lead); 10 doc-only stubs |
| `application` | `lib.rs`; stubs `emit.rs` (T11), `dev_store.rs` (T12) |
| `cli` (new) | `main.rs` (args/stdin/stdout/stderr handed to `cli::run`; returns `ExitCode`), `lib.rs` (`run` signature frozen, body `todo!` under `#[expect(clippy::todo, reason)]`), stubs `args`, `commands`, `features`, `output` |
| Tests | CA-04 expectation updates: `domain/tests/versions.rs` and `domain/tests/contract.rs` (`contract_version_is_1_0` → `contract_version_is_1_1`) now expect "1.1" — expectation changes required by an approved contract change, recorded here; no assertion removed or weakened |
| Runner | `dev/p02_check.py` (derived from `dev/p01_check.py`, which is unchanged): `cli` mode; per-slot `p02-*` target dirs; registry allowlist `dev/p02_registry.json` (86 entries keyed name+version) + lock sha pin; 8 members; dependency allowlist; P02 static checks 1–5 of request §8 with 22 new self-test cases (51 total); `cli/src/main.rs` may take stdout/stderr handles but no print macros; floors `dev/p02_baseline.json` = P01 floors (416 / 32) |
| Specs | CA-04 applied (`evidence/P02/CA-04/check.md`) |

## 2. Runs (`python3 -I -B dev/p02_check.py all --slot lead --out evidence/P02/T04/<run>`)
| Run | Result | Cause / action |
|---|---|---|
| `check-run1` | FAIL: fmt (exit 1), test (1 failed: `contract_version_is_1_0`) | Lead defects: generated code not rustfmt-formatted; a second 1.0 contract-version expectation missed. Fixed by `cargo fmt --all` (`cargo-fmt-fix.log`) and the CA-04 expectation update. Build scripts and proc-macros from T02 §4 ran here for the first time, inside `unshare -rn` |
| `check-run2` | PASS | — |
| `static-final-control` | `static --final` exit 1 (expected): remaining `todo!` in `cli::run` | control |
| `check-run3` | **PASS**: fmt; clippy `-D warnings` 0 warnings; test 23 suites **416 passed / 0 failed / 0 ignored**, 32 compile_fail; build; cli 2 suites, 0 tests (none yet); static all checks OK, self-test 51/51; lock sha `b34cb529…` | after switching `text::store_read` from `#[expect(dead_code)]` to `#[allow(dead_code, reason)]` (temporary, listed in the runner's `SRC_ALLOWS` until B1) so that wave-1b workers using the helpers do not hit an unfulfilled-expectation error |

## 3. Deviations from the frozen request (lead decisions, persistence-internal; no contract meaning changed)
- **D-T04-1 (request §5.5):** `write_replace`/`write_new` take a W3-owned `persistence::paths::StorePath` (built only from `StoreKind` file names or report stems) instead of `domain::store::values::AppRelPath`, so W3 (wave 1a) does not depend on W1's `values.rs` in the same wave. Narrower than the original.
- **D-T04-2 (request §5.5):** the `File` needed for `File::try_lock` lives in `fs.rs` as a `LockHandle` returned by a `RealFs` method; `lock.rs` holds the handle (keeps `std::fs` confined to `fs.rs`, static check 4).
- **D-T04-3:** `AppEvent::event_type()`/`store_message()` live in `domain::contract` (allowlisted `redact_static` caller) rather than in `application`, because static summaries need `redact_static` and P02 forbids `Redactor` impls in src.

## 4. Open item for the owner (found in T04, not acted on)
- **Q-T04-1 (envelope key collision):** `AppEvent::RunStarted { run_id, at }` already has an `at` field. The CA-04 envelope, serialised flattened "beside `event_type`", would emit `at` twice for that event. The envelope is wave 3 (T11), so nothing built so far is affected. Options (recommended first): (a) rename `RunStarted.at` → `started_at` within app contract 1.1 (no 1.1 consumer exists yet; golden test updated); (b) serialise the envelope nested (`{"event_id","at","level","event":{…}}`), amending the CA-04 text; (c) drop `RunStarted.at` (the envelope `at` is the emission time, which can differ from the run start).

## 5. Limitations
Build scripts run without a filesystem sandbox (as P00/P01). `cli` mode has no scenarios yet (T12). P02 static checks are regex/structure heuristics with negative controls, not a parser.

# P01-T09 — integration, verification and final reviews

Date: 2026-10-26.
- **Authority:** owner approval of T08/B2 and T09 (2026-10-26).
- **Executed by:** Opus (lead). Read-only `reviewer` and `security-reviewer`. Debug cycle P01-D04 (two `sonnet-implementer` instances).
- **Result: PASS.** The final integrated run is `check-run3` (the commit containing this record = the verified tree plus records only; see §5).

## 1. Steps
1. Removed the redundant `dead_code` allow on `redact_static` (lead).
2. `check-run1` on B2 plus that change: **PASS**, 379/0/0, 17 compile_fail.
3. Final reviews over `2667a31..HEAD` (both reports summarised in §3).
4. Lead fixes to the runner and docs, with negative controls (§2). Checkpoint `26157b4` committed and pushed; `check-run2` on the clean committed tree: **PASS**, 379/17.
5. P01-D04a/b (`evidence/debug/P01-D04/record.md`) were verified, merged, and the worktrees were removed after preservation checks.
6. Lead doc fix in the `contract.rs` module header (free text in events is `RedactedText` only). The baseline floors were raised to 415/32 (`dev/p01_baseline.json`). `check-run3` with `--final`: **PASS**.

## 2. Lead runner hardening (from the final reviews), with controls
- **Committed `--final` floors.** `dev/p01_baseline.json` holds the floors (rev M2); `--final` always applies them.
- **New static checks:**
  - trait impls and derives of `RedactedText` limited to {Display, Serialize} + {Clone, Debug, Eq, Hash, PartialEq} (rev M1);
  - `redact_static` call sites only in allowlisted files, with a literal or ALL_CAPS const argument (sec L1, rev L7);
  - no `Box::leak` in src (sec L1);
  - `expose` matched at word level and exempt by exact path (sec L3);
  - multi-line `impl Redactor` and `Redactor as` aliases detected (sec L2);
  - `#[allow]` in src limited to an approved list (sec L4);
  - no `build.rs`, proc-macro crate, `.cargo/config*` or `rust-toolchain*` in the workspace or its ancestors (sec L6).
- **Pinning and hashing:**
  - `clippy.toml`/`rustfmt.toml` hashed with the manifests (sec L6);
  - whole-lock SHA-256 pinned to the T02 lock (sec I2);
  - `SanitisedText` removed from the event allowlist (sec I1).
- **Self-test:** 25/25.
- **Negative controls:**
  - `negative-controls-static.json`: a scratch copy with injected `expose` misuse, a wrapped `impl Redactor`, `redact_static(leaked)`, `Box::leak`, an unapproved `#[allow]`, `build.rs` and `.cargo/config`. Every one was caught.
  - `negative-control-redactedtext-impl.json`: an injected `impl From<SanitisedText> for RedactedText` was caught.

## 3. Final review dispositions
| Finding | Disposition |
|---|---|
| rev M1 route-by-route guards | Fixed: static impl-set check + D04a/D04b guard pairs |
| rev M2 floors optional | Fixed: committed baseline floors under `--final` |
| rev L1 over-escaping sampled | Fixed: D04a exhaustive scalar test |
| rev L2 versions untested | Fixed: D04b `tests/versions.rs` |
| rev L3 superseded criterion wording | Fixed: `plan.md`, `planning/P01-execution-plan.md` annotated |
| rev L4 evidence on a dirty tree | Fixed: checkpoint commit `26157b4`, then re-runs |
| rev L5 `ChatFailure::ProviderOutage` not in Arch §6 (it comes from UI Schema §15) | **Owner acceptance needed** (P01 report) |
| rev L6 events don't constrain state values (e.g. `RunFinished` with a non-terminal state) | Limitation: validated at coordinator emission (P02+) |
| rev L7 `redact_static` discipline | Fixed: static call-site check |
| rev L8 private intra-doc link | Fixed: D04b |
| rev L9 golden JSON ≠ store `Event` record; key order not pinned | Limitation: envelope/mapping is P02 (owner decision 2) |
| rev L10 stale T04 note | Annotated as superseded |
| sec L1–L3, L6, I1, I2 | Fixed (§2) |
| sec L4 lints are `warn` + `-D warnings`; B2 wording "no allow anywhere" | `#[allow]` allowlist added. The B2 wording is corrected here: the only src allow is `contract.rs` `result_large_err`; test files carry file- or helper-level unwrap/expect/panic allows. **Recommendation for the owner:** set those workspace lints to `deny` and add `clippy::allow_attributes_without_reason` (changes approved T01 §6.1, so not applied) |
| sec L5 weak controls, unguarded traits | Fixed: D04b (and the D04a follow-up for the D02 pair) |
| sec I3/I4, rev §9 | Recorded owner decisions and later-phase items (P02, P03, P06, P17) |

## 4. Acceptance criteria (`planning/P01-execution-plan.md` §6; criterion 3 superseded by T01 §5.5, criterion 4 amended by T01-D3)
| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | Layout per Tech §4; toolchain pin verified; offline locked build | **Met, with recorded deviations** | 7 crates; `tests/{fixtures,contracts,integration}` deferred (T01 §1); pin = runner version gate (A5 = 5a), so a plain `cargo` call is not pinned; all Cargo `--frozen` in netns |
| 2 | fmt, clippy `-D warnings`, test, build exit 0, with counts | **Met** | `check-run3`: 0/0/0/0; 415 passed / 0 failed / 0 ignored |
| 3 | Every Arch §6 pilot command/event present; reserved → `OperationFailed{deferred}` | **Met** | 23+6 commands, 32+2 events; exhaustive matches; `tests/contract.rs` literal tables. Addition `ChatFailure::ProviderOutage` awaits owner acceptance |
| 4 | Golden serialisation per event; round-trip per type with a read path; commands without serde | **Met** | 34 golden cases; round-trips for ids, time, vocab, states, `SanitisedText`; compile_fail guards |
| 5 | Raw text cannot reach safe-text fields; events carry no `String` free text | **Met** | 32 compile_fail guards (one-line pairs); static field-type allowlist (events: `RedactedText` only); `RedactedText` impl-set check; negative controls |
| 6 | State-machine tests cover all allowed and reject all disallowed transitions | **Met** | 81/81/36 pairs against independent literal tables; schema cross-check |
| 7 | Dependencies exactly serde (derive) + serde_json at approved versions | **Met** | static lock (version, source, checksum) + whole-lock hash `fb8b7d07…`; manifests unchanged since B0 |
| 8 | Lockfile committed; advisory status recorded | **Met (A4 = 4a)** | `workbench/Cargo.lock` committed; **cargo-audit not run, install deferred by owner**; no fresh RustSec screen since P00 (2026-10-24) |
| 9 | Reviewer/security-reviewer findings resolved or recorded | **Met** | B1 §4, this §3; open owner items listed in §6 |

## 5. Final run
`python3 -I -B dev/p01_check.py all --slot lead --final --out evidence/P01/T09/check-run3`, run on `c9fc3c6` plus the `contract.rs` module-doc fix, the baseline-floor update and untracked evidence (`summary.json` `dirty`).
- Result: env (1.97.1, netns errno 101), fmt 0, clippy 0 (0 warnings), test 0 (20 suites, **415 passed, 0 failed, 0 ignored, 32 compile_fail**; floors 415/32), build 0, static 17/17 checks (self-test 25/25).
- The commit containing this record adds only records and evidence on top of that tree.

## 6. Open items for the owner (carried to the P01 report)
1. Accept or remove `ChatFailure::ProviderOutage` (UI Schema §15) as an addition to Arch §6.
2. Optional lint tightening: `deny` levels and `allow_attributes_without_reason` (changes T01 §6.1).
3. Previously decided, now carried as open future work: `SaveWorkflow` fields (P14); event envelope (P02); `RedactedText` concatenation (P17); escape-set observations (D5 unchanged).
4. cargo-audit request (`planning/P01-T03-cargo-audit-request.md`): recommended before P02.

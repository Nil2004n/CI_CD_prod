# P01 report — workspace, domain types and contracts

Date: 2026-10-26 (conformance section §10 added the same day at owner request).
- **Phase branch:** `phase/P01-foundation`; base `main` `485b3a8`; the planning commit is `2667a31`.
- **Prepared by:** Opus (lead). No Haiku documentation agent is installed.
- **Status:** **complete and verified by the lead; awaiting owner acceptance.** Merging to `main` is a separate approval.

This report states what was done and what was verified. It is not a claim that the code is secure or production-ready.

## 1. Scope delivered (`planning/P01-execution-plan.md` §1)
- **Cargo workspace `workbench/`:** 7 crates per Tech §4 (`domain` holds content; 6 empty libraries).
  - Settings: edition 2024, resolver 3.
  - Dependencies: `serde` `=1.0.229` (std, derive) and `serde_json` `=1.0.154` (dev-only).
  - Lock `fb8b7d07…`: 11 registry crates, each byte-equal to its accepted P00 lock entry.
- **`domain` modules:**
  - `ids`: 17 typed UUID IDs (canonical lowercase form; nil rejected; errors never echo input).
  - `time`: `UtcTimestamp` (RFC 3339 with `Z` only, years 0001–9999, no dependency).
  - `text`: `SanitisedText`/`RedactedText` (closed construction, frozen escape set, `Redactor` trait, crate-private `redact_static`).
  - `vocab`: schema enums.
  - `states`: run, check and approval state machines (Arch §7 plus D1/D2).
  - `contract`: app contract v1.0 — 29 commands (23 pilot, 6 reserved), 34 events (32 pilot, 2 reserved), delivery classes, `dispatch_guard`.
  - `versions`.
- **Dev tooling:** `dev/p01_check.py`, the verified runner (offline, netns, `--frozen`, version gate, static allowlist checks, self-test, committed floors in `dev/p01_baseline.json`).

**Not in P01** (per plan): persistence, permissions/credentials, Git, process supervision, TUI, network, scanners, real API, UUID generation.

## 2. Tasks
| Task | Result | Record |
|---|---|---|
| T01 request | Approved with D1–D6 | `planning/P01-T01-request.md` |
| T02 offline resolution + exposure | PASS. Plan fact corrected (serde_derive uses syn 3.0.9) | `evidence/P01/T02/record.md` |
| T03 cargo-audit request | Documents only; install deferred (A4 = 4a) | `planning/P01-T03-cargo-audit-request.md` |
| T04 scaffold, B0 `8a1bec3` | PASS (run 1 fmt fail in a lead stub, fixed) | `evidence/P01/T04/record.md` |
| T05–T07 wave 1, B1 `d95090c` | PASS after lead runner fix; D01, D02 | `evidence/P01/B1/record.md` |
| T08 contract, B2 `829dad3` | PASS; D03 (report count) | `evidence/P01/B2/record.md` |
| T09 verification + final reviews | PASS; D04; acceptance criteria assessed | `evidence/P01/T09/record.md` |
| T10 lint tightening, report, freeze | PASS; D05 | this report, `evidence/debug/P01-D05/record.md`, `evidence/P01/P01-freeze.json` |

## 3. Final verification (lead; `evidence/P01/T10/check-final`)
- **Command:** `python3 -I -B dev/p01_check.py all --slot lead --final --out evidence/P01/T10/check-final`.
- **Tree:** HEAD `cf54227`, plus this T10 documentation: the Arch §6 line and untracked evidence, with no code difference.
- **Environment:** rustc/cargo/rustdoc 1.97.1 called directly, `env -i`, isolated `CARGO_HOME`, per-repo target dir, every Cargo command in `unshare -rn` (probe errno 101), `--frozen`.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --frozen -- -D warnings` (lints now `deny` + `allow_attributes_without_reason`) | exit 0, 0 warnings |
| `cargo test --workspace --frozen` | exit 0. 20 suites: **415 passed, 0 failed, 0 ignored**. Unit 3, contract 22, ids_time 279, states 23, text 19, versions 5, doctests 64 (32 run + **32 compile_fail**). Floors 415/32 from the committed baseline |
| `cargo build --workspace --frozen` | exit 0 |
| static (17 checks) | PASS. Checks include: no `unsafe`; no print macros; no `todo!`; contract field-type allowlist; `RedactedText` impl set; `redact_static` call sites; `expose` sites; `#[allow]` allowlist; no `build.rs`/`.cargo` config; lock (version, source, checksum) and whole-lock hash; manifests ⊆ {serde, serde_json}; self-test 25/25 |
| Lockfile and manifests during every step | unchanged (hash-checked) |
| cargo-audit | **not run.** Install deferred by the owner (A4 = 4a); the last RustSec screen is from P00, 2026-10-24 |

**Negative controls** (the checks can fail): `evidence/P01/T09/negative-controls-static.json` and `negative-control-redactedtext-impl.json`. **Manual-pending checks:** none for P01; there is no UI or runtime behaviour.

## 4. Acceptance criteria
Criterion 3 is superseded by T01 §5.5; criterion 4 is amended by T01-D3. The detailed table is in `evidence/P01/T09/record.md` §4, and the current state follows.

| # | Status |
|---|---|
| 1 Layout, pin, offline locked build | Met, with recorded deviations: test-fixture dirs deferred; the pin is enforced by the runner, not by `rust-toolchain.toml` (A5 = 5a) |
| 2 fmt/clippy/test/build exit 0 with counts | Met (§3) |
| 3 All Arch §6 commands/events; reserved → `OperationFailed{deferred}` | Met |
| 4 Golden serialisation per event; round-trip per readable type; commands without serde | Met |
| 5 Raw text cannot reach safe-text fields; no `String` free text in events | Met (32 compile_fail guards, static allowlist, impl-set check) |
| 6 State-machine transitions exhaustive | Met |
| 7 Dependencies exactly as approved | Met |
| 8 Lockfile committed; advisory status recorded | Met under A4 (cargo-audit not run) |
| 9 Review findings resolved or recorded | Met |

## 5. Debug cycles and anomalies
| Cycle | What | Outcome |
|---|---|---|
| P01-D01 | Lead-suspected doctest heading shift (W2) | No defect: lead misdiagnosis |
| P01-D02 | No static constructor; a vacuous-risk doctest control (W2) | `redact_static`, control fixed |
| P01-D03 | T08 report claimed 33 tests (22 exist) | Report correction only |
| P01-D04 | Final-review guards (D04a text, D04b contract) | 15 guard pairs, exhaustive escape test, versions tests |
| P01-D05 | Allows without reason after the lint tightening | Reasons added |

**Lead-side anomalies, fixed and recorded in the B1/T09 records:**
- shared Cargo target dir linked a stale build;
- a zsh word-splitting error;
- the static checker flagged `#[cfg(test)]` code;
- regex bypasses in the checker;
- the version gate ran only in some modes;
- `result_large_err` on the frozen `dispatch_guard` signature (a local allow with reason).

No defect reached the two-cycle limit.

## 6. Agents (CLAUDE.md model-recording rule)
- **`sonnet-implementer`:** configured `model: claude-sonnet-5-5` in every spawn. The model that actually served them was not observable, and there was no substitution notice.
  - Workers: W1 (T05), W2 (T06, D01, D02), W3 (T07), W1′ (T08, D03), D04a, D04b, D05.
  - Self-reports were not used as evidence; two were inaccurate (D03, D05 doctest wording).
- **`reviewer` and `security-reviewer`:** read-only, two passes each (B1, final).
- All worker worktrees and branches were removed after preservation checks. Every worker commit is in the phase history.

## 7. Owner decisions recorded during P01
- A1–A7 (recommended set); T01-D1–D6; the CLAUDE.md worker-limit amendment.
- B1 decisions:
  1. `SaveWorkflow` provisional until P14;
  2. event envelope at emission (P02);
  3. `RedactedText` concatenation decided in P17;
  4. escape set D5 unchanged.
- T09: `ChatFailure::ProviderOutage` accepted; recorded in Arch §6 (`03_Architecture.md`).
- T09/T10: lint tightening applied.
- cargo-audit "before P02" was approved as a recommendation. Each request stage (C-A … C-E) still needs explicit approval because it involves network access and installs.

## 8. Known limitations and carry-forward
- **No runtime behaviour yet:**
  - nothing calls `dispatch_guard` or emits events;
  - state values in events are not constrained (e.g. `RunFinished` with a non-terminal state); emission-time validation is P02+.
  - `AppEvent` JSON is not the store `Event` record; the envelope and mapping are P02. Golden tests compare JSON values, so key order is not pinned.
- **Redaction:**
  - `RedactedText` is a type-level marker: it proves a `Redactor` ran, not that secrets were removed. A real redactor is P06, and it must normalise invisible characters before matching.
  - `redact_static` safety relies on the literal-only rule, which the static check enforces for current call sites.
- **Escaping:** the set has known gaps (Unicode tags, U+206A–206F, U+00AD, …; owner decision 4).
  - LF is kept, so single-line renderers must handle it.
  - `<`/`>` are not HTML-escaped.
  - Literal `<U+XXXX>` input is indistinguishable from escaped text: hash pre-escape values, and never unescape.
- **Credentials:** `CredentialSecret` zeroing is best effort, not secure erase. `RawInput` is not zeroed and shows its length. KEY-01 hardening is P03.
- **Build-code exposure:** 6 build scripts and 1 proc-macro, with no filesystem sandbox. No advisory scan in P01.
- **Static checks:** pattern- and parser-based, not a Rust type checker. Rejected forms are listed in the records.
- **FR-01–FR-30** remain assigned to their phases. None was in P01 scope, and the P01 types do not block them.

## 9. Recommended next steps (owner decisions)
1. Accept P01 (with the limitations above) and approve merging `phase/P01-foundation` into `main` (`--no-ff`) and pushing `main`.
2. Approve cargo-audit stage C-A (single-crate fetch and lockfile inspection) before P02 starts.
3. Approve P02 planning (JSON store v1.3), including the event-envelope design (decision 2).

## 10. Conformance with plan, architecture and PRD (owner-requested check, 2026-10-26)
Full record: `evidence/P01/conformance/record.md`.
- **Fresh `--final` run:** PASS, 415/0/0, 32 compile_fail.
- **Scripted document cross-check:** **25 PASS, 2 NOTE.** The script parses Arch §4/§6/§7, Tech §4, the schema enums, UI §5 and PRD §16 directly.
  - Commands and events equal Arch §6: 29 and 34 names, with matching status and delivery.
  - State machines equal Arch §7 plus T01-D1/D2.
  - Schema enums match.
  - Reserved commands map to PRD-deferred capabilities.
  - The KEY-01 shape holds.
- **Process:**
  - plan §3.8 policy followed;
  - CLAUDE.md invariants hold (no secrets in the P01 diff, no network or installs, approved commits only, worker reports re-verified);
  - no spec changed except the owner-accepted Arch §6 line;
  - 0 unpushed commits.

**Limitations: expected and accepted for this phase.** Each one falls inside a later phase's planned scope (P02 envelope/emission/store, P03 credentials, P06 redaction, P11/P12 renderer escaping, P17 chat) or rests on an explicit owner decision (A3, A4, A5, D5 and the B1 decisions). None leaves an approved P01 requirement unmet.

**Two items that are not P01 limitations, raised for the owner:**
1. **Specification inconsistency.** The store `ToolManifest.availability` (`ready, missing, incompatible, deferred, unconfigured`) differs from UI §5 `tools.states` (`ready, missing, incompatible, unavailable, denied, deferred`). The code follows the UI list. A mapping decision is needed before P06.
2. **Carry-forward to P02.** The store models must use `RedactedText`/`SanitisedText` for all free-text fields (plan §4 P01 intent), and the P02 static checks should extend the type allowlist to them.

# P01 — workspace, domain types and contracts: execution plan (proposal)

Status: **planning only**, prepared by Opus 2026-10-24 at owner request ("start P01 planning, plan p01 into multiple tasks just like p00"). **Nothing in this document is approved for execution.** P01 execution, the phase branch, crate resolution/fetch, any install, worker spawning, commits and pushes each need the approvals listed in §7.

## 1. Scope (plan §4, P01 row)
- A Cargo workspace laid out per Tech §4.
- A pinned toolchain.
- Typed IDs and states (Arch §7 state machines).
- The canonical app command/event DTOs (Arch §6, app contract v1.0).
- `RedactedText`/`SanitisedText` newtypes, so that unsanitised text cannot reach persistence types.

**Phase checks:**
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`;
- `cargo build --workspace`;
- DTO round-trip tests;
- cargo-audit if installed (Tech §22).

**Not in P01:**
- store models and persistence (P02);
- permission evaluator and credentials (P03);
- Git operations (P04);
- the process supervisor (P06);
- the TUI (P15a);
- any runtime network, scanner execution or real API.

**Inputs from P00 (binding):**
- `spikes/p00-freeze/` is an inventory and grants **no build or adoption permission**.
- FR-01–FR-30 stay with their phases. None is assigned to P01, but the P01 types must not block them, for example FR-04 cleanup records and FR-29 containment.
- CA-01/CA-02: app contract v1.0, store 1.3, wrapper protocol 1.1.
- D-01 (Rust, Serde; Ratatui/Crossterm/Tokio added only in the phases that need them).
- B-02 is open for P01 crates. R-4: cargo-audit is due in P01.

## 2. Facts established for this plan (read-only checks, 2026-10-24)
| Fact | Basis |
|---|---|
| Toolchain `stable-x86_64-unknown-linux-gnu` = rustc 1.97.1, cargo 1.97.1; `rustfmt`, `cargo-fmt`, `clippy-driver`, `cargo-clippy` present | `ls` of the toolchain bin dir; `--version` |
| `serde`/`serde_core`/`serde_derive` 1.0.229, `syn` ~~2.0.119~~ **3.0.9** (corrected 2026-10-26 by P01-T02: `serde_derive 1.0.229` depends on syn 3.0.9 in the P00 lock; `evidence/P01/T02/record.md` §5), `proc-macro2` 1.0.107, `quote` 1.0.47, `unicode-ident` 1.0.27, `itoa` 1.0.18, `memchr` 2.8.3 are in the P00 spike lockfile (checksums verified at P00 fetches); `serde_json` 1.0.154 accepted-qualified in T06 | `spikes/rust/Cargo.lock`; `spikes/p00-freeze/crates.json` |
| Worker agents (`sonnet-implementer`, `implementer`) have Read/Grep/Glob/Edit/Write/Bash — **no messaging tool**; read-only `reviewer`/`security-reviewer` have Read/Grep/Glob | `.claude/agents/*.md` |
| The lead has `SendMessage` to subagents; worktree isolation works, but the harness creates worktrees **from the default branch**, so each packet's step 0 must fast-forward to the phase baseline | P00-T10/T11 records |

## 3. Task breakdown
| Task | Content | Owner | Network / installs | Depends on | Est. h |
|---|---|---|---|---|---|
| **P01-T01** | P01 request: workspace location and layout, toolchain-pin method, lint policy, frozen module interfaces (§5), test plan, approvals | Lead | none | — | 1–1.5 |
| **P01-T02** | Dependency request and resolution: `serde` (derive) 1.0.229 and `serde_json` 1.0.154 only. Options (owner choice): **(a) offline** resolution from the existing isolated `CARGO_HOME` cache with `--offline --locked` and checksum comparison against the P00 lock entries; **(b)** online lock-preserving resolve + RustSec screen + locked fetch (as P00-T06–T08). Build-code exposure report (`serde_derive` proc-macro, `syn`, `proc-macro2`, `quote`) | Lead | (a) none; (b) crates.io + RustSec | T01 | 1–2 |
| **P01-T03** | cargo-audit (R-4): installation request — version 0.22.2 from Request C, from source, `--locked`, isolated root — and an **offline advisory-DB plan** (cargo-audit fetches advisory-db from GitHub at run time) | Lead | install + GitHub fetch need approval | T01 | 1–2 (may stay deferred) |
| **P01-T04** | Workspace scaffold (baseline commit): `workbench/Cargo.toml` (workspace, `[workspace.lints]`, edition), `rust-toolchain.toml` or direct-binary pin per T01, 7 crate skeletons per Tech §4 (`domain`, `application`, `repositories`, `scanning`, `retrieval`, `persistence`, `terminal-ui`; only `domain` gets content in P01; the others are empty libraries with no dependencies), **frozen interface stubs** in `domain` (§5), `Cargo.lock`, local check script; verify the canonical commands once | Lead (owns all manifests and the lockfile) | none (offline build in netns) | T02 | 1–1.5 |
| **P01-T05** | Typed IDs and timestamps: ID newtypes with validated constructors and serde; RFC 3339 UTC timestamp newtype without new dependencies; property and edge tests | **W1** | none | T04 | 1–1.5 |
| **P01-T06** | Text-safety newtypes: `RedactedText`, `SanitisedText`; construction only via `domain::text::{redact, sanitise}` style functions; no `From<String>`/`Deserialize` into the safe types from raw input; control/bidi escaping; `compile_fail` doctests (rustdoc, no `trybuild` dependency) proving raw `String` cannot be passed where a safe type is required | **W2** | none | T04 | 1–1.5 |
| **P01-T07** | State machines (Arch §7): run execution, check outcome, approval; enums plus transition functions returning typed errors; exhaustive transition-table tests | **W3** | none | T04 | 1–1.5 |
| **P01-T08** | App contract v1.0 DTOs (Arch §6): every pilot command and event; reserved/deferred names returning `OperationNotAvailable`; delivery class (must-deliver / latest-value / coalescible); serde representation; round-trip tests; events carry only safe text types (no raw secrets, no raw child output) | W1 (wave 2) | none | T05, T06, T07 integrated | 1.5–2 |
| **P01-T09** | Integration and verification: merge worker commits into the phase branch; fmt/clippy `-D warnings`/test/build; cargo-audit if T03 installed it; read-only `reviewer` + `security-reviewer` pass over the text newtypes and DTOs; evidence `evidence/P01/` | Lead | none | T05–T08 | 1–1.5 |
| **P01-T10** | P01 report, freeze update (workspace crates and lock), status/handover; stop for acceptance | Lead | none | T09 | 0.5–1 |

**Estimate:** about 10–15 h hands-on, against plan §7.1's 6–10 h. The difference comes from the request, dependency and cargo-audit tasks, which §7.1 counted as approvals rather than work. Confidence is low. Rate-limit and approval waiting are not counted.

## 4. Bounded parallel execution (owner policy from P01 onward, recorded in plan §3.8)
```
T01 ─► T02 ─► T04 (baseline commit B0 on phase/P01-foundation)
 └──► T03 (independent; may stay deferred)
                 ├─► W1: T05 ids/time      ┐
                 ├─► W2: T06 text newtypes ├─ wave 1 (parallel, disjoint files)
                 └─► W3: T07 states        ┘
                         ▼ lead integrates (B1)
                 └─► W1: T08 DTOs (wave 2; needs T05–T07)
                         ▼ lead integrates (B2) ─► T09 ─► T10 ─► STOP
```
- **Why three workers only in wave 1:**
  - T05, T06 and T07 depend only on the frozen interface stubs in B0 and touch disjoint files.
  - T08 consumes all three, so running it in parallel would mean coding against stubs that panic. It runs alone in wave 2.
  - Everything else is lead-owned: shared manifests, lockfile, phase documentation and integration.
- **File ownership (wave 1):**

  | Worker | Owns |
  |---|---|
  | W1 | `workbench/crates/domain/src/ids.rs`, `.../src/time.rs`, `workbench/crates/domain/tests/ids_time.rs` |
  | W2 | `.../src/text.rs`, `workbench/crates/domain/tests/text.rs` |
  | W3 | `.../src/states.rs`, `workbench/crates/domain/tests/states.rs` |

  `domain/src/lib.rs`, every `Cargo.toml` and `Cargo.lock` are lead-only; workers must not touch them.
- **Worktrees and branches:**
  - Each worker gets its own worktree and branch `p01/w<N>-<task>`. Step 0 is `git merge --ff-only <B0>`.
  - Workers make **local checkpoint commits on their own branch only if the owner authorises it** (A6). They report the hashes; they never push, merge or touch other branches.
  - The lead reviews each commit and integrates it into `phase/P01-foundation` (merge, or cherry-pick on conflict).
- **Communication:**
  - The workers have no messaging tool. They use a lead-monitored, append-only board file with `flock`, and interface questions go to the lead. The lead relays and answers with `SendMessage` to running workers.
  - Messages are never reported as delivered unless the tool confirms delivery.
  - Shared-contract or scope changes need lead approval; owner-level changes need the owner.
- **Worker rules:** stay in scope and owned files; run `cargo fmt`/`clippy`/`test -p domain` offline through the lead-provided check script; report commit hashes, commands with exit codes, limitations and remaining work; leave an honest recovery note if interrupted.
- **Interruptions:** on a usage-limit stop, preserve partial state and stop. No automatic continuation, no repeated repair loops, no new dependencies or permissions without approval. Finished workers are shut down; worktrees are removed only after preservation checks.

## 5. Frozen interfaces in the B0 baseline (lead-authored stubs; workers implement bodies only)
| Module | Frozen surface (names and signatures; exact types fixed in T01) |
|---|---|
| `domain::ids` | `RepositoryId`, `WorkflowId`, `RunId`, `TaskId`, `CheckId`, `FindingId`, `ApprovalId`, `ReportId`; each `fn parse(&str) -> Result<Self, IdError>`, `fn as_str(&self) -> &str`, serde as a validated string |
| `domain::time` | `UtcTimestamp` with `parse_rfc3339`, `to_rfc3339`; serde as an RFC 3339 `Z` string |
| `domain::text` | `RedactedText`, `SanitisedText`; `fn sanitise(raw: &str) -> SanitisedText`; `fn redact(raw: &str, redactor: &dyn Redactor) -> RedactedText`; trait `Redactor`; `as_str`; `Serialize` only (no `Deserialize` from raw input; any read-back path is decided in P02) |
| `domain::states` | `RunState`, `CheckOutcome`, `ApprovalState`; `fn transition(self, ev) -> Result<Self, TransitionError>` per machine |
| `domain::contract` (wave 2) | `AppCommand`, `AppEvent`, `DeliveryClass`, `ContractStatus { Pilot, Reserved }`, `APP_CONTRACT_VERSION = "1.0"` |

## 6. Acceptance criteria (proposed)
1. Workspace layout matches Tech §4. The toolchain pin is verified, and the build is offline and locked in the isolated `CARGO_HOME`.
2. fmt, clippy `-D warnings`, test and build all exit 0, with recorded counts.
3. Every Arch §6 pilot command and event is present, and every reserved name returns `OperationNotAvailable`.
4. The round-trip test covers each DTO variant.
5. `compile_fail` doctests prove raw text cannot reach safe-text-typed fields. Events carry no `String` payload fields for free text.
6. State-machine tests cover every allowed transition and reject every disallowed one.
7. Dependencies are exactly `serde` (derive) and `serde_json` at the approved versions.
8. The lockfile is committed. Advisory status is recorded: cargo-audit result if installed, otherwise "not run" with the reason.
9. The reviewer and security-reviewer findings are resolved or recorded.

## 7. Approvals needed (none granted yet)
| # | Approval |
|---|---|
| A1 | P01 phase scope; create `phase/P01-foundation` from `main` `485b3a8` |
| A2 | Crate scope `serde` (derive) 1.0.229 + `serde_json` 1.0.154, and the resolution option (a) offline from the cache, or (b) online with a RustSec screen and locked fetch |
| A3 | Acceptance of the build-time code exposure (`serde_derive` proc-macro and its `syn`/`quote`/`proc-macro2` chain) for an offline build in the network namespace |
| A4 | cargo-audit: install now (from source, network and build scripts) plus an advisory-DB fetch, or keep deferred (then criterion 8 records "not run") |
| A5 | Toolchain-pin method: a `rust-toolchain.toml` naming `1.97.1` (the rustup proxy may try to install a toolchain by that name; network) versus the P00 direct-binary invocation plus a version-check script (no rustup) |
| A6 | Worker model: up to 3 `sonnet-implementer` workers in worktrees per §4, with local checkpoint commits on their own branches |
| A7 | Phase commit and push of `phase/P01-foundation` (merge to `main` separate) |

## 8. Risks
- Proc-macro build exposure. There is no filesystem sandbox, the same limitation as in P00.
- Stub-driven parallelism could hide interface gaps. Mitigation: wave-1 tests compile against the frozen signatures, and the lead integrates before wave 2.
- Worktree base drift (harness default branch). Mitigation: step-0 fast-forward and a HEAD check.
- The schedule (B-08) remains unresolved.

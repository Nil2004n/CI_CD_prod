# P01-T01 — P01 request: layout, toolchain pin, lints, frozen interfaces, test plan

Status: **prepared by Opus 2026-10-26; awaiting owner approval before P01-T02.** This is a documentation deliverable: no crate was resolved, fetched or built, no worker was spawned, and nothing was committed. Its base is `phase/P01-foundation` at `2667a31` (= `origin`), with a clean tree at the start.

Authority: the owner approved the recommended set A1–A7 on 2026-10-26 (1a, 2a with `zmij` named, 3a, 4a, 5a, 6a with the CLAUDE.md amendment, 7a). In the same message the owner pre-authorised debug cycles for anomalies or test failures found by the lead after a worker task, and asked for a stop after T01. §9 states the bounds.

Facts were checked read-only in this session:
- the toolchain directory holds `cargo`, `rustc`, `rustdoc`, `rustfmt`, `cargo-fmt`, `cargo-clippy` and `clippy-driver`, and rustup has only the `stable` toolchain;
- the isolated cache has every crate listed in §4 (`zmij-1.0.23.crate` included);
- `shellcheck` and `bats` are **not installed**;
- the schema has 17 UUID-typed ID fields (§5.1).

## 1. Location and layout (Tech §4)

```text
workbench/
  Cargo.toml            # [workspace], [workspace.package], [workspace.dependencies], [workspace.lints]
  Cargo.lock            # committed; lead-owned
  clippy.toml           # allow-unwrap/expect-in-tests only
  rustfmt.toml          # edition = "2024" only (no style overrides)
  crates/
    domain/             # only crate with content in P01
    application/  repositories/  scanning/  retrieval/  persistence/  terminal-ui/
                        # empty libraries: a crate doc comment, no dependencies, no code
  tests/                # Tech §4 directories reserved; created empty with README only when first used (not in P01)
dev/
  p01_check.py          # lead-owned check runner (§3); development only, not shipped
```

- Package names equal the directory names (`domain`, …, `terminal-ui`). Every crate sets `publish = false` and `license = "UNLICENSED"`; the licence is a placeholder until P20 and is not a licensing decision.
- `[workspace.package]`: `edition = "2024"`, `rust-version = "1.97"`, `version = "0.1.0"`. Resolver 3 (the edition-2024 default), stated explicitly.
- The empty crates get no inter-crate dependencies in P01. The dependency direction (`terminal-ui → application → {repositories, scanning, retrieval, persistence} → domain`) is recorded here and wired in the phases that need it.

## 2. Toolchain pin (A5 = 5a)
- There is no `rust-toolchain.toml`. Every Cargo command runs the toolchain binaries directly, in the P00 environment (P00-T03 §2, D3-1 record):
  ```
  env -i PATH=<tc>/bin:/usr/bin:/bin HOME=<pwt>/_cargo-home CARGO_HOME=<pwt>/_cargo-home \
         CARGO_TARGET_DIR=<pwt>/_cargo-target/p01-<slot> RUSTC=<tc>/bin/rustc \
         RUSTDOC=<tc>/bin/rustdoc CARGO_TERM_COLOR=never LANG=C.UTF-8 <tc>/bin/cargo …
  ```
  - `<tc>` = `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu`.
  - `<pwt>` = `~/.local/share/pwt-dev-tools`.
  - `<slot>` = `lead`, `w1`, `w2` or `w3`. A separate target directory per worktree prevents build-lock contention and cross-contamination.
  - No key, token or other host variable is passed.
- **Version gate:** before any Cargo command, `p01_check.py` requires `rustc -V` and `cargo -V` to report exactly `1.97.1`; otherwise it exits non-zero. `rust-version = "1.97"` is metadata only.
- **Network:** every Cargo invocation in P01 runs with `--offline` (resolution) or `--frozen` (everything else) inside `unshare -rn`. A network-namespace probe runs first and must fail to reach the network.

## 3. Check runner (`dev/p01_check.py`)

ShellCheck is not installed, and Tech §22 requires it for any shell script we implement. The runner is therefore Python, run as `python3 -I -B dev/p01_check.py <mode> --slot <slot> [--package domain]`, the same convention as the T09 harness.

Modes:
- `env`: version gate, namespace probe and environment listing;
- `fmt`: `cargo fmt --all -- --check`;
- `clippy`: `cargo clippy --workspace --all-targets --frozen -- -D warnings`;
- `test`: `cargo test --workspace --frozen`, which includes doctests;
- `build`: `cargo build --workspace --frozen`;
- `static`: the source checks in §6.4;
- `all`: every mode in that order.

Behaviour:
- It captures exit codes without pipes and writes the logs plus a `summary.json` to a directory the caller names (`evidence/P01/<step>/` for the lead; the worker's own scratch directory for workers).
- Before and after each step it records SHA-256 hashes of `Cargo.lock` and every `Cargo.toml`, and fails if any changed.
- It sets no environment beyond §2.

These are the canonical commands of Tech §22. They count as verified only after the lead's first B0 run (T04).

## 4. Dependencies (A2 = 2a, A3 = 3a)

| Where | Entry |
|---|---|
| `[workspace.dependencies]` | `serde = { version = "=1.0.229", default-features = false, features = ["std", "derive"] }`, `serde_json = { version = "=1.0.154" }` (default features = `std`) |
| `domain` `[dependencies]` | `serde` (workspace) |
| `domain` `[dev-dependencies]` | `serde_json` (workspace), for tests and doctests |

**Resolution (T02):**
1. `cargo generate-lockfile --offline`, inside `unshare -rn`.
2. **Acceptance check:** every lock entry's `(name, version, source, checksum)` must equal an entry in `spikes/rust/Cargo.lock`. The expected set is serde, serde_core, serde_derive, serde_json, zmij, itoa, memchr, proc-macro2, quote, syn and unicode-ident, plus the workspace members.
3. Any extra crate, different version or checksum mismatch → **stop and report** (A2/A3 condition). Nothing is fetched.

**Exposure report (T02):**
- enumerate every `build.rs` and proc-macro in that set, and confirm each was already built in an accepted P00 graph (T06/T08);
- any new item → stop.

Known limitations (recorded, not repaired):
- no fresh RustSec screen (the P00 screen dates from 2026-10-24);
- no filesystem sandbox for build code;
- cargo-audit not run (A4 = 4a).

## 5. Frozen interfaces (B0 stubs are lead-authored; workers implement bodies and tests only)

Stub bodies in B0 are `todo!()` under a local `#[allow(clippy::todo)]`. The `static` check (§6.4) fails if any `todo!`/`unimplemented!` or that allow remains at P01-T09. Signatures, type names, derives, serde representation and visibility below are frozen. Changing them needs lead approval, and owner approval where the contract meaning changes.

### 5.1 `domain::ids` (W1, T05)
- **Types (17):** `ApprovalId`, `ArtifactId`, `CheckId`, `ChunkId`, `ConversationId`, `EventId`, `FileId`, `FindingId`, `MessageId`, `RepoId`, `ReportId`, `RequestId`, `RunId`, `SourceId`, `SuppressionId`, `TaskId`, `WorkflowId`.
  - These are exactly the schema's UUID-typed `*_id` fields.
  - Arch §4's `RepoId` is used, not the plan's `RepositoryId`.
  - String-typed identifiers (`adapter_id`, `rule_id`, `model_id`, …) are not UUIDs and are out of P01.
- **Generated by one macro.** Each type is `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]` over a private `[u8; 16]`. The types are distinct and not interchangeable.
- **API:** `pub fn parse(s: &str) -> Result<Self, IdError>`, `pub fn as_bytes(&self) -> &[u8; 16]`, `Display`, `Debug`, `Serialize`, `Deserialize`. `Display` and serialize use the canonical form. `Deserialize` goes through `parse`. `Debug` prints `RunId(<canonical>)`.
- **Canonical form:** lowercase hyphenated `8-4-4-4-12` hex (36 chars).
  - **Rejected:** uppercase hex, braces/URN forms, surrounding whitespace, the nil UUID (all zeros), any other length.
  - Version and variant bits are **not** enforced (schema `format: uuid` is structural); this is recorded as a limitation.
- **`IdError`** = `enum { Length { found: usize }, Format, Nil }`, with `Display` and `std::error::Error`. **Errors never contain the input text**, because input may be hostile.
- **Generation (`new_v4`) is out of P01:** it needs a randomness source, which is a later dependency decision.

### 5.2 `domain::time` (W1, T05)
- **`UtcTimestamp`:** `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]` over private `(secs: i64, nanos: u32)`, Unix epoch.
- **API:**
  - `pub fn parse_rfc3339(s: &str) -> Result<Self, TimeError>`
  - `pub fn to_rfc3339(&self) -> String`
  - `pub fn from_unix(secs: i64, nanos: u32) -> Result<Self, TimeError>`
  - `pub fn unix_seconds(&self) -> i64`, `pub fn subsec_nanos(&self) -> u32`
  - `pub fn now() -> Self` (via `std::time::SystemTime`)
  - `Display`, `Debug`; `Serialize`/`Deserialize` as the RFC 3339 string.
- **Accepted input:** `YYYY-MM-DDTHH:MM:SS[.f{1,9}]Z`, with uppercase `T` and `Z` only (UTC only, Data §4), years 0001–9999 and real calendar dates (leap years).
  - **Rejected:** offsets other than `Z`, lowercase `t`/`z`, leap second `:60`, more than 9 fractional digits, whitespace, out-of-range fields.
  - The `Z`-only rule is stricter than the schema's `date-time`; the app writes only this form.
- **Canonical output:** no fraction when `nanos == 0`; otherwise the fraction with trailing zeros trimmed. Parsing then formatting is identity on canonical strings. Non-canonical input (e.g. `.500`) normalises to `.5`, and a test documents this.
- **`TimeError`** = `enum { Format, Range }`. It does not echo input.
- Civil-date conversion is written in-crate (days-from-civil algorithm); no dependency.

### 5.3 `domain::text` (W2, T06)
- **Types:** `pub struct SanitisedText(String)` and `pub struct RedactedText(String)`. The fields are private, and the only constructors are the two functions below.
- **API:**
  - `pub fn sanitise(raw: &str) -> SanitisedText`
  - `pub trait Redactor { fn redact<'a>(&self, raw: &'a str) -> std::borrow::Cow<'a, str>; }`
  - `pub fn redact(raw: &str, redactor: &dyn Redactor) -> RedactedText`. It runs the redactor first, so patterns see the original text, then applies the same escaping as `sanitise`.
  - Both types have `as_str(&self) -> &str`, `len`, `is_empty`, `Display` (the safe text), `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`.
  - `RedactedText::to_sanitised(&self) -> SanitisedText`. Redacted text is also sanitised.
- **Escape set** (each code point replaced by `<U+XXXX>`, uppercase hex, at least 4 digits):
  - C0 controls U+0000–U+001F **except** U+0009 TAB and U+000A LF;
  - U+007F; C1 controls U+0080–U+009F;
  - bidi controls U+061C, U+200E, U+200F, U+202A–U+202E, U+2066–U+2069;
  - zero-width and BOM U+200B–U+200D, U+2060, U+FEFF;
  - line/paragraph separators U+2028, U+2029.
  - Everything else passes unchanged, Hangul included (D-15 is about rendering, not sanitising).
- **Disclosed property:** `sanitise` is idempotent, but literal input text `<U+001B>` cannot be told apart from an escaped ESC. That is acceptable for display and persistence of untrusted text, and it is recorded.
- **Serde:**
  - `SanitisedText`: `Serialize` as a string, and **`Deserialize` that re-sanitises**. Idempotence makes this round-trip-exact and safe, but it never yields a `RedactedText`.
  - `RedactedText`: **`Serialize` only.** There is no `Deserialize`, `From<String>`, `From<&str>`, `FromStr` or `Default`. The P02 store read-back path is decided in P02.
- **Length bounding is not in P01;** output budgets belong to P06. There is no real redactor in P01: the trait plus test redactors only. The minimal redaction module is P06.

### 5.4 `domain::states` and `domain::vocab` (W3, T07)
- **`vocab` enums** (string values exactly as in `schemas/workbench-store-v1.3.schema.json`; Superseded 2026-10-26 by CA-03: store 1.4, UI schema 1.1, canonical file `schemas/workbench-store-v1.4.schema.json`; Data Schema §24.): `Capability` (11), `Category` (9), `Severity` (6), `Assessment` (3), `Coverage` (3).
  - All are `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]` with `#[serde(rename_all = "snake_case")]`.
  - Each has `pub const ALL: [Self; N]` and `pub fn as_str(self) -> &'static str`.
  - `Capability::is_deferred(self) -> bool` is true for `non_mutating_verification`, `documentation_fetch` and `embedding_transmission` (Data §18.6a).
- **`states` enums**, with the same derives: `RunState` (9 values = schema `Execution`), `CheckOutcome` (9 = `Outcome`), `ApprovalState` (6 = `Approval.status`).
- **API on each:** `pub fn transition(self, next: Self) -> Result<Self, TransitionError<Self>>`, `pub fn can_transition(self, next: Self) -> bool`, `pub fn is_terminal(self) -> bool`. `TransitionError<S> { from: S, to: S }`.
  - The API is keyed on the target state, which keeps tables exhaustive: 81/81/36 pairs.
  - Self-transitions are rejected.
- **Allowed transitions.** Everything not listed is rejected.

| Machine | Allowed (from → to) | Source |
|---|---|---|
| Run | queued → awaiting_approval, running, cancelled, interrupted; awaiting_approval → running, cancelling, interrupted; running → awaiting_approval, cancelling, completed, partial, failed, interrupted; cancelling → cancelled, **interrupted** | Arch §7. **`cancelling → interrupted` is a proposed addition (T01-D1)** |
| Check | pending → running, **skipped, unsupported, cancelled, incomplete**; running → passed, findings, failed, unsupported, skipped, incomplete, cancelled | Arch §7 and Data §6. **Pending → {skipped, unsupported, cancelled, incomplete} are proposed (T01-D2)** |
| Approval | pending → approved, denied, cancelled; approved → revoked, expired | Arch §7 plus schema enum. `consumed` is a separate boolean, not a state |

- **Terminal states:**
  - Run: completed, partial, failed, cancelled, interrupted.
  - Check: everything except pending and running.
  - Approval: denied, cancelled, revoked, expired.

### 5.5 `domain::contract` (W1 in wave 2, T08; field list frozen by the lead at B1)
- `pub const APP_CONTRACT_VERSION: &str = "1.0";`
- `pub enum ContractStatus { Pilot, Reserved }` and `pub enum DeliveryClass { MustDeliver, LatestValue, Coalescible }`.
- **`pub enum AppCommand`:** the 23 pilot commands and 6 reserved ones of Arch §6. `SuppressFinding` and `UpdateSuppression` are separate variants. Reserved variants carry no payload.
  - `fn status(&self) -> ContractStatus`.
  - `pub fn dispatch_guard(cmd: &AppCommand) -> Result<(), AppEvent>` returns `Err(AppEvent::OperationFailed { code: FailureCode::Deferred, .. })` for every reserved command, as Arch §6 says. Plan §5A/§6 wording `OperationNotAvailable` is superseded (A1 = 1a).
  - **Commands are in-process only: no `Serialize`/`Deserialize`.** They may carry raw user input (`String` for URL and chat text) and `CredentialSecret` for `ConfigureCredential`.
  - `CredentialSecret` is a newtype with no serde and no `Clone`, a `Debug` that prints `CredentialSecret(<redacted>)`, and a best-effort overwrite on drop. That overwrite is **not** a secure-erase guarantee; KEY-01 hardening is P03.
- **`pub enum AppEvent`:** every pilot event of Arch §6 plus the 2 reserved ones.
  - `fn status(&self)`, `fn delivery(&self) -> DeliveryClass`, exactly per Arch §6.
  - `#[derive(Serialize)]` with `#[serde(tag = "event_type")]`. Variant names serialise PascalCase, equal to the Arch §6 names and the `log.json` `event_type`; fields are snake_case.
  - Free-text fields are `RedactedText` or `SanitisedText` only. IDs, states, vocab and timestamps use their domain types.
  - **No `Deserialize` in P01** (it contains `RedactedText`); read-back is P02.
- **Freeze rule:** the lead writes the complete payload field list into the B1 stub, the reviewer checks it against Arch §6 and Data §7, and the worker then implements. Each field must trace to an Arch §6 purpose or a schema field.

### 5.6 `domain::versions` and `lib.rs` (lead)
- `STORE_SCHEMA_VERSION = "1.3"`, `WRAPPER_PROTOCOL_VERSION = "1.1"`, `UI_SCHEMA_VERSION = "1.0"`, and a re-export of `APP_CONTRACT_VERSION`.
- `lib.rs` declares the modules and `#![forbid(unsafe_code)]`, which is also set as a workspace lint.

## 6. Lint policy and checks

### 6.1 `[workspace.lints]`
| Group | Settings |
|---|---|
| `rust` | `unsafe_code = "forbid"`, `missing_debug_implementations = "warn"`, `unreachable_pub = "warn"`, `unused_must_use = "deny"` |
| `clippy` | `all = { level = "warn", priority = -1 }`; `dbg_macro`, `print_stdout`, `print_stderr`, `todo`, `unimplemented`, `unwrap_used`, `expect_used`, `panic`, `exit`, `mem_forget` = `"warn"` |

- With `-D warnings`, every warning above fails the clippy step.
- `clippy.toml` sets `allow-unwrap-in-tests = true`, `allow-expect-in-tests = true` and `allow-panic-in-tests = true`.
- `print_*` is denied because Arch §6 and Tech §18 forbid raw output paths. The stub `todo!()` allowances are local and removed by T09 (§6.4).
- No `pedantic`/`nursery` lints: they add churn with no safety value at this stage. Every crate inherits the lints with `[lints] workspace = true`.

### 6.2 Phase checks (lead-run at B0, B1, B2 and T09; evidence under `evidence/P01/`)
fmt, clippy `-D warnings`, test (unit, integration and doctests), build. All of them are `--frozen`, offline and inside the namespace. Each check records pass/fail/ignored counts. Any `#[ignore]` is a failure unless listed here, and none are planned.

### 6.3 Test plan (minimum cases; workers may add, never remove)
| Task | File | Required cases |
|---|---|---|
| T05 | `tests/ids_time.rs` + unit tests | • every ID type: valid parse, Display/serde round-trip, the reject cases of §5.1 (incl. nil, uppercase, 35/37 chars, non-hex, misplaced hyphen, whitespace, braces) <br>• error Display does not contain the input <br>• distinct types do not compile as each other (one `compile_fail` doctest plus a positive control) <br>• timestamps: epoch, 0001-01-01, 9999-12-31T23:59:59.999999999Z, 2024-02-29 valid / 2023-02-29 invalid, the §5.2 reject cases, `.500→.5`, ordering, `from_unix` range, serde round-trip <br>• at least 10,000 deterministic pseudo-random `from_unix → to_rfc3339 → parse` round-trips (in-test xorshift, fixed seed, no dependency) |
| T06 | `tests/text.rs` + doctests | • every code point in the §5.3 escape set is escaped, and its neighbours (U+0009, U+000A, U+0020, U+00A0, U+061B, U+202F, U+2065, U+206A, U+FEFE, U+FF00) are not <br>• ESC/CSI/OSC sequences are neutralised <br>• idempotence over a corpus plus a 10,000-case seeded generator <br>• Hangul/CJK/emoji pass unchanged <br>• `redact` order: redactor sees raw text, output is escaped, and a redactor that injects controls is still escaped <br>• `SanitisedText` deserialize re-sanitises <br>• **`compile_fail` doctests, each with a positive control:** `RedactedText` from `String`/`&str` via `From`/`Into`; tuple-struct construction; `serde_json::from_str::<RedactedText>`; `SanitisedText` from `String`; passing a `String` where `SanitisedText` is required <br>Stable rustdoc does not check the error code of a `compile_fail` test, so each positive control differs from its `compile_fail` sibling only in the offending line |
| T07 | `tests/states.rs` | • full 81/81/36 transition tables, asserted against an independent table written in the test, not derived from the implementation <br>• `is_terminal` per value; terminal states have no outgoing transitions <br>• error carries from/to <br>• **schema cross-check:** the test loads `schemas/workbench-store-v1.3.schema.json` via `include_str!` and asserts every enum's string set equals the schema enum (Execution, Outcome, Approval.status, Capability, Category, Severity, Assessment, Coverage) <br>• `is_deferred` set <br>*(Superseded 2026-10-26 by CA-03: store 1.4, UI schema 1.1, canonical file `schemas/workbench-store-v1.4.schema.json`; Data Schema §24.)* |
| T08 | `tests/contract.rs` | • every Arch §6 command and event name is present (asserted against a name list in the test) <br>• `status()` and `delivery()` match Arch §6 for every variant <br>• `dispatch_guard` returns `OperationFailed{code=Deferred}` for all 6 reserved commands and `Ok` for all 23 pilot commands <br>• golden JSON for every event variant (`serde_json::to_value` equals the expected literal; `event_type` equals the variant name) <br>• full round-trip for every serde type that has a read path (IDs, timestamps, vocab, states, `SanitisedText`) <br>• `CredentialSecret` Debug never contains the secret <br>• the seeded-secret marker from a test redactor never appears in serialised events |

**Change to acceptance criterion 4 (`planning/P01-execution-plan.md` §6), proposed as T01-D3:**
- *was:* "The round-trip test covers each DTO variant".
- *becomes:* "every event variant has a golden serialisation test; every type with a read path has a full round-trip; commands are in-process and have no serde".

The reason: `RedactedText` deliberately has no `Deserialize` (§5.3), so events cannot round-trip in P01 without weakening the text-safety guarantee.

### 6.4 `static` checks (part of `p01_check.py`)
- no `todo!`, `unimplemented!` or `allow(clippy::todo)` left in `workbench/` (enforced from T09);
- no `unsafe` (also enforced by the lint);
- no `println!`/`eprintln!`/`dbg!` outside tests;
- in `contract.rs`, no `String`/`&str`/`Box<str>`/`Cow<` field type inside `AppEvent`. Command fields may carry them only on lines marked `// raw-input:`;
- `Cargo.lock` contains exactly the §4 set;
- no dependency outside §4 appears in any manifest.

## 7. Parallel execution (A6 = 6a)
| Wave | Worker | Task | Owns (exclusive) | Branch / worktree |
|---|---|---|---|---|
| 1 | W1 | T05 | `crates/domain/src/ids.rs`, `src/time.rs`, `tests/ids_time.rs` | `p01/w1-ids-time` |
| 1 | W2 | T06 | `src/text.rs`, `tests/text.rs` | `p01/w2-text` |
| 1 | W3 | T07 | `src/states.rs`, `src/vocab.rs`, `tests/states.rs` | `p01/w3-states` |
| 2 | W1′ | T08 | `src/contract.rs` (bodies only; types frozen at B1), `tests/contract.rs` | `p01/w1-contract` |

Paths are relative to `workbench/`.

- **Lead-only files:** `lib.rs`, `versions.rs`, every `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `rustfmt.toml`, `dev/`, `evidence/`, `planning/`, the root control files and the specs.
- **Spawning:** `sonnet-implementer` (`model: claude-sonnet-5-5`, as configured), with `isolation: "worktree"`.
- **Packet step 0:** `git merge --ff-only <B0>` (or `<B1>`), then check HEAD. The harness creates worktrees from `main`.
- **Commits:** workers may commit locally on their own branch only. They never push, merge, fetch, install or touch other branches.
- **Permitted commands:** `python3 -I -B dev/p01_check.py {fmt,clippy,test,build,static} --slot w<N> --package domain`, `git status/diff/add/commit/log` on their branch, and read-only file inspection. No network, no Cargo outside the runner, no edits outside owned files.
- **Stop condition:** owned-file tests pass under the runner, or a blocker is found, or the packet's scope would need changing. In each case the worker reports and stops.
- **Report format:** commit hashes; changed files; each command with its exit code and counts; deviations; open interface questions; limitations; remaining work. Unrun checks are labelled "not run".
- **Coordination:**
  - Workers have no messaging tool. Interface questions go in their report, or are appended under `flock` to the lead board `<pwt>/_p01/board.md`, which is outside the repository.
  - The lead answers with `SendMessage` and records whether delivery was confirmed.
- **Integration:** the lead reviews each worker branch, merges into `phase/P01-foundation` (cherry-pick on conflict), runs §6.2 itself, and runs the read-only `reviewer`. `security-reviewer` covers `text.rs` and `contract.rs`.
- **Shutdown:** finished workers are shut down. Worktrees and branches are removed only after preservation checks, with safe `-d`.

## 8. Commits and pushes (A7 = 7a)
Lead commits, each followed by a non-force push of `phase/P01-foundation`:
- **B0:** scaffold, stubs, lockfile, runner and T02 records; this request is committed with B0;
- **B1:** wave 1 integrated, plus the contract field list;
- **B2:** T08 integrated;
- **T10:** report.

The merge to `main` is separate, after P01 acceptance. This T01 deliverable stays uncommitted until B0.

## 9. Debug cycles (owner pre-authorisation, 2026-10-26)
- **Trigger:** an anomaly or test failure that the lead finds in its own verification after a worker task.
- **What the lead does:** opens `P01-Dnn` and records the confirmed defect against hypotheses, the evidence and a bounded fix. It then re-spawns the responsible worker (same task, same owned files, fresh packet carrying the diagnosis and the named retests). The lead re-verifies itself, and the cycle is logged in `plan.md` §9 and `evidence/debug/P01-Dnn/`.
- **Bounds:**
  - at most two cycles per defect, then revised diagnosis and an owner decision (CLAUDE.md);
  - the pre-authorisation does **not** cover changes to the frozen interfaces (§5), acceptance criteria, dependencies or lockfile, scope or permissions; those stop for the owner;
  - assertions are never weakened and cases are never removed.
- **Lead-owned defects** (manifests, runner, stubs) are fixed by the lead under the same logging.

## 10. Decisions for the owner at this stop
| ID | Proposal | Default if approved |
|---|---|---|
| T01-D1 | Add `cancelling → interrupted` (crash during cancellation); the Arch §7 diagram has no exit for it, and Data §12.7 recovery does not list `cancelling` | Add, and note it as a P01 clarification of Arch §7 (no spec text change in P01) |
| T01-D2 | Allow `pending → skipped / unsupported / cancelled / incomplete` (denied before start, missing tool, run cancelled before start, recovery of abandoned work: Data §6, §18.18) | Add |
| T01-D3 | Criterion 4 reworded as in §6.3; `RedactedText` and events Serialize-only in P01; commands not serde | Adopt |
| T01-D4 | Strict canonical forms: lowercase UUIDs with nil rejected; `Z`-only RFC 3339 with no leap second | Adopt |
| T01-D5 | The §5.3 escape set and the `<U+XXXX>` form (with the ambiguity disclosed) | Adopt |
| T01-D6 | Python check runner instead of a shell script (ShellCheck not installed) | Adopt |

Next action after approval: **P01-T02** (offline resolution and exposure report, lead). It is followed by **T04** (B0 scaffold, lead), and only then the wave-1 workers. T03 (the cargo-audit request, documentation only) runs alongside T02.

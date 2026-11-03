# PWT YAML Security Workbench — Development Plan

Plan revision: 1 (2026-10-23), with updates of 2026-10-24, 2026-10-26, 2026-10-27/28 (P02 progress, FR-32/FR-33) and 2026-10-29 (P02 accepted and merged; **P03 planned, §5C; C1–C10 = recommended**), 2026-10-30/31 (P03 T01–T04 accepted, wave 1 + P03-D01 done, cargo-audit installed; progress only, no scope change), 2026-11-01 (P03 accepted and merged; continuity rewritten for a developer takeover), 2026-11-01 (**P04 planned, §5E; E1–E14 = recommended**), 2026-10-31 (**owner requirement D-18 automatic LLM evidence analysis; CA-05 design direction accepted with qualifications the same day — not implementation or release acceptance**, §1, §4, §5D, §7, §8) (P01 progress, D-16/CA-03, FR-31, **D-17 headless CLI §4.1**): status (task progress, answered questions, blocker states, branch-policy record) and owner-directed follow-up requirement register entries (§5.2). Scope/estimate change on 2026-10-26: D-17 adds a headless CLI increment to every feature phase from P02 (§4.1, §7.1); no phases added or removed. Current Git state: see `status.md`.

This plan is a schedule and control proposal, not evidence of implemented code or passed tests. No phase starts without explicit owner approval. Specification references are to the v0.3.1 documents (`01_PRD.md` … `05_Database_Data_Schema.md`).

## 1. Decision register

Owner decisions received 2026-10-23 (planning-stage approval message). Recorded verbatim in substance; Opus has not added decisions.

| ID | Decision | Where applied |
|---|---|---|
| D-01 | Rust + Ratatui, Crossterm, Tokio, Serde confirmed; other dependencies via spikes | PRD §5, Tech §1, Arch A-02 |
| D-02 | Linux x86_64 is the first pilot target; portable architecture and native adapter interfaces preserved; Windows/macOS are extensions, not described as verified/supported until their acceptance matrix passes | PRD §5/§16, Tech §1, Arch A-10 |
| D-03 | Native Rust process adapters and direct Git are the pilot baseline; `.sh` wrappers are optional parity extension X6; tool-to-capability mapping kept; ShellCheck/Bats only for scripts actually implemented | Tool matrices (4 docs), Tech §17/§22, Arch A-07/A-09 |
| D-04 | Baseline yamllint, Gitleaks, rust-schema, Checkov; framework adapters actionlint, kubeconform; conditional zizmor, Compose, Helm, Trivy; versions/coverage frozen after spikes; no silent downgrade | Tool matrices, PRD §16 |
| D-05 | Scanners separately installed, detected by preflight; Python prerequisite + isolated environment declared; host Python 3.14 compatibility not assumed; no auto-install, sudo or bundling | Tech §3 |
| D-06 | Minimal OpenRouter BYOK assistant stays in the pilot (explicit model; session-only credentials, keyring if verified; local lexical retrieval over selected findings/excerpts; redaction, isolation, citations; no tools; deterministic reports without AI). Advanced retrieval, embeddings, historical chat deferred. Mock HTTP is the test baseline. Production model ID is the owner's; not invented | PRD §16, Tech §20 |
| D-07 | Plan a PDF renderer comparison spike; WeasyPrint not selected for being installed; no spike execution yet | Tech §21, P00-T08 |
| D-08 | Resolve C2 (schema `const false` + code), C3 (category/capability, supervisor-controlled timeout/cancel), C4/C5 (one canonical command/event contract, reserved deferred interfaces), C6/C7 (version separation, storage layout) | CA-01 (Data §22) |
| D-09 | 10-hour sessions; sessions/week not assumed; estimates in hours and sessions; hands-on vs waiting vs contingency separated; honest AI-inclusive re-estimate | §7 |
| D-10 | Installs not approved; prepare a separate installation request | P00-T02 |
| D-11 | Plan a versioned cached Kubernetes schema approach; no download/bundling yet | Tech §25, P00-T10 |
| D-12 | Figma timing unconfirmed; demo UI + stable contracts proceed; Figma translation separately estimated and nonblocking; demo-styling ship approval at release cut | P15a, P18 |
| D-13 | HTTPS and SSH; no unapproved submodule recursion, LFS/filter execution or hooks; credential mechanism preserved; limits documented after a spike | Tech §5, P00-T04 |
| D-14 | Phase-branch policy; no commits/merges/pushes without explicit approval; record actual revisions and dirty state | §3.6 |
| D-16 | (2026-10-26) CA-03 before the P01 merge: canonical tool availability `ready, missing, incompatible, unconfigured, unavailable, deferred` in store and UI; `denied` is a permission badge, not a tool state; store 1.3 → 1.4, UI schema 1.0 → 1.1, app contract stays 1.0 | Data §24, UI §5/§14, P01 |
| D-15 | (2026-10-24) Korean (Hangul script) support is **out of scope for the entire project**. Korean text in assessed files is still processed as data: never silently dropped and never a crash; where displayed or rendered it degrades visibly (e.g. `<U+XXXX>`). No Hangul font, IME or Korean-specific layout is a project target | PRD §16, P00-T08 |
| D-18 | (2026-10-31) Automatic LLM evidence analysis is a required product capability: after scanning and evidence sanitisation the configured LLM analyses each assessment's evidence automatically, without the chat being opened; interactive RAG chat stays a separate optional interface. Newly explicit; not covered by earlier approvals. **CA-05 design direction accepted with qualifications (owner, 2026-10-31)**: required in the pilot; chat separate; independent AI-only findings out of the pilot; cited interpretation, recommendations and uncertainty in scope; cross-repository analysis out of the pilot; Q-02 superseded (history kept); the lead contract task precedes dependent implementation; canonical schema/application changes not authorised. Open: O-AN-2 limits, O-AN-6 evaluation (`planning/CA-05-decision-proposals.md`); real-API evaluation unapproved (`planning/CA-05-automatic-evidence-analysis.md` r2) | PRD §6 ANL-01–ANL-12, all specs (CA-05 sections), §4, §5D, §7 |
| D-17 | (2026-10-26) Headless developer CLI: from P02, every feature phase adds command-line access to the feature it creates, so each feature can be exercised headless as soon as it exists (detail §4.1). The CLI is a second presentation adapter over the same coordinator and Arch §6 command/event contract as the TUI; it never bypasses permissions, redaction or the store writer | §4, §4.1, §7 |

Planning corrections 1–6 from the same message are applied: capability labels (PRD §16, SCAN-05/SCAN-06/RAG sources kept), minimal AI kept, early slice carries redaction/coverage (P01/P06/P07), per-tool hypothesis spikes (P00-T09), narrow retrieval sources (P17), and the agent loop (§3).

Pre-implementation contract amendment **CA-01** (store schema, wrapper protocol, command/event contract, UI schema version, storage layout) is recorded in Data Schema §22. Version identifiers were retained; the exception closes at the first implementation commit touching these contracts.

### Open owner questions (non-blocking for P00-T01)

| ID | Question | Blocks |
|---|---|---|
| Q-01 | Sessions per week (cadence) for the four-week cap | Schedule fit only |
| Q-02 | Confirm AI report narration (RES-07 optional part) is deferred, as labelled | **Superseded 2026-10-31 by D-18/CA-05** (owner decision; narration replaced by the required automatic analysis). Question text above kept as the historical record |
| Q-03 | Keep version identifiers under CA-01/CA-02, or bump store 1.3 / protocol 1.1 before P01 | **Answered 2026-10-24:** CA-02 versioned — store 1.3, wrapper protocol 1.1 |
| Q-04 | Production OpenRouter model ID (owner-supplied) | Real-API acceptance only; mocks need none |
| Q-05 | Which scope exchanges (§7.4), if any, to accept | Pilot cut |
| Q-06 | Approve read-only web lookup of official release/licence pages for P00-T02/T03/T10 | **Answered 2026-10-24** for P00-T02 (Q-06) and P00-T03 (C-1, crates.io/RustSec/docs.rs). P00-T10 lookups approved by the owner 2026-10-24 (T10 approval; lookups L1–L11, `evidence/P00/P00-T10/lookups.md`) |

## 2. Capability labels

The canonical pilot/conditional/deferred register is **PRD §16**. Phases below implement only `pilot` items unless an extension (X1–X6) or conditional item is separately approved. Deferred items keep their reserved interfaces (Architecture §6) and surface as explicit gaps.

## 3. Development control

### 3.1 Roles and model verification
- Owner: phase scope, debug authorisation, scope changes, next-phase acceptance, installs/network/API/commits.
- Opus 5.5 (this orchestrator, reported ID `claude-opus-5-5`): shared contracts, integration, all real verification commands, diagnosis, reports.
- Sonnet 5.5 workers: bounded tasks with explicit file ownership and base revision. **Model not yet verified.** At first delegation Opus records the agent definition's `model:` value, the configured alias resolution if observable, and any substitution; a worker's self-report is not proof.
- Haiku documentation worker: drafts `status.md`/`handover.md` (and assigned report drafts) from Opus's evidence packet only. **Model not yet verified**; same rule. Until the proposed agent definitions (`proposals/agents/`) are approved and installed, Opus writes these files directly and says so.
- Runtime OpenRouter model is unrelated to these development models.

### 3.2 Worker assignment packet (Sonnet)
Every assignment states: phase/task ID; base commit and branch/worktree; allowed files/crates (exclusive ownership); frozen contracts it must not change; tests to add; commands it may run (no network unless approved); stop condition; report format (changed files, commands run with exit codes, open questions). One mutating worker at a time initially; at most two with disjoint files once stable. Shared contracts (`crates/domain` DTOs, schemas, `Cargo.lock`) are owned by Opus.

### 3.3 Verification authority
Opus inspects the integrated diff, then runs the phase's listed checks itself. Evidence per check: command, cwd, relevant environment (toolchain versions, offline flags), revision + dirty state, exit code, pass/fail/skip counts, evidence path, manual-pending items, known limitations. Assertions are never weakened and regression cases never removed to pass.

### 3.4 Documentation loop
Opus prepares a sanitised evidence packet → Haiku drafts `status.md`/`handover.md` → Opus validates the diff against evidence and Git state → STOP and report to owner. One documentation writer at a time.

### 3.5 Debug loop
A failing check authorises nothing. Opus reports confirmed bugs vs hypotheses, severity, evidence and a bounded fix. An owner-approved debug cycle `Pxx-Dnn` covers named fixes plus listed retests, then the same report/document/stop loop. Two unsuccessful cycles on one defect → revised diagnosis and owner decision. Cycles are logged in §9.

### 3.6 Branches and commits (D-14)
Proposed: one branch per phase, `phase/Pxx-<slug>`, created from the approved base at phase start. Commits only with explicit approval (typically at phase acceptance); no merges or pushes without approval. `status.md` records actual HEAD and dirty state; no clean checkpoint is claimed that Git does not show. Policy retained by owner on 2026-10-24. Historical exceptions (explicitly owner-authorised at the time, not a policy change): `1e4a5b3`, `db402ca`, `4c659eb`, `008e80c` were committed/pushed directly on `main`. The conflict with this policy was reported only after the `008e80c` push. Published history is not rewritten. From 2026-10-24 P00 work continues on `phase/P00-decisions-spikes` (created from `008e80c`); merges to `main` and pushes need their own approval.

### 3.7 Evidence layout
`evidence/<phase-or-cycle>/` holds logs and check records (see `evidence/README.md`). Root continuity files stay compact and link to evidence.

### 3.8 Bounded parallel execution (owner policy, 2026-10-24, from P01 onward)

**Lead (Opus) role:** coordinator, reviewer and integrator. Each phase gets a proposed bounded parallel plan, with **up to three workers** only where the dependency graph supports independent work. This supersedes the earlier "one, at most two" rule in §3.2 for P01 onward. CLAUDE.md was amended to match on 2026-10-26 (owner approval A6 = 6a).

**Before spawning, the lead:**
- reads the plan, acceptance criteria and approvals;
- fixes shared interfaces and a common baseline commit;
- gives each worker an explicit scope, file ownership, tests and stop condition;
- gives each worker a separate worktree and branch;
- verifies the messaging mechanism and worktree support.

**Communication:** workers may communicate directly about dependencies and interfaces. Shared-contract or scope changes need lead approval. If direct messaging is unavailable (as now: worker agents have no messaging tool), communication is routed through the lead, with a lead-monitored board file; messages are never claimed as delivered when they were not.

**Workers:**
- stay in scope and avoid shared control files unless assigned;
- test their changes;
- make local checkpoint commits only where authorised;
- report commit hashes, tests, limitations and remaining work;
- keep honest recovery notes if interrupted.

**Lead ownership:** shared manifests and lockfiles, phase documentation and integration. The lead reviews worker commits, integrates them into the phase branch, resolves conflicts and runs combined regression and security checks.

**Hard limits:**
- no automatic merge to `main` and no push without the phase's explicit approvals;
- no new dependencies, broadened permissions or repeated repair loops without approval;
- finished workers are shut down;
- on a usage-limit interruption, preserve partial state and stop; never schedule automatic continuation.

## 4. Phase roadmap

Tier per phase refers to PRD §16 labels. Each phase approval covers its tasks and listed checks only. "Checks" become runnable commands only after P01 verifies them.

```text
P00 → P01 → P02 → (P03 ∥ P04) → P05 → P06 → P07★ → P08 → P09 → P10 → P11 → P12 → P13 → P14 → P15b → P16 → P17 → P19 → P20
P15a (demo TUI on mocks) may start after P01; P18 (Figma) after P15a + Figma delivery; X1–X6 only on separate approval.
```

| Phase | Tier | Scope | Key checks (proposed) | Needs before start |
|---|---|---|---|---|
| P00 | pilot | Decisions frozen, spike protocols, install/crate requests, spikes, tool/crate freeze (detail §5) | Spike records per protocol | Phase approval; T02 needs Q-06; spikes need D-10/crate approvals |
| P01 | pilot | Cargo workspace (crates per Tech §4), pinned toolchain, typed IDs/states, canonical command/event DTOs (Arch §6), `RedactedText`/`SanitisedText` newtypes so unsanitised text cannot reach persistence types | fmt, clippy `-D warnings`, test, build verified; DTO round-trip tests; cargo-audit if installed | Crate download approval |
| P02 | pilot | **Accepted and merged 2026-10-29; detail §5B.** **Headless CLI crate `crates/cli` created (D-17, §4.1).** JSON store v1.4 (CA-03): typed models, schema + semantic validation (Data §11, §18), single writer, lock, temp+replace, recovery, export manifests | Schema conformance of all roots; interruption/malformed/full-disk fixtures; same-day naming | P01 |
| P03 | pilot | **Accepted and merged 2026-11-01 (detail §5C; report `evidence/P03/P03-report.md`).** Permission evaluator (scope/revision/fingerprint), child-env sanitiser (strip model key/unrelated secrets), credential abstraction (session-only; keyring behind spike result) | Scope-match tables; env-strip tests; no key in any serialised output | P01, P02 |
| P04 | pilot | **Planned 2026-11-01, E1–E14 = recommended; execution not started; detail §5E.** URL validation (HTTPS/SSH only), clone/fast-forward update, unsafe-state detection, per-repo locks, provenance, Git safety controls from P00-T04 | Local fixture repos (dirty, diverged, hooks, filters, submodules); no source mutation | P00-T04, P02 |
| P05 | pilot | Inventory, exclusions/symlinks/limits, classifier (generic/Actions/Kubernetes; Compose/Helm reported conditional), tristate selection + selectors, reference detection recorded as gaps (SCAN-05) | Hostile filenames, symlink escape, limits, selector semantics | P04 |
| CA-05 | pilot (required; owner 2026-10-31) | **Lead contract task — must precede every dependent CA-05 implementation; execution not authorised yet:** apply CA-05 (domain analysis types, store 1.5 schema + D26 rules, app contract 1.2, UI schema 1.2, migration notes) before P07; proposed at the start of P06 (§5D) | Schema/contract checks; floors; 1.4 records stay readable | Separate owner approval to execute; P03 closed |
| P06 | pilot | Process supervisor (timeout/cancel/process-tree kill per CA-01/CA-02), adapter trait + protocol DTOs, output bounds, preflight + `tools.json`, **minimal redaction module and coverage ledger core**; must satisfy FR-01–FR-03 (§5.2) | Fake-tool fixtures: crash, timeout, oversized/malformed output, ANSI, early stdout close, secret-bearing stderr, tree kill; regression RC-FLOOD (FR-01) | P00-T05, P02 |
| P07★ | pilot | Vertical slice (headless): local synthetic repo → discover → select → in-process YAML syntax check → redacted finding + coverage ledger (other categories explicit `unsupported`) → JSON report via store → deterministic Markdown | Seeded secret absent from report/log; malformed YAML yields finding + honest coverage; velocity recalibration; (CA-05) deterministic evidence snapshot + digest, analysis stage reported `blocked`/`no_provider_configured` honestly | P05, P06 (+ CA-05 if approved) |
| P08 | pilot | yamllint, Gitleaks, rust-schema adapters per frozen contract sheets | Per-tool golden outputs, exit maps, repo-config injection, scope, redaction | P07, installs |
| P09 | pilot | Checkov adapter (app-owned config, no platform APIs, custom checks blocked) | Same per-tool suite | P07, install |
| P10 | pilot | Normalisation extension: fingerprints, cross-engine dedup with provenance, coverage aggregation | Dedup provenance; counts agree with checks | P08, P09 |
| P11 | pilot | Report finalisation, naming, export manifests, regenerate without rescanning (`RegenerateExport`); (CA-05) combined report sections for scanner evidence / AI analysis / limitations and stage notices, export revisions per analysis version | JSON/MD agree; regeneration; collision; AI-incomplete report never shows success | P10 |
| P12 | pilot | Restricted local PDF with selected renderer, independent export state | No remote fetch/active content/local file read; PDF failure keeps JSON/MD | P00-T08 decision, P11 |
| P13 | pilot | actionlint + kubeconform adapters; schema cache consumption (cache-only) | Missing CRD = gap; workflow commands never run | P00-T09/T10, installs, cache approval |
| P14 | pilot | Saved workflows, revision-bound approvals, app-open triggers, overlap/missed handling | DST/timezone, overlap, no run after exit | P11 |
| P15a | pilot | Demo TUI on labelled mocks using Arch §6 DTOs (terminal-ui crate only) | Snapshot/resize/input tests; mock marker visible | P01 |
| P15b | pilot | TUI ↔ real backend integration | UI acceptance scenarios (UI §12) automated parts; manual matrix pending | P14, P15a |
| P16 | pilot | OpenRouter BYOK client: explicit model, `allow_fallbacks=false`, ZDR/deny, streaming, typed errors, no tools; mock HTTP only. **(CA-05, scope extension pending approval)** structured output, analysis coordinator (trigger, eligibility, limits, retries, idempotency, reconciliation), response/citation validation, `AnalysisFile` persistence | Mock HTTP suite incl. no-route fail-closed, redirect auth-strip, key absent from logs; CA-05 AC-01–AC-16 | P03, P11 (+ CA-05) |
| P17 | pilot | (CA-05) Chat retrieves the saved analysis as `ai_analysis` secondary material and stays optional. Local lexical retrieval over selected findings, redacted excerpts, local approved rule descriptions and schema metadata; context isolation; citations; chat panel | Cross-repo leakage, seeded-secret payload, invalid citation, prompt-injection fixtures | P10, P15b, P16 |
| P18 | conditional | Figma translation (presentation only) | Snapshots + manual terminal checks | Figma delivery (D-12) |
| P19 | pilot | Recovery/security/privacy regression hardening; (CA-05) adversarial analysis fixtures and model-quality evaluation corpus/rubric (O-AN-6) | Full agreed regression | P17 |
| P20 | pilot | Linux x86_64 packaging, prerequisite/install guide, tool manifest, notices, manual acceptance | Release gates PRD §9/§15 (incl. gate 8, CA-05) | P19 |
| X1–X6 | conditional | zizmor, Compose, Helm, Trivy (needs pinnable build), Windows/macOS, `.sh` parity | Own contract sheets | Separate approval each |

Every feature phase from P02 also delivers its §4.1 CLI increment and that increment's scripted scenarios, as part of its own acceptance checks.

AI report narration is no longer a deferred item: D-18 replaces it with the required automatic evidence analysis (CA-05, phases above, §5D).

Deferred (no phase; interfaces reserved): SCAN-06 live verification, SCAN-05 non-image references, SCAN-09 historical secrets, RES-06 suppression/baseline, run comparison, embeddings, chat persistence/historical chat, remote documentation ingestion, draggable splits.

### 4.1 Headless developer CLI (D-17, owner decision 2026-10-26)

**Purpose.** A developer can try every feature from the command line as soon as the phase that creates it is accepted, without waiting for the TUI (P15b). It is also the scripted acceptance path for those phases.

**Simple usage (target shape, from P02).**
- Build once: `cargo build -p cli` through the phase runner (same pinned toolchain, `env -i`, offline, as for every other check).
- Try one feature by hand: `pwt-workbench-cli --data-dir <tmp> <group> <command> [args] [--json]`, e.g. `store validate`, `repo add <url>`, `scan start <run-id>`.
- See what can be tried: `pwt-workbench-cli help` lists every command; `pwt-workbench-cli features` prints the feature index (below).
- Test everything headless in one go: `python3 -I -B dev/<phase runner> cli` runs all scripted CLI scenarios of all accepted phases (also included in `all`). No TUI, terminal or display is needed; scenarios use temporary data dirs and synthetic fixtures only.

**Definition of done per feature (every phase from P02).** A feature counts as delivered only when all three exist:
1. a CLI command (or a flag on an existing one) that exercises it headless through the coordinator;
2. at least one scripted scenario test that runs that command against a synthetic fixture and asserts on its `--json` events and exit code (plus at least one negative/failure case);
3. a row in the feature index: requirement ID (PRD/Tech/FR), command, scenario test name, mode (`real`, `mock`, `dev`, or `manual-pending`).
A test in the CLI crate checks that every index row names an existing command and an existing test, so the index cannot silently go stale. Features that cannot be exercised headless (e.g. pure TUI layout) are listed in the index as `manual-pending` with the reason, never omitted.

**Design rules** (constraints, binding on every phase):

**Same path as the TUI.**
- The new crate `workbench/crates/cli`, binary `pwt-workbench-cli`, parses arguments into Arch §6 `AppCommand`s and sends them through the `application` coordinator.
- It prints the resulting `AppEvent`s. It never calls Git, scanners, the store or HTTP directly.
- Commands without an Arch §6 equivalent are labelled `dev` (see P06), call only domain/application entry points, and are excluded from release builds unless the owner decides otherwise at P20.

**Output.**
- Default output is human-readable. `--json` emits one serialised `AppEvent` per line (JSON Lines), using the existing golden-tested serialisation.
- Exit codes are documented per command. Execution status is kept separate from findings: exit 0 means the command executed, never "clean".

**Data location.**
- `--data-dir <dir>` is required for every command that touches app data.
- The CLI never defaults to the user's real application-data directory during development. Test scenarios use temporary directories.

**Permissions.**
- Approvals are never implied. A pending `ApprovalRequested` stops the command and prints the request. The user approves explicitly (`approvals approve <approval-id>` or an interactive confirmation on a TTY), then re-runs.
- There is no blanket `--yes` and no auto-approval flag. Network operations follow the same approval and owner rules as everywhere else.

**Secrets.**
- Keys are never read from argv or environment variables. `credential set` reads from a TTY prompt with echo off, or from stdin with `--stdin`.
- Raw input appears only as `RawInput` inside commands. Printed output carries only `RedactedText`/`SanitisedText`, escaped for the terminal.

**Mocks.** Mock providers and fake tools are opt-in (`--mock-provider`, `dev` fixtures). They are labelled in every output line and can never satisfy real-scan or real-API acceptance.

**Dependencies.** Argument parsing uses `std` only, unless a parser crate is approved through a normal dependency request.

**Tests.** Each phase adds scripted CLI scenarios (`workbench/crates/cli/tests/<phase>_*.rs`) that run the binary on synthetic fixtures. They are part of the regression suite and of the runner's committed test floors.

**Per-phase increments** (commands are proposals; exact names are fixed in each phase request):

| Phase | CLI increment | Scripted scenario examples |
|---|---|---|
| P02 | Crate and binary skeleton; `--version` (all contract versions); `help`, `features` (feature index + staleness test); runner `cli` mode; `store init`, `store validate`, `store show <file>`, `store recover` | Init a temp store; validate all roots; malformed/interrupted-write recovery; schema 1.4 rejection of a 1.3-labelled file |
| P03 | `approvals list/approve/deny/revoke`; `credential set --session` (stdin/TTY), `credential status`, `credential delete`; `dev env-preview <tool>` (sanitised child environment, values masked) | Scope/revision mismatch prompts again; the key never appears in output, argv or logs |
| P04 | `repo add <url>`, `repo list`, `repo refresh <id>`, `repo status <id>`, `repo remove <id> [--delete-checkout] [--purge-index] [--keep-reports]` | Dirty/diverged/hook/filter/submodule fixture repos unchanged and skipped with reason; remote clone only with network approval |
| P05 | `discover <repo>`, `files list [--filter]`, `select file/repo/visible/all/mode/patterns` | Hostile filenames escaped; symlink escape excluded with reason; tristate counts |
| P06 | `tools preflight`, `tools list`; `dev supervise <fake-tool-fixture>` | Crash, timeout, output flood (RC-FLOOD), early close and secret-bearing stderr reported as typed outcomes |
| P07★ | `plan review`, `scan start <run-id>`, `scan cancel <run-id>`, `runs list/show`, `report show <report-id>` | End-to-end slice on a synthetic repo; seeded secret absent from all output; malformed YAML gives a finding plus honest coverage |
| P08, P09, P13 | Adapters selectable in `plan review --adapters …`; per-tool coverage in `runs show` | Per-tool golden runs; missing tool gives a coverage gap, not a pass |
| P10–P12 | `report export <id> --format json/md/pdf`, `report regenerate <id> --format md/pdf` | Dedup provenance visible; PDF failure keeps JSON/MD; regeneration without rescanning |
| P14 | `workflow save/list/show/delete/enable/disable/run`; `dev triggers tick --at <UTC time>` (deterministic clock) | Overlap skipped; missed-while-closed recorded; DST cases |
| P15a/P15b | No new commands; **parity test**: the same scripted command sequence through the CLI and through the TUI reducer produces the same event sequence | — |
| P16 | `provider set --model <id>`, `chat send` with `--mock-provider` (real provider only after owner approval of a real-API check) | No-route fail-closed; key absent; typed failures |
| P17 | `chat context set …`, `chat send` with citations | Cross-repo isolation; invalid citation flagged; injection fixtures inert |
| P19/P20 | Hardening of CLI paths; owner decision whether the CLI ships in the release or stays development-only | Full regression includes all CLI scenarios |

**Specification follow-up.** A short amendment is needed: Tech §4 (crate list), Architecture §3/§4 (the CLI as a second presentation adapter) and UI Schema §17 (CLI/TUI parity). It will be proposed in the P02 request for owner approval. Until then this section is plan-level only. **Done 2026-10-27:** amendment CA-04 approved by the owner and applied (Data Schema §25; app contract 1.1, document 0.3.5; record `evidence/P02/CA-04/check.md`).

## 5. P00 — decisions, protocols and spikes

P00 approval would authorise only the tasks the owner names. Tasks needing network, installs or crates stay blocked until their separate approval.

| Task | Content | Network/installs | Hours |
|---|---|---|---|
| P00-T01 | Write spike protocol sheets (§5.1) and synthetic fixture specifications (hostile filenames/ANSI, seeded fake secrets, repo config/ignore injection files, malformed/alias-bomb YAML, multi-document, symlinks, Git hooks/filters/submodule fixtures). Documentation only | None | 3–5 |
| P00-T02 | D-10 installation request: exact versions, official sources, checksum/signature verification, install location (user-owned, no sudo), privileges, network, Python compatibility, rollback/removal — baseline tools in one request, conditional tools separately | Read-only lookup of official pages (Q-06) | 3–5 |
| P00-T03 | Crate request for spikes/P01: candidate crates with versions, licences, advisories to check | Read-only lookup (Q-06); download only after approval | 1–2 |
| P00-T04 | Git safety spike (D-13) on local fixture repos: hooks, filter/LFS process, submodule recursion, protocol allowlist, credential helper preservation; HTTPS/SSH remote checks only with separate network approval | Local; remote part separate | 3–5 |
| P00-T05 | Process-tree supervision spike on Linux: process group/session, graceful then forced kill, orphan detection, output bounding (std-only if feasible) | None | 3–5 |
| P00-T06 | YAML parser + JSON-schema validator spike: duplicate keys, alias bounds, multi-doc, source mapping, Draft 2020-12 + `format` | Crates (T03 approval) | 5–8 |
| P00-T07 | Linux keyring spike: Secret Service availability/headless behaviour; decides KEY-01 OS-store mode | Crates | 2–3 |
| P00-T08 | PDF renderer comparison (D-07): restricted rendering (no remote/active/local-file reads), Unicode/tables, runtime deps, packaging, licence; candidates include in-process Rust options and restricted external renderers | Installs for absent candidates | 4–6 |
| P00-T09 | Per-tool spikes: yamllint, Gitleaks, Checkov, actionlint, kubeconform (§5.1) | D-10 installs | 10–15 |
| P00-T10 | Kubernetes schema cache plan (Tech §25): candidate origins, target version, licence, integrity/freshness | Read-only lookup; no download | 2–3 |
| P00-T11 | Freeze: tool versions/flags/coverage, crate selections, contract sheets, P00 report | None | 2–4 |

### 5.0 P00 progress (2026-10-24)

**P00 accepted by the owner 2026-10-24 with qualifications** (closure pass; `evidence/P00/P00-report.md` §11). All limitations and FR-01–FR-30 remain binding on their phases. **P01 is ready for planning, not executing**; P01 execution, merge to `main` and push to `main` need separate approval.

| Task | State | Evidence |
|---|---|---|
| P00-T01 | Accepted | `spikes/P00-T01-*.md`, `evidence/P00/P00-T01/` |
| P00-T02 | Request accepted; approved installs (Request A, B1) done and accepted with limitations; PyPI provenance exception; cargo-audit deferred to P01 | `spikes/P00-T02-installation-request.md`, `evidence/P00/P00-T02/` |
| P00-T03 | Rev 1 committed (`f1bd80d`); rev 2 research-backed request complete; D3-1–D3-3 decided 2026-10-24 (D3-1 executed; D3-2/D3-3 provisional); no separate historical acceptance record. **Closure decision (owner, 2026-10-24): research/design selections ratified as provisional where not exercised** (not proof those crates were tested) | `spikes/P00-T03-crate-request.md`, `evidence/P00/P00-T03/` |
| D3-1 (T05 dependency) | libc 0.2.191 resolved, fetched, built offline in network namespace; not executed | `spikes/rust/`, `evidence/P00/P00-T05-D3-1/` |
| P00-T05 | **Accepted 2026-10-24 as a feasibility spike with findings** (not production-supervisor acceptance); F-1 → FR-01, F-2/F-3 → CA-02, F-4 → FR-02, untested cases → FR-03 | `evidence/P00/P00-T05/` |
| P00-T04 | **Accepted 2026-10-24 (qualified: tested procedure, Git 2.53.0, this host/fixtures only)**; run 1 failed, D01 re-run 36/0/0 (+1 observation) | `evidence/P00/P00-T04/` |
| CA-02 | Applied, versioned and confirmed by owner: store 1.3, wrapper protocol 1.1, specs v0.3.3 | Data Schema §23, `evidence/P00/CA-02/`, `evidence/P00/CA-02-versioning/` |
| P00-T06 | **Accepted 2026-10-24 based on D01, with documented conditions/limitations** (dependency/pipeline suitability for the tested cases; no fallback required); first run 96/5/6 preserved; D01 121/0/7, J16 35/35; carry-forward FR-05–FR-17 (§5.2) | `spikes/P00-T06-request-and-fixtures.md`, `evidence/P00/P00-T06/` |
| P00-T07 | **Accepted 2026-10-24 as a completed feasibility spike with qualified findings** (tested isolated setup only; direct in-process keyring calls, persistent storage, native cancellation, real prompter and other implementations **not** accepted); spike-only advisory exception (RUSTSEC-2026-0370/-0371/-0372); run1 (omitted K04b) and run2 preserved; carry-forward FR-18–FR-24 (§5.2) | `spikes/P00-T07-request-and-isolation.md`, `evidence/P00/P00-T07/` |
| P00-T08 | **Accepted 2026-10-24 with qualifications** (P1 pulldown-cmark + printpdf, featureless; tested cases only; whole-font embedding; no shaping/RTL; empty `/Annots` permitted; Korean out of scope D-15; visual correctness unverified; WeasyPrint suspended); run1 failed, D01 9/10 written, verdicts unchanged | `spikes/P00-T08-request.md`, `evidence/P00/P00-T08/`, `evidence/debug/P00-T08-D01/` |
| P00-T09 | **Accepted 2026-10-24 with qualifications** (run1 contracts for yamllint/gitleaks/kubeconform/actionlint; Checkov revision 2 only, Kubernetes selected-file workflow; FR-25–FR-29; limits in `status.md`). Formal run1 done (checkov R1 stop condition fired, preserved); debug cycle P00-T09-D01 (Checkov contract revision 2, provisionally accepted as candidate) done with its own formal run; P00-T09-D02 (H-3 fix, attribution) done; carry-forward FR-25–FR-29 — see `status.md` and the records | `spikes/P00-T09-request.md`, `spikes/t09-tools/`, `evidence/P00/P00-T09/` |
| P00-T10 | **Accepted 2026-10-24** as a completed schema-cache planning and offline reference-prototype task (not a production cache, schema coverage or collision-safe resolution); R-1 → FR-30 (P13 mandatory); O-1–O-6 open | `spikes/P00-T10-*.md`, `spikes/t10-cache/`, `evidence/P00/P00-T10/` |
| P00-T11 | **Accepted 2026-10-24** as the freeze/report preparation task, with qualifications (checker 139/139 after closure revision) | `spikes/P00-T11-request.md`, `spikes/p00-freeze/`, `evidence/P00/P00-report.md`, `evidence/P00/P00-T11/` |

### 5.2 Follow-up requirements register (from accepted spikes)

| ID | Source | Assigned | Requirement |
|---|---|---|---|
| FR-01 | P00-T05 F-1 | P06 (mandatory) | Output budgets enforced at the reader boundary: reader stops consuming at the per-stream budget and notifies the coordinator immediately (event/channel, no periodic polling); coordinator starts bounded SIGTERM → grace → SIGKILL; pipes closed or drained within a bounded budget without deadlock; `output_streams` `truncated` + `transport_status = output_limit_exceeded` recorded (CA-02). Distinguish bytes consumed, retained in memory, persisted (sanitised) and child-generated (unknowable beyond consumption). **Proposed measurable target** (owner to confirm at P06 approval): bytes consumed ≤ budget + one read buffer (≤ 64 KiB) per stream; bytes retained ≤ budget; termination initiated ≤ 50 ms after budget exhaustion; group gone ≤ grace + cleanup bound. No claim that the child writes nothing beyond the limit (kernel pipe buffering). Regression RC-FLOOD: re-run FX-PROC-FLOOD-OUT/-ERR; spike baseline (52 MB / 78 MB consumed past a 1 MiB limit, `evidence/P00/P00-T05/results.json`) must not recur |
| FR-02 | P00-T05 F-4 | P06 (decision) | The spike's adoption/reaping relied on being PID 1 of a PID namespace; this does **not** carry over to the ordinary application. P06 must decide and verify: how tool processes get a dedicated process group/session; how orphaned descendants are discovered and reaped (e.g. `PR_SET_CHILD_SUBREAPER`, PID namespace, cgroup); whether such containment is required; and which descendants remain outside process-group guarantees (group/session changers, daemonisers) |
| FR-03 | P00-T05 untested | P06 verification matrix | Add ANSI/control-sequence output (FX-PROC-ANSI) and early stdout closure while the child keeps running (FX-PROC-CLOSE-EARLY); early EOF is not proof of child exit |
| FR-04 | P00-T05 F-2/F-3 | CA-02 (done; versioned store 1.3 / protocol 1.1) | Descendant cleanup recorded separately from the primary termination reason and from the child's own exit/signal; `not_applicable` stream state for spawn failures |

FR-05–FR-17 come from the P00-T06 acceptance (owner, 2026-10-24) and are mandatory production implementation/regression requirements. The spike passing them (`evidence/P00/P00-T06/D01/`) is not evidence that the production implementation does; each must be implemented and re-verified in its phase. Case IDs refer to `spikes/P00-T06-request-and-fixtures.md` and the D01 record. Phase assignment is Opus's proposal from §4 scopes (P07 = first production YAML parse, P08 = rust-schema adapter, P02 = store schema validation); the owner may reassign.

| ID | Source | Assigned | Requirement |
|---|---|---|---|
| FR-05 | P00-T06 D01 | P07; regress in P08 | Original-input position mapping built in O(n) per input (no rescanning per lookup) |
| FR-06 | P00-T06 D01 | P07; P08, P10 (finding locations) | Position convention: zero-based byte offset into the original input bytes (a stripped BOM counts); one-based line; one-based Unicode-scalar column. Regression Y16-P1–P7, E1, E2 |
| FR-07 | P00-T06 D01 | P07 | At most one leading UTF-8 BOM removed before parsing, recorded (`bom_stripped`), original-byte provenance preserved. Regression Y16-P3/P5/P7, E2, B1 |
| FR-08 | P00-T06 limitation | P07; P10/P11 reports, P15 UI | No invented end spans; no display-width column claims |
| FR-09 | P00-T06 Y06/Y07 | P07 | Duplicate keys detected before any lossy map conversion; distinct YAML keys (`1` vs `"1"`) not treated as duplicates. Regression Y06, Y07 |
| FR-10 | P00-T06 Y07–Y09, Y18–Y20 | P07; P08 | Explicit `unsupported_for_validation` projection policies (non-JSON numbers, non-string/complex keys, merge keys, non-core tags never instantiated); nothing silently coerced |
| FR-11 | P00-T06 Y11/Y15 | Limits designed P05, enforced P07 | Per-input resource budgets (input bytes, documents, scalar length, projected nodes/alias events, depth). **Production default values remain a later explicit owner decision**; spike values are not adopted by default |
| FR-12 | P00-T06 D01 Y14 | P07 | Composer enforces its depth limit for both block and flow nesting (parser guard covers flow only). Regression Y14 flow 64/65/201, block 64/65/300 |
| FR-13 | P00-T06 D01 Y14 | P07 | Parser-imposed and composer-imposed limits classified separately (`depth(parser)` vs `depth(composer)`); unrelated syntax errors stay `syntax_error` |
| FR-14 | P00-T06 D01 Y14 | P07; any parser version change | Recursion-error mapping is pinned to the exact parser version's diagnostic (saphyr-parser 0.2.2 `recursion limit exceeded`); a dedicated regression (Y14 flow 10,001) must fail visibly if wording changes. Dependency upgrades need approval and re-verification |
| FR-15 | P00-T06 J04/J05/J08/J09/J15 | P02 (store validation); P08 | `format` assertions explicitly enabled; external `$ref` retrieval (HTTP and file) disabled/compiled out; unavailable schema → `schema_unavailable`, never valid |
| FR-16 | P00-T06 J01–J16 | P02 (J11–J13, J16); P08 (J01–J07, J14, J15) | Current-contract fixtures (store 1.4 / protocol 1.1 examples and version-const negatives) and the differential comparison carried into the production test suites |
| FR-17 | P00-T06 D01 B2/B3 | P07 | Repeated or internal U+FEFF remains content and is recorded; no blanket normalisation without a justified, owner-approved policy |

FR-18–FR-24 come from the P00-T07 acceptance (owner, 2026-10-24). They are mandatory for **any future persistent OS credential-store implementation** and go into the production acceptance matrix. Persistent storage itself remains pending a separate architecture/security decision (K-D not chosen); session-only mode stays required and must not depend on a working desktop/keyring service. The helper is not implemented now. Evidence: `evidence/P00/P00-T07/record.md`.

| ID | Source | Assigned | Requirement |
|---|---|---|---|
| FR-18 | T07 K06, source review | P03 (if OS-store mode is adopted) | **Execution boundary:** potentially blocking credential-store operations run outside the main application process in a bounded, separately terminable helper. Timeout is never implemented by abandoning a blocked thread in the app. Helper startup, IPC, operation and cleanup each have deadlines. Concurrent helpers and repeated attempts are bounded |
| FR-19 | T07 design | P03 | **Credential transport:** bounded private IPC. Secrets only through the necessary private in-memory/pipe channel, never argv, routine environment variables, logs or durable IPC files. Helper identity, protocol and response framing validated; late responses after timeout discarded |
| FR-20 | T07 source (`unixexec:`/`ibus:`, fallbacks) | P03 | **Bus/address policy:** parse and validate the complete allowed address (no `starts_with("unix:")` check). Reject executable, autolaunch and unapproved transports, and alternate-address lists that escape policy. Explicit source/fallback policy (`DBUS_SESSION_BUS_ADDRESS` → `$XDG_RUNTIME_DIR/bus` → `/run/user/<euid>/bus`) is decided, not inherited. The disclosed binding limit is preserved |
| FR-21 | T07 K01–K06 | P03 | **Outcomes:** distinguish unavailable, missing entry, locked/denied, timeout and internal/protocol failure. On failure offer session-only mode or unavailable AI; **no plaintext fallback** |
| FR-22 | T07 K06 | P03 | **Ambiguous operations:** a timed-out write or delete may have completed at the service. No rollback claim and no blind retry of an uncertain operation; explicit reconciliation and user-visible outcome |
| FR-23 | T07 K04b, untested prompter | P03 adoption test | **Prompt/service effects:** killing a helper does not establish that a desktop prompt or service-side operation was cancelled. Real-user/private-service prompt and cancellation behaviour is an adoption test, not a property proven by T07 |
| FR-24 | T07 advisory exception | P03 adoption; release | **Dependency acceptance:** the maintenance exception stays limited to T07. Re-screen the backend path (including whether a maintained zbus-6-compatible release exists) before P03 adoption and again before release |
| FR-25 | P00-T09 gitleaks (pending T09 acceptance) | P08 | Never rely on exit 0 alone for successful coverage (nonexistent target exits 0); validate target existence/type before invocation; treat repository-controlled ignore/suppression (`.gitleaksignore`, `gitleaks:allow`) as a coverage issue (`--gitleaks-ignore-path` did not neutralise the repo ignore file); debug output prohibited in the application contract; `--redact=100` does not make stderr safe to persist; persist only approved normalised fields through the output guard |
| FR-26 | P00-T09 kubeconform (pending T09 acceptance) | P13; P00-T10 | Treat kind/apiVersion-derived schema paths as untrusted (`../` in `kind` escaped the schema root); the T10 schema-cache plan must address traversal and containment; exit codes alone do not establish successful validation coverage (classify from parsed per-resource status) |
| FR-27 | P00-T09 + P00-T09-D01 checkov (pending T09 acceptance) | P09 | Only contract revision 2 (`evidence/P00/P00-T09/contract-checkov-rev2.md`): private cwd, app-owned HOME/XDG/TMP, app config outside input, explicit `--framework`, `--skip-download`, `--skip-results-upload`, `CKV_SKIP_PACKAGE_UPDATE_CHECK=True`, `-f` on allowlisted staged copies, never `-d`/cwd = repository/external checks; R1 rejected. Empty output or zero checks must be distinguished from a clean scan using input expectations and validated result structure; in-file suppressions surfaced; selected-file coverage limits reported; production staging helper still to be implemented and verified |
| FR-28 | P00-T09 actionlint (pending T09 acceptance) | P13 | ShellCheck/Pyflakes integration remains unverified: disable the helpers explicitly; do not install them or claim the capability was tested without a separate approval and verification |
| FR-29 | P00-T09 self-test (S2) | P06 (with FR-02) | Process-group cleanup does not contain descendants that leave the group (`setsid`); a bounded harness return is not proof of containment; production containment remains a separate requirement |
| FR-30 | P00-T10 finding R-1 (accepted 2026-10-24) | P13 (mandatory acceptance requirement) | Before production schema lookup is enabled: manifest entries identify the complete API group, version and kind; lookup matches the complete identity, never a truncated group prefix; unknown or ambiguous identities return unsupported/not assessed and never fall back to a similarly named built-in schema; regression tests include the `apps.example.com/v1` Deployment and `networking.istio.io/v1` Ingress collisions plus valid built-in controls. Until then, unknown API groups must not be routed through the T10 filename mapping. The T10 prototype does not fix R-1 |

FR-31 comes from the P01 conformance check (owner approval 2026-10-26); FR-32/FR-33 from the P02-T14 security review (owner 2026-10-28, Q-T14-2/-3). It makes the plan §4 P01 intent ("unsanitised text cannot reach persistence types") enforceable in P02.

| ID | Source | Assigned | Requirement |
|---|---|---|---|
| FR-31 | P01 conformance check | P02 (mandatory acceptance requirement) | **Safe text in store models.** (1) A reviewed classification table assigns every plain-string field of the store schema to: closed enum; validated newtype (relative/confined path, Git ref, credential-free remote URL, IANA timezone, slug, model ID, redaction version, parser/command-policy IDs); `RedactedText` (all text from users, tools or models, including workflow descriptions and approval purposes); or `SanitisedText` (application-derived display text only). (2) `RedactedText` read-back only through a persistence-crate-private path (sealed token), re-escaping on read; a public `Deserialize` stays impossible (compile_fail guard kept); the trust assumption (app-owned store, Architecture trust boundary 3) is recorded. (3) Enforcement: the static field-type allowlist extended to every store-model struct (bare `String`/`&str`/`Cow`/`serde_json::Value` rejected, with negative controls); a schema-completeness test that fails when a plain-string schema property has no classification entry; compile_fail guards for representative fields |
| FR-32 | P02-T14 security review S-L2; owner Q-T14-2 (2026-10-28) | P06 (with the real redactor) or P19 hardening at the latest | **Default-ignorable code points in terminal text.** Extend the `domain::text` sanitise escape set to the remaining default-ignorable/invisible code points (U+206A–206F, U+180B–180D, variation selectors VS1–VS256, U+17B4/U+17B5, U+FFFC) with positive/negative tests. Visual-disguise risk only; no terminal-control effect today. |
| FR-33 | P02-T14 security review S-L3 (disclosed T07 limitation); owner Q-T14-3 (2026-10-28) | P03 | **Root against a foreign data directory.** The `.app.lock` uid ownership proxy does not hold when the process runs as euid 0. Refuse to open a data directory not owned by root when euid is 0 (typed error, exit code per CLI table), with tests. |

Process rule from T07 (owner, 2026-10-24): future spike requests enumerate every helper tool and operation, including verification utilities; an unlisted tool is not covered by approval of the surrounding stage.

### 5.1 Per-tool spike protocol (C1/C11 are hypotheses per tool)
For each pinned tool record: (a) every repository config/ignore/allowlist/plugin/custom-check file it auto-discovers, and whether an explicit app-owned config fully neutralises each; (b) explicit-file input honoured vs directory walk — if walking, a staged selected-file copy strategy and its effect on line mapping; (c) output format(s), where raw secret material can appear (stdout, stderr, report file) and the effect of tool redaction options; (d) exit codes for clean, findings, parse error, crash, bad flag; (e) network attempts in default mode and whether they can be disabled verifiably; (f) behaviour on malformed YAML with a seeded fake secret (SCAN-08); (g) version string and binary checksum. Results feed a contract sheet: adapter ID, version, invocation template, config policy, scope strategy, parser ID, exit mapping, network policy, limitations.

## 5A. P01 — workspace, domain types and contracts (planned 2026-10-24; approved for execution, accepted and merged 2026-10-26)

Detail, interfaces, parallel waves and approvals A1–A7: `planning/P01-execution-plan.md`.

| Task | Content | Owner | Network/installs | Hours |
|---|---|---|---|---|
| P01-T01 | P01 request: layout, toolchain-pin method, lint policy, frozen module interfaces, test plan | Lead | None | 1–1.5 |
| P01-T02 | Dependency request: `serde` (derive) 1.0.229 + `serde_json` 1.0.154; offline (cache) or online (RustSec + locked fetch) resolution; build-exposure report | Lead | Option-dependent (A2/A3) | 1–2 |
| P01-T03 | cargo-audit installation request (R-4) and offline advisory-DB plan | Lead | Install + fetch (A4) or stays deferred | 1–2 |
| P01-T04 | Workspace scaffold `workbench/` (7 crates per Tech §4; only `domain` has content), toolchain pin, lints, frozen interface stubs, lockfile, check script; baseline commit B0 | Lead | None | 1–1.5 |
| P01-T05 | Typed IDs + UTC timestamp newtypes, validation, serde, tests | W1 (wave 1) | None | 1–1.5 |
| P01-T06 | `RedactedText`/`SanitisedText` newtypes; no raw construction/deserialisation; `compile_fail` doctests | W2 (wave 1) | None | 1–1.5 |
| P01-T07 | Run/check/approval state machines (Arch §7) with exhaustive transition tests | W3 (wave 1) | None | 1–1.5 |
| P01-T08 | App contract v1.0 command/event DTOs (Arch §6), reserved → `OperationFailed{code="deferred"}` (T01 §5.5 supersedes `OperationNotAvailable`), delivery classes, round-trip tests | W1 (wave 2) | None | 1.5–2 |
| P01-T09 | Integration; fmt/clippy `-D warnings`/test/build; cargo-audit if installed; reviewer + security-reviewer; evidence | Lead | None | 1–1.5 |
| P01-T10 | P01 report, freeze update, status/handover; stop for acceptance | Lead | None | 0.5–1 |

**Progress (2026-10-26):** T01–T10 done; CA-03 applied; **P01 accepted by the owner 2026-10-26** (`evidence/P01/P01-report.md`); **merged into `main` 2026-10-26** (`--no-ff` merge `7324da6` of `phase/P01-foundation` at `5a3e4ed`). Debug cycles P01-D01–D06 (§9).

P01 total about 10–15 h hands-on, against the §7.1 estimate of 6–10 h. The difference is the request, dependency and cargo-audit work. Confidence is low.

## 5B. P02 — JSON store v1.4, validation, writer, recovery and CLI skeleton (planned 2026-10-26; approved for execution 2026-10-26, A1–A10 = recommended options; **accepted and merged 2026-10-29**)

Prepared by Opus at owner request from Arch §3/§4/§6/§11/§12, Tech §4/§13/§14/§22, Data §2/§4/§6/§11–§13/§15/§17–§19/§24, PRD RES-03/RES-04/RES-05, UI §17, this plan (§4, §4.1, §5.2) and `status.md`/`handover.md`. **Owner approval 2026-10-26:** "I approve all recommended options, commit and push in a new phase branch" — A1=1a, A2=2a, A3=3a, A4=4a, A5=5a, A6=6a, A7=7a, A8=8a, A9=9a, A10=10a. Phase branch `phase/P02-store` created from `main` (`4a300cb`). Network steps stay limited to what A2/A8 name (cargo-audit stage C-A; offline resolution); anything else still needs separate approval. **Progress (2026-10-27):** T01 approved with T01-D1–D12 = recommended (`planning/P02-T01-request.md`); cargo-audit C-A PASS; T02 PASS; T03 CA-04 approved and applied; T04 B0 PASS (`a9ea9ce`, pushed); wave 1a (T05a+T07) B1a PASS 497/32 (`d73d807`, pushed, `evidence/P02/wave1a/record.md`); P02-D03 PASS 523/32 (O1–O10 + Q-T04-1, pushed); wave 1b (T05b+T06) + P02-D04 PASS 603/32; P02-D05 + prep PASS 627/32; **wave 2 (T08+T09+T10) + P02-D06 PASS 735/32, close-out `evidence/P02/wave2/record.md`**. Then wave 3 (T11–T13, B3 `49b8cba`, B3b `bf6130b`), T14 + D07/D08 (B4 `0cc562e`) and T15 (`84fedd4`), all pushed. **Closed 2026-10-29:** owner accepted P02 ("I accept p02, commit and push to the phase branch, merge to the main branch"); `--no-ff` merge `4747214` into `main`, check on `main` PASS 866/0/0, 39, cli 48; `main` pushed. Report `evidence/P02/P02-report.md`, freeze `evidence/P02/P02-freeze.json`.

### 5B.1 Scope
**In P02** (plan §4 P02 row, D-17, FR-15/FR-16/FR-31, P01 B1 decision 2):
- Typed Rust models for all ten store roots of `schemas/workbench-store-v1.4.schema.json` (Config, Repository, Workflow, Approval, Log, Suppression (reserved; read/validate only, never written), RagIndex, Conversation, Report, ToolManifest) and their 63 `$defs`, with FR-31 safe-text field types. *(Corrected 2026-10-27, P02-T01-D2: only 57 of the 63 `$defs` are reachable from the store roots; the six wrapper-protocol defs (WrapperRequest, WrapperControl, TerminationRecord, StreamRecord, DescendantCleanup, TerminationPolicy) are modelled in P06, and `Check` references none of them. FR-31 covers 104 plain-string properties, Appendix A `planning/P02-T01-fr31-classification.md`.)*
- Schema validation against the embedded canonical 1.4 schema (FR-15: `format` assertions on, external `$ref` compiled out, unavailable schema → `schema_unavailable`, never valid) and version gate (unknown/newer `schema_version` rejected).
- Semantic validation of Data §11 rules 1–25 and §18 rules 1–18 as pure functions over the models, each rule with an ID and positive/negative tests.
- Persistence protocol (Data §12, Arch §11): exclusive `.app.lock`, single serialised writer, validate → same-directory temp → flush → replace → directory sync, recovery copy; startup reconciliation (leftover temp files, log ↔ `backup/` report IDs, abandoned runs → `interrupted`, pending approvals stay pending).
- Report naming and export manifests (Data §13): `YYYYMMDDTHHMMSSZ_<slug>_<run-id>`, bounded slug, same-second collision check, independent `json_written`/`md_written`/`pdf_written`. P02 writes manifests and JSON only; Markdown/PDF generation is P10–P12.
- Event envelope (B1 decision 2): `event_id`/`at`/`level` added at emission; mapping `AppEvent` → store `Event` record; emission-time validation (e.g. `RunFinished` only with a terminal state).
- D-17 CLI skeleton (§4.1): `workbench/crates/cli`, binary `pwt-workbench-cli`, `--version`, `help`, `features` + feature index and staleness test, runner `cli` mode, `store init|validate|show|recover` (labelled `dev`, since Arch §6 has no store commands); scripted scenarios `tests/p02_*.rs`.

**Not in P02:** permission scope matching and grant consumption (P03; P02 checks only approval record shape and static invariants), credentials (P03), Git/inventory (P04/P05), process supervision and real redactor (P06), Markdown/PDF rendering (P10–P12), RAG content (P17), migrations from 1.0–1.3 (no such stores exist; Data §19), retention/compaction (open policy, Data §16), the TUI.

### 5B.2 Facts established for this plan (read-only, 2026-10-26)
| Fact | Basis |
|---|---|
| Workspace has 7 crates; `persistence` and `application` are empty stubs; `domain` holds ids, time, text, vocab, states, contract v1.0, versions | `workbench/crates/*/src` |
| Canonical schema has 10 roots in `oneOf` and 63 `$defs` | `schemas/workbench-store-v1.4.schema.json` |
| The P00 T06 graph (jsonschema 0.60.1 defaults off + serde_json + saphyr-parser) is 88 packages incl. regex, fancy-regex, referencing, uuid-simd, getrandom, num-*; built in P00 from the isolated cache | `spikes/rust/Cargo.lock`, `evidence/P00/P00-T06/` |
| `sha2` 0.11.1, `tempfile` 3.27.0, `uuid` 1.29.0, `getrandom` 0.4.3 are in the P00 lockfile; sha2/tempfile are **provisional** (no spike exercised them) | `spikes/p00-freeze/crates.md` §3 | *(Corrected 2026-10-27, P02-T02: the sha2 0.11.1 chain was built as a dependency in the accepted T07/T08 graphs; no spike called its API.)*
| Runner `dev/p01_check.py` modes: env, fmt, clippy, test, build, static, all; floors in `dev/p01_baseline.json` (416/32) | `dev/p01_check.py` |
| Rust 1.97.1 std provides `File::lock`/`try_lock` (stable since 1.89) and `sync_all`; exclusive lock, temp+replace and directory sync need no extra crate on Linux | toolchain version; to be confirmed by a T04 build |
| cargo-audit not installed; stage C-A recommended before P02, not yet approved | `status.md`, `planning/P01-T03-cargo-audit-request.md` |

### 5B.3 Tasks
Waves follow §3.8: up to three `sonnet-implementer` workers, disjoint owned files, own worktree/branch from a common baseline, lead integration and verification after each wave.

| Task | Content | Owner | Network/installs | Hours |
|---|---|---|---|---|
| P02-T01 | P02 request: module layout, frozen interfaces (model module map, `StoreWriter`, `FsOps` failure-injection seam, validation result types, envelope struct, CLI command table), FR-31 classification method, version/migration policy, durability policy, test plan, packets, debug bounds, decisions | Lead | None | 1.5–2 |
| P02-T02 | Dependency request: `jsonschema` 0.60.1 defaults off (graph as accepted in T06), `sha2` 0.11.1 (report/manifest digests); offline resolution from the isolated cache; lock delta, build-exposure review, RustSec screening (cargo-audit if C-A–C-E approved and done) | Lead | Option-dependent (A2/A3) | 1–2 |
| P02-T03 | Contract amendment CA-04 draft: envelope fields vs Data `Event` def, D-17 CLI in Tech §4, Arch §3/§4, UI §17; contract-version effects (expected: app contract minor or none, store 1.4 unchanged) | Lead | None | 0.5–1 |
| P02-T04 | Scaffold: `persistence` module skeleton with frozen stubs, `crates/cli` + binary stub, workspace manifest, lockfile, runner `dev/p02_check.py` (P01 runner + `cli` mode + P02 floors + extended static allowlist hook), baseline **B0** | Lead | None | 1.5–2 |
| P02-T05 | Wave 1 — models A: shared value `$defs` (Id, Digest, GitOid, RelativePath, Timestamp via domain types), Config, Repository/FileRecord, ToolManifest/LicenseRecord, Approval/InferenceScope, Workflow/Schedule/Selector/RepoTarget/ScanSettings; round-trip against Data §8/§9 examples | W1 | None | 1.5–2.5 |
| P02-T06 | Wave 1 — models B: Log/RunSummary/Event, Report/Check/Finding/Artifact/SourceRef/ReportManifest/Engine/Target/Plan snapshots, TerminationRecord/StreamRecord/WrapperControl subset used by Check, Suppression (read-only), RagIndex/Chunk, Conversation/Message/ChatContext, Provider/ProviderPolicy/AIEnrichment | W2 | None | 2–3 |
| P02-T07 | Wave 1 — atomic file layer: `.app.lock` (exclusive, non-blocking, second instance → typed error), same-directory temp, flush, replace, directory sync, recovery copy, data-dir path confinement, `FsOps` seam for injected interruption/ENOSPC/EACCES | W3 | None | 1.5–2 |
| P02-T08 | Wave 2 — schema validation: embedded 1.4 schema (checksum-pinned), FR-15 settings, version gate, root discrimination by `store_type`, FR-16 fixtures J11–J13/J16 + differential check, typed errors with JSON pointer (no value echo) | W1 | None | 1.5–2 | **Done 2026-10-27** (W1 `p02-t08-w1`, per-worktree 660/0/0): `schema.rs`, `tests/schema.rs`, `tests/fixtures/store/**` (33 tests) — integrated B2, uncommitted |
| P02-T09 | Wave 2 — semantic validation: Data §11 rules 1–25, §18 rules 1–18; rule-ID table marking each rule implemented / store-level part only / producer phase; positive + negative test per rule | W2 | None | 2–3 | **Done 2026-10-27** (W2 `p02-t09-w2`, per-worktree 642/0/0): `semantic/{rules,ids,workflow,approval,evidence,ai}.rs` (44 IDs), `tests/semantic.rs` (15 tests) — integrated B2, uncommitted |
| P02-T10 | Wave 2 — writer and recovery: `StoreWriter` (serialised, validate → write, storage errors never swallowed, Data §11 rule 25), startup reconciliation, report naming + collision check, export-manifest status updates | W3 | None | 2–2.5 | **Done 2026-10-27** (W3 `p02-t10-w3`, per-worktree 680/0/0): `writer/recovery/report_name/atomic.rs`, `tests/{writer,recovery,report_name}.rs` (53 incl. 1 doctest) — integrated B2, uncommitted |
| P02-T11 | Wave 3 — envelope and emission: `application` emitter adding `event_id`/`at`/`level`, UUID v4 + UTC clock behind injectable traits (deterministic in tests), `AppEvent` → `Event` mapping for every pilot event, emission-time state validation | W1 | None | 1.5–2 | **Done 2026-10-28** (W1 `p02-t11-w1`, lead integrated 765/0/0, 32): `application/src/emit.rs`, `application/tests/emit.rs` (30 tests) — on B2 `2448abf`, uncommitted (`evidence/P02/T11/record.md`) |
| P02-T12 | Wave 3 — CLI: argument parser (std only), `--version`, `help`, `features`, `--json` JSON Lines, `--data-dir` required, exit-code table, `store init|validate|show|recover`, feature index + staleness test, scenarios `tests/p02_*.rs` | W2 | None | 2–3 | **Done 2026-10-28** (W2 `p02-t12-w2`, lead integrated 805/0/0, 32, cli 34; first `--final` PASS): `application/src/dev_store.rs`, `cli/src/*`, `cli/tests/p02_scenarios.rs` (40 tests) — uncommitted (`evidence/P02/T12/record.md`) |
| P02-T13 | Wave 3 — FR-31 enforcement: classification table for every plain-string schema property, schema-completeness test, sealed `RedactedText` read-back path in `persistence`, compile_fail guards; runner static-allowlist extension is lead-owned (done in T14) | W3 | None | 1.5–2 | **Done 2026-10-28** (W3 `p02-t13-w3`, lead integrated `--final` 840/0/0, 39 compile_fail): `domain/src/store/classes.rs` (105 rows, 7 guards), `domain/tests/fr31.rs` (21 tests); read-back stays in `domain::text::store_read` per T01-D1 — on B3 `49b8cba`, uncommitted (`evidence/P02/T13/record.md`) |
| P02-T14 | Integration: merge waves, runner static extension with negative controls, fixtures (interrupted write, malformed JSON, ENOSPC, EACCES, stale lock, leftover temp, same-second reports, Unicode/hostile paths), full check, reviewer + security-reviewer, debug cycles | Lead | None | 2–3 | **Done 2026-10-28** (W1 fixtures + lead static extension/differential B; D07 F-1 fix, D08 flaky-test fix; reviews 0 high; `--final` 866/0/0, 39, cli 48) — committed locally as B4 `0cc562e` (`evidence/P02/T14/record.md`) |
| P02-T15 | P02 report, freeze update, status/handover; stop for acceptance | Lead | None | 0.5–1 | **Done 2026-10-29** (lead; `--final` 866/0/0, 39, cli 48, static 28/28; `evidence/P02/P02-report.md`, `P02-freeze.json`) — uncommitted, awaiting P02 acceptance |

Dependencies: T01 → T02, T03, T04; T04 → wave 1 (T05, T06, T07); wave 1 → wave 2 (T08 needs T05+T06; T09 needs T05+T06; T10 needs T07 + model stubs); wave 2 → wave 3 (T11 needs T06 Event; T12 needs T08–T10; T13 needs T05+T06); T14 → T15. Lead commits a baseline after each wave (B1 after wave 1, B2 after wave 2) as in P01. *(Revised 2026-10-27, P02-T01-D3: wave 1 split into 1a — W1 values/plan/provider (T05a) ∥ W3 fs/lock/atomic (T07), baseline B1a — and 1b — W1 config-side roots (T05b) ∥ W2 evidence-side roots (T06), baseline B1; wave 3 ends with baseline B3. Packets and ownership: `planning/P02-T01-request.md` §10.)*

**Estimate:** 22–34 h hands-on (§7.1 had 10–16 h + CLI 2–3 h; P01 ran 10–15 h against 6–10 h, and P02 covers 10 roots and 43 semantic rules). Confidence low; §7.1 to be updated at P02 approval. *(Revised at T01, 2026-10-27: 23–35.5 h, low confidence — +0.5–1 h for B1a, +0.5 h for the domain additions.)*

### 5B.4 Acceptance criteria (proposed)
1. fmt, clippy `-D warnings`, test, build PASS through `dev/p02_check.py all --final`; P01 floors not lowered; new P02 floors committed.
2. Every store root: valid examples (Data §8–§10 plus generated minimal/maximal records) pass schema **and** typed round trip; each root has at least one schema-negative and one version-negative fixture (newer version, 1.3 label).
3. FR-15 and FR-16 (J11–J13, J16) pass; a fixture with an external `$ref` never triggers a fetch and never validates.
4. Every Data §11/§18 rule appears in the rule table; every rule marked implemented has passing positive and negative tests; deferred parts name their phase.
5. Recovery: injected interruption at each write step leaves the last validated copy readable; malformed/foreign files are reported, never overwritten; ENOSPC/EACCES produce `StorageFailure` and stop further writes; abandoned runs become `interrupted`; pending approvals are never turned into grants.
6. Two reports in the same second for the same repo get distinct names; slugs are bounded and never raw paths.
7. FR-31 (1)–(3) met, including the schema-completeness test and its negative control.
8. Envelope: every pilot `AppEvent` maps to a schema-valid `Event`; non-terminal `RunFinished` and similar are rejected at emission.
9. CLI: §4.1 definition of done met for every P02 feature; `dev/p02_check.py cli` runs all scenarios headless; no command reads the real app-data dir; no secret appears in any CLI output fixture.
10. reviewer + security-reviewer: no unresolved high findings; medium findings fixed or accepted by the owner.
11. cargo-audit result recorded, or "not run" with the reason (as in P01 criterion 8).

### 5B.5 Risks
- jsonschema graph is large (88 packages, regex engines, build scripts); exposure reviewed by pattern only, no filesystem sandbox (as P00/P01).
- Rename/directory-sync durability is tested on this Linux host and filesystem only; no crash-proof or multi-file ACID claim (Data §12).
- ENOSPC is simulated through the `FsOps` seam; a real small-tmpfs test inside `unshare -rm` is optional (A5).
- Semantic rules that depend on later producers (approvals, runs, findings) are tested on synthetic records only until those phases exist.
- Worker velocity: models B (T06) and semantic rules (T09) are the largest tasks; split further if a wave-1 report shows overrun.

### 5B.6 Owner decisions needed (recommended option first)
| # | Decision | Options |
|---|---|---|
| A1 | Scope | **1a** §5B.1 as written (T01–T15); 1b defer RagIndex/Conversation models to P17 (saves ~1 h; FR-31 completeness then covers only written roots) |
| A2 | Dependencies | **2a** jsonschema 0.60.1 (defaults off) + sha2 0.11.1, offline from the isolated cache, every lock entry equal to P00; stop if anything is missing; 2b also `uuid`/`getrandom` instead of std `/dev/urandom` for IDs |
| A3 | Build exposure | **3a** accepted for an offline netns build if every build script/proc-macro was in an accepted P00 graph; else stop and report |
| A4 | Version policy | **4a** accept 1.4 only; reject everything else with `unsupported_version` (no 1.3 stores exist); 4b also read 1.3 read-only |
| A5 | Durability tests | **5a** `FsOps` injection + real rename/sync on this host; 5b additionally a real ENOSPC test on a small tmpfs in `unshare -rm` |
| A6 | Workers | **6a** up to 3 `sonnet-implementer` workers per wave in worktrees, local commits on own branches (as P01 A6) |
| A7 | Commits/pushes | **7a** lead commits at B0/B1/B2/report, each with non-force push of `phase/P02-store`; merge to `main` separate |
| A8 | cargo-audit | **8a** approve stage C-A before T02; 8b keep deferred, criterion 11 = "not run" |
| A9 | Debug cycles | **9a** pre-authorise debug cycles for anomalies the lead finds after a worker task, bounds as P01 §9 (two attempts per defect, then owner) |
| A10 | CA-04 | **10a** draft in T03, apply only after owner approval, before wave 3 |

## 5C. P03 — permission evaluator, child-environment sanitiser, session credentials and CLI (planned 2026-10-29; **C1–C10 = recommended options recorded 2026-10-29**; **accepted and merged 2026-11-01**, see progress note below and `evidence/P03/P03-report.md`)

Prepared by Opus at owner request ("plan p03 into multiple subtasks") from plan §4 (P03 row), §4.1 (P03 CLI row), §5.2 (FR-18–FR-24, FR-33), PRD §8 and KEY-01, Tech §11 and §20, Arch §6 (`ResolveApproval`, `RevokeApproval`, `ConfigureCredential`, `Approval*` and `CredentialStatusChanged` events), Arch §7 (approval state machine) and §10, Data §11 rules 12–17 and 23, UI §10, the P01 report §8 (KEY-01 hardening carried to P03) and the P02 report §8. Phase branch `phase/P03-permissions-credentials` was created from `main` `01a3cbd` to hold this plan. **Owner 2026-10-29: "update plan with the recommendations"** — C1=1a, C2=2a, C3=3a, C4=4a, C5=5a, C6=6a, C7=7a, C8=8a, C9=9a, C10=10a are recorded as the plan's decisions (§5C.6). Network, installs, new crates and anything outside these decisions still need separate approval.

**Progress (2026-10-31):** T01–T03 done (no CA-05; `application → sha2` edge only); T04 B0 `5e29908`; **T01–T04 accepted by the owner 2026-10-30**. Wave 1 (T05 ∥ T06 ∥ T07, owner go 2026-10-30) integrated as B1 `b1ca4cb`; review findings fixed in debug cycle P03-D01 `b9fdc0e` (§9); lead check PASS 1005/0/0, 41 compile_fail, floors 1005/41 (`evidence/debug/P03-D01/check-lead`). D01 adds binding requirements for T08 (workflow-scope grants with `run_id = null`; user-facing destinations via `canonical()`; only `approvals.rs` acts on `Permitted`; run-grant end-of-run expiry; deferred capabilities rejected) — `evidence/debug/P03-D01/record.md`. cargo-audit 0.22.2 installed under the owner's install grant (workbench lock: 0 advisories). Wave 2 (T08 W1 ∥ T09 W3, owner go 2026-10-31; request §5.4 amended for D01, packets `planning/P03-packets/{_wave2-common,T08-W1,T09-W3}.md`) integrated as B2: lead check PASS 1053/0/0, 41 compile_fail, floors 1053/41 (`evidence/P03/B2/record.md`). Wave 3 (T10 W2 ∥ T11 fixtures W1 + lead rule 12) integrated as B3 `f63c39a`; debug cycle P03-D02 (+ rule 13) `091224b`; T12 final check `all --final` PASS **1138/0/0, 41 compile_fail**, cargo-audit 0 advisories, FR-33 real-process check PASS; report `evidence/P03/P03-report.md`, freeze `evidence/P03/P03-freeze.json`. **P03 accepted by the owner 2026-11-01 and merged into `main`.** Live state: `status.md`.

### 5C.1 Scope
**In P03:**
- **Permission requests and fingerprints.** A typed `PermissionRequest` per Arch §10 (capability, operation summary, arguments fingerprint, workflow ID/revision, run ID, target repos, destinations/resources, data categories, purpose, requested lifetime). The canonical operation fingerprint is SHA-256 over a documented canonical encoding of the normalised operation. Per Data §11 rule 15, it binds a record to an operation; it is not authorisation.
- **Permission evaluator** (pure; owned by `application` per Tech §4). Given a request, the approvals store and the current workflow revision, it returns `Permitted(grant)`, `NeedsApproval(reason)` or `Refused(reason)`. The scope-match table covers capability, workflow ID/revision, run ID, repo set, destinations (normalised host/port/scheme/resource), resource constraints, data categories, fingerprint, lifetime, expiry and status. Rules:
  - Denied, revoked, expired and consumed grants never authorise (rule 13).
  - A changed workflow revision invalidates grants, even when the stale ID remains listed (rule 14).
  - Raw secrets are a prohibited data category under any grant (rule 16).
  - There are no wildcard or "all network" grants (A-08).
- **Approval lifecycle** over the P02 `StoreWriter`:
  - Record a pending request, then resolve it as approve once, approve workflow scope (saved workflow and visible revision only, UI §10) or deny. Revoke and expire as well.
  - Once-grants are consumed atomically before use and reconciled after an interruption (rule 12).
  - Pending requests are never turned into grants, and denials are persisted without sensitive payloads (Tech §11).
  - Events `ApprovalRequested`, `ApprovalResolved` and `ApprovalRevoked` go through the P02 emitter.
  - Only an explicit command with an actor can approve. Repository text or a chat answer cannot (Arch §10, UI §10).
- **Child-environment sanitiser** (no process spawning; that is P06):
  - Builds a child environment from an empty base plus a per-tool allowlist profile, with app-owned `HOME`/`XDG_*`/`TMPDIR` locations per tool.
  - Strips model keys (`OPENROUTER_API_KEY` and any variable holding the session credential) and unrelated secrets, whatever the allowlist says. Unrelated secrets are matched by a name-pattern deny list and a value check against the session credential.
  - Profiles: `git`, `yamllint`, `gitleaks`, `checkov` (P00-T09 rev 2 variables), `actionlint`, `kubeconform`, `internal`.
  - Output is a closed type that P06 must use to spawn. A masked preview is available for the CLI.
- **Session credential abstraction (KEY-01, session-only):**
  - A `CredentialStore` trait with a session (in-memory, process-lifetime) implementation.
  - Statuses `absent`, `present`, `invalid_format` and `unavailable` (FR-21 outcome names, reused for later modes).
  - Format validation only; no provider call (P16).
  - Replace and delete; `CredentialStatusChanged` events never carry key material; config holds only a reference (rule 23).
  - KEY-01 hardening carried from P01: best-effort clearing of `CredentialSecret` and `RawInput` on drop with std only, length not revealed by `Debug`, and the key never in argv, environment, logs or store files.
- **FR-33:** when euid is 0, refuse a data directory not owned by root (typed error; CLI exit code per the table).
- **CLI increment (§4.1):**
  - `approvals list`, `approvals approve <id> [--scope once|workflow]`, `approvals deny <id>`, `approvals revoke <id>`.
  - `credential set --session` (stdin only), `credential status`, `credential delete`.
  - `dev env-preview <tool>` (values masked).
  - Feature-index rows and scenarios `cli/tests/p03_*.rs`.
- **Semantic registry:** narrow the `rest_in` text of D11-12, D11-13 and D11-16/17 to what P03 now enforces at use time. The remaining parts go to P16/P17, and the status-count test changes with owner-visible arithmetic.

**Not in P03:**
- OS credential-store mode, FR-18–FR-23 helper and IPC, and any keyring dependency (C2).
- Validating a key against OpenRouter, and HTTP of any kind (P16).
- Git operations, which create the first real requests (P04).
- Spawning processes with the sanitised environment (P06).
- The real redactor and disclosure redaction before transmission (P06/P16/P17).
- The TUI approval dialog (P15).
- Redirect validation at request time (P16).
- Scheduling triggers (P14).

### 5C.2 Facts established for this plan (read-only, 2026-10-29)
| Fact | Basis |
|---|---|
| `domain::store::approval::Approval` already models status, capability, workflow ID/revision, run ID, repos, purpose, fingerprint (`Digest`), destinations, resource constraints, data categories, lifetime, timestamps, actor, `consumed` and inference scope | `workbench/crates/domain/src/store/approval.rs` |
| Contract v1.1 already has `ResolveApproval{decision}`, `RevokeApproval`, `ConfigureCredential{action}`, `ApprovalDecision`, `ApprovalLifetime`, `CredentialStatus`, events `ApprovalRequested/Resolved/Revoked`, `CredentialStatusChanged` and the `CredentialSecret` guards (not Clone/Serialize/Display/PartialEq/Hash) | `workbench/crates/domain/src/contract.rs` |
| Approval state machine `pending → approved/denied/cancelled`; grants later revoked or expired | `domain/src/states.rs`, Arch §7 |
| Semantic rules D11-12, D11-13, D11-16, D11-17 are `Partial` with the rest in P03 (D11-17 shared with P16) | `persistence/src/semantic/rules.rs` |
| `sha2 =0.11.1` is a workspace dependency used by `persistence` only; giving `application` a direct edge changes `Cargo.lock` edges, not the registry set | `workbench/Cargo.toml`, `Cargo.lock` |
| No `zeroize`/`secrecy` crate in the offline cache; `keyring 4.2.0`, `zbus-secret-service-keyring-store` are cached from P00-T07 (spike-only advisory exception) | `~/.local/share/pwt-dev-tools/_cargo-home/registry/cache/` |
| `unsafe_code = "forbid"`: no-echo TTY input (termios) is not available without `unsafe` | `workbench/Cargo.toml` |
| P02 runner `dev/p02_check.py` and floors 866/39 are the verified starting point | `evidence/P02/P02-report.md` |

### 5C.3 Tasks
Waves follow §3.8: up to three `sonnet-implementer` workers, disjoint owned files, each in its own lead-prepared worktree from a common baseline, with lead integration and verification after each wave. File ownership is final in T01.

| Task | Content | Owner | Network/installs | Hours |
|---|---|---|---|---|
| P03-T01 | P03 request: module layout, frozen interfaces (`PermissionRequest`, canonical fingerprint encoding, `Evaluation`, `ApprovalService`, `ToolEnvProfile`/`ChildEnv`, `CredentialStore`, CLI command/exit table), scope-match table spec (dimension × outcome), env allowlist/deny policy per tool, test plan, packets, debug bounds, decisions | Lead | None | 1.5–2 |
| P03-T02 | Dependency/lock check: no new crates (C3); manifest edges (`application → sha2`); lock delta = edges only, registry set unchanged; FR-24 re-screen recorded as not applicable while C2 = session-only | Lead | None | 0.5 |
| P03-T03 | Contract check and CA-05 draft only if needed (e.g. `ApprovalDecision` scope variant, `CredentialStatus` values, dev CLI commands in Data §25); expected: app contract 1.1 → 1.2 or unchanged, store 1.4 unchanged | Lead | None | 0.5–1 |
| P03-T04 | Scaffold: frozen stubs (`application::{permission,approvals,child_env,credential}`), runner `dev/p03_check.py` (P02 runner + P03 floors + static hooks), baseline **B0** | Lead | None | 1–1.5 |
| P03-T05 | Wave 1 — permission request, canonical fingerprint and evaluator (pure), scope-match table tests over every dimension (positive + negative), host normalisation (case, trailing dot, port defaults, IDN/punycode rejected or normalised per T01, userinfo rejected) | W1 | None | 2–3 |
| P03-T06 | Wave 1 — child-environment sanitiser: profiles, empty base, allowlist, deny patterns, value check against the session credential, app-owned HOME/XDG/TMP, masked preview, env-strip tests (seeded keys under many names, multi-line values, `=` in values, duplicate names, non-UTF-8 names) | W2 | None | 1.5–2 |
| P03-T07 | Wave 1 — session credential store and KEY-01 hardening: trait, session implementation, statuses, format validation, replace/delete, best-effort clearing, `Debug` without length, compile_fail guards, events without key material | W3 | None | 1.5–2 |
| P03-T08 | Wave 2 — approval lifecycle over `StoreWriter`: request → pending, resolve (once / workflow scope / deny), revoke, expiry, workflow-revision invalidation, atomic once-grant consumption before use + reconciliation after interruption, events; semantic registry `rest_in` updates | W1 | None | 2–3 |
| P03-T09 | Wave 2 — FR-33 euid-0 refusal (persistence data-dir check, typed error, CLI exit mapping) with tests through a uid seam (real root not required) | W3 | None | 0.5–1 |
| P03-T10 | Wave 3 — CLI: `approvals …`, `credential …` (stdin only; TTY stdin refused, C5), `dev env-preview <tool>`, feature-index rows + staleness test, scenarios `cli/tests/p03_*.rs` (scope/revision mismatch asks again; key never in stdout/stderr/argv/store files) | W2 | None | 2–3 |
| P03-T11 | Integration: merge waves; runner static extension (env reads only in the sanitiser and allowlisted CLI input path; `CredentialSecret::expose` sites; no `std::process::Command` before P06); hostile fixtures (stale revision, changed destination, expired, consumed, denied, revoked, raw-secret category, wildcard/lookalike hosts); seeded-key sweep over every output and store file; reviewer + security-reviewer; debug cycles | Lead (+W1 fixtures) | None | 2–3 |
| P03-T12 | P03 report, freeze, status/handover; stop for acceptance | Lead | None | 0.5–1 |

Dependencies: T01 → T02, T03, T04; T04 → wave 1 (T05 ∥ T06 ∥ T07); wave 2: T08 needs T05 (+ P02 writer), T09 independent; wave 3: T10 needs T06–T09; T10 → T11 → T12. The lead commits a baseline after each wave (B1, B2, B3) on the phase branch, as in P02.

**Estimate:** 16–24 h hands-on (low confidence). §7.1 had 8–12 h + CLI 1–2 h. P02 ran at about twice its first estimate.

### 5C.4 Acceptance criteria (proposed)
1. fmt, clippy `-D warnings`, test and build pass through `dev/p03_check.py all --final`. The P02 floors (866/39) are not lowered, and new P03 floors are committed.
2. Scope-match table: every dimension has at least one positive and one negative case. Denied, revoked, expired and consumed grants never authorise. A changed workflow revision, destination, data category, repo set or fingerprint needs a new approval. Raw secrets are refused under every grant, and no wildcard grant exists.
3. Fingerprint: the canonical encoding is documented and pinned by golden vectors. Equal operations give equal fingerprints, and any relevant change gives a different one. No code path treats the fingerprint as authorisation by itself.
4. Lifecycle: a once-grant is consumed exactly once, including across an injected interruption at every write step (reconciliation never double-authorises and never silently re-grants). Pending requests are never treated as grants. Every transition emits a schema-valid event, and only explicit commands approve.
5. Child environment: built from an empty base, with no variable inherited outside the profile. Seeded keys under any name, and any value equal to the session credential, are absent from every profile's output. The preview masks values.
6. Credentials: session-only; no plaintext persistence anywhere (seeded-key sweep over store files, logs, stdout/stderr and argv); statuses as specified; replace/delete work; no key material in events or `Debug`. OS-store mode is absent and its FR-18–FR-24 status is recorded.
7. FR-33: with euid 0 (seam), a non-root-owned data directory is refused with the typed error and the documented exit code; the root-owned case still works.
8. CLI: the §4.1 definition of done is met for every P03 feature; `cli` mode runs all scenarios headless; no command reads the real app-data directory or a real environment secret.
9. reviewer + security-reviewer: no unresolved high findings; medium findings fixed or accepted by the owner.
10. Dependency state recorded: no new crates, lock delta edges only (or the C3 alternative as approved); cargo-audit status stated.

### 5C.5 Risks
- Host normalisation is easy to get subtly wrong (IDN, trailing dots, ports, case). Decision C6 keeps it narrow: reject what is not plainly ASCII.
- "Best-effort clearing" without `unsafe` or `zeroize` cannot guarantee the compiler keeps the write. It must be disclosed, never claimed as secure erase.
- Session-only credentials in a one-shot CLI process last only one invocation (C4). Scenarios must test within one process, and `status` in a new process must honestly report `absent`.
- The evaluator has no real callers until P04 (Git) and P16 (OpenRouter), so tests use synthetic requests only.
- Changing the semantic registry counts touches P02 tests. Arithmetic must be explicit, and no assertion may be weakened.

### 5C.6 Owner decisions (recommended option first; **chosen 2026-10-29: all recommended**)
| ID | Topic | Options |
|---|---|---|
| C1 | Scope | **1a (chosen)** §5C.1 as written (T01–T12); 1b defer the CLI `approvals` commands to P14 (saves about 1 h; breaks the D-17 rule) |
| C2 | Credential mode | **2a (chosen)** session-only; OS-store mode, FR-18–FR-23 and the keyring dependency stay out (record "not adopted"; FR-24 re-screen not applicable); 2b plan OS-store mode as a later P03b with its own approval |
| C3 | Dependencies | **3a (chosen)** no new crates; `application` gets a direct `sha2` edge (lock edges only); best-effort clearing with std only; 3b request `zeroize` (needs a network fetch and a new crate approval) |
| C4 | CLI credential lifetime | **4a (chosen)** `credential set --session` holds the key for that one invocation and says so; scenarios run set/status/delete inside one process via a scripted `--then` sequence or the test harness; 4b a background dev agent holding the key (rejected: new IPC surface) |
| C5 | CLI key input | **5a (chosen)** stdin pipe only; refuse when stdin is a TTY (no-echo input needs `unsafe` termios; masked entry is the TUI's job, P15); 5b read a TTY with echo (rejected: shows the key) |
| C6 | Destination normalisation | **6a (chosen)** ASCII-only hosts, lower-cased, one trailing dot removed, scheme default ports made explicit; userinfo, IP-literal shortcuts outside the allowlist and non-ASCII/punycode hosts rejected; 6b full IDNA processing (needs a crate) |
| C7 | Grant lifetimes | **7a (chosen)** `once`, `run` and `workflow` (with ID/revision) as in the schema; no time-only "session" grant; expiry checked against the injected clock; 7b add a session lifetime (contract change) |
| C8 | Contract changes | **8a (chosen)** CA-05 drafted in T03 only if the frozen interfaces need it; applied only after owner approval (as A10/CA-04); 8b no contract change allowed, interfaces adapted instead |
| C9 | Workers and commits | **9a (chosen)** up to three `sonnet-implementer` workers per wave in lead-prepared worktrees; lead commits B0/B1/B2/B3/report on `phase/P03-permissions-credentials` with non-force pushes; merge to `main` separate; 9b lead-only |
| C10 | Debug cycles | **10a (chosen)** pre-authorised, two attempts per defect, then owner (as P02 A9); 10b each cycle needs owner approval |

## 5D. CA-05 — automatic LLM evidence analysis (design direction accepted with qualifications 2026-10-31; implementation not approved, not started)

Change contract r2 (vocabulary, four separate dimensions, logical analysis vs attempts and authoritative selection, legacy `ai_enrichment` handling, AI-only findings out of the pilot, ANL-01–ANL-12, failure design, acceptance AC-01–AC-18, recorded decisions): `planning/CA-05-automatic-evidence-analysis.md`. Proposals for the open O-AN-2 limits and O-AN-6 evaluation, and a separate real-API evaluation request: `planning/CA-05-decision-proposals.md` (nothing ratified). Draft store 1.5 definitions: `planning/CA-05-draft/`, Data Schema §26. All implementation work below is **pending**; documenting the design does not complete it.

| Work | Phase | Owner | Pending decision |
|---|---|---|---|
| Foundational types/contract (domain analysis types, store 1.5, app contract 1.2, UI 1.2) | CA-05 lead task before P07 (proposed: start of P06); precedes all rows below | Lead | Execution approval |
| Evidence snapshot builder, digest, honest stage state without provider | P07 | W + lead | — |
| Excerpt redaction for snapshots | P06/P07 | W | — |
| Combined report sections, stage notices, export revisions | P11 | W | — |
| UI analysis tab, stage progress and failure states | P15a (labelled mocks) / P15b | W | — |
| Provider integration with structured output, application-owned retries | P16 | W | O-AN-2 limits |
| Automatic orchestration: attempts, authoritative selection, recovery, validation, idempotency | P16 (scope extension) | W + lead | — (pilot scope decided) |
| RAG reuse of the authoritative analysis | P17 | W | — |
| Safety/adversarial regression, model-quality evaluation, release gate | P19/P20 | Lead + owner | O-AN-6; real-API evaluation approval |

## 5E. P04 — repository registration, hardened Git acquisition/update, checkout safety and CLI (planned 2026-11-01; **E1–E14 = recommended options recorded 2026-11-01**; execution not started)

Prepared by Opus at owner request ("plan P04 with muliple sub-tasks with utmost details") from plan §4 (P04 row), §4.1 (P04 CLI row), §5.2, D-13, PRD REP-01–REP-06 and §16, Tech §5 and §11, Arch §3/§4/§6/§10/§11/§12, Data §2/§4/§11/§14 and the `Repository` model, the accepted P00-T04 Git-safety spike (`evidence/P00/P00-T04/record.md`, run 2 after D01), the P03 interfaces (`planning/P03-T01-request.md` §5–§7) and the P03 report §8. **Owner 2026-11-01: "I approve all recommended options, commit and push to phase branch"** — E1=1a, E2=2a, E3=3a, E4=4a, E5=5a, E6=6a, E7=7a, E8=8a, E9=9a, E10=10a, E11=11a, E12=12a, E13=13a, E14=14a are recorded as the plan's decisions (§5E.6). Phase branch `phase/P04-git-acquisition` created from `main` to hold this plan. Starting T01 still waits for an explicit owner go; network (including the E6 real-remote check), installs, new crates and anything outside these decisions still need separate approval.

### 5E.1 Scope
**In P04:**
- **Remote URL intake (REP-01, Tech §5.1–§5.2):** parse user input (`AddRepository { url: RawInput }`) into the store's credential-free `RemoteUrl` (`https://host[:port]/path`, `ssh://[user@]host[:port]/path`, scp-like `user@host:path`); reject embedded passwords/tokens, `file://`, `git://`, `ext::`, local paths, option-looking values (leading `-`), control characters, query/fragment; derive `CanonicalRemote` (duplicate detection), `Slug`, display name; map the remote to the P03 `NormalisedDestination` used by grants; refuse loopback, link-local (incl. `169.254.169.254`), private-range and `localhost` hosts by default (closes P03-D01 B5/H4 for Git; decision E13).
- **Registry (REP-01, REP-06):** register/list/status/remove in `repo.json` through the P02 `StoreWriter`; UUID identity; checkout path `checkouts/<repo-id>` (app-relative, confined); default ref (branch/tag/commit); checkout state machine `not_cloned → updating → ready | dirty | error`; `last_commit_oid`, `last_update_at`, `last_error_redacted`; events `RepositoryRegistered`, `RepositoryStatusChanged`, `RepositoryRemoved`.
- **Hardened Git runner (E2):** argument arrays only, explicit working directory, environment from the P03 `git` child-env profile plus the credential policy (E3), pinned Git binary path and version gate (E4), timeouts, bounded output, no shell, no interactive prompts, process-group termination on timeout.
- **Hardened clone (Tech §5.4, §5.8–§5.9, P00-T04 D01 procedure):** `clone --no-checkout --template=<empty app dir>` with the hardening `-c` options; create `.git/info/`, write `* -filter`, read it back exactly; resolve the target commit; list every path of that commit and require `check-attr filter` = `unset` for all; `checkout -f` the verified commit; require `HEAD` = verified commit; any failure aborts before checkout (fail closed) and leaves state `error`.
- **Hardened update (REP-02, REP-05, Tech §5.5–§5.7):** refuse in-progress merge/rebase/cherry-pick/bisect, tracked modifications, untracked files that an update would overwrite, and divergence; fetch; re-verify the attributes override and the target commit's paths; `merge --ff-only <verified oid>`; require `HEAD` = target. Never stash, reset, clean, force or resolve conflicts. A refused repository is skipped with a reason; other repositories continue.
- **Provenance (REP-06):** exact commit OID (SHA-1 or SHA-256 repositories), ref kind/value, Git version, time; captured after update and before any inventory (P05).
- **Locking (Tech §5.4, §10):** the existing `.app.lock` plus a per-repository exclusive lock so no two operations (and later no update/scan) overlap on one checkout.
- **Permissions (WF-03/WF-04, Arch §10):** clone needs a `git_acquire` grant, update a `git_update` grant, both through `ApprovalService::authorise` with the remote as destination (granularity E7) and data category `repository_metadata`; `NeedsApproval` → pending request + exit 6 in the CLI; denial → skipped.
- **Removal (Data §14):** deregister; optional `--delete-checkout` deletes only the confined, non-symlinked app checkout; `--purge-index` accepted (nothing to purge before P17); reports kept by default.
- **CLI increment (§4.1 P04 row):** `repo add <url>`, `repo list`, `repo refresh <id>`, `repo status <id>`, `repo remove <id> [--delete-checkout] [--purge-index] [--keep-reports]`; feature-index rows; scenarios `cli/tests/p04_*.rs`.
- **Hostile-repository fixtures:** hooks (template, `core.hooksPath`, planted), smudge and LFS-like process filters (incl. odd file names), submodule recursion, `ext::`/`file://` transport, `core.fsmonitor`, env-injected config, repository-local config that tries to add helpers/filters/`core.sshCommand`/`url.*.insteadOf`, symlinks pointing outside the checkout, path names with spaces/newlines/control characters — all ported from P00-T04 G01–G15 to Rust integration tests plus new ones.

**Not in P04:** file inventory, classification and selection (P05); the general process supervisor with output budgets and descendant cleanup (P06, FR-01/FR-02/FR-29) — P04's runner is a bounded Git-only precursor that P06 replaces; scanning (P07+); workflow-scope reuse of Git grants and scheduled refresh (P14); TUI (P15); submodules, Git LFS content, shallow/partial clone, sparse checkout, signed-commit verification, history (SCAN-09); proxies (E3); Windows/macOS Git (X5).

### 5E.2 Facts established for this plan (read-only, 2026-11-01)
| Fact | Consequence | Basis |
|---|---|---|
| The `repositories` crate is an empty placeholder (no dependencies) | P04 fills it; manifest edges only (`repositories → domain`, `application → repositories`); no new crate expected | `workbench/crates/repositories/{Cargo.toml,src/lib.rs}` |
| Store types already exist: `Repository` (remote_url, canonical_remote, local_path, default_ref_kind/value, registered_at, last_update_at, last_commit_oid, checkout_state, last_error_redacted, inventory fields), `RepositoryFile`, `CheckoutState` (5 values), `RemoteUrl` grammar (https/ssh/scp-like, no `@` in https authority, no query/fragment, no leading `-`), `CanonicalRemote`, `GitRefName`, `GitOid`, `AppRelPath` | No store-schema change expected for the registry; T03 confirms | `domain/src/store/{repository,values}.rs`, schema 1.4 |
| App contract 1.1 already has `AddRepository{url: RawInput}`, `RefreshRepository`, `RemoveRepository{delete_checkout, purge_derived_index, …}`, events `RepositoryRegistered/Removed/StatusChanged` with `RepoOperation{clone, update}` and `RepoOperationStatus{started, succeeded, failed, skipped}` | No contract change expected | `domain/src/contract.rs` |
| `Config` has no Git settings (no credential-helper field) | A persisted helper setting would be a store change (CA-06); E3 recommends reading the user's helper at run time instead | schema `$defs/Config` |
| P03 `git` child-env profile: app-owned `HOME`/`XDG_CONFIG_HOME`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`, `GIT_ATTR_NOSYSTEM=1`, inherits nothing; deny rules drop every `SSH_*`, `*_PROXY`, `GIT_ASKPASS`, `GIT_CONFIG_*` name | As accepted, the user's credential helper **and SSH agent are unavailable**, which conflicts with REP-03/D-13 ("credential mechanism preserved") → owner decision E3 and a P03 §7 policy amendment | `planning/P03-T01-request.md` §7, `application/src/child_env.rs` |
| P03 static rule: no `std::process::Command`/`Command::new` in `src` until P06 | P04 must spawn Git → owner decision E2 and a scoped static-rule change | `dev/p03_check.py` |
| `unsafe_code = "forbid"`: no `kill(-pgid)` via libc | Timeout termination of Git's helper children (`git-remote-https`, `ssh`) cannot use libc; GNU coreutils `timeout` signals the whole process group by default — verified locally 2026-11-01: grandchild killed by default, survives with `--foreground` (control) | local test; `timeout (GNU coreutils) 9.10` |
| P00-T04 accepted only for Git 2.53.0, this host, local `file://` fixtures; untested: real HTTPS/SSH, credential helpers over the network, `core.sshCommand`/`GIT_SSH`, proxies, `include`/`includeIf`, system config, diff/textconv drivers, other submodule entry points, attribute macros outside `$GIT_DIR/info/attributes` precedence | P04 tests these locally where possible (repo-local config, includeIf, textconv, macros) and needs a separately approved real-remote check (E6) | `evidence/P00/P00-T04/record.md` |
| Host: Git 2.53.0 `/usr/bin/git`, OpenSSH 10.3p1 | Version gate default (E4); whether OpenSSH resolves `~/.ssh` from the password database rather than `$HOME` (needed when Git runs with an app-owned `HOME`) is **not yet verified** — T01 verifies locally without network | `git --version`, `ssh -V` |
| Local fixtures need the `file` transport, which the product policy refuses | A test-only transport exception must not be reachable in release builds → decision E5 | P00-T04 `g_hard_file` |
| P03 `NormalisedDestination` accepts canonical dotted-quad IPv4 and any ASCII host incl. `localhost` | Git remotes need an extra refusal list (E13) | P03-D01 B5/H4 |
| Runner floors 1138/41; `dev/p03_check.py` static rules incl. 12/13 | P04 runner derives from it; floors start at 1138/41 | `dev/p03_baseline.json` |

### 5E.3 Tasks

**T01 checkpoint 2026-11-03:** owner explicitly approved T01 only and requested GPT-6.1 Terra sub-agents in branch sessions, with the primary session orchestrating/testing and no commits or pushes. Request prepared at `planning/P04-T01-request.md`, procedures at `planning/P04-T01-git-safety.md`, future unapproved packets at `planning/P04-T01-packets/`, evidence/report at `evidence/P04/T01/record.md`. Owner approved the initial-ref and provenance design ("okay i approve"); interfaces are frozen under request §10. T02–T12 remain unstarted and unapproved by the current instruction. No commit/push/merge authority is inferred from historical E9.

**Subsequent T02 checkpoint:** owner authorized T02. After an initial missing-cache failure, the owner requested a solution and approved fetching the locked artifacts. Full offline metadata and the all-target tree passed in scratch, with an edge-only lockfile delta and unchanged registry identities; see `evidence/P04/T02/record.md`. T02 passed its check; product manifests and lockfile remain unchanged. T03–T12 remain unstarted; stop for owner review.
Waves follow §3.8: up to three `sonnet-implementer` workers, disjoint owned files, lead-prepared worktrees from a common baseline, append-only coordination log, lead integration and verification after each wave. Ownership is fixed in T01.

| Task | Content | Owner | Network/installs | Hours |
|---|---|---|---|---|
| P04-T01 | P04 request: module layout; frozen interfaces (`RemoteSpec` intake and errors, `GitRunner` trait + real runner, `HardenedGit` ops and outcomes, `CheckoutLock`, `RepositoryService` in `application`, refusal reasons, CLI command/exit table); the exact hardened clone/update procedures step by step (from P00-T04 D01, with every abort point); credential policy per E3; transport/test policy per E5; refusal list per E13; local verification of the OpenSSH home behaviour and of `process_group` + `timeout` termination (no network); test plan; packets; debug bounds | Lead | None | 2–3 |
| P04-T02 | Dependency/lock check: manifest edges only (`repositories → domain`, `application → repositories`; dev-only feature per E5); offline resolution in a scratch copy; registry set unchanged; lock delta = edges only | Lead | None | 0.5 |
| P04-T03 | Contract check: confirm no store/app-contract change (registry, events, commands already present); document the per-repository lock path in Arch §11/Data §2 as a layout note; draft CA-06 only if a frozen contract must change (e.g. a persisted Git setting if E3 = 3b) | Lead | None | 0.5–1 |
| P04-T04 | Scaffold: frozen stubs (`repositories::{remote, git::{runner, ops}, lock}`, `application::repository_service`, CLI seams), P03 policy amendments approved under E2/E3 applied by the lead (child-env `git` profile, static rules), runner `dev/p04_check.py` (P03 runner + P04 static rules: `Command::new` only in `repositories/src/git/runner.rs`; no `sh`/`bash -c`; Git invoked only through `HardenedGit`; fixture transport unreachable in non-test builds; env reads unchanged), floors 1138/41, baseline **B0** | Lead | None | 1–1.5 |
| P04-T05 | Wave 1 — remote intake: `RawInput` → `RemoteUrl`/`CanonicalRemote`/`Slug`/display name/`NormalisedDestination`; rejection table (credentials in userinfo or path, schemes, options, control/non-ASCII, IP/loopback/private/link-local/`localhost` per E13, over-length); duplicate detection; ≥ 50 table cases; no echo of input in errors | W1 | None | 1.5–2 |
| P04-T06 | Wave 1 — Git runner: std-only bounded runner (argv, cwd, env from child_env + E3, `process_group(0)`, wall-clock timeout via coreutils `timeout` with kill-after per E2, stdout/stderr byte caps with reader threads, `--quiet`/no progress, no stdin), Git version probe and gate (E4), exit/stderr classification into fieldless error kinds without echoing URLs or helper output, fake runner for unit tests; tests: timeout kills helper grandchildren, output caps, env exactness (seeded-key sweep), version refusal | W2 | None | 2–3 |
| P04-T07 | Wave 1 — registry and locks: `repo.json` register/update/remove through `StoreWriter`; checkout path allocation and confinement (`checkouts/<repo-id>`, no symlink traversal, FR-33 data dir); per-repository lock (non-blocking, typed `Locked`); safe checkout deletion (refuse symlinks, paths outside `checkouts/`, mount points); state transitions; events | W3 | None | 1.5–2 |
| P04-T08 | Wave 2 — hardened clone and update (`HardenedGit`): the T01 procedures; tag and commit refs as verified detached checkouts (E8); provenance capture; integration tests on local fixtures (E5) porting P00-T04 G01–G15 plus new cases: repo-local config adding helpers/filters/`core.sshCommand`/`url.insteadOf`, `include`/`includeIf`, textconv/diff drivers, attribute macros, symlinks out of the tree, SHA-256 object format, empty repository, missing default branch, ref name injection (`--upload-pack`-like values, leading `-`) | W1 | None | 3–4 |
| P04-T09 | Wave 2 — repository service in `application`: `AddRepository` (validate, duplicate refusal, register, event), `RefreshRepository` (clone if `not_cloned`, else update; `git_acquire`/`git_update` request via `ApprovalService::authorise`; `NeedsApproval` → pending; denial/refusal → skipped with reason; state and provenance written; events started/succeeded/failed/skipped), `RemoveRepository` (retention choices); one repository's failure never stops another; no repository text reaches approvals | W2 | None | 2–3 |
| P04-T10 | Wave 3 — CLI: `repo add|list|refresh|status|remove` (§4.1), exit table (0 / 2 usage / 3 locked / 4 storage / 5 refused or failed / 6 approval needed), `--json` shapes, feature rows + staleness test, scenarios `cli/tests/p04_*.rs`: dirty/diverged/hook/filter/submodule fixtures unchanged and skipped with reason; clone only after approval (6 → approve → 0); duplicate add refused; remove with and without `--delete-checkout`; seeded-key sweep incl. a credential-helper canary | W3 | None | 2–3 |
| P04-T11 | Integration: merge waves; static rules; hostile fixtures (W1) over the integrated code; seeded-key and canary sweeps (no helper output, URL credentials or key in any output/store file); reviewer + security-reviewer (Git/process/credential focus); debug cycles; real-remote manual check if approved (E6) | Lead (+W1 fixtures) | E6 only, with separate approval | 3–4 |
| P04-T12 | P04 report, freeze, status/handover; stop for acceptance | Lead | None | 0.5–1 |

Dependencies: T01 → T02, T03, T04; T04 → wave 1 (T05 ∥ T06 ∥ T07); wave 2: T08 needs T06, T09 needs T05 + T07 + T08's frozen interface (T08 and T09 can run in parallel against the frozen `HardenedGit` trait with a fake); wave 3: T10 needs T09; T10 → T11 → T12. Lead commits B1, B2, B3 after each wave.

**Estimate:** about **20–29 h** hands-on (low confidence). §7.1 had 10–16 h + CLI 1–2 h; P02 and P03 each ran at roughly 1.5–2× their first estimate, and P04 adds the first real process execution and a credential-policy change.

### 5E.4 Acceptance criteria (proposed)
1. `dev/p04_check.py all --final` passes (fmt, clippy `-D warnings`, test, build, cli, static); floors not lowered below 1138/41; new floors committed.
2. Remote intake: every accepted form round-trips to `RemoteUrl`/`CanonicalRemote`; every rejection class has a test; no rejected input is echoed; duplicates refused; E13 refusals in force.
3. Git runner: argv only, no shell; environment exactly the approved profile (seeded keys and denied names absent; credential policy E3 exactly); timeout terminates Git and its helper grandchildren (test with a hanging helper); output capped; version gate enforced.
4. Hardened clone: P00-T04 controls hold in Rust for hooks (template, hooksPath, planted), smudge/process filters (incl. odd names), submodules, `ext::`/`file://`, fsmonitor, env-injected config; any setup or verification failure aborts before checkout and leaves state `error`; `HEAD` equals the verified commit.
5. Hardened update: dirty, untracked-incoming, in-progress and diverged checkouts are refused with the exact reason and left byte-for-byte unchanged (worktree and `HEAD` digests); fast-forward only; never stash/reset/clean/force.
6. Repository-local config cannot add helpers, filters, hooks, `core.sshCommand` or URL rewrites that take effect; `include`/`includeIf`, textconv and attribute-macro cases tested; any control that cannot be shown effective is documented and its operation refused.
7. Provenance: commit OID (SHA-1 and SHA-256 fixtures), ref, Git version and time recorded; reports in later phases can identify exactly what was assessed.
8. Permissions: clone and update happen only under a matching `git_acquire`/`git_update` grant through `ApprovalService::authorise`; a changed remote asks again; denial → skipped; no repository text can approve.
9. Locks and isolation: concurrent operations on one repository refused (`Locked`); one repository's failure does not stop others; checkout paths stay confined; removal never deletes outside `checkouts/<repo-id>` or follows symlinks.
10. CLI: the §4.1 definition of done for every P04 command; no command uses the real app-data dir or a real credential.
11. Credentials: no Git credential, helper output, URL secret or session key appears in any output, log, store file or event; the credential mechanism of E3 works on local fixtures (helper canary invoked with the expected protocol/host only).
12. reviewer + security-reviewer: no unresolved high; mediums fixed or accepted by the owner.
13. Dependencies: no new crates; lock delta edges only; cargo-audit status stated.
14. Real HTTPS/SSH behaviour: either verified by the approved manual check (E6) or listed as manual-pending with the reason.

### 5E.5 Risks
- **First real process execution.** Without the P06 supervisor, timeouts and descendant cleanup rely on coreutils `timeout` and process groups; helpers that call `setsid` escape the group (FR-29). Disclosed; P06 replaces the runner.
- **Credential policy (E3).** Preserving the user's helper/agent widens what Git can reach; reading global config at all re-opens config-inheritance paths that the spike did not test (`include`, `includeIf`). Mitigation: read only `credential.helper` values in a hardened, non-executing call, pass them explicitly, keep `GIT_CONFIG_GLOBAL=/dev/null` for operations.
- **Git version drift.** Controls were verified on 2.53.0 only; another version could change option semantics. Mitigation: version gate (E4).
- **Untested real-network behaviour.** Credential helpers, SSH host-key prompts (`GIT_TERMINAL_PROMPT=0`, `BatchMode`), proxies and redirects are untested until E6.
- **Malicious repository content.** Symlinks, huge files and odd names reach the checkout (inventory and reading are P05+); P04 must at least never follow symlinks during deletion or path checks.
- **Same-user process risk** (P03 §8): Git helpers run as the user and could touch the data dir; hooks/filters are disabled, but the P06 entry gate decision remains open.
- **Schedule.** +20–29 h widens the gap in §7.3 (B-08).

### 5E.6 Owner decisions (recommended option first; **chosen 2026-11-01: all recommended**)
| ID | Topic | Options |
|---|---|---|
| E1 | Scope | **1a** §5E.1 as written (T01–T12); 1b defer `repo remove --delete-checkout` (saves ~1 h; leaves checkouts on disk) |
| E2 | Git execution before P06 | **2a** bounded Git-only runner in `repositories/src/git/runner.rs` (std only: argv, `process_group(0)`, coreutils `timeout -k` for group termination, byte caps); static rule changed to allow `Command::new` only in that file; P06 replaces it; 2b pull the P06 supervisor core forward into P04 (+8–12 h, mixes phases); 2c postpone real Git until after P06 (breaks roadmap order) |
| E3 | Credential mechanism (REP-03/D-13 vs P03 `git` profile) | **3a** keep global/system config untrusted for operations; amend the P03 `git` profile to inherit `SSH_AUTH_SOCK` only (agent socket path, no secret; still subject to the key-value probe); read the user's `credential.helper` values once per operation with a hardened read-only `git config --global --get-all credential.helper` (real `HOME`, no execution) and pass them as `-c credential.helper= -c credential.helper=<value>`; proxies unsupported in the pilot (documented); 3b let Git read the real global config (wider untested surface); 3c anonymous HTTPS + SSH agent only (no helper; private HTTPS repos unsupported) |
| E4 | Git version policy | **4a** allowlist Git 2.53.x (the verified line); refuse other versions with a clear message; widening = re-running the P00-T04 checks for that version with owner approval; 4b warn and continue |
| E5 | Local fixture transport in tests | **5a** test-only Cargo feature `fixture-transport` on `repositories` (off by default, enabled only through a dev-dependency self-reference for tests; static rule proves it absent from non-test builds) allowing `file` transport for fixtures, exactly as the spike's `g_hard_file`; 5b fake `GitRunner` only (no real Git in tests — much weaker evidence) |
| E6 | Real HTTPS/SSH check | **6a** one manual check after T11 with separate owner approval: clone/update a public HTTPS repository and an SSH repository via the user's agent, plus a private HTTPS repository via the user's helper if available; record as manual-pending until then; 6b none in P04 |
| E7 | Grant destination granularity | **7a** per repository: `https://host:port/path` (ssh: `host:port`, path in the fingerprint); 7b per host (broader; one approval covers every repository on that host) |
| E8 | Ref kinds | **8a** branch (fast-forward updates) plus tag and commit as verified detached checkouts (update = fetch + re-verify + move only to the pinned target); 8b branch only in P04, tag/commit with P14 workflows |
| E9 | Workers and commits | **9a** as P03 C9: up to three `sonnet-implementer` workers per wave in lead-prepared worktrees with the coordination log; lead commits B0–B3/report on `phase/P04-git-acquisition` with non-force pushes; merge to `main` separate; 9b lead-only |
| E10 | Debug cycles | **10a** pre-authorised, two attempts per defect, then owner (as P03 C10); 10b each cycle needs approval |
| E11 | Removal semantics | **11a (chosen)** deregister by default; `--delete-checkout` deletes only the confined app checkout after symlink and path checks; reports kept unless the owner later adds a purge option; 11b deregister only |
| E12 | Lock layout | **12a (chosen)** per-repository lock files `checkouts/.locks/<repo-id>.lock` using the P02 lock primitive, documented in Arch §11/Data §2 (layout note, no schema change); 12b in-process locks only (no protection across processes) |
| E13 | Internal destinations | **13a (chosen)** refuse loopback, link-local, private-range IPv4 and `localhost` remotes by default (Git only, P04); an explicit owner-approved allowlist can be added later; 13b accept them (P03 behaviour) |
| E14 | Contract changes | **14a (chosen)** none expected; CA-06 drafted in T03 only if needed and applied only after owner approval; 14b no contract change allowed |

**Owner questions carried in from P03 that touch P04:** the P06 entry gate (same-user processes and the approvals store) and the R7/R8 scope question do not block P04 but should be decided before P06/P16.


## 6. Pilot acceptance anchors
Release gates PRD §9 and §15 apply. Mocks never satisfy real-scan acceptance. Manual terminal/mouse/PDF checks stay listed as manual-pending until performed. Real OpenRouter behaviour is unverified until a separately approved real-API acceptance check.

## 7. Estimates (D-09)

Unit: hands-on hours (Opus orchestration + Sonnet implementation + Opus review/verification + documentation loop) and 10-hour sessions. Per-phase hours already include reporting/documentation, so it is not added again. **Confidence: low** until P01 and P07 provide measured velocity; earlier lower P00 figures (8–14 h) under-counted the per-tool spikes.

### 7.1 Hands-on hours by phase

| Phase | Hours | Phase | Hours |
|---|---|---|---|
| P00 | 38–61 | P11 | 6–10 |
| P01 | 6–10 | P12 | 6–10 |
| P02 | 22–34 (revised at approval 2026-10-26, §5B.3; §7.2 totals not yet recomputed) | P13 | 10–16 |
| P03 | 8–12 (§5C.3 proposes 16–24 incl. CLI; not yet approved) | P14 | 10–14 |
| P04 | 10–16 | P15a | 14–22 |
| P05 | 8–12 | P15b | 10–16 |
| P06 | 12–18 | P16 | 8–12 |
| P07 | 8–12 | P17 | 12–18 |
| P08 | 12–18 | P19 | 10–16 |
| P09 | 8–14 | P20 | 8–12 |
| P10 | 6–10 | | |

**CA-05 automatic evidence analysis (D-18, 2026-10-31; not yet in the figures above or §7.2):** contract task 3–5 h; P07 +1–2 h; P11 +2–3 h; P15a/P15b +2–4 h; P16 +8–12 h; P17 +1–2 h; P19 +3–5 h; evaluation corpus/rubric 4–8 h — about **+24–41 h** hands-on (low confidence; recorded at the owner's acceptance of the CA-05 direction, 2026-10-31). This widens the four-week gap in §7.3; no dates, budget or staffing are assumed.

**CLI increments (D-17), not yet in the figures above:** about +2–3 h in P02 (crate, skeleton, scenario harness) and +1–2 h per later feature phase (~14 phases), so roughly **+16–31 h** hands-on in total. To be folded into each phase's estimate in its request.

Separately estimated, not in the pilot total: P18 Figma translation 8–16 h (timing unknown); X1–X6 ≈ 54–96 h combined.

### 7.2 Totals for the AI-inclusive pilot (P00–P17 except P18, plus P19, P20)

| Component | Hours | Sessions (10 h) |
|---|---|---|
| Hands-on | 220–345 | 22–34.5 |
| Contingency (15–20 % of hands-on, held separately) | 33–69 | 3.3–6.9 |
| Hands-on + contingency | 253–414 | 25.3–41.4 |
| Owner approval / dependency waiting | Calendar time, not counted above: ~21 phase gates plus ~6 separate approvals (installs, crates, network lookup, schema cache, real API, release cut) | Depends on owner turnaround |

### 7.3 Fit against the four-week cap
Sessions per week are not confirmed (Q-01). Fitting 25–41 sessions in four weeks needs about 6.3–10.4 ten-hour sessions per week, before any approval waiting. At 5 sessions/week (200 h, **not** an assumed fact) the AI-inclusive pilot does not fit even at the low hands-on estimate (220 h > 200 h, before contingency); cumulative hands-on hours would reach about P10 at the high estimate or P16 at the low estimate. This is reported as-is rather than lowering estimates.

**Schedule assumption (owner, 2026-10-31):** the four-week cap is no longer treated as an assured commitment. With D-18 (+24–41 h, low confidence) the AI-inclusive pilot is about 244–386 h hands-on before contingency, plus the CLI increments. No scope exchange (§7.4) has been chosen; each needs explicit owner approval together with Q-01 (cadence) and Q-05 (scope exchanges).

### 7.4 Scope exchanges for owner consideration (none applied)
| Option | Effect | Approx. saving |
|---|---|---|
| E1 | kubeconform + schema cache → conditional | 6–10 h |
| E2 | Triggers → manual saved workflows only (WF-02 automation deferred) | 4–6 h |
| E3 | Demo TUI limited to wide + compact layouts | 4–6 h |
| E4 | OS keyring mode stays out (session-only) | 2–3 h |
| E5 | Checkov → conditional (explicit baseline downgrade; only by owner decision) | 8–14 h |
| E6 | Extend the calendar cap or increase sessions/week | — |

E1–E4 together save about 16–25 h and do not by themselves make the pilot fit at 5 sessions/week. Safety controls, redaction, coverage honesty, PDF, and the minimal assistant are not proposed for cutting; since D-18 (2026-10-31) the required automatic evidence analysis is in the same category, and any reduction of it is an owner decision (O-AN-1).

## 8. Risks and blockers

| ID | Item | Status |
|---|---|---|
| B-01 | No tool installs approved (D-10): blocks P00-T08/T09, P08, P09, P13 | **Resolved for P00-T09 tools** (Request A + B1 installed 2026-10-24) and **for cargo-audit** (0.22.2 installed 2026-10-31, `evidence/P03/cargo-audit-C-E/record.md`); open only for conditional tools not yet defined by P08/P09/P13 (the PDF renderer is crates, no install) |
| B-02 | No crate download approval: blocks P00-T06/T07, P01 | **Resolved for P00** (D3-1 executed; T06–T08 crate fetches separately approved and accepted); open for P01 workspace crates (approved dependency requests) |
| B-03 | No network approval for official-page lookup (Q-06): blocks P00-T02/T03/T10 | Resolved for T02/T03 and T10 (owner approval 2026-10-24) |
| B-04 | Worker models unverified | **Resolved for Sonnet workers** (2026-10-31): API-reported model `claude-sonnet-5-5` in every assistant message of all worker/reviewer transcripts checked (`status.md`, Agents row); self-reports still not evidence of work. Open for `haiku-docs` (still a proposal; Opus writes status/handover) |
| B-05 | Trivy on host reports `Version: dev` — not pinnable | X4 only |
| B-06 | Linux Secret Service may be unavailable (headless) | **Resolved for P00** by P00-T07 (qualified); OS-store mode decision stays with P03 (FR-18–FR-24) |
| B-07 | Some tools may not neutralise repo config or honour explicit files | **Resolved for P00** by P00-T09 (qualified): repo-config channels recorded per tool; Checkov revision 2 only; FR-25–FR-28 |
| B-08 | Schedule does not fit four weeks at 5 sessions/week | Owner decision (Q-01, Q-05) |
| R-AN-1 | (CA-05) Automatic analysis widens routine provider disclosure and cost; model output can be wrong, uncited or manipulated by repository text | Mitigations designed (snapshot bounds, approvals, redaction, validation, limits, no tools); unverified until P16/P19; model quality needs a reviewed corpus (O-AN-6) |
| R-AN-2 | (CA-05) Contract change (store 1.5, app 1.2, UI 1.2) after P02's implemented store 1.4 | Lead contract task before P07 keeps it ahead of the first real reports; 1.4 records stay readable (Data §26) |
| B-09 | Python 3.14 compatibility with Checkov/yamllint unknown | **Resolved** by B1: standalone CPython 3.12.15 in isolated venvs (system Python untouched) |

## 9. Debug-cycle log

| Cycle | Defect | State |
|---|---|---|
| P00-T04-D01 | D-1 fail-open attributes setup (no `.git/info/` with empty template); D-2 inert G06 hook fixture; cleanup listing artefact | Completed 2026-10-24; re-run passed; P00-T04 accepted (qualified) |
| P00-T06-D01 | Parser char offsets vs bytes; BOM not stripped; parser recursion-limit classification; Y18 expected-name error | Completed 2026-10-24; re-run passed; P00-T06 accepted |
| P00-T08-D01 | `/Annots` criterion vs printpdf (D-A accepted: empty inline array); H-1 BOM as RTL; H-2 failed PDFs deleted; O-1 overflow diagnosis | Completed 2026-10-24; 9/10 written, U1 Korean (out of scope, D-15); P00-T08 accepted with qualifications |
| P00-T09-D01 | Checkov R1 executed repository Python (cwd discovery + config merge); replacement contract R2 (private cwd, staged allowlisted `-f`, app-owned config/HOME) | Completed 2026-10-24: one correction (comparison control); formal run: no execution under R2, potency controls fired, findings preserved; T09 acceptance pending |
| P00-T09-D02 | H-3: synthetic PEM body lines not covered by the output guard; D01 marker attribution question | Completed 2026-10-24: all PEM components registered (full values, ≥15-char fragments, boundary partials, withheld-excerpt fail-safe); self-test 29/29; fresh gitleaks run equal to run1; reprocessing 0 changes; attribution resolved from existing D01 evidence; raw run1 result moved to private storage |
| P01-D01 | Lead-suspected doctest heading shift in `text.rs` (T06, W2) | Completed 2026-10-26: **no defect** (lead misdiagnosis); wording clarified, `///` lines only (`622f989`) |
| P01-D02 | Review findings on `text.rs`: no static constructor for app-authored `RedactedText`; deserialise doctest control could pass vacuously | Completed 2026-10-26: `pub(crate) redact_static` + serde_json in control (`3b1547e`); integrated run 5 PASS |
| P01-D03 | T08 hand-back claimed 33 tests in `tests/contract.rs`; 22 exist and run | Completed 2026-10-26: report correction only (W1′); no code change |
| P01-D04 | Final-review guards: RedactedText/SanitisedText construction routes, weak doctest controls, unguarded traits, untested versions, over-escaping sampled, private doc link | Completed 2026-10-26: D04a (`e5d7ba5`, `9c47fcd`) + D04b (`6694b34`), fresh `sonnet-implementer` instances in W2/W1′ scopes; integrated run 415/0/0, 32 compile_fail |
| P01-D05 | Four test-file `#[allow]` without `reason` after owner-approved lint tightening (`deny` + `allow_attributes_without_reason`) | Completed 2026-10-26: reasons added (`38851f3`); integrated `--final` run 415/0/0, 32 compile_fail |
| P01-D06 | CA-03 domain migration (`ToolStatus` → `ToolAvailability`; store 1.4 / UI 1.1 in tests) | Completed 2026-10-26: `4b4b3fe`; integrated `--final` 416/0/0, 32 compile_fail; CA-03 check 29/0/0 |
| P02-D01 | Lead defect in FR-31 Appendix A rev 1: string-valued maps not covered; `RendererSetting.parameters_redacted` typed S (should be R) | Completed 2026-10-27, attempt 1: generator `dev/p02_fr31_appendix.py` rev 2 (105 rows), `store_read::redacted_map`, worker `8d22da8`; 489/0/0 (`evidence/debug/P02-D01/`) |
| P02-D02 | Wave-1a review findings within pre-authorisation: serde error echo (store_read, markers, ConstFalse/ConstTrue), unpinned D01 type, ASCII-only bound tests, nullable-key behaviour, unrecorded FsOps deviations; container-level echo assigned to T08 read boundary | Completed 2026-10-27, attempt 1: lead `8043ebd` + worker `3ab305a`; B1a check 497/0/0, 32 compile_fail; 11 rule/dependency items held for owner (`evidence/debug/P02-D02/`) |
| P02-D03 | Owner-approved O1–O10 (value-rule hardening, data-dir ownership/mode, hardened opens, escape set) and Q-T04-1 (`RunStarted.started_at`) | Completed 2026-10-27, attempt 1: lead `b50f64c`, W3 `b7a9174`, W1 `2311014`; integrated 523/0/0, 32 compile_fail (`evidence/debug/P02-D03/record.md`; entry added 2026-10-27 with D04) |
| P02-D04 | Wave-1b review findings within pre-authorisation: field order untested, vacuous asserts, `EventType` sync blind spots, W1 nullable/R-field checks, overstated no-echo doc; lead rustdoc/allow reason | Completed 2026-10-27, attempt 1 (uncommitted): integrated 603/0/0, 32 compile_fail; negative controls NC1–NC3 fail as intended (`evidence/debug/P02-D04/record.md`) |
| P02-D05 | Owner-approved E-1 (`closed_enum!` for all store enums), E-2 (`Identifier`/`PolicyId` no `/`, `@`, `..`), Q-D03-1 (`fired_at`/`skipped_at` + envelope-name guard), R-D03-1/-5, plus wave-2 prep (§5.12 lead seams) | Completed 2026-10-27, attempt 1 (uncommitted): lead + W1 (`p02-d05-w1`); integrated 627/0/0, 32 compile_fail; negative controls NC4–NC7 fail as intended; runner mtime staleness defect found and fixed (`evidence/debug/P02-D05/record.md`) |
| P02-D06 | Wave-2 merge: T08 maximal fixtures violated T09 semantic rules (rules correct, fixtures fixed); lead test defect (queued run in `log.max`); runner hid later binaries without `--no-fail-fast` | Completed 2026-10-27, attempt 1 (uncommitted): fresh W1 on T08 test files only; lead `store_integration.rs` (5 tests) + runner fix; integrated 735/0/0, 32 compile_fail; NC8 replayed green-to-red-to-green (`evidence/debug/P02-D06/record.md`; wave record `evidence/P02/wave2/record.md`) |
| P02-D07 | T14 finding F-1: `atomic::replace_steps` left a stray `.<name>.prev.tmp-<hex>` after a successful rewrite when `<name>.prev` already linked the target inode | Completed 2026-10-28, attempt 1: `persistence/src/atomic.rs` + regressions; strict zero-stray matrices; lead revert control fails 5 tests; integrated 866/0/0, 39 (`evidence/debug/P02-D07/record.md`) |
| P02-D08 | Intermittent post-release assertion in `lock_held_by_another_holder_exits_3` (sibling-test `fork`→`exec` descriptor inheritance, hypothesis) | Completed 2026-10-28, attempt 1: test-only bounded wait; 16 clean `cli` runs (`evidence/debug/P02-D08/record.md`) |
| P03-D01 | Wave-1 review findings: M1 `OperationArg` derived `Debug` printed argument values; G4 accepted a `workflow_revision` grant with null workflow; inherited `TZ` unvalidated; duplicate declared env names only unit-tested | Completed 2026-10-31, attempt 1: lead + W1′/W2′ (`sonnet-implementer`); negative controls caught; integrated 1005/0/0, 41 compile_fail; deferred items bind T08/T10/T11 (`evidence/debug/P03-D01/record.md`, commit `b9fdc0e`) |
| P03-D02 | Wave-3 review findings: human `approvals list` line forgery (LF/TAB), inference scope hidden, missing CLI tests, overstated FR-33 row, `approvals list` error class, `ToolDirs` Debug paths, `RequestOutcome::Permitted` hazard | Completed 2026-11-01, attempt 1: W2′/W1′ + lead rule 13; integrated `--final` 1138/0/0, 41 compile_fail (`evidence/debug/P03-D02/record.md`, `091224b`) |

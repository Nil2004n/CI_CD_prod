# PWT YAML Security Workbench — Development Plan

Plan revision: 1 (2026-10-23), with updates of 2026-10-24: status (task progress, answered questions, blocker states, branch-policy record) and owner-directed follow-up requirement register entries FR-01–FR-24 (§5.2); no scope, phase or estimate changes. Current Git state: see `status.md`.

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
| D-15 | (2026-10-24) Korean (Hangul script) support is **out of scope for the entire project**. Korean text in assessed files is still processed as data: never silently dropped and never a crash; where displayed or rendered it degrades visibly (e.g. `<U+XXXX>`). No Hangul font, IME or Korean-specific layout is a project target | PRD §16, P00-T08 |

Planning corrections 1–6 from the same message are applied: capability labels (PRD §16, SCAN-05/SCAN-06/RAG sources kept), minimal AI kept, early slice carries redaction/coverage (P01/P06/P07), per-tool hypothesis spikes (P00-T09), narrow retrieval sources (P17), and the agent loop (§3).

Pre-implementation contract amendment **CA-01** (store schema, wrapper protocol, command/event contract, UI schema version, storage layout) is recorded in Data Schema §22. Version identifiers were retained; the exception closes at the first implementation commit touching these contracts.

### Open owner questions (non-blocking for P00-T01)

| ID | Question | Blocks |
|---|---|---|
| Q-01 | Sessions per week (cadence) for the four-week cap | Schedule fit only |
| Q-02 | Confirm AI report narration (RES-07 optional part) is deferred, as labelled | P11/P17 scope |
| Q-03 | Keep version identifiers under CA-01/CA-02, or bump store 1.3 / protocol 1.1 before P01 | **Answered 2026-10-24:** CA-02 versioned — store 1.3, wrapper protocol 1.1 |
| Q-04 | Production OpenRouter model ID (owner-supplied) | Real-API acceptance only; mocks need none |
| Q-05 | Which scope exchanges (§7.4), if any, to accept | Pilot cut |
| Q-06 | Approve read-only web lookup of official release/licence pages for P00-T02/T03/T10 | **Answered 2026-10-24** for P00-T02 (Q-06) and P00-T03 (C-1, crates.io/RustSec/docs.rs). P00-T10 lookups still need approval |

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
| P02 | pilot | JSON store v1.3: typed models, schema + semantic validation (Data §11, §18), single writer, lock, temp+replace, recovery, export manifests | Schema conformance of all roots; interruption/malformed/full-disk fixtures; same-day naming | P01 |
| P03 | pilot | Permission evaluator (scope/revision/fingerprint), child-env sanitiser (strip model key/unrelated secrets), credential abstraction (session-only; keyring behind spike result) | Scope-match tables; env-strip tests; no key in any serialised output | P01, P02 |
| P04 | pilot | URL validation (HTTPS/SSH only), clone/fast-forward update, unsafe-state detection, per-repo locks, provenance, Git safety controls from P00-T04 | Local fixture repos (dirty, diverged, hooks, filters, submodules); no source mutation | P00-T04, P02 |
| P05 | pilot | Inventory, exclusions/symlinks/limits, classifier (generic/Actions/Kubernetes; Compose/Helm reported conditional), tristate selection + selectors, reference detection recorded as gaps (SCAN-05) | Hostile filenames, symlink escape, limits, selector semantics | P04 |
| P06 | pilot | Process supervisor (timeout/cancel/process-tree kill per CA-01/CA-02), adapter trait + protocol DTOs, output bounds, preflight + `tools.json`, **minimal redaction module and coverage ledger core**; must satisfy FR-01–FR-03 (§5.2) | Fake-tool fixtures: crash, timeout, oversized/malformed output, ANSI, early stdout close, secret-bearing stderr, tree kill; regression RC-FLOOD (FR-01) | P00-T05, P02 |
| P07★ | pilot | Vertical slice (headless): local synthetic repo → discover → select → in-process YAML syntax check → redacted finding + coverage ledger (other categories explicit `unsupported`) → JSON report via store → deterministic Markdown | Seeded secret absent from report/log; malformed YAML yields finding + honest coverage; velocity recalibration | P05, P06 |
| P08 | pilot | yamllint, Gitleaks, rust-schema adapters per frozen contract sheets | Per-tool golden outputs, exit maps, repo-config injection, scope, redaction | P07, installs |
| P09 | pilot | Checkov adapter (app-owned config, no platform APIs, custom checks blocked) | Same per-tool suite | P07, install |
| P10 | pilot | Normalisation extension: fingerprints, cross-engine dedup with provenance, coverage aggregation | Dedup provenance; counts agree with checks | P08, P09 |
| P11 | pilot | Report finalisation, naming, export manifests, regenerate without rescanning (`RegenerateExport`) | JSON/MD agree; regeneration; collision | P10 |
| P12 | pilot | Restricted local PDF with selected renderer, independent export state | No remote fetch/active content/local file read; PDF failure keeps JSON/MD | P00-T08 decision, P11 |
| P13 | pilot | actionlint + kubeconform adapters; schema cache consumption (cache-only) | Missing CRD = gap; workflow commands never run | P00-T09/T10, installs, cache approval |
| P14 | pilot | Saved workflows, revision-bound approvals, app-open triggers, overlap/missed handling | DST/timezone, overlap, no run after exit | P11 |
| P15a | pilot | Demo TUI on labelled mocks using Arch §6 DTOs (terminal-ui crate only) | Snapshot/resize/input tests; mock marker visible | P01 |
| P15b | pilot | TUI ↔ real backend integration | UI acceptance scenarios (UI §12) automated parts; manual matrix pending | P14, P15a |
| P16 | pilot | OpenRouter BYOK client: explicit model, `allow_fallbacks=false`, ZDR/deny, streaming, typed errors, no tools; mock HTTP only | Mock HTTP suite incl. no-route fail-closed, redirect auth-strip, key absent from logs | P03 |
| P17 | pilot | Local lexical retrieval over selected findings, redacted excerpts, local approved rule descriptions and schema metadata; context isolation; citations; chat panel | Cross-repo leakage, seeded-secret payload, invalid citation, prompt-injection fixtures | P10, P15b, P16 |
| P18 | conditional | Figma translation (presentation only) | Snapshots + manual terminal checks | Figma delivery (D-12) |
| P19 | pilot | Recovery/security/privacy regression hardening | Full agreed regression | P17 |
| P20 | pilot | Linux x86_64 packaging, prerequisite/install guide, tool manifest, notices, manual acceptance | Release gates PRD §9/§15 | P19 |
| X1–X6 | conditional | zizmor, Compose, Helm, Trivy (needs pinnable build), Windows/macOS, `.sh` parity | Own contract sheets | Separate approval each |

Deferred (no phase; interfaces reserved): SCAN-06 live verification, SCAN-05 non-image references, SCAN-09 historical secrets, RES-06 suppression/baseline, run comparison, AI report narration (Q-02), embeddings, chat persistence/historical chat, remote documentation ingestion, draggable splits.

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

| Task | State | Evidence |
|---|---|---|
| P00-T01 | Accepted | `spikes/P00-T01-*.md`, `evidence/P00/P00-T01/` |
| P00-T02 | Request accepted; approved installs (Request A, B1) done and accepted with limitations; PyPI provenance exception; cargo-audit deferred to P01 | `spikes/P00-T02-installation-request.md`, `evidence/P00/P00-T02/` |
| P00-T03 | Rev 1 committed (`f1bd80d`); rev 2 research-backed request complete; D3-1–D3-3 decided 2026-10-24 (D3-1 executed; D3-2/D3-3 provisional); no separate explicit acceptance record | `spikes/P00-T03-crate-request.md`, `evidence/P00/P00-T03/` |
| D3-1 (T05 dependency) | libc 0.2.191 resolved, fetched, built offline in network namespace; not executed | `spikes/rust/`, `evidence/P00/P00-T05-D3-1/` |
| P00-T05 | **Accepted 2026-10-24 as a feasibility spike with findings** (not production-supervisor acceptance); F-1 → FR-01, F-2/F-3 → CA-02, F-4 → FR-02, untested cases → FR-03 | `evidence/P00/P00-T05/` |
| P00-T04 | **Accepted 2026-10-24 (qualified: tested procedure, Git 2.53.0, this host/fixtures only)**; run 1 failed, D01 re-run 36/0/0 (+1 observation) | `evidence/P00/P00-T04/` |
| CA-02 | Applied, versioned and confirmed by owner: store 1.3, wrapper protocol 1.1, specs v0.3.3 | Data Schema §23, `evidence/P00/CA-02/`, `evidence/P00/CA-02-versioning/` |
| P00-T06 | **Accepted 2026-10-24 based on D01, with documented conditions/limitations** (dependency/pipeline suitability for the tested cases; no fallback required); first run 96/5/6 preserved; D01 121/0/7, J16 35/35; carry-forward FR-05–FR-17 (§5.2) | `spikes/P00-T06-request-and-fixtures.md`, `evidence/P00/P00-T06/` |
| P00-T07 | **Accepted 2026-10-24 as a completed feasibility spike with qualified findings** (tested isolated setup only; direct in-process keyring calls, persistent storage, native cancellation, real prompter and other implementations **not** accepted); spike-only advisory exception (RUSTSEC-2026-0370/-0371/-0372); run1 (omitted K04b) and run2 preserved; carry-forward FR-18–FR-24 (§5.2) | `spikes/P00-T07-request-and-isolation.md`, `evidence/P00/P00-T07/` |
| P00-T08 | **Accepted 2026-10-24 with qualifications** (P1 pulldown-cmark + printpdf, featureless; tested cases only; whole-font embedding; no shaping/RTL; empty `/Annots` permitted; Korean out of scope D-15; visual correctness unverified; WeasyPrint suspended); run1 failed, D01 9/10 written, verdicts unchanged | `spikes/P00-T08-request.md`, `evidence/P00/P00-T08/`, `evidence/debug/P00-T08-D01/` |
| P00-T09 | **Awaiting owner acceptance.** Formal run1 done (checkov R1 stop condition fired, preserved); debug cycle P00-T09-D01 (Checkov contract revision 2) done with its own formal run; carry-forward FR-25–FR-29 — see `status.md` and the records | `spikes/P00-T09-request.md`, `spikes/t09-tools/`, `evidence/P00/P00-T09/` |
| P00-T10 – T11 | Not approved, not started | — |

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
| FR-16 | P00-T06 J01–J16 | P02 (J11–J13, J16); P08 (J01–J07, J14, J15) | Current-contract fixtures (store 1.3 / protocol 1.1 examples and version-const negatives) and the differential comparison carried into the production test suites |
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

Process rule from T07 (owner, 2026-10-24): future spike requests enumerate every helper tool and operation, including verification utilities; an unlisted tool is not covered by approval of the surrounding stage.

### 5.1 Per-tool spike protocol (C1/C11 are hypotheses per tool)
For each pinned tool record: (a) every repository config/ignore/allowlist/plugin/custom-check file it auto-discovers, and whether an explicit app-owned config fully neutralises each; (b) explicit-file input honoured vs directory walk — if walking, a staged selected-file copy strategy and its effect on line mapping; (c) output format(s), where raw secret material can appear (stdout, stderr, report file) and the effect of tool redaction options; (d) exit codes for clean, findings, parse error, crash, bad flag; (e) network attempts in default mode and whether they can be disabled verifiably; (f) behaviour on malformed YAML with a seeded fake secret (SCAN-08); (g) version string and binary checksum. Results feed a contract sheet: adapter ID, version, invocation template, config policy, scope strategy, parser ID, exit mapping, network policy, limitations.

## 6. Pilot acceptance anchors
Release gates PRD §9 and §15 apply. Mocks never satisfy real-scan acceptance. Manual terminal/mouse/PDF checks stay listed as manual-pending until performed. Real OpenRouter behaviour is unverified until a separately approved real-API acceptance check.

## 7. Estimates (D-09)

Unit: hands-on hours (Opus orchestration + Sonnet implementation + Opus review/verification + documentation loop) and 10-hour sessions. Per-phase hours already include reporting/documentation, so it is not added again. **Confidence: low** until P01 and P07 provide measured velocity; earlier lower P00 figures (8–14 h) under-counted the per-tool spikes.

### 7.1 Hands-on hours by phase

| Phase | Hours | Phase | Hours |
|---|---|---|---|
| P00 | 38–61 | P11 | 6–10 |
| P01 | 6–10 | P12 | 6–10 |
| P02 | 10–16 | P13 | 10–16 |
| P03 | 8–12 | P14 | 10–14 |
| P04 | 10–16 | P15a | 14–22 |
| P05 | 8–12 | P15b | 10–16 |
| P06 | 12–18 | P16 | 8–12 |
| P07 | 8–12 | P17 | 12–18 |
| P08 | 12–18 | P19 | 10–16 |
| P09 | 8–14 | P20 | 8–12 |
| P10 | 6–10 | | |

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

### 7.4 Scope exchanges for owner consideration (none applied)
| Option | Effect | Approx. saving |
|---|---|---|
| E1 | kubeconform + schema cache → conditional | 6–10 h |
| E2 | Triggers → manual saved workflows only (WF-02 automation deferred) | 4–6 h |
| E3 | Demo TUI limited to wide + compact layouts | 4–6 h |
| E4 | OS keyring mode stays out (session-only) | 2–3 h |
| E5 | Checkov → conditional (explicit baseline downgrade; only by owner decision) | 8–14 h |
| E6 | Extend the calendar cap or increase sessions/week | — |

E1–E4 together save about 16–25 h and do not by themselves make the pilot fit at 5 sessions/week. Safety controls, redaction, coverage honesty, PDF, and the minimal assistant are not proposed for cutting.

## 8. Risks and blockers

| ID | Item | Status |
|---|---|---|
| B-01 | No tool installs approved (D-10): blocks P00-T08/T09, P08, P09, P13 | **Resolved for P00-T09 tools** (Request A + B1 installed 2026-10-24); open for T08 renderer candidates and conditional tools; cargo-audit pending P01 (R-4) |
| B-02 | No crate download approval: blocks P00-T06/T07, P01 | Open — research done (P00-T03 rev 2); first libc-only resolve/fetch/build request D3-1 awaiting approval |
| B-03 | No network approval for official-page lookup (Q-06): blocks P00-T02/T03/T10 | Resolved for T02/T03; open for T10 |
| B-04 | Worker models unverified | Open — first Sonnet workers delegated in P00-T08-D01 and P00-T09 (configured `claude-sonnet-5-5`; self-reports unverified); Haiku/Sonnet agent definitions still proposals; Opus documentation fallback disclosed in `status.md` |
| B-05 | Trivy on host reports `Version: dev` — not pinnable | X4 only |
| B-06 | Linux Secret Service may be unavailable (headless) | P00-T07 |
| B-07 | Some tools may not neutralise repo config or honour explicit files | P00-T09; capability disabled if uncontrollable |
| B-08 | Schedule does not fit four weeks at 5 sessions/week | Owner decision (Q-01, Q-05) |
| B-09 | Python 3.14 compatibility with Checkov/yamllint unknown | **Resolved** by B1: standalone CPython 3.12.15 in isolated venvs (system Python untouched) |

## 9. Debug-cycle log

| Cycle | Defect | State |
|---|---|---|
| P00-T04-D01 | D-1 fail-open attributes setup (no `.git/info/` with empty template); D-2 inert G06 hook fixture; cleanup listing artefact | Completed 2026-10-24; re-run passed; P00-T04 accepted (qualified) |
| P00-T06-D01 | Parser char offsets vs bytes; BOM not stripped; parser recursion-limit classification; Y18 expected-name error | Completed 2026-10-24; re-run passed; P00-T06 accepted |
| P00-T08-D01 | `/Annots` criterion vs printpdf (D-A accepted: empty inline array); H-1 BOM as RTL; H-2 failed PDFs deleted; O-1 overflow diagnosis | Completed 2026-10-24; 9/10 written, U1 Korean (out of scope, D-15); P00-T08 accepted with qualifications |
| P00-T09-D01 | Checkov R1 executed repository Python (cwd discovery + config merge); replacement contract R2 (private cwd, staged allowlisted `-f`, app-owned config/HOME) | Completed 2026-10-24: one correction (comparison control); formal run: no execution under R2, potency controls fired, findings preserved; T09 acceptance pending |

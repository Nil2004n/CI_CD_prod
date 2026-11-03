# PWT YAML Security Workbench — Architecture

Consolidated revision 0.3. Includes all decisions through 23 October 2026: OpenRouter BYOK; local JSON/Markdown/PDF; named tool and wrapper plan; one core developer plus a Figma design team; Opus-led verification, Sonnet implementation, Haiku documentation and owner-approved debug cycles. This specification is not evidence of implemented code or completed tests. Earlier v0.1/v0.2 wording is superseded by this consolidated set.

Version: 0.3.5 (amendments CA-01, CA-02, CA-03, CA-04; store contract 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1) | Date: 2026-10-23 | Status: Proposed architecture

Change record 2026-10-31 (proposed, not implemented): **CA-05 automatic LLM evidence analysis** (PRD ANL-01–ANL-12; change contract `planning/CA-05-automatic-evidence-analysis.md`). Sections marked "CA-05" describe proposed behaviour pending owner approval; current contract versions above are unchanged until CA-05 is applied.
Scope: Single-user local application; Rust-oriented reference design, subject to language sign-off.

## 1. Architectural intent

Use a modular monolith with an event-driven terminal UI, a bounded assessment coordinator, JSON persistence, capability-gated external integrations, a required automatic evidence-analysis stage (CA-05) and an optional interactive RAG chat. External scanners are supervised subprocesses, not separate product microservices. Scanning, findings, coverage and the JSON evidence report must remain usable if every AI service is unavailable; the assessment is then reported `incomplete` (analysis stage not completed), never as fully completed.

## 2. System context

```mermaid
flowchart LR
  U[Local user] --> T[Terminal application]
  T --> G[Installed Git and credential mechanism]
  G --> H[Approved Git hosts]
  T --> L[Tool-managed checkouts]
  T --> S[Approved scanners and renderers]
  T --> P[Local JSON storage and reports]
  T --> N[Permission-controlled external access]
  N --> V[Vulnerability and schema sources]
  N --> OR[OpenRouter with user BYOK]
  OR --> M[Eligible downstream provider]
  T -. automatic evidence analysis: approved redacted snapshot (CA-05) .-> N
  T -. optional interactive chat: approved redacted context .-> N
```

Trust boundaries:

1. Repository content, remote schemas/docs and scanner outputs are untrusted input.
2. Git credentials remain outside application-owned persistence.
3. App-owned storage may contain sensitive repository metadata even after secret redaction.
4. Network/provider access crosses a disclosure boundary requiring explicit scope.
5. Models produce untrusted advisory text; they do not control permission grants or execution.
6. (CA-05) Automatic evidence analysis crosses boundary 4 during the normal workflow, not only when chat is used: the redacted evidence snapshot leaves the device to OpenRouter and the downstream provider under an approved `model_transmission` scope. Model output re-enters as untrusted data and is validated (schema, citations) before persistence.

## 3. Component architecture

```mermaid
flowchart TB
  subgraph UI[Presentation]
    D[Dashboard and selection]
    C[Chat workspace]
    A[Approval dialog]
  end
  D --> B[Command bus and state reducer]
  C --> B
  A --> B
  B --> O[Application coordinator]
  O --> W[Workflow scheduler]
  O --> R[Repository manager]
  O --> I[Inventory and scan planner]
  O --> E[Assessment executor]
  O --> Q[RAG coordinator]
  O --> K[Permission evaluator]
  O --> AN[Analysis coordinator - CA-05]
  AN --> SB[Evidence snapshot builder]
  SB --> RD
  AN --> K
  AN --> MA
  AN --> VAL[Response and citation validator]
  VAL --> PW
  E --> X[Named scanner / schema / renderer adapters]
  X --> PS[Process supervisor]
  PS --> WR[App-owned wrappers or native tool execution]
  X --> NG[Network policy gateway]
  Q --> RD[Redaction and source preparation]
  RD --> IX[Scoped retrieval index]
  IX --> MA[Model adapter]
  MA --> NG
  O --> PW[Single persistence writer]
  E --> F[Finding normaliser and coverage ledger]
  F --> PW
  Q --> PW
  F --> SB
  PW --> DS[JSON store and backup reports]
  PW --> RG[Combined report generator]
  DS -. saved analysis as secondary source .-> IX
  O --> EV[Typed application events]
  EV --> B
```

The analysis coordinator (CA-05) is started by the application coordinator when a per-repository report is finalised. It shares the model adapter, network gateway, redaction module and permission evaluator with chat, but no chat component is on its path: closing or never opening the chat panel has no effect on it.

Every network/process operation must be authorised before execution. Adapters that perform their own network I/O must expose/enforce equivalent policy; merely drawing a gateway does not constrain their internal traffic.

A headless developer CLI (`pwt-workbench-cli`, D-17, CA-04) is a second presentation adapter at the same level as the TUI: it turns arguments into the commands of §6, sends them through the same coordinator and prints the resulting events (human-readable, or one JSON event per line). It does not call Git, scanners, the store or HTTP itself. Commands with no §6 equivalent are labelled `dev`, call only domain/application entry points and are excluded from release builds unless the owner decides otherwise at P20.

## 4. Ownership and interfaces

| Component | Owns | Must not own |
|---|---|---|
| UI | Focus/layout/input, projections of app state | Git execution, scanner business logic, direct data-file writes |
| CLI (developer, D-17) | Argument parsing, event printing, exit codes | Git/scanner/store/HTTP access, implicit approvals, default app-data location |
| Coordinator | Use-case sequencing, active run ownership | Raw terminal rendering |
| Scheduler | Due-time evaluation while app is alive | Background services, implicit catch-up scans |
| Repo manager | Managed checkout identity, locks and safe Git invocation | Credential storage, force-reset behaviour |
| Planner | Frozen target list, capability tasks, approvals needed | Claims that unsupported checks passed |
| Executor | Bounded task execution, cancellation, outcomes | UI state or unrestricted command strings |
| Permission evaluator | Scope matching, grant/deny/revoke records | Model-authored approvals |
| Normaliser | Finding IDs, source attribution, redaction and coverage | Invented locations or scanner evidence |
| RAG coordinator | Context selection, retrieval and provider requests for interactive chat | File modification or command execution; being a prerequisite for evidence analysis |
| Analysis coordinator (CA-05) | Eligibility, logical analyses and their attempts, authoritative-result selection, recovery of interrupted attempts, limits/application-owned retries/idempotency, stage events | Changing findings, outcomes, severities or coverage; modifying the evidence report; granting approvals; provider fallback; automatic replay |
| Evidence snapshot builder (CA-05) | Deterministic bounded snapshot and digest from one finalised report | Reading other repositories, sending unredacted text, unbounded excerpts |
| Response/citation validator (CA-05) | Analysis-schema and citation checks, rejection counts | Repairing more than once, inventing or reassigning citations |
| Persistence writer | Validation, serialised writes, recovery, exports | Security decisions based on filenames alone |

Core domain types: RepoId, FileId, WorkflowId, RunId, TaskId, FindingId, ApprovalId, ConversationId, ChunkId, SourceRef, CheckOutcome and ExecutionStatus; CA-05 proposes AnalysisId, AnalysisStatus, AnalysisReason and AssessmentCompletion. Prefer typed IDs over interchangeable strings in Rust internals.

## 5. End-to-end manual assessment sequence

```mermaid
sequenceDiagram
  actor User
  participant UI
  participant Coordinator
  participant Policy
  participant Git
  participant Planner
  participant Executor
  participant Analysis as Analysis coordinator (CA-05)
  participant Model as Model adapter / OpenRouter
  participant Store
  User->>UI: Add repo URL
  UI->>Coordinator: Register/acquire
  Coordinator->>Policy: Check acquisition scope
  Policy-->>UI: Approval request if required
  User->>UI: Approve concrete operation
  Coordinator->>Git: Clone or safe update
  Git-->>Coordinator: Commit identity / failure
  Coordinator->>Planner: Discover YAML inventory
  Planner-->>UI: Files + exclusions + errors
  User->>UI: Select repos/files/checks
  UI->>Coordinator: Start scan
  Coordinator->>Planner: Freeze plan + workflow revision
  Planner->>Policy: Evaluate all known capabilities
  Policy-->>UI: Missing approvals
  User->>UI: Approve / deny
  Coordinator->>Store: Persist run before execution
  Coordinator->>Executor: Dispatch permitted tasks
  loop Each task result
    Executor-->>Coordinator: Findings + coverage + errors
    Coordinator->>Store: Persist sanitised checkpoint
    Coordinator-->>UI: Progress/event projection
  end
  Coordinator->>Store: Per-repo JSON evidence report (never waits for AI)
  Coordinator-->>UI: Scanner stage terminal (findings and coverage usable)
  Coordinator->>Analysis: Report finalised (automatic, no user action)
  Analysis->>Policy: model_transmission + inference scope
  alt grant missing
    Policy-->>UI: Approval request (analysis awaiting_approval)
    User->>UI: Approve / deny
  end
  Analysis->>Model: Redacted evidence snapshot (bounded)
  Model-->>Analysis: Structured response
  Analysis->>Analysis: Validate schema + citations (≤ 1 repair)
  Analysis->>Store: Immutable attempt record (AnalysisFile) + run log manifest
  Analysis-->>UI: AnalysisFinished (completed / blocked / failed …)
  Coordinator->>Store: Combined Markdown + restricted local PDF (evidence + analysis or stage notice)
  Coordinator-->>UI: Final execution, assessment, coverage and completion state
```

Denied tasks do not become passes. Dynamic discoveries that introduce new network scope pause at a new approval boundary. The analysis branch (CA-05, proposed) runs per repository; a denied or failed analysis leaves scanner evidence and the JSON report untouched and the assessment `incomplete`.

## 6. Runtime model

- One main application process owns the terminal and scheduler.
- UI rendering/input and asynchronous work communicate over bounded typed channels.
- Git/scanner/render processes are children tracked by run/task IDs.
- Blocking parse/filesystem workloads run outside the UI event path.
- One persistence writer serialises authoritative changes.
- One exclusive lock prevents multiple app instances writing the same store; a read-only second-instance mode may be considered later.
- Per-repo locks prevent app-owned updates overlapping assessment.
- Terminating the parent must not leave unattended assessment intentionally running; platform-specific child-tree cleanup is a release test.

### Canonical application command/event contract (app contract v1.1, CA-01, CA-04)

This section is the single canonical command/event vocabulary (resolves C4/C5). UI Schema §17, the demo TUI and mocks refer to it; on conflict this section wins. The contract has its own version line (`app_contract_version` 1.1; 1.0 → 1.1 by CA-04, additive), independent of the document revision, store schema v1.4, wrapper protocol v1.1 and UI schema v1.1. Commands are typed requests from the UI reducer to the coordinator; events are typed projections back. Field-level DTOs are defined in code during P01 and reviewed as a shared contract.

Status: **pilot** = command/event is implemented in the pilot. **reserved (deferred)** = name and intent reserved so later work does not invent a conflicting interface; it is not implemented, is not shown as available in the UI and returns an explicit `OperationFailed{code="deferred"}` if dispatched.

| Command | Status | Purpose / requirement |
|---|---|---|
| AddRepository | pilot | Validate URL, register, request acquisition approval (REP-01) |
| RefreshRepository | pilot | Clone if absent, otherwise fast-forward update under lock (REP-02, REP-05) |
| RemoveRepository | pilot | Deregister with explicit choices for checkout deletion, derived-index purge and report retention (Data §14) |
| DiscoverFiles | pilot | Re-inventory a repository (DISC-01–DISC-05) |
| UpdateSelection | pilot | Apply tristate/selector changes (DISC-03) |
| ReviewScanPlan | pilot | Build and freeze a reviewable plan, list required approvals |
| StartScan | pilot | Execute a reviewed plan |
| CancelRun | pilot | Request supervisor cancellation of a run (WF-05) |
| SaveWorkflow | pilot | Create/update a workflow; increments revision (WF-01) |
| DeleteWorkflow | pilot | Remove a workflow definition; history retained |
| SetWorkflowEnabled | pilot | Enable/disable app-open triggers (WF-02) |
| RunWorkflow | pilot | Manual run of a saved workflow |
| ResolveApproval | pilot | Approve once / approve workflow scope / deny a pending request |
| RevokeApproval | pilot | Revoke an existing grant (Approval state machine) |
| ConfigureCredential | pilot | Set, validate, replace or delete the BYOK key (session-only; OS store if verified) (KEY-01) |
| ConfigureProvider | pilot | Set explicit model ID and optional provider allowlist (KEY-02) |
| SetChatContext | pilot | Choose repo/run/finding context for retrieval (RAG-01, RAG-07) |
| SendChatMessage | pilot | Retrieve, redact, request approval if needed, send (RAG) |
| CancelChatResponse | pilot | Stop an in-flight streamed response |
| ExportReport | pilot | Locate/open-path information for an existing report export (no external open) |
| RegenerateExport | pilot | Re-derive a failed/missing Markdown or PDF from retained JSON without rescanning (RES-08) |
| InspectDependencies | pilot | Re-run preflight and refresh `tools.json` view (TOOL-01) |
| RequestShutdown | pilot | Cancel work, persist, restore terminal |
| SuppressFinding / UpdateSuppression | reserved (deferred) | RES-06 suppression/baseline |
| CompareRuns | reserved (deferred) | History comparison with scope-compatibility warnings |
| GenerateReportNarrative | reserved (deferred); **withdrawn if CA-05 is applied** | Superseded by automatic evidence analysis (ANL) and `ReanalyseReport` |
| ReanalyseReport | proposed (CA-05, app contract 1.2; not implemented) | Explicit retry (new attempt of an uncompleted logical analysis with unchanged inputs) or re-analysis (new logical analysis version) for a report (ANL-11) |
| CancelAnalysis | proposed (CA-05, app contract 1.2; not implemented) | Cancel an in-flight analysis (ANL-08) |
| PersistConversation | reserved (deferred) | Opt-in sanitised chat history |
| StartLiveVerification | reserved (deferred) | SCAN-06 non-mutating verification |

| Event | Status | Delivery |
|---|---|---|
| RepositoryRegistered, RepositoryRemoved | pilot | must-deliver |
| RepositoryStatusChanged | pilot | must-deliver; carries clone/update started/succeeded/failed/skipped state (replaces RepoUpdateStarted/RepoUpdateFailed) |
| InventoryUpdated | pilot | must-deliver |
| SelectionChanged | pilot | latest-value (intermediate states may be coalesced) |
| WorkflowSaved, WorkflowDeleted, WorkflowStateChanged | pilot | must-deliver |
| TriggerFired, TriggerSkipped | pilot | must-deliver (overlap/missed reasons) |
| RunPlanned, RunStarted | pilot | must-deliver |
| RunStateChanged | pilot | must-deliver; includes `cancelling` and `awaiting_approval` transitions (replaces RunCancelling) |
| TaskStarted, CheckCompleted | pilot | must-deliver |
| TaskProgress | pilot | coalescible |
| FindingObserved | pilot | must-deliver |
| RunFinished | pilot | must-deliver; terminal execution/assessment/coverage (replaces RunFinalised) |
| ApprovalRequested, ApprovalResolved, ApprovalRevoked | pilot | must-deliver |
| ReportWritten | pilot | must-deliver; per-format JSON/MD/PDF status |
| ExportStatusChanged | pilot | must-deliver; regeneration outcome |
| DependencyStatusChanged | pilot | must-deliver |
| CredentialStatusChanged | pilot | must-deliver; never contains key material |
| ChatRetrievalCompleted | pilot | must-deliver; selected source IDs only (replaces ChatRetrievalStarted) |
| ChatResponseStarted, ChatResponseFinished | pilot | must-deliver; finish carries completed/cancelled/incomplete (replaces ChatCompleted) |
| ChatResponseChunk | pilot | coalescible by merging text, never dropping it (replaces ChatTokenReceived) |
| ChatFailed | pilot | must-deliver; typed reason (invalid key, credits, rate limit, no compliant route, timeout, truncation, provider outage — the last added 2026-10-26 by owner acceptance at P01-T09, from UI Schema §15; app contract 1.0 unchanged as no implementation had been released) |
| StorageFailure, OperationFailed | pilot | must-deliver |
| AnalysisStateChanged | proposed (CA-05, app contract 1.2) | must-deliver; analysis ID, report ID, state, reason code (no content) |
| AnalysisFinished | proposed (CA-05, app contract 1.2) | must-deliver; analysis ID, attempt, terminal status, reason, validation outcome, AI evidence coverage, workflow completion state |
| SuppressionChanged, RunComparisonReady | reserved (deferred) | — |

Persisted `log.json` events use these names in `event_type`; mock events in the demo TUI carry a visible mock marker and cannot satisfy real-scan acceptance.

No event includes raw secret values or raw child-process output. Progress can be coalesced; findings, permission decisions and terminal run states cannot be silently dropped.

Event envelope (CA-04). The coordinator adds three fields to every event at emission: `event_id` (UUID v4 from the operating system's random source; no fallback if it is unavailable), `at` (UTC RFC 3339, `Z` form) and `level` (`error` for `StorageFailure`, `OperationFailed`, `ChatFailed`; `warning` for `TriggerSkipped` and for `RunFinished` with `failed`, `partial` or `interrupted`; otherwise `info`). Serialised events carry the envelope fields beside `event_type`. Emission rejects an event whose payload contradicts the state machines of §7 (for example `RunFinished` or `CheckCompleted` with a non-terminal state, or a `RunStateChanged` pair §7 does not allow) and reserved events; such a rejection is a programming error, reported as `OperationFailed`. Must-deliver events except `Chat*` are persisted to `log.json` as Data Schema `Event` records (`message_redacted` = the event's detail text or a fixed per-event summary; `code` = its failure code or null). Coalescible and latest-value events and chat events are not persisted to the run log.

## 7. State machines

### Run execution

```mermaid
stateDiagram-v2
  [*] --> queued
  queued --> awaiting_approval
  awaiting_approval --> running: approved scope
  queued --> running: grants already valid
  running --> awaiting_approval: new capability required
  awaiting_approval --> running: approve or skip affected task
  queued --> cancelled
  awaiting_approval --> cancelling
  running --> cancelling
  cancelling --> cancelled
  running --> completed
  running --> partial
  running --> failed
  running --> interrupted: crash / forced termination
  awaiting_approval --> interrupted: crash / forced termination
  queued --> interrupted: persisted abandoned run
```

`completed` describes execution, not a clean security outcome. `partial` means useful results exist but coverage was lost; `failed` means assessment produced no usable target coverage. Graceful cancellation is `cancelled`; abnormal termination is `interrupted` after recovery.

### Check outcome

`pending -> running -> passed | findings | failed | unsupported | skipped | incomplete | cancelled`.

A task can produce findings and still fail later: retain findings and mark its check outcomes incomplete as appropriate. Final assessment summary is `findings`, `no_findings` or `unknown`; coverage is a separate `complete`, `partial` or `none` field. Display full qualification even if findings exist.

### Approval

`pending -> approved | denied | cancelled`. Approved grants may later be revoked or expire. An operation is permitted only while its exact relevant constraints match a valid grant.

### Evidence analysis attempt (CA-05, design direction accepted with qualifications; not implemented)

```mermaid
stateDiagram-v2
  [*] --> pending: report finalised
  pending --> awaiting_approval: grant missing
  pending --> running: grant valid
  pending --> blocked: no provider / no key / limit reached / no compliant route
  pending --> not_performed: insufficient evidence
  awaiting_approval --> running: approved
  awaiting_approval --> blocked: denied
  running --> validating: response received
  validating --> running: one repair attempt
  validating --> completed: schema + citations valid
  validating --> failed: invalid after repair / unsupported summary
  running --> failed: timeout after send / provider error after retries
  running --> cancelled: user cancel
  awaiting_approval --> cancelled
  running --> interrupted: crash / exit
  awaiting_approval --> interrupted: crash / exit
```

Each diagram path is one **attempt** of a **logical analysis**; its terminal state is written once as an immutable record. Only attempt 1 starts automatically; explicit retry adds attempt n+1 to a logical analysis without a completed attempt; explicit re-analysis creates a new logical analysis version. At most one attempt per logical analysis completes. The authoritative analysis of a report is the completed attempt of the highest version that has one; a failed newer version does not erase it. At startup, non-terminal attempts become `interrupted` records (an in-flight request may have been billed) and are not replayed. No exactly-once execution or billing is claimed.

### Workflow completion and the four dimensions (CA-05)

`in_progress -> complete | incomplete`. `complete` requires: scanner execution terminal (`completed` or `partial`), an authoritative completed analysis for every repository report, and every JSON evidence report written. Otherwise `incomplete` with `incomplete_stages ⊆ {scanner, analysis, report}`.

Four dimensions are kept separate in records, events, UI and reports: **workflow completion** (stages), **scanner coverage** (`complete`/`partial`/`none`), **AI evidence coverage** (how much eligible evidence the validated analysis covered: `complete`/`partial`/`none`) and **finding presence** (`findings`/`no_findings`/`unknown`). None implies another; workflow completion never means "no security findings".

## 8. Scope, provenance and generated artifacts

A run stores an immutable snapshot of selected repository refs, commit IDs, file paths/digests, categories, settings, dependency versions and workflow revision. File identity is repository plus relative path; content identity is digest plus commit context.

Source files and generated artifacts are distinct entities. A rendered artifact can point to several source/value inputs. Only record exact source mappings supplied by the renderer; otherwise show the generated location and input provenance without claiming an exact source line.

External references have origin, resolved identity/digest, retrieval time and approval ID. Reference cycles/depth/size limits yield coverage gaps.

## 9. RAG data path

```mermaid
flowchart LR
  S[Explicit repo / run / finding context] --> E[Eligible source resolver]
  E --> R[Secret redaction and control-character sanitisation]
  R --> C[Chunking with provenance]
  C --> I[Local retrieval index]
  I --> F[Context filter + ranking + token budget]
  F --> P[Disclosure policy / approval]
  P --> OR[OpenRouter BYOK]
  OR --> M[Approved eligible inference endpoint]
  M --> O[Answer with validated citation identifiers]
  O --> U[Chat panel / evidence navigation]
```

Requirements:

- Filter by allowed context before ranking.
- Local lexical retrieval is a proposed baseline. An embedding backend is optional and independently approved.
- Index only redacted content; separate caches by repo/commit/redaction version.
- Repository removal can purge associated chunks and conversations according to explicit retention choice.
- Never expose execution or file-write tools to the model.
- Validate citation references against retrieved source IDs; flag unsupported assertions rather than manufacturing citations.
- Stale history remains available for explicit historical questions, with stale badges.
- Chat availability cannot determine scan availability.
- (CA-05) Automatic evidence analysis does not use this chat path's context selection; it uses the deterministic evidence snapshot (Technical Specification §26). Chat may retrieve the saved analysis as `ai_analysis` secondary chunks and must label it AI-generated.

## 10. Permission architecture

A permission request contains capability, operation summary, arguments fingerprint, workflow revision, target repos, destinations/resources, data categories, purpose and requested lifetime. A grant cannot be inferred from a conversational answer or repository content.

Initial clone requires its own concrete approval before any workflow exists. Approved saved workflows reuse only their authorised operations. Denied live/network checks remain visible in coverage. Redirects, recursive references and scanner-controlled downloads must not escape policy.

No unrestricted secret verification: an adapter must not submit detected secret values to external services. Existing Git credentials are used only by approved Git operations.

## 11. Storage and consistency

```text
<app-data>/
  config.json
  repo.json
  workflows.json
  approvals.json
  log.json
  tools.json                   # preflight/manifest view; application-owned, not executable config
  suppressions.json            # reserved (RES-06 deferred); not written by the pilot
  .app.lock
  checkouts/.locks/<repo-id>.lock   # P04 planned per-repository lock (E12); not a JSON store
  checkouts/<repo-id>/...
  backup/<utc-time>_<repo-slug>_<run-id>.json
  backup/<utc-time>_<repo-slug>_<run-id>.md
  backup/<utc-time>_<repo-slug>_<run-id>.pdf
  analysis/<utc-time>_<repo-slug>_<run-id>_a<version>-t<attempt>.json   # proposed (CA-05): one immutable record per analysis attempt
  rag/index.json
  rag/conversations.json       # sanitised persistence only if enabled
  cache/...                    # approved sanitised/cache material
  tmp/...                      # bounded generated artifacts, cleaned safely
```

Directory root is resolved by the platform application-data mechanism or explicit user configuration. No exact OS path is assumed here. User-facing backup directory can be relocated by configuration.

- No cross-file transaction guarantee. Persist run lifecycle before tasks and final report identity after finalisation.
- JSON report is authoritative scanner evidence. Deterministic Markdown and restricted local PDF derive from the same evidence plus, under CA-05, the validated analysis record or a stage notice; AI content is labelled by origin and never replaces evidence.
- Persist independent JSON/Markdown/PDF export outcomes; regenerate failed derived formats without rerunning assessment.
- Startup reconciles logs with reports and marks abandoned runs interrupted.
- Incomplete writes never overwrite the only validated copy; use validated replacement/recovery handling tested per OS.
- Workflow updates increment revision and invalidate affected grants, without mutating old run snapshots.

## 12. Failure and recovery design

| Failure | Behaviour |
|---|---|
| Git auth/update failure | Sanitised actionable message; skip repo; continue others |
| Missing scanner | Unsupported capability, dependency guidance, no clean badge |
| Scanner timeout/crash | Preserve earlier findings; failed/incomplete task |
| Scanner output exceeds budget | Reader stops at budget, immediate termination; output marked truncated; check incomplete, never passed (CA-02) |
| Leftover descendants after child exit | Group members cleaned up and recorded separately from the child's own status (CA-02); descendants outside the group remain a stated limitation |
| Invalid schema/renderer missing inputs | Specific gap; other applicable checks continue |
| Approval denied | Skip affected operation; qualified summary |
| Terminal resize | Reflow layout; preserve app/run state |
| Storage full/permission denied | Prominent error; stop starting new tasks if durable evidence cannot be preserved |
| Model unavailable | Scans and evidence reports remain usable; chat shows an error; automatic analysis (CA-05) becomes `blocked`/`failed` with a reason and the assessment is `incomplete` |
| Analysis output invalid / citations unsupported (CA-05) | At most one repair attempt; then `failed`; rejected items counted; nothing invented in reports |
| Analysis interrupted by exit/crash (CA-05) | `interrupted` at startup reconciliation; no automatic replay (possible charge) |
| Analysis record not persisted (CA-05) | Writer halts; attempt not completed; scanner records and evidence report untouched |
| Evidence report unreadable or digest mismatch (CA-05) | No request; attempt `blocked`/`evidence_unavailable` |
| Combined export fails (CA-05) | Evidence JSON and analysis records kept; export marked failed and regenerable without rescanning or re-analysis |
| Forced termination | Reconcile orphaned runs/reports at next startup |
| External API rate limit | Bounded retry only for safe operations; preserve failure context |

## 13. Security boundaries and limitations

- Read-only assessment is policy plus implementation discipline, not a guarantee supplied by Rust or an ordinary child process.
- Fixed binary/argument policies and app-owned temporary output are mandatory. Stronger OS sandboxing is a separate hardening decision.
- Repositories may contain malicious Git configuration inputs, templates, control characters or oversized data. Clone/render/parser safety requires targeted tests.
- Escape terminal control sequences in filenames, logs, docs and model responses before display.
- Do not automatically open URLs or execute copied suggested commands.
- Network destination policy alone does not prevent every scanner subprocess from making connections; integrate enforceable tool settings or appropriate isolation.
- Secret redaction cannot guarantee all sensitive business information is removed; provider disclosure remains explicitly approved.
- (CA-05) Automatic analysis widens routine provider disclosure compared with chat-only use; it is bounded by the evidence snapshot, approved data categories and limits, and the model has no tools.

## 14. Architecture decisions and unresolved items

| ADR | Proposed decision | Reason / unresolved condition |
|---|---|---|
| A-01 | Modular monolith | Local app; avoid distributed deployment complexity |
| A-02 | Rust + Ratatui/Crossterm, Tokio, Serde | Accepted by owner (D-01, 2026-10-23); other dependencies via spikes |
| A-03 | External scanner adapters | Reuse rule engines; dependency coverage must be tested |
| A-04 | JSON persistence | Matches agreed storage; scale/retention limits need benchmarks |
| A-05 | No daemon | Matches terminal-open runtime |
| A-06 | Chat has no execution tools | Preserves advisory safety model |
| A-07 | Direct Git invocation, script optional | Accepted by owner (D-03); `.sh` wrappers are extension X6 |
| A-08 | No implicit all-network grant | Approval matches requested external access requirement |
| A-09 | Native Rust process adapters implement protocol v1.1 DTOs | Accepted (D-03); supervisor owns timeout/cancel/process-tree kill out-of-band (CA-01) |
| A-10 | Linux x86_64 first pilot target, portable interfaces | Accepted (D-02); Windows/macOS unverified extensions (X5) |
| A-12 | (Proposed, CA-05) Automatic evidence analysis as a separate coordinator stage reusing the provider adapter | Required by the owner (2026-10-31); keeps chat optional and scanner evidence independent; pending approval |
| A-11 | Independent version lines: document 0.3.x, store 1.4, wrapper protocol 1.1, app command/event contract 1.1 (CA-04), UI schema 1.1 (CA-03) | Resolves C6; see Data Schema §17 and §22 |

Physical deployment matrix, scanner/framework coverage, OpenRouter inference-model choice and performance budgets are sign-off gates, not architecture facts established by this draft.


## 15. Tool execution topology

```mermaid
flowchart TD
  SEL[Frozen selected YAML and approved context] --> CLASS[Configuration classifier]
  CLASS --> GEN[yamllint / parser and Gitleaks]
  CLASS --> ACT[actionlint and optional zizmor]
  CLASS --> K8S[kubeconform and supported Checkov]
  CLASS --> CMP[Conditional Compose validation]
  CLASS --> HELM[Conditional Helm lint / local render]
  HELM --> K8S
  CLASS --> IMG[Approved referenced-image resolution]
  IMG --> TRIVY[Conditional Trivy vulnerability task]
  GEN --> NORMAL[Tool-specific parsers and sanitisation]
  ACT --> NORMAL
  K8S --> NORMAL
  CMP --> NORMAL
  TRIVY --> NORMAL
  NORMAL --> LEDGER[Checks / findings / coverage / provenance]
  LEDGER --> STORE[Single writer and reports]
```

All execution passes through manifest/preflight, permission policy and process supervision. Scripts are an adapter transport, not an alternate uncontrolled execution path. Shell scripts are optional (extension X6); the pilot uses direct executable invocation through native adapters, which must provide the full protocol contract.

The core alone owns JSON parsing/normalisation, selected-scope enforcement, deduplication, durable data and UI events. Application-supplied wrappers may invoke only allowlisted pinned tools. No eval, arbitrary commands, repository custom-check loading, fixes, deployment or cluster scans. Unrelated secrets/model key are removed from child environments.

## 16. Network and output trust boundaries

The in-process policy layer is not a hosted PWT gateway. It approves core HTTP and tool requests, including transitive schemas/references, registry/advisory access and OpenRouter disclosure. Tools making internal connections must expose enforceable configuration/isolation; a diagram or consent flag alone does not intercept those connections. Unsupported enforcement disables the capability with explanation.

Native tool results are untrusted: enforce bytes/time/memory limits, map actual exit semantics, parse pinned formats, redact before logs/persistence/display and escape terminal control sequences. A tool can produce findings and then fail; preserve evidence and incomplete coverage. Temporary sensitive output, if unavoidable, requires restricted approved handling and cleanup, not an assertion of guaranteed secure deletion.

## 17. BYOK and report deployment

```mermaid
flowchart LR
  subgraph DEVICE[User device]
    SCAN[Local scanners] --> EVID[Normalised JSON evidence]
    EVID --> MD[Deterministic Markdown]
    MD --> PDF[Restricted local PDF]
    EVID --> SNAP[Evidence snapshot builder - CA-05]
    SNAP --> CONSENT
    EVID --> RET[Local filtered redacted retrieval]
    RET --> CONSENT[Approved disclosure and routing policy]
    KEY[OS key store or session memory] --> CLIENT[OpenRouter client]
    CONSENT --> CLIENT
  end
  CLIENT --> OR[OpenRouter]
  OR --> LLM[Eligible downstream model endpoint]
```

No PWT inference server/account/shared key. Interactive chat is optional; automatic evidence analysis is a required stage (CA-05) that uses the same client. No-key/network failure does not stop scanning or the evidence report; it leaves the analysis stage `blocked`/`failed` and the assessment `incomplete`. Model receives no execution tools. Local lexical retrieval requires no vector database/embedding API. Request policy initially denies data collection, requires ZDR endpoints and disables fallback; no eligible route produces an explicit error. It is not a promise that intermediary metadata is absent.

Record independent export states in log manifests. JSON report snapshots are immutable; Markdown/PDF regeneration uses retained evidence and the validated analysis version it names (CA-05; earlier text: approved narrative version). A new analysis version creates a new export revision, not mutation of accepted evidence.

## 18. Development control plane

```mermaid
flowchart TD
  HUMAN[Human phase and debug approval] --> OPUS[Opus 5.5 orchestrator]
  SPEC[Five root specifications and approved plan] --> OPUS
  OPUS --> SONNET[Scoped Sonnet 5.5 implementers]
  SONNET --> DIFF[Integrated reviewed changes]
  DIFF --> TEST[Opus executes functional and regression checks]
  TEST --> REPORT[Opus evidence / diagnosis / proposed solution]
  REPORT --> HAIKU[Haiku documentation scribe]
  HAIKU --> STATE[status.md / handover.md drafts]
  STATE --> VALIDATE[Opus validates documentation diff]
  VALIDATE --> STOP[Stop and present report]
  STOP --> HUMAN
  DESIGN[Figma design team] --> DEMO[Developer demo / final TUI translation]
  DEMO --> DIFF
```

A feature phase approval authorises its bounded implementation plus the explicitly listed local functional/regression checks. Opus delegates implementation to Sonnet 5.5, inspects the integrated changes and invokes the real verification commands. It diagnoses confirmed failures versus suspected risks and proposes a bounded solution. Haiku drafts/updates `status.md` and `handover.md` from Opus's evidence packet; Opus validates the diff before the checkpoint is accepted. STOP for the owner. No failed test automatically authorises a fix. An explicitly approved debug cycle authorises the named repairs plus listed retests; repeat only with new owner approval. Starting the next feature requires approval even if every mandatory test passes. After two unsuccessful cycles on the same defect, escalate the diagnosis and scope rather than retry indefinitely. This replaces the earlier requirement for a separate human pause between implementation and its already-authorised tests.

Runtime `approvals.json` is for the end user's assessment/network scope. Development approvals in plan/status refer to code changes and verification. These are separate trust domains; neither can authorise the other. Claude Code/subagent permissions must not be shipped into the product RAG assistant.

Haiku is a constrained documentation writer, not a test/security authority. It uses an Opus-prepared structured packet and cannot edit code, plan/specs or approval facts. Opus validates against evidence and actual Git state; only one documentation writer is active. Agent-written flags are not proof of human approval. Process instructions are not a sandbox; preserve native permissions and human review.

## 19. UI-design and continuity boundaries

UI team provides Figma only unless separately agreed. Developer implements a demo using stable service DTOs/mock events while core work proceeds; final design maps to terminal character cells. Mocks are labelled and never counted as real scanner verification. Design integration changes presentation; contract/rule/permission changes require owner review.

Root plan/status/handover carry phase, code revision, command evidence, dirty worktrees, approvals and next permitted action. Archive long logs, keep handover compact, verify changed/stale facts on resume. No zero-token or automatically verified-continuity claim.

## 20. Architecture release gates

- Same feature verified against real integrated code, not only worker assertions.
- Tools/versions/config/network/exit mappings match the frozen release manifest.
- Wrapper versus native execution tested on the declared platforms.
- Cross-engine overlap, missing dependencies, denied requests and unknown schemas remain explicit.
- Licensing/source/NOTICE obligations checked before bundling; Docker Desktop and hosted features not assumed universally free.
- Four-week pilot cut approved for one developer with Figma design support; broad target coverage remains an explicit roadmap.
- (CA-05) Automatic evidence analysis: deterministic contract/security cases AC-01–AC-18 pass with mock HTTP; analysis failure never hides behind a completed assessment; model-quality evaluation on a reviewed corpus is reported separately.

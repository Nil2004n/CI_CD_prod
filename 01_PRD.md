# PWT YAML Security Workbench — Product Requirements Document

Consolidated revision 0.3. Includes all decisions through 23 October 2026: OpenRouter BYOK; local JSON/Markdown/PDF; named tool and wrapper plan; one core developer plus a Figma design team; Opus-led verification, Sonnet implementation, Haiku documentation and owner-approved debug cycles. This specification is not evidence of implemented code or completed tests. Earlier v0.1/v0.2 wording is superseded by this consolidated set.

Version: 0.3.3 (amendments CA-01, CA-02; store contract 1.3, wrapper protocol 1.1) | Date: 2026-10-23 | Status: Review-ready draft
Owner: PWT | Working product name: PWT YAML Security Workbench

## 1. Document status and decision rules

This document consolidates the requirements supplied in the project discussion. The working product name is not an approved brand.

- Agreed: directly requested or explicitly accepted in the discussion.
- Proposed: implementation or product detail recommended for review, not an approved delivery commitment.
- Open: needs a named decision owner and resolution before release acceptance.
- Requirement IDs describe target behaviour, not proof that functionality exists.

Companion documents: Technical Specification, Architecture, UI Schema, and Database/Data Schema. The PRD controls user-visible behaviour. Technical choices remain proposed until engineering sign-off. Do not interpret this draft as an approved schedule, budget, or guarantee of universal security coverage.

## 2. Product summary

A locally installed, cross-platform terminal application that registers Git repositories, discovers all `.yaml` and `.yml` files, lets users select multiple repositories and files, and assesses them for security weaknesses, misconfigurations, exposed secrets, syntax problems, and schema errors. It supports named reusable workflows, automation while the application is open, structured reports, and a source-grounded RAG assistant embedded in the terminal UI.

The product is an advisory assessment tool. It does not automatically fix configurations, commit, push, deploy, execute repository workflows, or operate as a background service.

## 3. Problem and intended users

Users need a repeatable way to inspect repository configuration beyond GitHub Actions. They need to distinguish actual findings from failed or unsupported checks, inspect several repositories without juggling independent scanner consoles, and understand why a finding matters.

Primary roles:

| Role | Main tasks |
|---|---|
| Security engineer | Select assessment scope, investigate findings, review coverage and evidence |
| Developer / DevOps engineer | Inspect configuration problems and remediation suggestions |
| Reviewer / team lead | Review per-repository reports, scan history, and recurring workflows |

Delivery ownership: one core developer uses Claude Code for implementation and verification. A UI team supplies Figma designs; the developer builds a demo TUI and implements the approved design later. UI coding is not assumed to be supplied by the design team.

The initial product is a single-user local application. Shared tenancy, central account management, RBAC, and cloud dashboards are not agreed requirements.

## 4. Goals and non-goals

### Goals

1. Discover YAML configurations across repositories, not only CI/CD directories.
2. Make selection, execution, progress, and evidence inspectable through a mouse-responsive terminal dashboard.
3. Reuse scan selections and settings across multiple named workflows.
4. Preserve accurate coverage and failure information.
5. Protect repository content and secret values during scanning, reporting, and AI assistance.
6. Help users understand results through grounded chat without making chat mandatory for normal operations.

### Non-goals

- Automatic remediation, repository-content editing, commits, pushes, or deployment.
- Automatic execution of repository scripts or workflows to discover what they do.
- A claim that arbitrary YAML can receive every possible security/schema check.
- An always-on daemon or scans after the terminal application exits.
- Replacement of Git credential management.
- Whole-application penetration testing or exhaustive source-code/dependency scanning unrelated to selected configuration.
- Mandatory cloud accounts, enterprise collaboration, or a browser UI.

## 5. Confirmed scope versus proposed implementation

| Area | Status | Decision |
|---|---|---|
| YAML discovery | Agreed | Both extensions; every configuration type present is eligible for discovery |
| Selection | Agreed | Multiple repositories and files; mouse-responsive checkboxes |
| Assessment | Agreed target | Security weaknesses, misconfiguration, secrets, syntax/schema, references and referenced vulnerabilities |
| Schemas and templates | Agreed target | Automatic and user-supplied schemas; source and rendered templates |
| Authentication | Agreed | Existing Git credentials/credential manager are prerequisites |
| Workflow lifetime | Agreed | Automation only while terminal application is open |
| UI | Agreed foundation | Full-screen, mouse-responsive, panel-based terminal UI and integrated RAG window |
| Hybrid UI | Proposed | Dashboard-first with expandable conversational workspace |
| Rust stack | Agreed (D-01) | Rust, Ratatui/Crossterm, Tokio, Serde reference stack; other dependencies remain subject to spikes |
| Repository script | Agreed (D-03) | Rust reads `repo.json` and invokes Git directly; native Rust process adapters are the pilot baseline; `.sh` wrappers are optional parity extension X6 |
| Storage | Agreed | `log.json`; dated JSON/Markdown/PDF under `backup` |
| LLM runtime | Agreed | OpenRouter BYOK; no bundled shared key or PWT inference server |
| Development | Agreed | Opus 5.5 orchestrator/test executor, Sonnet 5.5 workers, Haiku documentation, human gates |
| Platform versions | Partly agreed (D-02) | Linux x86_64 is the first pilot target; Windows/macOS are explicit extensions (X5), not verified or supported until their acceptance matrix passes; exact distribution/terminal matrix remains open |

## 6. Functional requirements

### Repository management

| ID | Requirement | Acceptance evidence |
|---|---|---|
| REP-01 | Accept one or more supported repository URLs and register them persistently. | Register two repos; restart; both remain visible. |
| REP-02 | Clone repositories not yet present; update existing tool-managed checkouts using Git. | First use and subsequent update take distinct paths. |
| REP-03 | Use preconfigured Git authentication; do not implement an application login or persist Git secrets. | Private repo works via prerequisite credentials; secrets absent from app files. |
| REP-04 | Use tool-managed checkouts; never delete non-YAML files to construct the inventory. | Non-YAML content remains unchanged after discovery/scan. |
| REP-05 | Detect unsafe local state/update failures, report them, and skip assessment of that repo for the attempted run. Never reset or resolve conflicts automatically. | Dirty/conflicting fixture remains unmodified; other repos continue. |
| REP-06 | Preserve repository identity, selected ref, checkout path, and commit provenance. | Reports identify exactly what was assessed. |

### Discovery and selection

| ID | Requirement | Acceptance evidence |
|---|---|---|
| DISC-01 | Recursively inventory `.yaml`/`.yml` in the checkout, across configuration families. | Fixtures outside `.github` appear. |
| DISC-02 | Display repository-relative paths grouped by repository. | Same filename in two repos is unambiguous. |
| DISC-03 | Support per-file, per-repository, and global select/clear, with partial-selection states. | Checkbox states match underlying selection. |
| DISC-04 | Make exclusions and discovery errors visible. Proposed: skip Git object metadata and do not follow symlinks outside checkout. | Inventory summary accounts for excluded/failed paths. |
| DISC-05 | Preserve explicit file choices; dynamic all-YAML selections include new files. Missing selected paths are reported. | Add/delete fixture files and rerun saved workflows. |

### Assessment

| ID | Requirement | Acceptance evidence |
|---|---|---|
| SCAN-01 | Assess syntax, schema, security weaknesses/misconfigurations, and secrets as separately tracked capabilities. | Valid, invalid, insecure, and secret-bearing fixtures produce distinct results. |
| SCAN-02 | Detect configuration type and select supported rules. Unknown types still receive applicable generic checks. | Unknown YAML is not labelled fully security-validated. |
| SCAN-03 | Support detected schemas and user-supplied schemas; show schema identity/version and unavailable/ambiguous schemas. | Missing schema produces unsupported/incomplete status, not pass. |
| SCAN-04 | Support template-source assessment and approved local rendering, keeping source and rendered results separate. | Renderer inputs/digest and generated locations recorded. |
| SCAN-05 | Assess referenced images/dependencies/actions and external/reusable configurations where an approved adapter supports them. | Referenced target and evidence version recorded; unsupported targets explicit. |
| SCAN-06 | Allow necessary external retrieval and non-mutating verification only within approved scope. | Out-of-scope operation pauses; denied operation becomes skipped/incomplete. |
| SCAN-07 | Run selected repositories with bounded concurrency and expose per-file/per-check progress. | A failed task does not hide successful tasks. |
| SCAN-08 | Continue applicable secret/text checks even when YAML parsing fails, where the adapter can do so safely. | Malformed YAML containing a seeded secret yields both relevant outcomes. |
| SCAN-09 | Offer historical secret assessment as a proposed explicit option; checkout-only scanning is the proposed default. | Historical mode is not silently enabled. |

“All vulnerabilities” expresses category breadth, not guaranteed detection of every vulnerability. The release coverage matrix is authoritative for supported checks. Static analysis, retrieved evidence, and live verification must be distinguishable.

### Workflows and execution

| ID | Requirement | Acceptance evidence |
|---|---|---|
| WF-01 | Save multiple named workflows including repo/ref/file selectors, categories, renderer/schema settings, triggers and approval scope. | Reload two independent workflows without selection leakage. |
| WF-02 | Support manual execution and application-open automation. Proposed triggers: interval and daily-local-time. | Trigger fires while open and does not run after exit. |
| WF-03 | Reuse valid approvals only within the authorised workflow revision and constraints. | Changed external destination invalidates relevant approval. |
| WF-04 | Request fresh approval for new operations; pause instead of silently broadening access. | New network destination prompts before request. |
| WF-05 | Support cancellation and honest interruption records; persist completed results incrementally. | Forced termination yields interrupted run on restart. |
| WF-06 | Proposed: prevent overlapping runs of the same workflow; record missed/skipped triggers without replaying a backlog by default. | Long-running scan does not create an unbounded queue. |

### Findings, reports, and history

| ID | Requirement | Acceptance evidence |
|---|---|---|
| RES-01 | Show severity, category, rule, tool/version, repository/commit, file/line where available, redacted evidence, explanation and suggestions. | Finding is traceable to source and engine. |
| RES-02 | Distinguish execution completion from finding presence and coverage completeness. | A completed scan can have findings; incomplete scan never appears clean. |
| RES-03 | Persist activity/run history in `log.json`. | Restart retains events and final run summaries. |
| RES-04 | Save per-repo JSON, Markdown and locally generated PDF reports in `backup`, with timestamp/repo/run identifiers preventing collisions. | Two same-day runs create distinct files. |
| RES-05 | Retain completed partial evidence when one repository or task fails. | Partial report lists missing coverage and failure reasons. |
| RES-06 | Proposed: allow justified suppression and baseline comparison without modifying repository files. | Suppression reason/expiry retained; raw finding count stays inspectable. |

### RAG assistant

| ID | Requirement | Acceptance evidence |
|---|---|---|
| RAG-01 | Provide a contextual chat panel and expandable view grounded in approved selected files, reports, logs, workflows and rule/schema documentation. | Selection and provenance visible beside conversation. |
| RAG-02 | Cite supporting repository paths, line ranges, run/report IDs, or documentation identifiers. | Evidence links navigate to details. |
| RAG-03 | Distinguish scanner-confirmed findings from interpretation/suggestions; acknowledge missing context. | Failed checks are not described as passed. |
| RAG-04 | Apply secret redaction before indexing, logging or provider transmission; obtain scope-specific approval for external data sharing. | Seeded secrets absent from model payload/index/logs. |
| RAG-05 | Treat retrieved instructions as untrusted content; no model-driven command execution, file editing or approval bypass. | Malicious YAML comments cannot trigger tools/actions. |
| RAG-06 | Keep scanning usable when provider/embedding services fail or approval is denied. | Non-chat functions remain operational. |
| RAG-07 | Track repository/commit/run context and identify stale sources; exclude unrelated repositories unless explicitly included. | Cross-repo leakage test passes. |

### UI requirements

UI-01: Mouse selection, scrolling, panel focus, keyboard alternatives and terminal resize support.

UI-02: Distinct repository/file selection, central operational workspace, progress/findings, and chat areas.

UI-03: Visible approval requests, interrupted/partial status, global selection count and current context.

UI-04: Proposed: collapsible/expandable panels, draggable splits where supported, compact mode on small terminals, no colour-only status encoding.

## 7. User journeys

### Manual assessment

Add URL -> approve acquisition/network scope -> clone/update -> discover -> select repositories/files -> choose checks -> review scan plan -> approve required capabilities -> execute -> inspect coverage/findings -> ask contextual questions -> view saved reports.

### Reusable automation

Select repos/files -> save named workflow -> configure schedule and permissions -> enable -> execute while app is open -> pause for new permissions if needed -> persist report/history. Closing the app stops scheduling; startup does not imply background scans occurred.

### Investigation

Select finding -> inspect evidence -> open “Ask about this finding” -> view grounded explanation -> copy suggested changes manually if desired. No apply-fix operation exists.

## 8. Safety and privacy requirements

- Reading repository content is expected; copying raw source into separate RAG/index/report storage is not automatically authorised.
- Tool operational writes are permitted: checkouts, app config, workflow definitions, sanitised logs/reports, approved caches and temporary artifacts.
- No automatic changes to assessed source configuration. Tool-managed Git updates are a separately approved operational exception.
- External access approval names capability, destinations/constraints, data categories, purpose and lifetime.
- Do not send detected raw secrets to a model, report, telemetry system or live secret-validation service.
- Redaction is best-effort and must not be advertised as a guarantee of complete de-identification.
- No hidden telemetry. OpenRouter and its downstream provider process approved external context; private-context privacy constraints and local retention must be explicit. BYOK does not mean offline or universal zero retention.

## 9. Release gates and measurable acceptance

The first release is not accepted until:

1. A named OS/architecture/terminal matrix is tested.
2. A coverage matrix names every promised framework, schema dialect, renderer, vulnerability source, scanner version and license/dependency.
3. Representative fixtures verify expected detections and documented limitations; false-positive/false-negative targets are agreed.
4. Repository-integrity, approval, redaction, cross-repo context-isolation and interrupted-write recovery tests pass.
5. Multi-repo workflows, mouse/keyboard selection and resize/cancellation paths work end-to-end.
6. Report fields and coverage summaries agree with JSON schemas and UI counts.
7. Performance is benchmarked on agreed hardware, repository sizes and concurrency; no unsupported timing promise is made here.

## 10. Proposed delivery slices

| Slice | Demonstrable outcome |
|---|---|
| Foundation | TUI shell, managed checkouts, inventory, selection, safe persistence |
| Assessment | Syntax/schema/secrets/security adapters, normalised findings, reports |
| Workflows | Multiple saved workflows, in-app scheduling, approvals, interruptions |
| Assistant | Redacted contextual retrieval, model integration, citations, focus mode |
| Hardening | Approved platform/check matrix, cancellation, recovery, packaging and benchmarks |

These slices are sequencing guidance, not dates. Estimate only after dependency and team review.

## 11. Open decisions register

| ID | Decision | Proposed owner |
|---|---|---|
| O-01 | Rust/Java final choice and UI library sign-off | Engineering lead |
| O-02 | Supported OS versions, architectures, terminals and scanner availability | Engineering lead + QA |
| O-03 | First-release supported frameworks/checks and acceptance fixtures | Security lead |
| O-04 | OpenRouter inference model, eligible routes, disclosure/retention; no embedding API required for pilot | Owner/security |
| O-05 | Solo-development pilot cut within four-week cap, Figma timing and review capacity | Owner + PWT manager |
| O-06 | Retention limits, schedules, history mode and resource defaults | Product + engineering |
| O-07 | Cross-platform replacement of mandatory `repo_ext.sh` | Engineering lead |
| O-08 | Coverage/performance/quality targets and scanner license review | Security + QA |

Resolution status (owner decisions 2026-10-23, recorded in `plan.md`): O-01 resolved by D-01 (Rust). O-02 partly resolved by D-02 (Linux x86_64 first). O-04 partly resolved by D-06 (minimal BYOK assistant stays in the pilot; the owner supplies the production model ID, which is not chosen by development agents). O-05 remains open (D-09 cadence, D-12 Figma timing). O-07 resolved by D-03. O-03, O-06 and O-08 remain open.


## 12. Named scanner and wrapper roadmap

The following is the complete planned integration inventory, not a claim that every tool is installed or every adapter is accepted for the pilot. The owner freezes the pilot matrix before implementation; conditional tools remain visible as deferred/unsupported until implemented and verified.

| Adapter ID | Tool | Wrapper / native equivalent | Capability | Proposed release status |
|---|---|---|---|---|
| git | Git | `pull.sh` / core Git invocation | Clone new managed repos; safe update existing repos | Foundation |
| yamllint | yamllint | `scan_yaml.sh` | Generic YAML diagnostics; style separated from syntax | Pilot baseline |
| gitleaks | Gitleaks | `scan_secrets.sh` | Current-checkout suspected-secret detection | Pilot baseline |
| checkov | Checkov | `scan_misconfig.sh` | Security policies for supported configuration frameworks | Pilot baseline |
| actionlint | actionlint | `scan_actions.sh` | GitHub Actions structure/expressions; controlled embedded-script analysis | Pilot framework adapter |
| kubeconform | kubeconform | `scan_kubernetes.sh` | Versioned Kubernetes schema checks, approved CRD schemas | Pilot framework adapter |
| zizmor | zizmor | `scan_actions_security.sh` | GitHub Actions security-specific analysis, no fixes | Conditional extension X1 (owner D-04) |
| docker-compose | Docker Compose CLI | `validate_compose.sh` | Explicit Compose model validation, not security certification | Conditional on Compose scope |
| helm | Helm | `scan_helm.sh` | Chart lint and controlled local template rendering | Conditional on Helm scope |
| trivy | Trivy | `scan_referenced_images.sh` | Known vulnerabilities in approved referenced images | Conditional on image-vulnerability scope |
| rust-schema | Chosen Rust schema validator | In-process adapter, not mandatory shell | Automatic bindings/local supplied schemas for supported dialects | Pilot baseline |

Invocation (owner D-03, CA-01): the Linux pilot invokes every tool through an application-owned native Rust process adapter (Git directly). The named `.sh` wrappers are the optional parity extension X6, not a mandatory runtime layer; the tool-to-capability mapping above is retained. Exact versions, flags and supported-framework coverage are frozen only after the approved per-tool spikes; a baseline capability is never silently downgraded. The canonical pilot/conditional/deferred label register is PRD §16.

Development-only tooling: Rustfmt/Cargo formatting, Clippy, Cargo tests/build, cargo-audit, ShellCheck and Bats-core. Scanner fixtures and contract tests verify the tool integration itself. These must not all become mandatory customer runtime dependencies.

### New product requirements

| ID | Requirement | Acceptance evidence |
|---|---|---|
| TOOL-01 | Show per-tool version, availability, supported capability, release tier and known limitations. | Missing tool produces a coverage gap, not pass. |
| TOOL-02 | Route by configuration type and selected scope; do not run every framework tool against every YAML file. | Unknown YAML gets generic checks only; decisions inspectable. |
| TOOL-03 | Use versioned wrapper/native contracts with timeout, cancellation, safe arguments and documented output/exit mappings. | Fixtures cover findings exits, crashes, malformed output and interruption. |
| TOOL-04 | No automatic custom checks/plugins/config from the assessed repo; exceptions require reviewed support and approval. | Malicious repo config cannot execute commands or silently suppress checks. |
| TOOL-05 | Schema/registry/advisory downloads require network approval; cached/local-first operation and freshness are visible. | Denial is reported without fabricated coverage. |
| TOOL-06 | Keep validation, security policy, secrets, renderer results and image vulnerabilities distinct; deduplicate overlapping evidence without losing engine provenance. | Findings remain attributable and counts meaningful. |
| DIST-01 | Native standalone open-source scanning baseline must not require a paid vendor-cloud account. | Baseline tests succeed without platform enrichment credentials. |
| DIST-02 | Before bundling any dependency, verify pinned-version license, notices, source obligations, platform support and binary origin. | Release manifest and third-party notice/source package review. |

## 13. BYOK assistant and reporting requirements

KEY-01: User supplies OpenRouter key; session-only or supported OS credential-store storage. No plaintext fallback, bundled key, runtime PWT account or PWT LLM gateway. Config stores only credential reference, never the key. Provide validation/replacement/deletion; absent key affects AI only.

KEY-02: Explicit configurable inference-model ID, distinct from coding-agent models. Default private-context request policy is `data_collection=deny`, `zdr=true`, `allow_fallbacks=false`. `allow_fallbacks` is a schema constant (`const false`, CA-01) and an enforced request field; changing it needs a new contract version and owner approval. A compliant route being unavailable is an explicit error; no silent model/privacy downgrade. Eligible-provider allowlist may further restrict the request.

KEY-03: Only relevant approved redacted findings/source excerpts leave the device. Do not send the entire repository or raw secrets. No automatic web/plugin tools; model cannot execute commands or modify files. Show intermediary and downstream disclosure.

RES-07: Normalised scanner evidence produces JSON and deterministic Markdown before optional AI narration. Narration is labelled, cited, bounded and cannot alter findings or coverage. Unknown citations are not invented evidence.

RES-08: Local restricted Markdown-to-PDF conversion adds a third output. Disable active code, remote resource fetches and unrestricted local file reads. Preserve JSON/Markdown if PDF fails; retain per-format status and allow regeneration without rescanning.

RES-09: Reports use timestamp/repo/run IDs to avoid overwriting. Record requested/actual inference metadata where available, source/approval IDs, tool versions and data freshness; no raw payload or API key.

## 14. UI and development delivery

The UI team receives the self-contained UI Schema and produces Figma frames/components/states. The developer uses mock/demo data against stable command/event DTOs to build a working terminal UI in parallel. Figma-to-terminal translation changes presentation by default; contract/security changes require owner approval. Mock scans never count as real acceptance.

A feature phase approval authorises its bounded implementation plus the explicitly listed local functional/regression checks. Opus delegates implementation to Sonnet 5.5, inspects the integrated changes and invokes the real verification commands. It diagnoses confirmed failures versus suspected risks and proposes a bounded solution. Haiku drafts/updates `status.md` and `handover.md` from Opus's evidence packet; Opus validates the diff before the checkpoint is accepted. STOP for the owner. No failed test automatically authorises a fix. An explicitly approved debug cycle authorises the named repairs plus listed retests; repeat only with new owner approval. Starting the next feature requires approval even if every mandatory test passes. After two unsuccessful cycles on the same defect, escalate the diagnosis and scope rather than retry indefinitely. This replaces the earlier requirement for a separate human pause between implementation and its already-authorised tests.

Persist continuity in root `plan.md`, `status.md`, `handover.md` and concise `CLAUDE.md`. Haiku is a documentation scribe, not approval/test authority. Record genuine owner decisions and actual code/evidence state. Resuming reads compact controls plus relevant plan sections and verifies Git state; no zero-token or zero-revalidation claim.

The four-week cap remains a pilot constraint, not the old five-person estimate applied to one person. Opus must estimate the actual checkout and propose a feasible release cut. No automatic safety-gate deletion to meet a date. All target capabilities, including rendered templates, image/action/dependency references and non-mutating verification, retain explicit implemented/pilot/deferred labels.

## 15. Additional acceptance gates

1. Complete tool matrix and routing scope is frozen; exact versions/platforms/flags are verified, not guessed.
2. Tool wrappers never run repository scripts, apply fixes, deploy, or inherit the model key.
3. Tests cover application-owned config, hidden exclusions, exit mappings, control characters, oversized input and network denial.
4. JSON/Markdown/PDF and normalised evidence agree; AI/renderer failures remain independent.
5. BYOK key and seeded secrets are absent from logs, durable intermediate data, RAG index, reports, Figma and releases.
6. Same-feature formal checks include relevant regression; release runs the agreed full suite and manual platform/UI checks.
7. No claim of universal YAML coverage, no payment-free claim for cloud inference/Docker Desktop, and no unverified production-certification claim.

## 16. Pilot capability release labels

Owner decisions D-01 to D-14 (2026-10-23) and D-15 (2026-10-24). This register is canonical; other documents and `plan.md` refer to it. Labels: **pilot** = in the Linux x86_64 pilot scope and acceptance; **conditional** = implemented only after a named owner decision, approval or spike result; **deferred** = retained target, not in the pilot, with any interface explicitly reserved. **out of scope** = not a project target at any stage (owner decision). A label is a scope decision, not evidence of implementation.

| Requirement / capability | Label | Pilot boundary |
|---|---|---|
| REP-01–REP-06 | pilot | HTTPS and SSH remotes (D-13); submodule recursion, LFS/filter execution and hooks disabled unless separately approved |
| DISC-01–DISC-05 | pilot | Symlinks not followed; exclusions/errors listed |
| SCAN-01 | pilot | Syntax, schema, secrets and misconfiguration tracked as separate capabilities |
| SCAN-02 | pilot | Generic YAML, GitHub Actions and Kubernetes classification; Compose/Helm detected and reported as conditional/unsupported until X2/X3 |
| SCAN-03 | pilot | Local user-supplied schemas and the approved versioned Kubernetes schema cache (D-11); runtime remote schema/`$ref` fetch is conditional on `schema_fetch` approval |
| SCAN-04 | pilot (source) / conditional (rendering) | Template sources are classified and assessed as sources; approved local rendering requires Helm extension X3 |
| SCAN-05 | conditional / deferred | Referenced images: conditional (Trivy X4, needs a pinnable build). Referenced actions, reusable workflows, dependencies and external configurations: deferred. Where a pilot parser identifies such a reference it is recorded as an explicit unsupported coverage gap, never as passed |
| SCAN-06 | deferred | Non-mutating live verification is not executed in the pilot; the `non_mutating_verification` capability is reserved and any such task is reported as deferred/skipped |
| SCAN-07, SCAN-08 | pilot | SCAN-08 behaviour per tool is verified in P00 spikes |
| SCAN-09 | deferred | Historical secret mode; `secret_scope="history"` is rejected by pilot semantic validation |
| WF-01–WF-06 | pilot | Interval/daily triggers while the app is open; reduction to manual saved workflows is a possible owner-approved scope exchange, not a default |
| RES-01–RES-05, RES-08, RES-09 | pilot | JSON, deterministic Markdown and restricted local PDF; renderer chosen after the D-07 spike |
| RES-06 | deferred | Suppression/baseline; `SuppressionFile` schema and commands are reserved, `suppressions.json` is not written by the pilot |
| RES-07 | pilot (deterministic) / deferred (AI narration, pending owner confirmation) | Reports never depend on AI |
| RAG-01 | pilot (narrowed) | Local lexical retrieval over selected findings, relevant redacted source excerpts, local approved rule descriptions and schema metadata. Logs/workflows as retrieval sources and remote documentation ingestion (`documentation_fetch`) are deferred separate capabilities |
| RAG-02–RAG-07 | pilot | Redaction before indexing/transmission, context isolation, citation validation, no execution/write tools; historical chat is deferred |
| Embedding retrieval, chat persistence | deferred | `embedding_key` stays null; `chat_persistence_enabled=false` |
| Korean (Hangul script) support | out of scope (D-15) | Korean content is still assessed as data and is never dropped silently or allowed to crash; rendering/display degrades to visible code-point labels; no Hangul font coverage requirement |
| KEY-01 | pilot (session-only) / conditional (OS credential store) | OS credential-store mode only if the Linux keyring spike verifies a supported backend; never plaintext fallback |
| KEY-02, KEY-03 | pilot | Explicit owner-supplied model ID; mock HTTP is the automated baseline; real provider behaviour unverified until a separately approved real-API acceptance check |
| UI-01–UI-03 | pilot | Demo TUI on labelled mocks, then real backend |
| UI-04 | pilot (collapse/compact) / deferred (draggable splits) | |
| Figma translation | conditional | Depends on Figma delivery (D-12); separately estimated, nonblocking; shipping demo styling needs owner approval at release cut |
| TOOL-01–TOOL-06, DIST-01, DIST-02 | pilot | Scanners are separately installed prerequisites detected by preflight (D-05); no bundling or automatic install |
| yamllint, Gitleaks, Checkov, rust-schema | pilot baseline | Versions frozen after spikes; no silent downgrade |
| actionlint, kubeconform | pilot framework adapters | kubeconform depends on the approved schema cache |
| zizmor (X1), Compose (X2), Helm (X3), Trivy (X4) | conditional | Separate install requests and owner selection |
| Windows/macOS (X5) | conditional extension | Not described as supported until their acceptance matrix passes |
| `.sh` wrapper parity (X6) | conditional extension | ShellCheck/Bats required only for scripts actually implemented |

# PWT YAML Security Workbench — UI Schema and Interaction Specification

Consolidated revision 0.3. Includes all decisions through 23 October 2026: OpenRouter BYOK; local JSON/Markdown/PDF; named tool and wrapper plan; one core developer plus a Figma design team; Opus-led verification, Sonnet implementation, Haiku documentation and owner-approved debug cycles. This specification is not evidence of implemented code or completed tests. Earlier v0.1/v0.2 wording is superseded by this consolidated set.

Version: 0.3.1 (pre-implementation amendment CA-01) | Date: 2026-10-23 | Status: Proposed UI design
Foundation: dashboard-first terminal with mouse-responsive panels and expandable chat. UI team supplies Figma designs; developer implements the demo/final terminal UI. Exact visual design remains subject to review.

## 1. Experience principles

- Operational dashboard first; conversational explanation second.
- All routine scanning tasks available without a model.
- Mouse and keyboard parity; visible focus and no colour-only meaning.
- Display scope, provenance, approval and coverage honestly.
- Remediation stays text-only; no Apply Fix button or hidden action execution.
- Use restrained dark styling, clear borders and semantic accents; support terminal-default/light/high-contrast alternatives.

## 2. Screen hierarchy

```text
ApplicationShell
├── HeaderBar
│   ├── ProductTitle
│   ├── ViewTabs: Scan | Workflows | History
│   ├── ActiveWorkflowLabel
│   └── Actions: AddRepo | Refresh | Run | Stop | Help
├── MainLayout
│   ├── SelectionPane
│   │   ├── RepoSearch
│   │   ├── RepositoryTree
│   │   ├── FileFilter
│   │   ├── FileList
│   │   └── SelectionSummary
│   ├── WorkspacePane
│   │   ├── ScanView: Progress | Findings | Coverage | Activity
│   │   ├── WorkflowView: List | Editor | ApprovalScope
│   │   ├── HistoryView: Runs | ReportDetails | Compare [deferred]
│   │   └── DetailPane: Finding | Check | File | Report
│   └── ChatPane [collapsible / expandable]
│       ├── ContextChips
│       ├── Conversation
│       ├── EvidenceDrawer
│       ├── Composer
│       └── ProviderAndDisclosureStatus
├── StatusBar
└── ModalLayer
    ├── AddRepository
    ├── ReviewScanPlan
    ├── PermissionRequest
    ├── WorkflowEditor
    ├── CancelOrExitConfirmation
    └── Settings / BYOKCredentialSetup / DependencyDiagnostics / Help
```

This tree is a product UI contract, not an existing framework schema.

## 3. Wide-layout wireframe

```text
┌ PWT YAML Security Workbench ─ Scan | Workflows | History ─ Run Stop ┐
│ Repositories / Files │ Workspace                 │ Assistant       │
│ [Filter repos...]    │ Run: 7b92... · running    │ repo: api       │
│ ☑ api [main]         │ api       ███████░  70%   │ scan: 7b92...   │
│ ◩ platform [main]    │ platform  ███░░░░░  30%   │ commit: a12...  │
│ ☐ frontend [main]    ├ Findings | Coverage | Log ┤                 │
│                     │ Sev  Repo  File  Rule      │ Conversation    │
│ Files: api           │ H    api   ...   ...       │ with navigable  │
│ ☑ deploy/api.yaml    │ M    api   ...   ...       │ source markers  │
│ ☑ chart/values.yml   ├ Selected detail ──────────┤                 │
│ ☐ config/test.yaml   │ Explanation + evidence    │ Ask [________]  │
│ 2 repos · 5 files    │ Suggestion · Ask assistant│ [Send] [Expand] │
├ running · coverage partial · 1 approval pending · Tab focus · ? help┤
└───────────────────────────────────────────────────────────────────┘
```

## 4. Responsive layout rules

Proposed initial breakpoints; validate with terminal/font testing:

| Terminal size | Layout |
|---|---|
| At least 150 columns and 40 rows | Three columns; selection 25%, workspace 45%, chat 30% |
| 100–149 columns and at least 30 rows | Selection + workspace; chat switches into workspace or opens as overlay |
| 80–99 columns or 24–29 rows | Single main pane with Selection / Results / Chat tabs |
| Below 80 columns or 24 rows | Compact navigation and minimum-size warning; active scans continue |

Dimensions use character cells, not pixels. Minima override ratios. If a splitter would make content unusable, clamp it or collapse the pane. Preserve focus, selected IDs and scroll anchors across resize; clipping must not change the actual selected file scope.

Draggable splits are deferred for the pilot; always offer keyboard/configuration alternatives. Hover/mouse tracking depends on terminal capability; lack of hover must not disable clicks or keyboard actions.

## 5. Declarative UI schema

The following YAML is a proposed implementation configuration/contract, not repository content to execute or a Ratatui-native format. Its version line is `ui_schema_version` (1.0), independent of the store contract (`schema_version` 1.2), wrapper protocol 1.0 and app command/event contract 1.0 (CA-01, resolves C6).

```yaml
ui_schema_version: "1.0"
ui:
  id: pwt-workbench
  initial_view: scan
  layout_mode: adaptive
  input:
    keyboard: required
    mouse_click: supported_when_available
    mouse_scroll: supported_when_available
    mouse_move: optional
    paste: composer_or_focused_text_input_only
  regions:
    - id: header
      kind: action_bar
      actions: [add_repo, refresh_inventory, review_scan, cancel_run, help]
    - id: selection
      kind: repository_file_selector
      selection_model: hierarchical_tristate
      controls: [repo_filter, file_filter, select_visible, clear_visible]
    - id: workspace
      kind: tabbed_workspace
      views: [scan, workflows, history]
      scan_tabs: [progress, findings, coverage, activity]
      detail_mode: split_or_fullscreen
    - id: chat
      kind: grounded_chat
      visibility: collapsible
      focus_mode: expandable
      context_required: true
      command_execution: forbidden
      file_writes: forbidden
    - id: status
      kind: status_bar
      fields: [run_status, selection_count, coverage, approval_count, shortcuts]
  modals:
    - id: permission
      blocking_for: requested_operation
      fields:
        - capability
        - purpose
        - workflow_revision
        - repositories
        - destinations
        - data_categories
        - lifetime
      actions: [approve_once, approve_workflow_scope, deny, inspect]
    - id: scan_plan
      fields: [repositories, refs, files, checks, context_inputs, external_access]
      actions: [start, edit, cancel]
  safety:
    escape_control_sequences: true
    mask_secrets: true
    display_raw_scanner_output: false
    auto_open_external_links: false
    apply_fix_action: false
  assistant:
    provider: openrouter
    credential_mode: user_byok
    storage_choices: [session_only, os_credential_store]
    plaintext_fallback: forbidden
    model_selection: explicit
    privacy: {data_collection: deny, zdr: true, allow_fallbacks: false}
  exports:
    formats: [json, md, pdf]
    pdf: local_restricted
    independent_status: true
  tools:
    diagnostic_fields: [tool, version, capability, availability, release_tier, network, license_review]
    states: [ready, missing, incompatible, unavailable, denied, deferred]
    auto_install: forbidden
```

## 6. View-model schema

Conceptual application state; persistence schemas are in the Data Schema document.

```text
AppViewModel
  active_view: scan | workflows | history | chat_focus
  focused_region: header | selection | workspace | detail | chat | modal
  terminal_size: { columns, rows }
  layout: { mode, split_ratios, collapsed_regions }
  selection: { repo_ids[], file_ids[], selector_modes, discovery_revision }
  active_run_id: UUID | null
  run_projection: { execution, assessment, coverage, progress, pending_approvals }
  finding_query: { severity[], category[], repo_ids[], text, sort }
  active_finding_id: UUID | null
  chat: { conversation_id, context, provider_status, stream_status }
  modal: Permission | ScanPlan | AddRepo | Settings | null
  notifications: sanitised bounded list
```

Input produces typed intents; the UI reducer changes view state or dispatches application commands. No widget writes JSON files or launches commands directly.

## 7. Component contracts

### Repository/file selector

Repository row fields: checkbox, display name, selected ref, update status, YAML count, selection count, latest scan status. File row fields: checkbox, relative path, detected type, syntax/scan state and exclusion reason if applicable.

- Unchecked: no eligible files selected.
- Checked: all currently eligible files selected.
- Partial: some eligible files selected.
- Disabled: unavailable for selection, with inspectable reason.
- Clicking a repository checkbox selects/clears its eligible descendants.
- Filtering changes visibility, not existing selection.
- “Select visible” explicitly applies to filtered rows; “Select all in repository” is a separate action.
- Switching active repository changes the file list but preserves selections elsewhere.
- Newly discovered files follow saved selector mode, not a visual assumption based on checkbox state.

### Progress panel

Show run/task phase, active repositories, queued/running/completed task counts, elapsed time, findings count and approval wait. Show indeterminate progress when total work is unknown. Never represent an unsupported check as 100% passed.

### Findings table

Columns: severity, category, repository, relative path, line/document if available, rule/title, engine and review status. Sort/filter are local UI operations. Distinguish empty results, no selected scope, running-not-yet-results and incomplete assessment.

### Finding detail

Show rule/tool version, commit, location, redacted evidence, impact, explanation, remediation text, coverage limitations, related findings and source references. Actions: ask assistant, copy redacted suggestion, inspect source. Justified suppression is deferred (RES-06) and not shown in the pilot. No execute/apply action.

### Coverage panel

Show each selected capability against file/framework, check outcome and reason. Summaries account for discovery exclusions, missing targets, unsupported schemas, renderer failures and denied network tasks. “No findings” always includes coverage qualification.

### Workflow editor

Fields: name, repositories/refs, explicit vs all-YAML vs pattern selection, categories, schema/renderer settings, trigger/timezone, overlap policy, approved scope and outputs. Saving increments revision; show which grants will no longer match. Schedule label explicitly states “Runs only while this application is open.”

### History

Group by run and repository. Show timestamp, workflow revision, commit, execution, assessment, coverage and report-export status. Run comparison is deferred (reserved `CompareRuns`); when implemented, a comparison warns when target scope, engine/rules or commit context differs. Opening historical evidence never silently replaces current selection.

## 8. Chat workspace

- Context chips identify repo, commit, run, files/finding and whether historical.
- “Ask about this finding” attaches evidence identifiers, not unrestricted repository access.
- Show OpenRouter, requested model, actual model/downstream provider when returned, privacy constraints and approved disclosure. Never imply local-only AI for a cloud request.
- Composer supports multiline entry, streaming stop and retry; do not steal global shortcut keys while typing.
- Citations navigate to source/file/report details; unknown citation IDs are marked invalid.
- Distinguish “scanner evidence,” “assistant explanation,” and “suggested configuration.”
- Stale context displays a warning and offers an explicit refresh; never swap provenance mid-answer.
- Permission denial or provider failure yields a local notice, not a failed scan.
- Expanded mode reuses the conversation and context, preserving dashboard state.
- Chat history persistence is deferred for the pilot; when added it is opt-in and only sanitised eligible content may be saved.

## 9. Keyboard map proposal

| Action | Key | Scope |
|---|---|---|
| Move focus | Tab / Shift+Tab | Global unless modal owns focus |
| Move in list | Up/Down; PageUp/PageDown | Focused list |
| Toggle checkbox | Space | Focused selectable row |
| Open details | Enter | Focused row |
| Search/filter | Ctrl+F | Focused list/workspace |
| Review/start scan | F5 | Outside modal; opens plan review |
| Cancel active operation | F6 | Opens cancellation confirmation |
| Focus/expand chat | F7 | Global |
| Back/close overlay | Esc | Never silently exits active application |
| Help | F1 | Global |
| Exit | Ctrl+Q | Confirm if work active |

Keys are remappable. Plain letters must remain ordinary text inside the chat/filter composer. Ctrl+C handling must preserve expected copy/input behaviour and still offer a safe interruption path; final binding needs terminal testing.

## 10. Approval dialog

Display concrete action, repository/ref, destinations, data categories, renderer/scanner identity, purpose and grant lifetime. Provide inspectable details and default focus on a non-destructive option. “Approve workflow scope” is available only for a saved workflow and visible revision.

No automatic approval countdown. A chat response, repository text or earlier broad preference cannot approve a new action. Pending requests show in status bar even when chat is focused.

## 11. Accessibility and display safety

- Pair colour with labels/icons; provide ASCII fallback for box drawing/checkbox glyphs.
- Use stable alignment and configurable density.
- Escape terminal control sequences from filenames, tool output, source and model text.
- Show complete paths through detail/inspection rather than ambiguous truncation.
- Keep focus indicators visible and restore focus after closing modals.
- Do not assume GUI-like screen-reader behaviour; include accessibility testing in platform review.
- Copy/export functions use redacted evidence only.

## 12. UI acceptance scenarios

1. Select files in two repos, filter to one, and run: selected hidden items remain visibly accounted for in plan review.
2. Resize wide -> compact -> wide while scanning: selection, focus, progress and conversation survive.
3. Deny an external capability: task becomes skipped/incomplete; dashboard stays usable.
4. Expand chat, inspect a citation and return: scan state and context remain intact.
5. Missing scanner and zero findings: no misleading clean-success screen.
6. Permission request arrives during typing: notify without approving or dropping composer text.
7. Malicious ANSI filename/model text: display safely without terminal side effects.
8. Mouse unavailable: all core actions remain keyboard-accessible.
9. Cancel/exit during scan: explain interruption, retain evidence and restore terminal on normal exit.

## 13. Open UI decisions

Hybrid layout sign-off, final theme, exact breakpoints, shortcut conflicts, screen-reader expectations, panel drag support and history comparison scope require validation. These details must not block the initial functional prototype.


## 14. Complete tool/coverage presentation

This UI is a scanner workbench, not a single opaque “security scan” button. Tool names must be inspectable without flooding the default view.

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


### Dependency diagnostics

A settings/diagnostics panel shows installed versus required tool/adapter version, role, supported platform, binary origin, availability, capability/release tier, network requirement and provisional/verified licensing status. An uninstalled optional tool is not an application crash. A missing selected capability is a visible gap; no silent installation or paid-service activation.

### Scan-plan review

Show selected repo/ref/commit/files, tool-to-file routing, additional context/renderer/value inputs, required schema/registry/advisory downloads, requested permissions and deferred capabilities. Directory-only scanners cannot silently broaden read scope. Tool defaults/repo exclusions are not hidden policy.

### Progress and details

Expose tool/task state, findings count, elapsed time and approval waits. Distinguish syntax errors, style warnings, schema failures, misconfigurations, suspected secrets and image vulnerabilities. Preserve source/rendered artifact distinction and resolved image identity. Show original engine/rule provenance even for deduplicated findings.

Tool crash/timeout, missing CRD schema, stale advisory source, network denial and unsupported framework need concrete reasons. A completed process or zero displayed findings is not automatically a clean full assessment.

## 15. BYOK setup and reporting states

Credential modal: masked key input, session-only/OS-store selection, validation, replace/delete and unavailable-store handling without plaintext fallback. No raw saved-key reveal. Model choice is explicit and independent of development Opus/Sonnet/Haiku models.

Chat states: no key, invalid key, credit limit, rate limit, privacy-compliant route unavailable, streaming, cancelled, incomplete response, provider outage, stale context, uncited/invalid citation. Scanning remains available. Context chips identify repo/commit/run/file/finding and data categories leaving the device; OpenRouter intermediary plus downstream model access is disclosed.

Report view lists separate JSON/Markdown/PDF generation, paths, time, failure and retry/regenerate states. PDF is local; do not claim success before artifact verification. AI narrative unavailable/denied/failed is separate from scanner status. Historical report content is not overwritten by a new narration.

## 16. Figma handoff checklist — self-contained design brief

The UI team designs; the core developer implements terminal components. Deliver:

1. Named Figma revision/link/source plus reference exports if access is restricted.
2. Wide, medium and compact layouts annotated in terminal rows/columns, minimums and collapse rules; pixels alone are insufficient.
3. Component library: panels/focus, tabs, action buttons, tristate checkboxes, lists/tables, inputs, status/severity badges, progress, evidence links and modals.
4. Semantic tokens, ASCII/Unicode alternatives, dark/light/high-contrast considerations; avoid dependence on browser gradients/proportional fonts or hover-only operations.
5. Interaction notes for click, scroll, keyboard navigation, filtering, select-visible versus select-all, resizing, chat expansion and source navigation.
6. States: empty/loading/running, complete with/without findings, incomplete/failed/unsupported, approval pending/denied, cancelled/interrupted.
7. Workflows, concrete approvals, credentials/privacy, dependencies, chat citations and independent export-failure screens.
8. Keyboard parity and accessible text labels beyond colour.
9. Synthetic example data only; no API keys or client scan evidence in Figma.

The Figma package is a design source, not automatically generated Ratatui code. Terminal font/DPI is user-controlled. New features/business rules are not implied by visual designs.

## 17. Demo-first integration contract

Developer implements a minimal functional demo now against stable typed mocks and actual application services as they land. Mark mock mode visibly. Use the same DTOs in demo/final UI; do not create separate business logic in presentation.

When design arrives: record revision -> map terminal-cell layout/tokens -> identify deviations -> request approval for contract changes -> update presentation -> run snapshot/input/resize tests -> perform manual terminal checks. Preserve selection, scan state, approvals and conversation provenance. Delayed design does not block scanner integration; retaining demo styling for pilot requires owner approval.

Commands and events: the single canonical vocabulary, with pilot versus reserved (deferred) status and delivery rules, is Architecture §6 "Canonical application command/event contract" (app contract v1.0). This document does not keep a separate list. Reserved commands are not rendered as available actions; designers may show them only as clearly labelled future/deferred states.

Widgets do not invoke shell/Git/scanners/OpenRouter or write authoritative JSON. Extend DTOs only through reviewed contract changes.

## 18. UI verification additions

- Mixed tool availability/versions and conditional adapters render honestly.
- Secrets/ANSI sequences cannot leak through diagnostics/source snippets/model Markdown.
- Tool/card ordering does not change effective file selection.
- No approval by default focus/countdown; no content/model-triggered execution.
- Model outage and PDF failure leave completed scanner evidence accessible.
- Snapshot tests are not proof that real mouse/keyboard/terminal behaviour passed; manual matrix remains explicit.
- Development Haiku/Opus reports are not end-user scan findings; no coding-agent console is exposed in the shipped RAG panel.

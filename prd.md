Version: 0.1 draft | Date: 2026-11-12 | Status: proposed implementation baseline

# PWT Product Requirements Document

## 1. Product definition
PWT is a local-first desktop workbench for assessing YAML/YML using installed open-source security tools. It discovers selected files, runs applicable scanners, normalizes and redacts evidence, and retains session-specific JSON and Markdown reports.

The desktop GUI has a terminal-inspired aesthetic; it does not run inside a terminal and does not expose an unrestricted shell. An existing coding agent connects through MCP for evidence retrieval, explanation, and agent-owned editing. PWT is not an embedded chatbot or autonomous remediation engine.

## 2. Decision status
Confirmed direction from the owner: rebuild from a fresh repository; replace the TUI with a terminal-like GUI; retain the scanner-wrapper philosophy and external coding-agent integration.
Owner-confirmed direction for this rebuild: Java is the implementation language (JavaFX desktop GUI, Java MCP SDK, Maven build). The desktop GUI has a terminal-inspired appearance; it is not a TUI and not an embedded terminal emulator. PWT orchestrates installed open-source scanners through Java adapters; an external coding agent handles conversation and approved YAML edits; MCP exposes PWT operations to that agent. No embedded chatbot, cloud backend, or Rust application core.
Proposed technical decisions: JavaFX desktop UI, SQLite local state, Java MCP SDK, Maven build. These are implementation recommendations, not historical owner approvals.
Existing Rust code is reference material. Reuse verified rules, fixtures, and contracts where useful; do not mechanically translate an entire old architecture.

## 3. Users and value
Primary users: developers, security engineers, penetration testers, and red-team practitioners assessing repositories they are authorized to inspect.
Value: one honest assessment workspace rather than manually assembling scanner commands and reconciling outputs. PWT must distinguish a clean result from an incomplete assessment.

## 4. MVP user journey
1. Launch the desktop app and register a local repository/directory.
2. Discover YAML beneath the selected root; review explicit exclusions and select files.
3. Inspect scanner availability and applicable checks.
4. Run an assessment and see progress, cancellation, and scanner outcomes.
5. Inspect normalized findings, locations, redacted evidence, and coverage gaps.
6. Open retained JSON/Markdown reports and compare runs when snapshots are comparable.
7. Configure a coding agent to launch the PWT MCP entrypoint with the same local data directory.
8. Ask the agent to inspect findings; let the agent propose and perform edits under its own approval controls.
9. Explicitly rescan the edited files and inspect the new results.

## 5. Functional requirements and acceptance
| ID | Requirement | Acceptance |
|---|---|---|
| F01 | Register and select multiple local roots | Canonical duplicate roots are rejected; missing roots remain visible as unavailable |
| F02 | Safe YAML discovery | Finds .yaml/.yml case-insensitively below the root; no symlink traversal or parent traversal; excluded paths explained |
| F03 | Explicit target selection | Run persists selected paths and a content snapshot; no silent expansion |
| F04 | Scanner inventory | Reports installed/missing/unsupported versions and executable resolution without installing automatically |
| F05 | Real scanner adapters | Each adapter declares scope, capabilities, supported versions, exit semantics, and output limits |
| F06 | Honest outcomes | Clean, findings, failed, skipped, timed-out, cancelled, and unavailable are distinguishable |
| F07 | Normalized findings | Stable fields, source tool/rule, severity, path, location, and redacted evidence |
| F08 | Execution control | Bounded concurrent tasks, cancel, timeout, output-size limits, child-process cleanup |
| F09 | Durable sessions | Restart retains history; unfinished runs are reconciled, not displayed as completed |
| F10 | Reports | Every terminal run produces or can regenerate schema-versioned JSON and Markdown, including failures |
| F11 | Terminal-inspired GUI | Repo/file navigation, scanner controls, progress, findings, details, history, and redacted logs |
| F12 | MCP access | Registered-repo discovery, run inspection, findings, reports, and explicit scan initiation share GUI state |
| F13 | No covert mutation | No PWT YAML edit, commit, push, shell, or remediation tool in MVP |
| F14 | Before/after comparison | Matching rule fingerprints and coverage expose added/persisting/not-observed findings without unsupported claims of resolution |
| F15 | Secret handling | Seeded secrets absent from all persisted outputs, logs, exports, GUI, and MCP responses |
| F16 | Offline operation | Installed local scanners operate without a cloud account; no telemetry or model API is required |

## 6. MVP scanner scope
The intended complete scanner/validation scope for the rebuild is exactly these six integrations:
1. yamllint, 2. Gitleaks, 3. Checkov, 4. actionlint, 5. kubeconform, 6. local user-supplied schema validation.
These are the planned integrations, not optional candidates. Implement them incrementally: the first real scanner adapter is a Phase 2 milestone, not the final scope; full scanner integration belongs to Phase 5, with hardening in Phase 6.
Syntax validation is not schema validation. Schema coverage must name the supported document family and schema version. Arbitrary YAML does not automatically receive Kubernetes, CI, or cloud validation.

## 7. GUI contract
Desktop layout: repositories/files on the left; progress and findings centrally; evidence detail alongside or below findings; collapsible redacted execution log; history view; action palette.
Dark default theme, monospace presentation, readable contrast, severity labels in addition to colors, keyboard navigation, mouse support, selectable text, and resizable panels.
No fake terminal animation, unrestricted shell prompt, or built-in AI chat. UI must remain responsive during scans; detailed rendering and performance budgets are established with the first vertical slice.
MCP connection status must reflect actual observed sessions or say unknown; sharing a database alone is not a connection indicator.

## 8. Nonfunctional requirements
Local-only storage by default; restrictive permissions where supported; bounded parsing and execution; durable transactions; no raw secret evidence retained by default; atomic report publication; explicit database migration versions.
Initial validation target: Linux development environment. Windows/macOS releases require separate scanner and packaging validation; do not label them supported before passing checks.

## 9. Not in MVP
Cloud dashboard, Supabase, embedded LLM, model billing, remote repository cloning, scanner auto-install, arbitrary shell, Git commit/push, autonomous edits, scheduling, workflow designer, PDF, extension marketplace, and whole-repository vulnerability coverage beyond declared adapters.
PDF, named workflows, scheduling, richer schema catalogs, and additional operating systems belong in a later backlog. This deliberately narrows the historical roadmap; it does not mark those features completed.

## 10. Release gates
Release only when all MVP requirements have test evidence or explicit owner-approved deferral; one real scan is reproduced from a clean setup; failure cases remain visible; GUI and MCP agree on IDs and results; seeded-secret leakage tests pass; no known critical path escape or command-injection defect remains.
No completion percentages without a denominator and evidence ledger.

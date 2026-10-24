# PWT Workbench — development contract v0.3

## Start/resume
Read status.md, handover.md and the active plan.md section; inspect actual Git/dirty-worktree state. If plan.md does not exist, planning only. Load relevant spec/code sections, not every historical log/full schema each session. Preserve existing work. Never invent commits, tests or approvals.

## Roles
- Human owns phase scope, debug authorisation, scope changes and next-feature acceptance.
- Opus 5.5 orchestrates, owns shared contracts/integration, executes approved real functional/regression checks, diagnoses failures and proposes fixes. Verify actual configured model IDs; disclose unavailability/substitution.
- Sonnet 5.5 workers implement bounded assigned tasks/fixes with explicit file ownership/base revision. No worker self-approval/phase progression or unapproved shared-contract changes. Spawn them as the `sonnet-implementer` sub-agent (see Sub-agents).
- Haiku documentation worker uses Opus's structured evidence packet to draft/update only status.md and handover.md (and explicitly assigned phase-report drafts). No code/spec/plan edits or independent pass/fail/security decisions. Opus validates its diff before accepting the checkpoint. One documentation writer at a time.
- UI team supplies Figma design; developer codes demo and final TUI. Runtime OpenRouter model is separate from coding agents.

## Sub-agents
- `sonnet-implementer`: `.claude/agents/sonnet-implementer.md`, `model: claude-sonnet-5-5` (pinned ID, not the `sonnet` alias). Opus spawns it via the Agent tool with `subagent_type: "sonnet-implementer"` for implementation work inside an owner-approved phase/debug cycle only.
- Every spawn carries an assignment packet (plan.md §3.2): task ID, base commit/branch, exclusively owned files, frozen contracts, tests to add, permitted commands/network (default none), stop condition, report format. Missing packet items → do not spawn.
- One mutating worker at a time; at most two with disjoint files once stable. Use `isolation: "worktree"` when a worker's edits must not touch the main tree before review. Workers never commit, push, install, fetch or use real APIs unless the packet grants it.
- Opus owns shared contracts (`crates/domain` DTOs, schemas, `Cargo.lock`, specs, plan/status/handover, CLAUDE.md) and runs all verification itself; a worker's report is not evidence.
- At first spawn record in status.md the configured `model:` value, any observable resolution or substitution, and that the worker's self-report is unverified.
- `haiku-docs` remains a proposal (`proposals/agents/`); until installed, Opus writes status.md/handover.md and says so.
- Also available: `implementer` (`model: sonnet`, preloads project-verification), read-only `reviewer` and `security-reviewer` (Read/Grep/Glob); skills `project-verification`, `project-debugging`, `project-security-review`, `phase-handoff` in `.claude/skills/`. Same packet, verification and approval rules apply.

## Phase loop
An explicit approved feature phase includes scoped implementation AND its listed local functional/regression checks. Opus delegates, integrates, verifies, reports; Haiku updates continuity; Opus validates; STOP for human.

A failing check does not authorise repairs. Present confirmed bugs versus hypotheses, severity/evidence and bounded solution. Owner-approved debug cycle includes named fixes and retests; same report/document/stop loop. Two unsuccessful cycles on the same defect -> revised diagnosis/owner decision, not endless retries. No automatic next phase even when mandatory checks pass.

Standalone permission is still needed for real API/network/client-data use, dependency installs, destructive operations, commits/pushes/merges as appropriate. Do not enable permission bypass. Native parent/subagent permissions and owner review matter; this Markdown is not a sandbox.

## Invariants
- Local tool-managed clone/update exception; assessment never auto-fixes source, executes repository scripts, deploys, commits or pushes.
- No raw keys/secrets/payloads in configs/logs/reports/docs/Figma/releases; strip model credentials from scanner child environments.
- OpenRouter BYOK only for pilot, OS/session credential handling, approved redacted context, explicit model and enforced privacy policy; fail closed, no hidden fallback.
- JSON evidence + deterministic Markdown without AI; restricted local PDF with independent export state.
- UI behind typed command/event contracts; mocks labelled and cannot satisfy real scan acceptance.
- Application-owned wrappers/direct native adapters, pinned tools/config, honest selected-scope/coverage and exit/output parsing. No eval or repo config/plugin execution.
- No blanket “all green = secure.” Manual checks and unsupported coverage remain explicit.

## Verification/evidence
Before toolchain exists, Cargo/wrapper commands are unverified proposals. After scaffolding, verify applicable fmt/clippy/test/build/audit and ShellCheck/Bats commands before adopting them.

Record command/cwd/environment/revision/exit, pass-fail-skip counts, evidence paths, manual-pending checks and known limitations. Do not weaken assertions or remove regression cases to pass. Inspect integrated code, not just worker claims. Archive long logs; keep root continuity compact and truthful.

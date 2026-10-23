# Proposed CLAUDE.md amendment (for owner review — not applied)

Proposed additions to `CLAUDE.md` (development contract v0.3 → v0.3.1). Existing text is kept; these lines would be appended as a new section.

```markdown
## Pilot baseline (owner decisions 2026-10-23, plan.md §1)
- Specs v0.3.1 with amendment CA-01; store schema 1.2, wrapper protocol 1.0, app command/event contract 1.0 (Architecture §6, canonical), UI schema 1.0. Canonical schema file: schemas/workbench-store-v1.2.schema.json; regenerate embedded/wrapper copies from it.
- Linux x86_64 first pilot target; Windows/macOS unverified extensions.
- Native Rust process adapters and direct Git; .sh wrappers are extension X6. ShellCheck/Bats only for scripts that exist.
- Capability labels: PRD §16 is canonical (pilot/conditional/deferred).
- Scanners are user-installed prerequisites found by preflight; no auto-install/sudo/bundling.
- Production OpenRouter model ID is owner-supplied; mock HTTP for tests; real API only with separate approval.

## Agents
- Worker definitions: .claude/agents/sonnet-implementer.md (model: sonnet), .claude/agents/haiku-docs.md (model: haiku). Record configured model and any substitution at first use; self-reports are not proof.
- Until the Haiku agent is installed, Opus writes status.md/handover.md and says so.

## Evidence and Git
- Evidence under evidence/<phase>/ and evidence/debug/<cycle>/; see evidence/README.md.
- Phase branches phase/Pxx-<slug>; no commit/merge/push without explicit approval; status.md records real HEAD and dirty state.
```

Optional enforcement (also for review): project `.claude/settings.json` permission rules restricting the documentation agent's edits to `status.md`/`handover.md`, since agent Markdown is not a sandbox. Not drafted until the owner chooses whether to adopt it.

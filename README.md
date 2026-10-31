# PWT Workbench — consolidated specification v0.3

Date: 2026-10-23. This package supersedes earlier specification revisions for planning; it does not overwrite files on your computer or establish implemented functionality.

## Package

Five canonical root Markdown specifications are the deliverables. The ZIP also includes extracted store/wrapper schemas, revised CLAUDE.md/bootstrap guidance, a constrained Haiku documentation-agent template and revision notes. No application code or pre-approved plan.md is included.

## Use

1. Review/merge the five root specifications into the actual project. Preserve existing source and real progress records.
2. Send the UI Schema to the Figma team; developer implements the demo/final terminal UI.
3. Merge the supplied CLAUDE.md and agent/workflow changes with existing control files. Do not keep the older contradictory rule that only Opus may physically write status/handover; Haiku can draft those files, with Opus validation. Do not erase real approval history.
4. Have Opus inspect the checkout and create plan.md using the revised bootstrap. Each approved feature phase includes bounded implementation and its specified tests/regression. Debug/fix cycles and the next feature require separate owner approval.
5. Include the full tool/wrapper roadmap and explicit pilot/conditional/deferred labels in the plan. Proposed baseline is not universal coverage or licence clearance.
6. Pin tool/model versions, CLI flags, OS targets and output/exit contracts during spikes. No API keys/client data in planning or fixtures.

Document v0.3.3 (amendments CA-01 2026-10-23, CA-02 2026-10-24) and store schema v1.3 are separate. Wrapper protocol is v1.1; app command/event contract is v1.0; UI schema is v1.0. CA-01 changed contract text before any implementation existed and kept the version identifiers; see Data Schema §22. CA-02 (§23) adds descendant-cleanup, child-signal and stream-state fields and is versioned (store 1.2 → 1.3, protocol 1.0 → 1.1); records under 1.2/1.0 are not valid under 1.3/1.1 (§19). Migration/read-only historical handling is specified in the Data Schema document. Existing stored reports cannot be upgraded merely by changing the version string.

No current app tests, scanners, API calls, platform installs or Git operations were executed in preparing this package. Package verification covers document structure and schema fixtures only.

## Start here (current state, 2026-10-31)

The sections above describe the original v0.3 specification package and are kept as history. Current facts:

- **Product (intended, not yet built in full):** a configuration-security workbench combining established checks, automatic evidence-grounded LLM analysis and interactive RAG investigation. Automatic analysis is a newly explicit requirement (2026-10-31, D-18) whose design direction (CA-05) the owner accepted with qualifications the same day (`planning/CA-05-automatic-evidence-analysis.md`); it is not implemented, and canonical schema/application changes are not yet authorised.
- **Specs/contracts now:** spec v0.3.5 (CA-01–CA-04), store schema 1.4 (`schemas/workbench-store-v1.4.schema.json`), wrapper protocol 1.1, app contract 1.1, UI schema 1.1.
- **Code:** Rust workspace in `workbench/` (crates `domain`, `application`, `persistence`, `cli`, plus placeholders `repositories`, `scanning`, `retrieval`, `terminal-ui`). P00–P02 are accepted and merged into `main`; P03 (permissions, child environment, session credentials) is in progress on `phase/P03-permissions-credentials`.
- **Read in this order:** `CLAUDE.md` (working contract) → `status.md` (live state, open owner decisions) → `handover.md` (how to resume, how to run checks, lessons) → `plan.md` (§4 roadmap, §5C current phase) → `planning/P03-T01-request.md` (frozen P03 interfaces).
- **Checks:** only through `dev/p03_check.py` on the P03 branch (see `handover.md`); do not run `cargo` directly.
- **Agents:** active definitions are in `.claude/agents/` and skills in `.claude/skills/`; `proposals/` holds not-yet-adopted items (`haiku-docs`, a CLAUDE.md amendment) and the original `sonnet-implementer` proposal; the active copy is the one in `.claude/agents/`.

## Repository state note (2026-10-23, historical)

This repository contains the five specifications, `CLAUDE.md` and `schemas/`. The bootstrap guidance, Haiku documentation-agent template and revision notes mentioned above are not present here. Planning controls now live in `plan.md`, `status.md` and `handover.md`; evidence in `evidence/`; agent definitions and the CLAUDE.md amendment are in `proposals/` awaiting owner review (not active).

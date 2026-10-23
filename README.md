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

Document v0.3.1 (pre-implementation amendment CA-01, 2026-10-23) and store schema v1.2 are separate. Wrapper protocol is v1.0; app command/event contract is v1.0; UI schema is v1.0. CA-01 changed contract text before any implementation existed and kept the version identifiers; see Data Schema §22. Migration/read-only historical handling is specified in the Data Schema document. Existing stored reports cannot be upgraded merely by changing the version string.

No current app tests, scanners, API calls, platform installs or Git operations were executed in preparing this package. Package verification covers document structure and schema fixtures only.

## Repository state note (2026-10-23)

This repository contains the five specifications, `CLAUDE.md` and `schemas/`. The bootstrap guidance, Haiku documentation-agent template and revision notes mentioned above are not present here. Planning controls now live in `plan.md`, `status.md` and `handover.md`; evidence in `evidence/`; agent definitions and the CLAUDE.md amendment are in `proposals/` awaiting owner review (not active).

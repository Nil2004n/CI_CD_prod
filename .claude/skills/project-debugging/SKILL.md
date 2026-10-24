---
name: project-debugging
description: Reproduce a failed check or spike run in this repository and identify its root cause without repairing it. Use when a build, test, harness or verification step failed and a diagnosis or debug-cycle proposal (Pxx-Dnn) is needed.
allowed-tools: Bash, Read, Grep, Glob
---

# Project debugging (PWT Workbench)

A failing check authorises **no repair** (CLAUDE.md "Phase loop"). Your output is a diagnosis plus a bounded proposal, then stop.

## Steps
1. **Freeze the evidence.** Locate the failing record under `evidence/` and note its path, command, revision and exit code. Do not edit or delete it.
2. **Reproduce** only if the current approval covers re-running that exact command, in the same isolated environment (see the `project-verification` skill for verified commands). Otherwise diagnose from the existing logs, results and source alone, and say so.
3. **Read the source** of the failing component and of the relevant dependency (crate sources: `~/.local/share/pwt-dev-tools/_cargo-home/registry/src/index.crates.io-*/`, read-only). Cite `file:line`.
4. **Classify each finding:**
   - **Confirmed:** shown by a log, an output or a source line.
   - **Hypothesis:** plausible but not shown; state what evidence would confirm it.
   - **Harness/process defect vs product/library behaviour vs criterion mismatch.** A criterion the library cannot meet is reported to the owner. Never weaken a criterion, delete a regression case or change an expectation after seeing output.
5. **Propose a bounded debug cycle `Pxx-Dnn`:** the named fixes, files touched, the retests (full formal re-run in a new evidence subdirectory, earlier runs preserved) and the owner decisions required. If the same defect has already failed two cycles, propose a revised diagnosis or architecture decision instead of another retry.

## Never
- Patch code or tests, add dependencies or features, raise limits, disable a check, or silently replace a formal run.
- Claim a fix works without an approved re-run.

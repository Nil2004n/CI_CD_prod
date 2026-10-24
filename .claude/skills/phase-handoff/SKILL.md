---
name: phase-handoff
description: Close an approved stage or debug cycle by updating only status.md and handover.md from real Git state and existing evidence. Use at the end of a stage, before stopping for the owner. Not for plan/spec/code edits or for deciding pass/fail.
allowed-tools: Read, Grep, Glob, Edit, Bash(git status:*), Bash(git log:*), Bash(git diff:*), Bash(git rev-parse:*), Bash(git branch:*), Bash(git ls-remote:*)
---

# Phase handoff (PWT Workbench)

You record facts; you do not decide them (CLAUDE.md "Roles", Arch §18).

## Steps
1. **Collect the evidence packet** from real sources only:
   - phase/cycle ID and the owner's actual decisions (latest owner message and `status.md` Approvals);
   - `git rev-parse HEAD`, `git status --short`, `git branch --show-current` and, if relevant, `git ls-remote origin <branch>`;
   - commands with exit codes and pass/fail/skip counts, taken from `evidence/**/record.md` and its logs;
   - manual-pending checks, known limitations, blockers, and the next permitted action.
2. **Edit only `status.md` and `handover.md`.** Keep them compact and link to `evidence/` for detail. Preserve approval history; append, do not rewrite it.
3. **Disclose the writer:** "Updated by Opus directly (Haiku documentation agent not defined/approved)" while `haiku-docs` is not installed.
4. Write state as it will be after any approved commit only if that commit is actually being made in the same step; otherwise mark the files as uncommitted.

## Never
- Invent tests, counts, commits, pushes or approvals; upgrade "unverified" to "passed"; or classify vulnerabilities as confirmed.
- Edit code, specs, `plan.md`, `CLAUDE.md` or evidence records.

## Final report
The files changed, the facts that could not be placed (marked "unverified"), and the exact next approval needed.

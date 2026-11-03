# Developer handover — P04-T04, 2026-11-03

Start with `status.md`, `plan.md` §5E, and `evidence/P04/T04/record.md`, then inspect `git status -sb`, `git log --oneline -5`, and `git worktree list`. The current branch is `phase/P04-git-acquisition`. The owner requested the T04 checkpoint commit and push to that branch; verify actual HEAD and remote state. No merge or further phase is authorized by that request. Local `AGENTS.md` is ignored and is not included in a fresh checkout; consult the active session instructions, `CLAUDE.md`, and any applicable repository instructions.

## Current state and next gate

P00–P03 were accepted and merged. P04-T01–T03 completed design and review; T04's fail-closed B0 scaffold passed its local final review, but P04 has not been accepted and T05–T12 have not started. Ask the owner to authorize the first implementation wave before assigning T05 remote intake, T06 Git runner and T07 registry/locks. Packet templates in `planning/P04-T01-packets/` are drafts: fill base revision, ownership, frozen interfaces, commands and tests, and use disjoint worktrees as required by the active development contract. Do not treat a worker report as verification.

Read `planning/P04-T01-request.md` §10 and its interface sections, `planning/P04-T01-git-safety.md`, `planning/P04-T01-CA06-proposal.md`, `planning/P04-T03-contract-review.md` and `planning/P04-T03-approval-amendment.md`. The owner approved CA-06 as store 1.5 and future CA-05 as 1.6 and approved the cross-client direction; canonical schemas and DTOs are not amended. The incompatible-legacy-ref policy and cross-root migration journal remain explicit gates; no real data migration is authorized. URL-only registration requires separate pre-discovery and resolved-target authorizations, and successful Git version provenance is to be retained.

## What T04 actually provides

`repositories::{remote,git,lock}`, application registry/service and a typed, unwired CLI seam exist. They do not implement repository registration, clone, update or CLI operations: Git environment, runner and fixed SSH policy constructors and service calls fail closed. The Git child-env profile admits only the exact `SSH_AUTH_SOCK` name after absolute-path, NUL, parent-component and secret checks. T06 must implement and test the remaining trusted SSH policy, pinned Git runner and credential behavior; do not infer these guarantees from the stub.

The empty `fixture-transport` feature and its self-dev-dependency were removed with owner approval because a release build could enable it. Explicit feature activation is now rejected. Do not add fixture transport until a test-only design and release negative controls are reviewed; the T08 tests remain pending. Static rules are lexical guardrails, and the current source has no Git process invocation; the full future call path needs review and tests. SSH authentication, host-key handling, hostile Git configuration, safe deletion and process escapes are not verified. Real-remote checks and any network, install or actual migration require their own authorization.

## Verification and environment

The final T04 run from repository root was:

```sh
python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/final-review
```

It exited 0: 1,140 passed, 0 failed, 0 ignored across 56 suites; 41 compile-fail cases; 101 CLI tests passed; fmt, clippy, build and static checks passed. See `evidence/P04/T04/record.md` and `final-review/` for logs, revision and earlier failures. The namespace-identity probe shows a distinct namespace; it is not an external-connectivity test. No current cargo-audit/advisory database was found, so no current audit is claimed.

`dev/p04_check.py` is host-specific: it points to `/home/debjoyy/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin` (Rust/Cargo 1.99.1) and `/tmp/p04-t04-devtools`, with a copied offline Cargo registry, separate target directory, `env -i`, `unshare -rn`, `--frozen`, and dev/test debug info disabled. These paths and the temporary cache may not exist on takeover. Inspect and deliberately adapt them without weakening isolation or test floors; obtain permission for installs or network. The older P03 runner points to absent `/home/kali` paths and Rust 1.97.1. `/tmp` had only about 480 MiB free after T04 checks: inspect capacity before rebuilding, and do not delete worktrees or artifacts without authorization. `workbench/target/` was removed with owner approval, not the source or dependency cache.

Prior T04 CLI verification had a single exit-3 lock failure, followed by a passing unchanged retest. The test now uses the suite's bounded exit-3 retry; lock inheritance is plausible but not conclusively reproduced. Full history is in the T04 record. Worktrees in `/tmp` are temporary; `git worktree list` showed both retained T04 worker trees and missing/prunable older entries. Preserve them unless cleanup is authorized. Worker models were requested as `gpt-6.1-terra`, but resolved identities and worker self-reports were not independently verified.

# P04 wave 1 — initial integration, 2026-11-03

Owner authorized T05–T07 with GPT-6.1 Terra workers, primary integration/testing, and no commit or push. Base `f71749c53704afbd87150ac6d31b65f69377000b`, branch `phase/P04-git-acquisition`; integrated checkout dirty. Workers were requested as `gpt-6.1-terra` in separate worktrees `/tmp/p04-t05-remote`, `/tmp/p04-t06-runner` and `/tmp/p04-t07-registry`; resolved identities are not independently observable and their reports are not test evidence. No network, installs, actual data, credentials, commits or pushes were used.

Integrated T05 remote validation with synthetic tables; hostname safety is lexical and does not establish DNS or redirect isolation. T06 currently implements bounded stdin and a constrained environment, but invocation/output construction, process execution and SSH policy remain unavailable. T07 has registry reads and a Linux checkout-lock draft; registry writes remain unavailable because existing signatures do not provide a write clock and `replace` also lacks operation/status/reason for its event. The existing schema still cannot represent the approved unresolved reference; migration and journal decisions remain gates. Checkout deletion/adapter and lock-order enforcement are not implemented. Lock path checks are not a defence against all same-user substitutions.

## Primary verification

Rust/Cargo 1.99.1, `dev/p04_check.py` isolated copied offline Cargo cache, `unshare -rn`, `--frozen`, debug info disabled. Commands run from repository root:

| Command | Exit | Result |
| --- | --- | --- |
| `python3 -I -B dev/p04_check.py test --package repositories --slot lead --out evidence/P04/W1/initial-repositories` | 0 | Before T07 integration: 5 passed, 0 failed, 0 ignored, across 5 suites. |
| `python3 -I -B dev/p04_check.py test --package repositories --slot lead --out evidence/P04/W1/integrated-repositories` | 0 | After integration: 8 passed, 0 failed, 0 ignored, across 6 suites. |
| `python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/W1/integrated-initial` | 1 | fmt/test/build/CLI pass: 1147 passed, 0 failed, 0 ignored; 41 compile-fail; CLI 101/0/0. Clippy fails on two collapsible `if` statements and unspecified truncation semantics in lock open; static rejects filesystem use in `repositories/src/lock.rs` under the inherited P03 restriction. Logs/summary in `integrated-initial/`. |

Confirmed check failures require an owner-approved debug cycle. Proposed bounded fixes: collapse the two conditionals, specify `.truncate(false)` for the lock file (never truncate a held lock), add a narrowly scoped static filesystem exception for the approved lock module with positive/negative controls, then rerun the full checks without weakening floors. Review no-follow/ownership/identity and same-user race limits independently. Separate owner decision is pending on supplying clock and event details through the internal registry seam; no schema change is proposed. Safe operation-specific Git invocation, pinned executable/cwd trust and a release-isolated test fake remain unimplemented and need a concrete reviewed design. The test-only transport was removed at T04 and must not be reintroduced as a release-enabled feature.

`/tmp` had approximately 196 MiB free after verification; the isolated target is temporary and must not be cleaned without authorization. The project filesystem had 4.6 GiB free. No wave 1 acceptance or B1 is claimed; do not commit or push without the owner.

# P03 B3 — wave 3 (T10 CLI, T11 hostile fixtures + static rule) integrated

Owner approval 2026-10-31: "I approve wave 3, start T10 and T11 …". Base `3acc301` (packets `planning/P03-packets/{_wave3-common,T10-W2,T11-W1}.md`; code = B2 `4d6c70f`, accepted). Workers `sonnet-implementer` (`model: claude-sonnet-5-5`): W2 = T10 (`p03/t10-w2`), W1 = T11 fixtures (`p03/t11-w1`). Coordination log copy: `coordination-log.md`. Worker reports are self-reports; evidence below.

## Integration
- W2: `application/src/dev_permission.rs` (last `todo!`), `cli/src/{args,commands,output,features,lib}.rs`, `cli/tests/p02_scenarios.rs` (feature-index staleness test; `help_lists_every_command` updated to the exact 16-command list after a lead decision), new `cli/tests/p03_cli.rs` (35) + 11 unit tests.
- W1: new `application/tests/p03_hostile.rs` (27; 16 production mutations caught in a scratch copy, worker-reported).
- Lead: `dev/p03_check.py` static rule 12 — `Evaluation::Permitted` and `permission::evaluate` only in `application/src/approvals.rs` and `permission/` (D01 H1); 14 new self-test cases (104/104).
- Lead decisions: help lists all commands (assertion stays exact, 7 → 16); no FR-33 CLI seam in T10 (gap recorded).

## Lead verification
- `python3 -I -B dev/p03_check.py all --final --slot lead --out evidence/P03/B3/check-lead-final` (cwd repo root): **OVERALL PASS** — fmt, clippy `-D warnings`, test 53 suites **1126/0/0**, 0 ignored, 41 compile_fail, build, cli 94, static incl. rule 12 and no `todo!` (`--final`). 1053 + 46 + 27 = 1126. Floors raised to 1126/41.
- Lead seeded-key sweep against the real binary in `unshare -rn` (`lead_sweep.py`, `lead_sweep.out`): 24 processes (store init, credential set/status/delete, env-preview ×7 tools × human/JSON, permission-check 6 → approve → 0, approvals list); key and an 8-byte core absent from every output, argv and all 8 files under the data dir. `resource_constraints` deliberately not seeded (stored verbatim, sanitised — recorded limitation).

## Reviews (read-only `reviewer` and `security-reviewer`, Sonnet, over B1–B3)
No high finding; no authorisation path outside `ApprovalService::authorise`; no key leak path found. Confirmed medium items go to debug cycle P03-D02 (`evidence/debug/P03-D02/record.md`). Recorded, not fixed in P03 (owner/later phase): R7/R8 ignore inference scope (owner question, pinned by `p03_hostile.rs`); approvals store has no integrity binding — same-user edits/CLI approve (P06 entry gate: scanner children must not reach the data dir or CLI); no cap on pending records; post-write emit failure message; stale `--scope once` UX note; std stdin buffer residue (best effort only); identical-looking approvals except scope.

# Status

Updated: 2026-10-29 by Opus directly (no Haiku documentation agent installed). **P02 closed:** accepted by the owner, merged into `main` and pushed. **P03 planned (plan §5C, 12 tasks T01–T12, decisions C1–C10 = recommended options, recorded 2026-10-29) on `phase/P03-permissions-credentials`; T01 not started (waits for the owner's go).**

Full history up to P02 close, verbatim: `evidence/P02/continuity/status-at-P02-close.md` (sha256 `ca2fb867…`). P00/P01 detail lives there and in `evidence/P00/P00-report.md`, `evidence/P01/P01-report.md`.

| Item | State |
|---|---|
| Stage | **P03 planned 2026-10-29; C1–C10 = recommended recorded; T01 awaits the owner's go** (plan §5C: permission evaluator + fingerprint, approval lifecycle, child-environment sanitiser, session-only credentials with KEY-01 hardening, FR-33, CLI `approvals`/`credential`/`dev env-preview`). Waves: T05 ∥ T06 ∥ T07, then T08 ∥ T09, then T10; T11 integration, T12 report. Estimate 16–24 h (low confidence). No code written |
| P03 | Planned; **C1–C10 = 1a…10a** (owner: "update plan with the recommendations"): full scope, session-only credentials (no OS store/keyring), no new crates (`application → sha2` edge only, std-only clearing), one-invocation CLI key, stdin-only key input, ASCII-only hosts, once/run/workflow lifetimes, CA-05 only if needed and owner-approved, up to 3 workers + phase-branch commits/pushes, pre-authorised debug cycles (2 attempts). Branch `phase/P03-permissions-credentials` from `main` `01a3cbd` |
| P00 | Accepted 2026-10-24 with qualifications; merged `5888826`. Report `evidence/P00/P00-report.md` |
| P01 | Accepted 2026-10-26; merged `7324da6`. Report `evidence/P01/P01-report.md`, freeze `evidence/P01/P01-freeze.json` |
| P02 | **Accepted 2026-10-29** ("I accept p02, commit and push to the phase branch, merge to the main branch"); T15 `84fedd4` pushed to `phase/P02-store`; `--no-ff` merge `4747214` into `main` (tree = `84fedd4`); `main` pushed (owner: "commit and push main"). Report `evidence/P02/P02-report.md`, freeze `evidence/P02/P02-freeze.json` |
| Verification on `main` | `python3 -I -B dev/p02_check.py all --slot lead --final --out evidence/P02/merge/check-main` (cwd repo root, `4747214`): **OVERALL PASS** — fmt, clippy `-D warnings`, test 41 suites 866/0/0, 0 ignored, 39 compile_fail, build, cli 48, static 28/28. Floors 866/39 (`dev/p02_baseline.json`) |
| Manual check | **Closed 2026-10-29: PASS.** `real_storage_failure_exits_4_when_permissions_are_enforced` run with uid 1000 in `unshare -Un --map-current-user` (network still isolated); the enforced branch is shown to have run (`eacces-read`/`eacces-write` temp dirs observed); root-mapped control skipped it as expected. `evidence/P02/manual-exit4/record.md`. The runner itself still skips this branch |
| Specs / contracts | Spec v0.3.5 (CA-01–CA-04); store 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1; schema `schemas/workbench-store-v1.4.schema.json` sha256 `909e2a55…` |
| Dependencies | `serde =1.0.229`, `serde_json =1.0.154`, `jsonschema =0.60.1`, `sha2 =0.11.1`, `libc =0.2.191`; `Cargo.lock` sha256 `6c9d4998…` (94 packages, 86 registry). cargo-audit: stage C-A only; C-A2–C-E not approved |
| Git | `phase/P03-permissions-credentials` = the planning commit containing this file, pushed; `main` = `origin/main` = `01a3cbd`. Phase branches `phase/P00-decisions-spikes`, `phase/P01-foundation`, `phase/P02-store` (`84fedd4`) kept locally and on origin, all merged. No worktrees. Untracked `.commandcode/` (local, never commit) |
| Disk / tools | Clean-up 2026-10-29: lead Cargo target dir deleted (next runner call rebuilds offline from `_cargo-home`), `git gc`. `pwt-dev-tools` 819 MB; disk 68 %. Worktree archive `~/.local/share/pwt-dev-tools/_archive/p02-worktrees-2026-10-28.tar.gz` kept |
| Agents | `sonnet-implementer` configured `model: claude-sonnet-5-5` for every P02 spawn; resolution not observable, no substitution notice; self-reports unverified. `haiku-docs` still a proposal |

## Carried forward from P02 (report §8)
- FR-32 (default-ignorable code points; P06, P19 at the latest), FR-33 (refuse euid 0 on a foreign data dir; P03) — plan §5.2.
- Semantic rules: 16 partial + 9 producer rules finish in later phases (each names its phase in `persistence/src/semantic/rules.rs`).
- Fault injection via `FsOps` only; durability tested on this host and filesystem only.
- Recorded lows: L-T11-1..4, L-T12-1..3, L-T13-1..4, L-T14-1..4; `report.max` selected-ref coherence (P07).
- Info: `dev/p02_registry.json` `lock_sha256` field is stale (`b34cb529…`); the runner does not read it.

## Open owner decisions
| ID | Item | Recommended |
|---|---|---|
| P03 start | Decisions recorded; no task started | Say "start P03" (T01 request, lead only) |
| Dev caps | T09 resource caps (timeout ≤ 86 400 s, output ≤ 64 MiB, ≤ 64 scanner processes, ≤ 1 024 active repositories) are development defaults pending FR-11 | Confirm or replace |
| cargo-audit | C-A2–C-E (native C build exposure `aws-lc-sys`, `ring`; install; advisory-DB fetch) | Decide before a phase that needs an advisory scan |
| Questions | Q-01 cadence, Q-02 AI narration deferral, Q-04 OpenRouter model ID, Q-05 scope exchanges (plan §1) | Owner |
| Other | O-1–O-6 (P00-T10, P13) | At P13 planning |

## Blockers
B-01 (installs beyond P00 tools), B-04 (worker models unverified), B-08 (schedule fit; Q-01/Q-05) open; B-02, B-03, B-05–B-07, B-09 resolved or scoped (plan §8).

## Next permitted action
None without the owner. Next on owner go: P03-T01 (request, lead only). Not approved: starting P03 tasks, cargo-audit C-A2+, network, new dependencies.

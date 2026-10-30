# Status

Updated: 2026-10-31 by Opus directly (no Haiku documentation agent installed). **P03 wave 1 (T05–T07) done and integrated (B1 `b1ca4cb`); debug cycle P03-D01 closed (1 attempt); all checks PASS 1005/41. Stopped before wave 2 (T08, T09), which needs the owner's go.** Owner approved wave 1 on 2026-10-30 ("I approve wave 1, start T05 T06 T07 …") and granted installations ("solve the blockages b01 b04 b08, perrmission grnated for installations").

Full history up to P02 close, verbatim: `evidence/P02/continuity/status-at-P02-close.md` (sha256 `ca2fb867…`). P00/P01 detail lives there and in `evidence/P00/P00-report.md`, `evidence/P01/P01-report.md`.

| Item | State |
|---|---|
| Stage | **P03 wave 1 done (T05 ∥ T06 ∥ T07, B1 `b1ca4cb`) + D01; wave 2 (T08 W1 ∥ T09 W3) awaits the owner's go.** Packets `planning/P03-packets/{_wave1-common,T05-W1,T06-W2,T07-W3,D01-W1,D01-W2}.md`. B1 evidence `evidence/P03/B1/` (lead check, golden-vector recomputation 7/7, 4 mutation controls caught). D01 `evidence/debug/P03-D01/record.md` (M1 `OperationArg` Debug leak fixed; G4 conformance; `TZ` validated as IANA name, request §7 amended; duplicate declared names fail closed). Earlier: T01–T04 accepted 2026-10-30 (B0 `5e29908`). Estimate for the rest 9–14 h (low confidence) |
| P03 | Planned; **C1–C10 = 1a…10a** (owner: "update plan with the recommendations"): full scope, session-only credentials (no OS store/keyring), no new crates (`application → sha2` edge only, std-only clearing), one-invocation CLI key, stdin-only key input, ASCII-only hosts, once/run/workflow lifetimes, CA-05 only if needed and owner-approved, up to 3 workers + phase-branch commits/pushes, pre-authorised debug cycles (2 attempts). Branch `phase/P03-permissions-credentials` from `main` `01a3cbd` |
| P00 | Accepted 2026-10-24 with qualifications; merged `5888826`. Report `evidence/P00/P00-report.md` |
| P01 | Accepted 2026-10-26; merged `7324da6`. Report `evidence/P01/P01-report.md`, freeze `evidence/P01/P01-freeze.json` |
| P02 | **Accepted 2026-10-29** ("I accept p02, commit and push to the phase branch, merge to the main branch"); T15 `84fedd4` pushed to `phase/P02-store`; `--no-ff` merge `4747214` into `main` (tree = `84fedd4`); `main` pushed (owner: "commit and push main"). Report `evidence/P02/P02-report.md`, freeze `evidence/P02/P02-freeze.json` |
| Verification B1+D01 | `python3 -I -B dev/p03_check.py all --slot lead --out evidence/debug/P03-D01/check-lead` (cwd repo root): **OVERALL PASS** — fmt, clippy `-D warnings`, test 48 suites 1005/0/0, 0 ignored, 41 compile_fail, build, cli 48, static. B1 alone: 998/41 (`evidence/P03/B1/check-lead`). Floors 1005/41 (`dev/p03_baseline.json`). Reviewer + security-reviewer on B1: no wrong `Permitted`, no key/env leak; deferred items listed in the D01 record. `--final` still fails by design (T08–T10 stubs) |
| Verification on `main` | `python3 -I -B dev/p02_check.py all --slot lead --final --out evidence/P02/merge/check-main` (cwd repo root, `4747214`): **OVERALL PASS** — fmt, clippy `-D warnings`, test 41 suites 866/0/0, 0 ignored, 39 compile_fail, build, cli 48, static 28/28. Floors 866/39 (`dev/p02_baseline.json`) |
| Manual check | **Closed 2026-10-29: PASS.** `real_storage_failure_exits_4_when_permissions_are_enforced` run with uid 1000 in `unshare -Un --map-current-user` (network still isolated); the enforced branch is shown to have run (`eacces-read`/`eacces-write` temp dirs observed); root-mapped control skipped it as expected. `evidence/P02/manual-exit4/record.md`. The runner itself still skips this branch |
| Specs / contracts | Spec v0.3.5 (CA-01–CA-04); store 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1; schema `schemas/workbench-store-v1.4.schema.json` sha256 `909e2a55…`. P03-T03: no CA-05; domain gained `FixedText`, `Destination::to_redacted`, `RawInput` Drop/Debug hardening (no serialised change) |
| Dependencies | `serde =1.0.229`, `serde_json =1.0.154`, `jsonschema =0.60.1`, `sha2 =0.11.1`, `libc =0.2.191`; `Cargo.lock` sha256 **`d4b2efad…`** (unchanged in wave 1). **cargo-audit 0.22.2 installed** (C-A2–C-E, 2026-10-30/31, owner install grant): workbench lock 94 crates **0 advisories** (DB `2295466c`, 2026-10-30; offline, yanked check not run). C-A2 stop rules met and disclosed: `aws-lc-sys` bundled C was built; cargo-audit's own graph has 13 vulnerability + 4 unsound advisories in its network stack (tool only run `--no-fetch` in `unshare -rn`). `evidence/P03/cargo-audit-C-E/record.md` |
| Git | `phase/P03-permissions-credentials`: B0 `5e29908`, B1 `b1ca4cb`, then the D01 commit containing this file; pushed. `main` = `origin/main` = `01a3cbd`. Worktrees `.claude/worktrees/p03-t05-w1` (`p03/d01-w1`) and `p03-t06-w2` (`p03/d01-w2`) at `b1ca4cb` hold D01 worker copies already integrated; removable. Untracked `.commandcode/` (local, never commit) |
| Disk / tools | Clean-up 2026-10-30 (owner request): deleted target dirs `p02-lead`, `p03-w1`, `p03-lead` (none left; the next runner call rebuilds offline from `_cargo-home`, about 10 min), scratch `_p03/` and session scratch (B0 evidence already committed, verified identical); `git worktree prune`, `git gc`, `git fsck` clean apart from 3 harmless dangling blobs. `pwt-dev-tools` 819 MB; disk 68 %. Worktree archive `~/.local/share/pwt-dev-tools/_archive/p02-worktrees-2026-10-28.tar.gz` kept |
| Agents | `sonnet-implementer` `model: claude-sonnet-5-5`: wave 1 = 3 spawns (T05/T06/T07), D01 = 2 spawns; `reviewer` + `security-reviewer` 1 each on B1. **B-04 evidence:** the API-reported model in every assistant message of all 40 earlier `sonnet-implementer` transcripts (and all reviewer/security-reviewer/Explore transcripts) is `claude-sonnet-5-5` (2 175 messages; 2 client-side `<synthetic>` error entries). Self-reports remain unverified as evidence of work. `haiku-docs` still a proposal |

## Carried forward from P02 (report §8)
- FR-32 (default-ignorable code points; P06, P19 at the latest), FR-33 (refuse euid 0 on a foreign data dir; P03) — plan §5.2.
- Semantic rules: 16 partial + 9 producer rules finish in later phases (each names its phase in `persistence/src/semantic/rules.rs`).
- Fault injection via `FsOps` only; durability tested on this host and filesystem only.
- Recorded lows: L-T11-1..4, L-T12-1..3, L-T13-1..4, L-T14-1..4; `report.max` selected-ref coherence (P07).
- Info: `dev/p02_registry.json` `lock_sha256` field is stale (`b34cb529…`); the runner does not read it.

## Open owner decisions
| ID | Item | Recommended |
|---|---|---|
| P03 wave 2 | Wave 1 + D01 done; T08 needs T05 (+ P02 writer), T09 independent | Approve wave 2: T08 (W1) ∥ T09 (W3); T08 packet carries the D01 requirements (workflow-scope grants with `run_id = null`; show `canonical()` destinations; only `approvals.rs` acts on `Permitted`) |
| R7/R8 semantics | A denied identical request is refused permanently (no new pending); a pending identical record shadows a matching grant (spec §6 as written) | Keep for P03 (fail closed); revisit with the TUI approval flow |
| cargo-audit tool | Its own graph carries 13 + 4 advisories (network stack, unreachable offline) | Keep, offline-only use |
| Dev caps | T09 resource caps (timeout ≤ 86 400 s, output ≤ 64 MiB, ≤ 64 scanner processes, ≤ 1 024 active repositories) are development defaults pending FR-11 | Confirm or replace |
| Questions | Q-01 cadence, Q-02 AI narration deferral, Q-04 OpenRouter model ID, Q-05 scope exchanges (plan §1) | Owner |
| Other | O-1–O-6 (P00-T10, P13) | At P13 planning |

## Blockers
B-01 **resolved for cargo-audit** (installed 2026-10-31); still open only for conditional tools that P08/P09/P13 have not yet defined (no concrete request exists; the PDF renderer is crates, no install). **B-04 resolved for Sonnet workers** (API-reported model evidence above); open only for `haiku-docs` (not installed). **B-08 open:** needs the owner's answers to Q-01 (cadence) and Q-05 (scope exchanges); owner asked to clarify. B-02, B-03, B-05–B-07, B-09 resolved or scoped (plan §8).

## Next permitted action
None without the owner. Next on owner go: P03 wave 2 (packets T08-W1, T09-W3 from the D01 commit). Not approved: wave 2 and later P03 tasks, network, new dependencies.

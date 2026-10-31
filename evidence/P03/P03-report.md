# P03 report — permission evaluator, child-environment sanitiser, session credentials and CLI

Date: 2026-11-01.
- **Phase branch:** `phase/P03-permissions-credentials`, created from `main` `01a3cbd`. Code baseline **D02 `091224b`**; this report and the freeze are added on top (docs/evidence only).
- **Prepared by:** Opus (lead), under the owner's T12 approval ("I approve T12, write the P03 report"). No Haiku documentation agent is installed.
- **Status:** **complete, awaiting owner acceptance.** Not merged to `main`.

This report states what was done and what was verified. It is not a claim that the code is secure or production-ready.

## 1. Scope delivered (plan §5C.1, decisions C1–C10 = recommended)
- **Permission requests and fingerprint (`application::permission`):** typed `PermissionRequest`; canonical fingerprint encoding v1 (SHA-256 over a documented byte encoding of the operation; context matched as separate dimensions, T01-D1); ASCII-only destination normalisation (C6: https/ssh, default ports explicit, trailing dot, userinfo/query/IDN/punycode/IP-shortcut rejection).
- **Evaluator (pure):** R1–R8 request checks and G1–G14 grant dimensions with tie-break and `Mismatch` reporting (request §6). Denied, revoked, expired, consumed and pending records never authorise; stale workflow revision refused; raw secrets refused under every grant; no wildcard grant.
- **Approval lifecycle (`application::approvals`):** request → pending; resolve (once / workflow scope with `run_id = null` / deny), revoke, `expire_due`, `expire_run`; a `once` grant is written `consumed` before `Authorised` is returned; deferred capabilities refused before any write; events `ApprovalRequested/Resolved/Revoked` through the P02 emitter; CLI entry points.
- **Child-environment sanitiser (`application::child_env`):** empty base, per-tool allowlist profiles (`git`, `yamllint`, `gitleaks`, `checkov`, `actionlint`, `kubeconform`, `internal`), deny patterns and a value check against the session key, app-owned HOME/XDG/TMP, masked preview, `TZ` only for `internal` and only if a valid IANA name.
- **Session credentials (`application::credential`, KEY-01, C2/C4/C5):** session-only store, `read_key` (stdin pipe only, TTY refused before reading, 16–4 096 printable ASCII), status/replace/delete, best-effort clearing (not secure erase), no key in `Debug`, events, argv, environment, logs or files.
- **FR-33 (`persistence`):** effective uid read from `/proc/self/status`; as euid 0, a data directory not owned by root is refused (`RootForeignDataDir`, exit 5); unreadable euid fails closed.
- **CLI (D-17):** `approvals list|approve|deny|revoke`, `credential set --session|status|delete`, `dev env-preview <tool>`, `dev permission-check` (decides only through `ApprovalService::authorise`); 16 commands, 64 feature-index rows with staleness test.
- **Dev tooling:** `dev/p03_check.py` (P02 runner + P03 floors, static rules incl. rule 12 `Evaluation::Permitted`/`permission::evaluate` only in `approvals.rs` and rule 13 `RequestOutcome::Permitted` only in `approvals.rs`; 111 self-test cases), floors `dev/p03_baseline.json` = **1138 / 41**.
- **Dependencies:** no new crates (C3). Only change: `application → sha2` edge (lock edges only; registry set unchanged).

**Not in P03** (per plan): OS credential-store mode and FR-18–FR-23 (C2, not adopted), any HTTP/provider call (P16), Git operations (P04), process spawning with the sanitised env (P06), the real redactor (P06), TUI approval dialog (P15), scheduling (P14). CA-05 (automatic LLM analysis) is a separate, unapplied design.

## 2. Tasks
| Task | Result | Record |
|---|---|---|
| T01 request, T02 dependency/lock check, T03 contract check (no CA-05 for P03) | Approved; commit `26e1d19` | `planning/P03-T01-request.md`, `evidence/P03/T02/`, `evidence/P03/T03/` |
| T04 scaffold + runner (T04b worker), B0 `5e29908` | PASS 866/39; T01–T04 accepted 2026-10-30 | `evidence/P03/B0/record.md` |
| Wave 1: T05 evaluator (W1) ∥ T06 child env (W2) ∥ T07 credentials (W3), B1 `b1ca4cb` | PASS 998/41; golden vectors recomputed by the lead 7/7; 4 mutation controls caught | `evidence/P03/B1/` |
| Debug cycle P03-D01, `b9fdc0e` | PASS 1005/41 (1 attempt) | `evidence/debug/P03-D01/record.md` |
| cargo-audit install + audit (owner install grant) | 0 advisories | `evidence/P03/cargo-audit-C-E/record.md` |
| Wave 2: T08 approvals (W1) ∥ T09 FR-33 (W3), B2 `4d6c70f` | PASS 1053/41; NC1/NC2 caught; **accepted 2026-10-31** | `evidence/P03/B2/record.md` |
| Wave 3: T10 CLI (W2) ∥ T11 hostile fixtures (W1) + lead static rule 12, B3 `f63c39a` | PASS `--final` 1126/41; binary key sweep clean; reviews 0 high | `evidence/P03/B3/record.md` |
| Debug cycle P03-D02 + lead rule 13, `091224b` | PASS `--final` 1138/41 (1 attempt) | `evidence/debug/P03-D02/record.md` |
| T12 final check, cargo-audit re-run, FR-33 process check, freeze, report | PASS (§3) | this report, `evidence/P03/P03-freeze.json`, `evidence/P03/T12/` |

## 3. Final verification (lead, 2026-11-01)
- **Command:** `python3 -I -B dev/p03_check.py all --final --slot lead --out evidence/P03/T12/final` (cwd repo root).
- **Tree:** HEAD `091224b`; dirty only `?? .commandcode/` (local), `?? evidence/P03/T12/` (this step), `?? workbench/target/` (unexplained build cache, see §8).
- **Environment:** rustc/cargo 1.97.1 called directly, `env -i`, isolated `CARGO_HOME`, every Cargo command in `unshare -rn` (no network), `--frozen`; lock and manifests hash-checked unchanged around every step.

| Check | Result |
|---|---|
| fmt | exit 0 |
| clippy `--workspace --all-targets -D warnings` | exit 0 |
| test `--workspace --no-fail-fast` | exit 0. **54 suites, 1138 passed, 0 failed, 0 ignored, 41 compile_fail** (floors 1138/41) |
| build | exit 0 |
| cli (`cargo test -p cli --tests`) | exit 0, 5 suites, **101 passed** |
| static `--final` | PASS: no `todo!`/`unimplemented!`, allow-list equal to the approved list, env reads only in `cli/src/main.rs`, no `Command::new`, `expose`/`CredentialSecret::new` only in allowed files, `Authorisation{}`/`Grant{}` construction rules, rules 12–13, registry set and lock SHA |
| cargo-audit (`--no-fetch`, `unshare -rn`, DB `2295466c`) | exit 0: **94 crates, 0 vulnerabilities, 0 warnings** (`evidence/P03/T12/audit.json`); yanked check not run offline |
| FR-33 real-process check (`evidence/P03/T12/fr33-process/run.txt`) | A: euid 0 in a user namespace, data dir owned by an unmapped uid → exit 5, "refusing to use a data directory not owned by root while running as root". B (control): own 0700 dir mapped to root → accepted, exit 0. C (control): euid 1000, same foreign dir → exit 4 storage error, no FR-33 refusal |
| Seeded-key sweep, real binary (B3, re-run after D02 on the rebuilt binary) | 24 processes, 8 data files: key and an 8-byte fragment in no output, argv or file (`evidence/debug/P03-D02/lead_sweep.out`) |

Negative controls (the checks can fail): B1 golden-vector and 4 mutation controls; B2 NC1 (no once-grant consumption → 3 FAILED) and NC2 (no FR-33 owner check → 2 FAILED); D02 renderer control (→ 1 FAILED); runner self-tests 111/111 including planted violations for every static rule. Worker-run mutations (W1 T11: 16 production mutations, D02 B1/B2; W2 D02 decode) are self-reported.

Test growth: 866 (P02) → 998 (B1) → 1005 (D01) → 1053 (B2) → 1126 (B3) → 1138 (D02); compile_fail 39 → 41.

## 4. Acceptance criteria (plan §5C.4)
| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | fmt/clippy/test/build pass via `all --final`; P02 floors not lowered; new floors committed | **Met** | §3; `dev/p03_baseline.json` 1138/41 |
| 2 | Scope-match table: every dimension positive + negative; inactive grants never authorise; changes need new approval; raw secrets refused; no wildcard | **Met** | `application/tests/p03_evaluate.rs` (T05), `p03_hostile.rs` groups 1–3 |
| 3 | Fingerprint encoding documented, golden vectors pinned, equal ops equal, changes differ, fingerprint never authorisation alone | **Met** | `p03_fingerprint.rs`; lead recomputation 7/7 (`evidence/P03/B1/golden_lead.txt`) |
| 4 | Once-grant consumed exactly once incl. fault injection at every write step; pending never a grant; schema-valid events; only explicit commands approve | **Met** for the tested fault matrix (no exactly-once claim beyond it) | `p03_approvals.rs` fault matrix, B2 NC1, `p03_hostile.rs` race test |
| 5 | Child env from empty base; seeded keys absent from every profile; preview masks | **Met** | `p03_child_env.rs`, CLI `env_preview_*`, sweeps |
| 6 | Session-only credentials; no plaintext anywhere; statuses; replace/delete; no key in events/Debug; OS-store absent and FR-18–FR-24 status recorded | **Met** (OS store not adopted, C2; FR-24 re-screen not applicable) | `p03_credential.rs`, CLI credential tests, lead binary sweep |
| 7 | FR-33 refused as euid 0 with typed error and exit code; root-owned case works | **Met** | `persistence/tests/p03_fr33.rs`, `application/tests/p03_fr33.rs`, real-process check §3 |
| 8 | CLI definition of done for every P03 feature; headless; no real app-data dir or real env secret | **Met** | `cli/tests/p03_cli.rs`, 64 feature rows + staleness test |
| 9 | reviewer + security-reviewer: no unresolved high; mediums fixed or accepted by the owner | **Met** — no high in either review (B1, B3); mediums fixed in D01/D02; remaining items listed in §8 for owner decision | `evidence/debug/P03-D01/`, `P03-D02/` |
| 10 | No new crates; lock delta edges only; cargo-audit status stated | **Met** | `evidence/P03/T02/`, lock sha `d4b2efad…`, §3 audit |

## 5. Debug cycles
| Cycle | Defects | Outcome |
|---|---|---|
| P03-D01 | `OperationArg` derived `Debug` printed argument values (lead B0 code); G4 accepted a workflow-lifetime grant with null workflow; inherited `TZ` unvalidated; duplicate declared env names | Fixed, attempt 1; 1005/41 |
| P03-D02 | Line forgery in human `approvals list` (LF/TAB kept by the frozen escape set); inference scope not shown; missing CLI tests (run/model paths, JSON shapes, closed stdout); overstated FR-33 row; `approvals list` error class differed from the writer path; `ToolDirs` Debug printed paths; `RequestOutcome::Permitted` hazard (rule 13) | Fixed, attempt 1; 1138/41 |

## 6. Agents (CLAUDE.md model-recording rule)
`sonnet-implementer`, configured `model: claude-sonnet-5-5`: T04b ×1, wave 1 ×3, D01 ×2, wave 2 ×2, wave 3 ×2 (the two wave-3 workers were resumed for D02 rather than respawned). Read-only `reviewer` and `security-reviewer`: once each on B1 and on B1–B3. API-reported model in the wave-3/D02 worker transcripts still on disk: every assistant message `claude-sonnet-5-5` (198 and 244 messages) plus one client-side `<synthetic>` entry each (usage-limit error, 2026-10-31); earlier transcripts were checked at B-04 (status.md). Worker reports are self-reports; evidence is the lead's runs above. Workers coordinated through append-only logs (copies in `evidence/P03/B2/`, `B3/`, `evidence/debug/P03-D02/`).

## 7. Owner decisions recorded during P03
C1–C10 = recommended (2026-10-29); T01–T04 approved and accepted (2026-10-30); wave 1 approved, install grant for cargo-audit (2026-10-30); wave 2 approved, B2 accepted (2026-10-31); wave 3 approved (2026-10-31); T12 approved (2026-11-01). Lead decisions inside those: request §5.4 amendment for D01 (`expire_run`, workflow-scope `run_id = null`, deferred capabilities refused, canonical display), help lists all 16 commands, no FR-33 CLI seam, constraint bounds 16 × 512 B, rules 12–13.

## 8. Known limitations and carry-forward
**Owner decisions requested (none blocks acceptance):**
- **R7/R8 ignore inference scope.** A pending/denied `model_transmission` request for one model blocks a valid grant for another model of the same operation (fails closed; pinned by `p03_hostile.rs` and a CLI test). A one-line fix exists but changes the frozen §6 table. Recommended: decide before P16.
- **Approvals store has no integrity binding.** A same-user process can hand-edit `approvals.json` or run `approvals approve`. Recommended: record as a **P06 entry gate** — scanner children must not reach the data directory or the CLI (sandbox or separate uid).
- **§5.7 reading.** After a workflow revision change a request at the old revision is refused (exit 5, `stale_workflow_revision`); one at the new revision gets 6. Owner to confirm this reading of "6 after a revision change".
- O-B2-1 (pending record carries validity in `expires_at`), O-B2-2 (revoke overwrites actor), O-B2-3 (lost `ApprovalRequested` on log failure) — still open.

**Recorded limitations:** no cap on pending records; post-write emit failure reports a generic message (frozen `ApprovalError`); `--scope once` on a stale request succeeds without a "will not match" note (UX, P15); std stdin buffer may retain key bytes (best-effort clearing only); approvals differing only in operation look alike in `list` (TUI); `dev permission-check` constraints stored sanitised and bounded (16 × 512 B, no control characters); ssh destinations stored as `host:port` (path only in the fingerprint); private/loopback IPv4 destinations accepted (P04/P16); a `once` grant also serves a run-lifetime request of the same operation (consumed on use); `credential` commands emit no `CredentialStatusChanged` (no data dir/emitter in the CLI); cargo-audit DB not refreshed since 2026-10-30, yanked check offline-skipped; cargo-audit's own network stack carries advisories (tool used offline only).

**Carried from P02:** FR-32 (P06/P19), semantic producer rules for later phases (D11-12/13/16/17 now name P04/P07/P14/P16/P17 remainders), FsOps-only fault injection, P02 low findings.

**Housekeeping:** an untracked 3.9 GB `workbench/target/` build cache appeared in the main tree on 2026-10-31 22:32 from an unidentified plain Cargo run (the runner never builds there); left in place for the owner. Worktrees `p03-t08-w1`, `p03-t09-w3`, `p03-t10-w2`, `p03-t11-w1` still exist; their content is committed. Disk free about 7.6 GB.

## 9. Recommended next steps (owner decisions)
1. Accept P03 (with §8), then authorise the `--no-ff` merge into `main` and its push. (This report and the freeze are committed and pushed on the phase branch under C9.)
2. Decide the R7/R8 scope question, the P06 entry gate and the §5.7 reading.
3. Authorise clean-up: the four worktrees and their target dirs, and `workbench/target/`.
4. Then P04 planning (or CA-05 contract task scheduling per D-18).

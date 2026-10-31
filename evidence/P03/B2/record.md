# P03 B2 — wave 2 (T08 approval lifecycle, T09 FR-33) integrated

Owner approval 2026-10-31: "I approve wave 2, start T08 and T09 …". Base `1537e36` (packets `planning/P03-packets/{_wave2-common,T08-W1,T09-W3}.md`; request §5.4 amended for D01). Workers: `sonnet-implementer` (`model: claude-sonnet-5-5`), W1 = T08 in `.claude/worktrees/p03-t08-w1` (branch `p03/t08-w1`), W3 = T09 in `.claude/worktrees/p03-t09-w3` (branch `p03/t09-w3`). Worker reports are self-reports; the evidence is below.

## Coordination
Append-only log outside Git (owner asked the workers to communicate); copy: `coordination-log.md`. W3 posted the new `DataDir::new` refusal rule (INFO 15:30Z) before W1 integrated anything; W1 confirmed plain tempdirs, no chown. No REQUEST/BLOCKED; no file outside ownership touched (lead checked `git status` of both worktrees and the main tree).

## Integration
Lead copied the owned files into the main checkout (fresh mtimes):
- W1: `application/src/approvals.rs`, `persistence/src/semantic/rules.rs` (D11-12/13/16/17 `rest_in` only; all stay Partial, counts unchanged), new `application/tests/p03_approvals.rs` (34 tests).
- W3: `persistence/src/fs.rs` (`process_effective_uid`, `parse_status_euid`), `persistence/src/paths.rs` (FR-33 in `DataDir::new`), new `persistence/tests/p03_fr33.rs` (11), `application/tests/p03_fr33.rs` (3).

## Lead verification
`python3 -I -B dev/p03_check.py all --slot lead --out evidence/P03/B2/check-lead` (cwd repo root, HEAD `1537e36` + the files above): **OVERALL PASS** — fmt, clippy `-D warnings`, test 51 suites 1053/0/0, 0 ignored, 41 compile_fail, build, cli 48, static. Arithmetic 1005 + 34 + 14 = 1053. Not `--final` (T10 stub `dev_permission.rs` still `todo!`).

Negative controls (lead, `negative-controls/`; source restored and `diff`-identical afterwards):
- NC1 `authorise` without the once-grant consumption write → 3 FAILED (`a_once_grant_is_consumed_exactly_once`, `without_a_fault_the_consumption_is_durable_and_final`, `once_grant_consumption_survives_a_fault_at_every_write_step`).
- NC2 FR-33 owner comparison removed from `DataDir::new` → 2 FAILED in `p03_fr33`.

## Lead code review (no blocking defect found)
Consumption is written before `Authorised` is returned; deferred capabilities refused before any read/write; workflow-scope grants stored with `run_id = null`; status changes only in `resolve`/`revoke`; `expire_run` per amendment. FR-33: euid read failure → `PathRejected`; owner taken from the canonical directory; only when euid = 0.

## Items for the owner (design choices by W1, inside the frozen interface)
- O-B2-1 Pending records carry the request's validity as `expires_at = requested_at + validity`; on approval `expires_at = decided_at + validity`. Readers must not treat a pending record's `expires_at` as a deadline. Alternative: a store field in a later contract change.
- O-B2-2 `revoke` overwrites `actor` with the revoker; the approver's identity is lost (keeping `decided_at`).
- O-B2-3 A log-write failure after a pending request is saved loses its `ApprovalRequested` event permanently (spec §5.4 as written; the request is reused, not re-emitted).
Recorded limitations: `SyncDir` failure during consumption returns `Err` with the grant already consumed on disk (lost, never reused); ssh destinations collapse to `host:port` in the store form (D01 L1; fingerprint still distinguishes paths); deferred-capability and record-validity refusals report `InvalidRequest`.

## Still open for T11
Static rule "only `approvals.rs` acts on `Permitted`" (D01 H1); reviewer + security-reviewer over B1+B2; hostile fixtures and seeded-key sweep.

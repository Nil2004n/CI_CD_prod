# P02-T11 — envelope and emission: integration record (uncommitted)

Date: 2026-10-28. Lead: Opus. Authority: owner 2026-10-28 "commit B2 locally, no push, and approve t11 … branch sessions do the work, main session for test, orchestration and debug cycle initiation". Only T11 is approved; T12/T13 are not.

Base: B2 `2448abf` (local commit, not pushed; integrated PASS 735/0/0, 32 compile_fail, `evidence/P02/recheck-2026-10-28/`).

## 1. Worker

| Item | Value |
|---|---|
| Packet | `planning/P02-packets/T11-W1.md` (incl. Amendment 1) |
| Agent | `sonnet-implementer`, configured `model: claude-sonnet-5-5`; actual resolution not observable, no substitution notice; self-report unverified, lead re-ran everything |
| Worktree | `.claude/worktrees/p02-t11-w1`, branch `p02/t11-w1` at `2448abf`, no commits; increment unstaged |
| Owned files | `workbench/crates/application/src/emit.rs` (+434/−1), `workbench/crates/application/tests/emit.rs` (new, 1285 lines, 30 tests) |

**Spawn 1 — BLOCKED, no change (lead error).** Spawned with harness `isolation: worktree`, which created a separate harness worktree from the wrong commit and confined writes to it; the lead-prepared worktree was unreachable. The worker stopped without working around the guard and left an uncompiled draft (scratchpad, not evidence). The harness removed its unchanged worktree. Fix (Amendment 1): re-spawn without harness isolation, confined by the packet to the lead-prepared worktree; the lead answered its three questions (non-persisted set, `store_message`, list-valued IDs). **Spawn 2 — PASS** per its report (765/0/0, 32; four worker mutation runs).

## 2. Lead verification (main tree = `2448abf` + the two files copied byte-identical from the worktree)

sha256: `emit.rs` `6888e458…c4356c`, `tests/emit.rs` `cb82ff3a…c33a6772` (identical to the worktree, `cmp`).

| Run | Result | Evidence |
|---|---|---|
| `dev/p02_check.py all --slot lead --min-passed 765 --min-compile-fail 32` | **OVERALL PASS** — fmt, clippy, build, static, cli PASS; test 37 suites, **765 passed / 0 failed / 0 ignored, 32 compile_fail** | `integrated-check/`, `integrated-check.stdout` |
| floor control `test --min-passed 766` | OVERALL FAIL on the floor only (765/0/0) | `control-766/`, `control-766.stdout` |
| lead negative control: `ChatFailed` mapped to a persisted record | test FAIL as intended — 4 tests: `persisted_set_is_exactly_the_documented_set`, `non_persisted_events_are_those_that_are_chat_not_must_deliver_or_reserved`, `every_persisted_event_validates_through_the_real_schema_validator`, `each_persisted_event_validates_on_its_own` (the real validator rejects a chat event in the log, D11-24). File restored byte-identical (`cmp` vs worktree), fresh mtime | `nc-lead-chatfailed-persisted/`, `.stdout` |
| re-run after the control, floors 765/32 | **OVERALL PASS**, 765/0/0, 32 | `integrated-check-post-nc/`, `.stdout` |

Arithmetic: 765 = 735 (B2) + 30 (`application/tests/emit.rs`; the 31st `#[test]` grep hit is not a test function). Floors raised in `dev/p02_baseline.json` to **765/32** (uncommitted).

## 3. Lead code review (inline; formal reviewer/security-reviewer pass is in T14)

Checked against request §5.8, T01-D9/D10 and Amendment 1:
- Level table: exhaustive `match` over `AppEvent` and `RunState`, no `_` arm; matches §5.8.
- Validation precedes `next_bytes()`, so rejected events consume no ID; reserved check via `status()`; transition via `can_transition`.
- `UrandomIds`: `std::fs::File::open("/dev/urandom")` + `read_exact`; errors carry `ErrorKind` only; no fallback.
- `to_store_event`: `None` for reserved, non-must-deliver (`delivery()`), and the `Chat*` arms of an exhaustive `match`; `message_redacted = store_message()`; `code` only for `OperationFailed`.
- Errors carry closed enums / `ErrorKind` only; no payload text.

Observations (low, no change, no debug cycle):
- L-T11-1: `to_store_event` uses `?` on `EventType::from_wire` and `EventCode::parse`, so a future mapping failure would be indistinguishable from "not persisted". Currently pinned by `persisted_set_is_exactly_the_documented_set` (exact 25-event set) — a silent drop would fail that test.
- L-T11-2: `validate` uses a `_ => Ok(())` arm (packet required exhaustiveness only for the level table and the store mapping); a new event with an emission-time invariant would not be forced into it.
- L-T11-3 (worker-disclosed): `UrandomIds` open-failure and short-read paths are not exercised by tests (fixed device path, no injection seam by spec); covered by review and the error Display/Debug test only.
- L-T11-4 (worker-disclosed): `to_store_event` carries `level` as given (public fields); D11-25 catches failure events below `error` at validation.

Deviation (worker-reported, name only): packet text said `delivery_class()`; the domain method is `AppEvent::delivery()`.

## 4. Not done

T11 not committed (B3 commit only after wave 3, owner decision). Reviewer/security-reviewer pass deferred to T14 per plan. T12, T13 not approved. No network, no dependency change, no push.

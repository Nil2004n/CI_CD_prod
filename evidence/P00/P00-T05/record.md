# P00-T05 — Linux process-supervision spike

Date: 2026-10-24. Authority: owner approval of P00-T05 (bounded spike: implementation, offline build, fixture execution, verification, evidence, scoped commit), 2026-10-24. Executed by Opus. Branch `phase/P00-decisions-spikes`, base `646b112` (+ uncommitted spike source/fixtures during the run).

**Result: formal run 15 pass / 0 fail (harness exit 0), confirmed by inspecting the per-case evidence in `results.json`.** Spike code only; no application code, no other P00 task.

## Inputs
- Source: `spikes/rust/t05-process/src/main.rs` (std + `libc =0.2.191` only; no manifest change). Fixtures: `spikes/rust/t05-process/fixtures/*.sh` (mode 0644, run via `/bin/sh`; canary generated at runtime, never stored).
- Build: `unshare -rn env -i PATH=<toolchain bin>:/usr/bin:/bin HOME=… CARGO_HOME=~/.local/share/pwt-dev-tools/_cargo-home CARGO_TARGET_DIR=~/.local/share/pwt-dev-tools/_cargo-target RUSTC=<toolchain rustc> cargo build --frozen` → exit 0 (`build-1.log`). `Cargo.lock` unchanged (`lock-before-build.sha256` = `lock-after-build.sha256` = `68050af5…7e63`).
- Artifact (rebuilt; differs from the D3-1 placeholder as expected): `~/.local/share/pwt-dev-tools/_cargo-target/debug/t05-process`, SHA-256 `e906d5227dabb457100906e35e803867712e515af5b61048224b93137e1a6c9f` (`artifact.sha256`).

## Execution environment and watchdog
Command: `timeout -s KILL 300 unshare -rn --pid --fork --mount-proc --kill-child env -i PATH=/usr/bin:/bin LANG=C <artifact> <fixtures-dir> evidence/P00/P00-T05/results.json` → exit 0 (`run-1.log`).

- New user + network + PID + mount namespaces (util-linux 2.42.1); no sudo, no host change.
- Verified beforehand (`env-check.log`): inner shell is PID 1; only its own PIDs visible; only `lo` (DOWN), 0 routes; `curl https://1.1.1.1/` exit 7. Watchdog test: `timeout -s KILL 2` around a namespace with two `sleep` processes → exit 137 and 0 leftover processes on the host (read-only `pgrep` count; nothing signalled by name).
- The harness refuses to run unless it is PID 1. As namespace init it is the reaper for orphaned fixture descendants; if it dies, the kernel kills the whole namespace. `--kill-child` + outer `timeout` give an independent bound.
- Orchestrator shell outside: pid/pgid/sid 131501 — not addressable from inside the PID namespace. Inside, the harness reports pgid 0 and sid 0 because its group/session leader is outside the namespace.
- After the run, host check: 0 leftover `sleep 1000`, 0 `yes PWT-FLOOD-LINE` (read-only count).
- **No filesystem sandbox**: the harness and fixtures ran with the user's file permissions; a stripped environment does not make other files inaccessible.

## Signalling safety
Each child is spawned with `process_group(0)` (new group, pgid = pid). Before any `kill(-pgid, sig)` the target must: be > 1, equal the recorded spawn PID, differ from the harness pid/pgid, and still have members in `/proc`. Never used: `kill(0, …)`, `kill(-1, …)`, name matching. Reaping uses `waitpid(pid, WNOHANG)` for the direct child and `waitpid(-pgid, WNOHANG)` for group members reparented to the harness. Every case's `other_ns_processes_after` is empty, i.e. nothing at all remained in the namespace after each case.

## Cases (limits: cleanup wait 3000 ms for all; output limit 1 MiB per stream for all)

| Case | Expected | Timeout / grace / cancel | PID / PGID | Observed | Status |
|---|---|---|---|---|---|
| FX-PROC-EXIT-CODES/0–3 | normal exit with code N, no signal | 5000 / 1000 / – | 2/2, 5/5, 8/8, 11/11 | exit 0, 1, 2, 3; no signals sent; reaped; tree exited | pass ×4 |
| FX-PROC-EXIT-CODES/signal | self SIGUSR1 → signal termination | 5000 / 1000 / – | 14/14 | exit none, signal 10; no supervisor signal; reaped | pass |
| FX-PROC-SLEEP/timeout | timeout → SIGTERM → graceful | 2000 / 1000 / – | 17/17 | `kill(-17,SIGTERM)` rc 0; sh (17) and sleep (20) reaped with signal 15; 2016 ms | pass |
| FX-PROC-SLEEP/cancel | supervisor cancel → graceful | 30000 / 1000 / 1000 | 21/21 | reason cancel_requested; sh (21) + sleep (25) signal 15; 1015 ms | pass |
| FX-PROC-TERM-IGNORE | SIGTERM ignored → SIGKILL after grace | 1500 / 1000 / – | 26/26 | SIGTERM rc 0, then SIGKILL rc 0; sh (26) + sleep (29) signal 9; method forced; 2516 ms | pass |
| FX-PROC-GRANDCHILD | parent exits 0; leftover grandchild detected, terminated, reaped | 5000 / 1000 / – | 30/30 | parent exit 0; leftover `pid=33 ppid=1 pgid=30` (reparented to harness); SIGTERM to group; 33 reaped signal 15 | pass |
| FX-PROC-FLOOD-OUT | stdout over 1 MiB → terminate | 10000 / 1000 / – | 34/34 | limit hit; SIGTERM; sh + yes reaped signal 15; 31 ms; **51,981,930 B read before death** | pass (see F-1) |
| FX-PROC-FLOOD-ERR | stderr over 1 MiB → terminate | 10000 / 1000 / – | 38/38 | as above; **78,083,460 B read**; 22 ms | pass (see F-1) |
| FX-PROC-DUAL-STREAM | 512 KiB on each stream, no deadlock | 10000 / 1000 / – | 42/42 | exit 0; stdout 524,288 B and stderr 524,288 B fully read; no signals | pass |
| FX-PROC-SECRET-STDERR | canary on stderr sanitised before persistence | 5000 / 1000 / – | 49/49 | raw stderr contained `PWTFAKE_` (in memory); persisted excerpt `api_token=[REDACTED-CANARY]`; exit 0 | pass |
| FX-PROC-SPAWN-FAIL/missing | spawn error, no process | – | none | `NotFound` (os error 2) | pass |
| FX-PROC-SPAWN-FAIL/not-executable | spawn error, no process | – | none | `PermissionDenied` (os error 13) on fixture with mode 0644 | pass |

Canary persistence check: `grep -rEo 'PWTFAKE_C10_[0-9a-f]{8,}' evidence/P00/P00-T05 spikes` → exit 1 (no raw canary in any durable file).

## Findings (not failures of the approved checks; no fix applied)
- **F-1 Output-limit overshoot.** Termination is triggered by a 10 ms polling loop while reader threads keep draining, so ~50–78× the 1 MiB limit passed through (not retained: excerpt 512 B; in-memory stderr capture ≤ 64 KiB). Proposed bounded fix for the product supervisor (P06): stop reading at the limit and wake the supervisor immediately (channel/condvar), then terminate; keep the limit as a retention *and* consumption bound.
- **F-2 Contract gap (CA-01).** Cleaning up leftover descendants after the direct child exits has no `TerminationRecord.reason` value (enum: none, timeout, cancel_requested, output_limit, supervisor_shutdown). The spike used `leftover_descendants`. Proposal: add a reason value (contract change → owner decision before P06).
- **F-3 Reporting artefact.** Spawn-failure cases show `reader_finished=false` because no readers existed; harmless but should be `null`/not-applicable in a product record.
- **F-4 Group/session identity inside the PID namespace** appears as 0 for the harness; separation from the orchestrator rests on the namespace boundary, documented above.

## Untested / limitations
- Descendants that change process group or session (`setsid`, double-fork daemonisation) were **not** exercised (outside approved scope). Process-group signalling does not contain them; here only the PID-namespace teardown would.
- ANSI output (FX-PROC-ANSI) and early-closed stdout (FX-PROC-CLOSE-EARLY) not exercised.
- Only `/bin/sh` (dash) fixtures, one Linux host, one run; no timing statistics. Success of `kill()` calls was not taken as proof: termination is evidenced by `waitpid` statuses and empty `/proc` group/namespace scans.
- The sanitiser matches the synthetic `PWTFAKE_` marker only; it demonstrates sanitise-before-persist, not secret-detection quality.
- Disclosed: during fixture authoring, `sh dual_stream.sh` was executed once directly (outside the namespaces) to check its byte count (head/tr on `/dev/zero`; no signalling or network use).

## Files
`record.md` (this), `results.json` (per-case sanitised evidence), `run-1.log`, `env-check.log`, `build-1.log`, `artifact.sha256`, `lock-before-build.sha256`, `lock-after-build.sha256`.

# P00-T04 — local Git-safety spike

Date: 2026-10-24. Authority: owner approval of P00-T04 (local Git-safety test), 2026-10-24. Executed by Opus. Branch `phase/P00-decisions-spikes`, base `2649849` (+ uncommitted CA-02 / T04 work).

**Result: formal verification FAILED — 32 result rows: 26 pass, 4 fail, 1 inconclusive, 1 observation. P00-T04 is not complete. No repair has been made; a bounded debug cycle is proposed below.**

## Setup
- Harness: `spikes/t04-git/run_t04.sh` (bash; SHA-256 in `harness.sha256`). Git 2.53.0 (`/usr/bin/git`). Only synthetic local repositories and synthetic canary helpers created by the harness in a scratch directory (`~/.local/share/pwt-dev-tools/_spikes/t04-run1`, `<SCR>` in results) — removed by the harness at the end (verified).
- Execution: `timeout -s KILL 300 unshare -rn --pid --fork --mount-proc --kill-child env -i PATH=/usr/bin:/bin LANG=C /bin/bash spikes/t04-git/run_t04.sh <SCR> evidence/P00/P00-T04/results.tsv` → exit 1 (`run-1.log`).
- Setup checks before execution (`env-check.log`): inner shell PID 1, only own PIDs visible, `lo` DOWN, 0 routes, `curl https://1.1.1.1/` exit 7; watchdog run exit 137 with 0 host leftovers; scratch absent before run.
- Every Git call: `env -i` (PATH, LANG, scratch `HOME`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`, synthetic author identity), `timeout 30`, stdout/stderr to bounded per-call logs. Global/system Git configuration and the real project remote were not read or edited. No real hooks or credential helpers were executed; the only executables run besides Git and coreutils were the harness's synthetic canary helpers (each only creates a marker file).
- Candidate hardened invocation: `-c core.hooksPath=/dev/null -c core.fsmonitor=false -c protocol.allow=never -c protocol.https.allow=always -c protocol.ssh.allow=always -c submodule.recurse=false`; clone = `clone --no-checkout --template=<empty dir>`, write `.git/info/attributes` = `* -filter`, then `checkout -f`; update = refuse in-progress state, refuse non-empty `status --porcelain --untracked-files=all`, `fetch origin`, `merge --ff-only refs/remotes/origin/main`.
- Test-only local transport exception: `-c protocol.file.allow=always`, added per command only for local fixture clone/fetch (`g_hard_file`). It is not part of the application transport policy (HTTPS/SSH only); G11 shows the hardened policy refuses `file://` and local paths without it.

## Results (`results.tsv`)

| Case | Positive control (same fixture, ordinary Git) | Hardened result | Status |
|---|---|---|---|
| G01 initial clone | — | cloned, HEAD = source HEAD | pass |
| G02 fast-forward update | — | updated to upstream OID | pass |
| G03a dirty tracked file | — | REFUSED_DIRTY, worktree/HEAD digest unchanged | pass |
| G03b untracked file in incoming path | — | REFUSED_DIRTY, unchanged | pass |
| G03c in-progress conflicting merge (MERGE_HEAD present) | — | REFUSED_INPROGRESS, unchanged | pass |
| G03d diverged history | — | REFUSED_DIVERGED (ff-only refused), unchanged | pass |
| G04 hooks via global `init.templateDir` | fired (1 marker) | not fired; hook not copied | pass |
| G05 global `core.hooksPath` | fired (1) | not fired | pass |
| G06 post-merge hook planted in managed checkout | **not fired — fixture inert** | not fired, updated | **fail (control) / inconclusive** |
| G07 attribute smudge filter (global driver) | fired (1) | **fired (1)**, content raw | **fail** |
| G07u same, on update | — | **fired (2)**, updated | **fail** |
| G08 LFS-like required process filter | fired (1) | **fired (1)**, clone rc 1, content missing | **fail** |
| G09 submodule recursion (global `submodule.recurse=true`) | materialised with `--recurse-submodules` | not recursed; `.git/modules` absent | pass |
| G09u update with recursion configured | — | updated, not recursed | pass |
| G10 `ext::` transport helper | fired (1) | not fired; `transport 'ext' not allowed` | pass |
| G11 / G11b `file://` and local path without exception | — | refused: `transport 'file' not allowed` | pass |
| G12 global `core.fsmonitor` | fired (2) | not fired | pass |
| G13 config injected via inherited `GIT_CONFIG_COUNT` | fired (2) | not fired (env stripped) | pass |
| G13b injection kept + `-c core.fsmonitor=false` | — | not fired (command-line `-c` took precedence here) | observation |
| G14 credential helper (`git credential fill`, local) | fired (1) | helper still configured under hardened options (preserved, as D-13 requires) | pass |
| CLEANUP | — | logs 2,809 B total (max 192 B); scratch removed; process list showed `tr` | pass (see note) |

## Diagnosis

Confirmed (from `run-1.log` errors and results):
- **D-1 (procedure defect, fail-open).** `clone --template=<empty dir>` creates no `.git/info/` (nor `.git/hooks/`). Writing `.git/info/attributes` therefore failed ("No such file or directory") for all 10 hardened clones, and `hard_clone` **continued to checkout** because the write's failure was not checked. The `* -filter` override was never in place, so G07/G07u/G08 filters ran. These failures do not show that the `info/attributes` approach is ineffective; it was never applied.
- **D-2 (fixture defect).** For the same reason `.git/hooks/post-merge` could not be planted in G06; the positive control was inert, so G06's hardened result proves nothing.

Hypotheses (not yet verified):
- With `.git/info/` created and the write checked, `* -filter` in `.git/info/attributes` (highest attribute precedence) will prevent smudge and process filters during checkout and update.
- The `tr` entry in the cleanup process list is the listing pipeline's own `tr` stage, not a leftover process.

## Proposed bounded debug cycle P00-T04-D01 (not started)
1. `hard_clone`: `mkdir -p "$dest/.git/info"`; abort (fail closed) if the attributes file cannot be written; then verify before checkout that `git check-attr filter -- <tracked paths>` reports `unset` for every path (abort otherwise).
2. G06 fixture: `mkdir -p .git/hooks` before planting; abort the case as inconclusive if the hook file is absent.
3. Cleanup check: snapshot `/proc` PIDs to a variable before filtering, so the listing pipeline cannot appear in its own result.
4. Retest: full harness re-run (all rows) under the same verified namespace setup; expected behaviour unchanged.

## Limitations (independent of the defects)
- Real HTTPS/SSH remotes, credential-helper invocation during network fetch, `core.sshCommand`/`GIT_SSH`, proxy commands, `includeIf`/`include` config, system-level config and `diff`/`textconv` drivers were not exercised.
- `submodule.recurse` was checked for clone and update only; other recursion entry points were not.
- One Git version, one host, local `file://` transport only.
- No filesystem sandbox: Git and helpers ran with the user's file permissions.

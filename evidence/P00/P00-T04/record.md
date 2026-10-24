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

## Debug cycle P00-T04-D01 (owner-approved 2026-10-24) — appended; run-1 evidence above unchanged

**Result: formal re-run PASSED — 37 rows: 36 pass, 0 fail, 0 inconclusive, 1 observation (G13b). Harness exit 0.** Evidence: `run-2.log`, `results-2.tsv`, `harness-2.sha256`, `env-check-2.log`, `tr-investigation.log`.

### Root cause (confirmed) and changes
- D-1: `clone --template=<empty dir>` creates neither `.git/info/` nor `.git/hooks/`; the run-1 procedure wrote the attributes override without creating the directory or checking the write, then checked out anyway (fail-open).
- Changes to `spikes/t04-git/run_t04.sh`:
  - `attr_setup`: `mkdir -p .git/info` with explicit success and directory check; write `* -filter`; read back and require exact content. Any failure sets a reason and aborts **before** checkout.
  - `attr_verify`: list every path of the **target commit** with `ls-tree -r -z --name-only <commit>` (NUL-separated, safe for spaces/newlines), then `check-attr -z --source=<commit> --stdin filter`; require exactly 3 fields per path, path order matching, attribute `filter`, value `unset` for every path. `unspecified`, `set`, any driver name, malformed or short output, or zero paths → abort. This checks the target tree, not the empty pre-checkout worktree.
  - `hard_clone`: clone `--no-checkout` → resolve target commit → setup → verify → `checkout -f` → require HEAD = verified commit. `hard_update`: in-progress / dirty refusal → fetch → resolve target → require exact override content → verify against the fetched target → `merge --ff-only <verified oid>` → require HEAD = target.
  - Dependent clones go through `need_clone`, which records a failing setup row instead of continuing, and hardened clone/checkout must succeed for a "not fired" result to count.
  - G07 fixture adds file names with a space and with an embedded newline.
- D-2: G06 now creates `.git/hooks`, plants `post-merge`, and confirms the file exists and is executable; a missing or non-firing control is recorded as inconclusive, never pass.
- Process observation: replaced with a builtin-only `/proc` scan (glob + `read`, no child processes) that records pid/ppid/pgid/comm and classifies PID 1 as the harness and anything else as unexpected (fails the cleanup row). The watchdog (`timeout`, `unshare`) runs outside the PID namespace and is not visible inside it.

### `tr` investigation (`tr-investigation.log`)
In a fresh namespace with no fixtures: (A) the run-1 pipeline verbatim still reported `tr`; (B) an unfiltered listing showed only pid 1 (`bash`), pid 15 (`bash`, the pipeline's `while` subshell, ppid 1) and pid 16 (`tr`, ppid 1); (C) the builtin-only scan saw only pid 1. Conclusion supported by observation: the run-1 `tr` was the observation pipeline's own final stage, not a fixture leftover. The run-1 row is kept as recorded.

### Run-2 results

| Case | Positive control | Hardened / behaviour | Status |
|---|---|---|---|
| G01–G03d clone, ff update, dirty, untracked, in-progress merge, diverged | — | as run 1 | pass ×6 |
| G04 template hook | fired (1) | not fired, hook not copied | pass |
| G05 global hooksPath | fired (1) | not fired | pass |
| G06 post-merge hook in managed checkout | hook installed+executable, fired (1) | not fired, updated | pass |
| G07 smudge filter (5 paths incl. space/newline names) | fired (3) | not fired, all content raw, 5 paths verified `unset` | pass |
| G07u same on update | — | not fired, updated, new file raw | pass |
| G08 LFS-like required process filter | fired (1) | clone rc 0, not fired, content raw | pass |
| G15 positive control (G15 smudge helper) | fired (4) | — | pass |
| G15-clone-attr_dir_blocked (file placed where `.git/info` must be) | — | aborted `ATTR_DIR_FAILED`, nothing checked out, not fired | pass |
| G15-clone-weak_override (`*.txt -filter` written after setup) | — | aborted `ATTR_NOT_UNSET(unspecified)`, nothing checked out, not fired | pass |
| G15-update-attr_removed (override deleted after fetch) | — | `REFUSED_ATTR_SETUP`, worktree/HEAD digest unchanged, not fired | pass |
| G15-update-weak_override | — | `REFUSED_ATTR_VERIFY:ATTR_NOT_UNSET(unspecified)`, unchanged, not fired | pass |
| G09 / G09u submodules | materialised | not recursed (clone and update) | pass |
| G10 `ext::` | fired (1) | refused | pass |
| G11 / G11b file transport without exception | — | refused | pass |
| G12 global fsmonitor | fired (2) | not fired | pass |
| G13 env-injected fsmonitor | fired (2) | not fired (env stripped) | pass |
| G13b injection kept + `-c core.fsmonitor=false` | — | not fired | observation |
| G14 credential helper | fired (1) | preserved under hardened options | pass |
| CLEANUP | — | logs 3,270 B (max 192 B); scratch removed; builtin scan: only pid 1 (`bash`, harness) | pass |

The `mkdir: … File exists` line in `run-2.log` is the deliberate G15 `attr_dir_blocked` fault.

### Remaining untested / limitations
Unchanged from run 1 (real HTTPS/SSH, network credential-helper invocation, `core.sshCommand`/`GIT_SSH`, proxy commands, `include`/`includeIf`, system config, diff/textconv drivers, other submodule entry points, one Git version/host, no filesystem sandbox). Additionally: attribute macros and `filter` set via other attribute files outside `$GIT_DIR/info/attributes` precedence were not separately exercised; the override relies on documented `info/attributes` precedence, which run 2 observed for `.gitattributes` in the cloned tree only.

## Owner acceptance (2026-10-24) — appended; runs 1 and 2 above unchanged

P00-T04 accepted on the basis of D01's local verification. Acceptance is limited to the tested procedure, Git 2.53.0, this host and this fixture set. It does **not** establish real HTTPS/SSH authentication behaviour, all configuration-inheritance paths, arbitrary attribute macros, other submodule entry points, or complete Git security. Run 1 (its failures, the inert G06 fixture and the `tr` observation later explained) is preserved as recorded.

# P02 manual check — unprivileged exit-4 test (closed 2026-10-29)

Owner 2026-10-29: "the unprivileged exit-4 test complete that". Lead: Opus. Tree: `main` `853bc74` (P02 code = `84fedd4`) plus uncommitted docs only.

**Why it was manual-pending** (`evidence/P02/P02-report.md` §3, T14 R-M1): `real_storage_failure_exits_4_when_permissions_are_enforced` returns early, and still counts as passed, when a mode-000 file can be opened. Inside the runner's `unshare -rn` the user maps to root, so that branch never ran.

**Method:** `python3 -I -B evidence/P02/manual-exit4/run.py evidence/P02/manual-exit4/out` (cwd repo root). Same toolchain 1.97.1, `env -i` variables, `CARGO_HOME`, lead target dir and `--frozen` as `dev/p02_check.py`. The only change is the namespace: `unshare -Un --map-current-user` instead of `unshare -rn`, so uid stays 1000 while the network namespace stays empty. Command: `cargo test -p cli --frozen --test p02_failures -- real_storage_failure_exits_4_when_permissions_are_enforced --exact --test-threads=1`, with `TMPDIR` set to a private directory.

**Proof that the enforced branch ran:** a host-side poller recorded the test's temp-directory names under that `TMPDIR`. `eacces-read` and `eacces-write` are created only after the early-return guard.

| Run | Namespace probe | Test | Temp dirs seen |
|---|---|---|---|
| enforced | uid/euid 1000; mode-000 open → EACCES; connect errno 101 | exit 0, 1 passed / 0 failed | `eacces-control`, `probe`, **`eacces-read`, `eacces-write`** (plus build linker temps) |
| control (`unshare -rn`) | uid 0; mode-000 open succeeded; connect errno 101 | exit 0, 1 passed (early return) | `eacces-control` only |

Every verdict in `out/result.json` is true: protected files unchanged, no temp dirs left behind.

**Result: PASS.** With permissions enforced, the real CLI binary exits 4 on an unreadable root (`validate`, `show`) and on a read-only data directory (`init`, `recover`). The error text is fixed with no path echo, and a refused write leaves the store tree unchanged. This closes the P02 manual-pending item.

**Limits:**
- Runs in a user namespace with the current uid mapped, not as a separate unprivileged account outside every namespace. Permission checks are the same, because the kernel enforces the uid-1000 mode bits.
- Single run on this host and filesystem only.
- The runner still skips this branch in `all`. No runner or test change has been proposed.

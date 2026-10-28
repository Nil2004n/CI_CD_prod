# P02-T12 — headless CLI and `dev` store entry points: integration record (uncommitted)

Date: 2026-10-28. Lead: Opus. Authority: owner 2026-10-28 "commit B2 locally, no push, and approve t12, spawn the required sonnet subagents … branch sessions do the work, main session for test, orchestration and debug cycle initiation". T12 approved; T13 not. "Commit B2 locally" was already satisfied by `2448abf`; T11 and T12 remain uncommitted (no separate commit approval).

Base: B2 `2448abf` + T11 (integrated 765/32, uncommitted; `evidence/P02/T11/record.md`), staged by the lead in the worker worktree (staged diff sha256 `9251bb90…fabd63`; lead check of that base PASS 765/0/0, 32).

## 1. Worker

| Item | Value |
|---|---|
| Packet | `planning/P02-packets/T12-W2.md` |
| Agent | `sonnet-implementer`, configured `model: claude-sonnet-5-5`, no harness isolation (confined by packet to the lead-prepared worktree; T11 lesson); resolution not observable, no substitution notice; self-report unverified, lead re-ran everything |
| Worktree | `.claude/worktrees/p02-t12-w2`, branch `p02/t12-w2` at `2448abf`, no commits; lead base staged and unchanged (sha re-checked), increment unstaged |
| Owned files changed | `application/src/dev_store.rs` (906 lines), `cli/src/{lib,args,commands,features,output}.rs`, new `cli/tests/p02_scenarios.rs` (1165 lines); `cli/src/main.rs` and all frozen files unchanged; no dependency change |

Worker-reported: final `all --min-passed 805` PASS (805/0/0, 32; `cli` 34); three worker mutations (Locked→Storage, renamed feature scenario, broken `--data-dir` parse) each failed the intended tests.

## 2. Lead verification (main tree = `2448abf` + T11 + T12, files copied byte-identical from the worktree; hashes in `sha256.txt`)

| Run | Result | Evidence |
|---|---|---|
| `all --slot lead --min-passed 805 --min-compile-fail 32` | **OVERALL PASS** — env, fmt, clippy, build, static PASS; test 38 suites **805/0/0, 32 compile_fail**; cli 3 suites **34/0/0** | `integrated-check/`, `.stdout` |
| floor control `test --min-passed 806` | FAIL on the floor only (805/0/0) | `control-806/`, `.stdout` |
| lead negative control: a missing root reported `valid` (`validated_record`) | FAIL as intended — `dev_store::tests::validate_is_read_only_and_reports_missing_roots` (unit) and `validate_fresh_store_and_single_root` (scenario, through the binary); test 803/2, cli 33/1. Restored byte-identical (`cmp` vs worktree), fresh mtime | `nc-lead-missing-as-valid/`, `.stdout` |
| **`all --final`** (floors from `dev/p02_baseline.json` = 805/32) | **OVERALL PASS** — first passing `--final` run in P02 (no `todo!` left; static checks all ok) | `final-check/`, `.stdout` |

Arithmetic: 805 = 765 + 6 (`dev_store` unit) + 13 (`cli` unit: args 6, commands 4, output 3) + 21 (scenarios). `cli` step 34 = 13 + 21. Floors raised to **805/32** (uncommitted).

Escaping control not run as a mutation: every stdout/stderr line is a `SanitisedText`, which only `domain::text::sanitise` constructs, so an unsanitised write does not compile without a `domain` change; behaviour is pinned by `seeded_marker_never_reaches_output_unescaped` (ESC, U+202E, U+2066, U+0085).

## 3. Lead code review (inline; formal reviewer/security-reviewer pass in T14)

Checked against request §5.9, T01-D7/D8/D12 and the packet: `run` keeps the frozen signature, no panic path on write failure; `validate`/`show` take the lock and read through `DataDir` → `FsOps::read` (64 MiB bound) → `read::decode` with the real `SchemaValidator`, no reconciliation or writes; `show` re-serialises the typed `AnyStore`; `recover` = `StoreWriter::open` (+ `restore_previous`); exit codes from `DevError::class()` via one `Exit` enum; usage errors never echo arguments; no env/home/cwd lookups in src (static PASS).

Observations (low, no change, no debug cycle):
- L-T12-1: `validate`/`show` create `.app.lock` if absent (via `lock::acquire`) — the only write; documented in module docs.
- L-T12-2 (worker-disclosed): exit 4 is pinned by the mapping test and injected-fault unit tests only; no end-to-end binary case (sandbox runs as root, so permission faults cannot be provoked). Same for `SchemaUnavailable` (exit 5).
- L-T12-3: lock-held scenarios use an in-process `File::try_lock` holder, not a second running CLI instance.

## 4. Worker decisions for owner confirmation (no change made)
- Q-T12-1: a fresh store validates as 5 `valid` + 4 `missing` (`config`, `suppressions`, `rag_index`, `conversations`; T01-D7 creates only five roots); `missing` is exit 0.
- Q-T12-2: `store show` of a missing root → verdict `missing`, exit 0 (alternative: exit 5).
- Q-T12-3: relative or non-existent `--data-dir` → exit 5 (`DataDirRejected`, filesystem-checked), not 2 (parser does no filesystem checks).
- Q-T12-4: no `EventEnvelope`/`AppEvent` is emitted by P02 CLI commands (dev records only); `--version`/`help`/`features` JSON lines use `"mode":"real"` with a `record` member.
- Q-T12-5: a failed `--restore-previous` returns the `StoreReconciled` record and exit 5 (reconciliation already happened).

## 5. Not done
Not committed (B3 after wave 3, owner decision). T13 not approved (`domain/src/store/classes.rs` still a B0 doc stub — `--final` does not check for it). T14 (runner static extension, CLI failure fixtures, reviews) and T15 not started. No network, no dependency change, no push.

# P02 wave 1a — integration record, baseline B1a

Date: 2026-10-27. Lead: Opus. Authority: P02 A6/A7/A9 (owner, 2026-10-26); T01-D3/D11 (owner, 2026-10-27); "I approve T04 and CA-04, proceed".
B1a = the commit containing this record, on `phase/P02-store` (B0 `a9ea9ce`).

## 1. Tasks and workers
All workers are `sonnet-implementer`, configured `model: claude-sonnet-5-5`, with `isolation: worktree`. Actual model resolution is not observable, there was no substitution notice, and every self-report is unverified; the lead re-ran everything.

| Task | Worker | Packet | Commit | Owned files touched (verified with `git diff --stat`) |
|---|---|---|---|---|
| T07 fs/fault/paths/lock/atomic | W3 | inline spawn prompt (same content as request §5.5/§9/§10 plus D-T04-1/2) | `38349f5` | `persistence/src/{fs,fault,paths,lock,atomic}.rs`, `tests/fs_atomic.rs` (34 tests) |
| T05a values/plan/provider | W1 | `planning/P02-packets/T05a-W1.md` | `67b0324` | `domain/src/store/{values,plan,provider}.rs`, `tests/store_{values,plan}.rs` (35 tests) |
| P02-D01 (debug) | fresh worker | `D01-W1.md` | `8d22da8` | `plan.rs`, `store_plan.rs` (+4 tests) |
| P02-D02 (debug) | fresh worker | `D02-W1.md` | `3ab305a` | `provider.rs`, `store_plan.rs`, `store_values.rs` (+5 tests) |

Lead commits:
- `d7a94b5` and `cc21f03` (merges);
- `4153f78` (D01 lead part);
- `3381a90` (D01 merge);
- `8043ebd` (D02 lead part, +3 unit tests);
- `8e91df3` (D02 merge);
- this commit (records, floors, status/handover).

**Spawn notes.**
- Three inline-prompt attempts to spawn W1 got no answer from the auto-mode classifier. This was a tool-side error, not a refusal. The fourth attempt worked, using a packet file.
- Workers reported the same transient classifier failures on Bash calls and retried unchanged commands.
- The owner then added project-local permission rules for the workers' permitted commands (`.claude/settings.local.json`, git-excluded).

## 2. Lead verification
`python3 -I -B dev/p02_check.py all` was run for every branch:

| Run | Branch / tree | Result |
|---|---|---|
| `lead-verify-w3/` | W3 `38349f5` | PASS 450/0/0, 32 compile_fail |
| `lead-verify-w1/` | W1 `67b0324` | PASS 451/0/0, 32 compile_fail |
| `evidence/debug/P02-D01/lead-verify-worker/` | D01 `8d22da8` (on W1+W3) | PASS 489/0/0 |
| `lead-d02-run1..3/` | lead D02 part (uncommitted) | FAIL, all lead defects, kept: (1) a literal U+202E/U+2028 written into a test string (rustc refuses bidi characters in literals), plus formatting; (2) a container-level serde echo (finding D02-2); (3) formatting |
| `lead-d02-run4/` | lead D02 part | PASS 492/0/0 |
| `evidence/debug/P02-D02/lead-verify-worker/` | D02 `3ab305a` | PASS 497/0/0 |
| **`B1a-check/`** | integrated `8e91df3` + floors | **PASS:** fmt; clippy `-D warnings`; test 26 suites **497 passed / 0 failed / 0 ignored**, 32 compile_fail, floors 497/32 enforced; build; cli (2 suites, 0 scenarios yet); static, all checks + self-test 51/51; lock `b34cb529…` unchanged |
| `B1a-floor-control/` | same, `--min-passed 498` | exit 1, as expected |

**Test arithmetic:** 416 (B0) + 34 (T07) + 35 (T05a) + 4 (D01) + 3 (lead D02 unit tests) + 5 (D02 worker) = **497**. Floors in `dev/p02_baseline.json` are raised to **497 / 32**.

Code read by the lead:
- `atomic.rs` in full, against §6.3;
- the W1 no-echo visitor (`values::deserialize_parsed`), which `mod.rs` now reuses;
- the D01/D02 diffs.

## 3. Reviews (read-only agents, HEAD `4153f78`)
- **`reviewer`:** no high; 5 medium; 10 low.
- **`security-reviewer`:** no high; 2 medium; 10 low.

Triage is in `evidence/debug/P02-D02/record.md`:
- in-scope items were fixed (D02-1, -3, -4, -5, -6);
- one item was moved to the read boundary (D02-2 → T08);
- several were recorded here (D02-7, -8, -10);
- several became packet requirements (D02-2 → T08, D02-9 → T10).

## 4. Recorded deviations and properties (D02-7, -8, -10)
**D-T04-4: W3 API deviations from request §5.5.** These are lead-accepted; they are persistence-internal and keep `std::fs` confined:
- mutating `FsOps` methods take a leading `step: WriteStep`;
- `symlink_metadata` returns `FileInfo { kind, len, mode }` instead of `std::fs::Metadata`;
- added `canonicalize`, `random_suffix` (`/dev/urandom`) and `open_lock_file` (in the trait, so a `Lock` fault can be injected);
- `acquire` takes `&dyn FsOps`;
- `remove_file`, `list_dir`, `symlink_metadata`, `canonicalize` and `random_suffix` are not fault-injectable;
- `MAX_REPORT_NAME_BYTES` = 200 (stricter than 255, so derived temp names stay under NAME_MAX);
- added `StorePath::{previous, parent, file_name}`, `DataDir::parent_of`, `atomic::is_temp_file_name`.

**`.prev` semantics.** After a successful `ReplacePrevious` followed by a failed `Replace`, `.prev` equals the current file and generation N−1 is lost; the current validated file survives. After a `SyncDir` failure the new content is already in place. Both follow from the §6.3 step order.

**Additive and unenforced items:**
- `ConstTrue` (in `provider.rs`) is an additive marker outside the §5.3 list;
- `PlanSnapshot.workflow_revision` `minimum: 1` is enforced by the schema layer only;
- an absent required-nullable key decodes as `None` at the typed layer (pinned by a test). The schema layer enforces presence (T08).

**Limitations disclosed by W3:**
- no crash or power-loss testing;
- path-based operations without `O_NOFOLLOW` (TOCTOU window, see O3);
- `RealFs::read` follows symlinks;
- the lock was tested only within one process (two-process case in T12);
- modes depend on umask (it can only tighten them).

## 5. Owner decisions held (would change frozen §5.2/§5.5 rules or add a dependency)
| ID | Finding | Recommendation (first) | Alternative |
|---|---|---|---|
| O1 | sec M1: `CredentialRef` key-likeness check is bypassed by `/ + . : @` separators (e.g. an AWS-secret shape is accepted) | Structured shape: `^[a-z0-9][a-z0-9._-]{0,63}(:[a-z0-9._-]{1,64})*$` (locator form only), plus the run check; stated as a heuristic | Count runs over the base64/URL alphabet only |
| O2 | sec M2 / rev M4: `DataDir` and `backup/`/`rag/` ownership and mode never checked; TOCTOU acceptance assumes 0700 | Reject a directory with `mode & 0o077 != 0` or not owned by the process (owner compared with the freshly created `.app.lock`'s uid; std only), with `PathRejected`; `store init` creates 0700 | Record as a disclosed limitation only |
| O3 | sec L1/L2, rev L7: lock/read/write open by path; no `O_NOFOLLOW`; FIFOs can block; hard-linked lock file | Add `libc =0.2.191` as a direct `persistence` dependency (already in the lock, accepted T05 graph; no lock change expected); `OpenOptionsExt::custom_flags(O_NOFOLLOW \| O_NONBLOCK)` + `fstat` regular/`nlink == 1` checks, no `unsafe` | Defer to P06 process hardening; keep as a limitation |
| O4 | sec L5, rev M5, rev L3: leading `-` accepted (`RemoteUrl` scp/ssh user, paths, patterns, tokens); `..`-like tokens | Reject a leading `-` in `RemoteUrl` (all forms) and in all token types; reject `.`/`..` as whole tokens. Paths and patterns keep `-` (repository file names); P04/P05 must pass them after `--` in argv arrays (recorded as an acceptance check) | Reject a leading `-` everywhere |
| O5 | sec L4: `Destination`/`RemoteUrl` hosts not canonical (case, numeric/hex IPv4 forms) | Require a lowercase host; reject all-numeric/hex dotted labels unless they parse as a canonical `Ipv4Addr`; loopback/link-local policy decided in P03/P16 | Canonicalise later at approval-matching time |
| O6 | sec L6: ssh/scp user unbounded, no key check | Cap the user at 32 bytes; apply the O1 run check | — |
| O7 | sec L10: `%00`–`%1F`, `%7F` and encoded `/ \ ? # @` accepted in URL paths | Reject them in `RemoteUrl`, `Destination`, `CanonicalRemote` | — |
| O8 | sec L8: escape/forbidden set omits U+00AD, U+034F, U+115F/1160, U+180E, U+2061–2064, U+3164, U+FFA0, U+FFF9–FFFB, tags U+E0000–E007F (P01-frozen set; P01 B1 decision 4 kept it unchanged) | Extend `text` escape set and `values` forbidden set together, with regression tests; text-safety change recorded against P01 D5 | Keep the P01 set; revisit in P06 with the redactor |
| O9 | rev L2: `SpdxExpression` accepts `" "`, `"()"`, padded values | Require ≥1 alphanumeric; no leading, trailing or double spaces | — |
| O10 | sec L9: `PathPattern` has no wildcard cap | Cap at 16 wildcards per pattern; P05 matcher must be linear-time (acceptance check) | Only the P05 matcher requirement |
| Q-T04-1 | `RunStarted.at` collides with the flattened envelope `at` (needed before wave 3) | Rename to `started_at` within app contract 1.1 | Nested envelope; drop the field |

If O1–O10 are approved, the lead proposes one bounded debug cycle **P02-D03** before wave 1b:
- W1 scope: `values.rs` + tests (O1, O4–O7, O9, O10);
- W3 scope: `paths.rs`/`fs.rs`/`lock.rs` + tests (O2, O3, the latter with the manifest change made by the lead);
- lead: `text.rs` escape set (O8) + specs/request text.

## 6. Packet requirements added for later tasks
- **T08:**
  - planted wrong-typed values at struct, array, map, enum and scalar positions never appear in any `Problem` (D02-2);
  - the schema layer rejects an absent required-nullable key (D02-6);
  - `workflow_revision: 0` is rejected by the schema layer.
- **T10:**
  - temp cleanup matches only `.<known store name or validated report name>[.prev].tmp-<16 hex>`;
  - only the root, `rag/` and `backup/` are scanned;
  - a regular-file check before each removal; removals are counted;
  - a `symlink_metadata` regular-file check before any store `read`;
  - the writer refuses to rotate a malformed or foreign target into `.prev` (D02-9).
- **T13:** the completeness test must cover string-map values (D01).
- **T12:** two-process lock conflict.

## 7. Not done at B1a
- Wave 1b (T05b, T06), wave 2 and wave 3 have not started.
- `text::store_read` keeps its temporary `#[allow(dead_code, reason)]` until B1, because `redacted`/`redacted_opt`/`redacted_vec` are first used by the wave-1b evidence-side models.
- The worker worktrees under `.claude/worktrees/` (4) are retained, untracked and git-excluded. Their branches are merged; removal needs preservation checks, done at B1.

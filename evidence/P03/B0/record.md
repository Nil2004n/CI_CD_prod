# P03-T04 — scaffold and baseline B0

Date: 2026-10-30. Lead (Opus) integration and verification. Authority: owner approval of P03-T01–T04 (2026-10-30).

## T04a — frozen stubs (lead)
As `planning/P03-T01-request.md` §3/§5:
- **Domain (fully implemented, no serialised contract change):**
  - `contract::FixedText`;
  - `RawInput` gets a best-effort clearing `Drop` and a `Debug` without length;
  - `text::redact_validated` (`pub(crate)`) and `Destination::to_redacted`.
- **Persistence:**
  - `StoreError::RootForeignDataDir`;
  - the provided method `FsOps::effective_uid`, with `FaultFs` delegating;
  - stubs `process_effective_uid` and `parse_status_euid`.
- **Application:**
  - `permission/{mod,destination,fingerprint,evaluate}.rs`. Types in `mod.rs` are complete; the bodies are stubs.
  - Stubs `approvals.rs`, `child_env.rs`, `credential.rs` and `dev_permission.rs`.
  - `DevError::RootForeignDataDir` (class Operation → exit 5).
- **CLI:**
  - `Session` and `run_session` (`run` unchanged in signature);
  - `main.rs` collects `HostEnv` from `vars_os()` and passes `stdin().is_terminal()`.
- Stub bodies are `todo!()` under `#[expect(clippy::todo, reason = "B0 stub")]`.

**Test changes, both stricter, not weaker:**
- `domain/tests/contract.rs` `raw_input_debug_shows_only_the_length` is renamed `raw_input_debug_shows_neither_text_nor_length`. It now asserts `RawInput(<redacted>)` and no length.
- `raw_input_commands_debug_hides_the_text` now requires `RawInput(<redacted>)` instead of the substring `bytes`, and rejects `bytes>`.

**Finding during integration (fixed in the lead's own stub):** the P02 `redact_static` rule takes its argument up to the first `)`, so the literals `"local-user (cli)"`/`"(tui)"` were reported. The texts were changed to `local-user/cli` and `local-user/tui`, and the rule is unchanged.

## T04b — runner `dev/p03_check.py` (worker W1)
- **Worker:** `sonnet-implementer`, configured `model: claude-sonnet-5-5`. The resolved model was not observable and no substitution notice appeared, so the self-report is unverified.
- **Packet and branch:** packet `planning/P03-packets/T04b-W1.md`; worktree `.claude/worktrees/p03-t04b-w1`, branch `p03/t04b-w1`, base `26e1d19`. The only output was the untracked `dev/p03_check.py`, which the lead copied into the main tree unchanged.
- **Lead review:**
  - the rule functions work on a file map and the self-tests call the same functions;
  - the allowlists match request §5.8;
  - lock hash = T02 value; floors come from `dev/p03_baseline.json`; target dirs use the `p03-` prefix.
- **Worker-stated deviations (accepted):** a few rules are stricter (any `redact_validated` mention; the `CredentialSecret as` alias; `process::*`; env brace and glob imports). Bypass limits of regex rules are recorded.

## Verification (lead, main tree, B0 content = this commit)
| Command (cwd repo root) | Result |
|---|---|
| `python3 -I -B dev/p03_check.py all --slot lead --out <scratch>/all-b0-2` | **OVERALL PASS**: fmt, clippy `-D warnings`, test 41 suites **866/0/0**, 0 ignored, **39** compile_fail, build, cli 48/0/0, static (all checks, self-test **167/167**) |
| `python3 -I -B dev/p03_check.py static --slot lead --final --out <scratch>/static-final-b0-2` | FAIL as intended: `todo!` stubs and `clippy::todo` expects remain until the waves land |
| Lead negative controls (scratch copy, `--repo`): `CredentialSecret::new` in `cli/src/commands.rs`; `std::process::Command::new`; `std::env::var_os`; `expose()` in cli; an `Authorisation { .. }` literal in cli | each **static FAIL**; restored copy **PASS** |
| Earlier lead runs with `dev/p02_check.py` (clippy/test/cli) before the P03 runner existed | first clippy run FAIL (3 unused `f` parameters and 2 unreachable consts in the lead's stubs), fixed; first test run FAIL (`raw_input_commands_debug_hides_the_text`, the old length placeholder), fixed as above; then PASS 866/39 and cli 48 |

Floors are unchanged at 866/39 (`dev/p03_baseline.json`); B0 adds no tests. `summary.json`/`static.json` are copied here.

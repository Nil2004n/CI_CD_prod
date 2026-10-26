# P01 wave 1 (T05–T07) and baseline B1

Date: 2026-10-26.
- **Authority:** owner approval "proceed to wave 1" (2026-10-26), with bounded debug cycles pre-authorised (`planning/P01-T01-request.md` §9).
- **Base:** B0 `8a1bec3`.
- **Executed by:** Opus (lead) with three `sonnet-implementer` workers in worktrees.
- **Result: B1 integrated, lead check run 5 PASS.** Baseline B1 is the commit containing this record.

## 1. Workers (first P01 spawn; CLAUDE.md model-recording rule)
| Worker | Task | Branch / commits | Configured model |
|---|---|---|---|
| W1 | T05 ids + time | `p01/w1-ids-time` `f6f2b5a` | `claude-sonnet-5-5` (agent definition) |
| W2 | T06 text newtypes (+ D01, D02) | `p01/w2-text` `2495d85`, `622f989`, `3b1547e` | same |
| W3 | T07 states + vocab | `p01/w3-states` `23a9dfa` | same |

- **Model:** the model that actually served the workers is **not observable** from the lead session; there was no substitution notice. Worker self-reports state only "configured: claude-sonnet-5-5 (unverified)". **Worker reports are not evidence.**
- **Scope:** every worker started from B0 via the step-0 fast-forward, touched only its owned files (lead `git diff --name-only` per branch), and made local commits on its own branch only. No pushes, no network, no installs.
- **Communication:** all of it went through hand-back reports and lead `SendMessage` resumes (delivery confirmed by the tool). The board file (`~/.local/share/pwt-dev-tools/_p01/board.md`) stayed empty.

## 2. Lead verification per branch
Command: the current lead runner, `python3 -I -B dev/p01_check.py all --slot lead --package domain --repo <worktree>`.

| Branch | Evidence | Result |
|---|---|---|
| W3 `23a9dfa` | `evidence/P01/T07/lead-verify` | **FAIL**, 5/18 failed. Cause: **lead runner defect**, see §3 |
| W1 `f6f2b5a` | `evidence/P01/T05/lead-verify2` | PASS, 281/0/0 |
| W2 `2495d85` | `evidence/P01/T06/lead-verify2` | PASS, 34/0/0, 8 compile_fail ok |
| W3 `23a9dfa` | `evidence/P01/T07/lead-verify2` | PASS, 23/0/0 |

**Lead code review** (diffs read in full for source, spot-checked for tests):
- the transition tables and terminal sets match T01 §5.4, including D1/D2;
- the escape set matches §5.3 exactly;
- ID parsing and the time algorithms are correct (bounds, leap years, negative epoch);
- no new public items;
- the fallbacks in the test helpers do not mask any assertion;
- each compile_fail doctest differs from its positive control only in the offending line.

## 3. Anomalies and how they were handled
| # | Anomaly | Classification | Handling |
|---|---|---|---|
| A1 | W3 verification failed: test binaries panicked at **B0** stub lines | **Lead runner defect**, confirmed. Cargo artifact names for workspace members do not depend on the workspace's absolute path, so the shared `p01-lead` target directory linked the main tree's stale `libdomain` (same `domain-b048e81a656f2923`, rlib time 10:52 = lead draft build). This violated T01 §2 (one target directory per worktree) | Runner fixed: target directory = `p01-<slot>-<sha256(repo path)[:12]>`, plus `--repo`. All three branches re-verified PASS. Failed run kept |
| A2 | First re-verification loop exited 2 for all branches | Lead shell error (zsh `set -- $t` does not word-split, the known P00 trap) | Nothing ran (workspace guard). Stray dirs inspected and removed. Re-run under `bash` |
| A3 | Lead suspected shifted doctest headings in W2's `text.rs` | **Lead misdiagnosis** | Debug cycle **P01-D01** (W2): no defect; wording clarified (`///` lines only). `evidence/debug/P01-D01/record.md` |
| A4 | Review findings on W2's `text.rs` (no static constructor; vacuous-risk doctest control) | Confirmed review findings | **P01-D02** (W2): `pub(crate) redact_static`, serde_json in control. `evidence/debug/P01-D02/record.md` |
| A5 | Run 4: new static check flagged W2's `#[cfg(test)]` identity redactor | **Lead checker defect** | Checker ignores `#[cfg(test)] mod` blocks; self-tests both ways |
| A6 | Run 2: clippy `result_large_err` on `dispatch_guard` once `ApprovalRequested` grew | Expected (reviewer #10); the signature is frozen by T01 §5.5 | Local `#[allow(clippy::result_large_err, reason = …)]` on that function only |

No worker defect required a code repair. The two debug cycles each ran once, so no defect reached the two-cycle limit.

## 4. Reviews (read-only agents; Read/Grep/Glob only, nothing run)
- `reviewer` and `security-reviewer` covered the integrated domain code, the lead contract draft and the runner. Their reports are not evidence; the lead triaged them.

| Finding | Source | Disposition |
|---|---|---|
| `AppCommand` derived `Debug` prints raw URL and chat text | sec M1, rev #7 | **Fixed (lead):** `RawInput` newtype, `Debug` shows the length only |
| `display_name` and `purpose` sanitised only; `adapter_id` free text | sec M2, rev #3 | **Fixed (lead):** `RedactedText`; closed `AdapterId` enum (Data §21 IDs, serde names `docker-compose`, `rust-schema`) |
| Static-check bypasses (tuple variants, comma generics, multi-line, aliases, paths, `OsString`/`PathBuf`/`Vec<u8>`/`Arc<str>`, braces in comments, `//` in strings, missing block passes) | sec M3/L6, rev #8 | **Fixed (lead):** allowlist type parser over comment-stripped, string-masked code; data enums checked recursively; tuple variants rejected; `--final` requires both enums; self-test 21/21 |
| Lock check compared versions only | rev #8 | **Fixed:** pinned (version, source, checksum) |
| Version gate only in `env`/`all` | rev #2 | **Fixed:** the gate runs before every Cargo mode |
| No test-count floor | rev #9 | **Fixed:** `--min-passed`, `--min-compile-fail`; compile_fail counted. B1 floors 341 / 9 |
| `ExportFormat::Markdown` → `"markdown"` vs store `md` | rev #1 | **Fixed:** `md`; the `ReportWritten` field is renamed `md` |
| Field list not traceable (selector modes/patterns, export location, plan approvals, approval-dialog fields) | rev #6 | **Fixed:** `SelectionChange::{Mode, Patterns}` + `SelectorMode`; `ExportStatusChanged.location`; `RunPlanned.required_approval_ids`; `ApprovalRequested` gains workflow id/revision, repo ids, destinations, `DataCategory`, `ApprovalLifetime` (UI Schema §5 permission modal) |
| `CredentialSecret` drop leaves spare capacity unzeroed | sec L3 | **Fixed:** zero the full capacity (best effort, still not secure erase) |
| `expose()` callable anywhere; no guard against `Clone`/serde regressions | sec L4 | **Partly:** static check on `.expose()` call sites; compile_fail guards assigned to T08 |
| Static constructor and doctest control | sec L1/L2, rev #5, #11 | **P01-D02** |
| `SaveWorkflow` payload provisional (selectors, schedule, settings arrive with P14) | rev #6 | **Owner decision** (report) |
| Event envelope (`event_id`, `at`, `level`) absent; most events carry no time | rev #6 | **Owner decision** (report); lead proposal: envelope added by the coordinator at emission, specified in P02 |
| `ChatResponseChunk` coalescing needs `RedactedText` concatenation; secrets split across chunks | rev #4 | **Owner decision** (frozen §5.3 surface; P17-relevant) |
| Escape-set gaps (Unicode tags U+E0000–E007F, U+206A–206F, U+FFF9–FFFB, variation selectors, fillers); LF in single-line fields; `<`/`>` not HTML-safe | sec I1 | **Owner observation** (D5 set unchanged) |
| `<U+XXXX>` collision; never unescape; hash pre-escape values | sec I2 | Recorded constraint for P02/P10 |
| No length bound on safe text | sec L5 | Accepted per T01; P06 budgets |
| `ToolStatus` vs store `availability` differ; `Coalescible` covers two meanings; one round-trip loop circular | rev #12 | Recorded; T08 tests pin delivery classes |

## 5. Integrated checks on the phase branch
Command: `python3 -I -B dev/p01_check.py all --slot lead [--min-…] --out evidence/P01/B1/check-run<N>`.

| Run | State | Result |
|---|---|---|
| 1 | W1+W3+W2(D01) merged, lead draft v1 | PASS: 338/0/0, 9 compile_fail |
| 2 | + lead review fixes | **FAIL** clippy (`result_large_err`, A6) |
| 3 | + allow | PASS: 338, 9 |
| 4 | + W2 D02 merged | **FAIL** static (A5) |
| 5 | + checker fix | **PASS**: env, fmt, clippy (0 warnings), test (18 suites, **341 passed, 0 failed, 0 ignored, 9 compile_fail ok**), build, static (11/11; self-test 21/21) |

**At B1, by design:** 8 `todo!` markers remain, all in `contract.rs` T08 stubs (informational until `--final` at T09). `AppCommand`/`AppEvent` have no behaviour yet (`status`, `delivery`, `dispatch_guard`).

## 6. Limitations
- Worker model resolution is unverified.
- The reviews were read-only and model-generated; they did not run code.
- Hardening the static check reduces bypass classes but does not parse Rust fully: macros generating fields, and types defined in other modules and referenced by path, are rejected rather than analysed.
- The worker worktrees used the B0 runner for their own runs; only lead runs are evidence.

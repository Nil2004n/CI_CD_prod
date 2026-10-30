# P03-D01 — wave-1 review findings (attempt 1 of 2; closed)

Authority: pre-authorised debug cycles, C10 = 10a (`planning/P03-T01-request.md` §9). Base B1 `b1ca4cb`. Findings from the lead-spawned `reviewer` and `security-reviewer` (Sonnet, read-only) on B1, confirmed by the lead in code.

| Item | Finding | Fix | Owner | Evidence |
|---|---|---|---|---|
| 1 (M1, medium) | `OperationArg` derived `Debug` → `{:?}` of a `PermissionRequest` printed argument values (lead-owned B0 code, `permission/mod.rs`) | Custom `Debug`: name + `value_bytes`; doc: arguments must never carry credentials (L5: unsalted fingerprint) | Lead | `tests/p03_request_debug.rs` (2 tests); negative control (derive restored) → 2 FAILED (`item1-negctl-summary.json`) |
| 2 (A2 low, A3 comment) | G4 accepted a `workflow_revision` grant with null workflow (reported `Lifetime` instead of `Workflow`); comment claimed rule 14 enforced twice | G4 fails such grants; comment corrected; A1 pinned by test (workflow-scope grant without run serves every run; one with run R1 → `Mismatch{Run}` in R2) | W1′ (`sonnet-implementer`) | worker mutation: G4 clause removed → 2 FAILED; lead diff review |
| 3 (L2, L4 low) | Inherited `TZ` unvalidated; declared-name uniqueness only unit-tested | `TZ` kept only if UTF-8 and `IanaTimezone::parse` OK (request §7 amended); `assemble` rejects duplicate declared names with `DeniedName` | W2′ (`sonnet-implementer`) | worker mutations A/B/C each caught; lead diff review |

**A1 decision (lead):** evaluator G6 unchanged (strict). **T08 requirement:** workflow-scope grants (`approve_workflow_scope`) are stored with `run_id = null`.

**Deferred, recorded (not fixed here):** L1 ssh store form drops the path → T08/T10 must show `canonical()` to the user; L3 token-like URL paths persisted (P02 limit, P06 gate); H1 `evaluate` is `pub` and `Grant` is `Copy` → T08/T11 static rule "only `approvals.rs` acts on `Permitted`"; H2 run-grant end-of-run expiry → T08; H5 best-effort clearing (documented); B1/H3 R7 permanent denial and R8 shadowing → owner question; B5/H4 private/loopback IPv4 destinations → P04/P16; B6 deferred capabilities → confirm rejection in semantic validation (T08).

**Lead verification:** `python3 -I -B dev/p03_check.py all --slot lead --out evidence/debug/P03-D01/check-lead` (cwd repo root, B1 + D01): OVERALL PASS — fmt, clippy, test 48 suites 1005/0/0, 0 ignored, 41 compile_fail, build, cli 48, static. Arithmetic 998 + 2 + 1 + 4 = 1005. Floors raised to 1005/41.

# P01 conformance check — system vs plan, architecture, PRD (2026-10-26)

**Owner request:** "perform a thorough check on the existing system and compare it with respect to the plan, architecture and PRD; are the limitations expected and desired for this phase?"
- Executed by Opus (lead), read-only, except for these evidence files and the P01 report section.
- Repository: `phase/P01-foundation` at `aaadfb2` = `origin`; `main` = `origin/main` = `485b3a8` (untouched); no worktrees.

## 1. Fresh verification
`python3 -I -B dev/p01_check.py all --slot lead --final --out evidence/P01/conformance/check-run`: **PASS**.
- env: 1.97.1; netns probe errno 101.
- fmt 0; clippy 0 (deny-level lints); build 0.
- test: 20 suites, 415 passed / 0 failed / 0 ignored, 32 compile_fail.
- static 17/17.

## 2. Document cross-check
Script: `evidence/P01/conformance/conformance.py`, run with `python3 -I -B`; output in `conformance.txt` and `conformance.json`. It parses the specification documents and schema directly, not the project's own tests.

**Result: 25 PASS, 2 NOTE.**

| Area | Result |
|---|---|
| Arch §6 command set (29) and event set (34) equal the Arch tables | PASS |
| Arch §6 pilot/reserved status and delivery class for every variant | PASS. The reserved events' "—" maps to MustDeliver by documented choice |
| Arch §6 reserved → `OperationFailed{code="deferred"}`; `ChatFailed` reasons incl. owner-accepted provider outage; no raw-string event fields | PASS |
| Arch §7 run machine: every Arch transition allowed; the only extra is `cancelling → interrupted` (T01-D1) | PASS |
| Arch §7 check machine = Arch + T01-D2; approval machine = Arch + schema revoked/expired | PASS |
| Arch §4 core domain types present or mapped | PASS. `ExecutionStatus` = `RunState`; `SourceRef` is a record type (P02); `SourceId` exists |
| Arch A-11 independent version lines | PASS |
| Tech §4 crate list = workspace members (7) | PASS. The script's first run misparsed the `tests/` subtree as crates; the parser was corrected and re-run |
| Tech §4 `tests/{fixtures,contracts,integration}` | **NOTE (expected).** Deferred by the approved T01 §1; created when first used |
| Tech §2 / D-01 dependency scope | PASS (serde, serde_json only) |
| Data §7 schema enums: `DataCategory`, `ApprovalLifetime`, `SelectorMode`, `ExportFormat`, `Severity` | PASS |
| UI §5 `tools.states` = `ToolStatus` | PASS |
| **Data §7 `ToolManifest.availability` vs UI §5 `tools.states`** | **NOTE: specification inconsistency (not a code defect).** Store: `ready, missing, incompatible, deferred, unconfigured`. UI: `ready, missing, incompatible, unavailable, denied, deferred`. The code follows the UI list, so `unconfigured` has no event value, and `unavailable`/`denied` have no store value. Needs an owner/lead mapping decision before P06 (preflight, `tools.json`) |
| PRD §16 deferred capabilities ↔ reserved commands (RES-06, run comparison, RES-07 narration, chat persistence, SCAN-06) | PASS |
| Data §18.6a deferred capabilities | PASS |
| PRD KEY-01: no serde, Clone or Display path for the key | PASS |

## 3. Plan and process
- **Plan §4 P01 row:** workspace, pinned toolchain, typed IDs/states, canonical DTOs and text newtypes delivered. Checks: fmt, clippy `-D warnings`, test and build verified. DTO round-trip amended by T01-D3. cargo-audit "if installed": not installed (A4).
- **Plan §5A tasks T01–T10:** all done. Records are in `evidence/P01/`.
- **Plan §3.8 parallel policy:** at most 3 workers; worktrees with step-0 baselines; lead-owned shared files; lead verification of every branch; finished workers shut down; worktrees removed after preservation checks.
- **CLAUDE.md:**
  - no secrets: P01 diff scan for credential patterns found 0 hits; synthetic markers appear only in tests;
  - no network or installs;
  - every commit and push was owner-approved;
  - workers never committed outside their own branches;
  - every worker report was re-verified;
  - the Haiku agent is still not installed: Opus wrote all continuity files and says so.
- **Files changed in P01:** only `workbench/`, `dev/`, `evidence/`, `planning/`, the root control files and one owner-accepted line in `03_Architecture.md`. No other spec or schema was changed.
- **Continuity:** `status.md` and `handover.md` match Git state; 0 unpushed commits.

## 4. Are the limitations expected and desired for this phase?
| Limitation | Classification | Basis |
|---|---|---|
| No runtime behaviour (nothing calls `dispatch_guard` or emits events) | **Expected** | Plan §4: P01 = types and contracts; coordinator and store from P02 |
| Event states not constrained; `AppEvent` JSON ≠ store `Event`; no envelope | **Expected, owner-decided** | B1 decision 2: envelope at emission, P02 |
| `RedactedText` proves a redactor ran, not that secrets were removed | **Expected (by design)** | The minimal redaction module is P06 (plan §4) |
| `redact_static` literal-only rule enforced by the static check, not the compiler | **Accepted design** | P01-D02 + T09 call-site check |
| Escape-set gaps; LF kept; not HTML-safe; `<U+XXXX>` ambiguity | **Owner-accepted** | D5, B1 decision 4. Renderer escaping belongs to P11/P12 |
| Best-effort key zeroing; `RawInput` not zeroed | **Expected** | KEY-01 hardening and credential module are P03 |
| Build scripts without a filesystem sandbox | **Owner-accepted** | A3, same as P00 |
| cargo-audit not run | **Owner-accepted, plan-conformant** | A4; plan P01 check is "if installed" |
| Toolchain pin enforced by the runner only | **Owner-chosen** | A5 = 5a |
| Test-fixture directories deferred | **Expected** | Approved T01 §1 |
| Static checks pattern/parser-based | **Accepted** | Negative controls show each check fires |
| Worker model resolution unverified | **Known plan risk** | Plan §8 B-04 |

**Answer: yes.** Every limitation is either inside the plan's phase boundary (it belongs to P02, P03, P06, P11/P12 or P17) or was explicitly accepted by the owner. None leaves an approved P01 requirement unmet.

**Not P01 limitations, raised for the owner:**
1. **Specification inconsistency:** `ToolManifest.availability` vs UI `tools.states` (§2). Decide the mapping before P06.
2. **Carry-forward requirement:** plan §4 says the text newtypes exist "so unsanitised text cannot reach persistence types". The P02 store models must use `RedactedText`/`SanitisedText` for every free-text field, and the P02 static checks should extend the field-type allowlist to them.

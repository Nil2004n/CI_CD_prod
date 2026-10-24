# P00 report (draft)

Draft for Opus review and owner decision, base `3decec1`, 2026-10-24. Compiled only from repository records (cited by path). Nothing here is an acceptance, a test result of its own, or a statement that a phase is complete.

## 1. Summary and status

- **P00 accepted by the owner 2026-10-24 with qualifications** (section 11). This report was drafted at T11 and updated at closure. `status.md` records T01, T02, T04, T05, T06, T07, T08, T09 and T10 as accepted by the owner (several qualified), T03 as decided (D3-1 to D3-3) without a separate "accepted" record, and T11 as the freeze in this draft, pending owner acceptance.
- P00 produced documentation, spike code and evidence only. There is no application code; spike results are not production evidence (`status.md`, "Implementation" and "Application tests" rows).
- Spikes establish feasibility for the tested cases, hosts and pinned versions. Every accepted task carries qualifications (section 2), and carry-forward requirements FR-01 to FR-30 (section 6) must be implemented and re-verified in their phases.
- No blanket "all green = secure": manual checks, unsupported coverage and unverified items stay explicit (sections 7 and 8).

## 2. Per-task outcome (P00-T01 to T11)

Sources: `status.md` rows, `plan.md` §5.0, and the task records named below.

| Task | Acceptance state | Key qualifications and evidence |
|---|---|---|
| T01 spike protocols and fixture specifications | Accepted by owner 2026-10-24 | Documentation only; tool flags, file names and exit codes in it were unverified candidates for later spikes. `spikes/P00-T01-spike-protocols.md`, `spikes/P00-T01-fixture-specifications.md`, `evidence/P00/P00-T01/check.md` |
| T02 installation request and installs | Accepted as installation planning only (2026-10-24); installation checkpoint accepted with limitations | Request A (Go binaries) and B1 (standalone CPython 3.12, isolated venvs) installed and verified; Gitleaks and kubeconform have no upstream signatures; PyPI provenance not verified (development-only exception); cargo-audit deferred to P01 (R-4). `spikes/P00-T02-installation-request.md`, `evidence/P00/P00-T02/install-record.md`, `evidence/P00/P00-T02/lookups.md` |
| T03 crate request (rev 2) and D3-1 to D3-3 | **No separate explicit acceptance record** (`status.md`, `plan.md` §5.0); D3-1 executed, D3-2/D3-3 provisional selections | Direct-candidate RustSec screening only; transitive graphs not reviewed at that stage; selections are candidates, not validated dependencies. `spikes/P00-T03-crate-request.md`, `evidence/P00/P00-T03/research.md`, `evidence/P00/P00-T05-D3-1/record.md` **Closure decision (owner, 2026-10-24):** the T03 research/design selections are ratified as **provisional where not exercised by a spike**. This is a current closure decision, not a historical acceptance and not evidence that those crates were tested. |
| T04 Git-safety spike | Accepted 2026-10-24 (qualified) | Limited to the tested procedure, Git 2.53.0, this host and fixture set; not real HTTPS/SSH auth, all config-inheritance paths, attribute macros, other submodule entry points. Run 1 failed (4 fail, 1 inconclusive); D01 re-run 36 pass / 0 fail (+1 observation). `evidence/P00/P00-T04/record.md` |
| T05 process supervision | Accepted 2026-10-24 as a feasibility spike with findings | Not production-supervisor acceptance: output-limit enforcement, cleanup outside the PID-1 harness, escaped descendants, ANSI and early close unverified. Formal run 15 pass / 0 fail. F-1 to FR-01, F-2/F-3 to CA-02, F-4 to FR-02. `evidence/P00/P00-T05/record.md` |
| T06 YAML parser and JSON Schema | Accepted 2026-10-24 based on D01, with conditions | Tested combination only (saphyr-parser 0.2.2 + spike composer + jsonschema 0.60.1, defaults off). First run 96 pass / 5 fail; D01 121 pass / 0 fail / 7 observations; J16 35/35. Suite conformance, production limits (P05) and build-time code beyond pattern review unverified. FR-05 to FR-17. `evidence/P00/P00-T06/record.md` |
| T07 keyring | Accepted 2026-10-24 as a feasibility spike with qualified findings | Tested isolated setup only. Not accepted: direct in-process use, persistent storage as pilot or product feature, native cancellation, real prompter, other implementations. Spike-only advisory exception K-A. `dbus-send` was an unlisted operation (disclosed). FR-18 to FR-24. `evidence/P00/P00-T07/record.md`, `evidence/P00/P00-T07/advisory-exception.md` |
| T08 PDF renderer | Accepted 2026-10-24 with qualifications | pulldown-cmark + printpdf, both featureless, tested cases, debug build. Run1 failed; D01 wrote 9 of 10 PDF cases (Korean out of scope, D-15; no reclassification re-run). Whole-font embedding, no shaping/bidi/RTL, empty `/Annots` permitted, **visual correctness unverified**, WeasyPrint suspended. `evidence/P00/P00-T08/record.md`, `evidence/debug/P00-T08-D01/record.md` |
| T09 per-tool scanner spikes | Accepted 2026-10-24 with qualifications | yamllint, gitleaks, kubeconform, actionlint per run1 contracts; Checkov contract revision 2 only, for the tested Kubernetes selected-file workflow (R1 variants rejected). Limits: output guard covers registered synthetic fixtures only, write tracking is not confinement, process-group escape not contained, ShellCheck/Pyflakes untested, production adapters and staging helper not built. FR-25 to FR-29. `evidence/P00/P00-T09/record.md`, `evidence/debug/P00-T09-D01/record.md`, `evidence/debug/P00-T09-D02/record.md` |
| T10 Kubernetes schema cache plan | Accepted 2026-10-24 (qualified) | Planning and offline reference prototype (47 unit tests, consistency check 64/64 by Opus); not a production cache, schema coverage or collision-safe resolution. Finding R-1 to FR-30 (mandatory for P13). O-1 to O-6 open. `evidence/P00/P00-T10/record.md`, `spikes/P00-T10-schema-cache-plan.md` |
| T11 freeze and P00 report | **Accepted 2026-10-24 as the freeze/report preparation task, with qualifications** | `spikes/p00-freeze/tools.json`, `spikes/p00-freeze/tools.md`, `spikes/p00-freeze/crates.json`, `spikes/p00-freeze/crates.md`, this report. Request: `spikes/P00-T11-request.md` |

CA-02 (store 1.3, wrapper protocol 1.1, specs v0.3.3) was applied and confirmed by the owner in P00 (`evidence/P00/CA-02/`, `evidence/P00/CA-02-versioning/`; `status.md`).

## 3. Decisions and open questions

### 3.1 Decisions D-01 to D-15 (`plan.md` §1; owner, 2026-10-23, D-15 on 2026-10-24)

| ID | Decision (gist) |
|---|---|
| D-01 | Rust + Ratatui, Crossterm, Tokio, Serde; other dependencies via spikes |
| D-02 | Linux x86_64 first pilot target; Windows/macOS not described as supported until verified |
| D-03 | Native Rust process adapters and direct Git are the baseline; `.sh` wrappers optional (X6) |
| D-04 | Baseline yamllint, Gitleaks, rust-schema, Checkov; adapters actionlint, kubeconform; conditional zizmor, Compose, Helm, Trivy; versions/coverage frozen after spikes |
| D-05 | Scanners installed separately and detected by preflight; Python prerequisite declared; no auto-install, sudo or bundling |
| D-06 | Minimal OpenRouter BYOK assistant stays in the pilot (explicit model, session-only credentials, redaction); mock HTTP is the test baseline |
| D-07 | PDF renderer comparison spike; WeasyPrint not selected for being installed |
| D-08 | Contract conflicts C2 to C7 resolved through CA-01 |
| D-09 | 10-hour sessions; estimates in hours and sessions with contingency separated |
| D-10 | Installs not approved by default; separate installation request |
| D-11 | Versioned cached Kubernetes schema approach; no download or bundling yet |
| D-12 | Figma timing unconfirmed; demo UI and stable contracts proceed |
| D-13 | HTTPS and SSH; no unapproved submodule recursion, LFS/filter execution or hooks |
| D-14 | Phase-branch policy; no commits, merges or pushes without approval |
| D-15 | Korean (Hangul) support out of scope for the entire project; Korean text still processed as data and degraded visibly (`<U+XXXX>`) |

### 3.2 Open questions (`plan.md` §1, "Open owner questions")

| ID | Question | State |
|---|---|---|
| Q-01 | Sessions per week for the four-week cap | **Open** |
| Q-02 | Confirm AI report narration (RES-07 optional part) is deferred | **Open** |
| Q-03 | Keep version identifiers or bump store 1.3 / protocol 1.1 | Answered 2026-10-24: store 1.3, wrapper protocol 1.1 |
| Q-04 | Production OpenRouter model ID | **Open** (real-API acceptance only) |
| Q-05 | Which scope exchanges (§7.4), if any | **Open** |
| Q-06 | Read-only web lookup of official pages | Answered 2026-10-24 for T02, T03 and T10 (T10 approval; `evidence/P00/P00-T10/lookups.md`). History: when this report was drafted, the `plan.md` §1 Q-06 row still said T10 lookups needed approval; that row was **corrected at T11 integration** and is no longer a defect. |

## 4. Debug cycles (`plan.md` §9)

| Cycle | Defect | Outcome |
|---|---|---|
| P00-T04-D01 | Fail-open attributes setup (no `.git/info/` with empty template); inert G06 hook fixture; cleanup listing artefact | Completed; re-run 36/0/0 (+1 observation); T04 accepted (qualified). `evidence/P00/P00-T04/record.md` |
| P00-T06-D01 | Parser character offsets vs bytes; BOM not stripped; recursion-limit classification; Y18 expected-name error | Completed; re-run passed (121/0/7, J16 35/35); T06 accepted. `evidence/P00/P00-T06/record.md` |
| P00-T08-D01 | `/Annots` criterion vs printpdf (empty inline array accepted); H-1 BOM counted as RTL; H-2 failed PDFs deleted; O-1 overflow diagnosis | Completed; 9/10 written, U1 Korean failure out of scope (D-15); T08 accepted with qualifications. `evidence/debug/P00-T08-D01/record.md` |
| P00-T09-D01 | Checkov contract R1 executed repository Python (cwd discovery and config merge); replacement R2 | Completed; no execution under R2, potency controls fired, findings preserved. `evidence/debug/P00-T09-D01/record.md` |
| P00-T09-D02 | H-3 PEM body lines not covered by the output guard; D01 attribution question | Completed; self-test 29/29, fresh gitleaks run equal to run1, attribution resolved from existing evidence. `evidence/debug/P00-T09-D02/record.md` |

History: when this report was drafted, `evidence/debug/README.md` still said "None exist". It was **corrected at T11 integration** and now lists the three debug-cycle directories; this is no longer a defect.

## 5. Frozen artefacts (pointers; hashes are not restated here)

| Artefact | Path |
|---|---|
| Tool freeze (machine-readable and narrative) | `spikes/p00-freeze/tools.json`, `spikes/p00-freeze/tools.md` (written by W1 in parallel; this draft does not restate their content) |
| Crate freeze | `spikes/p00-freeze/crates.json`, `spikes/p00-freeze/crates.md` |
| Freeze consistency checker (Opus, per request §1) | `spikes/p00-freeze/check_freeze.py` |
| Install record with trusted hashes | `evidence/P00/P00-T02/install-record.md` |
| Scanner contract sheets | `evidence/P00/P00-T09/contract-yamllint.md`, `contract-gitleaks.md`, `contract-kubeconform.md`, `contract-actionlint.md`, `contract-checkov.md` (revision 1, rejected for invocation), `contract-checkov-rev2.md` (current); related `evidence/debug/P00-T09-D01/contract-checkov_r2.md` |
| Contracts amended in P00 | `schemas/workbench-store-v1.3.schema.json`, `schemas/wrapper-control-v1.1.schema.json`, `schemas/wrapper-request-v1.1.schema.json`; `evidence/P00/CA-02/`, `evidence/P00/CA-02-versioning/` |
| Schema-cache plan and prototype | `spikes/P00-T10-schema-cache-plan.md`, `spikes/t10-cache/` |
| Advisory exception (spike-only) | `evidence/P00/P00-T07/advisory-exception.md` |
| Follow-up requirements register | `plan.md` §5.2 |

## 6. Follow-up requirements FR-01 to FR-30 by target phase (`plan.md` §5.2)

Assignments are as recorded in the register (Opus proposals where the register says so; the owner may reassign). Spike passes are not evidence that the production implementation meets these.

| Target | ID | Gist |
|---|---|---|
| CA-02 (done) | FR-04 | Descendant cleanup recorded separately from the primary termination reason; `not_applicable` stream state for spawn failures |
| P02 | FR-15 (with P08) | `format` assertions enabled; external `$ref` retrieval disabled; unavailable schema is `schema_unavailable`, never valid |
| P02 | FR-16 (J11 to J13, J16; with P08) | Current-contract fixtures and the differential comparison carried into production suites |
| P03 | FR-18 | Blocking credential-store operations run in a bounded, separately terminable helper (if OS-store mode is adopted) |
| P03 | FR-19 | Bounded private IPC; secrets never in argv, routine environment, logs or durable files |
| P03 | FR-20 | Full parse and validation of the bus address; reject executable, autolaunch and unapproved transports; explicit address-source policy |
| P03 | FR-21 | Distinguish unavailable, missing, locked/denied, timeout and internal failure; no plaintext fallback |
| P03 | FR-22 | Timed-out write or delete may have completed; no rollback claim and no blind retry |
| P03 | FR-23 | Prompt and service effects of killing a helper are an adoption test |
| P03 | FR-24 | Re-screen the backend path before P03 adoption and before release; exception limited to T07 |
| P06 | FR-01 (mandatory) | Output budgets enforced at the reader boundary with immediate coordinator notification; measurable targets proposed; regression RC-FLOOD |
| P06 | FR-02 (decision) | Decide and verify process-group/session, orphan reaping and containment outside a PID-1 namespace |
| P06 | FR-03 | Add ANSI and early stdout closure cases to the verification matrix |
| P06 | FR-29 (with FR-02) | Process-group cleanup does not contain `setsid` escapees; bounded harness return is not proof of containment |
| P07 | FR-05 (regress in P08) | O(n) position mapping per input |
| P07 | FR-06 (also P08, P10) | Byte offset / 1-based line / 1-based Unicode-scalar column convention |
| P07 | FR-07 | At most one leading BOM removed, recorded, original-byte provenance kept |
| P07 | FR-08 (also P10, P11, P15) | No invented end spans or display-width column claims |
| P07 | FR-09 | Duplicate keys detected before lossy map conversion; `1` vs `"1"` not duplicates |
| P07 | FR-10 (also P08) | Explicit `unsupported_for_validation` projection policies; nothing silently coerced |
| P05 / P07 | FR-11 | Per-input resource budgets designed in P05 and enforced in P07; production defaults are a later owner decision |
| P07 | FR-12 | Composer enforces depth for block and flow nesting |
| P07 | FR-13 | Parser-imposed and composer-imposed limits classified separately |
| P07 (and any parser change) | FR-14 | Recursion-error mapping pinned to the exact parser version; dependency upgrades need approval and re-verification |
| P07 | FR-17 | Repeated or internal U+FEFF remains content and is recorded |
| P08 | FR-25 | Never rely on gitleaks exit 0 alone; validate target; repository ignore/suppression is a coverage issue; no debug output; output guard before persistence |
| P09 | FR-27 | Checkov only via contract revision 2 (private cwd, app-owned config, staged `-f`, no downloads); empty output is not "clean" |
| P13 | FR-26 (also T10) | Treat kind/apiVersion-derived schema paths as untrusted; classify kubeconform coverage from parsed per-resource status |
| P13 | FR-28 | ShellCheck/Pyflakes integration stays unverified; disable explicitly; no capability claim |
| P13 (mandatory) | FR-30 | Complete API group/version/kind matching; unknown or ambiguous identities are unsupported, never fall back to a similarly named built-in schema |

Phase assignments for FR-05 to FR-17 are Opus proposals from the §4 scopes (`plan.md` §5.2). Proposed P12 follow-ups from the T08 acceptance (artifact verification including the empty-`/Annots` rule, quarantine of not-written PDFs, visible code-point degradation, visual inspection before release, subsetting and shaping/RTL decisions) are recorded in `evidence/P00/P00-T08/record.md` and are **not** registered as FRs.

## 7. Open owner decisions

| Item | Detail (source) |
|---|---|
| O-1 | Schema origin (plan proposes origin A pinned, B as fallback; `spikes/P00-T10-schema-cache-plan.md` §11) |
| O-2 | Target Kubernetes version(s) (proposal 1.36 only) |
| O-3 | Staleness threshold and whether stale warns or blocks (no default proposed) |
| O-4 | Use of the datreeio CRDs catalogue (proposal: off by default) |
| O-5 | Redistribution and bundling, NOTICE review (proposal: not bundled) |
| O-6 | Approval of the first `schema_fetch` and revision pin (separate approval) |
| B-08 / Q-01 / Q-05 | Schedule does not fit four weeks at 5 sessions/week; sessions per week and scope exchanges E1 to E6 undecided (`plan.md` §7.3, §7.4, §8) |
| Q-02, Q-04 | AI narration deferral; production model ID |
| D-15 form | Plain PRD §16 edit versus formal amendment (`status.md`, "Next permitted action") |
| H-3 / output-guard scope | The guard covers registered synthetic fixtures only; production redaction is not established (`status.md` T09 row) |
| K-A exception | **Decided at P00 closure (owner, 2026-10-24): historical/inactive.** Valid only for the completed T07 experiment and its original lockfile and scope; not applicable to the current frozen lockfile; no further keyring build or adoption authorised; a future build needs a new decision and re-screen (FR-24) (`evidence/P00/P00-T07/advisory-exception.md`) |
| Persistent OS credential storage (K-D) | Not chosen; session-only mode required; decision pending (`evidence/P00/P00-T07/advisory-exception.md`) |
| T08 open choices | Subsetting/shaping amendment candidates, page-image rendering tool for visual checks, font licensing (P20), reclassification re-run for U1 Korean (not approved) (`evidence/P00/P00-T08/record.md`) |
| FR-11 defaults, FR-01 targets | Production limit values and the proposed measurable FR-01 targets await owner confirmation at the relevant phase (`plan.md` §5.2) |
| Not approved (status.md "Next permitted action") | Schema downloads, cache implementation, interface repair, worktree deletion, further repair, tool upgrades, dependency changes, merge or push to `main` |

## 8. Risks and limitations carried forward

| Item | Source |
|---|---|
| No filesystem sandbox: build scripts, proc-macros, daemons and scanners ran with the user's file permissions; `env -i`, namespaces and isolated Cargo directories are not sandboxing | `evidence/P00/P00-T05-D3-1/record.md`, `evidence/P00/P00-T07/record.md`, `evidence/P00/P00-T08/record.md` |
| Dependency review is pattern-based (grep), published advisories only, absence of an advisory is not evidence of safety; cargo-audit not yet run (P01, R-4) | T03, T06 to T08 records; `plan.md` §8 |
| Integrity limits: crates.io checksum-only; Gitleaks and kubeconform without upstream signatures; PyPI provenance not verified | `evidence/P00/P00-T02/install-record.md`, `status.md` |
| Spike evidence covers synthetic fixtures, one host, one Git version, pinned tool versions; no production adapter or helper exists | T04, T09 records, `status.md` |
| Keyring: unresponsive bus blocks indefinitely; real desktop prompter, PAM login sessions, KWallet/KeePassXC untested; spike-only maintenance exception | `evidence/P00/P00-T07/record.md` |
| PDF: visual correctness unverified; no shaping/bidi; whole-font embedding; WeasyPrint suspended on an owner-reported advisory not verified by Opus | `evidence/P00/P00-T08/record.md`, `status.md` |
| Process supervision: production output-limit enforcement and escaped-descendant containment unverified | `evidence/P00/P00-T05/record.md`, FR-01 to FR-03, FR-29 |
| T10: prototype has the R-1 collision defect (not repaired), non-atomic checks, synthetic data, no signatures, kubeconform not run | `evidence/P00/P00-T10/record.md` |
| B-04: worker models are configured, not verified; worker self-reports are not evidence; Opus wrote the T08 harness before the worker definition loaded (disclosed) | `plan.md` §8, `status.md`, `evidence/P00/P00-T08/record.md` |
| B-05 Trivy reports `Version: dev` (X4 only); B-06 headless Secret Service; B-07 tools that cannot neutralise repository config would be disabled | `plan.md` §8 |
| Disclosed scope deviations kept on record: `dual_stream.sh` run once outside the namespaces (T05); `dbus-send` unlisted (T07); one unsanctioned heredoc edit by a T10 worker | T05, T07, T10 records |
| Continuity files are written by Opus directly (no Haiku documentation agent) | `status.md` |

## 9. Estimate note

`plan.md` §7.1 estimates P00 at 38 to 61 hands-on hours (T11 estimated 2 to 4 hours in §5), with low confidence until P01 and P07 provide measured velocity. **No actual hours were measured or recorded for P00 in any repository record**, so no comparison against the estimate can be made here.

## 10. Closure checklist (each item needs the owner's explicit approval)

| # | Step | State |
|---|---|---|
| 1 | Owner acceptance of T11 (this freeze and report) | Done 2026-10-24 (qualified) |
| 2 | Owner acceptance of P00 as a whole | Done 2026-10-24 (qualified; section 11) |
| 3 | Approval to clean up retained worker worktrees and branches (`.claude/worktrees/agent-*`) | Pending; deletion not approved (`status.md`) |
| 4 | Commit of the T11 deliverables | Pending (owner: commit and push after approval; `spikes/P00-T11-request.md`) |
| 5 | Merge of `phase/P00-decisions-spikes` to `main` | Pending; not part of T11 |
| 6 | Push to `main` | Pending; not part of T11 |

No automatic progression to P01 (`CLAUDE.md` phase loop).

## 11. P00 closure (owner-approved closure pass, 2026-10-24)
**Reconciliation, from existing records only:**
- **T01–T11:** acceptance states as in section 2. T03 is ratified as provisional where not exercised (closure decision, not historical acceptance). T11 is accepted.
- **T09 acceptance chain** (complete, in order): run1 (`a2d8488`), then D01 Checkov revision 2 (`0cd446f`), then D02 with the H-3 validation (self-test 29/29, fresh gitleaks run identical to run1) and the D01 marker-attribution resolution (`de4cbf0`), then the owner's acceptance (`ea50cb7`), Checkov revision 2 only.
- **FR-01–FR-30:** every requirement has a destination phase in `plan.md` §5.2. FR-04 is done (CA-02). FR-30 is a mandatory P13 acceptance requirement.
- **Open decisions:** no record marks any open item as a P00 blocker. Each is assigned to a later phase:
  - Q-01/Q-05/B-08: schedule and pilot cut;
  - Q-02: P11/P17;
  - Q-04: real-API acceptance;
  - O-1–O-6: P13 and schema approval;
  - D-15 form: documentation formality, PRD §16 already records D-15;
  - B-04: model verification, process risk;
  - B-01: conditional tools and T08 renderer candidates.
- **B-02 and B-06/B-07** were reconciled from the records: resolved for P00, open for P01 / later as stated in `plan.md` §8.

**Result:**
- P00 is accepted with qualifications. Every limitation and later-phase requirement above is preserved.
- P01 is **ready for planning, not executing**: P01 execution, merge to `main` and push to `main` each need separate approval.

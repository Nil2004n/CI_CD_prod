# P02-T03 — Contract amendment CA-04 (draft, not applied)

Status: **draft by Opus 2026-10-27, for owner approval (P02 A10 = 10a: apply only after approval, before wave 3).** No specification file was edited. On approval, the lead applies the text below verbatim to the named sections, adds Data Schema §25 as the record, bumps the document version to 0.3.5 in all five headers, and runs a consistency check (`evidence/P02/CA-04/check.md`: every changed version string, every cross-reference, the store schema hash unchanged).

Basis: P01 B1 owner decision 2 ("event envelope (event_id/at/level) added by the coordinator at emission, specified in P02"); D-17 (`plan.md` §4.1, "Specification follow-up"); P02-T01 decisions T01-D9, T01-D10, T01-D12 (approved 2026-10-27).

## 1. Summary of changes
| # | Section | Change | Version effect |
|---|---|---|---|
| C1 | Architecture §6 (contract) | Every emitted event carries an envelope `event_id` (UUID v4), `at` (UTC RFC 3339, `Z` only) and `level` (`debug`/`info`/`warning`/`error`), added by the coordinator at emission. Envelope fields sit beside `event_type` in the serialised form. Emission rejects events whose payload contradicts the state machines (non-terminal `RunFinished`/`CheckCompleted`, illegal `RunStateChanged` pair, reserved events) | **app contract 1.0 → 1.1** (additive; 1.0 was published with the P01 merge) |
| C2 | Architecture §6 (persistence of events) | Must-deliver events are persisted to `log.json` as store `Event` records, except `Chat*` events (chat metadata stays out of the run log; Data §11.24). `message_redacted` = the event's own detail text if it has one, else a fixed per-event summary; `code` = failure code where the event has one | none (store `Event` def unchanged; store 1.4) |
| C3 | Architecture §3/§4 | The headless developer CLI is a second presentation adapter beside the TUI: same command bus/coordinator, same events, no direct Git/scanner/store/HTTP access. Commands without an Arch §6 equivalent are labelled `dev` and are excluded from release builds unless the owner decides otherwise at P20 | none |
| C4 | Technical Specification §4 | Workspace layout gains `cli/` (binary `pwt-workbench-cli`) | none |
| C5 | UI Schema §17 | CLI/TUI parity: the same scripted command sequence through the CLI and through the TUI reducer produces the same event sequence (test from P15a/P15b); the CLI never substitutes for manual TUI checks | none (UI schema 1.1 unchanged; no widget or state change) |
| C6 | Data Schema §17 version list, Architecture A-11, all document headers | app contract **1.1**; document **0.3.5** | as stated |

Unchanged: store contract **1.4** (no schema change; canonical file hash `909e2a55…0e2e`), wrapper protocol **1.1**, UI schema **1.1**.

## 2. Proposed text

**Architecture §6, contract heading:** `### Canonical application command/event contract (app contract v1.1, CA-01, CA-04)`. In the first paragraph, replace "(`app_contract_version` 1.0)" with "(`app_contract_version` 1.1; 1.0 → 1.1 by CA-04, additive)".

**Architecture §6, new paragraph after the event table:**
> Event envelope (CA-04). The coordinator adds three fields to every event at emission: `event_id` (UUID v4 from the operating system's random source; no fallback if it is unavailable), `at` (UTC RFC 3339, `Z` form) and `level` (`error` for `StorageFailure`, `OperationFailed`, `ChatFailed`; `warning` for `TriggerSkipped` and for `RunFinished` with `failed`, `partial` or `interrupted`; otherwise `info`). Serialised events carry the envelope fields beside `event_type`. Emission rejects an event whose payload contradicts the state machines of §7 (for example `RunFinished` or `CheckCompleted` with a non-terminal state, or a `RunStateChanged` pair §7 does not allow) and reserved events; such a rejection is a programming error, reported as `OperationFailed`. Must-deliver events except `Chat*` are persisted to `log.json` as Data Schema `Event` records (`message_redacted` = the event's detail text or a fixed per-event summary; `code` = its failure code or null). Coalescible and latest-value events and chat events are not persisted to the run log.

**Architecture §3, paragraph after the diagram:**
> A headless developer CLI (`pwt-workbench-cli`, D-17) is a second presentation adapter at the same level as the TUI: it turns arguments into the commands above, sends them through the same coordinator and prints the resulting events (human-readable, or one JSON event per line). It does not call Git, scanners, the store or HTTP itself. Commands with no §6 equivalent are labelled `dev`, call only domain/application entry points and are excluded from release builds unless the owner decides otherwise at P20.

**Architecture §4, table row added after "UI":**
> | CLI (developer, D-17) | Argument parsing, event printing, exit codes | Git/scanner/store/HTTP access, implicit approvals, default app-data location |

**Architecture §14, A-11:** "app command/event contract 1.0" → "app command/event contract 1.1 (CA-04)".

**Technical Specification §4, layout block:** add after `terminal-ui/`:
> `cli/             # headless developer CLI (D-17): same commands/events as the TUI; dev-only commands labelled`

**UI Schema §17, paragraph added after "Commands and events: …":**
> CLI parity (CA-04). The developer CLI uses the same Architecture §6 commands and events. From P15a/P15b, a parity test runs one scripted command sequence through the CLI and through the TUI reducer and requires the same event sequence. CLI scenarios do not replace snapshot, input, resize or manual terminal checks. Replace "(app contract v1.0)" with "(app contract v1.1)".

**Data Schema §17, first paragraph:** "the app command/event contract (Architecture §6) is v1.0" → "is v1.1 (CA-04, §25)".

**Data Schema §25 (new record):**
> ## 25. Contract amendment CA-04 (event envelope, developer CLI)
> Date: <application date>. Authority: owner approval of P02-T03 (P01 B1 decision 2; D-17). Store contract stays 1.4: the store `Event` definition already carries `event_id`, `at`, `level`, `event_type`, the five optional IDs, `message_redacted` and `code`; CA-04 defines how application events fill it (Architecture §6). App contract 1.0 → 1.1 (additive envelope fields on the serialised event; no command or event removed or renamed). Wrapper protocol 1.1 and UI schema 1.1 unchanged. Document 0.3.4 → 0.3.5. No stored data exists under app contract 1.0 (no persistence code before P02), so no migration applies.

**Headers (five documents):** `Version: 0.3.5 (amendments CA-01, CA-02, CA-03, CA-04; store contract 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1)`.

## 3. Code effects when applied (lead, before wave 3)
- `domain::contract::APP_CONTRACT_VERSION` "1.0" → "1.1"; the `domain/tests/versions.rs` expectation updated in the same commit (an expectation change, recorded, not a weakened assertion).
- No change to `AppEvent` variants or fields; the envelope is the `application::emit::EventEnvelope` of request §5.8.

## 4. Decision for the owner
| ID | Proposal | Recommended |
|---|---|---|
| CA-04 | Apply C1–C6 as drafted (app contract 1.1, document 0.3.5) | Approve; applied before wave 3 with its consistency check |
| CA-04-alt | Keep app contract 1.0 and treat the envelope as a transport detail | Not recommended: the serialised event shape (CLI `--json`, future TUI mocks) changes, so consumers need a version signal |

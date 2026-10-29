"""Applies contract amendment CA-04 (planning/P02-T03-CA-04-draft.md, owner-approved 2026-10-27) to the five
specifications. Every replacement must match exactly once; otherwise nothing is written. Run once from the repo root."""
import pathlib, sys
DATE = "2026-10-27"
HDR_OLD = "Version: 0.3.4 (amendments CA-01, CA-02, CA-03; store contract 1.4, wrapper protocol 1.1, UI schema 1.1)"
HDR_NEW = "Version: 0.3.5 (amendments CA-01, CA-02, CA-03, CA-04; store contract 1.4, wrapper protocol 1.1, app contract 1.1, UI schema 1.1)"
ENVELOPE = (
    "\nEvent envelope (CA-04). The coordinator adds three fields to every event at emission: `event_id` (UUID v4 from the "
    "operating system's random source; no fallback if it is unavailable), `at` (UTC RFC 3339, `Z` form) and `level` "
    "(`error` for `StorageFailure`, `OperationFailed`, `ChatFailed`; `warning` for `TriggerSkipped` and for `RunFinished` "
    "with `failed`, `partial` or `interrupted`; otherwise `info`). Serialised events carry the envelope fields beside "
    "`event_type`. Emission rejects an event whose payload contradicts the state machines of §7 (for example `RunFinished` "
    "or `CheckCompleted` with a non-terminal state, or a `RunStateChanged` pair §7 does not allow) and reserved events; such "
    "a rejection is a programming error, reported as `OperationFailed`. Must-deliver events except `Chat*` are persisted to "
    "`log.json` as Data Schema `Event` records (`message_redacted` = the event's detail text or a fixed per-event summary; "
    "`code` = its failure code or null). Coalescible and latest-value events and chat events are not persisted to the run log.\n")
CLI_ARCH3 = (
    "\n\nA headless developer CLI (`pwt-workbench-cli`, D-17, CA-04) is a second presentation adapter at the same level as "
    "the TUI: it turns arguments into the commands of §6, sends them through the same coordinator and prints the resulting "
    "events (human-readable, or one JSON event per line). It does not call Git, scanners, the store or HTTP itself. Commands "
    "with no §6 equivalent are labelled `dev`, call only domain/application entry points and are excluded from release "
    "builds unless the owner decides otherwise at P20.")
UI17 = (
    "\n\nCLI parity (CA-04). The developer CLI uses the same Architecture §6 commands and events. From P15a/P15b, a parity "
    "test runs one scripted command sequence through the CLI and through the TUI reducer and requires the same event "
    "sequence. CLI scenarios do not replace snapshot, input, resize or manual terminal checks.")
DATA25 = f"""

## 25. Contract amendment CA-04 (event envelope, developer CLI)

Date: {DATE}. Authority: owner approval of P02-T03 (`planning/P02-T03-CA-04-draft.md`; basis P01 B1 decision 2 and D-17). CA-01–CA-03 are unchanged.

| Item | Change |
|---|---|
| Event envelope | `event_id`, `at`, `level` added by the coordinator at emission and serialised beside `event_type` (Architecture §6) |
| Event persistence | Must-deliver events except `Chat*` become store `Event` records; `message_redacted` = detail text or fixed summary; `code` = failure code or null |
| Developer CLI | Second presentation adapter (Architecture §3/§4, Technical Specification §4, UI Schema §17); `dev` commands excluded from release builds unless decided at P20 |

Version handling: app contract 1.0 → **1.1** (additive envelope fields on the serialised event; no command or event removed or renamed). Store contract stays **1.4**: the store `Event` definition already carries `event_id`, `at`, `level`, `event_type`, the five optional IDs, `message_redacted` and `code`; CA-04 defines how application events fill it. Wrapper protocol 1.1 and UI schema 1.1 unchanged; canonical store schema file unchanged. Document 0.3.4 → 0.3.5. No data was stored under app contract 1.0 (no persistence code before P02), so no migration applies.
"""
EDITS = {
    "01_PRD.md": [(HDR_OLD, HDR_NEW)],
    "02_Technical_Specification.md": [
        (HDR_OLD, HDR_NEW),
        ("    terminal-ui/     # Ratatui components, reducer, input routing\n",
         "    terminal-ui/     # Ratatui components, reducer, input routing\n"
         "    cli/             # headless developer CLI (D-17, CA-04): same commands/events as the TUI; dev-only commands labelled\n"),
    ],
    "03_Architecture.md": [
        (HDR_OLD, HDR_NEW),
        ("### Canonical application command/event contract (app contract v1.0, CA-01)",
         "### Canonical application command/event contract (app contract v1.1, CA-01, CA-04)"),
        ("(`app_contract_version` 1.0), independent", "(`app_contract_version` 1.1; 1.0 → 1.1 by CA-04, additive), independent"),
        ("No event includes raw secret values or raw child-process output. Progress can be coalesced; findings, permission decisions and terminal run states cannot be silently dropped.\n",
         "No event includes raw secret values or raw child-process output. Progress can be coalesced; findings, permission decisions and terminal run states cannot be silently dropped.\n" + ENVELOPE),
        ("merely drawing a gateway does not constrain their internal traffic.",
         "merely drawing a gateway does not constrain their internal traffic." + CLI_ARCH3),
        ("| UI | Focus/layout/input, projections of app state | Git execution, scanner business logic, direct data-file writes |\n",
         "| UI | Focus/layout/input, projections of app state | Git execution, scanner business logic, direct data-file writes |\n"
         "| CLI (developer, D-17) | Argument parsing, event printing, exit codes | Git/scanner/store/HTTP access, implicit approvals, default app-data location |\n"),
        ("app command/event contract 1.0, UI schema 1.1 (CA-03)", "app command/event contract 1.1 (CA-04), UI schema 1.1 (CA-03)"),
    ],
    "04_UI_Schema.md": [
        (HDR_OLD, HDR_NEW),
        ("wrapper protocol 1.1 and app command/event contract 1.0 (CA-01", "wrapper protocol 1.1 and app command/event contract 1.1 (CA-04; CA-01"),
        ('"Canonical application command/event contract" (app contract v1.0).', '"Canonical application command/event contract" (app contract v1.1).'),
        ("Widgets do not invoke shell/Git/scanners/OpenRouter or write authoritative JSON. Extend DTOs only through reviewed contract changes.",
         "Widgets do not invoke shell/Git/scanners/OpenRouter or write authoritative JSON. Extend DTOs only through reviewed contract changes." + UI17),
    ],
    "05_Database_Data_Schema.md": [
        (HDR_OLD, HDR_NEW),
        ("This document is v0.3.4; machine store schema is v1.4; wrapper control protocol is v1.1; the app command/event contract (Architecture §6) is v1.0;",
         "This document is v0.3.5; machine store schema is v1.4; wrapper control protocol is v1.1; the app command/event contract (Architecture §6) is v1.1 (CA-04, §25);"),
    ],
}
texts = {}
for f, edits in EDITS.items():
    t = pathlib.Path(f).read_text()
    for old, new in edits:
        n = t.count(old)
        if n != 1:
            sys.exit(f"{f}: expected 1 match, found {n}: {old[:80]!r}")
        t = t.replace(old, new)
    texts[f] = t
texts["05_Database_Data_Schema.md"] = texts["05_Database_Data_Schema.md"].rstrip("\n") + "\n" + DATA25
for f, t in texts.items():
    pathlib.Path(f).write_text(t)
print("applied", sum(len(e) for e in EDITS.values()), "replacements + Data §25")

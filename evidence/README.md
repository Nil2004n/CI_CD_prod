# Development evidence

Development-control evidence only (code changes and verification). Not end-user scan data, not runtime `approvals.json`.

## Layout

```text
evidence/
  P00-planning/   # documentation-only planning stage (2026-10-23)
  P00/            # P00 spike records, once P00 is approved
  Pxx/            # one directory per approved feature phase
  debug/Pxx-Dnn/  # one directory per owner-approved debug cycle
```

## Check record format

Each check record states: command, cwd, relevant environment (tool versions, offline/network setting), repository revision and dirty state, exit code, pass/fail/skip counts, evidence file paths, manual-pending checks and known limitations. Long logs are archived here, not in root continuity files.

## Rules

- No API keys, credentials, raw secrets (including seeded fixture secrets in raw form), client repository content or raw provider payloads.
- Never record a test, approval or commit that did not happen.
- Helper scripts stored here are Opus-authored planning aids; read before running, and run only within an approved phase.

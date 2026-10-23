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

## Command exit capture (owner instruction 2026-10-24)

Capture the producer command's own exit status immediately, with no pipe in between, then inspect or tail its log separately:

```sh
cmd ... > "$LOG" 2>&1; rc=$?; echo "exit=$rc"; tail -n 20 "$LOG"
```

Never report an exit code from a pipeline (`cmd | tail` reports `tail`'s status; zsh has no `PIPESTATUS`). Corrected past cases stay in their records with the original mistake visible.

## Trusted-input re-hash

Before executing a protected tool input, recompute its SHA-256 and compare with the trusted value recorded in `evidence/P00/P00-T02/install-record.md` ("Trusted hashes"); on mismatch, do not execute and report. This detects changes since recording; it is not complete protection against host tampering.

## Rules

- No API keys, credentials, raw secrets (including seeded fixture secrets in raw form), client repository content or raw provider payloads.
- Never record a test, approval or commit that did not happen.
- Helper scripts stored here are Opus-authored planning aids; read before running, and run only within an approved phase.

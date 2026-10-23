# P00-T01 document check

| Field | Value |
|---|---|
| Date | 2026-10-24 |
| Command | `python3 -I evidence/P00/P00-T01/check_t01_docs.py "$PWD"` |
| cwd | repository root |
| Environment | Python 3.14.7 standard library only; no network |
| Revision | `1e4a5b3` + uncommitted `spikes/`, `evidence/P00/P00-T01/`, `status.md`, `handover.md`, `evidence/P00/README.md` |
| Exit | 0 (final run) |
| Counts | pass 10, fail 0, skip 0 |
| Output | `check-output.txt` |

Checks: fixture catalogue defines IDs; every fixture reference in the protocols resolves (wildcards by prefix); references inside the catalogue resolve; every fixture belongs to a coverage-map area; no PEM private-key header, GitHub-/AWS-/Slack-style token, long mixed high-entropy string or credential-bearing URL in either document.

Run history (not hidden):
1. First run: 7 pass / 3 fail. (a) The protocols' reference "FX-SEC rules" pointed to no fixture: a wording defect, fixed to cite fixture specification §1. (b) `FX-PROC-EXIT-n` was not captured by the checker's ID pattern: renamed to `FX-PROC-EXIT-CODES`. (c) The "long high-entropy run" pattern matched hyphenated prose (`repository-controlled-…`): a checker false positive. The pattern was narrowed to require at least one digit and one uppercase letter. **This narrows a check after it failed; the hit was inspected and confirmed to be prose.**
2. Second run: 9 pass / 1 fail. A second "FX-SEC rules" reference remained; fixed.
3. Final run: 10 pass / 0 fail.

Limitations: structural and pattern checks only. They do not show that the protocols are complete or correct; tool behaviour claims in the protocols remain unverified candidates until the T04–T09 spikes. Not an application test.

# CA-04 — application record and consistency check

Date: 2026-10-27. Authority: owner approval "I approve T04 and CA-04, proceed" (2026-10-27) of `planning/P02-T03-CA-04-draft.md`. Applied by Opus (specs are lead-owned).

| Step | Command (repo root) | Exit | Evidence |
|---|---|---|---|
| Schema hashes before | `sha256sum schemas/*.json` | 0 | `schemas-before.sha256` |
| Apply | `python3 -I -B evidence/P02/CA-04/apply_ca04.py` (16 exact-once replacements + Data §25; aborts without writing if any match count ≠ 1) | 0 | `apply_ca04.py` |
| Check | `python3 -I -B evidence/P02/CA-04/check_ca04.py` | 0 — **27/0/0** | `check.txt` |
| Negative control | same check on scratch copies with the PRD header reverted to 0.3.4 | 1 (2 FAIL), as expected | `check-negative-control.txt` |

Checked: all five headers `0.3.5 … app contract 1.1`; Arch §6 heading/paragraph/envelope text, §3 CLI paragraph, §4 CLI row, A-11; Tech §4 `cli/`; UI §17 parity paragraph and v1.1 reference; Data §17 version sentence and §25 record; every remaining "app contract 1.0" mention is a historical statement (3: Arch §6 ChatFailed note, Data §24 CA-03 note, Data §25 itself); the three schema files byte-unchanged (store schema `909e2a55…`).

Not changed: `README.md` (describes the original v0.3 package and is already historical: it names v0.3.3 / app contract 1.0); `plan.md` references are updated separately. Code effect (`APP_CONTRACT_VERSION` "1.1" and its test expectation) is applied with the P02-T04 scaffold.

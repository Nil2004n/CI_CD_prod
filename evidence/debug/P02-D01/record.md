# P02-D01 — FR-31 classification gap: `RendererSetting.parameters_redacted`

Opened: 2026-10-27, by Opus, after wave-1a task P02-T05a (W1). Authority: owner pre-authorisation of debug cycles for anomalies the lead finds after a worker task (P02 A9 = 9a; bounds `planning/P02-T01-request.md` §11). This record is attempt 1 of at most 2.

## Confirmed defect (evidence)
- **Lead defect, Appendix A revision 1.** The generator counted `type: string`, nullable strings and arrays of strings as plain-string properties. It did not count string-valued maps (`type: object, additionalProperties: {type: string}`).
- **What was missed.** A lead scan of all 57 reachable `$defs` found exactly one such property: `RendererSetting.parameters_redacted`. No other map-valued or open-object property exists.
- **How it surfaced.** W1 reported it as interface question 1 in the T05a report. W1 typed it provisionally as `BTreeMap<Identifier, SanitisedText>` (class S) and did not stop. The packet said to STOP when an R field appears; the field had no Appendix A row, so it was not formally an R field, but stopping was the expected handling. This is recorded as a process note, not a defect.
- **Correct class: R.** The parameters are authored by the workflow owner, and Data §11.11 says renderer parameters "cannot … contain raw secrets in persisted state". By the Appendix A rule of thumb (user-originated text → R), the values must be `RedactedText`. Class S would let user text count as application-derived text.
- **Hypotheses:** none open. The scope is fully established by the schema scan.

## Bounded fix
| Part | Owner | Change |
|---|---|---|
| L1 | lead | `dev/p02_fr31_appendix.py`, revision 2 of the generator, now in the repository: covers string-map values; `parameters_redacted` → R. Appendix A regenerated: **105** properties (E 6, N 66, R 27, S 6); exit 0 |
| L2 | lead | `domain/src/text.rs`: `pub(crate) fn store_read::redacted_map<K>` → `BTreeMap<K, RedactedText>`, re-escaping values. Additive and `pub(crate)`, the same kind of change as the approved §5.7 helpers. `clippy -p domain` PASS (`lead-clippy/`) |
| W | fresh `sonnet-implementer`, W1 scope | `domain/src/store/plan.rs`: `RendererSetting.parameters_redacted` becomes `BTreeMap<Identifier, RedactedText>` with `#[serde(deserialize_with = "crate::text::store_read::redacted_map")]`. `tests/store_plan.rs`: existing cases kept; add (a) an embedded ESC in a parameter value reads back as `<U+001B>`, (b) an invalid key (control character) is rejected without echo, (c) round trip of a clean map |

**Not covered here** (later work): T13's completeness test must also enumerate string-map values. That requirement is now in Appendix A and will go into the T13 packet.

## Retest (named)
1. The lead runs `dev/p02_check.py all` on the D01 worker branch.
2. The lead runs `dev/p02_check.py all` on the integrated phase branch (B1a candidate).
3. `dev/p02_fr31_appendix.py` exits 0 with 105 rows.

## Result
(filled in after the retest)

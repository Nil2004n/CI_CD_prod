# P01-D04 — guards from the final reviews (D04a text scope, D04b contract scope)

Date: 2026-10-26.
- **Authority:** owner pre-authorisation (`planning/P01-T01-request.md` §9).
- **Source:** lead-triaged findings of the final read-only `reviewer` and `security-reviewer` (P01-T09).
- **Base:** checkpoint `26157b4`.
- **Workers:** two **fresh** `sonnet-implementer` instances (configured `claude-sonnet-5-5`, unverified). They took over the W2 and W1′ file scopes, because the original worktrees had been removed after integration. They ran in parallel worktrees with disjoint files.

| Part | Findings | Commits | Change |
|---|---|---|---|
| D04a (`text.rs` doc lines, `tests/text.rs`) | rev M1 (construction routes), rev L1 (over-escaping only sampled) | `e5d7ba5`, `9c47fcd` | 7 control + compile_fail pairs: `RedactedText` from `SanitisedText` (`From`, `Into`), `Default`, `FromStr`, `TryFrom`; `SanitisedText` `Default`, `FromStr`. Exhaustive test over all Unicode scalar values with an independent literal oracle (82 escaped) |
| D04a follow-up | lead mechanical pair check: the "deserialising" pair (from the **lead's own P01-D02 packet**) differed in two lines | `9c47fcd` | One `///` line added to the compile_fail case. Now every pair differs in exactly one line |
| D04b (`contract.rs` doc lines, `tests/contract.rs`, new `tests/versions.rs`) | sec L5, rev M1, rev L2, rev L8 | `6694b34` | Both weak controls fixed; 8 new pairs (`CredentialSecret` Display/PartialEq/Hash; `RawInput` Clone/PartialEq/Hash; `AppCommand` Deserialize; `SanitisedText` in a `RedactedText` event field); private intra-doc link removed; version constants tested and cross-checked against every `schema_version`/`protocol_version` const in both schema files |

## Lead verification
- **Doc-only check:** for both parts, `git diff 26157b4..` shows 0 non-`///` changed lines in `text.rs` and `contract.rs`.
- **Mechanical pair check:** all compile_fail pairs now differ from their control in exactly one line (text.rs 15, contract.rs 16, ids.rs 1 = 32).

| Run | Evidence | Result |
|---|---|---|
| D04a branch | `evidence/P01/T09/D04a/lead-verify` | 394/0/0, 24 compile_fail, PASS |
| D04b branch | `evidence/P01/T09/D04b/lead-verify` | 400/0/0, 25 compile_fail, PASS |
| Integrated | `evidence/P01/T09/check-run3` (`--final`, baseline floors 415/32) | **415/0/0, 32 compile_fail, PASS** |

Integrated per-suite counts: unit 3, contract 22, ids_time 279, states 23, text 19, versions 5, doctests 64 (32 run, 32 compile_fail).

Cycle count: 1 for each part (the D04a follow-up is part of the same cycle). Closed.

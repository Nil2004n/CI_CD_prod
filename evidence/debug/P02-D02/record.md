# P02-D02 — wave-1a review findings within the debug pre-authorisation

Opened 2026-10-27 by Opus after the read-only `reviewer` and `security-reviewer` runs over wave 1a (HEAD `4153f78`). Authority: P02 A9 = 9a. That pre-authorisation covers test additions and fixes that change **no frozen interface, rule, criterion, dependency or scope**. Findings outside it go to the owner (see "Not in this cycle"). Attempt 1 of 2.

Both reviews reported **no high-severity finding**. Reviewer: 5 medium, 10 low. Security reviewer: 2 medium, 10 low. Both full reports are in the session transcript; the triage below covers every item.

## In this cycle
| ID | Source | Confirmed defect | Fix | Owner |
|---|---|---|---|---|
| D02-1 | sec L7, rev L1 | `store_read` helpers and `ConstFalse`, `SchemaVersion` and the `*StoreType` markers decoded through `String`/`bool::deserialize`, so serde_json quoted a wrong-typed value (``invalid type: integer `123` ``) | No-echo visitors (`deserialize_any`, fixed messages). `mod.rs` reuses `values::deserialize_parsed` | lead (done) |
| D02-2 | (found by lead test) | serde_json also quotes a scalar given where a **container** is expected, for every derived struct/`Vec`/map in the crate. Field helpers cannot prevent this | The guarantee belongs at the read boundary: `persistence::read` must never forward serde messages (frozen §5.4 `TypedDecode` carries none). Added as a **mandatory T08 test**: planted wrong-typed values at struct, array, map and enum positions must not appear in any `Problem`. Unit test `container_type_mismatch_is_rejected` records the boundary | lead (T08 packet) |
| D02-3 | rev M2 | The D01 tests would pass with `SanitisedText`; nothing pins the field type | Compile-time type assertion `let _: &BTreeMap<Identifier, RedactedText> = &…` | worker |
| D02-4 | rev L1 | `ConstTrue` (`provider.rs`) quotes wrong-typed input | No-echo visitor, plus test | worker |
| D02-5 | rev L9 | Byte bounds are tested with ASCII only | Multibyte tests: an N-byte bound rejects a string of fewer than N chars but more than N bytes, for `RelPath` (non-ASCII allowed) and the bound-equal case; ASCII-only types reject non-ASCII | worker |
| D02-6 | rev M1 | Typed layer decodes an absent required-nullable key as `None` (serde default for `Option`); §5.1 says required keys are never skipped | **Lead decision:** the schema layer (`required`) is the guard, so the typed layer is not required to reject; no `#[serde(default)]` is introduced. The worker adds a test pinning the current typed behaviour for one `FileRecord` key; T08 adds the schema-layer negative | worker + T08 |
| D02-7 | rev M3 | `FsOps` deviations not recorded | Recorded as **D-T04-4** in the B1a record: `step` parameter, `FileInfo`, `canonicalize`/`random_suffix`/`open_lock_file`, `remove_file` not injectable, `MAX_REPORT_NAME_BYTES` 200 | lead |
| D02-8 | rev L5, sec info | `.prev` holds the current generation after a failed `Replace` (N-1 lost) | Documented in the B1a record as a property of the §6.3 step order | lead |
| D02-9 | sec L3, rev L4, rev L6, rev L7 | `is_temp_file_name` matches any stem; cleanup failures are silent; `read` follows symlinks and FIFOs | **T10 packet requirements:** stem must be a known store name (± `.prev`) or a validated report name; scan only the root, `rag/` and `backup/`; regular-file check before remove; count removals; `symlink_metadata` regular-file check before any `read` | lead (T10 packet) |
| D02-10 | rev L10, L8 | `ConstTrue` outside the frozen list; `workflow_revision` minimum 1 not listed | Acknowledged in the B1a record (additive, owned file); schema layer covers the minimum | lead |

## Not in this cycle → owner decisions (B1a stop)
Each would change a frozen §5.2/§5.5 rule or contract, or add a dependency:
- sec M1: `CredentialRef` heuristic;
- sec M2 / rev M4: `DataDir` ownership and mode;
- sec L1/L2: `O_NOFOLLOW` (needs `libc`);
- sec L5 / rev M5: leading `-`;
- sec L4: host canonical form;
- sec L6: ssh user bound;
- sec L10: percent-encoded controls;
- sec L8: the P01 escape set;
- rev L2: SPDX degenerate values;
- sec L9: wildcard cap;
- rev L3: token values that look like paths or options.

## Retest (named)
1. The lead runs `all` after D02-1: **PASS** (run 4, `evidence/P02/wave1a/lead-d02-run4/`): 492 / 0 / 0, 32 compile_fail. Runs 1–3 failed on the lead's own new code and are kept, not removed:
   - run 1: a literal U+202E/U+2028 in a test string (rustc rejects bidi characters in literals), plus formatting;
   - run 2: the container-level echo, D02-2;
   - run 3: formatting.
2. The lead runs `all` on the worker branch.
3. The lead runs `all` on the integrated branch (B1a check).

## Result
(filled in after the retest)

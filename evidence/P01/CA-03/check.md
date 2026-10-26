# CA-03 contract check (store 1.4 / protocol 1.1 / UI schema 1.1; canonical tool availability)

Authority: owner approval 2026-10-26 of the P01 conformance-check recommendation, applied before the P01 merge. These are document and schema checks; application-code verification is recorded separately (`evidence/P01/CA-03/record.md`).

| Field | Value |
|---|---|
| Apply | `git mv schemas/workbench-store-v1.3.schema.json schemas/workbench-store-v1.4.schema.json`, then `python3 -I -B evidence/P01/CA-03/apply_ca03.py "$PWD"`: exit 0 ("store consts updated=10 availability enum=[ready, missing, incompatible, unconfigured, unavailable, deferred] store examples=3"; `apply-output.txt`). Prose edits (5 headers to 0.3.4, current-version references, UI §5/§14, Data §17/§19/§22-note/§24, plan D-16/FR-31/P02 row/FR-16) by a lead script with exact-match assertions |
| Check | `python3 -I -B evidence/P01/CA-03/check_ca03.py "$PWD" > evidence/P01/CA-03/check-output.txt 2>&1` |
| Environment | Python 3.14.7, jsonschema 4.26.0 (pre-installed), `format` checking on, no network |
| Final result | exit 0; **pass 29, fail 0, skip 0** (`check-output.txt`) |

## Run history (kept, not hidden)
1. **Run 1** (`check-output-1.txt`): exit 1, 28/1. The stale-reference scan listed 6 lines. Five of them were historical records: the CA-02 facts in `handover.md:36` and `status.md:16`, and the approved T01/P01 planning text (`P01-T01-request.md:145,206`, `P01-execution-plan.md:31`). They were annotated "Superseded 2026-10-26 by CA-03 …" and not rewritten. One was a current statement: the `status.md:33` Specs row, which was updated.
2. The check's allowlist gained exactly one pattern, matching only lines that carry that explicit annotation. This keeps the exemption narrow (the lesson from CA-02's loose first allowlist).
3. **Run 2:** 29/0/0.

## Checks
- the v1.4 file exists and v1.3 is absent;
- all three files are meta-valid;
- all 10 store roots have `schema_version` const 1.4, with no 1.3/1.2 const left;
- protocol stays at 1.1;
- availability equals the canonical list, accepts every canonical value and rejects `denied`;
- titles and `$comment` name CA-03; the wrapper `$defs` and the Data §7 copy are identical to the canonical file;
- the three §8–§10 examples are 1.4 and validate, and each is rejected when labelled 1.3; the §23 WrapperControl example still validates at 1.1;
- CA-01 and CA-02 fields are retained;
- UI `tools.states` equals the store list; `ui_schema_version` is "1.1";
- Data §24 is present; all five spec headers read 0.3.4, store 1.4, UI 1.1;
- no stale current-contract references remain in the root/planning documents;
- the historical CA-02 evidence is present and untouched.

Limitations: semantic rules are not schema-enforced. Historical evidence (`evidence/P00-planning/`, `evidence/P00/CA-02*`) references the old file names and is not expected to run against the current tree.

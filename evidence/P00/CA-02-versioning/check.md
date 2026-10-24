# CA-02 versioning check (store 1.3 / wrapper protocol 1.1)

Authority: owner decision on Q-03, 2026-10-24 (CA-02 becomes a versioned amendment). Document/schema checks only — separate from, and not evidence for, the P00-T04 Git-safety tests.

| Field | Value |
|---|---|
| Apply | `git mv` of the three schema files to `workbench-store-v1.3`, `wrapper-request-v1.1`, `wrapper-control-v1.1`; `python3 -I evidence/P00/CA-02-versioning/apply_versions.py "$PWD"` → exit 0 ("store consts updated=10 protocol consts updated=2 store examples=3 §23 example=1") |
| Check | `python3 -I evidence/P00/CA-02-versioning/check_versions.py "$PWD" > evidence/P00/CA-02-versioning/check-output.txt 2>&1; rc=$?` |
| Environment | Python 3.14.7, jsonschema 4.26.0 (pre-installed), `format` checking on, no network |
| Final result | exit 0; pass 24, fail 0, skip 0 (`check-output.txt`) |

Run history (kept, not hidden):
1. Run 1: 24/0/0 — but review of the check found its stale-reference allowlist too broad (any line containing "CA-01" was exempt), which let `status.md:23` ("store 1.2, wrapper protocol 1.0") pass. Output preserved as `check-output-1-loose-allowlist.txt`. One assertion was also rewritten for clarity (same logic: no store `const 1.2` / protocol `const 1.0` left).
2. Run 2 (tightened allowlist): exit 1, 23 pass / 1 fail — `no stale current-contract version references in current docs (['status.md:23'])`. Output file was overwritten by run 3; the failing line is recorded here verbatim.
3. `status.md` Specs row updated to the 1.3/1.1 contract (a planned documentation update in this task, not a check change). Run 3: 24/0/0.

Checks: new file names present, old absent; meta-validity; all 10 store root types `schema_version` const 1.3; `WrapperRequest`/`WrapperControl` `protocol_version` const 1.1; no store const 1.2 / protocol const 1.0 left; titles; wrapper `$defs` and Data Schema §7 embedded copy identical to canonical; §8–§10 examples are 1.3 and validate, and the same examples labelled 1.2 are rejected; §23 WrapperControl example is 1.1 and validates, labelled 1.0 is rejected; a pre-CA-02-shaped record is rejected; CA-01 `allow_fallbacks` const retained; CA-02 required fields present; no stale current-contract references in current root/proposal/spike documents (allowlisted: transition lines with "→", the historical CA-01 text in §22 and its supersession note, the schema `$comment`, the §17 history sentence and §19 migration guidance); historical evidence files present.

Historical evidence not modified: `evidence/P00-planning/*` (CA-01 check, written against v1.2/v1.0 files) and `evidence/P00/CA-02/*` (CA-02 field check against the then-named files). Their scripts reference the old file names and are not expected to run against the current tree.

Limitations: no application code reads/writes these contracts yet; semantic rules (Data Schema §11, §18, §23) are not schema-enforced.

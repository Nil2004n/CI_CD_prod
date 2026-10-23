# CA-01 contract check (planning stage)

| Field | Value |
|---|---|
| Date | 2026-10-23 |
| Command | `python3 -I evidence/P00-planning/verify_schema.py "$PWD"` |
| cwd | repository root |
| Environment | Python 3.14.7 (host), jsonschema 4.26.0 (pre-installed system package; nothing installed), rfc3339-validator present so `date-time` format is checked; no network |
| Revision | `805ced4` + uncommitted planning changes (dirty tree) |
| Exit | 0 |
| Counts | pass 22, fail 0, skip 0 |
| Output | `contract-check-output.txt` |

Scope: Draft 2020-12 meta-validation of the three schema files; `$defs` identity of wrapper schemas and the Data Schema §7 embedded copy with the canonical store schema; validation of the three Data Schema examples (§8–§10); positive/negative cases for `allow_fallbacks`, the Technical Specification provider snippet, and the new WrapperRequest/WrapperControl fields.

Limitations: this verifies contract documents only. It is not an application test; no application code exists. The UI Schema YAML block was not machine-validated (no schema defines it). Synthetic test documents are minimal, not full coverage of the schema.

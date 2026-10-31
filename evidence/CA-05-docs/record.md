# CA-05 documentation change — check record (2026-10-31, revision 2)

Scope: documentation/design only. r1 at the owner's request; r2 after the owner accepted the CA-05 design direction with qualifications (not implementation or release acceptance) and asked for a documentation-only reconciliation. No application code, runtime configuration, dependency manifest, lock file, canonical store schema or assessment data changed. Base: `phase/P03-permissions-credentials` at `4d6c70f` (B2). Change contract: `planning/CA-05-automatic-evidence-analysis.md` r2; proposals: `planning/CA-05-decision-proposals.md`.

Commands (cwd = repo root, host Python 3 with host jsonschema 4.26.0, offline, nothing installed):

| Command | Result |
|---|---|
| `python3 -I -B planning/CA-05-draft/generate_draft.py` | Regenerates both draft files; rerun into a scratch comparison: byte-identical (reproducible) |
| `python3 -I -B evidence/CA-05-docs/check_draft.py` | exit 0, **OVERALL PASS** — draft `$defs` + store 1.4 `$defs` form a valid Draft 2020-12 schema; 3 attempt records, 2 manifests and 2 completion fragments validate (format checking on); D26-02–D26-07, D26-09, D26-11, D26-12 hold on the examples; no `$defs` name clash (`check_draft.out`) |
| Negative controls (scratch copies): unknown citation; `verification_status="executed"`; `status="passed"`; non-empty AI-only `observations`; second completed attempt in one logical analysis; wrong `ai_evidence_coverage`; recommendation tied to no finding/check | each exit 1 as intended (`negative_controls.out`) |
| `python3 -I -B evidence/CA-05-docs/check_docs.py` | exit 1, **OVERALL FAIL by design while baseline rows exist** — CA-05 regressions **0**; Data §7 embedded schema equals canonical 1.4; §26 JSON blocks parse and equal the draft examples; ANL-01–ANL-12 defined once, no stray IDs; **9 BASELINE rows** in `plan.md` P02 tables (§5B.2/§5B.3) that fail identically at `4d6c70f` — pre-existing, not repaired in this task (`check_docs.out`) |
| `git diff --quiet 4d6c70f -- schemas workbench dev` | no change; canonical store schema sha256 still `909e2a55…` |

Not done (by instruction): no Cargo/runner/scanner/model run, no validator installation, no production-schema change, no migration, no external-service writes. **Mermaid diagrams are unverified** (no renderer; syntax reviewed by reading only).

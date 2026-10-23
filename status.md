# Status

Updated: 2026-10-24 by Opus directly (Haiku documentation agent not defined/approved; this is not a Haiku draft).

| Item | State |
|---|---|
| Stage | P00 in progress — installation checkpoint accepted; P00-T03 (documentation-only crate request) completed, awaiting owner decisions C-1–C-6 |
| P00-T01 | **Accepted by owner (2026-10-24).** Deliverables: `spikes/P00-T01-spike-protocols.md`, `spikes/P00-T01-fixture-specifications.md` |
| P00-T02 | **Accepted by owner (2026-10-24) as installation planning only** (not acceptance of installs or tests). `spikes/P00-T02-installation-request.md`; lookups `evidence/P00/P00-T02/lookups.md`. Owner decisions: R-1 approve Request A (reuse Gitleaks if identical and verified); R-2 B1 standalone Python 3.12, isolated envs, no system Python changes; R-3 listed stable releases, no age rule; R-4 cargo-audit deferred to P01 (pending dependency, not a check); R-5 use upstream verification where available, ask before extra tooling |
| Installs | **Request A + B1 installed and verified; checkpoint accepted by owner with limitations (2026-10-24), not acceptance of scanner behaviour** (record: `evidence/P00/P00-T02/install-record.md`). Gitleaks 8.30.1 (reused `~/.local/bin/gitleaks`, byte-identical to verified official binary), actionlint 1.7.12, kubeconform 0.8.0, CPython 3.12.15+20261021, uv 0.13.5, yamllint 1.38.0 (venv, 3 pkgs), Checkov 3.3.30 (venv, 96 pkgs), all under `~/.local/share/pwt-dev-tools/`. Checksums match; SLSA attestations verified with official cosign 3.1.2 for actionlint, uv, CPython (negative controls fail as expected); Gitleaks/kubeconform have no upstream signatures (limitation); PyPI provenance not verified (limitation, V3 fallback proposed). Offline version checks exit 0. cargo-audit deferred to P01 (pending, not a check) |. **PyPI provenance exception accepted** for controlled development only (not provenance-verified; revisit before customer distribution/sensitive client use)
| P00-T03 | **Completed (documentation only); awaiting owner review.** `spikes/P00-T03-crate-request.md`: 15 Tier S spike candidates, Tier P baseline (info only); all versions/licences/MSRV unresolved pending lookups (C-1). Check 6/0/0 (`evidence/P00/P00-T03/check.md`). No crate downloaded, resolved or built |
| P00-T04 onward | Not approved, not started |
| Implementation | None. No application code, fixtures or generators exist |
| Application tests | None run; none passed by implication |
| Verification | P00-T02 digest cross-check: 8 of 8 request digests match the lookup log (in-session Python check whose console output was not saved to a file; not an install verification). P00-T01 document check 10/0/0 after two corrected runs (`evidence/P00/P00-T01/check.md`); planning CA-01 contract check 22/0/0 (`evidence/P00-planning/contract-check.md`). Both are document checks, not application tests |
| Approvals | 2026-10-23: planning documents, CA-01, commit + push of planning checkpoint. 2026-10-24: P00-T01; acceptance of P00-T01 and commit + push; P00-T02 and Q-06 read-only lookups. Granted 2026-10-24: P00-T02 acceptance; Request A; B1 provisioning with yamllint/Checkov in isolated envs; scoped local P00-T02 commit. Granted 2026-10-24 (later): V1 attestation verification, resume B1, local commit. Later 2026-10-24: installation checkpoint acceptance with limitations; PyPI provenance exception; push of `4c659eb`..`008e80c`; P00-T03 (documentation only). Not granted: cargo-audit (P01), crates, real API, scanner spikes, P00-T04/T05/T09, crate lookups/downloads/builds, V3 |
| Git | Branch `phase/P00-decisions-spikes` created from `008e80c` (= `main` = `origin/main`); phase-branch policy retained (`plan.md` §3.6); earlier direct `main` commits/pushes are recorded historical exceptions, conflict reported after the `008e80c` push. Uncommitted at time of writing: P00-T03 docs/evidence, README/record edits |
| Specs | v0.3.1 (CA-01); store 1.2, wrapper protocol 1.0, app contract 1.0, UI schema 1.0 |
| Agents | Proposals only (`proposals/agents/`); `.claude/` absent. Worker models unverified. P00-T01 written by Opus, no worker used |

## Blockers
B-01 resolved for P00 baseline tools (cargo-audit pending P01), B-02 crates, B-08 schedule fit. B-03 resolved for read-only lookups only. B-09 resolved: Checkov runs on standalone CPython 3.12.15 — see `plan.md` §8. Q-06 answered (read-only lookups approved). Q-01–Q-05 open.

## Next permitted action
None. Proposed: owner decides C-1–C-6 in the crate request (C-1 = read-only crates.io/RustSec lookups for Tier S). Also: phase-branch vs `main` policy confirmation. Exit codes are captured per `evidence/README.md`; protected inputs re-hashed before execution.

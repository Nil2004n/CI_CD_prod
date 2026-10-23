# Status

Updated: 2026-10-24 by Opus directly (Haiku documentation agent not defined/approved; this is not a Haiku draft).

| Item | State |
|---|---|
| Stage | P00 in progress — P00-T02 installs complete (Request A, B1); awaiting next-task approval |
| P00-T01 | **Accepted by owner (2026-10-24).** Deliverables: `spikes/P00-T01-spike-protocols.md`, `spikes/P00-T01-fixture-specifications.md` |
| P00-T02 | **Accepted by owner (2026-10-24) as installation planning only** (not acceptance of installs or tests). `spikes/P00-T02-installation-request.md`; lookups `evidence/P00/P00-T02/lookups.md`. Owner decisions: R-1 approve Request A (reuse Gitleaks if identical and verified); R-2 B1 standalone Python 3.12, isolated envs, no system Python changes; R-3 listed stable releases, no age rule; R-4 cargo-audit deferred to P01 (pending dependency, not a check); R-5 use upstream verification where available, ask before extra tooling |
| Installs | **Request A + B1 installed and verified** (record: `evidence/P00/P00-T02/install-record.md`). Gitleaks 8.30.1 (reused `~/.local/bin/gitleaks`, byte-identical to verified official binary), actionlint 1.7.12, kubeconform 0.8.0, CPython 3.12.15+20261021, uv 0.13.5, yamllint 1.38.0 (venv, 3 pkgs), Checkov 3.3.30 (venv, 96 pkgs), all under `~/.local/share/pwt-dev-tools/`. Checksums match; SLSA attestations verified with official cosign 3.1.2 for actionlint, uv, CPython (negative controls fail as expected); Gitleaks/kubeconform have no upstream signatures (limitation); PyPI provenance not verified (limitation, V3 fallback proposed). Offline version checks exit 0. cargo-audit deferred to P01 (pending, not a check) |
| P00-T03 onward | Not approved, not started |
| Implementation | None. No application code, fixtures or generators exist |
| Application tests | None run; none passed by implication |
| Verification | P00-T02 digest cross-check: 8 of 8 request digests match the lookup log (in-session Python check whose console output was not saved to a file; not an install verification). P00-T01 document check 10/0/0 after two corrected runs (`evidence/P00/P00-T01/check.md`); planning CA-01 contract check 22/0/0 (`evidence/P00-planning/contract-check.md`). Both are document checks, not application tests |
| Approvals | 2026-10-23: planning documents, CA-01, commit + push of planning checkpoint. 2026-10-24: P00-T01; acceptance of P00-T01 and commit + push; P00-T02 and Q-06 read-only lookups. Granted 2026-10-24: P00-T02 acceptance; Request A; B1 provisioning with yamllint/Checkov in isolated envs; scoped local P00-T02 commit. Granted 2026-10-24 (later): V1 attestation verification, resume B1, local commit. Not granted: push, cargo-audit (P01), crates, real API, scanner spikes, P00-T03+, PyPI provenance approach (V3) |
| Git | Local commits on `main` ahead of `origin/main` (`db402ca`): `4c659eb` (P00-T02 docs) and the install-evidence commit containing this file; **not pushed** |
| Specs | v0.3.1 (CA-01); store 1.2, wrapper protocol 1.0, app contract 1.0, UI schema 1.0 |
| Agents | Proposals only (`proposals/agents/`); `.claude/` absent. Worker models unverified. P00-T01 written by Opus, no worker used |

## Blockers
B-01 resolved for P00 baseline tools (cargo-audit pending P01), B-02 crates, B-08 schedule fit. B-03 resolved for read-only lookups only. B-09 resolved: Checkov runs on standalone CPython 3.12.15 — see `plan.md` §8. Q-06 answered (read-only lookups approved). Q-01–Q-05 open.

## Next permitted action
None. Proposed: P00-T03 (crate request) or spike tasks P00-T04/T05 (local, no installs) — each needs approval. Push of local commits not authorised. Open: PyPI provenance decision (V3 or accept).

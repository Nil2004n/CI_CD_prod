# Planning inspection and reconciliation (archived summary)

Source: read-only inspection performed by Opus in the preceding planning session on 2026-10-23, accepted by the owner as planning evidence subject to decisions D-01–D-14 and corrections 1–6.

## Repository at inspection
- Branch `main`, commit `805ced4` (only commit), clean tree, one worktree, remote `origin`.
- Present: five specifications, `README.md`, `CLAUDE.md`, `schemas/` (3 files).
- Absent: `plan.md`, `status.md`, `handover.md`, `.claude/`, agent definitions, `Cargo.toml`, source, tests, scripts, CI; the README-mentioned bootstrap guidance, Haiku template and revision notes.
- Existing verified code: none. Existing unverified code: none.

## Host tools (version checks only, not re-run in this session)
Present: git 2.53.0, rustc/cargo 1.97.1, clippy, rustfmt, gitleaks 8.30.1, trivy (reports `Version: dev`), WeasyPrint 67.0, Python 3.14.7. Missing: cargo-audit, ShellCheck, Bats, yamllint, Checkov, actionlint, kubeconform, zizmor, docker/compose, helm, pandoc, typst. Host: Kali Rolling x86_64.

## Consistency findings
| ID | Finding | Resolution |
|---|---|---|
| C1 | Repo config/ignore files can override scanner policy | Per-tool hypothesis, P00-T09 (Tech §18) |
| C2 | `allow_fallbacks` not constrained in schema | CA-01: `const false` + request enforcement |
| C3 | WrapperRequest lacks capability/category; no cancel/timeout contract | CA-01 |
| C4 | Divergent event lists (Arch §6 vs UI §17) | CA-01: canonical Arch §6 |
| C5 | Missing commands for required/proposed features | CA-01: pilot commands + reserved deferred |
| C6 | UI YAML used store version 1.2 | CA-01: `ui_schema_version` 1.0 |
| C7 | Arch §11 layout omitted `tools.json`/`suppressions.json` | CA-01 |
| C8 | SCAN-05/SCAN-06/RAG-01 doc sources lacked adapters | PRD §16 labels |
| C9 | `.sh` wrappers vs portability | D-03 |
| C10 | Python prerequisite undeclared | D-05, Tech §3 |
| C11 | Directory-only tools vs selected scope | Per-tool hypothesis, P00-T09 |
| C12 | Parser/validator/keyring/PDF/process-kill need spikes | P00-T05–T08 |
| C13 | Kubernetes schemas need network or cache | D-11, Tech §25, P00-T10 |
| C14 | Scaffolding needs crate downloads | Separate approval (B-02) |

# P00-T10 — Kubernetes schema cache plan: record

Written by Opus, 2026-10-24.
- **Approval:** owner, 2026-10-24 (approve T10; Sonnet sub-agents in branch sessions; integrate, test and report in the main session; commit and push only after a later approval).
- **Base:** `ea50cb7` (phase branch). Nothing committed.

## Work and roles
| Part | By | Files |
|---|---|---|
| Read-only lookups L1–L11 (WebFetch of public pages and GitHub metadata; no schema download) | Opus | `evidence/P00/P00-T10/lookups.md` |
| Request and frozen interface | Opus | `spikes/P00-T10-request.md` (§2 completed at integration with helper signatures; §4 test command corrected) |
| Plan | W1 `sonnet-implementer` (configured `claude-sonnet-5-5`, unverified), worktree branch `worktree-agent-ae1a93cee4fbb22d5` | `spikes/P00-T10-schema-cache-plan.md` (§13 added by Opus) |
| Prototype and tests | W2 `sonnet-implementer` (configured `claude-sonnet-5-5`, unverified), worktree branch `worktree-agent-ae4c40e4855c76820` | `spikes/t10-cache/cache.py`, `spikes/t10-cache/test_cache.py` |
| Consistency checker | Opus | `spikes/t10-cache/check_consistency.py` |

**Coordination:**
- The workers have no messaging tool. They used the append-only board (`flock`) in the session scratchpad: 2 stop notices, then INFO and DONE from each worker; no questions were needed.
- Ownership was disjoint, so no conflicts arose.
- The first attempts stopped correctly on the base check (worktrees were created from `008e80c`). The retries fast-forwarded to `ea50cb7`.
- **Disclosed by W2:** one unsanctioned plain `python3` heredoc was used to edit its own two files. No side effects were found: no cache artefacts, and the worktree held only the two files.

## Verification (Opus, main tree, after integration; worker reports are not evidence)
| Check | Command (cwd) | Result |
|---|---|---|
| Integration identity | sha256 of worktree file vs main-tree copy | identical for all 3 files |
| Unit tests | `/usr/bin/python3 -I -B -m unittest discover -s . -p 'test_cache.py'` (`spikes/t10-cache`) | exit 0; 47 run, 0 failures, 0 errors, 0 skipped |
| Consistency | `/usr/bin/python3 -I -B spikes/t10-cache/check_consistency.py .` (repo root) | First run 60/64. Three request-§2 omissions (Opus defect, fixed in the request) and one checker false negative (regex split across source lines; checker now compares module values). Re-run 64/64, exit 0. |
| Lookup citations | part of the consistency check | every `[Ln]` in the plan exists in `lookups.md` |
| Fact check | §8 "kubeconform exit 0, statusValid" against T09 `kc-traversal-kind` | confirmed |
| Bytecode | `find spikes -name __pycache__ -o -name '*.pyc'` | 0 |
| Prototype scope | grep of `cache.py` | stdlib only; no socket/HTTP/subprocess/write/delete calls |
| Review | containment, symlink walk, extra-file scan, lookup order | fails closed. Finding **R-1** (group collision in the frozen mapping, demonstrated on synthetic data) recorded in plan §13; not repaired. |

## Limitations
- Lookups were read through a summarising fetch tool. Listings were truncated, and per-kind files are unconfirmed.
- There are no upstream signatures.
- No legal review of licences.
- The prototype is not production code: its checks are not atomic (TOCTOU), and it uses synthetic data only.
- kubeconform was not run in T10.
- Kubeconform's own `KindSuffix` rule was not re-checked.
- Worktrees and their branches are retained until the owner approves integration (cleanup after commit).

## Acceptance (owner, 2026-10-24)
- **Accepted** as a completed schema-cache planning and offline reference-prototype task, with its findings and limitations recorded.
- **Not accepted:** a production cache implementation, actual schema coverage, or collision-safe schema resolution.
- **R-1** is a mandatory P13 acceptance requirement (`plan.md` FR-30; plan §14). The prototype does not fix it, and unknown API groups must not be routed through the current mapping.
- **O-1 to O-6** remain open.
- **Retained:** the worker deviation, lookup limitations, missing signatures, non-atomic prototype checks, and the absence of a kubeconform run.

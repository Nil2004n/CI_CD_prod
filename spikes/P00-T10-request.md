# P00-T10 — Kubernetes schema cache plan (D-11, Tech §25): request and work split

Status: approved by the owner on 2026-10-24 ("I accept and approve t10, spawn the required sonnet5.5 sub-agents in branch sessions complete the task, test and merge them in the main session give me the report, once I approve them in the main session we'll commit and push"). Commit and push need the owner's later approval.

## 1. Scope
**Deliverables:**
1. A plan document `spikes/P00-T10-schema-cache-plan.md` covering:
   - candidate origins;
   - target Kubernetes version(s);
   - licence and redistribution;
   - integrity;
   - freshness;
   - the network / `schema_fetch` operation;
   - missing CRD/kind handling;
   - **FR-26**: kind/apiVersion-derived paths are untrusted; the cache needs traversal and containment rules;
   - P13 hand-off;
   - open owner decisions.
2. An offline, stdlib-only reference prototype in `spikes/t10-cache/`: cache name mapping, input validation, path containment, manifest verification. Tested on synthetic data only, to make the plan's FR-26 and integrity rules concrete and testable.

**Not in scope:**
- downloading any schema set or file;
- running kubeconform;
- network access by workers;
- installs;
- changes to specs or contracts;
- commits, pushes, merges.

**Read-only lookups:** done by Opus only, recorded in `evidence/P00/P00-T10/lookups.md` (L1–L11). That file is the frozen evidence input. Workers must cite L-numbers and must not add facts not present there.

## 2. Frozen interface (shared by W1 and W2; changes only through Opus)
**Cache layout** (one *cache set* per origin + revision + Kubernetes version):

```
<cache_root>/<set_id>/manifest.json
<cache_root>/<set_id>/v<MAJOR>.<MINOR>.<PATCH>-standalone-strict/<kind>-<group0>-<version>.json   # named groups
<cache_root>/<set_id>/v<MAJOR>.<MINOR>.<PATCH>-standalone-strict/<kind>-<version>.json            # core group
```
`set_id` = `[a-z0-9][a-z0-9._-]{0,63}`.

**Name mapping** (kubeconform `{{.ResourceKind}}{{.KindSuffix}}` as observed in T09: `deployment-apps-v1.json`):
- `kind` is lowercased.
- For apiVersion `group/version`, the suffix is `-<first DNS label of group>-<version>`.
- For core `v1`, the suffix is `-v1`.

**Input validation, before any mapping:**
- `kind` must match `^[A-Za-z][A-Za-z0-9]{0,62}$`.
- `apiVersion` must match `^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*/)?v[0-9]+(?:(?:alpha|beta)[0-9]+)?$`.
- Anything else yields `invalid_identifier` (reported as `unsupported`, never `passed`).

**Containment:** the mapped path is joined under the version directory and resolved. All of the following must hold:
- it stays inside `<cache_root>/<set_id>/`;
- no component is a symlink;
- the file is a regular file.

Otherwise the result is `containment_violation` (the cache set becomes unavailable).

**Manifest (`manifest.json`, UTF-8 JSON object):**

```json
{
  "manifest_version": "1",
  "set_id": "...",
  "origin": {"name": "...", "url": "...", "publisher": "...", "revision": "<40-hex commit or tag>", "licence": "SPDX id"},
  "kubernetes_version": "v1.36.5",
  "variant": "standalone-strict",
  "retrieved_at": "RFC 3339 UTC",
  "files": {"<relative path>": "<sha256 hex>", "...": "..."}
}
```

**Set verification (`verify_set(cache_root, set_id) -> (status, details)`).** `status` is one of:
- `available`;
- `unavailable_missing_manifest`;
- `unavailable_bad_manifest`;
- `unavailable_hash_mismatch`;
- `unavailable_missing_file`;
- `unavailable_extra_file`;
- `unavailable_symlink`;
- `unavailable_not_regular`.

Any failure makes the **whole set unavailable**; it is never partially trusted.

**Lookup (`lookup(cache_root, set_id, kind, api_version) -> (status, path_or_None)`).** `status` is one of:
- `found`;
- `unsupported_missing_schema` (gap);
- `invalid_identifier`;
- `containment_violation`;
- `set_unavailable`.

**Freshness (`freshness(manifest, now, max_age_days) -> "fresh" | "stale" | "unknown"`).** The threshold is an owner policy input; the prototype takes it as a parameter.

**Helper functions.** These were specified in the W2 assignment packet rather than in the first version of this section; added here by Opus at integration for completeness, with no change to their behaviour:
- `validate_identifiers(kind, api_version) -> bool`;
- `schema_filename(kind, api_version) -> str` (raises `ValueError` on invalid identifiers);
- `load_manifest(set_dir) -> dict` (raises `ValueError` on any defect).

## 3. Work split (`sonnet-implementer`, configured `claude-sonnet-5-5`, separate worktrees, base `ea50cb7`)
| Worker | Owns (exclusively) | Task |
|---|---|---|
| **W1 (plan)** | `spikes/P00-T10-schema-cache-plan.md` | Write the plan per §1 from L1–L11 and the frozen interface §2. Compare origins A/B/C plus exclusions; recommend, but leave decisions to the owner. |
| **W2 (prototype)** | `spikes/t10-cache/cache.py`, `spikes/t10-cache/test_cache.py` | Implement §2 exactly (stdlib only) and unit tests on synthetic temporary data. |

**Coordination (agents have no direct messaging tool):**
- Shared append-only board at `<scratchpad>/t10-coord/board.md`.
- Append only via `flock <board>.lock -c 'cat >> <board>'`, with entries `[W1|W2 UTC-time] TYPE: text` (TYPE = INFO / QUESTION / ANSWER / DONE).
- Rules:
  - read the board at start, before finishing, and when blocked;
  - never wait on the other worker for more than 2 board checks;
  - the frozen interface wins;
  - unresolved questions go to the final report for Opus, not into a waiting loop;
  - never edit the other worker's files.

**Permitted commands:**
- `git rev-parse`, `git status`, `ls`, `cat`, `grep`;
- `/usr/bin/python3 -I -B` (W2: unittest and syntax checks; W1: none);
- `flock`, `date`.

No network, installs, commits or file deletion outside the worker's own scratch.

## 4. Verification by Opus (after integration into the main tree)
- Re-run W2's tests: `/usr/bin/python3 -I -B -m unittest discover -s . -p 'test_cache.py'` from `spikes/t10-cache/`, plus a no-`__pycache__` check. Correction at integration: the originally stated form `-m unittest -v test_cache` cannot import the module under `-I` (cwd is not on `sys.path`); W2 reported this.
- Opus-written cross-check: the interface named in the plan equals the implemented names and statuses, and every fact in the plan cites an L-number present in the lookup log.
- Review of both diffs.
- Worker reports are not evidence.

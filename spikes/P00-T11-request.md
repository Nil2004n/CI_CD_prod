# P00-T11 — freeze and P00 report: request and work split

Status: approved by the owner, 2026-10-24 ("I approve T11, prepare the freeze and P00 report, spawn the required sonnet5.5 sub-agents in branch sessions complete the task, test and merge them in the main session give me the report, once I approve them in the main session we'll commit and push").
- Commit and push need the owner's later approval.
- P00 acceptance, merge to `main` and push to `main` are **not** part of T11.

## 1. Scope
Plan §5 T11: freeze tool versions/flags/coverage, crate selections, contract sheets, and the P00 report.

**Deliverables** (new files only; documentation and machine-readable records):

| File | Content |
|---|---|
| `spikes/p00-freeze/tools.json` + `tools.md` | Tool freeze: every P00 tool with version, path, hash basis, status, invocation contract, coverage, limitations and FR links; plus the contract-sheet index |
| `spikes/p00-freeze/crates.json` + `crates.md` | Crate freeze: Rust toolchain, every direct spike dependency with exact version, features, status and evidence; provisional, fallback, excluded and deferred items; advisory exception |
| `evidence/P00/P00-report.md` | P00 report draft: per-task outcome and qualifications, decisions, debug cycles, FR-01–FR-30 summary, open decisions, risks and limitations, closure checklist |
| `spikes/p00-freeze/check_freeze.py` (Opus) | Offline consistency checker (test) |

**Rules:**
- **Freeze principle:** record only what accepted records establish. Nothing is upgraded, re-resolved, installed or downloaded. A freeze entry never upgrades an earlier acceptance qualification (for example, a "provisional" stays provisional; T03 primaries not exercised by a spike stay provisional).
- **Allowed sources** (read-only, in the repository at `3decec1`): `plan.md`, `status.md`, `spikes/*.md`, `evidence/**` records and contract sheets, `spikes/rust/**/Cargo.toml`, `spikes/rust/Cargo.lock`. No network, and no facts from elsewhere.
- **Not in scope:** changes to specs, contracts, plan, status, handover or CLAUDE.md (Opus owns those); running scanners or cargo; reading the private raw evidence location.

## 2. Frozen formats (shared by W1, W2 and the Opus checker)
`tools.json`:

```json
{
  "freeze_version": "1",
  "frozen_at": "2026-10-24",
  "base_commit": "3decec1",
  "tools": [
    {
      "id": "gitleaks",
      "role": "baseline | framework | interpreter | install-helper | verification-helper | deferred",
      "version": "8.30.1",
      "path": "~/.local/bin/gitleaks",
      "sha256": "<64-hex or null>",
      "hash_basis": "install-record trusted hashes | observed (not trusted) | none",
      "status": "accepted-qualified | accepted-limitations | pending | deferred",
      "contract": ["<repo-relative path>", "..."],
      "invocation": "<allowed invocation summary, exactly as the contract states, or null>",
      "rejected_invocations": ["..."],
      "coverage": "<text>",
      "limitations": ["..."],
      "requirements": ["FR-xx", "..."],
      "evidence": ["<repo-relative path>", "..."]
    }
  ],
  "contract_sheets": [
    {"tool": "checkov", "path": "...", "kind": "generated | opus-authored", "status": "current | superseded-for-invocation"}
  ]
}
```

Required tool ids:
- **Scanners:** `yamllint`, `gitleaks`, `checkov`, `actionlint`, `kubeconform`.
- **Interpreter and helpers:** `python3.12`, `uv`, `cosign`.
- **Deferred:** `cargo-audit`.
- **Toolchain:** the Rust toolchain goes in `crates.json`.

`crates.json`:

```json
{
  "freeze_version": "1",
  "frozen_at": "2026-10-24",
  "base_commit": "3decec1",
  "toolchain": {"rustc": "1.97.1", "cargo": "1.97.1", "channel": "stable-x86_64-unknown-linux-gnu", "evidence": ["..."]},
  "lockfile": {"path": "spikes/rust/Cargo.lock", "sha256": "<64-hex>"},
  "crates": [
    {
      "name": "libc",
      "version": "0.2.191",
      "default_features": true,
      "features": [],
      "spike_package": "t05-process | t06-yaml | t07-keyring | t08-pdf | null",
      "purpose": "<text>",
      "status": "accepted-qualified | provisional | deferred | excluded | fallback",
      "advisory_exceptions": ["RUSTSEC-..."],
      "evidence": ["..."],
      "notes": ["..."]
    }
  ],
  "build_rules": ["build per package (-p), never --workspace (feature unification)", "..."]
}
```

- `crates[]` must contain **every direct dependency pinned in a spike `Cargo.toml`** with exactly that version.
- It must also contain the T03 primaries not exercised by a spike (status `provisional`), the T03 fallbacks (`fallback`) and the exclusions (`excluded`), each with `version` exactly as T03 states, or `null` if none is stated.

Field names, status vocabularies and ids are fixed. Changes go only through Opus (post a QUESTION on the board).

## 3. Work split (`sonnet-implementer`, configured `claude-sonnet-5-5`, separate worktrees)
**Step 0 for each worker:** `git merge --ff-only 3decec1` in its own fresh worktree. The harness creates worktrees from `008e80c`, an ancestor of `3decec1`.

| Worker | Owns exclusively |
|---|---|
| **W1** | `spikes/p00-freeze/tools.json`, `spikes/p00-freeze/tools.md` |
| **W2** | `spikes/p00-freeze/crates.json`, `spikes/p00-freeze/crates.md`, `evidence/P00/P00-report.md` |

- W2's report refers to tool facts by pointing to `spikes/p00-freeze/tools.json` and the contract sheets. It does not restate hashes.
- If W2 needs a tool fact for the report, it reads the T09/T02 records itself or asks W1 on the board.

**Coordination board:** `<scratchpad>/t11-coord/board.md`.
- Append only via `flock <board>.lock -c 'cat >> <board>'`, with entries `[W1|W2 UTC] INFO|QUESTION|ANSWER|DONE: text`.
- Read the board at start, before finishing, and when unsure.
- Never wait for the other worker for more than 2 board checks. The frozen formats win; unresolved items go into the final report.
- Never edit the other worker's files.

**Permitted commands:** the step-0 merge once; `git rev-parse`, `git status`, `ls`, `cat`, `grep`, `sed -n`, `sha256sum` (repository files only), `date`, `flock`; `/usr/bin/python3 -I -B` for JSON syntax validation only.
- No network, scanners, cargo, installs, commits or deletes.

## 4. Verification (Opus, main tree, after integration)
`check_freeze.py` (stdlib, offline, read-only) checks:
- JSON shape and vocabularies;
- trusted hashes against the install record;
- installed binaries' current SHA-256 (local read-only hashing);
- crate pins against `Cargo.toml`/`Cargo.lock`;
- lockfile hash;
- existence of every referenced path;
- every FR id against `plan.md` §5.2;
- that the report covers T01–T11 and FR-01–FR-30.

Opus reviews both diffs against the source records. Worker reports are not evidence.

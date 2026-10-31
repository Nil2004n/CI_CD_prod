# Handover — takeover guide

Checkpoint: **2026-11-01, for the developer taking over on 2026-11-02.** Written by Opus (lead). P00–P03 are accepted and merged into `main`; no phase is active; the next step needs the owner. Live state and open decisions: `status.md`. Earlier handovers, verbatim: `evidence/P03/continuity/handover-at-P03-close.md` (sha256 `c7c45699…`, P03 checkpoints) and `evidence/P02/continuity/handover-at-P02-close.md` (sha256 `69bb8973…`, P00–P02 checkpoints and lessons).

---

## 0. First hour (do these in order)
1. `cd /home/kali/Desktop/CI_CD_prod && git status -sb` → expect `## main...origin/main` and only `?? .commandcode/` (the owner's local folder: never commit, edit or delete it).
2. Read, in order: `CLAUDE.md` (the working contract — binding), `status.md` (live state), this file, `plan.md` §3 (development control), §4 (roadmap), §4.1 (CLI rule), §5C (P03, the most recent phase), §5D (CA-05), §8 (risks).
3. Skim the latest phase report `evidence/P03/P03-report.md` (what exists, how it was verified, known limitations).
4. Prove the toolchain works without building: `python3 -I -B dev/p03_check.py env --slot lead --out /tmp/claude-1000/pwt-env-check` → `OVERALL PASS` (checks rustc/cargo 1.97.1 and that the build namespace has no network).
5. Optional full check (rebuilds from scratch, ≈10 min, ~4–5 GB): `python3 -I -B dev/p03_check.py all --final --slot lead --out <new dir>` → expect 1138 passed, 0 failed, 0 ignored, 41 compile_fail. Put real evidence runs under `evidence/…`, throwaway runs under a scratch dir.
6. Do **not** start any phase work. Ask the owner which next step is approved (§8).

---

## 1. The project in one page
- **Product:** PWT YAML Security Workbench — a local, single-user Rust terminal application that registers Git repositories, discovers every `.yaml`/`.yml`, lets the user select repositories/files, runs pinned local scanners (yamllint, Gitleaks, Checkov, rust-schema; actionlint, kubeconform as framework adapters), records findings and honest coverage, writes JSON/Markdown/PDF reports, and (planned) analyses the sanitised evidence automatically with the user's own OpenRouter key (CA-05) plus an optional grounded RAG chat. Advisory only: no auto-fix, no commits/pushes to assessed repos, no background daemon.
- **Specifications (root):** `01_PRD.md` (requirements, IDs, release labels §16), `02_Technical_Specification.md`, `03_Architecture.md` (command/event contract §6, state machines §7), `04_UI_Schema.md`, `05_Database_Data_Schema.md` (store schema §7, semantic rules §11/§18, amendments CA-01–CA-04 §22–§25, CA-05 draft §26). Version 0.3.5.
- **Pilot target:** Linux x86_64 (D-02); Windows/macOS are unverified extensions.
- **Built so far (crates in `workbench/crates/`):** `domain` (IDs, time, safe-text types, vocab, states, app contract 1.1 DTOs, typed store models for all store roots), `persistence` (schema + semantic validation, single writer, lock, atomic replace, recovery, report naming, FR-33), `application` (event emission, dev store commands, permission evaluator, approvals, child environment, session credentials, dev permission check), `cli` (headless developer CLI `pwt-workbench-cli`). Placeholders: `repositories`, `scanning`, `retrieval`, `terminal-ui`.
- **Not built yet:** Git operations (P04), inventory/selection (P05), process supervision/real redactor (P06), the first end-to-end scan (P07), scanner adapters (P08/P09/P13), reports beyond JSON (P10–P12), workflows/scheduling (P14), TUI (P15), OpenRouter client and automatic analysis (P16), RAG (P17).
- **Tests:** 1138 passing tests + 41 compile_fail doctests, all through the runner.

## 2. How work is governed (binding)
- **Roles (`CLAUDE.md`):** the **owner** approves every phase, wave, debug cycle (unless pre-authorised for that phase), install, network use, commit/push/merge and scope change. **Opus (lead)** plans, owns shared contracts (`crates/domain` DTOs, schemas, `Cargo.lock`, specs, plan/status/handover, CLAUDE.md), integrates, runs all verification and writes continuity. **Sonnet 5.5 workers** (`sonnet-implementer`) implement bounded tasks from an assignment packet in their own worktree. A worker's report is never evidence; only the lead's runner output under `evidence/` is.
- **Phase loop:** approved phase → scoped implementation + its listed checks → lead verifies → report → STOP. A failing check does not authorise repairs; propose a debug cycle (confirmed bug vs hypothesis, severity, bounded fix). Two failed attempts on one defect → revised diagnosis/owner decision. No automatic next phase.
- **Always needs separate owner permission:** network access (real APIs, fetches, lookups), installs and new dependencies, destructive operations, commits/pushes/merges unless the phase decision grants them (P03 had C9: lead commits/pushes on the phase branch), client data.
- **Invariants you must keep:** no raw keys/secrets in configs, logs, reports, docs or evidence; strip model credentials from child environments; no `eval`/repo config/plugin execution; mocks labelled and never counted as real acceptance; no "all green = secure" claims; record command, cwd, revision, exit, counts and evidence paths for every check; never weaken assertions or delete regression cases to pass.
- **Quote the owner.** Records cite the owner's words for each approval (spelling kept as written).

## 3. Repository map
| Path | What it is |
|---|---|
| `CLAUDE.md` | Working contract (roles, phase loop, invariants, verification rules) |
| `README.md` | Package history + "Start here" summary |
| `plan.md` | Decisions D-01–D-18 (§1), open questions Q-01–Q-06, control rules (§3), roadmap (§4), CLI rule (§4.1), per-phase plans (§5–§5D), estimates (§7), risks/blockers (§8), debug-cycle log (§9) |
| `status.md` / `handover.md` | Live state / this guide |
| `01_…05_*.md` | Specifications |
| `schemas/` | Canonical store schema 1.4 and wrapper protocol schemas (hash-pinned; changing them is a contract amendment) |
| `workbench/` | Cargo workspace (`Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `clippy.toml`, `crates/`) |
| `dev/` | Check runners: `p03_check.py` (current), `p02_check.py`/`p01_check.py` (historical records), floors `p03_baseline.json`, `p02_differential.py`, `p02_fr31_appendix.py`, `p02_registry.json` |
| `planning/` | Phase requests (`P0N-T01-request.md` — the frozen interfaces of a phase), worker packets `P0N-packets/`, CA-05 change contract, decision proposals and drafts |
| `evidence/` | Every verification record: `P0N/` per phase (task records, baselines B0..Bn, reports, freezes), `debug/Pxx-Dnn/` per debug cycle, `CA-05-docs/`, `cleanup-2026-11-01/`, continuity archives `P0N/continuity/` |
| `spikes/` | P00 spike harnesses and requests (historical) |
| `proposals/` | Not-adopted items: `agents/haiku-docs.md`, original `sonnet-implementer` proposal, a CLAUDE.md amendment |
| `.claude/agents/`, `.claude/skills/` | Installed sub-agents and skills (§6.4) |
| `.claude/settings.json` | Shared allow rules for the P03 runner; `.claude/settings.local.json` is machine-local (git-excluded) |
| `.claude/worktrees/` | Worker worktrees during a wave (git-excluded; currently empty) |

## 4. Environment
- **Host assumptions:** Kali Linux x86_64, user **`kali`**, repo at `/home/kali/Desktop/CI_CD_prod`. The runner hard-codes `/home/kali/.local/share/pwt-dev-tools` and `/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`. Unprivileged user namespaces must work (`unshare -rn true` exits 0).
- **Toolchain:** Rust 1.97.1 (exact version gate in the runner), edition 2024, resolver 3, `unsafe_code = "forbid"`.
- **Runner Python:** host `python3` (3.14.7), always `python3 -I -B`.
- **Isolated tool root `~/.local/share/pwt-dev-tools/`** (keep everything; evidence cites it): `_cargo-home` (offline crate cache — the build never touches the network), `_cargo-target/` (per-slot build dirs, created on demand, delete after a phase), `cargo-audit/` (0.22.2 binary, RustSec DB clone `2295466c`, offline-rebuild staging), scanner installs (`actionlint/1.7.12`, `kubeconform/0.8.0`, `yamllint/1.38.0` venv, `checkov/3.3.30` venv, `python/3.12.15…`, `uv/0.13.5`), `_staging` (install provenance), `_private-evidence` (sensitive spike output kept off-repo), `_artifacts`, `_p01`/`_p02` (old run outputs), `_archive` (scratch and git-object archives). Gitleaks 8.30.1 is `~/.local/bin/gitleaks`. Provenance: `evidence/P00/P00-T02/install-record.md` (PyPI provenance exception: yamllint/Checkov wheels are not provenance-verified — development only).
- **Disk:** each Cargo slot dir grows to 4–5 GB; three parallel workers + lead need ~20 GB. Free ≈31 GB now. Delete slot dirs after a phase is committed.
- **Remote:** `origin` = `https://github.com/Nil2004n/CI_CD_prod.git` (HTTPS). Pushes need owner approval.

## 5. Everyday commands (cwd = repo root unless stated)
| Purpose | Command |
|---|---|
| State | `git status -sb`; `git log --oneline -10`; `git worktree list` |
| Full check | `python3 -I -B dev/p03_check.py all --slot lead --out <dir>` (add `--final` to forbid `todo!`/placeholders and unapproved allows) |
| One step / one crate | `python3 -I -B dev/p03_check.py <env|fmt|fmt-fix|clippy|test|build|static|cli> --slot lead --out <dir> [--package <crate>]` |
| Worker tree | add `--repo <worktree path> --slot w1|w2|w3` (each slot has its own target dir) |
| Floors | Committed floors `dev/p03_baseline.json` (1138/41) are enforced **only with `--final`** (the runner takes the max of the file and the flags); without `--final` pass `--min-passed N --min-compile-fail M` explicitly when a floor matters. Raise after each accepted baseline; never lower without the owner |
| cargo-audit (offline) | `unshare -rn env -i HOME=<tmp> PATH=/usr/bin:/bin ~/.local/share/pwt-dev-tools/cargo-audit/0.22.2/bin/cargo-audit audit --no-fetch --db ~/.local/share/pwt-dev-tools/cargo-audit/advisory-db/clone --file workbench/Cargo.lock --json` |
| Binary key sweep | `unshare -rn python3 -I -B evidence/P03/B3/lead_sweep.py <target>/debug/pwt-workbench-cli <scratch dir>` (binary under `~/.local/share/pwt-dev-tools/_cargo-target/p03-lead-*/debug/`) |
| CA-05 draft checks | `python3 -I -B planning/CA-05-draft/generate_draft.py` (regenerate), `python3 -I -B evidence/CA-05-docs/check_draft.py` (PASS), `python3 -I -B evidence/CA-05-docs/check_docs.py` (exits 1 by design: 0 CA-05 regressions + 9 pre-existing `plan.md` rows — never report it as passing) |
| P02 differential | `python3 -I -B dev/p02_differential.py --out <dir>` (host jsonschema 4.26.0) |

What the runner does: calls the 1.97.1 binaries directly with `env -i` and the isolated `CARGO_HOME`, runs every Cargo command inside `unshare -rn` (no network), uses `--frozen`, hashes `Cargo.lock` and every manifest before/after each step, captures exit codes from the producer, writes `summary.json`, `env.json`, `static.json` and per-step logs. **Never run plain `cargo`** (an unexplained plain build left a 3.9 GB `workbench/target/` on 2026-10-31; it was deleted).

Static rules (`dev/p03_check.py static`, 111 self-test cases): environment reads only in `cli/src/main.rs`; no `std::process::Command` until P06; `CredentialSecret::expose`/`::new` only in `domain/src/contract.rs` and `application/src/credential.rs`; `serde_json::from_*` only in `persistence/src/{read,schema}.rs` and `application/src/dev_permission.rs`; `redact_validated` only in `domain/src/store/values.rs`; `Authorisation {}` only in `approvals.rs`, `Grant {}` only under `permission/`; rule 12 `Evaluation::Permitted`/`permission::evaluate` only in `approvals.rs` (+`permission/`); rule 13 `RequestOutcome::Permitted` only in `approvals.rs`; `#[allow]`/`#[expect]` equal to the approved list; registry crate set, lock SHA and manifest dependency allowlist unchanged; P02 rules (store field types, redacted fields).

Git conventions: phase branch `phase/P0N-<name>` from `main`; worker branches `p0N/<task>-w<N>`; commit subjects name the unit (`P03 B2: …`, `P03-D02: …`, `P03 T12: …`, `docs: …`); every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; merges to `main` are `--no-ff` and only on the owner's acceptance; stage explicit paths (never `git add -A`).

## 6. How a phase is run (the P03 pattern, which worked)
1. **Plan** (owner asks "plan P0N"): add `plan.md` §5x with scope, facts checked read-only, tasks with owners/hours, acceptance criteria, risks and owner decisions (options with a recommendation). Stop for the owner's choice.
2. **T01 request** (`planning/P0N-T01-request.md`): module layout, frozen interfaces (signatures, error enums, DTOs), policies, test plan per task, debug bounds, lead decisions. Lead-only (shared contracts). T02 dependency/lock check (offline resolution in a scratch copy; registry set must not change without approval). T03 contract check (draft a CA-nn only if a frozen contract must change — needs owner approval). T04 scaffold: frozen stubs with `todo!("P0N-Tnn")` + `#[expect(clippy::todo, reason = "B0 stub")]`, new runner `dev/p0N_check.py` derived from the previous one, baseline **B0**.
3. **Waves:** up to three `sonnet-implementer` workers with disjoint owned files, each in its own lead-prepared worktree from a common baseline. Per wave write `planning/P0N-packets/_waveN-common.md` + one packet per worker (task ID, base commit, worktree/branch/slot, scratch dir, owned files, frozen items, implement list, tests, permitted commands, stop condition, report format). Create worktrees: `git worktree add -b p0N/tNN-wX .claude/worktrees/p0N-tNN-wX <base>`. Spawn with the Agent tool, `subagent_type: "sonnet-implementer"`, a short prompt pointing to the packet file, `run_in_background: true`, **no** `isolation` parameter.
4. **Coordination between workers** (owner requirement): an append-only log `.claude/worktrees/_<wave>-coord.md` (outside Git). Workers append one line per event with Bash `printf … >>` (`<UTC> <W#> <START|INFO|QUESTION|ANSWER|REQUEST|FINDING|BLOCKED|DONE>: …`), read it before every test run and before reporting. The lead posts `LEAD` lines and can message a running worker with SendMessage. A worker that needs a file it does not own posts `REQUEST` and stops that line of work; the lead decides. Copy the log into the baseline's evidence folder at integration.
5. **Integration (lead, main checkout):** verify each worktree's `git status` and HEAD; copy owned files into the main tree **and `touch` them** (Cargo uses mtimes); run `all` (or `all --final` once no stubs remain); compare counts with the arithmetic (previous floor + added tests); run lead negative controls (revert one fix, `touch`, expect failure, restore byte-identical, `touch`, rebuild before using any binary); write `evidence/P0N/Bn/record.md`; raise floors; commit `P0N Bn: …`.
6. **Reviews:** spawn read-only `reviewer` and `security-reviewer` over the phase diff with spec references and a request for ranked, located findings. Confirm each finding in code before acting.
7. **Debug cycle** (`Pxx-Dnn`): packet per worker + `evidence/debug/Pxx-Dnn/record.md` (finding, fix, owner, then outcome and lead verification). Base = the committed baseline. Max two attempts per defect.
8. **Report (T12-equivalent):** fresh `all --final` at the committed HEAD, cargo-audit, any manual/real-process checks, freeze JSON (`evidence/P03/T12/make_freeze.py` is the template), `evidence/P0N/P0N-report.md` (scope, tasks, verification table, acceptance criteria table, debug cycles, agents and model evidence, owner decisions, limitations, next steps). Stop.
9. **Acceptance → merge → clean-up** only on the owner's words: record acceptance, `git merge --no-ff` into `main`, check the merge tree equals the phase tree, push; then remove worktrees (after verifying every file is on `main`), delete merged worker branches with `git branch -d`, delete slot target dirs, archive scratch, `git gc`.

### 6.4 Agents and skills
- `sonnet-implementer` (`model: claude-sonnet-5-5`): the implementation worker. At first spawn in a phase record the configured model; afterwards check `message.model` in its transcript (`/tmp/claude-1000/-home-kali-Desktop-CI-CD-prod/<session>/tasks/<id>.output`: `grep -oh '"model":"[^"]*"' <file> | sort | uniq -c`) for B-04 evidence.
- `implementer` (`model: sonnet`, preloads project-verification), `reviewer`, `security-reviewer` (read-only: Read/Grep/Glob).
- Skills: `project-verification`, `project-debugging`, `project-security-review`, `phase-handoff`.
- Workers may hit the usage limit mid-task; their state is kept — resume them with SendMessage after the reset instead of respawning, and check their worktree first.

## 7. Codebase tour (what to read before touching each area)
- **`domain`** — `ids.rs` (typed UUIDs), `time.rs` (UTC timestamps), `text.rs` (`RedactedText`/`SanitisedText`; frozen escape set — note it keeps LF/TAB, so renderers must escape them per line), `vocab.rs` (capabilities, categories), `states.rs` (run/check/approval state machines), `contract.rs` (app contract 1.1 commands/events, `CredentialSecret`, `RawInput`, `FixedText`), `store/*` (typed models of every store root; FR-31 safe-text classes).
- **`persistence`** — `fs.rs` (`FsOps` seam, `RealFs`, euid from `/proc/self/status`), `paths.rs` (`DataDir`, FR-33), `lock.rs`, `atomic.rs` (temp → flush → replace → dir sync), `read.rs`/`schema.rs`/`validate.rs` (embedded pinned schema), `semantic/*` (rule registry with Implemented/Partial status and `rest_in` phase), `writer.rs` (`StoreWriter`), `recovery.rs`, `report_name.rs`, `fault.rs` (fault-injection doubles).
- **`application`** — `emit.rs` (event envelope), `dev_store.rs` (dev store commands, `DevError` classes → exit codes), `permission/` (`mod.rs` frozen types, `destination.rs` normalisation C6, `fingerprint.rs` v1 encoding, `evaluate.rs` R1–R8/G1–G14), `approvals.rs` (the only place that acts on `Permitted` and builds `Authorisation`), `child_env.rs` (profiles, `HostEnv`, `ChildEnv`, `ToolDirs`), `credential.rs` (`SessionCredentialStore`, `read_key`), `dev_permission.rs` (`dev permission-check` DTO).
- **`cli`** — `main.rs` (the only environment read), `lib.rs` (`Session`, `run_session`), `args.rs`, `commands.rs` (16 commands, exit table 0/2/3/4/5/6), `output.rs` (human + JSON Lines), `features.rs` (64-row feature index; a staleness test ties every row to a command and a test).
- **Frozen interfaces** of P03: `planning/P03-T01-request.md` §5 (incl. the 2026-10-31 §5.4 amendment), scope-match table §6, env policy §7. Changing them is a lead/owner decision, not a worker edit.

## 8. What comes next (needs the owner's go)
**Option A — P04 (Git acquisition/update; plan §4 row P04, 10–16 h + CLI 1–2 h):** URL validation (HTTPS/SSH only), clone and fast-forward update, unsafe-state detection (dirty, diverged, hooks, filters, submodules skipped with reason), per-repository locks, provenance, Git safety controls from the P00-T04 spike (`evidence/P00/P00-T04/record.md`: `env -i`, scratch HOME, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`, no inherited `GIT_CONFIG_COUNT`, fail-closed attributes), CLI `repo add|list|refresh|status|remove` (plan §4.1). First real callers of the P03 evaluator (`git_acquire`, `git_update` with destination grants) — use `ApprovalService::authorise` and the `git` child-env profile. Note: P04 is the first phase that spawns processes, but the process supervisor is P06; agree with the owner how Git is invoked before P06 (static rule "no `Command::new` until P06" must change by an approved decision). Private/loopback destinations and the P06 entry gate (approvals store integrity) are relevant here.
**Option B — CA-05 lead contract task** (domain analysis types, store schema 1.5, app contract 1.2, UI schema 1.2, rules D26-xx, migration notes) — must precede any CA-05-dependent implementation; proposed before P07. Needs explicit approval; O-AN-2/O-AN-6 can be ratified first.
Start either with: a plan section in `plan.md` + owner decisions, then T01 request (§6).

## 9. Known limitations and risks to keep in mind
See `status.md` "Carried forward" and `evidence/P03/P03-report.md` §8. Highest-impact: approvals store has no integrity binding (P06 gate); R7/R8 scope blindness (fails closed); best-effort key clearing only; the schedule exceeds the original four-week cap (B-08); PyPI provenance exception for yamllint/Checkov; cargo-audit DB last refreshed 2026-10-30 and its yanked check cannot run offline.

## 10. Lessons learned (P00–P03)
- Restoring a file with `cp -a` keeps the old mtime, so Cargo reuses a stale (mutant) build: always `touch` after a restore, and rebuild before using a binary for evidence (a D02 sweep was nearly taken on a mutant binary).
- Each slot target dir is per repo path *and* slot; a worker's `--repo <scratch copy>` creates extra target dirs (W1 left two in P03) — clean them up.
- The runner runs tests as a mapped root inside `unshare -rn`: permission-bit tests (mode 000) do not fail there; root-only branches need a uid-mapped manual run (`evidence/P02/manual-exit4/`, `evidence/P03/T12/fr33-process/`).
- Tests that hold a `flock` in-process race with sibling `fork`→`exec`; use bounded waits or a separate process.
- Spawn workers with a short prompt pointing to a packet file; give the exact base commit; do not use the harness `isolation` option with a lead-prepared worktree; check the main tree's `git status` after workers finish.
- Workers cannot message each other (no SendMessage in their tool set): the append-only coordination log is the channel, and it worked in waves 2 and 3.
- Reviewers find frozen-contract issues too (D01 M1 was lead code): review lead stubs with the same scrutiny; confirm every reviewer claim in code before assigning a fix.
- Keep a finding test `#[ignore]`d only until its debug cycle lands (the runner fails on ignored tests).
- The P02 `redact_static` rule cuts its argument at the first `)`: fixed texts must not contain parentheses.
- `--no-fail-fast` for test steps; negative-control scripts must restore with a fresh mtime.
- zsh has no `PIPESTATUS` (use `pipestatus`, or avoid pipes when you need the exit code).
- Never describe a checker as passing when it exits non-zero for known baseline reasons (CA-05 `check_docs.py`); report baseline vs new failures separately.
- Owner decisions are recorded verbatim with the date; approvals in one context do not extend to the next.

## 11. Evidence and history index
- Phase reports: `evidence/P00/P00-report.md`, `evidence/P01/P01-report.md`, `evidence/P02/P02-report.md`, `evidence/P03/P03-report.md`; freezes `evidence/P0N/P0N-freeze.json`.
- P03 baselines: `evidence/P03/{B0,B1,B2,B3}/`, T12 `evidence/P03/T12/`, cargo-audit `evidence/P03/cargo-audit-C-E/`.
- Debug cycles: `evidence/debug/` (index `evidence/debug/README.md`; outcomes in `plan.md` §9).
- CA-05: `planning/CA-05-*.md`, `planning/CA-05-draft/`, `evidence/CA-05-docs/record.md`.
- Clean-ups: `status.md` history archive; `evidence/cleanup-2026-11-01/`; archives under `~/.local/share/pwt-dev-tools/_archive/` (scratch tarballs; `git-unreachable-2026-11-01.bundle` — restore with `git fetch <bundle> 'refs/archive/*:refs/archive/*'`).

## 12. Not approved
Any phase after P03 (P04+), CA-05 contract application or implementation, real OpenRouter/API calls and any other network access, new dependencies or installs, pushes/merges not approved by the owner. Open decisions: `status.md`.

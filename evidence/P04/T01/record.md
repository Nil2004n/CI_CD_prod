# P04-T01 — request/design and local prerequisite verification

Date: 2026-11-03. Primary branch `phase/P04-git-acquisition`; base and unchanged HEAD `c1832672f12aa41129d882f4d58dc06331df490e`. Scope approved by owner: T01 only, GPT-6.1 Terra branch-session workers, main orchestration/review/testing, stop for acceptance, no commits/pushes. Exact owner words are preserved in status.md.

**Outcome: T01 request package prepared and reviewed, local prerequisite probes passed on this host; stopped for owner review. This is not an unconditional interface freeze or authorisation to implement.** Initial-ref and durable Git-version decisions remain explicit before dependent work. Safe-deletion guarantees and same-user race limits require acceptance or revision; future fixture proof remains mandatory.

## Deliverables and checklist

| T01 requirement | Deliverable/result |
|---|---|
| Module layout, ownership and interface proposals | planning/P04-T01-request.md §2, including RemoteSpec, runner, HardenedGit, prior OID, checkout guard/path, registry/service seams and error classes |
| Exact clone/update procedures and abort points | planning/P04-T01-git-safety.md; detached managed HEAD, explicit target fetch, attribute check, refusal boundary and config allowlist |
| Credential/transport/refusal policies | Request §§3–5 and safety annex; E3 direct helpers and fixed SSH policy, E5 production transport isolation remains future proof, E13 lexical destination refusal with DNS limitation |
| OpenSSH home behavior | PASS: synthetic IdentityAgent expands under passwd home despite different HOME; IdentityFile remains literal in dump. No real config, keys, agent or sockets read |
| Rust process_group + timeout | PASS: standalone edition-2024 Rust launch, default kills cooperative grandchild, foreground control leaves it alive, cleanup removes it; Rust 1.99.1 only |
| Test plan, packets and debug bounds | Request §7; six T05–T10 templates plus common rules; explicit future approvals/B0 required; no dispatch of unapproved tasks |
| Owner decisions/compatibility gaps | Request §8 and environment.json. No guessed default branch or invented provenance field |
| Continuity and stop | plan.md §5E.3 checkpoint, status.md and handover.md updated; T02+ unstarted |

## Lead verification

All commands executed by primary session, cwd `/home/debjoyy/Projects/CI_CD_prod` unless exact subprocess cwd is recorded in JSON. Revision above; working tree dirty with T01 docs/evidence plus previously requested .gitignore. Environments/argv/exits are recorded in the cited JSON. No product tests are attributed to worker reports.

| Check | Command/evidence | Outcome |
|---|---|---|
| Local SSH-home/Python process groups | `python3 -I -B evidence/P04/T01/probe_local.py` → local-probes.json | Exit 0; 3 pass, 0 fail, 0 inconclusive; default/control cleanup confirmed |
| Actual Rust process-group launch | `python3 -I -B evidence/P04/T01/run_rust_probe.py /home/debjoyy/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc` → rust-process-groups.json | Exit 0; compilation exit 0, 2 cases pass. Inner timeout exit 124 is expected for both cases |
| First Rust attempt | Same wrapper before compile allowance adjustment → rust-process-groups-attempt1.json | Exit 1: compile exceeded 15 seconds, no cases ran. Preserved; bounded retry with 60-second compile allowance succeeded. No product code/debug repair |
| Synthetic fixed SSH policy | Inline lead Python builds synthetic config and runs `ssh -G -F <synthetic-config> p04.invalid` → ssh-policy.json | ssh exit 0; 7 selected policy checks pass. Config dump only; not authentication/isolation proof |
| Environment readiness | Version/path and `unshare -rn true` checks → environment.json | Git 2.53.0, SSH 10.6p1, Python 3.14.8, Rust/Cargo 1.99.1; namespace creation exit 0; old tool/cache paths missing |
| Delivery/integrity | Lead Python SHA256/AST/JSON/file checks plus `git diff --check` → delivery-checks.json | 18 pass, 0 fail; all 147 protected source/schema/manifests unchanged; HEAD unchanged |

No full Cargo regression/build/audit run: T01 changes documentation/probe evidence only; the old runner is pinned to absent /home/kali paths and Rust 1.97.1, and the offline dev-tool cache is missing. Historical 1138 passing tests/41 compile-fail are a preserved floor, not a current-host rerun. No dependencies installed, no network or real credentials used. Standalone rustc probe compiled into a temporary directory only, with no Cargo or package downloads.

## Review corrections

Two workers performed read-only cross-reviews after drafting. Primary review found missing prior-pin and checkout-path seams, environment dependency-cycle risk, inaccessible fake outputs, contradictory stdin/caps/ownership, and incomplete target-fetch/config policies. The request/annex and packets were revised within T01 documentation scope. Final review confirmed material R1 corrections; remaining path/wording mismatches were corrected by primary integration. No product debug cycle was opened and no product source was changed. Worker assignments/model configuration are in assignments.md; resolved model identity is not independently exposed.

Proposals deliberately retain explicit limits: lexical destination checks cannot prevent DNS rebinding; configured user credential helpers execute trusted user code; process groups do not stop setsid escape; same-user races are not eliminated; fetch may modify Git metadata even when worktree/HEAD stay unchanged; real SSH/HTTPS and hostile P04 fixtures remain unverified. No broad security or functional acceptance claim is made.

## Owner review and next action

Review planning/P04-T01-request.md §8. Initial-ref choice: explicit required ref flags (CLI change) versus a separately approved optional/unresolved stored ref (CA-06). Durable version choice: separately approved schema field (CA-06) versus explicitly reduced durable-provenance acceptance. Recommended direction is to preserve URL-only add through an honest unresolved-ref contract amendment and add durable Git-version storage in the same separately reviewed amendment; no amendment has been applied. Safe deletion remains fail-closed with stated same-user race limits and must pass later confinement/mount fixtures. Environment restoration or supported runner portability also needs a concrete approved plan before baseline verification, with installs separately approved if needed.

**STOP. Await owner acceptance/revisions. No T02–T12 execution, commit, push or merge is authorised by this checkpoint.** Worker worktrees are retained with uncommitted drafts for inspection/recovery; no branch is deleted.

## Owner approval and T01 close

On 2026-11-03 the owner responded “okay i approve” to the request to approve or revise the proposed contract changes. The initial-ref discovery and durable Git-version design is approved and frozen as the T01 baseline; see `planning/P04-T01-request.md` §10. Schema version and migration ordering remain for T03 due to CA-05. No canonical schema or product implementation was changed by this approval. T01 complete; stop before T02. No commits or pushes.

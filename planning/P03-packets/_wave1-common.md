# Wave 1 — rules common to T05 (W1), T06 (W2), T07 (W3)

Issued by Opus 2026-10-30 under the owner's approval of wave 1 ("I approve wave 1, start T05 T06 T07"). Base: `phase/P03-permissions-credentials` at `d0ae1716c385a5633bf110555db7716a5a19536c` (= B0 `5e29908` code + docs-only commits). Quality bar: production.

## Step 0
`cd` into your worktree. `git rev-parse HEAD` = `d0ae1716…` and `git status --short` is empty. Otherwise STOP.

## Read first
- `planning/P03-T01-request.md`: §1 facts, §3 layout, §5 frozen interfaces (your section), §6/§7/§8 as named in your packet, §9.
- `CLAUDE.md` invariants. The frozen stub you own and `application/src/permission/mod.rs`, `application/src/lib.rs`.
- An existing test file for style, e.g. `workbench/crates/application/tests/emit.rs`.

## Frozen
Names, signatures, derives and visibility of every public item in the stubs (§5). Everything outside your owned files: other workers' files, `domain`, `persistence`, `cli`, manifests, `Cargo.lock`, `dev/`, `planning/`, docs. You may add **private** helpers and private submodule items inside your owned files. If a frozen item blocks you, STOP and report; do not work around it.

## Engineering rules
- Remove every `todo!` and its `#[expect(clippy::todo, …)]` in your owned files. No new `#[allow]`/`#[expect]` (the static rule compares against an approved list). No `unsafe`, no `unwrap`/`expect` in production code if the workspace lints forbid them (check `workbench/Cargo.toml` `[workspace.lints]`). No environment reads, no `std::process::Command`, no new dependencies.
- Every new assertion must be able to fail. For each test group, the report names the code change that would make it fail.
- Fixture secrets are synthetic and obviously fake (e.g. `pwt-test-FAKE-…`). Never print a key value in an assertion message.
- Leave edits **unstaged** (new untracked files are fine). Never `git add/commit/stash/checkout/reset`.
- No network, no installs. Cargo only through the runner.

## Permitted commands (cwd = your worktree)
- `python3 -I -B dev/p03_check.py <fmt-fix|fmt|clippy|test|build|static|cli|all> --slot <your slot> --out <your scratch>/<new dir> [--package application]`
- `git status`, `git diff`, `git log`, `git rev-parse`; read-only inspection anywhere in the repo.
- `cp -a`, `diff`, `rm -rf`, `mkdir` inside your scratch directory only; `python3 -I` one-off scripts in your scratch (e.g. independent golden-vector computation).

The first Cargo run builds offline from scratch (≈10–20 min; three workers build in parallel on 5 cores). Use `--package application` for quick iterations, then `all` at the end.

## Stop condition
`python3 -I -B dev/p03_check.py all --slot <slot> --out <scratch>/all-final` is OVERALL PASS (non-`--final`: other workers' stubs still hold `todo!`), `grep -n 'todo!' <your owned src files>` is empty, and your tests cover your packet's list — or you hit a blocker. Then report.

## Report (final message)
1. `git status --short` and `git diff --stat`.
2. Per owned file: what you implemented (short).
3. Every command with exit code; the final `all` PASS/FAIL lines; test counts (passed/failed/ignored, compile_fail) and how many tests you added (per file).
4. Test-to-requirement map (packet items → test names) and, per group, what would make it fail.
5. Deviations, open questions, limitations.
6. "configured: claude-sonnet-5-5 (unverified self-report)". Do not claim acceptance.

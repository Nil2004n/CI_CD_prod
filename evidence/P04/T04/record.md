# P04-T04 interrupted verification — 2026-11-03

Base `d40ad71ce72724073de4e86f5d53c20bf29ea7b5`; dirty integration checkout on `phase/P04-git-acquisition`. Owner authorized T04 and prohibited commit/push. Three requested `gpt-6.1-terra` workers drafted disjoint scaffold files in isolated worktrees; resolved identities and worker reports are not independently verified. Primary integrated the drafts and owns shared files.

Host: Rust/Cargo 1.99.1, Git 2.53.0. No network, installs, real data, commit or push. The P03 runner's `/home/kali` toolchain and dev-tool paths are absent. The following checks do not establish B0:

| Command | Cwd | Exit | Result |
| --- | --- | --- | --- |
| `cargo test --offline --locked -p domain` | `workbench` | 0 | Domain tests and doctests passed; counts were not captured in a durable complete log. This ran before the worker scaffolds were integrated. |
| `python3 -I -B dev/p04_check.py static --slot lead --out /tmp/p04-t04-static` | repository root | 1 | Failed before repository and service drafts were integrated. See `static-summary.json`: unsafe-word lexical false positives and P03 lock/dependency rules reject the P04 manifest edge. Not a final integrated check. |
| `cargo check --offline --locked --workspace` | `workbench` | 0 | Integrated scaffold compiles with four missing-Debug warnings in application; not clippy, test, build or B0. |

The static failures require an owner-approved debug cycle before correction. The runner's lexical rules cannot prove fixture feature isolation or the complete Git call graph. P04 runner environment isolation/toolchain needs review on this host; full regression, static rerun, clippy, build, CLI, fixture release isolation and applicable security checks remain pending. No real-remote check is authorized. T04 is incomplete; no B0 is claimed.

## Owner-approved debug cycle

Owner approved corrections to the reported static failures and scaffold warnings. The lock hash and dependency allowlist now reflect the exact T02 edge-only delta; the unsafe-code scan masks literals and retains a negative-control self-test. Debug implementations for the application scaffold avoid disclosing service internals.

| Command | Cwd | Exit | Result |
| --- | --- | --- | --- |
| `python3 -I -B dev/p04_check.py static --slot lead --out /tmp/p04-t04-debug-static-final` | repository root | 0 | Integrated static checks and self-tests pass; `debug-static-summary.json`. Lexical limits remain. |
| `cargo fmt --all -- --check` | `workbench` | 0 | Passed with installed Rust 1.99.1. |
| `python3 -I -B dev/p04_check.py env --slot lead --out /tmp/p04-t04-env` | repository root | 1 | Pinned `/home/kali` binaries absent; network-namespace probe also failed in the runner environment. See `debug-env-summary.json`; separate `unshare -rn true` succeeded. |
| `unshare -rn cargo clippy --offline --locked --workspace --all-targets -- -D warnings` | `workbench` | 101 | Rust 1.99.1 reports `clippy::useless_format` in existing `crates/domain/tests/store_models_a.rs:1845`; no change made to that test. |

The existing pinned toolchain is absent and the newer toolchain reveals a separate clippy failure; no compliant B0 is claimed. A fresh owner decision is required for those distinct issues. Static checks do not prove fixture feature isolation or the complete call graph.

`unshare -rn cargo test --offline --locked --workspace` in `workbench` exited 101 before tests ran: the project filesystem reached 100% capacity, with explicit `No space left on device` and linker bus errors. Log: `/tmp/p04-t04-debug-tests.log` (temporary); `workbench/target` measured 4.6 GiB and includes pre-existing artifacts, preserved. No regression count is claimed.

The owner subsequently authorized removal of `workbench/target/`. `rm -rf workbench/target` exited 0; `df -h .` then showed 4.6 GiB available. Application source and Cargo's dependency cache were not removed. Only the installed `stable` Rust 1.99.1 toolchain is listed by `rustup toolchain list`; 1.97.1 is not installed. The Clippy finding is an existing `json!(format!("{MARKER}"))` expression at `crates/domain/tests/store_models_a.rs:1845`; replacing the redundant formatting with the same string value is a proposed fix, not yet authorized or applied. Runner adaptation or a pinned-toolchain install requires a separate decision.

## Host adaptation and retest

The owner approved the proposed Clippy expression change and runner adaptation to Rust 1.99.1. The marker value remains the same. Runner uses direct installed toolchain binaries, an isolated copy of the existing Cargo registry under `/tmp/p04-t04-devtools`, `env -i`, `unshare -rn`, `--frozen`, and debug-info-disabled development/test builds to conserve space. The network probe reports errno 1: a separate probe confirmed that the sandbox denies socket creation before any connect, so this does not independently establish network-namespace isolation.

`python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/host-run` (repository root, dirty base d40ad71) exited 1: environment, clippy, build, and static passed; test passed 1138/0/0 across 54 suites with 41 compile-fail cases; fmt initially failed due to formatting of the test edit; CLI failed 100/1/0 across five suites. `a_closed_stdout_keeps_permission_check_in_the_safe_direction` expected exit 5 but observed exit 3 at `p03_cli.rs:2689`; cause unconfirmed. Logs and summary are in `host-run/`.

After formatting that same test edit, `python3 -I -B dev/p04_check.py fmt --slot lead --out evidence/P04/T04/host-fmt-retest` exited 0. A separate, unchanged full CLI retest using `python3 -I -B dev/p04_check.py cli --slot lead --out evidence/P04/T04/host-cli-retest` exited 0 with 101/0/0 across five suites. The earlier failure remains unexplained; do not represent the combined run as passing or claim B0. Further diagnosis/fix of the distinct CLI failure requires owner approval. Fixture-feature isolation, full call-path proof, and T04 policy/CLI seam completeness remain pending.

## Owner-approved CLI debug cycle

The failed assertion received store-locked exit 3. The existing test helper documents a transient test-only lock inheritance window during parallel process spawn and retries exit 3 for up to 10 seconds. The closed-stdout test bypassed that helper. This is a plausible explanation, not a reproduced or independently proven cause. The test now uses the same bounded retry only for exit 3; its expected exit 5 and grant-consumption assertions remain intact. No production behavior changed.

`python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/host-debug2` (repository root, dirty base d40ad71, Rust/Cargo 1.99.1, isolated copied cache, `unshare -rn`, `--frozen`, dev/test debug info disabled) exited 0: env, fmt, clippy, build, static passed; workspace tests 1138/0/0 across 54 suites and 41 compile-fail cases; CLI tests 101/0/0 across five suites. Full logs and machine-readable results are in `host-debug2/`. Socket creation is blocked by the parent sandbox, so the network probe does not independently establish netns isolation. The Cargo feature tree was inspected, but the draft feature has no transport implementation and the requested negative-control release isolation proof is still pending. T04 remains incomplete: the Git child-environment E3 policy amendment and CLI seams are not implemented, and static lexical checks do not prove the complete call graph. No B0 or phase acceptance is claimed.

## Subsequent T04 continuation

Owner authorized continuation. The Git child environment now permits only the exact `SSH_AUTH_SOCK` name for Git, subject to absolute, NUL-free, no-parent-component path and secret checks; tests cover invalid values and other profiles. A typed CLI repository-command seam was added without parser or execution wiring. `python3 -I -B dev/p04_check.py test --package application --slot lead --out evidence/P04/T04/e3-test` exited 0: 255/0/0, 2 compile-fail cases.

The integrated `python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/host-final-attempt` exited 1: environment, test (1139/0/0, 41 compile-fail), build, CLI (101/0/0) and static passed. Formatting failed on the new CLI seam and clippy failed on `collapsible_if` in `child_env.rs`. The owner must approve a bounded debug cycle before fixes; no B0 claimed. Feature release isolation, the full call-path guarantee, remaining E3 fixed SSH policy and independent network namespace proof remain pending. At completion `/tmp` had 956 MiB free; workspace filesystem had 4.6 GiB free at start.

Owner approved the named formatting and Clippy fixes. The CLI seam was formatted and the conditional collapsed without changing the socket validation. `python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/host-debug3` (repository root, dirty base d40ad71, Rust 1.99.1, isolated cache, no network, `unshare -rn`, debug info disabled) exited 0: env, fmt, clippy, build and static passed; 1139/0/0 workspace tests, 41 compile-fail and 101/0/0 CLI tests. See `host-debug3/` for logs. The sandbox blocks socket creation; the runner probe does not independently prove network namespace isolation. The fixed SSH policy, release-feature negative controls and complete Git call-path guarantee remain pending; no B0 claimed.

## Completion attempt and release-feature gate

Owner requested completion. A `FixedSshPolicy` seam was added which always refuses construction; the Git environment and process runner also refuse construction, so no SSH process can run at this scaffold checkpoint. A local test verifies the refusal. The complete trusted SSH configuration belongs to the later Git-runner implementation; no SSH isolation or real authentication is claimed here.

The runner's environment check now compares parent and `unshare -rn` network namespace identities without an external connection; the identities differ. This establishes a distinct namespace, not a complete network-security test. `python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/host-finish` exited 0: env, fmt, clippy, build and static passed; 1140/0/0 workspace tests, 41 compile-fail, 101/0/0 CLI tests.

Release negative-control: from `workbench`, in `unshare -rn` and an isolated Cargo home, `cargo rustc --frozen --release -p repositories --lib -- --print cfg` exited 0 and did not list `fixture-transport`; the same command with `--features fixture-transport` exited 0 and did list it. Output in `release-default-cfg.txt` and `release-feature-cfg.txt`. Thus Cargo can enable that feature in a release build; it is empty today and contains no fixture transport implementation, but this does not satisfy the approved release-unreachability gate for any future implementation. T04 B0 is not established. The approved design says to block fixture transport or revise the design with owner approval; a bounded fix must remove the unsafe feature path or establish a genuinely separate test-only design with negative controls. Static Git call-path enforcement also remains incomplete and lexical checks are not a proof.

## Owner-approved fixture isolation correction

The owner authorized the proposed removal of the empty release-reachable feature and self-dev-dependency. The manifest and lockfile now have no fixture feature or self-edge. The runner rejects its reintroduction in workspace manifests and checks additional direct runner references, with negative controls. A manual search of production source found no process invocation; runner construction and SSH policy construction fail closed. This is evidence for the current scaffold, not proof of future code or a substitute for T06/T08 review.

From `workbench`, `cargo metadata --offline --locked --format-version 1 --no-deps` exited 0 and reports no `repositories` features. `cargo rustc --offline --locked --release -p repositories --lib --features fixture-transport -- --print cfg` exited 101 as expected, explicitly reporting that the feature does not exist (`fixture-release-refusal.log`). Default release cfg check with isolated Cargo home and `unshare -rn` exited 0 (`fixture-release-default.log`). No fixture transport exists; future implementation requires a separate reviewed test-only isolation design.

`python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/fixture-debug-all` (repository root, base d40ad71 dirty, Rust/Cargo 1.99.1, isolated offline cache and namespace, dev/test debug info disabled) exited 0: env, fmt, clippy, build and static passed; 1140/0/0 workspace tests across 56 suites with 41 compile-fail cases, and 101/0/0 CLI tests across five suites. See `fixture-debug-all/` for exact commands and logs. The namespace identity check is not an independent external network probe. B0's scaffold is verified locally but remains an uncommitted dirty checkpoint; T05–T12, actual Git operations, full SSH policy, migration and real-remote checks remain outside this result. No commit, push or phase acceptance is claimed.

## Final T04 review

At owner request, primary reviewed the integrated dirty checkout at `d40ad71` and reran `python3 -I -B dev/p04_check.py all --final --slot lead --out evidence/P04/T04/final-review` from repository root. Exit 0: env/fmt/clippy/test/build/CLI/static pass; workspace 1140 passed, 0 failed, 0 ignored across 56 suites and 41 compile-fail cases; CLI 101 passed, 0 failed, 0 ignored across five suites. Toolchain Rust/Cargo 1.99.1, isolated offline copied Cargo cache, `unshare -rn`, debug info disabled; logs and summary in `final-review/`. `git diff --check` exited 0. The namespace identity check shows a distinct namespace but is not an external connection test. Default release feature exclusion and explicit feature rejection are recorded above; no fixture transport has been implemented. `cargo-audit` and the former advisory database were not found on this host, so no current advisory scan is claimed.

The current CLI seam is not wired and the repository service, Git environment, SSH policy and Git runner deliberately fail closed. The approved CA-06/approval draft has not been applied to canonical contracts; legacy refs and journal design remain gates. No real HTTPS/SSH, migration, helper, hostile-repository, deletion race or future implementation security claim is supported by this scaffold check. `status.md` and `handover.md` contain superseded checkpoint paragraphs; their newest checkpoint governs. Final review leaves all changes uncommitted and unpushed for owner review; no next-wave authorization inferred. `/tmp` had approximately 480 MiB free after verification.

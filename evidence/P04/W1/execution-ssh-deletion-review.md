# P04 wave 1 — execution, SSH and deletion continuation

2026-11-04. Owner requested completion of Git execution, SSH and checkout deletion with `gpt-6.1-terra` workers in separate branches; no commit or push. Base `f71749c53704afbd87150ac6d31b65f69377000b`; integrated tree dirty. Requested worker model identities and self-reports are not independently verified.

## Git execution candidate

The runner worker drafted a version-only constructor and supervisor in `/tmp/p04-w1-resume-runner/workbench/crates/repositories/src/git/runner.rs`. It is not integrated: `GitRunner::run` remains refused in the candidate. An independent read-only reviewer identified cleanup and proof gaps in an earlier draft, including an error path that could leave a child running, an unbounded helper wait, and descendants surviving after closing output. The worker revised the draft, but the revision has not passed independent review. It uses `/proc` to observe an unreaped leader and a detached reaper on failure, whose lifecycle and error behavior need review.

Primary package check outside the filesystem sandbox, still `unshare -rn`, `env -i` and frozen Cargo, exited 1: 13 passed, 5 failed, 0 ignored (`version-runner-candidate/`). All five failures occur while constructing the pinned kill capability because its root-ownership requirement is not met in this user namespace; they do not test supervisor behavior. Do not weaken production ownership checks. A proposed bounded debug is to provide a unit-test-only synthetic trusted-executable fixture and potency controls, leave production constructors unchanged, review error cleanup, and rerun tests; any positive production trust test needs a suitable isolated environment.

## SSH policy candidate

The SSH worker drafted `git/ssh.rs`, module export and synthetic tests in `/tmp/p04-w1-resume-remote`. Primary package check passed 16/0/0 (`ssh-policy-candidate/`); this does not test a successful policy construction, OpenSSH's effective behavior or runner integration. Independent review found same-user path/in-place substitution limits, no positive end-to-end policy test, no local `ssh -G` behavior and no proof of binding to a cleared Git environment and validated repository configuration. Existing runner `FixedSshPolicy` is unchanged and refuses. Candidate is not integrated or security accepted.

## Checkout deletion

The deletion worker found the current safe standard-library primitives insufficient to prove descriptor-relative no-follow traversal, mount confinement and replacement-race safety. `libc` is in the lockfile but safe syscall wrappers (`rustix`, `nix`, `cap-std`, `openat`) were not found in the existing local registry; the project prohibits unsafe code and new dependencies or installs need separate approval. The existing deletion adapter remains refused. A candidate design is a reviewed safe syscall wrapper/native adapter with descriptor-relative open/traversal/unlink, no-follow and no-cross-mount controls, identity checks, partial-failure handling, and adversarial race and mount tests. Specific dependency/version and its offline availability are not yet established; no lookup or install was performed.

No network, installs, real credentials/data, commit, push or production Git/SSH/deletion operation was performed. T05–T07 and P04 remain incomplete; no B1 or security acceptance is claimed.

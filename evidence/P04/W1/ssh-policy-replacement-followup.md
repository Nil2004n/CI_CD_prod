# P04 W1 SSH policy replacement follow-up

2026-11-04, dirty `phase/P04-git-acquisition` at base `f71749c53704afbd87150ac6d31b65f69377000b`. A requested `gpt-6.1-terra` worker added synthetic trusted-policy replacement tests; resolved identity and self-report are unverified. Primary integrated the test changes and verified them as follows from `workbench`, using offline locked Cargo and no real credentials.

- Managed sandbox `cargo test --offline --locked -p repositories --test p04_ssh`: 2 passed, 2 failed with EPERM creating Unix sockets, 2 ignored.
- Outside sandbox under `unshare -rn` and a 30-second outer timeout: 4 passed, 0 failed, 2 ignored. The ignored tests require explicitly provisioned trusted host paths.
- First trusted-host fixture run under `unshare -rn` failed at policy construction (0 passed, 1 failed, 5 filtered); the namespace does not expose the trusted executable ownership required by the policy. No trust check was weakened.
- Separate fresh private synthetic host fixtures under `/home/debjoyy`, outside the sandbox without a network namespace, with offline Cargo and no network operation: each of the two ignored tests passed individually (1/0/0, 5 filtered each). Fixtures were removed. These verify in-place config modifications and mode changes, config symlink replacement, known-hosts inode replacement and agent socket replacement on this host.
- `cargo clippy --offline --locked -p repositories --test p04_ssh -- -D warnings`, `cargo fmt --all -- --check`, and root `git diff --check` passed.

These checks do not establish SSH authentication, passwd-home isolation in an actual connection, resistance to concurrent same-user changes, or integration into a production Git runner. SSH execution remains refused. No full phase check, B1, acceptance, commit or push.

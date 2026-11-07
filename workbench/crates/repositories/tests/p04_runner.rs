use repositories::git::runner::{GitError, GitStdin, STDIN_CAP};

#[test]
fn stdin_enforces_the_byte_cap_and_masks_debug() {
    assert!(GitStdin::bytes(vec![0; STDIN_CAP]).is_ok());
    assert!(matches!(
        GitStdin::bytes(vec![0; STDIN_CAP + 1]),
        Err(GitError::InvalidInvocation)
    ));
    let input = GitStdin::bytes(b"sensitive-input".to_vec()).unwrap();
    assert_eq!(format!("{input:?}"), "GitStdin(redacted)");
    assert_eq!(format!("{:?}", GitStdin::null()), "GitStdin(redacted)");
}

fn approved_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    [
        ("PATH", "/usr/bin:/bin"),
        ("LANG", "C.UTF-8"),
        ("HOME", "/app/home"),
        ("XDG_CONFIG_HOME", "/app/config"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_ATTR_NOSYSTEM", "1"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}

#[test]
fn environment_rejects_unapproved_duplicate_and_unsafe_values() {
    use repositories::git::runner::GitEnv;
    assert!(GitEnv::from_approved_pairs(approved_env()).is_ok());
    for (name, value) in [
        ("GIT_SSH_COMMAND", "secret"),
        ("HTTP_PROXY", "secret"),
        ("SSH_AUTH_SOCK", "relative"),
        ("SSH_AUTH_SOCK", "/a/../b"),
        ("SSH_AUTH_SOCK", "/a\0b"),
        ("PATH", "/tmp"),
    ] {
        let mut pairs = approved_env();
        pairs.push((name.into(), value.into()));
        assert!(matches!(
            GitEnv::from_approved_pairs(pairs),
            Err(GitError::InvalidInvocation)
        ));
    }
    let mut missing = approved_env();
    missing.pop();
    assert!(matches!(
        GitEnv::from_approved_pairs(missing),
        Err(GitError::InvalidInvocation)
    ));
    let env = GitEnv::from_approved_pairs(approved_env()).unwrap();
    assert_eq!(format!("{env:?}"), "GitEnv(redacted)");
}

#[test]
fn runner_and_ssh_remain_closed_without_trust_and_policy_proofs() {
    use repositories::git::runner::{FixedSshPolicy, GitEnv, ProcessGitRunner};
    use std::path::PathBuf;

    assert!(matches!(
        FixedSshPolicy::new(
            PathBuf::from("/app/ssh_config"),
            PathBuf::from("/app/known_hosts")
        ),
        Err(GitError::InvalidInvocation)
    ));
    let env = GitEnv::from_approved_pairs(approved_env()).unwrap();
    assert!(matches!(
        ProcessGitRunner::new(
            PathBuf::from("/usr/bin/git"),
            PathBuf::from("/usr/bin/timeout"),
            env
        ),
        Err(GitError::InvalidInvocation)
    ));
}

#[test]
fn environment_accepts_only_the_exact_optional_agent_name_and_safe_path() {
    use repositories::git::runner::GitEnv;
    let mut pairs = approved_env();
    pairs.push(("SSH_AUTH_SOCK".into(), "/app/agent.sock".into()));
    assert!(GitEnv::from_approved_pairs(pairs).is_ok());
    for name in ["SSH_AGENT_PID", "SSH_AUTH_SOCK_EXTRA", "ssh_auth_sock"] {
        let mut pairs = approved_env();
        pairs.push((name.into(), "/app/agent.sock".into()));
        assert!(matches!(
            GitEnv::from_approved_pairs(pairs),
            Err(GitError::InvalidInvocation)
        ));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn trust_capabilities_refuse_unpinned_roles_and_nonabsolute_directories() {
    use repositories::git::runner::{ExecutableKind, TrustedExecutable, TrustedNonRepositoryDir};
    use std::path::PathBuf;

    assert!(matches!(
        TrustedNonRepositoryDir::new(PathBuf::from("relative")),
        Err(GitError::InvalidInvocation)
    ));
    for (path, role) in [
        ("/usr/bin/timeout", ExecutableKind::Git),
        ("/usr/bin/kill", ExecutableKind::Timeout),
        ("/usr/bin/git", ExecutableKind::Kill),
    ] {
        assert!(matches!(
            TrustedExecutable::new(PathBuf::from(path), role),
            Err(GitError::InvalidInvocation)
        ));
    }
}

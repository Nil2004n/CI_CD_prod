#![cfg(target_os = "linux")]
use repositories::git::ssh::TrustedSshPolicy;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path =
            std::env::temp_dir().join(format!("pwt-ssh-policy-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self(path))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn app_authored_config_has_fixed_isolation_options() {
    let bytes = TrustedSshPolicy::config_bytes(
        &PathBuf::from("/trusted/known_hosts"),
        &PathBuf::from("/trusted/agent.sock"),
    )
    .unwrap();
    assert_eq!(bytes, b"Host *\n    IdentityFile none\n    IdentitiesOnly no\n    IdentityAgent /trusted/agent.sock\n    UserKnownHostsFile /trusted/known_hosts\n    GlobalKnownHostsFile /dev/null\n    ProxyCommand none\n    ProxyJump none\n    PermitLocalCommand no\n    ForwardAgent no\n    BatchMode yes\n    StrictHostKeyChecking yes\n");
}

#[test]
fn unsafe_paths_and_missing_trust_fail_closed() {
    for path in [
        "relative",
        "/tmp/../known",
        "/tmp/with space",
        "/tmp/$(touch-marker)",
        "/tmp/%h",
    ] {
        assert!(
            TrustedSshPolicy::config_bytes(&PathBuf::from(path), &PathBuf::from("/trusted/agent"))
                .is_err()
        );
    }
    assert!(
        TrustedSshPolicy::new(
            PathBuf::from("/missing/config"),
            PathBuf::from("/missing/known_hosts"),
            PathBuf::from("/missing/agent")
        )
        .is_err()
    );
}

#[test]
fn policy_rejects_writable_ancestor_and_symlinked_inputs() {
    let fixture = Fixture::new().unwrap();
    let config = fixture.0.join("config");
    let known = fixture.0.join("known_hosts");
    let agent = fixture.0.join("agent");
    std::fs::write(&known, b"synthetic host key\n").unwrap();
    let _listener = UnixListener::bind(&agent).unwrap();
    std::fs::write(
        &config,
        TrustedSshPolicy::config_bytes(&known, &agent).unwrap(),
    )
    .unwrap();
    // /tmp is writable by other users: valid-looking leaf files do not make it trusted.
    assert!(TrustedSshPolicy::new(config.clone(), known.clone(), agent.clone()).is_err());
    let link = fixture.0.join("link");
    symlink(&known, &link).unwrap();
    assert!(TrustedSshPolicy::new(config, link, agent).is_err());
}

// Run explicitly with a private fixture directory under trusted ancestors and a
// host view where /usr/bin/ssh has its real root ownership. It is ignored in
// ordinary runs rather than silently passing when those prerequisites are absent.
#[test]
#[ignore = "requires an explicitly provisioned trusted host fixture"]
fn trusted_policy_constructs_and_rejects_replacement() {
    let root = PathBuf::from(std::env::var_os("P04_SSH_TRUST_FIXTURE").expect("fixture path"));
    let config = root.join("config");
    let known = root.join("known_hosts");
    let agent = root.join("agent");
    std::fs::write(&known, b"").unwrap();
    let _listener = UnixListener::bind(&agent).unwrap();
    std::fs::write(
        &config,
        TrustedSshPolicy::config_bytes(&known, &agent).unwrap(),
    )
    .unwrap();
    for path in [&config, &known, &agent] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let policy = TrustedSshPolicy::new(config.clone(), known, agent).unwrap();
    assert!(policy.revalidate().is_ok());
    assert_eq!(
        policy.command().unwrap(),
        format!(
            "/usr/bin/ssh -F {} -o BatchMode=yes -o StrictHostKeyChecking=yes",
            config.display()
        )
    );
    // An in-place edit retains the inode, so identity checks alone are insufficient.
    std::fs::write(&config, b"Host *\n    StrictHostKeyChecking no\n").unwrap();
    assert!(policy.revalidate().is_err());
    std::fs::write(
        &config,
        TrustedSshPolicy::config_bytes(&root.join("known_hosts"), &root.join("agent")).unwrap(),
    )
    .unwrap();
    assert!(policy.revalidate().is_ok());
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o666)).unwrap();
    assert!(policy.command().is_err());
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(policy.revalidate().is_ok());
    std::fs::rename(&config, root.join("old_config")).unwrap();
    symlink(root.join("old_config"), &config).unwrap();
    assert!(policy.revalidate().is_err());
}

#[test]
#[ignore = "requires an explicitly provisioned trusted host fixture"]
fn trusted_policy_rejects_known_hosts_and_agent_replacement() {
    let root = PathBuf::from(std::env::var_os("P04_SSH_TRUST_FIXTURE").expect("fixture path"));
    let config = root.join("config");
    let known = root.join("known_hosts");
    let agent = root.join("agent");
    std::fs::write(&known, b"").unwrap();
    let listener = UnixListener::bind(&agent).unwrap();
    std::fs::write(
        &config,
        TrustedSshPolicy::config_bytes(&known, &agent).unwrap(),
    )
    .unwrap();
    for path in [&config, &known, &agent] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let policy = TrustedSshPolicy::new(config, known.clone(), agent.clone()).unwrap();
    std::fs::rename(&known, root.join("old_known_hosts")).unwrap();
    std::fs::write(&known, b"").unwrap();
    std::fs::set_permissions(&known, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(policy.revalidate().is_err());
    std::fs::remove_file(&known).unwrap();
    std::fs::rename(root.join("old_known_hosts"), &known).unwrap();
    assert!(policy.revalidate().is_ok());
    std::fs::rename(&agent, root.join("old_agent")).unwrap();
    let replacement = UnixListener::bind(&agent).unwrap();
    std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(policy.revalidate().is_err());
    drop((listener, replacement));
}

#[test]
fn openssh_effective_config_suppresses_hostile_user_config_and_default_identities() {
    let fixture = Fixture::new().unwrap();
    let home = fixture.0.join("home");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir(home.join(".ssh")).unwrap();
    let hostile_config = home.join(".ssh/config");
    std::fs::write(&hostile_config, b"Host *\n  ProxyCommand touch /tmp/pwt-ssh-should-not-run\n  IdentityFile /tmp/pwt-hostile-identity\n  StrictHostKeyChecking no\n").unwrap();
    // The control proves these synthetic settings are effective when explicitly loaded.
    let control = Command::new("/usr/bin/ssh")
        .env_clear()
        .env("HOME", &home)
        .args(["-G", "-F"])
        .arg(&hostile_config)
        .arg("example.invalid")
        .output()
        .unwrap();
    assert!(control.status.success());
    let control_text = String::from_utf8(control.stdout).unwrap();
    assert!(
        control_text
            .lines()
            .any(|line| line == "identityfile /tmp/pwt-hostile-identity")
    );
    assert!(
        control_text
            .lines()
            .any(|line| line == "proxycommand touch /tmp/pwt-ssh-should-not-run")
    );
    assert!(
        control_text
            .lines()
            .any(|line| line == "stricthostkeychecking false")
    );
    let known = fixture.0.join("known_hosts");
    let agent = fixture.0.join("agent");
    let config = fixture.0.join("config");
    std::fs::write(&known, b"").unwrap();
    let _listener = UnixListener::bind(&agent).unwrap();
    std::fs::write(
        &config,
        TrustedSshPolicy::config_bytes(&known, &agent).unwrap(),
    )
    .unwrap();
    // -G prints effective configuration and does not connect or authenticate.
    let output = Command::new("/usr/bin/ssh")
        .env_clear()
        .env("HOME", &home)
        .args(["-G", "-F"])
        .arg(&config)
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "example.invalid",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let has = |line: &str| text.lines().any(|item| item == line);
    assert!(has("identityfile none"));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("identityfile "))
            .count(),
        1
    );
    assert!(has("identitiesonly no"));
    assert!(has(&format!("identityagent {}", agent.display())));
    assert!(has(&format!("userknownhostsfile {}", known.display())));
    assert!(has("globalknownhostsfile /dev/null"));
    assert!(
        has("proxycommand none") || !text.lines().any(|line| line.starts_with("proxycommand "))
    );
    assert!(has("proxyjump none") || !text.lines().any(|line| line.starts_with("proxyjump ")));
    assert!(has("permitlocalcommand no"));
    assert!(has("forwardagent no"));
    assert!(has("batchmode yes"));
    assert!(has("stricthostkeychecking true"));
    assert!(!text.contains("pwt-hostile-identity"));
    assert!(!text.contains("pwt-ssh-should-not-run"));
}

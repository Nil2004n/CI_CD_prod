//! The P04 Git-only SSH agent exception remains narrowly scoped and secret checked.
use application::child_env::{HostEnv, SecretProbe, ToolDirs, ToolProfile, build};
use std::ffi::OsString;
use std::path::Path;

struct Probe;
impl SecretProbe for Probe {
    fn appears_in(&self, value: &[u8]) -> bool {
        value.windows(6).any(|w| w == b"SECRET")
    }
}

#[test]
fn agent_socket_is_inherited_only_for_git_and_only_when_valid() {
    let dirs = ToolDirs::new(
        Path::new("/a"),
        Path::new("/b"),
        Path::new("/c"),
        Path::new("/d"),
    )
    .unwrap();
    for (value, accepted) in [
        ("/tmp/agent.sock", true),
        ("relative", false),
        ("/tmp/../socket", false),
        ("/tmp/SECRET", false),
    ] {
        let host = HostEnv::from_pairs([(OsString::from("SSH_AUTH_SOCK"), OsString::from(value))]);
        for profile in ToolProfile::ALL {
            let env = build(profile, &dirs, &host, &Probe).unwrap();
            let present = env.vars().any(|(name, _)| name == "SSH_AUTH_SOCK");
            assert_eq!(present, accepted && profile == ToolProfile::Git);
        }
    }
}

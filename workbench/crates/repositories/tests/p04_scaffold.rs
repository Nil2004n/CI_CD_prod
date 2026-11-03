//! The T04 process and SSH seams must remain unavailable until their implementation is reviewed.
use repositories::git::runner::{FixedSshPolicy, GitEnv};
use std::path::PathBuf;

#[test]
fn scaffold_refuses_git_environment_and_ssh_policy() {
    assert!(GitEnv::from_approved_pairs(Vec::new()).is_err());
    assert!(
        FixedSshPolicy::new(
            PathBuf::from("/app/ssh.conf"),
            PathBuf::from("/app/known_hosts")
        )
        .is_err()
    );
}

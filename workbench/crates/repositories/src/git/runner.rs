//! Process runner seam. Construction fails closed until the runner is implemented.
use std::{ffi::OsString, path::PathBuf, process::ExitStatus};

pub const STDIN_CAP: usize = 8 * 1024 * 1024;
pub const STDOUT_CAP: usize = 8 * 1024 * 1024;
pub const STDERR_CAP: usize = 64 * 1024;
pub const MAX_PATHS: usize = 100_000;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_HELPERS: usize = 64;
pub const MAX_HELPER_BYTES: usize = 4096;
pub const OP_TIMEOUT_SECS: u64 = 120;
pub const KILL_GRACE_SECS: u64 = 2;
pub const VERSION_TIMEOUT_SECS: u64 = 5;

// Executable identity is checked again immediately before any future spawn.
// A path-based spawn still has a same-user/mount substitution race; this is
// not an fd-based execution guarantee and does not by itself enable execution.
#[cfg(target_os = "linux")]
/// Pinned executable role for the Linux runner.
#[derive(Debug)]
pub enum ExecutableKind {
    Git,
    Timeout,
    Kill,
}

#[cfg(target_os = "linux")]
/// Checked executable identity; this does not execute the file or establish its contents.
/// Path use remains subject to mount and substitution races.
pub struct TrustedExecutable {
    path: PathBuf,
    file: std::fs::File,
    device: u64,
    inode: u64,
}

#[cfg(target_os = "linux")]
impl TrustedExecutable {
    /// Check the role's exact path, root ownership, permissions, type and open-file identity.
    pub fn new(path: PathBuf, kind: ExecutableKind) -> Result<Self, GitError> {
        use std::os::unix::fs::MetadataExt;
        let pinned = match kind {
            ExecutableKind::Git => "/usr/bin/git",
            ExecutableKind::Timeout => "/usr/bin/timeout",
            ExecutableKind::Kill => "/usr/bin/kill",
        };
        if path != std::path::Path::new(pinned) {
            return Err(GitError::InvalidInvocation);
        }
        check_executable_path(&path)?;
        let file = std::fs::File::open(&path).map_err(|_| GitError::InvalidInvocation)?;
        let metadata = file.metadata().map_err(|_| GitError::InvalidInvocation)?;
        let result = Self {
            path,
            file,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        result.revalidate()?;
        Ok(result)
    }

    fn revalidate(&self) -> Result<(), GitError> {
        use std::os::unix::fs::MetadataExt;
        check_executable_path(&self.path)?;
        let path_metadata =
            std::fs::symlink_metadata(&self.path).map_err(|_| GitError::InvalidInvocation)?;
        let file_metadata = self
            .file
            .metadata()
            .map_err(|_| GitError::InvalidInvocation)?;
        if path_metadata.dev() != self.device
            || path_metadata.ino() != self.inode
            || file_metadata.dev() != self.device
            || file_metadata.ino() != self.inode
        {
            return Err(GitError::InvalidInvocation);
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn check_executable_path(path: &std::path::Path) -> Result<(), GitError> {
    use std::os::unix::fs::MetadataExt;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| GitError::InvalidInvocation)?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(GitError::InvalidInvocation);
        }
        if current == path {
            if !metadata.is_file() || metadata.mode() & 0o111 == 0 || metadata.mode() & 0o6000 != 0
            {
                return Err(GitError::InvalidInvocation);
            }
        } else if !metadata.is_dir() {
            return Err(GitError::InvalidInvocation);
        }
    }
    Ok(())
}

// A path-based capability, not a confinement or mount-boundary primitive. Checks
// and use remain vulnerable to same-user replacement and mount races. No process
// uses this capability until the remaining runner gates are established.
#[cfg(target_os = "linux")]
/// Checked non-repository directory; this is not a mount-confinement guarantee.
/// The effective UID check relies on Linux `/proc/self/status` being trustworthy.
pub struct TrustedNonRepositoryDir {
    path: PathBuf,
    directory: std::fs::File,
    device: u64,
    inode: u64,
    uid: u32,
}

#[cfg(target_os = "linux")]
impl TrustedNonRepositoryDir {
    /// Check absolute components, ownership, permissions, repository markers and identity.
    /// Same-user path replacement and mount races remain possible.
    pub fn new(path: PathBuf) -> Result<Self, GitError> {
        use std::os::unix::fs::MetadataExt;
        let uid = effective_uid()?;
        check_non_repository_dir(&path, uid)?;
        let directory = std::fs::File::open(&path).map_err(|_| GitError::InvalidInvocation)?;
        let metadata = directory
            .metadata()
            .map_err(|_| GitError::InvalidInvocation)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o022 != 0 {
            return Err(GitError::InvalidInvocation);
        }
        let result = Self {
            path,
            directory,
            device: metadata.dev(),
            inode: metadata.ino(),
            uid,
        };
        result.revalidate()?;
        Ok(result)
    }

    fn revalidate(&self) -> Result<(), GitError> {
        use std::os::unix::fs::MetadataExt;
        check_non_repository_dir(&self.path, self.uid)?;
        let path_metadata =
            std::fs::symlink_metadata(&self.path).map_err(|_| GitError::InvalidInvocation)?;
        let handle_metadata = self
            .directory
            .metadata()
            .map_err(|_| GitError::InvalidInvocation)?;
        if path_metadata.uid() != self.uid
            || !same_directory_identity(&path_metadata, &handle_metadata, self.device, self.inode)
        {
            return Err(GitError::InvalidInvocation);
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn same_directory_identity(
    path: &std::fs::Metadata,
    handle: &std::fs::Metadata,
    device: u64,
    inode: u64,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    path.is_dir()
        && handle.is_dir()
        && path.dev() == device
        && path.ino() == inode
        && handle.dev() == device
        && handle.ino() == inode
}

#[cfg(target_os = "linux")]
fn effective_uid() -> Result<u32, GitError> {
    use std::io::Read;
    let status =
        std::fs::File::open("/proc/self/status").map_err(|_| GitError::InvalidInvocation)?;
    let mut bytes = Vec::new();
    status
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| GitError::InvalidInvocation)?;
    if bytes.len() > 64 * 1024 {
        return Err(GitError::InvalidInvocation);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| GitError::InvalidInvocation)?;
    let mut entries = text.lines().filter_map(|line| line.strip_prefix("Uid:"));
    let values: Vec<_> = entries
        .next()
        .ok_or(GitError::InvalidInvocation)?
        .split_whitespace()
        .collect();
    if entries.next().is_some()
        || values.len() != 4
        || values
            .iter()
            .any(|value| value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(GitError::InvalidInvocation);
    }
    values[1].parse().map_err(|_| GitError::InvalidInvocation)
}

#[cfg(target_os = "linux")]
fn check_non_repository_dir(path: &std::path::Path, uid: u32) -> Result<(), GitError> {
    use std::path::Component;
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(GitError::InvalidInvocation);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        check_directory_component(&current, uid)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn check_directory_component(path: &std::path::Path, uid: u32) -> Result<(), GitError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| GitError::InvalidInvocation)?;
    if !metadata.is_dir()
        || (metadata.uid() != 0 && metadata.uid() != uid)
        || metadata.mode() & 0o022 != 0
    {
        return Err(GitError::InvalidInvocation);
    }
    // Refuse Git's ordinary repository marker and conservatively reject a
    // possible bare-repository HEAD, including dangling symlinks.
    for marker in [".git", "HEAD"] {
        match std::fs::symlink_metadata(path.join(marker)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(GitError::InvalidInvocation),
        }
    }
    Ok(())
}

pub struct GitStdin {
    bytes: Option<Vec<u8>>,
}
impl GitStdin {
    pub fn null() -> Self {
        Self { bytes: None }
    }
    pub fn bytes(mut bytes: Vec<u8>) -> Result<Self, GitError> {
        if bytes.len() > STDIN_CAP {
            bytes.fill(0);
            return Err(GitError::InvalidInvocation);
        }
        Ok(Self { bytes: Some(bytes) })
    }
}

impl Drop for GitStdin {
    fn drop(&mut self) {
        if let Some(bytes) = &mut self.bytes {
            bytes.fill(0);
        }
    }
}

pub struct GitEnv {
    pairs: Vec<(OsString, OsString)>,
}

/// Seam for the application-authored, trusted SSH configuration. No policy can be constructed
/// until the Git runner implements and validates the P04 safety annex.
pub struct FixedSshPolicy {
    _private: (),
}

impl FixedSshPolicy {
    /// Refuse SSH until trusted executable, configuration and known-hosts handling exist.
    pub fn new(_config: PathBuf, _known_hosts: PathBuf) -> Result<Self, GitError> {
        Err(GitError::InvalidInvocation)
    }
}
impl GitEnv {
    pub fn from_approved_pairs(pairs: Vec<(OsString, OsString)>) -> Result<Self, GitError> {
        const REQUIRED: [&str; 8] = [
            "PATH",
            "LANG",
            "HOME",
            "XDG_CONFIG_HOME",
            "GIT_CONFIG_NOSYSTEM",
            "GIT_CONFIG_GLOBAL",
            "GIT_TERMINAL_PROMPT",
            "GIT_ATTR_NOSYSTEM",
        ];
        let mut seen = std::collections::HashSet::new();
        for (name, value) in &pairs {
            if name.as_encoded_bytes().contains(&0)
                || value.as_encoded_bytes().contains(&0)
                || !seen.insert(name.as_os_str())
            {
                return Err(GitError::InvalidInvocation);
            }
            let valid = match name.to_str() {
                Some("PATH") => value == "/usr/bin:/bin",
                Some("LANG") => value == "C.UTF-8",
                Some("GIT_CONFIG_NOSYSTEM" | "GIT_ATTR_NOSYSTEM") => value == "1",
                Some("GIT_CONFIG_GLOBAL") => value == "/dev/null",
                Some("GIT_TERMINAL_PROMPT") => value == "0",
                Some("HOME" | "XDG_CONFIG_HOME" | "SSH_AUTH_SOCK") => {
                    let path = std::path::Path::new(value);
                    path.is_absolute()
                        && !path
                            .components()
                            .any(|part| matches!(part, std::path::Component::ParentDir))
                }
                _ => false,
            };
            if !valid {
                return Err(GitError::InvalidInvocation);
            }
        }
        if REQUIRED
            .iter()
            .any(|name| !seen.contains(std::ffi::OsStr::new(name)))
        {
            return Err(GitError::InvalidInvocation);
        }
        Ok(Self { pairs })
    }
}

impl Drop for GitEnv {
    fn drop(&mut self) {
        for (name, value) in &mut self.pairs {
            name.clear();
            value.clear();
        }
    }
}

// These descriptions cannot execute until the process runner's trust gates exist.
#[cfg(test)]
enum GitOperation {
    Version,
    IsAncestor {
        old: domain::store::values::GitOid,
        target: domain::store::values::GitOid,
    },
}
pub struct GitInvocation {
    // Preserve opacity in non-test builds too: a public empty struct could otherwise
    // be constructed by any downstream crate.
    #[cfg(not(test))]
    _private: (),
    #[cfg(test)]
    operation: GitOperation,
}
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GitAnswer {
    VersionSupported,
    Ancestor(bool),
}
#[cfg(test)]
impl GitInvocation {
    pub(crate) fn version() -> Self {
        Self {
            operation: GitOperation::Version,
        }
    }
    pub(crate) fn is_ancestor(
        old: domain::store::values::GitOid,
        target: domain::store::values::GitOid,
    ) -> Self {
        Self {
            operation: GitOperation::IsAncestor { old, target },
        }
    }
    fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = [
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "submodule.recurse=false",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        match &self.operation {
            GitOperation::Version => args.push("--version".into()),
            GitOperation::IsAncestor { old, target } => args.extend([
                "merge-base".into(),
                "--is-ancestor".into(),
                old.as_str().into(),
                target.as_str().into(),
            ]),
        }
        args
    }
    fn timeout_secs(&self) -> u64 {
        match self.operation {
            GitOperation::Version => VERSION_TIMEOUT_SECS,
            GitOperation::IsAncestor { .. } => OP_TIMEOUT_SECS,
        }
    }
    fn classify(
        &self,
        status: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<GitAnswer, GitError> {
        if stdout.len() > STDOUT_CAP || stderr.len() > STDERR_CAP {
            return Err(GitError::OutputLimit);
        }
        match &self.operation {
            GitOperation::Version => {
                if status != Some(0) {
                    return Err(GitError::Exit);
                }
                let version = stdout
                    .strip_prefix(b"git version 2.53.")
                    .and_then(|v| v.strip_suffix(b"\n"))
                    .ok_or(GitError::UnsupportedVersion)?;
                if version.is_empty() || !version.iter().all(u8::is_ascii_digit) {
                    return Err(GitError::UnsupportedVersion);
                }
                Ok(GitAnswer::VersionSupported)
            }
            GitOperation::IsAncestor { .. } => {
                if !stdout.is_empty() {
                    return Err(GitError::Exit);
                }
                match status {
                    Some(0) => Ok(GitAnswer::Ancestor(true)),
                    Some(1) => Ok(GitAnswer::Ancestor(false)),
                    _ => Err(GitError::Exit),
                }
            }
        }
    }
}
pub struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
impl GitOutput {
    pub fn status(&self) -> &ExitStatus {
        &self.status
    }
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitError {
    InvalidInvocation,
    UnsupportedVersion,
    Spawn,
    Timeout,
    OutputLimit,
    Exit,
    Io,
}

pub trait GitRunner {
    fn run(&mut self, call: GitInvocation) -> Result<GitOutput, GitError>;
}

pub struct ProcessGitRunner {
    _private: (),
}
impl ProcessGitRunner {
    pub fn new(_git: PathBuf, _timeout: PathBuf, _env: GitEnv) -> Result<Self, GitError> {
        Err(GitError::InvalidInvocation)
    }
}
impl GitRunner for ProcessGitRunner {
    fn run(&mut self, _call: GitInvocation) -> Result<GitOutput, GitError> {
        Err(GitError::InvalidInvocation)
    }
}

masked_debug!(
    GitStdin,
    GitEnv,
    FixedSshPolicy,
    GitInvocation,
    GitOutput,
    ProcessGitRunner
);

#[cfg(target_os = "linux")]
masked_debug!(TrustedExecutable, TrustedNonRepositoryDir);

#[cfg(test)]
mod operation_tests {
    use super::*;
    use domain::store::values::GitOid;

    #[cfg(target_os = "linux")]
    #[test]
    fn directory_components_and_open_identity_fail_closed() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
        use std::time::{SystemTime, UNIX_EPOCH};

        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(self.0.join("link"));
                let _ = std::fs::remove_file(self.0.join("dir/HEAD"));
                let _ = std::fs::remove_dir(self.0.join("dir/.git"));
                let _ = std::fs::remove_dir(self.0.join("dir"));
                let _ = std::fs::remove_dir(self.0.join("old"));
                let _ = std::fs::remove_dir(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(
            std::env::temp_dir().join(format!("pwt-runner-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir(&fixture.0).unwrap();
        let dir = fixture.0.join("dir");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let uid = effective_uid().unwrap();
        assert!(check_directory_component(&dir, uid).is_ok());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            check_directory_component(&dir, uid),
            Err(GitError::InvalidInvocation)
        );
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(
            check_directory_component(&dir, uid),
            Err(GitError::InvalidInvocation)
        );
        std::fs::remove_dir(dir.join(".git")).unwrap();
        std::fs::write(dir.join("HEAD"), b"fixture").unwrap();
        assert_eq!(
            check_directory_component(&dir, uid),
            Err(GitError::InvalidInvocation)
        );
        std::fs::remove_file(dir.join("HEAD")).unwrap();
        symlink(&dir, fixture.0.join("link")).unwrap();
        assert_eq!(
            check_directory_component(&fixture.0.join("link"), uid),
            Err(GitError::InvalidInvocation)
        );

        let handle = std::fs::File::open(&dir).unwrap();
        let before = handle.metadata().unwrap();
        let current = std::fs::symlink_metadata(&dir).unwrap();
        assert!(same_directory_identity(
            &current,
            &before,
            before.dev(),
            before.ino()
        ));
        std::fs::rename(&dir, fixture.0.join("old")).unwrap();
        std::fs::create_dir(&dir).unwrap();
        let replacement = std::fs::symlink_metadata(&dir).unwrap();
        assert!(!same_directory_identity(
            &replacement,
            &before,
            before.dev(),
            before.ino()
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn trusted_directory_refuses_relative_and_parent_paths() {
        assert!(matches!(
            TrustedNonRepositoryDir::new(PathBuf::from("relative")),
            Err(GitError::InvalidInvocation)
        ));
        assert!(matches!(
            TrustedNonRepositoryDir::new(PathBuf::from("/tmp/../etc")),
            Err(GitError::InvalidInvocation)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn complete_directory_capability_checks_and_revalidates_identity() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        use std::time::{SystemTime, UNIX_EPOCH};

        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(self.0.join("link"));
                let _ = std::fs::remove_dir(self.0.join("dir/.git"));
                let _ = std::fs::remove_dir(self.0.join("dir"));
                let _ = std::fs::remove_dir(self.0.join("old"));
                let _ = std::fs::remove_dir(&self.0);
            }
        }
        // A caller may provide a private, trusted ancestor for the positive path.
        // Common temporary directories are world-writable and correctly refused.
        let parent = std::env::var_os("PWT_TRUST_TEST_PARENT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture =
            Fixture(parent.join(format!("pwt-capability-{}-{nonce}", std::process::id())));
        std::fs::create_dir(&fixture.0).unwrap();
        std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        let dir = fixture.0.join("dir");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&dir, fixture.0.join("link")).unwrap();
        assert!(matches!(
            TrustedNonRepositoryDir::new(fixture.0.join("link")),
            Err(GitError::InvalidInvocation)
        ));

        let uid = effective_uid().unwrap();
        if check_non_repository_dir(&dir, uid).is_err() {
            // Untrusted ancestors allow only the rejection check; the positive
            // constructor and replacement checks below require a trusted parent.
            assert!(matches!(
                TrustedNonRepositoryDir::new(dir),
                Err(GitError::InvalidInvocation)
            ));
            return;
        }
        let capability = TrustedNonRepositoryDir::new(dir.clone()).unwrap();
        assert_eq!(
            format!("{capability:?}"),
            "TrustedNonRepositoryDir(redacted)"
        );
        assert!(capability.revalidate().is_ok());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(capability.revalidate(), Err(GitError::InvalidInvocation));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(capability.revalidate(), Err(GitError::InvalidInvocation));
        std::fs::remove_dir(dir.join(".git")).unwrap();
        std::fs::rename(&dir, fixture.0.join("old")).unwrap();
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(capability.revalidate(), Err(GitError::InvalidInvocation));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pinned_executable_capabilities_check_host_metadata() {
        for (path, role) in [
            ("/usr/bin/git", ExecutableKind::Git),
            ("/usr/bin/timeout", ExecutableKind::Timeout),
            ("/usr/bin/kill", ExecutableKind::Kill),
        ] {
            let path = PathBuf::from(path);
            let trusted = check_executable_path(&path).is_ok();
            let result = TrustedExecutable::new(path, role);
            if trusted {
                let capability = result.unwrap();
                assert_eq!(format!("{capability:?}"), "TrustedExecutable(redacted)");
                assert!(capability.revalidate().is_ok());
            } else {
                // Untrusted host metadata permits only the rejection check;
                // positive construction and revalidation require a trusted host.
                assert!(matches!(result, Err(GitError::InvalidInvocation)));
            }
        }
        assert_eq!(format!("{:?}", ExecutableKind::Git), "Git");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn executables_reject_unpinned_paths_and_swapped_roles() {
        assert!(matches!(
            TrustedExecutable::new(PathBuf::from("/tmp/git"), ExecutableKind::Git),
            Err(GitError::InvalidInvocation)
        ));
        assert!(matches!(
            TrustedExecutable::new(PathBuf::from("/usr/bin/timeout"), ExecutableKind::Git),
            Err(GitError::InvalidInvocation)
        ));
        assert!(matches!(
            TrustedExecutable::new(PathBuf::from("/usr/bin/git"), ExecutableKind::Timeout),
            Err(GitError::InvalidInvocation)
        ));
        assert!(matches!(
            TrustedExecutable::new(PathBuf::from("/usr/bin/timeout"), ExecutableKind::Kill),
            Err(GitError::InvalidInvocation)
        ));
    }

    #[test]
    fn operations_fix_arguments_and_timeouts() {
        let version = GitInvocation::version();
        assert_eq!(
            version.args(),
            [
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "protocol.allow=never",
                "-c",
                "protocol.https.allow=always",
                "-c",
                "protocol.ssh.allow=always",
                "-c",
                "submodule.recurse=false",
                "--version"
            ]
            .map(OsString::from)
        );
        assert_eq!(version.timeout_secs(), VERSION_TIMEOUT_SECS);
        assert!(GitOid::parse("--upload-pack=evil").is_err());
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let call =
            GitInvocation::is_ancestor(GitOid::parse(&a).unwrap(), GitOid::parse(&b).unwrap());
        assert_eq!(
            &call.args()[12..],
            &[
                OsString::from("merge-base"),
                OsString::from("--is-ancestor"),
                a.into(),
                b.into()
            ]
        );
        assert_eq!(call.timeout_secs(), OP_TIMEOUT_SECS);
    }

    #[test]
    fn classification_rejects_bad_status_framing_and_limits() {
        let version = GitInvocation::version();
        assert_eq!(
            version.classify(Some(0), b"git version 2.53.0\n", b""),
            Ok(GitAnswer::VersionSupported)
        );
        for bytes in [
            b"git version 2.53.0".as_slice(),
            b"git version 2.53.x\n",
            b"git version 2.54.0\n",
            b"git version 2.53.0\nsecret",
        ] {
            assert_eq!(
                version.classify(Some(0), bytes, b"secret"),
                Err(GitError::UnsupportedVersion)
            );
        }
        assert_eq!(version.classify(None, b"", b"secret"), Err(GitError::Exit));
        assert_eq!(
            version.classify(Some(0), &vec![0; STDOUT_CAP + 1], b""),
            Err(GitError::OutputLimit)
        );
        let oid = GitOid::parse(&"a".repeat(40)).unwrap();
        let call = GitInvocation::is_ancestor(oid.clone(), oid);
        assert_eq!(
            call.classify(Some(0), b"", b""),
            Ok(GitAnswer::Ancestor(true))
        );
        assert_eq!(
            call.classify(Some(1), b"", b""),
            Ok(GitAnswer::Ancestor(false))
        );
        for status in [None, Some(2)] {
            assert_eq!(call.classify(status, b"", b"secret"), Err(GitError::Exit));
        }
        assert_eq!(call.classify(Some(1), b"secret", b""), Err(GitError::Exit));
        assert_eq!(
            call.classify(Some(0), b"", &vec![0; STDERR_CAP + 1]),
            Err(GitError::OutputLimit)
        );
    }
}

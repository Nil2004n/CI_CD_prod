//! Trusted, fixed OpenSSH policy. Path-based checks do not prevent same-user or mount races.
use super::runner::GitError;
use std::path::{Component, Path, PathBuf};

/// An SSH policy validated against app-authored configuration and trusted files.
/// Revalidate immediately before spawning; this is not an fd-based execution guarantee.
pub struct TrustedSshPolicy {
    config: PathBuf,
    known_hosts: PathBuf,
    agent: PathBuf,
    identities: [(u64, u64); 4],
}

impl std::fmt::Debug for TrustedSshPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrustedSshPolicy(redacted)")
    }
}

#[cfg(target_os = "linux")]
impl TrustedSshPolicy {
    /// Validate an existing app-authored config, trusted known-hosts and explicit agent socket.
    /// The exact config is deliberately fixed; callers must create it in a trusted directory.
    pub fn new(config: PathBuf, known_hosts: PathBuf, agent: PathBuf) -> Result<Self, GitError> {
        let identities = validate(&config, &known_hosts, &agent)?;
        Ok(Self {
            config,
            known_hosts,
            agent,
            identities,
        })
    }

    /// Recheck ownership, modes, file identities and exact configuration before use.
    pub fn revalidate(&self) -> Result<(), GitError> {
        if validate(&self.config, &self.known_hosts, &self.agent)? != self.identities {
            return Err(GitError::InvalidInvocation);
        }
        Ok(())
    }

    /// The only value permitted for the application-supplied GIT_SSH_COMMAND.
    /// It must be installed only after revalidation in a cleared child environment.
    pub fn command(&self) -> Result<String, GitError> {
        self.revalidate()?;
        Ok(format!(
            "/usr/bin/ssh -F {} -o BatchMode=yes -o StrictHostKeyChecking=yes",
            self.config.to_str().ok_or(GitError::InvalidInvocation)?
        ))
    }

    /// Exact bytes expected in the application-authored configuration file.
    pub fn config_bytes(known_hosts: &Path, agent: &Path) -> Result<Vec<u8>, GitError> {
        let known_hosts = safe_path(known_hosts)?;
        let agent = safe_path(agent)?;
        Ok(format!("Host *\n    IdentityFile none\n    IdentitiesOnly no\n    IdentityAgent {agent}\n    UserKnownHostsFile {known_hosts}\n    GlobalKnownHostsFile /dev/null\n    ProxyCommand none\n    ProxyJump none\n    PermitLocalCommand no\n    ForwardAgent no\n    BatchMode yes\n    StrictHostKeyChecking yes\n").into_bytes())
    }
}

#[cfg(target_os = "linux")]
fn safe_path(path: &Path) -> Result<&str, GitError> {
    let s = path.to_str().ok_or(GitError::InvalidInvocation)?;
    if !path.is_absolute()
        || path
            .components()
            .any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(GitError::InvalidInvocation);
    }
    Ok(s)
}

#[cfg(target_os = "linux")]
fn uid() -> Result<u32, GitError> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open("/proc/self/status")
        .map_err(|_| GitError::InvalidInvocation)?
        .take(65537)
        .read_to_string(&mut text)
        .map_err(|_| GitError::InvalidInvocation)?;
    if text.len() > 65536 {
        return Err(GitError::InvalidInvocation);
    }
    let mut lines = text.lines().filter_map(|line| line.strip_prefix("Uid:"));
    let values: Vec<_> = lines
        .next()
        .ok_or(GitError::InvalidInvocation)?
        .split_whitespace()
        .collect();
    if lines.next().is_some() || values.len() != 4 {
        return Err(GitError::InvalidInvocation);
    }
    values[1].parse().map_err(|_| GitError::InvalidInvocation)
}

#[cfg(target_os = "linux")]
fn checked(path: &Path, kind: u8, uid: u32) -> Result<(u64, u64), GitError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    safe_path(path)?;
    let mut part = PathBuf::new();
    for component in path.components() {
        part.push(component.as_os_str());
        let m = std::fs::symlink_metadata(&part).map_err(|_| GitError::InvalidInvocation)?;
        if m.uid() != 0 && m.uid() != uid || m.mode() & 0o022 != 0 {
            return Err(GitError::InvalidInvocation);
        }
        if part == path {
            let ok = match kind {
                0 => m.is_file() && m.uid() == 0 && m.mode() & 0o111 != 0 && m.mode() & 0o6000 == 0,
                1 => m.is_file() && m.uid() == uid,
                2 => m.file_type().is_socket() && m.uid() == uid,
                _ => false,
            };
            if !ok {
                return Err(GitError::InvalidInvocation);
            }
            return Ok((m.dev(), m.ino()));
        }
        if !m.is_dir() {
            return Err(GitError::InvalidInvocation);
        }
    }
    Err(GitError::InvalidInvocation)
}

#[cfg(target_os = "linux")]
fn validate(config: &Path, known_hosts: &Path, agent: &Path) -> Result<[(u64, u64); 4], GitError> {
    use std::io::Read;
    let uid = uid()?;
    let ids = [
        checked(Path::new("/usr/bin/ssh"), 0, uid)?,
        checked(config, 1, uid)?,
        checked(known_hosts, 1, uid)?,
        checked(agent, 2, uid)?,
    ];
    let expected = TrustedSshPolicy::config_bytes(known_hosts, agent)?;
    let mut actual = Vec::new();
    open_config(config, uid, ids[1])?
        .take(16385)
        .read_to_end(&mut actual)
        .map_err(|_| GitError::InvalidInvocation)?;
    let after = [
        checked(Path::new("/usr/bin/ssh"), 0, uid)?,
        checked(config, 1, uid)?,
        checked(known_hosts, 1, uid)?,
        checked(agent, 2, uid)?,
    ];
    if actual.len() > 16384 || actual != expected || after != ids {
        return Err(GitError::InvalidInvocation);
    }
    Ok(ids)
}

// Linux O_NOFOLLOW; use the opened handle's metadata, not only path metadata.
#[cfg(target_os = "linux")]
fn open_config(path: &Path, uid: u32, expected: (u64, u64)) -> Result<std::fs::File, GitError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    const O_NOFOLLOW: i32 = 0x20000;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
        .map_err(|_| GitError::InvalidInvocation)?;
    let metadata = file.metadata().map_err(|_| GitError::InvalidInvocation)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o022 != 0
        || (metadata.dev(), metadata.ino()) != expected
    {
        return Err(GitError::InvalidInvocation);
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    #[cfg(target_os = "linux")]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(target_os = "linux")]
    #[test]
    fn config_open_rejects_symlink_replacement_and_unsafe_mode() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwt-ssh-open-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config");
        let old = dir.join("old");
        std::fs::write(&path, b"synthetic").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let id = (meta.dev(), meta.ino());
        assert!(open_config(&path, meta.uid(), id).is_ok());
        std::fs::rename(&path, &old).unwrap();
        symlink(&old, &path).unwrap();
        assert!(open_config(&path, meta.uid(), id).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        assert!(open_config(&path, meta.uid(), id).is_err());
        let replacement = std::fs::metadata(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(
            open_config(
                &path,
                replacement.uid(),
                (replacement.dev(), replacement.ino())
            )
            .is_err()
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&old).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}

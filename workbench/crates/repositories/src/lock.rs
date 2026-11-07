//! Nonblocking Linux checkout locks. Path checks cannot prevent same-user substitution races.
use domain::ids::RepoId;
use std::path::Path;

pub struct CheckoutLock {
    file: std::fs::File,
    #[cfg(target_os = "linux")]
    data_dir: std::path::PathBuf,
    #[cfg(target_os = "linux")]
    repo_id: RepoId,
    #[cfg(target_os = "linux")]
    directories: Vec<(std::path::PathBuf, u64, u64)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockError {
    Locked,
    UnsafePath,
    Io,
}

/// Acquires a lock in pre-existing private checkout directories. Callers must hold the application
/// store lock first; this signature cannot verify that ordering. Unsupported platforms fail closed.
#[cfg(target_os = "linux")]
pub fn acquire_checkout_lock(data_dir: &Path, repo_id: RepoId) -> Result<CheckoutLock, LockError> {
    use std::fs::{self, OpenOptions, TryLockError};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::{Component, PathBuf};

    // Read the effective uid from the kernel; absence or ambiguity is a refusal.
    let status = fs::read_to_string("/proc/self/status").map_err(|_| LockError::UnsafePath)?;
    let mut uid_lines = status.lines().filter_map(|l| l.strip_prefix("Uid:"));
    let uid_line = uid_lines.next().ok_or(LockError::UnsafePath)?;
    if uid_lines.next().is_some() {
        return Err(LockError::UnsafePath);
    }
    let uid: u32 = uid_line
        .split_whitespace()
        .nth(1)
        .ok_or(LockError::UnsafePath)?
        .parse()
        .map_err(|_| LockError::UnsafePath)?;
    if !data_dir.is_absolute() {
        return Err(LockError::UnsafePath);
    }
    let mut current = PathBuf::new();
    let mut directories = Vec::new();
    for component in data_dir.components() {
        match component {
            Component::RootDir | Component::Normal(_) => current.push(component.as_os_str()),
            _ => return Err(LockError::UnsafePath),
        }
        let m = fs::symlink_metadata(&current).map_err(|_| LockError::UnsafePath)?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(LockError::UnsafePath);
        }
        directories.push((current.clone(), m.dev(), m.ino()));
    }
    let checkouts = data_dir.join("checkouts");
    let locks = checkouts.join(".locks");
    for dir in [data_dir, checkouts.as_path(), locks.as_path()] {
        let m = fs::symlink_metadata(dir).map_err(|_| LockError::UnsafePath)?;
        if !m.is_dir() || m.file_type().is_symlink() || m.uid() != uid || m.mode() & 0o077 != 0 {
            return Err(LockError::UnsafePath);
        }
        directories.push((dir.to_path_buf(), m.dev(), m.ino()));
    }
    reject_checkout_mounts(&checkouts)?;
    let path = locks.join(format!("{repo_id}.lock"));
    if let Ok(m) = fs::symlink_metadata(&path)
        && (!m.is_file() || m.file_type().is_symlink())
    {
        return Err(LockError::UnsafePath);
    }
    // Linux O_NOFOLLOW; no unsafe FFI and creation uses a private mode.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(0x20000)
        .open(&path)
        .map_err(|_| LockError::Io)?;
    let opened = file.metadata().map_err(|_| LockError::Io)?;
    let named = fs::symlink_metadata(&path).map_err(|_| LockError::UnsafePath)?;
    if !opened.is_file()
        || !named.is_file()
        || opened.uid() != uid
        || opened.nlink() != 1
        || opened.mode() & 0o077 != 0
        || opened.dev() != named.dev()
        || opened.ino() != named.ino()
    {
        return Err(LockError::UnsafePath);
    }
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Err(LockError::Locked),
        Err(TryLockError::Error(_)) => return Err(LockError::Io),
    }
    // A replacement while waiting for the lock must not produce a guard for a stale inode.
    // This narrows, but cannot eliminate, same-user pathname substitution races.
    let named = fs::symlink_metadata(&path).map_err(|_| LockError::UnsafePath)?;
    if !named.is_file() || opened.dev() != named.dev() || opened.ino() != named.ino() {
        return Err(LockError::UnsafePath);
    }
    for (dir, dev, ino) in &directories {
        let current = fs::symlink_metadata(dir).map_err(|_| LockError::UnsafePath)?;
        if !current.is_dir()
            || current.dev() != *dev
            || current.ino() != *ino
            || ([data_dir, checkouts.as_path(), locks.as_path()].contains(&dir.as_path())
                && (current.uid() != uid || current.mode() & 0o077 != 0))
        {
            return Err(LockError::UnsafePath);
        }
    }
    Ok(CheckoutLock {
        file,
        data_dir: data_dir.to_path_buf(),
        repo_id,
        directories,
    })
}

/// A checkout path bound to a held repository lock. Pathname checks cannot prevent a
/// same-user actor from replacing components after validation; revalidate before use.
pub struct ConfinedCheckout<'a> {
    guard: &'a CheckoutLock,
    path: std::path::PathBuf,
}

impl ConfinedCheckout<'_> {
    /// Revalidates the guard and path before returning the checkout location.
    pub fn path(&self) -> Result<&Path, LockError> {
        self.guard.validate_capability()?;
        Ok(&self.path)
    }
}

#[cfg(target_os = "linux")]
impl CheckoutLock {
    /// Binds a checkout capability to this guard's exact repository ID.
    pub fn checkout(&self, repo_id: RepoId) -> Result<ConfinedCheckout<'_>, LockError> {
        self.validate_checkout(repo_id)?;
        Ok(ConfinedCheckout {
            guard: self,
            path: self.data_dir.join("checkouts").join(repo_id.to_string()),
        })
    }

    fn validate_capability(&self) -> Result<(), LockError> {
        self.validate_checkout(self.repo_id)
    }

    fn validate_checkout(&self, repo_id: RepoId) -> Result<(), LockError> {
        use std::fs;
        use std::io::ErrorKind;
        use std::os::unix::fs::MetadataExt;

        if repo_id != self.repo_id {
            return Err(LockError::UnsafePath);
        }
        let checkouts = self.data_dir.join("checkouts");
        let locks = checkouts.join(".locks");
        let opened = self.file.metadata().map_err(|_| LockError::UnsafePath)?;
        let uid = opened.uid();
        if !opened.is_file() || opened.nlink() != 1 || opened.mode() & 0o077 != 0 {
            return Err(LockError::UnsafePath);
        }
        for (dir, dev, ino) in &self.directories {
            let current = fs::symlink_metadata(dir).map_err(|_| LockError::UnsafePath)?;
            if !current.is_dir()
                || current.dev() != *dev
                || current.ino() != *ino
                || ([
                    self.data_dir.as_path(),
                    checkouts.as_path(),
                    locks.as_path(),
                ]
                .contains(&dir.as_path())
                    && (current.uid() != uid || current.mode() & 0o077 != 0))
            {
                return Err(LockError::UnsafePath);
            }
        }
        reject_checkout_mounts(&checkouts)?;
        let named = fs::symlink_metadata(locks.join(format!("{repo_id}.lock")))
            .map_err(|_| LockError::UnsafePath)?;
        if !named.is_file() || named.dev() != opened.dev() || named.ino() != opened.ino() {
            return Err(LockError::UnsafePath);
        }
        match fs::symlink_metadata(checkouts.join(repo_id.to_string())) {
            Ok(m) if m.is_dir() && m.uid() == uid && m.mode() & 0o022 == 0 => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            _ => Err(LockError::UnsafePath),
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl CheckoutLock {
    pub fn checkout(&self, _repo_id: RepoId) -> Result<ConfinedCheckout<'_>, LockError> {
        Err(LockError::UnsafePath)
    }
    fn validate_capability(&self) -> Result<(), LockError> {
        Err(LockError::UnsafePath)
    }
}

#[cfg(target_os = "linux")]
fn reject_checkout_mounts(checkouts: &Path) -> Result<(), LockError> {
    use std::fs;
    use std::path::PathBuf;
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").map_err(|_| LockError::UnsafePath)?;
    for line in mountinfo.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 10 || !fields.contains(&"-") {
            return Err(LockError::UnsafePath);
        }
        let raw = fields[4].as_bytes();
        let mut decoded = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            if raw[i] == b'\\' {
                if i + 3 >= raw.len()
                    || !raw[i + 1..i + 4].iter().all(|b| (b'0'..=b'7').contains(b))
                {
                    return Err(LockError::UnsafePath);
                }
                let n = (u16::from(raw[i + 1] - b'0') << 6)
                    | (u16::from(raw[i + 2] - b'0') << 3)
                    | u16::from(raw[i + 3] - b'0');
                decoded.push(u8::try_from(n).map_err(|_| LockError::UnsafePath)?);
                i += 4;
            } else {
                decoded.push(raw[i]);
                i += 1;
            }
        }
        use std::os::unix::ffi::OsStringExt;
        let mount = PathBuf::from(std::ffi::OsString::from_vec(decoded));
        if !mount.is_absolute() {
            return Err(LockError::UnsafePath);
        }
        if mount.starts_with(checkouts) {
            return Err(LockError::UnsafePath);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn acquire_checkout_lock(
    _data_dir: &Path,
    _repo_id: RepoId,
) -> Result<CheckoutLock, LockError> {
    Err(LockError::UnsafePath)
}

masked_debug!(CheckoutLock);

masked_debug!(ConfinedCheckout<'_>);

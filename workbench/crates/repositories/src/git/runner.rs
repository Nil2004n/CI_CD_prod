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

pub struct GitStdin {
    _private: (),
}
impl GitStdin {
    pub fn null() -> Self {
        Self { _private: () }
    }
    pub fn bytes(_bytes: Vec<u8>) -> Result<Self, GitError> {
        Err(GitError::InvalidInvocation)
    }
}

pub struct GitEnv {
    _private: (),
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
    pub fn from_approved_pairs(_pairs: Vec<(OsString, OsString)>) -> Result<Self, GitError> {
        Err(GitError::InvalidInvocation)
    }
}

pub struct GitInvocation {
    _private: (),
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

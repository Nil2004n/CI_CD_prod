//! Child-environment sanitiser (Tech §5/§18, invariant "strip model credentials from scanner child
//! environments"; frozen interface and policy `planning/P03-T01-request.md` §5.5 and §7). Owner: W2, T06.
//!
//! [`build`] makes a [`ChildEnv`] for one tool from an empty base: fixed values and app-owned directories
//! per [`ToolProfile`], plus the few host variables a profile may inherit. Deny rules apply to the final
//! result whatever the profile says; a profile that would produce a denied variable fails closed. No
//! process is spawned here (P06), and no environment is read here: the host environment is an explicit
//! [`HostEnv`] input, collected once by the binary entry point.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use domain::text::SanitisedText;

/// The child-environment profiles (request §7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToolProfile {
    /// Git (P04).
    Git,
    /// yamllint.
    Yamllint,
    /// Gitleaks.
    Gitleaks,
    /// Checkov (contract revision 2: exact variable set).
    Checkov,
    /// actionlint.
    Actionlint,
    /// kubeconform.
    Kubeconform,
    /// Application-internal helper processes.
    Internal,
}

impl ToolProfile {
    /// Every profile.
    pub const ALL: [Self; 7] = [
        Self::Git,
        Self::Yamllint,
        Self::Gitleaks,
        Self::Checkov,
        Self::Actionlint,
        Self::Kubeconform,
        Self::Internal,
    ];

    /// The profile name as typed on the command line (`git`, `yamllint`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Yamllint => "yamllint",
            Self::Gitleaks => "gitleaks",
            Self::Checkov => "checkov",
            Self::Actionlint => "actionlint",
            Self::Kubeconform => "kubeconform",
            Self::Internal => "internal",
        }
    }

    /// The profile named `name`, if any.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == name)
    }
}

/// Why a child environment could not be built. Fieldless: never echoes a name or value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EnvError {
    /// The profile would produce a variable whose name is denied.
    DeniedName,
    /// A value the profile would set contains the session credential.
    SecretValue,
    /// A tool directory is not absolute, contains `..`, or is otherwise unusable.
    InvalidDir,
}

/// Where a child variable's value comes from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Origin {
    /// A constant of the profile.
    Fixed,
    /// One of the tool's app-owned directories.
    AppDir,
    /// The host environment (only names the profile allows).
    Inherited,
}

/// Checks whether a value contains the session credential.
pub trait SecretProbe {
    /// Whether the session credential occurs anywhere in `value`.
    fn appears_in(&self, value: &[u8]) -> bool;
}

/// The app-owned directories of one tool (created by P06, not here).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ToolDirs {
    home: PathBuf,
    config: PathBuf,
    cache: PathBuf,
    tmp: PathBuf,
}

impl ToolDirs {
    /// Validates the four directories: absolute, no `..` component.
    pub fn new(home: &Path, config: &Path, cache: &Path, tmp: &Path) -> Result<Self, EnvError> {
        let _ = (home, config, cache, tmp);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T06")
        }
    }

    /// The directories under `root/tools/<profile>/{home,config,cache,tmp}` (paths only; nothing is
    /// created).
    pub fn under(root: &Path, profile: ToolProfile) -> Result<Self, EnvError> {
        let _ = (root, profile);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T06")
        }
    }
}

/// A snapshot of the host environment, passed in explicitly. `Debug` shows only the number of entries.
pub struct HostEnv {
    vars: Vec<(OsString, OsString)>,
}

impl HostEnv {
    /// A snapshot of the given pairs, in order (duplicates kept as given).
    pub fn from_pairs<I: IntoIterator<Item = (OsString, OsString)>>(pairs: I) -> Self {
        Self {
            vars: pairs.into_iter().collect(),
        }
    }

    /// An empty host environment.
    pub fn empty() -> Self {
        Self { vars: Vec::new() }
    }

    /// The entries, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(k, v)| (k.as_os_str(), v.as_os_str()))
    }
}

impl std::fmt::Debug for HostEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HostEnv(<{} entries>)", self.vars.len())
    }
}

/// The sanitised environment of one child process. Only [`build`] constructs it; P06 must spawn with it
/// (and nothing else) after clearing the inherited environment. `Debug` shows names and origins only.
pub struct ChildEnv {
    vars: Vec<(OsString, OsString, Origin)>,
}

impl ChildEnv {
    /// The variables to set, in a stable order.
    pub fn vars(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.vars
            .iter()
            .map(|(k, v, _)| (k.as_os_str(), v.as_os_str()))
    }

    /// Names and origins for display; never a value.
    pub fn preview(&self) -> Vec<PreviewLine> {
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T06")
        }
    }
}

impl std::fmt::Debug for ChildEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = (&self.vars, f);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T06")
        }
    }
}

/// One line of [`ChildEnv::preview`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreviewLine {
    /// The variable name (sanitised).
    pub name: SanitisedText,
    /// Where the value comes from.
    pub origin: Origin,
}

/// Builds the child environment of `profile` (request §7).
pub fn build(
    profile: ToolProfile,
    dirs: &ToolDirs,
    host: &HostEnv,
    probe: &dyn SecretProbe,
) -> Result<ChildEnv, EnvError> {
    let _ = (profile, dirs, host, probe);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T06")
    }
}

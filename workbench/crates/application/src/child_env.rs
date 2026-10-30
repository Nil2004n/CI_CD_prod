//! Child-environment sanitiser (Tech §5/§18, invariant "strip model credentials from scanner child
//! environments"; frozen interface and policy `planning/P03-T01-request.md` §5.5 and §7). Owner: W2, T06.
//!
//! [`build`] makes a [`ChildEnv`] for one tool from an empty base: fixed values and app-owned directories
//! per [`ToolProfile`], plus the few host variables a profile may inherit. Deny rules apply to the final
//! result whatever the profile says; a profile that would produce a denied variable fails closed. No
//! process is spawned here (P06), and no environment is read here: the host environment is an explicit
//! [`HostEnv`] input, collected once by the binary entry point.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use domain::text::{SanitisedText, sanitise};

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
    ///
    /// A path must also be free of NUL bytes (it could not be passed to a child). Valid UTF-8 is not
    /// required. Nothing is created or looked up on disk.
    pub fn new(home: &Path, config: &Path, cache: &Path, tmp: &Path) -> Result<Self, EnvError> {
        for dir in [home, config, cache, tmp] {
            check_dir(dir)?;
        }
        Ok(Self {
            home: home.to_path_buf(),
            config: config.to_path_buf(),
            cache: cache.to_path_buf(),
            tmp: tmp.to_path_buf(),
        })
    }

    /// The directories under `root/tools/<profile>/{home,config,cache,tmp}` (paths only; nothing is
    /// created).
    pub fn under(root: &Path, profile: ToolProfile) -> Result<Self, EnvError> {
        check_dir(root)?;
        let base = root.join("tools").join(profile.as_str());
        Self::new(
            &base.join("home"),
            &base.join("config"),
            &base.join("cache"),
            &base.join("tmp"),
        )
    }

    fn get(&self, which: AppDir) -> &Path {
        match which {
            AppDir::Home => &self.home,
            AppDir::Config => &self.config,
            AppDir::Cache => &self.cache,
            AppDir::Tmp => &self.tmp,
        }
    }
}

/// Absolute, NUL-free and without a `..` component. A `..` inside a longer component name (`..x`) is a
/// normal name and is accepted.
fn check_dir(path: &Path) -> Result<(), EnvError> {
    let bad = !path.is_absolute()
        || path.as_os_str().as_bytes().contains(&0)
        || path.components().any(|c| matches!(c, Component::ParentDir));
    if bad {
        Err(EnvError::InvalidDir)
    } else {
        Ok(())
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

    /// Names and origins for display; never a value. Same order as [`ChildEnv::vars`].
    pub fn preview(&self) -> Vec<PreviewLine> {
        self.vars
            .iter()
            .map(|(name, _, origin)| PreviewLine {
                name: sanitise(&name.to_string_lossy()),
                origin: *origin,
            })
            .collect()
    }
}

impl std::fmt::Debug for ChildEnv {
    /// `ChildEnv { vars: ["NAME/Origin", …] }`: names and origins only, never a value.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shown: Vec<String> = self
            .vars
            .iter()
            .map(|(name, _, origin)| format!("{}/{origin:?}", sanitise(&name.to_string_lossy())))
            .collect();
        f.debug_struct("ChildEnv").field("vars", &shown).finish()
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
///
/// Order of the result: ascending by variable name, byte-wise, whatever the order of the host entries. The
/// result never holds two variables of one name.
///
/// Fails closed: [`EnvError::DeniedName`] if the profile declares a variable whose name is denied (checked
/// on the declared names, so it does not depend on what the host holds), [`EnvError::SecretValue`] if a
/// fixed or app-dir value that is set contains the session credential. Host entries are never an error:
/// an entry that the profile does not list, or that fails an inheritance rule, is dropped silently.
pub fn build(
    profile: ToolProfile,
    dirs: &ToolDirs,
    host: &HostEnv,
    probe: &dyn SecretProbe,
) -> Result<ChildEnv, EnvError> {
    assemble(&spec_for(profile), dirs, host, probe)
}

/// The directories a profile may point at.
#[derive(Clone, Copy)]
enum AppDir {
    Home,
    Config,
    Cache,
    Tmp,
}

impl AppDir {
    /// The variable that carries the directory.
    const fn var_name(self) -> &'static str {
        match self {
            Self::Home => "HOME",
            Self::Config => "XDG_CONFIG_HOME",
            Self::Cache => "XDG_CACHE_HOME",
            Self::Tmp => "TMPDIR",
        }
    }
}

/// What one profile sets and inherits (the request §7 table). `PATH` and `LANG` ([`BASE_FIXED`]) come on
/// top for every profile.
struct ProfileSpec {
    app_dirs: &'static [AppDir],
    fixed: &'static [(&'static str, &'static str)],
    inherited: &'static [&'static str],
}

/// Set for every profile.
const BASE_FIXED: [(&str, &str); 2] = [("PATH", "/usr/bin:/bin"), ("LANG", "C.UTF-8")];

fn spec_for(profile: ToolProfile) -> ProfileSpec {
    use AppDir::{Cache, Config, Home, Tmp};
    match profile {
        ToolProfile::Git => ProfileSpec {
            app_dirs: &[Home, Config],
            fixed: &[
                ("GIT_CONFIG_NOSYSTEM", "1"),
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_TERMINAL_PROMPT", "0"),
                ("GIT_ATTR_NOSYSTEM", "1"),
            ],
            inherited: &[],
        },
        ToolProfile::Yamllint => ProfileSpec {
            app_dirs: &[Home, Config, Cache, Tmp],
            fixed: &[],
            inherited: &[],
        },
        ToolProfile::Gitleaks => ProfileSpec {
            app_dirs: &[Home, Tmp],
            fixed: &[],
            inherited: &[],
        },
        ToolProfile::Checkov => ProfileSpec {
            app_dirs: &[Home, Config, Cache, Tmp],
            fixed: &[("CKV_SKIP_PACKAGE_UPDATE_CHECK", "True")],
            inherited: &[],
        },
        ToolProfile::Actionlint | ToolProfile::Kubeconform => ProfileSpec {
            app_dirs: &[Home, Cache, Tmp],
            fixed: &[],
            inherited: &[],
        },
        ToolProfile::Internal => ProfileSpec {
            app_dirs: &[Home, Tmp],
            fixed: &[],
            inherited: &["TZ"],
        },
    }
}

/// Builds the environment of `spec`. Split from [`build`] so that unit tests can feed a profile that no
/// real [`ToolProfile`] has (a denied name in the table).
fn assemble(
    spec: &ProfileSpec,
    dirs: &ToolDirs,
    host: &HostEnv,
    probe: &dyn SecretProbe,
) -> Result<ChildEnv, EnvError> {
    // Deny first, on every name the profile can produce (a superset of the final set), so a profile
    // error is the same for every host and always beats a value error.
    let declared = BASE_FIXED
        .iter()
        .map(|(name, _)| *name)
        .chain(spec.fixed.iter().map(|(name, _)| *name))
        .chain(spec.app_dirs.iter().map(|dir| dir.var_name()))
        .chain(spec.inherited.iter().copied());
    if any_denied(declared) {
        return Err(EnvError::DeniedName);
    }

    let mut vars: Vec<(OsString, OsString, Origin)> = Vec::new();
    for (name, value) in BASE_FIXED.iter().chain(spec.fixed.iter()) {
        if probe.appears_in(value.as_bytes()) {
            return Err(EnvError::SecretValue);
        }
        vars.push((OsString::from(*name), OsString::from(*value), Origin::Fixed));
    }
    for dir in spec.app_dirs {
        let path = dirs.get(*dir).as_os_str();
        if probe.appears_in(path.as_bytes()) {
            return Err(EnvError::SecretValue);
        }
        vars.push((
            OsString::from(dir.var_name()),
            path.to_os_string(),
            Origin::AppDir,
        ));
    }

    // The first occurrence of a name decides: if it fails a rule the name is dropped, and a later
    // duplicate is not used instead.
    let mut seen: Vec<&str> = Vec::new();
    for (name, value) in host.iter() {
        let Some(name) = name.to_str() else { continue };
        if name.is_empty() || name.contains('=') || !spec.inherited.contains(&name) {
            continue;
        }
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let bytes = value.as_bytes();
        if bytes.contains(&0) || probe.appears_in(bytes) {
            continue;
        }
        vars.push((
            OsString::from(name),
            value.to_os_string(),
            Origin::Inherited,
        ));
    }

    vars.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(ChildEnv { vars })
}

/// Name fragments that deny a variable wherever they occur (ASCII upper case).
const DENIED_CONTAINS: [&str; 8] = [
    "KEY",
    "TOKEN",
    "SECRET",
    "PASSW",
    "CREDENTIAL",
    "AUTH",
    "COOKIE",
    "SESSION",
];

/// Name prefixes that deny a variable (ASCII upper case).
const DENIED_PREFIXES: [&str; 18] = [
    "OPENROUTER_",
    "AWS_",
    "AZURE_",
    "GOOGLE_",
    "GCP_",
    "GITHUB_",
    "GH_",
    "GITLAB_",
    "CI_JOB_",
    "BC_",
    "PRISMA_",
    "SSH_",
    "GPG_",
    "DOCKER_",
    "NPM_",
    "PYPI_",
    "GIT_CONFIG_KEY_",
    "GIT_CONFIG_VALUE_",
];

/// Names that are denied exactly (ASCII upper case).
const DENIED_EXACT: [&str; 7] = [
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "GIT_CONFIG_COUNT",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
    "PYTHONSTARTUP",
];

/// Whether a variable name is denied (request §7). ASCII case-insensitive; names with other characters
/// are compared as they are.
fn is_denied(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    DENIED_CONTAINS.iter().any(|part| upper.contains(part))
        || DENIED_PREFIXES
            .iter()
            .any(|prefix| upper.starts_with(prefix))
        || upper.ends_with("_PROXY")
        || DENIED_EXACT.contains(&upper.as_str())
}

/// Whether any of the names is denied.
fn any_denied<'a>(mut names: impl Iterator<Item = &'a str>) -> bool {
    names.any(is_denied)
}

#[cfg(test)]
mod tests {
    //! Unit tests that need private items: the deny table and `assemble` with a profile that no real
    //! `ToolProfile` has. The public behaviour is covered by `tests/p03_child_env.rs`.

    use super::*;

    /// A probe for the fake key `pwt-test-FAKE-unit`.
    struct Probe;

    impl SecretProbe for Probe {
        fn appears_in(&self, value: &[u8]) -> bool {
            let key = b"pwt-test-FAKE-unit";
            value.windows(key.len()).any(|w| w == key)
        }
    }

    fn dirs() -> ToolDirs {
        ToolDirs::new(
            Path::new("/d/home"),
            Path::new("/d/config"),
            Path::new("/d/cache"),
            Path::new("/d/tmp"),
        )
        .expect("absolute dirs")
    }

    fn host(pairs: &[(&str, &str)]) -> HostEnv {
        HostEnv::from_pairs(
            pairs
                .iter()
                .map(|(k, v)| (OsString::from(*k), OsString::from(*v))),
        )
    }

    fn names(env: &ChildEnv) -> Vec<String> {
        env.vars()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect()
    }

    /// A profile with the shape of a real one, used as the positive control: it must be accepted, so the
    /// failures below are caused by the name and nothing else.
    const CLEAN: ProfileSpec = ProfileSpec {
        app_dirs: &[AppDir::Home],
        fixed: &[("PWT_FLAG", "1")],
        inherited: &["TZ"],
    };

    #[test]
    fn denied_names_are_denied_in_any_case() {
        // Would fail if a rule were removed from the tables or the comparison became case-sensitive.
        let denied = [
            // contains
            "KEY",
            "MY_KEY",
            "api_key",
            "Apikey",
            "TOKEN",
            "x_token_y",
            "SECRET",
            "client_secret",
            "PASSWORD",
            "PASSWD",
            "db_passw",
            "CREDENTIAL",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "AUTH",
            "Authorization",
            "COOKIE",
            "set_cookie",
            "SESSION",
            "xsession",
            // prefixes
            "OPENROUTER_API_KEY",
            "openrouter_x",
            "AWS_REGION",
            "aws_profile",
            "AZURE_X",
            "GOOGLE_X",
            "GCP_X",
            "GITHUB_ACTIONS",
            "github_x",
            "GH_HOST",
            "gh_x",
            "GITLAB_CI",
            "CI_JOB_ID",
            "ci_job_x",
            "BC_X",
            "bc_api",
            "PRISMA_X",
            "SSH_AGENT_PID",
            "ssh_x",
            "GPG_TTY",
            "gpg_x",
            "DOCKER_HOST",
            "docker_x",
            "NPM_CONFIG_X",
            "npm_x",
            "PYPI_X",
            "pypi_x",
            // proxies
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "https_proxy",
            "http_proxy",
            "ALL_PROXY",
            "no_proxy",
            "Ftp_Proxy",
            "X_PROXY",
            // exact names and families
            "GIT_ASKPASS",
            "git_askpass",
            "SSH_ASKPASS",
            "GIT_CONFIG_COUNT",
            "git_config_count",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_KEY_",
            "git_config_key_17",
            "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_VALUE_",
            "git_config_value_3",
            "LD_PRELOAD",
            "ld_preload",
            "Ld_Preload",
            "LD_LIBRARY_PATH",
            "ld_library_path",
            "PYTHONPATH",
            "pythonpath",
            "PYTHONSTARTUP",
            "pythonstartup",
        ];
        for name in denied {
            assert!(is_denied(name), "must be denied: {name}");
        }
    }

    #[test]
    fn near_miss_names_are_allowed() {
        // Would fail if a rule were widened to a bare prefix or exact names became substrings.
        let allowed = [
            "TZ",
            "LANG",
            "PATH",
            "HOME",
            "TMPDIR",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "LC_ALL",
            "TERM",
            "USER",
            "SHELL",
            "PWD",
            "CKV_SKIP_PACKAGE_UPDATE_CHECK",
            "GIT_CONFIG_GLOBAL",
            "GIT_CONFIG_NOSYSTEM",
            "GIT_TERMINAL_PROMPT",
            "GIT_ATTR_NOSYSTEM",
            "GIT_CONFIG",
            "GIT_CONFIG_COUNTS",
            "GIT_CONFIG_VALUE",
            "GIT_ASKPASSX",
            "PYTHONPATHS",
            "PYTHONHOME",
            "LD_LIBRARY",
            "LD_LIBRARY_PATHS",
            "LD_DEBUG",
            "GHOST",
            "GH",
            "AWS",
            "AWSOME",
            "GITHUB",
            "SSH",
            "DOCKER",
            "NPM",
            "BC",
            "BCRYPT",
            "CI",
            "CI_JOB",
            "CI_X",
            "PROXY",
            "PROXYSERVER",
            "PROXY_X",
            "OPENROUTER",
            "GPG",
            "GCP",
            "AZURE",
            "GOOGLE",
            "PRISMA",
            "PYPI",
            "GITLAB",
            "",
            "a",
        ];
        for name in allowed {
            assert!(!is_denied(name), "must be allowed: {name}");
        }
    }

    #[test]
    fn every_real_profile_declares_only_allowed_unique_names() {
        // Would fail if a table entry were renamed to a denied name or declared twice.
        for profile in ToolProfile::ALL {
            let spec = spec_for(profile);
            let declared: Vec<&str> = BASE_FIXED
                .iter()
                .map(|(n, _)| *n)
                .chain(spec.fixed.iter().map(|(n, _)| *n))
                .chain(spec.app_dirs.iter().map(|d| d.var_name()))
                .chain(spec.inherited.iter().copied())
                .collect();
            assert!(!any_denied(declared.iter().copied()), "{profile:?}");
            let mut sorted = declared.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), declared.len(), "{profile:?}");
        }
    }

    #[test]
    fn control_profile_is_accepted() {
        // Without this, the failures below could come from the shape of the profile.
        let env =
            assemble(&CLEAN, &dirs(), &host(&[("TZ", "UTC")]), &Probe).expect("clean profile");
        assert_eq!(names(&env), ["HOME", "LANG", "PATH", "PWT_FLAG", "TZ"]);
    }

    #[test]
    fn denied_fixed_name_fails_closed() {
        // Would fail if the deny check were skipped for fixed values.
        let spec = ProfileSpec {
            fixed: &[("OPENROUTER_API_KEY", "1")],
            ..CLEAN
        };
        assert_eq!(
            assemble(&spec, &dirs(), &HostEnv::empty(), &Probe).err(),
            Some(EnvError::DeniedName)
        );
        let spec = ProfileSpec {
            fixed: &[("https_proxy", "http://127.0.0.1:1")],
            ..CLEAN
        };
        assert_eq!(
            assemble(&spec, &dirs(), &HostEnv::empty(), &Probe).err(),
            Some(EnvError::DeniedName)
        );
    }

    #[test]
    fn deny_beats_allow_for_inherited_names() {
        // A profile that lists a denied name as inheritable still gets DeniedName: with the host holding
        // the variable, and with the host not holding it. Would fail if inheritance were checked only
        // against the allow list, or only when the host has the entry.
        const CASES: [&[&str]; 5] = [
            &["GITHUB_TOKEN"],
            &["LD_PRELOAD"],
            &["https_proxy"],
            &["PYTHONPATH"],
            &["GIT_CONFIG_COUNT"],
        ];
        for inherited in CASES {
            let denied = inherited[0];
            let spec = ProfileSpec { inherited, ..CLEAN };
            let with = host(&[(denied, "harmless")]);
            assert_eq!(
                assemble(&spec, &dirs(), &with, &Probe).err(),
                Some(EnvError::DeniedName),
                "{denied} present"
            );
            assert_eq!(
                assemble(&spec, &dirs(), &HostEnv::empty(), &Probe).err(),
                Some(EnvError::DeniedName),
                "{denied} absent"
            );
        }
    }

    #[test]
    fn denied_name_beats_secret_value() {
        // Both problems at once: the profile error is reported, not the value error.
        let spec = ProfileSpec {
            fixed: &[("MY_TOKEN", "pwt-test-FAKE-unit")],
            ..CLEAN
        };
        assert_eq!(
            assemble(&spec, &dirs(), &HostEnv::empty(), &Probe).err(),
            Some(EnvError::DeniedName)
        );
    }

    #[test]
    fn secret_in_fixed_value_fails_closed() {
        // Would fail if fixed values were not probed.
        let spec = ProfileSpec {
            fixed: &[("PWT_FLAG", "x-pwt-test-FAKE-unit-y")],
            ..CLEAN
        };
        assert_eq!(
            assemble(&spec, &dirs(), &HostEnv::empty(), &Probe).err(),
            Some(EnvError::SecretValue)
        );
    }
}

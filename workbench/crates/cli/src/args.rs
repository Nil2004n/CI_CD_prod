//! Argument parsing for the headless CLI (request §5.9). Standard library only.
//!
//! Grammar: `pwt-workbench-cli [--json] <command> [flags]`. Flags may appear anywhere after the program
//! name; every flag takes a separate value (`--data-dir D`, not `--data-dir=D`). Every argument must be
//! valid UTF-8, so a usage message never needs to repeat raw bytes; and no usage message repeats any
//! argument at all.
//!
//! The parser performs no file-system access and reads no environment: `--data-dir` is mandatory for every
//! command that uses a data directory (`store`, `approvals`, `dev`; not `credential`, `help`, `features`)
//! and there is no default. Whether the directory exists and is private is decided later by the
//! application layer (`dev_store`, `approvals`, `dev_permission`).

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use application::child_env::ToolProfile;
use application::dev_store::parse_root;
use domain::ids::ApprovalId;
use domain::store::StoreKind;

/// The flags the CLI knows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Flag {
    /// `--json`: JSON Lines output.
    Json,
    /// `--version`: print the contract versions.
    Version,
    /// `--data-dir <dir>`: the data directory of a command that uses one.
    DataDir,
    /// `--root <root>`: restrict `store validate` to one root.
    Root,
    /// `--restore-previous <root>`: restore a root in `store recover`.
    RestorePrevious,
    /// `--scope once|workflow`: the scope of `approvals approve`.
    Scope,
    /// `--session`: the key of `credential set` is held for this process only.
    Session,
}

impl Flag {
    /// The flag as typed on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "--json",
            Self::Version => "--version",
            Self::DataDir => "--data-dir",
            Self::Root => "--root",
            Self::RestorePrevious => "--restore-previous",
            Self::Scope => "--scope",
            Self::Session => "--session",
        }
    }
}

/// Why the arguments could not be turned into a [`Command`]. Every variant is a usage error (exit 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UsageError {
    /// No command was given.
    NoCommand,
    /// `store` was given without a subcommand.
    MissingSubcommand,
    /// An argument is not valid UTF-8.
    NonUtf8Argument,
    /// The command (or help topic) does not exist.
    UnknownCommand,
    /// A flag that no command accepts.
    UnknownFlag,
    /// A positional argument the command does not take.
    UnexpectedArgument,
    /// The same flag twice.
    DuplicateFlag(Flag),
    /// A flag that needs a value has none.
    MissingValue(Flag),
    /// A command that uses a data directory, without `--data-dir`.
    MissingDataDir,
    /// `store show` without a root.
    MissingRoot,
    /// A root name that is not one of the nine roots.
    UnknownRoot,
    /// A known flag the chosen command does not accept.
    FlagNotAllowed(Flag),
    /// `--version` together with a command.
    VersionWithCommand,
    /// A command group (`approvals`, `credential`, `dev`) without a subcommand.
    MissingSubcommandFor(&'static str),
    /// `approvals approve|deny|revoke` without an approval identifier.
    MissingId,
    /// An approval identifier that is not a canonical UUID.
    InvalidId,
    /// A `--scope` value other than `once` or `workflow`.
    InvalidScope,
    /// `credential set` without `--session` (the only supported mode).
    SessionRequired,
    /// `dev env-preview` without a tool name.
    MissingTool,
    /// A tool name that is not one of the seven profiles.
    UnknownTool,
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::NoCommand => "no command given",
            Self::MissingSubcommand => "the store command needs a subcommand",
            Self::NonUtf8Argument => "an argument is not valid UTF-8",
            Self::UnknownCommand => "unknown command or help topic",
            Self::UnknownFlag => "unknown flag",
            Self::UnexpectedArgument => "unexpected argument",
            Self::DuplicateFlag(flag) => {
                return write!(f, "{} given more than once", flag.as_str());
            }
            Self::MissingValue(flag) => return write!(f, "{} needs a value", flag.as_str()),
            Self::MissingDataDir => "--data-dir is required for this command",
            Self::MissingRoot => "a root name is required",
            Self::UnknownRoot => "unknown root name",
            Self::FlagNotAllowed(flag) => {
                return write!(f, "{} is not accepted by this command", flag.as_str());
            }
            Self::VersionWithCommand => "--version cannot be combined with a command",
            Self::MissingSubcommandFor(group) => {
                return write!(f, "the {group} command needs a subcommand");
            }
            Self::MissingId => "an approval identifier is required",
            Self::InvalidId => "the approval identifier is not a valid identifier",
            Self::InvalidScope => "the scope must be once or workflow",
            Self::SessionRequired => "credential set needs --session (the only supported mode)",
            Self::MissingTool => "a tool name is required",
            Self::UnknownTool => "unknown tool name",
        };
        f.write_str(text)
    }
}

impl std::error::Error for UsageError {}

/// The scope of `approvals approve` (request §5.7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ApprovalScope {
    /// `--scope once` (the default): this one operation (a `run` request keeps its run lifetime).
    Once,
    /// `--scope workflow`: the saved workflow at its current revision.
    Workflow,
}

/// A parsed command with its arguments.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// `--version`.
    Version,
    /// `help [command]`; `topic` holds the words after `help` (at most two).
    Help {
        /// The words after `help`, for example `["store", "init"]`.
        topic: Vec<String>,
    },
    /// `features`.
    Features,
    /// `store init --data-dir D`.
    StoreInit {
        /// The data directory.
        data_dir: PathBuf,
    },
    /// `store validate --data-dir D [--root K]`.
    StoreValidate {
        /// The data directory.
        data_dir: PathBuf,
        /// Restrict to one root.
        root: Option<StoreKind>,
    },
    /// `store show --data-dir D <root>`.
    StoreShow {
        /// The data directory.
        data_dir: PathBuf,
        /// The root to show.
        root: StoreKind,
    },
    /// `store recover --data-dir D [--restore-previous <root>]`.
    StoreRecover {
        /// The data directory.
        data_dir: PathBuf,
        /// Root to restore from its previous generation.
        restore_previous: Option<StoreKind>,
    },
    /// `approvals list --data-dir D`.
    ApprovalsList {
        /// The data directory.
        data_dir: PathBuf,
    },
    /// `approvals approve <id> --data-dir D [--scope once|workflow]`.
    ApprovalsApprove {
        /// The data directory.
        data_dir: PathBuf,
        /// The pending approval.
        id: ApprovalId,
        /// The scope of the grant.
        scope: ApprovalScope,
    },
    /// `approvals deny <id> --data-dir D`.
    ApprovalsDeny {
        /// The data directory.
        data_dir: PathBuf,
        /// The pending approval.
        id: ApprovalId,
    },
    /// `approvals revoke <id> --data-dir D`.
    ApprovalsRevoke {
        /// The data directory.
        data_dir: PathBuf,
        /// The approved grant.
        id: ApprovalId,
    },
    /// `credential set --session` (the key is read from standard input).
    CredentialSet,
    /// `credential status`.
    CredentialStatus,
    /// `credential delete`.
    CredentialDelete,
    /// `dev env-preview <tool> --data-dir D`.
    DevEnvPreview {
        /// The data directory the tool directories are placed under.
        data_dir: PathBuf,
        /// The tool profile.
        tool: ToolProfile,
    },
    /// `dev permission-check --data-dir D` (a JSON request on standard input).
    DevPermissionCheck {
        /// The data directory.
        data_dir: PathBuf,
    },
}

impl Command {
    /// The command name as listed in [`crate::commands::COMMANDS`] and in the feature index.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Version => "--version",
            Self::Help { .. } => "help",
            Self::Features => "features",
            Self::StoreInit { .. } => "store init",
            Self::StoreValidate { .. } => "store validate",
            Self::StoreShow { .. } => "store show",
            Self::StoreRecover { .. } => "store recover",
            Self::ApprovalsList { .. } => "approvals list",
            Self::ApprovalsApprove { .. } => "approvals approve",
            Self::ApprovalsDeny { .. } => "approvals deny",
            Self::ApprovalsRevoke { .. } => "approvals revoke",
            Self::CredentialSet => "credential set",
            Self::CredentialStatus => "credential status",
            Self::CredentialDelete => "credential delete",
            Self::DevEnvPreview { .. } => "dev env-preview",
            Self::DevPermissionCheck { .. } => "dev permission-check",
        }
    }
}

/// A parsed invocation: the global `--json` flag and the command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Invocation {
    /// Whether output is JSON Lines.
    pub json: bool,
    /// The command to run.
    pub command: Command,
}

/// The flags and words found on the command line, before they are checked against a command.
#[derive(Default)]
struct Raw {
    json: bool,
    version: bool,
    data_dir: Option<PathBuf>,
    root: Option<String>,
    restore_previous: Option<String>,
    scope: Option<String>,
    session: bool,
    words: Vec<String>,
}

/// Splits `tokens` into flags and words.
fn scan(tokens: &[String]) -> Result<Raw, UsageError> {
    let mut raw = Raw::default();
    let mut iter = tokens.iter();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--json" => raw.json = true,
            "--version" => {
                if raw.version {
                    return Err(UsageError::DuplicateFlag(Flag::Version));
                }
                raw.version = true;
            }
            "--data-dir" => {
                if raw.data_dir.is_some() {
                    return Err(UsageError::DuplicateFlag(Flag::DataDir));
                }
                raw.data_dir = Some(PathBuf::from(value(&mut iter, Flag::DataDir)?));
            }
            "--root" => {
                if raw.root.is_some() {
                    return Err(UsageError::DuplicateFlag(Flag::Root));
                }
                raw.root = Some(value(&mut iter, Flag::Root)?.to_owned());
            }
            "--restore-previous" => {
                if raw.restore_previous.is_some() {
                    return Err(UsageError::DuplicateFlag(Flag::RestorePrevious));
                }
                raw.restore_previous = Some(value(&mut iter, Flag::RestorePrevious)?.to_owned());
            }
            "--scope" => {
                if raw.scope.is_some() {
                    return Err(UsageError::DuplicateFlag(Flag::Scope));
                }
                raw.scope = Some(value(&mut iter, Flag::Scope)?.to_owned());
            }
            "--session" => {
                if raw.session {
                    return Err(UsageError::DuplicateFlag(Flag::Session));
                }
                raw.session = true;
            }
            other if other.starts_with('-') => return Err(UsageError::UnknownFlag),
            other => raw.words.push(other.to_owned()),
        }
    }
    Ok(raw)
}

/// The value of `flag`: the next token, which must not look like a flag.
fn value<'a>(iter: &mut std::slice::Iter<'a, String>, flag: Flag) -> Result<&'a str, UsageError> {
    match iter.next() {
        Some(next) if !next.starts_with('-') && !next.is_empty() => Ok(next.as_str()),
        _ => Err(UsageError::MissingValue(flag)),
    }
}

/// Resolves a root name.
fn root_of(name: &str) -> Result<StoreKind, UsageError> {
    parse_root(name).ok_or(UsageError::UnknownRoot)
}

/// Rejects `flag` if it was given.
fn deny(present: bool, flag: Flag) -> Result<(), UsageError> {
    if present {
        Err(UsageError::FlagNotAllowed(flag))
    } else {
        Ok(())
    }
}

/// Parses the process arguments (`args[0]`, the program name, is skipped).
///
/// Errors: any [`UsageError`]. The result of a successful parse is a valid command, but whether a data
/// directory exists is not checked here.
pub fn parse<I>(args: I) -> Result<Invocation, UsageError>
where
    I: IntoIterator<Item = OsString>,
{
    let tokens: Vec<String> = args
        .into_iter()
        .skip(1)
        .map(|a| a.into_string().map_err(|_| UsageError::NonUtf8Argument))
        .collect::<Result<_, _>>()?;
    let raw = scan(&tokens)?;
    let words: Vec<&str> = raw.words.iter().map(String::as_str).collect();
    // The P02 commands accept neither of the P03 flags.
    let store_flags = |extra_root: bool, extra_restore: bool| -> Result<(), UsageError> {
        deny(extra_root, Flag::Root)?;
        deny(extra_restore, Flag::RestorePrevious)?;
        deny(raw.scope.is_some(), Flag::Scope)?;
        deny(raw.session, Flag::Session)
    };
    if raw.version {
        if !words.is_empty()
            || raw.data_dir.is_some()
            || raw.root.is_some()
            || raw.restore_previous.is_some()
            || raw.scope.is_some()
            || raw.session
        {
            return Err(UsageError::VersionWithCommand);
        }
        return Ok(Invocation {
            json: raw.json,
            command: Command::Version,
        });
    }
    // The P02 command groups accept neither of the P03 flags.
    if matches!(words.first(), Some(&("help" | "features" | "store"))) {
        deny(raw.scope.is_some(), Flag::Scope)?;
        deny(raw.session, Flag::Session)?;
    }
    let command = match words.as_slice() {
        [] => return Err(UsageError::NoCommand),
        ["help", topic @ ..] => {
            deny(raw.data_dir.is_some(), Flag::DataDir)?;
            store_flags(raw.root.is_some(), raw.restore_previous.is_some())?;
            if topic.len() > 2 {
                return Err(UsageError::UnexpectedArgument);
            }
            Command::Help {
                topic: topic.iter().map(|t| (*t).to_owned()).collect(),
            }
        }
        ["features"] => {
            deny(raw.data_dir.is_some(), Flag::DataDir)?;
            store_flags(raw.root.is_some(), raw.restore_previous.is_some())?;
            Command::Features
        }
        ["features", ..] => return Err(UsageError::UnexpectedArgument),
        ["store"] => return Err(UsageError::MissingSubcommand),
        ["store", sub, rest @ ..] => {
            let sub = *sub;
            if !matches!(sub, "init" | "validate" | "show" | "recover") {
                return Err(UsageError::UnknownCommand);
            }
            let data_dir = raw.data_dir.clone().ok_or(UsageError::MissingDataDir);
            match sub {
                "init" => {
                    store_flags(raw.root.is_some(), raw.restore_previous.is_some())?;
                    if !rest.is_empty() {
                        return Err(UsageError::UnexpectedArgument);
                    }
                    Command::StoreInit {
                        data_dir: data_dir?,
                    }
                }
                "validate" => {
                    deny(raw.restore_previous.is_some(), Flag::RestorePrevious)?;
                    if !rest.is_empty() {
                        return Err(UsageError::UnexpectedArgument);
                    }
                    let data_dir = data_dir?;
                    let root = raw.root.as_deref().map(root_of).transpose()?;
                    Command::StoreValidate { data_dir, root }
                }
                "show" => {
                    store_flags(raw.root.is_some(), raw.restore_previous.is_some())?;
                    let data_dir = data_dir?;
                    match rest {
                        [] => return Err(UsageError::MissingRoot),
                        [name] => Command::StoreShow {
                            data_dir,
                            root: root_of(name)?,
                        },
                        _ => return Err(UsageError::UnexpectedArgument),
                    }
                }
                _ => {
                    deny(raw.root.is_some(), Flag::Root)?;
                    if !rest.is_empty() {
                        return Err(UsageError::UnexpectedArgument);
                    }
                    let data_dir = data_dir?;
                    let restore_previous =
                        raw.restore_previous.as_deref().map(root_of).transpose()?;
                    Command::StoreRecover {
                        data_dir,
                        restore_previous,
                    }
                }
            }
        }
        ["approvals"] => return Err(UsageError::MissingSubcommandFor("approvals")),
        ["approvals", sub, rest @ ..] => parse_approvals(&raw, sub, rest)?,
        ["credential"] => return Err(UsageError::MissingSubcommandFor("credential")),
        ["credential", sub, rest @ ..] => parse_credential(&raw, sub, rest)?,
        ["dev"] => return Err(UsageError::MissingSubcommandFor("dev")),
        ["dev", sub, rest @ ..] => parse_dev(&raw, sub, rest)?,
        _ => return Err(UsageError::UnknownCommand),
    };
    Ok(Invocation {
        json: raw.json,
        command,
    })
}

/// The flags no P03 command accepts: `--root` and `--restore-previous`.
fn deny_store_only(raw: &Raw) -> Result<(), UsageError> {
    deny(raw.root.is_some(), Flag::Root)?;
    deny(raw.restore_previous.is_some(), Flag::RestorePrevious)
}

/// The single approval identifier of `approvals approve|deny|revoke`.
fn approval_id(rest: &[&str]) -> Result<ApprovalId, UsageError> {
    match rest {
        [] => Err(UsageError::MissingId),
        [id] => ApprovalId::parse(id).map_err(|_| UsageError::InvalidId),
        _ => Err(UsageError::UnexpectedArgument),
    }
}

/// `approvals <sub> ...`.
fn parse_approvals(raw: &Raw, sub: &str, rest: &[&str]) -> Result<Command, UsageError> {
    if !matches!(sub, "list" | "approve" | "deny" | "revoke") {
        return Err(UsageError::UnknownCommand);
    }
    deny_store_only(raw)?;
    deny(raw.session, Flag::Session)?;
    if sub != "approve" {
        deny(raw.scope.is_some(), Flag::Scope)?;
    }
    let data_dir = raw.data_dir.clone().ok_or(UsageError::MissingDataDir);
    Ok(match sub {
        "list" => {
            if !rest.is_empty() {
                return Err(UsageError::UnexpectedArgument);
            }
            Command::ApprovalsList {
                data_dir: data_dir?,
            }
        }
        "approve" => {
            let id = approval_id(rest)?;
            let scope = match raw.scope.as_deref() {
                None | Some("once") => ApprovalScope::Once,
                Some("workflow") => ApprovalScope::Workflow,
                Some(_) => return Err(UsageError::InvalidScope),
            };
            Command::ApprovalsApprove {
                data_dir: data_dir?,
                id,
                scope,
            }
        }
        "deny" => {
            let id = approval_id(rest)?;
            Command::ApprovalsDeny {
                data_dir: data_dir?,
                id,
            }
        }
        _ => {
            let id = approval_id(rest)?;
            Command::ApprovalsRevoke {
                data_dir: data_dir?,
                id,
            }
        }
    })
}

/// `credential <sub> ...`: no data directory (the key is held in the process, never stored).
fn parse_credential(raw: &Raw, sub: &str, rest: &[&str]) -> Result<Command, UsageError> {
    if !matches!(sub, "set" | "status" | "delete") {
        return Err(UsageError::UnknownCommand);
    }
    deny_store_only(raw)?;
    deny(raw.data_dir.is_some(), Flag::DataDir)?;
    deny(raw.scope.is_some(), Flag::Scope)?;
    if sub != "set" {
        deny(raw.session, Flag::Session)?;
    }
    if !rest.is_empty() {
        return Err(UsageError::UnexpectedArgument);
    }
    Ok(match sub {
        "set" => {
            if !raw.session {
                return Err(UsageError::SessionRequired);
            }
            Command::CredentialSet
        }
        "status" => Command::CredentialStatus,
        _ => Command::CredentialDelete,
    })
}

/// `dev <sub> ...`.
fn parse_dev(raw: &Raw, sub: &str, rest: &[&str]) -> Result<Command, UsageError> {
    if !matches!(sub, "env-preview" | "permission-check") {
        return Err(UsageError::UnknownCommand);
    }
    deny_store_only(raw)?;
    deny(raw.scope.is_some(), Flag::Scope)?;
    deny(raw.session, Flag::Session)?;
    let data_dir = raw.data_dir.clone().ok_or(UsageError::MissingDataDir);
    if sub == "env-preview" {
        let tool = match rest {
            [] => return Err(UsageError::MissingTool),
            [name] => ToolProfile::parse(name).ok_or(UsageError::UnknownTool)?,
            _ => return Err(UsageError::UnexpectedArgument),
        };
        return Ok(Command::DevEnvPreview {
            data_dir: data_dir?,
            tool,
        });
    }
    if !rest.is_empty() {
        return Err(UsageError::UnexpectedArgument);
    }
    Ok(Command::DevPermissionCheck {
        data_dir: data_dir?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    fn argv(parts: &[&str]) -> Vec<OsString> {
        std::iter::once("pwt-workbench-cli")
            .chain(parts.iter().copied())
            .map(OsString::from)
            .collect()
    }

    fn parse_str(parts: &[&str]) -> Result<Invocation, UsageError> {
        parse(argv(parts))
    }

    /// Fails if `--json` is not recognised before, after or between the words.
    #[test]
    fn json_flag_is_global() {
        for parts in [["--json", "features"], ["features", "--json"]] {
            let inv = parse_str(&parts).unwrap();
            assert!(inv.json);
            assert_eq!(inv.command, Command::Features);
        }
        assert!(!parse_str(&["features"]).unwrap().json);
    }

    /// Fails if a non-UTF-8 argument is accepted.
    #[test]
    fn non_utf8_argument_is_a_usage_error() {
        let mut args = argv(&["features"]);
        args.push(OsString::from_vec(vec![0x66, 0xff, 0x6f]));
        assert_eq!(parse(args), Err(UsageError::NonUtf8Argument));
    }

    /// Fails if a flag value may be missing, may be another flag, or a flag may repeat.
    #[test]
    fn flag_values_are_required_and_not_repeatable() {
        assert_eq!(
            parse_str(&["store", "init", "--data-dir"]),
            Err(UsageError::MissingValue(Flag::DataDir))
        );
        assert_eq!(
            parse_str(&["store", "init", "--data-dir", "--json"]),
            Err(UsageError::MissingValue(Flag::DataDir))
        );
        assert_eq!(
            parse_str(&["store", "init", "--data-dir", "/a", "--data-dir", "/b"]),
            Err(UsageError::DuplicateFlag(Flag::DataDir))
        );
        assert_eq!(
            parse_str(&["store", "init"]),
            Err(UsageError::MissingDataDir)
        );
    }

    /// Fails if a flag is accepted by a command that does not define it.
    #[test]
    fn flags_are_checked_against_the_command() {
        assert_eq!(
            parse_str(&["store", "init", "--data-dir", "/a", "--root", "log"]),
            Err(UsageError::FlagNotAllowed(Flag::Root))
        );
        assert_eq!(
            parse_str(&[
                "store",
                "validate",
                "--data-dir",
                "/a",
                "--restore-previous",
                "log"
            ]),
            Err(UsageError::FlagNotAllowed(Flag::RestorePrevious))
        );
        assert_eq!(
            parse_str(&["features", "--data-dir", "/a"]),
            Err(UsageError::FlagNotAllowed(Flag::DataDir))
        );
        assert_eq!(
            parse_str(&["--version", "features"]),
            Err(UsageError::VersionWithCommand)
        );
        assert_eq!(parse_str(&["--bogus"]), Err(UsageError::UnknownFlag));
        assert_eq!(parse_str(&["bogus"]), Err(UsageError::UnknownCommand));
        assert_eq!(parse_str(&[]), Err(UsageError::NoCommand));
        assert_eq!(parse_str(&["store"]), Err(UsageError::MissingSubcommand));
        assert_eq!(
            parse_str(&["store", "bogus", "--data-dir", "/a"]),
            Err(UsageError::UnknownCommand)
        );
    }

    /// Fails if a root name is not resolved the same way for `show`, `--root` and `--restore-previous`.
    #[test]
    fn roots_resolve_for_every_command() {
        assert_eq!(
            parse_str(&["store", "show", "--data-dir", "/a", "tool_manifest"])
                .unwrap()
                .command,
            Command::StoreShow {
                data_dir: PathBuf::from("/a"),
                root: StoreKind::ToolManifest
            }
        );
        assert_eq!(
            parse_str(&["store", "validate", "--data-dir", "/a", "--root", "log"])
                .unwrap()
                .command,
            Command::StoreValidate {
                data_dir: PathBuf::from("/a"),
                root: Some(StoreKind::Log)
            }
        );
        assert_eq!(
            parse_str(&[
                "store",
                "recover",
                "--data-dir",
                "/a",
                "--restore-previous",
                "repositories"
            ])
            .unwrap()
            .command,
            Command::StoreRecover {
                data_dir: PathBuf::from("/a"),
                restore_previous: Some(StoreKind::Repositories)
            }
        );
        assert_eq!(
            parse_str(&["store", "show", "--data-dir", "/a"]),
            Err(UsageError::MissingRoot)
        );
        assert_eq!(
            parse_str(&["store", "show", "--data-dir", "/a", "report"]),
            Err(UsageError::UnknownRoot)
        );
        assert_eq!(
            parse_str(&["store", "validate", "--data-dir", "/a", "--root", "nope"]),
            Err(UsageError::UnknownRoot)
        );
    }

    /// Fails if a usage message repeats an argument.
    #[test]
    fn usage_messages_do_not_echo_arguments() {
        let err = parse_str(&["secret-marker-xyz"]).unwrap_err();
        assert!(!err.to_string().contains("secret-marker-xyz"));
    }

    const ID: &str = "00000000-0000-4000-8000-0000000000aa";

    fn id() -> ApprovalId {
        ApprovalId::parse(ID).unwrap()
    }

    /// Fails if an approvals command parses to the wrong variant, scope default or data directory.
    #[test]
    fn approvals_commands_parse() {
        let d = ["--data-dir", "/a"];
        let cmd = |parts: &[&str]| {
            let mut all = parts.to_vec();
            all.extend(d);
            parse_str(&all).unwrap().command
        };
        assert_eq!(
            cmd(&["approvals", "list"]),
            Command::ApprovalsList {
                data_dir: PathBuf::from("/a")
            }
        );
        assert_eq!(
            cmd(&["approvals", "approve", ID]),
            Command::ApprovalsApprove {
                data_dir: PathBuf::from("/a"),
                id: id(),
                scope: ApprovalScope::Once
            }
        );
        assert_eq!(
            cmd(&["approvals", "approve", ID, "--scope", "workflow"]),
            Command::ApprovalsApprove {
                data_dir: PathBuf::from("/a"),
                id: id(),
                scope: ApprovalScope::Workflow
            }
        );
        assert_eq!(
            cmd(&["approvals", "approve", "--scope", "once", ID]),
            Command::ApprovalsApprove {
                data_dir: PathBuf::from("/a"),
                id: id(),
                scope: ApprovalScope::Once
            }
        );
        assert_eq!(
            cmd(&["approvals", "deny", ID]),
            Command::ApprovalsDeny {
                data_dir: PathBuf::from("/a"),
                id: id()
            }
        );
        assert_eq!(
            cmd(&["approvals", "revoke", ID]),
            Command::ApprovalsRevoke {
                data_dir: PathBuf::from("/a"),
                id: id()
            }
        );
    }

    /// Fails if an approvals mistake is not the specific usage error: missing or malformed ID, bad scope,
    /// `--scope` on another subcommand, no data directory, other flags, extra words, no subcommand.
    #[test]
    fn approvals_mistakes_are_specific_usage_errors() {
        let with = |parts: &[&str]| {
            let mut all = parts.to_vec();
            all.extend(["--data-dir", "/a"]);
            parse_str(&all)
        };
        assert_eq!(with(&["approvals", "approve"]), Err(UsageError::MissingId));
        assert_eq!(
            with(&["approvals", "approve", "nope"]),
            Err(UsageError::InvalidId)
        );
        assert_eq!(
            with(&["approvals", "approve", ID, ID]),
            Err(UsageError::UnexpectedArgument)
        );
        assert_eq!(
            with(&["approvals", "approve", ID, "--scope", "x"]),
            Err(UsageError::InvalidScope)
        );
        assert_eq!(
            with(&["approvals", "deny", ID, "--scope", "once"]),
            Err(UsageError::FlagNotAllowed(Flag::Scope))
        );
        assert_eq!(
            with(&["approvals", "list", "--session"]),
            Err(UsageError::FlagNotAllowed(Flag::Session))
        );
        assert_eq!(
            with(&["approvals", "list", "--root", "log"]),
            Err(UsageError::FlagNotAllowed(Flag::Root))
        );
        assert_eq!(
            with(&["approvals", "list", "x"]),
            Err(UsageError::UnexpectedArgument)
        );
        assert_eq!(
            with(&["approvals", "bogus"]),
            Err(UsageError::UnknownCommand)
        );
        assert_eq!(
            parse_str(&["approvals", "approve", ID]),
            Err(UsageError::MissingDataDir)
        );
        assert_eq!(
            parse_str(&["approvals"]),
            Err(UsageError::MissingSubcommandFor("approvals"))
        );
    }

    /// Fails if the credential commands accept a data directory, miss the `--session` requirement or accept
    /// it where it has no meaning.
    #[test]
    fn credential_commands_parse() {
        assert_eq!(
            parse_str(&["credential", "set", "--session"])
                .unwrap()
                .command,
            Command::CredentialSet
        );
        assert_eq!(
            parse_str(&["--json", "credential", "status"])
                .unwrap()
                .command,
            Command::CredentialStatus
        );
        assert_eq!(
            parse_str(&["credential", "delete"]).unwrap().command,
            Command::CredentialDelete
        );
        assert_eq!(
            parse_str(&["credential", "set"]),
            Err(UsageError::SessionRequired)
        );
        assert_eq!(
            parse_str(&["credential", "status", "--session"]),
            Err(UsageError::FlagNotAllowed(Flag::Session))
        );
        assert_eq!(
            parse_str(&["credential", "status", "--data-dir", "/a"]),
            Err(UsageError::FlagNotAllowed(Flag::DataDir))
        );
        assert_eq!(
            parse_str(&["credential"]),
            Err(UsageError::MissingSubcommandFor("credential"))
        );
    }

    /// Fails if a `dev` command parses to the wrong variant, accepts an unknown tool, or accepts a
    /// positional word where it takes none.
    #[test]
    fn dev_commands_parse() {
        for tool in ToolProfile::ALL {
            assert_eq!(
                parse_str(&["dev", "env-preview", tool.as_str(), "--data-dir", "/a"])
                    .unwrap()
                    .command,
                Command::DevEnvPreview {
                    data_dir: PathBuf::from("/a"),
                    tool
                }
            );
        }
        assert_eq!(
            parse_str(&["dev", "permission-check", "--data-dir", "/a"])
                .unwrap()
                .command,
            Command::DevPermissionCheck {
                data_dir: PathBuf::from("/a")
            }
        );
        let d = ["--data-dir", "/a"];
        let with = |parts: &[&str]| {
            let mut all = parts.to_vec();
            all.extend(d);
            parse_str(&all)
        };
        assert_eq!(with(&["dev", "env-preview"]), Err(UsageError::MissingTool));
        assert_eq!(
            with(&["dev", "env-preview", "nope"]),
            Err(UsageError::UnknownTool)
        );
        assert_eq!(
            with(&["dev", "env-preview", "git", "git"]),
            Err(UsageError::UnexpectedArgument)
        );
        assert_eq!(
            with(&["dev", "permission-check", "x"]),
            Err(UsageError::UnexpectedArgument)
        );
        assert_eq!(
            parse_str(&["dev", "permission-check"]),
            Err(UsageError::MissingDataDir)
        );
        assert_eq!(
            parse_str(&["dev"]),
            Err(UsageError::MissingSubcommandFor("dev"))
        );
    }

    /// Fails if a new flag is accepted by a P02 command, or `--scope` / `--session` repeat.
    #[test]
    fn new_flags_are_rejected_by_the_p02_commands() {
        for parts in [
            vec!["features", "--scope", "once"],
            vec!["features", "--session"],
            vec!["help", "--session"],
            vec!["store", "init", "--data-dir", "/a", "--scope", "once"],
            vec!["store", "show", "--data-dir", "/a", "log", "--session"],
            vec!["store", "validate", "--data-dir", "/a", "--session"],
            vec!["store", "recover", "--data-dir", "/a", "--scope", "once"],
        ] {
            assert!(
                matches!(parse_str(&parts), Err(UsageError::FlagNotAllowed(_))),
                "{parts:?}"
            );
        }
        assert_eq!(
            parse_str(&["--version", "--session"]),
            Err(UsageError::VersionWithCommand)
        );
        assert_eq!(
            parse_str(&["credential", "set", "--session", "--session"]),
            Err(UsageError::DuplicateFlag(Flag::Session))
        );
    }

    /// Fails if a command name does not match what the parser produced for it.
    #[test]
    fn new_command_names_match_the_table() {
        let names: Vec<&str> = [
            Command::ApprovalsList {
                data_dir: PathBuf::new(),
            },
            Command::CredentialSet,
            Command::CredentialStatus,
            Command::CredentialDelete,
            Command::DevPermissionCheck {
                data_dir: PathBuf::new(),
            },
        ]
        .iter()
        .map(Command::name)
        .collect();
        assert_eq!(
            names,
            [
                "approvals list",
                "credential set",
                "credential status",
                "credential delete",
                "dev permission-check"
            ]
        );
    }
}

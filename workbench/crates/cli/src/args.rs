//! Argument parsing for the headless CLI (request §5.9). Standard library only.
//!
//! Grammar: `pwt-workbench-cli [--json] <command> [flags]`. Flags may appear anywhere after the program
//! name; every flag takes a separate value (`--data-dir D`, not `--data-dir=D`). Every argument must be
//! valid UTF-8, so a usage message never needs to repeat raw bytes; and no usage message repeats any
//! argument at all.
//!
//! The parser performs no file-system access and reads no environment: `--data-dir` is mandatory for store
//! commands and there is no default. Whether the directory exists and is private is decided later by
//! `application::dev_store`.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use application::dev_store::parse_root;
use domain::store::StoreKind;

/// The flags the CLI knows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Flag {
    /// `--json`: JSON Lines output.
    Json,
    /// `--version`: print the contract versions.
    Version,
    /// `--data-dir <dir>`: the data directory of a store command.
    DataDir,
    /// `--root <root>`: restrict `store validate` to one root.
    Root,
    /// `--restore-previous <root>`: restore a root in `store recover`.
    RestorePrevious,
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
    /// A store command without `--data-dir`.
    MissingDataDir,
    /// `store show` without a root.
    MissingRoot,
    /// A root name that is not one of the nine roots.
    UnknownRoot,
    /// A known flag the chosen command does not accept.
    FlagNotAllowed(Flag),
    /// `--version` together with a command.
    VersionWithCommand,
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
            Self::MissingDataDir => "--data-dir is required for store commands",
            Self::MissingRoot => "a root name is required",
            Self::UnknownRoot => "unknown root name",
            Self::FlagNotAllowed(flag) => {
                return write!(f, "{} is not accepted by this command", flag.as_str());
            }
            Self::VersionWithCommand => "--version cannot be combined with a command",
        };
        f.write_str(text)
    }
}

impl std::error::Error for UsageError {}

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
    let store_flags = |extra_root: bool, extra_restore: bool| -> Result<(), UsageError> {
        deny(extra_root, Flag::Root)?;
        deny(extra_restore, Flag::RestorePrevious)
    };
    if raw.version {
        if !words.is_empty()
            || raw.data_dir.is_some()
            || raw.root.is_some()
            || raw.restore_previous.is_some()
        {
            return Err(UsageError::VersionWithCommand);
        }
        return Ok(Invocation {
            json: raw.json,
            command: Command::Version,
        });
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
        _ => return Err(UsageError::UnknownCommand),
    };
    Ok(Invocation {
        json: raw.json,
        command,
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
}

//! CLI commands (request §5.9): the command table, the exit codes and the execution of a parsed command.
//!
//! The CLI calls only `application` (and uses `domain` types): the store commands go through
//! `application::dev_store`, which owns every file-system and persistence call. Nothing here reads the
//! environment or a path.

use std::io::Write;

use application::dev_store::{self, DevError, DevOutcome, FailureClass};
use domain::text::sanitise;
use domain::versions::{
    APP_CONTRACT_VERSION, STORE_SCHEMA_VERSION, UI_SCHEMA_VERSION, WRAPPER_PROTOCOL_VERSION,
};
use serde_json::json;

use crate::args::{Command, UsageError};
use crate::features;
use crate::output::Output;

/// The version of this crate (compile time).
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The process exit codes (request §5.9), defined once.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Exit {
    /// 0: the command executed. Results may include invalid stores; this is never "clean".
    Executed,
    /// 2: usage error.
    Usage,
    /// 3: the store is locked by another instance.
    Locked,
    /// 4: storage failure or halted writer.
    Storage,
    /// 5: the operation failed (typed).
    OperationFailed,
    /// 6: approval required. Reserved; no P02 command returns it.
    ApprovalRequired,
}

impl Exit {
    /// Every exit code, in numeric order.
    pub const ALL: [Self; 6] = [
        Self::Executed,
        Self::Usage,
        Self::Locked,
        Self::Storage,
        Self::OperationFailed,
        Self::ApprovalRequired,
    ];

    /// The numeric process exit code.
    pub fn code(self) -> u8 {
        match self {
            Self::Executed => 0,
            Self::Usage => 2,
            Self::Locked => 3,
            Self::Storage => 4,
            Self::OperationFailed => 5,
            Self::ApprovalRequired => 6,
        }
    }

    /// The exit code for a failure class of a `dev` store command.
    pub fn for_failure(class: FailureClass) -> Self {
        match class {
            FailureClass::Locked => Self::Locked,
            FailureClass::Storage => Self::Storage,
            FailureClass::Operation => Self::OperationFailed,
        }
    }
}

/// One entry of the command table, shown by `help`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CommandInfo {
    /// The command name, for example `store show`.
    pub name: &'static str,
    /// The usage line.
    pub usage: &'static str,
    /// One-sentence description.
    pub summary: &'static str,
    /// Whether `--data-dir` is required.
    pub data_dir: bool,
    /// `real` for the production path, `dev` for developer entry points.
    pub kind: &'static str,
}

/// Every command, in the order `help` lists them.
pub const COMMANDS: [CommandInfo; 7] = [
    CommandInfo {
        name: "--version",
        usage: "pwt-workbench-cli --version",
        summary: "print the application contract, store schema, wrapper protocol, UI schema and crate versions",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "help",
        usage: "pwt-workbench-cli help [command]",
        summary: "list the commands, or describe one (for example: help store, help store init)",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "features",
        usage: "pwt-workbench-cli features",
        summary: "print the feature index: requirement, command, scenario test, mode",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "store init",
        usage: "pwt-workbench-cli store init --data-dir <dir>",
        summary: "create the five collection roots (repo, workflows, approvals, log, tools) if missing; repeatable",
        data_dir: true,
        kind: "dev",
    },
    CommandInfo {
        name: "store validate",
        usage: "pwt-workbench-cli store validate --data-dir <dir> [--root <root>]",
        summary: "read-only: validate every root (or one); an invalid or missing root is reported, not a failure",
        data_dir: true,
        kind: "dev",
    },
    CommandInfo {
        name: "store show",
        usage: "pwt-workbench-cli store show --data-dir <dir> <root>",
        summary: "read-only: print the validated document of a root, or its problems only if it is invalid",
        data_dir: true,
        kind: "dev",
    },
    CommandInfo {
        name: "store recover",
        usage: "pwt-workbench-cli store recover --data-dir <dir> [--restore-previous <root>]",
        summary: "run startup reconciliation and report it; with --restore-previous restore a root from its .prev file",
        data_dir: true,
        kind: "dev",
    },
];

/// The roots a command accepts, for the help text.
const ROOT_HELP: &str = "roots: config, repositories, workflows, approvals, log, suppressions, rag_index, conversations, tool_manifest";

/// The exit-code legend of the help text.
const EXIT_HELP: &str = "exit codes: 0 executed (never \"clean\"), 2 usage error, 3 store locked by another instance, 4 storage failure or halted writer, 5 operation failed, 6 approval required (reserved)";

/// The one-line usage reminder printed after a usage error.
pub const USAGE_LINE: &str = "usage: pwt-workbench-cli [--json] <command> [flags]; run `pwt-workbench-cli help` for the commands";

/// Prints a usage error on standard error and returns the usage exit code.
pub fn usage_error<W: Write, E: Write>(error: UsageError, out: &mut Output<W, E>) -> Exit {
    out.error_line(&sanitise(&format!("error: {error}")));
    out.error_line(&sanitise(USAGE_LINE));
    Exit::Usage
}

/// Runs `command` and writes its output. The returned exit code is the command's own; the caller adjusts it
/// if standard output could not be written.
pub fn execute<W: Write, E: Write>(command: &Command, out: &mut Output<W, E>) -> Exit {
    match command {
        Command::Version => version(out),
        Command::Help { topic } => help(topic, out),
        Command::Features => {
            features::print(out);
            Exit::Executed
        }
        Command::StoreInit { data_dir } => finish(dev_store::init(data_dir), out),
        Command::StoreValidate { data_dir, root } => {
            finish(dev_store::validate(data_dir, *root), out)
        }
        Command::StoreShow { data_dir, root } => finish(dev_store::show(data_dir, *root), out),
        Command::StoreRecover {
            data_dir,
            restore_previous,
        } => finish(dev_store::recover(data_dir, *restore_previous), out),
    }
}

/// Prints the records of a store command, then its failure (if any) on standard error.
fn finish<W: Write, E: Write>(outcome: DevOutcome, out: &mut Output<W, E>) -> Exit {
    for record in &outcome.records {
        out.dev_record(record);
    }
    match outcome.failure {
        None => Exit::Executed,
        Some(error) => {
            out.error_line(&sanitise(&failure_text(&error)));
            Exit::for_failure(error.class())
        }
    }
}

/// The standard-error text of a failure: fixed text, plus the root name where the error concerns one.
fn failure_text(error: &DevError) -> String {
    match error {
        DevError::NoPreviousGeneration(kind)
        | DevError::PreviousInvalid(kind)
        | DevError::Malformed(kind) => {
            format!("error: {error} (root {})", dev_store::root_name(*kind))
        }
        _ => format!("error: {error}"),
    }
}

/// `--version`.
fn version<W: Write, E: Write>(out: &mut Output<W, E>) -> Exit {
    if out.is_json() {
        out.json_line(&json!({
            "mode": "real",
            "record": "version",
            "crate_version": CRATE_VERSION,
            "app_contract_version": APP_CONTRACT_VERSION,
            "store_schema_version": STORE_SCHEMA_VERSION,
            "wrapper_protocol_version": WRAPPER_PROTOCOL_VERSION,
            "ui_schema_version": UI_SCHEMA_VERSION,
        }));
    } else {
        out.line(&sanitise(&format!("pwt-workbench-cli {CRATE_VERSION}")));
        out.line(&sanitise(&format!(
            "application contract version: {APP_CONTRACT_VERSION}"
        )));
        out.line(&sanitise(&format!(
            "store schema version: {STORE_SCHEMA_VERSION}"
        )));
        out.line(&sanitise(&format!(
            "wrapper protocol version: {WRAPPER_PROTOCOL_VERSION}"
        )));
        out.line(&sanitise(&format!(
            "UI schema version: {UI_SCHEMA_VERSION}"
        )));
    }
    Exit::Executed
}

/// The commands a help topic selects: the command with that exact name, or every command that starts with
/// `<topic> ` (so `store` selects the four store commands). `version` is accepted for `--version`.
pub fn commands_for_topic(topic: &[String]) -> Vec<&'static CommandInfo> {
    if topic.is_empty() {
        return COMMANDS.iter().collect();
    }
    let joined = topic.join(" ");
    let wanted = if joined == "version" {
        "--version"
    } else {
        joined.as_str()
    };
    let prefix = format!("{wanted} ");
    COMMANDS
        .iter()
        .filter(|c| c.name == wanted || c.name.starts_with(&prefix))
        .collect()
}

/// `help [command]`.
fn help<W: Write, E: Write>(topic: &[String], out: &mut Output<W, E>) -> Exit {
    let selected = commands_for_topic(topic);
    if selected.is_empty() {
        return usage_error(UsageError::UnknownCommand, out);
    }
    if out.is_json() {
        for c in selected {
            out.json_line(&json!({
                "mode": "real",
                "record": "help",
                "command": c.name,
                "usage": c.usage,
                "summary": c.summary,
                "data_dir_required": c.data_dir,
                "kind": c.kind,
            }));
        }
        return Exit::Executed;
    }
    if topic.is_empty() {
        out.line(&sanitise(
            "pwt-workbench-cli: headless developer CLI (global flag: --json for JSON Lines)",
        ));
        out.line(&sanitise("commands:"));
    }
    for c in &selected {
        out.line(&sanitise(&format!("  {}", c.usage)));
        out.line(&sanitise(&format!("      [{}] {}", c.kind, c.summary)));
    }
    if topic.is_empty() || selected.iter().any(|c| c.data_dir) {
        out.line(&sanitise(ROOT_HELP));
    }
    if topic.is_empty() {
        out.line(&sanitise(EXIT_HELP));
    }
    Exit::Executed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails if an exit code changes value or two variants share one.
    #[test]
    fn exit_codes_are_the_frozen_values() {
        let codes: Vec<u8> = Exit::ALL.iter().map(|e| e.code()).collect();
        assert_eq!(codes, [0, 2, 3, 4, 5, 6]);
    }

    /// Fails if a failure class maps to the wrong exit code.
    #[test]
    fn failure_classes_map_to_exit_codes() {
        assert_eq!(Exit::for_failure(FailureClass::Locked).code(), 3);
        assert_eq!(Exit::for_failure(FailureClass::Storage).code(), 4);
        assert_eq!(Exit::for_failure(FailureClass::Operation).code(), 5);
    }

    /// Fails if a command name appears twice in the table.
    #[test]
    fn command_names_are_unique() {
        for (i, a) in COMMANDS.iter().enumerate() {
            for b in COMMANDS.iter().skip(i + 1) {
                assert_ne!(a.name, b.name);
            }
        }
    }

    /// Fails if `help store` stops selecting exactly the four store commands.
    #[test]
    fn store_topic_selects_the_store_commands() {
        let names: Vec<&str> = commands_for_topic(&[String::from("store")])
            .iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(
            names,
            [
                "store init",
                "store validate",
                "store show",
                "store recover"
            ]
        );
        assert!(commands_for_topic(&[String::from("nope")]).is_empty());
    }
}

//! CLI commands (P02 request §5.9, P03 request §5.7): the command table, the exit codes and the execution of
//! a parsed command.
//!
//! The CLI calls only `application` (and uses `domain` types): the store commands go through
//! `application::dev_store`, the approval commands through `application::approvals`, the credential
//! commands through the session's `SessionCredentialStore`, `dev env-preview` through
//! `application::child_env` and `dev permission-check` through `application::dev_permission`. Each of them
//! owns its file-system and persistence calls. Nothing here reads the environment or opens a path.
//!
//! The session key never leaves the credential store: it is read by `application::credential::read_key`
//! and moved into the store, and no function in this module can print it, because none ever holds it as
//! text.

use std::io::{Read, Write};
use std::path::Path;

use application::approvals::{self, Actor, ApprovalError, AuthOutcome};
use application::child_env::{self, Origin, ToolDirs, ToolProfile};
use application::credential::{self, CredentialStore, KeyFormatError};
use application::dev_permission::{self, CheckOutcome, MAX_REQUEST_BYTES, RequestDocError};
use application::dev_store::{self, DevError, DevOutcome, FailureClass};
use application::permission::{NeedReason, RefusalReason, ScopeDimension};
use domain::contract::{ApprovalDecision, ApprovalLifetime, CredentialStatus};
use domain::ids::ApprovalId;
use domain::text::sanitise;
use domain::versions::{
    APP_CONTRACT_VERSION, STORE_SCHEMA_VERSION, UI_SCHEMA_VERSION, WRAPPER_PROTOCOL_VERSION,
};
use serde::Serialize;
use serde_json::json;

use crate::Session;
use crate::args::{ApprovalScope, Command, UsageError};
use crate::features;
use crate::output::{Output, snake};

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
    /// 6: approval required (`dev permission-check` recorded or found a pending request).
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

/// The P03 commands (request §5.7), in the order `help` lists them after [`COMMANDS`].
pub const P03_COMMANDS: [CommandInfo; 9] = [
    CommandInfo {
        name: "approvals list",
        usage: "pwt-workbench-cli approvals list --data-dir <dir>",
        summary: "read-only: list every approval with its status, scope and destinations (canonical form where the record allows it)",
        data_dir: true,
        kind: "real",
    },
    CommandInfo {
        name: "approvals approve",
        usage: "pwt-workbench-cli approvals approve <id> --data-dir <dir> [--scope once|workflow]",
        summary: "approve a pending request: once (default) or for the saved workflow at its current revision",
        data_dir: true,
        kind: "real",
    },
    CommandInfo {
        name: "approvals deny",
        usage: "pwt-workbench-cli approvals deny <id> --data-dir <dir>",
        summary: "deny a pending request",
        data_dir: true,
        kind: "real",
    },
    CommandInfo {
        name: "approvals revoke",
        usage: "pwt-workbench-cli approvals revoke <id> --data-dir <dir>",
        summary: "revoke an approved grant",
        data_dir: true,
        kind: "real",
    },
    CommandInfo {
        name: "credential set",
        usage: "pwt-workbench-cli credential set --session",
        summary: "read a model key from standard input (a pipe, never a terminal) and hold it for this invocation only",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "credential status",
        usage: "pwt-workbench-cli credential status",
        summary: "print the credential status of this invocation (absent unless set earlier in the same process)",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "credential delete",
        usage: "pwt-workbench-cli credential delete",
        summary: "remove the key held by this invocation, if any",
        data_dir: false,
        kind: "real",
    },
    CommandInfo {
        name: "dev env-preview",
        usage: "pwt-workbench-cli dev env-preview <tool> --data-dir <dir>",
        summary: "print the names and origins of the sanitised child environment of a tool, values masked; nothing is created",
        data_dir: true,
        kind: "dev",
    },
    CommandInfo {
        name: "dev permission-check",
        usage: "pwt-workbench-cli dev permission-check --data-dir <dir>",
        summary: "read a JSON request from standard input and evaluate it against the stored approvals: 0 permitted (a once grant is consumed), 6 approval required (pending request recorded), 5 refused or invalid",
        data_dir: true,
        kind: "dev",
    },
];

/// The tools `dev env-preview` accepts, for the help text.
const TOOL_HELP: &str =
    "tools: git, yamllint, gitleaks, checkov, actionlint, kubeconform, internal";

/// The roots a command accepts, for the help text.
const ROOT_HELP: &str = "roots: config, repositories, workflows, approvals, log, suppressions, rag_index, conversations, tool_manifest";

/// The exit-code legend of the help text.
const EXIT_HELP: &str = "exit codes: 0 executed (never \"clean\"), 2 usage error, 3 store locked by another instance, 4 storage failure or halted writer, 5 operation failed, 6 approval required";

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
///
/// `session` holds the credential store and the host environment snapshot; `stdin` is read only by
/// `credential set --session` and `dev permission-check`, and `stdin_is_terminal` makes the first refuse
/// terminal input without reading it (C5).
pub fn execute<W: Write, E: Write>(
    command: &Command,
    session: &mut Session,
    stdin: &mut dyn Read,
    stdin_is_terminal: bool,
    out: &mut Output<W, E>,
) -> Exit {
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
        Command::ApprovalsList { data_dir } => approvals_list(data_dir, out),
        Command::ApprovalsApprove {
            data_dir,
            id,
            scope,
        } => {
            let decision = match scope {
                ApprovalScope::Once => ApprovalDecision::ApproveOnce,
                ApprovalScope::Workflow => ApprovalDecision::ApproveWorkflowScope,
            };
            approvals_resolve(data_dir, *id, decision, out)
        }
        Command::ApprovalsDeny { data_dir, id } => {
            approvals_resolve(data_dir, *id, ApprovalDecision::Deny, out)
        }
        Command::ApprovalsRevoke { data_dir, id } => approvals_revoke(data_dir, *id, out),
        Command::CredentialSet => credential_set(session, stdin, stdin_is_terminal, out),
        Command::CredentialStatus => {
            let status = session.credentials.status();
            credential_report(status, out)
        }
        Command::CredentialDelete => {
            let status = session.credentials.delete();
            credential_report(status, out)
        }
        Command::DevEnvPreview { data_dir, tool } => env_preview(data_dir, *tool, session, out),
        Command::DevPermissionCheck { data_dir } => {
            permission_check(data_dir, stdin, stdin_is_terminal, out)
        }
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
    format!("error: {}", failure_detail(error))
}

/// The fixed text of a store failure, plus the root name where the error concerns one.
fn failure_detail(error: &DevError) -> String {
    match error {
        DevError::NoPreviousGeneration(kind)
        | DevError::PreviousInvalid(kind)
        | DevError::Malformed(kind) => {
            format!("{error} (root {})", dev_store::root_name(*kind))
        }
        _ => error.to_string(),
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

/// Every command, P02 commands first: the table that the feature index and `help <topic>` refer to.
pub fn all_commands() -> impl Iterator<Item = &'static CommandInfo> {
    COMMANDS.iter().chain(P03_COMMANDS.iter())
}

/// The commands a help topic selects: the command with that exact name, or every command that starts with
/// `<topic> ` (so `store` selects the four store commands). `version` is accepted for `--version`.
///
/// Without a topic every command is listed ([`all_commands`]); a group name (`approvals`, `credential`,
/// `dev`, `store`) lists the commands of that group.
pub fn commands_for_topic(topic: &[String]) -> Vec<&'static CommandInfo> {
    if topic.is_empty() {
        return all_commands().collect();
    }
    let joined = topic.join(" ");
    let wanted = if joined == "version" {
        "--version"
    } else {
        joined.as_str()
    };
    let prefix = format!("{wanted} ");
    all_commands()
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
    if topic.is_empty() || selected.iter().any(|c| c.name.starts_with("store ")) {
        out.line(&sanitise(ROOT_HELP));
    }
    if selected.iter().any(|c| c.name == "dev env-preview") {
        out.line(&sanitise(TOOL_HELP));
    }
    if topic.is_empty() {
        out.line(&sanitise(EXIT_HELP));
    }
    Exit::Executed
}

// ---------------------------------------------------------------------------------------------------
// approvals

/// The exit code and the standard-error text of an approval failure. The text is fixed: it holds no path,
/// identifier or request content.
pub fn approval_failure(error: &ApprovalError) -> (Exit, String) {
    match error {
        ApprovalError::Store(dev) => (
            Exit::for_failure(dev.class()),
            format!("error: approval storage failed: {}", failure_detail(dev)),
        ),
        other => (Exit::OperationFailed, format!("error: {other}")),
    }
}

/// Prints an approval failure on standard error and returns its exit code.
fn report_approval_failure<W: Write, E: Write>(
    error: &ApprovalError,
    out: &mut Output<W, E>,
) -> Exit {
    let (exit, text) = approval_failure(error);
    out.error_line(&sanitise(&text));
    exit
}

/// `approvals list`.
fn approvals_list<W: Write, E: Write>(dir: &Path, out: &mut Output<W, E>) -> Exit {
    match approvals::list(dir) {
        Ok(list) => {
            if list.is_empty() && !out.is_json() {
                out.line(&sanitise("no approvals"));
            }
            for approval in &list {
                out.approval(approval);
            }
            Exit::Executed
        }
        Err(error) => report_approval_failure(&error, out),
    }
}

/// `approvals approve` and `approvals deny`.
fn approvals_resolve<W: Write, E: Write>(
    dir: &Path,
    id: ApprovalId,
    decision: ApprovalDecision,
    out: &mut Output<W, E>,
) -> Exit {
    match approvals::resolve(dir, id, decision, Actor::LocalUserCli) {
        Ok(state) => {
            let state_name = snake(&state);
            let decision_name = snake(&decision);
            if out.is_json() {
                out.json_line(&json!({
                    "mode": "real",
                    "record": "approval_resolved",
                    "approval_id": id,
                    "decision": decision_name,
                    "state": state_name,
                }));
            } else {
                let how = match decision {
                    ApprovalDecision::ApproveOnce => " (once)",
                    ApprovalDecision::ApproveWorkflowScope => " (workflow scope)",
                    ApprovalDecision::Deny => "",
                };
                out.line(&sanitise(&format!("approval {id}: {state_name}{how}")));
            }
            Exit::Executed
        }
        Err(error) => report_approval_failure(&error, out),
    }
}

/// `approvals revoke`.
fn approvals_revoke<W: Write, E: Write>(
    dir: &Path,
    id: ApprovalId,
    out: &mut Output<W, E>,
) -> Exit {
    match approvals::revoke(dir, id, Actor::LocalUserCli) {
        Ok(()) => {
            if out.is_json() {
                out.json_line(&json!({
                    "mode": "real",
                    "record": "approval_revoked",
                    "approval_id": id,
                    "state": "revoked",
                }));
            } else {
                out.line(&sanitise(&format!("approval {id}: revoked")));
            }
            Exit::Executed
        }
        Err(error) => report_approval_failure(&error, out),
    }
}

// ---------------------------------------------------------------------------------------------------
// credential

/// The credential status as the contract names it.
fn status_name(status: CredentialStatus) -> String {
    snake(&status)
}

/// The standard-error text of a key that was refused. Fixed: never the input, not even its length.
pub fn key_error_text(error: KeyFormatError) -> &'static str {
    match error {
        KeyFormatError::Empty => "error: no key was provided on standard input",
        KeyFormatError::TooShort => "error: the key is too short (16 to 4096 characters)",
        KeyFormatError::TooLong => "error: the key is too long (16 to 4096 characters)",
        KeyFormatError::Character => {
            "error: the key must be printable ASCII without spaces or line breaks"
        }
        KeyFormatError::TerminalInput => {
            "error: standard input is a terminal; pipe the key in (nothing was read)"
        }
        KeyFormatError::Read => "error: standard input could not be read",
    }
}

/// `credential set --session`: the key is read from `stdin` (a pipe), checked and moved into the session
/// store. Nothing of it is printed or returned.
fn credential_set<W: Write, E: Write>(
    session: &mut Session,
    stdin: &mut dyn Read,
    stdin_is_terminal: bool,
    out: &mut Output<W, E>,
) -> Exit {
    match credential::read_key(stdin, stdin_is_terminal) {
        Ok(secret) => {
            let status = session.credentials.set(secret);
            if out.is_json() {
                out.json_line(&json!({
                    "mode": "real",
                    "record": "credential",
                    "status": status_name(status),
                    "held": "this invocation only",
                }));
            } else {
                out.line(&sanitise(&format!("credential: {}", status_name(status))));
                out.line(&sanitise(
                    "the key is held for this invocation only (process memory; never written to disk)",
                ));
            }
            Exit::Executed
        }
        Err(error) => {
            out.error_line(&sanitise(key_error_text(error)));
            Exit::OperationFailed
        }
    }
}

/// `credential status` and `credential delete`: prints the status.
fn credential_report<W: Write, E: Write>(status: CredentialStatus, out: &mut Output<W, E>) -> Exit {
    if out.is_json() {
        out.json_line(&json!({
            "mode": "real",
            "record": "credential",
            "status": status_name(status),
        }));
    } else {
        out.line(&sanitise(&format!("credential: {}", status_name(status))));
    }
    Exit::Executed
}

// ---------------------------------------------------------------------------------------------------
// dev env-preview

/// The origin as shown.
fn origin_name(origin: Origin) -> &'static str {
    match origin {
        Origin::Fixed => "fixed",
        Origin::AppDir => "app-dir",
        Origin::Inherited => "inherited",
    }
}

/// The four tool directories, relative to the data directory (the directory itself is never echoed).
const TOOL_DIR_NAMES: [&str; 4] = ["home", "config", "cache", "tmp"];

/// The text a masked value is shown as.
const MASKED: &str = "<masked>";

/// `dev env-preview <tool>`: the child environment the tool would get from the session's host environment
/// and key. Names and origins only; the directories are derived under `<data-dir>/tools/<tool>/` and not
/// created; the data directory is not opened.
fn env_preview<W: Write, E: Write>(
    data_dir: &Path,
    tool: ToolProfile,
    session: &Session,
    out: &mut Output<W, E>,
) -> Exit {
    let failure = |text: &str, out: &mut Output<W, E>| {
        out.error_line(&sanitise(text));
        Exit::OperationFailed
    };
    let dirs = match ToolDirs::under(data_dir, tool) {
        Ok(dirs) => dirs,
        Err(_) => {
            return failure(
                "error: the data directory must be an absolute path without `..` for this command",
                out,
            );
        }
    };
    let env = match child_env::build(tool, &dirs, &session.host_env, &session.credentials) {
        Ok(env) => env,
        Err(child_env::EnvError::SecretValue) => {
            return failure(
                "error: a value of the environment would contain the session key; refused",
                out,
            );
        }
        Err(_) => {
            return failure(
                "error: the environment profile would set a denied variable; refused",
                out,
            );
        }
    };
    let name = tool.as_str();
    let preview = env.preview();
    if out.is_json() {
        let dirs: Vec<String> = TOOL_DIR_NAMES
            .iter()
            .map(|d| format!("<data-dir>/tools/{name}/{d}"))
            .collect();
        out.json_line(&json!({
            "mode": "dev",
            "record": "env_preview",
            "tool": name,
            "directories": dirs,
            "directories_created": false,
            "variables": preview.len(),
        }));
        for line in &preview {
            out.json_line(&json!({
                "mode": "dev",
                "record": "env_var",
                "name": line.name.as_str(),
                "origin": origin_name(line.origin),
                "value": MASKED,
            }));
        }
        return Exit::Executed;
    }
    out.line(&sanitise(&format!("tool: {name}")));
    out.line(&sanitise("directories (derived, not created):"));
    for d in TOOL_DIR_NAMES {
        out.line(&sanitise(&format!("  <data-dir>/tools/{name}/{d}")));
    }
    out.line(&sanitise("environment (names and origins; values masked):"));
    for line in &preview {
        out.line(&sanitise(&format!(
            "  {} [{}] {MASKED}",
            line.name.as_str(),
            origin_name(line.origin)
        )));
    }
    Exit::Executed
}

// ---------------------------------------------------------------------------------------------------
// dev permission-check

/// The snake_case name of a refusal reason, and the approval it refers to (an earlier denial).
fn refusal_parts(reason: RefusalReason) -> (&'static str, Option<ApprovalId>) {
    match reason {
        RefusalReason::RawSecretRequested => ("raw_secret_requested", None),
        RefusalReason::MissingDestination => ("missing_destination", None),
        RefusalReason::UnexpectedDestination => ("unexpected_destination", None),
        RefusalReason::LifetimeBinding => ("lifetime_binding", None),
        RefusalReason::UnknownWorkflow => ("unknown_workflow", None),
        RefusalReason::StaleWorkflowRevision => ("stale_workflow_revision", None),
        RefusalReason::InferenceScope => ("inference_scope", None),
        RefusalReason::PreviouslyDenied(id) => ("previously_denied", Some(id)),
        RefusalReason::InvalidRequest => ("invalid_request", None),
    }
}

/// The snake_case name of a scope dimension.
fn dimension_name(dimension: ScopeDimension) -> &'static str {
    match dimension {
        ScopeDimension::Status => "status",
        ScopeDimension::Capability => "capability",
        ScopeDimension::Fingerprint => "fingerprint",
        ScopeDimension::Workflow => "workflow",
        ScopeDimension::WorkflowRevision => "workflow_revision",
        ScopeDimension::Run => "run",
        ScopeDimension::Repos => "repos",
        ScopeDimension::Destinations => "destinations",
        ScopeDimension::ResourceConstraints => "resource_constraints",
        ScopeDimension::DataCategories => "data_categories",
        ScopeDimension::InferenceScope => "inference_scope",
        ScopeDimension::Lifetime => "lifetime",
        ScopeDimension::Expiry => "expiry",
        ScopeDimension::Consumption => "consumption",
    }
}

/// The name of a document rejection.
fn rejection_name(reason: RequestDocError) -> &'static str {
    match reason {
        RequestDocError::Read => "unreadable_or_too_large",
        RequestDocError::Decode => "not_a_valid_request",
        RequestDocError::Destination => "invalid_destination",
        RequestDocError::Name => "invalid_name",
    }
}

/// The JSON form of one permission-check result.
#[derive(Serialize)]
struct CheckRecord {
    mode: &'static str,
    record: &'static str,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_id: Option<ApprovalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lifetime: Option<ApprovalLifetime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    consumed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimension: Option<&'static str>,
}

impl CheckRecord {
    fn new(outcome: &'static str) -> Self {
        Self {
            mode: "dev",
            record: "permission_check",
            outcome,
            approval_id: None,
            lifetime: None,
            consumed: None,
            reason: None,
            dimension: None,
        }
    }
}

/// Reads the request document: at most [`MAX_REQUEST_BYTES`] bytes (one more is read so that a longer
/// document is seen as too large). `None` if standard input is a terminal (nothing is read) or cannot be
/// read.
fn read_request(stdin: &mut dyn Read, stdin_is_terminal: bool) -> Option<Vec<u8>> {
    if stdin_is_terminal {
        return None;
    }
    let mut doc = Vec::new();
    let limit = u64::try_from(MAX_REQUEST_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    stdin.take(limit).read_to_end(&mut doc).ok()?;
    Some(doc)
}

/// `dev permission-check`: 0 permitted, 6 approval required (the pending request's ID is printed), 5 refused,
/// rejected or failed.
fn permission_check<W: Write, E: Write>(
    dir: &Path,
    stdin: &mut dyn Read,
    stdin_is_terminal: bool,
    out: &mut Output<W, E>,
) -> Exit {
    let outcome = match read_request(stdin, stdin_is_terminal) {
        None => CheckOutcome::Rejected(RequestDocError::Read),
        Some(doc) => match dev_permission::check(dir, &doc) {
            Ok(outcome) => outcome,
            Err(error) => return report_approval_failure(&error, out),
        },
    };
    let mut record;
    let mut text;
    let exit;
    match outcome {
        CheckOutcome::Rejected(reason) => {
            record = CheckRecord::new("rejected");
            record.reason = Some(rejection_name(reason));
            text = format!("rejected: {}", rejection_name(reason));
            out.error_line(&sanitise("error: the request document was rejected"));
            exit = Exit::OperationFailed;
        }
        CheckOutcome::Evaluated(AuthOutcome::Authorised(authorisation)) => {
            let lifetime = authorisation.lifetime();
            let consumed = lifetime == ApprovalLifetime::Once;
            record = CheckRecord::new("permitted");
            record.approval_id = Some(authorisation.approval_id());
            record.lifetime = Some(lifetime);
            record.consumed = Some(consumed);
            text = format!(
                "permitted: approval {} (lifetime {}{})",
                authorisation.approval_id(),
                snake(&lifetime),
                if consumed { ", consumed" } else { "" }
            );
            exit = Exit::Executed;
        }
        CheckOutcome::Evaluated(AuthOutcome::NeedsApproval(reason, id)) => {
            record = CheckRecord::new("needs_approval");
            record.approval_id = Some(id);
            let (why, dimension) = match reason {
                NeedReason::NoGrant => ("no_grant", None),
                NeedReason::AlreadyPending(_) => ("already_pending", None),
                NeedReason::Mismatch { dimension, .. } => {
                    ("mismatch", Some(dimension_name(dimension)))
                }
            };
            record.reason = Some(why);
            record.dimension = dimension;
            text = format!("approval required: pending request {id} (reason: {why}");
            if let Some(dimension) = dimension {
                text.push_str(&format!(", {dimension}"));
            }
            text.push(')');
            out.error_line(&sanitise("error: approval required"));
            exit = Exit::ApprovalRequired;
        }
        CheckOutcome::Evaluated(AuthOutcome::Refused(reason)) => {
            let (why, denied) = refusal_parts(reason);
            record = CheckRecord::new("refused");
            record.reason = Some(why);
            record.approval_id = denied;
            text = format!("refused: {why}");
            out.error_line(&sanitise("error: the request was refused"));
            exit = Exit::OperationFailed;
        }
    }
    if out.is_json() {
        out.json_line(&record);
    } else {
        out.line(&sanitise(&text));
    }
    exit
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

    /// Fails if a command name appears twice across the core and P03 tables, or a usage line does not start
    /// with the program name and the command name.
    #[test]
    fn all_commands_are_unique_and_their_usage_names_them() {
        let all: Vec<&CommandInfo> = all_commands().collect();
        assert_eq!(all.len(), COMMANDS.len() + P03_COMMANDS.len());
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a.name, b.name);
            }
        }
        for c in P03_COMMANDS {
            assert!(
                c.usage
                    .starts_with(&format!("pwt-workbench-cli {}", c.name)),
                "{}",
                c.name
            );
            assert!(matches!(c.kind, "real" | "dev"));
        }
    }

    /// Fails if a group topic stops selecting exactly its commands.
    #[test]
    fn group_topics_select_the_p03_commands() {
        let names = |topic: &str| -> Vec<&str> {
            commands_for_topic(&[topic.to_owned()])
                .iter()
                .map(|c| c.name)
                .collect()
        };
        assert_eq!(
            names("approvals"),
            [
                "approvals list",
                "approvals approve",
                "approvals deny",
                "approvals revoke"
            ]
        );
        assert_eq!(
            names("credential"),
            ["credential set", "credential status", "credential delete"]
        );
        assert_eq!(names("dev"), ["dev env-preview", "dev permission-check"]);
    }

    /// Fails if a key refusal text names or counts the input, or two refusals share a text.
    #[test]
    fn key_refusal_texts_are_fixed_and_distinct() {
        let all = [
            KeyFormatError::Empty,
            KeyFormatError::TooShort,
            KeyFormatError::TooLong,
            KeyFormatError::Character,
            KeyFormatError::TerminalInput,
            KeyFormatError::Read,
        ];
        let texts: Vec<&str> = all.iter().map(|e| key_error_text(*e)).collect();
        for (i, a) in texts.iter().enumerate() {
            assert!(a.starts_with("error: "));
            assert!(!a.contains("pwt-test"));
            for b in texts.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    /// Fails if an approval failure maps to the wrong exit code: store classes keep theirs, everything else
    /// is an operation failure.
    #[test]
    fn approval_failures_map_to_exit_codes() {
        assert_eq!(
            approval_failure(&ApprovalError::Store(DevError::Locked)).0,
            Exit::Locked
        );
        assert_eq!(
            approval_failure(&ApprovalError::Store(DevError::Storage)).0,
            Exit::Storage
        );
        assert_eq!(
            approval_failure(&ApprovalError::Store(DevError::Halted)).0,
            Exit::Storage
        );
        assert_eq!(
            approval_failure(&ApprovalError::Store(DevError::DataDirRejected)).0,
            Exit::OperationFailed
        );
        for error in [
            ApprovalError::NotFound,
            ApprovalError::NotPending,
            ApprovalError::NotRevocable,
            ApprovalError::WorkflowScopeUnavailable,
            ApprovalError::Refused(RefusalReason::InvalidRequest),
            ApprovalError::Emit,
        ] {
            assert_eq!(approval_failure(&error).0, Exit::OperationFailed);
        }
    }
}

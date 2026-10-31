//! Headless developer CLI (D-17; Architecture §3/§4 as amended by CA-04; frozen surfaces
//! `planning/P02-T01-request.md` §5.9 and `planning/P03-T01-request.md` §5.7). Parses arguments into
//! application calls and prints the resulting events; never touches Git, scanners, the store or HTTP
//! directly, and never finds a data directory on its own (`--data-dir` is required for the store, approvals and
//! `dev` commands).
#![forbid(unsafe_code)]

pub mod args;
pub mod commands;
pub mod features;
pub mod output;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use application::child_env::HostEnv;
use application::credential::SessionCredentialStore;
use domain::text::sanitise;

use crate::commands::Exit;
use crate::output::Output;

/// State that lives for one process (`planning/P03-T01-request.md` §5.7, T01-D7): the session credential
/// store (C4: a key set with `credential set --session` lasts for this process only) and the host
/// environment snapshot used by `dev env-preview`. Tests keep one `Session` across several
/// [`run_session`] calls to script set/status/delete in one process. `Debug` shows no key material.
#[derive(Debug)]
pub struct Session {
    credentials: SessionCredentialStore,
    host_env: HostEnv,
}

impl Session {
    /// A session with no key and the given host environment snapshot.
    pub fn new(host_env: HostEnv) -> Self {
        Self {
            credentials: SessionCredentialStore::new(),
            host_env,
        }
    }
}

/// Runs one CLI invocation in a fresh [`Session`] with an empty host environment and a non-terminal
/// standard input. Exit codes (request §5.9): 0 executed (never "clean"), 2 usage error, 3 store locked,
/// 4 storage failure or halted writer, 5 operation failed, 6 approval required.
///
/// `args` includes the program name first, as `std::env::args_os()` does. If standard output cannot be
/// written (for example a closed pipe) the run does not panic: a command that would have exited 0 exits 5
/// instead, because its output was lost.
pub fn run<I, R, W, E>(args: I, stdin: R, stdout: W, stderr: E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    run_session(
        &mut Session::new(HostEnv::empty()),
        args,
        stdin,
        false,
        stdout,
        stderr,
    )
}

/// Runs one CLI invocation within `session`. `stdin_is_terminal` tells `credential set --session` to
/// refuse terminal input (C5). Exit codes as [`run`].
pub fn run_session<I, R, W, E>(
    session: &mut Session,
    args: I,
    stdin: R,
    stdin_is_terminal: bool,
    stdout: W,
    stderr: E,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    let mut stdin = stdin;
    let exit = match args::parse(args) {
        Err(error) => {
            let mut out = Output::new(false, stdout, stderr);
            let exit = commands::usage_error(error, &mut out);
            out.finish();
            exit
        }
        Ok(invocation) => {
            let mut out = Output::new(invocation.json, stdout, stderr);
            let mut exit = commands::execute(
                &invocation.command,
                session,
                &mut stdin,
                stdin_is_terminal,
                &mut out,
            );
            out.finish();
            if out.stdout_failed() && exit == Exit::Executed {
                out.error_line(&sanitise("error: output could not be written"));
                exit = Exit::OperationFailed;
            }
            exit
        }
    };
    ExitCode::from(exit.code())
}

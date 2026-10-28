//! Headless developer CLI (D-17; Architecture §3/§4 as amended by CA-04; frozen surface
//! `planning/P02-T01-request.md` §5.9). Parses arguments into application calls and prints the resulting
//! events; never touches Git, scanners, the store or HTTP directly, and never finds a data directory on
//! its own (`--data-dir` is required for store commands).
#![forbid(unsafe_code)]

pub mod args;
pub mod commands;
pub mod features;
pub mod output;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use domain::text::sanitise;

use crate::commands::Exit;
use crate::output::Output;

/// Runs one CLI invocation. Exit codes (request §5.9): 0 executed (never "clean"), 2 usage error, 3 store
/// locked, 4 storage failure or halted writer, 5 operation failed, 6 approval required (reserved).
///
/// `args` includes the program name first, as `std::env::args_os()` does. `stdin` is not read by any P02
/// command. If standard output cannot be written (for example a closed pipe) the run does not panic: a
/// command that would have exited 0 exits 5 instead, because its output was lost.
pub fn run<I, R, W, E>(args: I, _stdin: R, stdout: W, stderr: E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    let exit = match args::parse(args) {
        Err(error) => {
            let mut out = Output::new(false, stdout, stderr);
            let exit = commands::usage_error(error, &mut out);
            out.finish();
            exit
        }
        Ok(invocation) => {
            let mut out = Output::new(invocation.json, stdout, stderr);
            let mut exit = commands::execute(&invocation.command, &mut out);
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

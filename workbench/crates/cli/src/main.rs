//! `pwt-workbench-cli`: headless developer CLI (D-17, CA-04). All behaviour lives in [`cli::run_session`];
//! this binary only hands over the process arguments, the standard streams, whether standard input is a
//! terminal, and a snapshot of the host environment. This is the only place in production code that reads
//! the environment (`planning/P03-T01-request.md` §5.8); the snapshot is used solely as the input of the
//! child-environment sanitiser.
#![forbid(unsafe_code)]

use std::io::IsTerminal;
use std::process::ExitCode;

use application::child_env::HostEnv;

fn main() -> ExitCode {
    let stdin = std::io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let mut session = cli::Session::new(HostEnv::from_pairs(std::env::vars_os()));
    cli::run_session(
        &mut session,
        std::env::args_os(),
        stdin.lock(),
        stdin_is_terminal,
        std::io::stdout().lock(),
        std::io::stderr().lock(),
    )
}

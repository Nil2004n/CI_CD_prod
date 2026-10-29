//! `pwt-workbench-cli`: headless developer CLI (D-17, CA-04). All behaviour lives in [`cli::run`]; this
//! binary only hands over the process arguments and standard streams.
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run(
        std::env::args_os(),
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        std::io::stderr().lock(),
    )
}

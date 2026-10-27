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

/// Runs one CLI invocation. Exit codes (request §5.9): 0 executed (never "clean"), 2 usage error, 3 store
/// locked, 4 storage failure or halted writer, 5 operation failed, 6 approval required (reserved).
#[expect(
    clippy::todo,
    reason = "P02 B0 stub; implemented by W2 in wave 3 (T12)"
)]
pub fn run<I, R, W, E>(_args: I, _stdin: R, _stdout: W, _stderr: E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    todo!("P02-T12")
}

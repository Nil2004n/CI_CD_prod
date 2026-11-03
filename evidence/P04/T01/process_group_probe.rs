//! Local synthetic cooperative-process probe, compiled and run only by the lead wrapper.
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || (args[0] != "default" && args[0] != "foreground") {
        eprintln!("usage: process_group_probe default|foreground HELPER MARKER");
        return ExitCode::from(2);
    }
    let mut command = Command::new("/usr/bin/timeout");
    command.arg("-k").arg("1s");
    if args[0] == "foreground" {
        command.arg("--foreground");
    }
    let status = command.arg("2s").arg("/usr/bin/python3").arg("-I").arg("-B")
        .arg(&args[1]).arg(&args[2]).process_group(0).status();
    match status {
        Ok(s) => ExitCode::from(s.code().unwrap_or(125).clamp(0, 255) as u8),
        Err(e) => { eprintln!("launcher: {e}"); ExitCode::from(125) }
    }
}

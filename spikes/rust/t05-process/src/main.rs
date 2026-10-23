//! P00-T05 spike: Linux process-group supervision (timeout, cancel, SIGTERM -> SIGKILL
//! escalation, descendant cleanup, output bounds, stderr sanitisation, spawn failure).
//!
//! Spike code, not application code. It must run as PID 1 of a dedicated PID namespace
//! (see evidence/P00/P00-T05/record.md); it refuses to signal anything otherwise.
//! Only `std` and `libc` are used.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(10);
const EXCERPT_BYTES: usize = 512;

struct Limits {
    timeout: Duration,
    grace: Duration,
    cleanup: Duration,
    max_output_bytes: u64,
    cancel_after: Option<Duration>,
}

struct Case {
    id: &'static str,
    expect: &'static str,
    program: String,
    args: Vec<String>,
    limits: Limits,
    check: fn(&Outcome) -> Result<(), String>,
}

#[derive(Default)]
struct StreamResult {
    bytes: u64,
    excerpt_sanitised: String,
    limit_hit: bool,
    finished: bool,
}

#[derive(Default)]
struct Outcome {
    spawn_error: Option<String>,
    pid: i32,
    pgid_observed: i32,
    harness_pid: i32,
    harness_pgid: i32,
    exit_code: Option<i32>,
    term_signal: Option<i32>,
    reason: &'static str,
    method: &'static str,
    signals_sent: Vec<String>,
    leftover_after_child_exit: Vec<String>,
    group_live_after_cleanup: Vec<String>,
    other_ns_processes_after: Vec<String>,
    process_tree_exited: Option<bool>,
    reaped: Vec<String>,
    stdout: StreamResult,
    stderr: StreamResult,
    canary_in_raw_stderr: bool,
    elapsed_ms: u128,
}

/// Replace synthetic canaries (`PWTFAKE_` + [A-Za-z0-9_]*) and escape control bytes.
fn sanitise(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let mut out = String::new();
    let mut rest: &str = &text;
    while let Some(i) = rest.find("PWTFAKE_") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + "PWTFAKE_".len()..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(tail.len());
        out.push_str("[REDACTED-CANARY]");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out.chars()
        .map(|c| {
            if c == '\n' || c == '\t' || !c.is_control() {
                c.to_string()
            } else {
                format!("\\x{:02x}", c as u32)
            }
        })
        .collect()
}

/// (pid, state, ppid, pgrp, session) for every process visible in /proc.
fn proc_table() -> Vec<(i32, char, i32, i32, i32)> {
    let mut rows = Vec::new();
    let Ok(dir) = fs::read_dir("/proc") else { return rows };
    for entry in dir.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else { continue };
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
        let Some(close) = stat.rfind(')') else { continue };
        let f: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        let state = f[0].chars().next().unwrap_or('?');
        let (Ok(ppid), Ok(pgrp), Ok(sid)) = (f[1].parse(), f[2].parse(), f[3].parse()) else { continue };
        rows.push((pid, state, ppid, pgrp, sid));
    }
    rows
}

fn describe(rows: &[(i32, char, i32, i32, i32)]) -> Vec<String> {
    rows.iter()
        .map(|(p, s, pp, pg, sid)| format!("pid={p} state={s} ppid={pp} pgid={pg} sid={sid}"))
        .collect()
}

fn group_members(pgid: i32) -> Vec<(i32, char, i32, i32, i32)> {
    proc_table().into_iter().filter(|r| r.3 == pgid).collect()
}

fn live(rows: &[(i32, char, i32, i32, i32)]) -> Vec<(i32, char, i32, i32, i32)> {
    rows.iter().copied().filter(|r| r.1 != 'Z' && r.1 != 'X').collect()
}

/// Reap any exited process in the fixture group (direct child or reparented to us as PID 1).
fn reap_group(pgid: i32, child: i32, o: &mut Outcome) {
    loop {
        let mut status: libc::c_int = 0;
        let r = unsafe { libc::waitpid(-pgid, &mut status, libc::WNOHANG) };
        if r <= 0 {
            break;
        }
        let es = std::process::ExitStatus::from_raw(status);
        if r == child && o.exit_code.is_none() && o.term_signal.is_none() {
            o.exit_code = es.code();
            o.term_signal = es.signal();
        }
        o.reaped.push(format!("pid={r} code={:?} signal={:?}", es.code(), es.signal()));
    }
}

/// Validate a process-group target before any signal. Returns an error instead of signalling.
fn validate_target(pgid: i32, spawn_pid: i32) -> Result<(), String> {
    let me = unsafe { libc::getpid() };
    let my_pgid = unsafe { libc::getpgrp() };
    if pgid <= 1 {
        return Err(format!("refused: pgid {pgid} <= 1"));
    }
    if pgid != spawn_pid {
        return Err(format!("refused: pgid {pgid} != recorded spawn pid {spawn_pid}"));
    }
    if pgid == me || pgid == my_pgid {
        return Err(format!("refused: pgid {pgid} is the harness pid/pgid"));
    }
    if group_members(pgid).is_empty() {
        return Err(format!("refused: pgid {pgid} has no members"));
    }
    Ok(())
}

fn signal_group(pgid: i32, spawn_pid: i32, sig: i32, name: &str, o: &mut Outcome) -> bool {
    match validate_target(pgid, spawn_pid) {
        Ok(()) => {
            let r = unsafe { libc::kill(-pgid, sig) };
            let err = if r == 0 { String::new() } else { format!(" errno={}", std::io::Error::last_os_error()) };
            o.signals_sent.push(format!("kill(-{pgid}, {name}) rc={r}{err}"));
            r == 0
        }
        Err(e) => {
            o.signals_sent.push(e);
            false
        }
    }
}

/// Wait until no live member remains in the group (reaping as we go) or the deadline passes.
fn wait_group_empty(pgid: i32, child: i32, within: Duration, o: &mut Outcome) -> bool {
    let end = Instant::now() + within;
    loop {
        reap_group(pgid, child, o);
        if live(&group_members(pgid)).is_empty() {
            reap_group(pgid, child, o);
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        thread::sleep(POLL);
    }
}

/// SIGTERM, bounded grace, then SIGKILL, then bounded cleanup wait. Returns the method label.
fn terminate_group(pgid: i32, child: i32, l: &Limits, o: &mut Outcome) -> &'static str {
    if !signal_group(pgid, child, libc::SIGTERM, "SIGTERM", o) {
        return if live(&group_members(pgid)).is_empty() { "not_required" } else { "failed" };
    }
    if wait_group_empty(pgid, child, l.grace, o) {
        return "graceful";
    }
    if !signal_group(pgid, child, libc::SIGKILL, "SIGKILL", o) {
        return "failed";
    }
    if wait_group_empty(pgid, child, l.cleanup, o) { "forced" } else { "failed" }
}

fn reader(
    mut src: impl Read + Send + 'static,
    limit: u64,
    flag: Arc<AtomicBool>,
    keep_raw: bool,
) -> (mpsc::Receiver<(StreamResult, Vec<u8>)>, Arc<AtomicU64>) {
    let (tx, rx) = mpsc::channel();
    let count = Arc::new(AtomicU64::new(0));
    let c2 = count.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut head: Vec<u8> = Vec::new();
        let mut raw_all: Vec<u8> = Vec::new();
        let mut res = StreamResult::default();
        loop {
            match src.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let total = c2.fetch_add(n as u64, Ordering::SeqCst) + n as u64;
                    if head.len() < EXCERPT_BYTES {
                        let take = (EXCERPT_BYTES - head.len()).min(n);
                        head.extend_from_slice(&buf[..take]);
                    }
                    if keep_raw && raw_all.len() < 65536 {
                        raw_all.extend_from_slice(&buf[..n.min(65536 - raw_all.len())]);
                    }
                    if total > limit {
                        res.limit_hit = true;
                        flag.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
        res.bytes = c2.load(Ordering::SeqCst);
        res.excerpt_sanitised = sanitise(&head);
        res.finished = true;
        let _ = tx.send((res, raw_all));
    });
    (rx, count)
}

fn run_case(c: &Case) -> Outcome {
    let mut o = Outcome {
        harness_pid: unsafe { libc::getpid() },
        harness_pgid: unsafe { libc::getpgrp() },
        reason: "none",
        method: "not_required",
        ..Default::default()
    };
    let start = Instant::now();
    let spawned = Command::new(&c.program)
        .args(&c.args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn();
    let mut child = match spawned {
        Ok(ch) => ch,
        Err(e) => {
            o.spawn_error = Some(format!("{:?}: {}", e.kind(), e));
            o.reason = "none";
            o.method = "not_required";
            o.other_ns_processes_after = describe(&others());
            o.process_tree_exited = Some(others().is_empty());
            o.elapsed_ms = start.elapsed().as_millis();
            return o;
        }
    };
    let pid = child.id() as i32;
    o.pid = pid;
    o.pgid_observed = proc_table().iter().find(|r| r.0 == pid).map(|r| r.3).unwrap_or(-1);
    let limit_flag = Arc::new(AtomicBool::new(false));
    let (rx_out, _) = reader(child.stdout.take().unwrap(), c.limits.max_output_bytes, limit_flag.clone(), false);
    let (rx_err, _) = reader(child.stderr.take().unwrap(), c.limits.max_output_bytes, limit_flag.clone(), true);
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(after) = c.limits.cancel_after {
        let cf = cancel.clone();
        thread::spawn(move || {
            thread::sleep(after);
            cf.store(true, Ordering::SeqCst);
        });
    }
    // Supervision loop on the direct child only; std's Child::wait is never used.
    loop {
        let mut status: libc::c_int = 0;
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if r == pid {
            let es = std::process::ExitStatus::from_raw(status);
            o.exit_code = es.code();
            o.term_signal = es.signal();
            o.reaped.push(format!("pid={pid} code={:?} signal={:?}", es.code(), es.signal()));
            break;
        }
        let trigger = if limit_flag.load(Ordering::SeqCst) {
            Some("output_limit")
        } else if cancel.load(Ordering::SeqCst) {
            Some("cancel_requested")
        } else if start.elapsed() >= c.limits.timeout {
            Some("timeout")
        } else {
            None
        };
        if let Some(reason) = trigger {
            o.reason = reason;
            o.method = terminate_group(pid, pid, &c.limits, &mut o);
            break;
        }
        thread::sleep(POLL);
    }
    // Direct child gone (or terminated): any remaining group member is a leftover descendant.
    let leftovers = live(&group_members(pid));
    if !leftovers.is_empty() {
        o.leftover_after_child_exit = describe(&leftovers);
        if o.reason == "none" {
            o.reason = "leftover_descendants";
            o.method = terminate_group(pid, pid, &c.limits, &mut o);
        }
    }
    reap_group(pid, pid, &mut o);
    o.group_live_after_cleanup = describe(&live(&group_members(pid)));
    o.other_ns_processes_after = describe(&others());
    o.process_tree_exited = Some(o.group_live_after_cleanup.is_empty() && o.other_ns_processes_after.is_empty());
    // Bounded join of the pipe readers (a descendant holding a pipe must not hang us).
    for (rx, slot) in [(rx_out, 0), (rx_err, 1)] {
        match rx.recv_timeout(c.limits.cleanup) {
            Ok((res, raw)) => {
                if slot == 1 {
                    o.canary_in_raw_stderr = String::from_utf8_lossy(&raw).contains("PWTFAKE_");
                    o.stderr = res;
                } else {
                    o.stdout = res;
                }
            }
            Err(_) => {
                let unfinished = StreamResult { finished: false, ..Default::default() };
                if slot == 1 { o.stderr = unfinished } else { o.stdout = unfinished }
            }
        }
    }
    o.elapsed_ms = start.elapsed().as_millis();
    o
}

/// Processes in this PID namespace other than the harness itself.
fn others() -> Vec<(i32, char, i32, i32, i32)> {
    let me = unsafe { libc::getpid() };
    proc_table().into_iter().filter(|r| r.0 != me).collect()
}

fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn jl(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| js(s)).collect::<Vec<_>>().join(","))
}

fn jo<T: std::fmt::Debug>(v: Option<T>) -> String {
    v.map(|x| format!("{x:?}")).unwrap_or_else(|| "null".into())
}

fn stream_json(s: &StreamResult) -> String {
    format!(
        "{{\"bytes\":{},\"limit_hit\":{},\"reader_finished\":{},\"excerpt_sanitised\":{}}}",
        s.bytes, s.limit_hit, s.finished, js(&s.excerpt_sanitised)
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: t05-process <fixtures-dir> <results.json>");
        std::process::exit(2);
    }
    let fx = &args[1];
    let me = unsafe { libc::getpid() };
    if me != 1 {
        eprintln!("refusing to run: harness pid is {me}, expected 1 (dedicated PID namespace required)");
        std::process::exit(3);
    }
    let sh = "/bin/sh".to_string();
    let s = |name: &str| format!("{fx}/{name}");
    let lim = |t: u64, g: u64, out: u64, cancel: Option<u64>| Limits {
        timeout: Duration::from_millis(t),
        grace: Duration::from_millis(g),
        cleanup: Duration::from_millis(3000),
        max_output_bytes: out,
        cancel_after: cancel.map(Duration::from_millis),
    };
    const MIB: u64 = 1 << 20;
    let mut cases: Vec<Case> = Vec::new();
    for code in 0..=3 {
        cases.push(Case {
            id: ["FX-PROC-EXIT-CODES/0", "FX-PROC-EXIT-CODES/1", "FX-PROC-EXIT-CODES/2", "FX-PROC-EXIT-CODES/3"][code],
            expect: "normal exit with the given code; no signal sent; tree exited",
            program: sh.clone(),
            args: vec![s("exit_code.sh"), code.to_string()],
            limits: lim(5000, 1000, MIB, None),
            check: |o| {
                if o.term_signal.is_none() && o.signals_sent.is_empty() && o.process_tree_exited == Some(true) && o.exit_code.is_some() { Ok(()) } else { Err("unexpected".into()) }
            },
        });
    }
    // Per-code expectations are checked after the run (code must equal the argument).
    cases.push(Case {
        id: "FX-PROC-EXIT-CODES/signal",
        expect: "fixture kills itself with SIGUSR1: signal termination (10), no exit code, no supervisor signal",
        program: sh.clone(),
        args: vec![s("self_signal.sh")],
        limits: lim(5000, 1000, MIB, None),
        check: |o| if o.term_signal == Some(libc::SIGUSR1) && o.exit_code.is_none() && o.signals_sent.is_empty() && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected SIGUSR1 termination".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-SLEEP/timeout",
        expect: "timeout at 2 s; SIGTERM to group ends sh and sleep; method graceful; tree exited",
        program: sh.clone(),
        args: vec![s("sleep_forever.sh")],
        limits: lim(2000, 1000, MIB, None),
        check: |o| if o.reason == "timeout" && o.method == "graceful" && o.term_signal == Some(libc::SIGTERM) && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected graceful timeout".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-SLEEP/cancel",
        expect: "supervisor cancel at 1 s (timeout 30 s); SIGTERM; method graceful; tree exited",
        program: sh.clone(),
        args: vec![s("sleep_forever.sh")],
        limits: lim(30000, 1000, MIB, Some(1000)),
        check: |o| if o.reason == "cancel_requested" && o.method == "graceful" && o.process_tree_exited == Some(true) && o.elapsed_ms < 5000 { Ok(()) } else { Err("expected bounded graceful cancel".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-TERM-IGNORE",
        expect: "group ignores SIGTERM; after 1 s grace SIGKILL; method forced; child signal 9; tree exited",
        program: sh.clone(),
        args: vec![s("term_ignore.sh")],
        limits: lim(1500, 1000, MIB, None),
        check: |o| if o.reason == "timeout" && o.method == "forced" && o.term_signal == Some(libc::SIGKILL) && o.signals_sent.len() == 2 && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected forced escalation".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-GRANDCHILD",
        expect: "parent exits 0 leaving a grandchild in the group; supervisor detects leftover, terminates and reaps it; tree exited",
        program: sh.clone(),
        args: vec![s("grandchild.sh")],
        limits: lim(5000, 1000, MIB, None),
        check: |o| if o.exit_code == Some(0) && !o.leftover_after_child_exit.is_empty() && o.reason == "leftover_descendants" && o.method != "failed" && o.process_tree_exited == Some(true) && o.reaped.len() >= 2 { Ok(()) } else { Err("expected leftover detection and cleanup".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-FLOOD-OUT",
        expect: "unbounded stdout; output limit 1 MiB triggers termination; tree exited",
        program: sh.clone(),
        args: vec![s("flood.sh"), "out".into()],
        limits: lim(10000, 1000, MIB, None),
        check: |o| if o.reason == "output_limit" && o.stdout.limit_hit && o.method != "failed" && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected stdout limit termination".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-FLOOD-ERR",
        expect: "unbounded stderr; output limit 1 MiB triggers termination; tree exited",
        program: sh.clone(),
        args: vec![s("flood.sh"), "err".into()],
        limits: lim(10000, 1000, MIB, None),
        check: |o| if o.reason == "output_limit" && o.stderr.limit_hit && o.method != "failed" && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected stderr limit termination".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-DUAL-STREAM",
        expect: "512 KiB on stdout and 512 KiB on stderr (each > pipe buffer, < limit); exits 0 without deadlock; all bytes read",
        program: sh.clone(),
        args: vec![s("dual_stream.sh")],
        limits: lim(10000, 1000, MIB, None),
        check: |o| if o.exit_code == Some(0) && o.stdout.bytes == 524288 && o.stderr.bytes == 524288 && o.signals_sent.is_empty() && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected full concurrent read".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-SECRET-STDERR",
        expect: "fixture writes a runtime-generated PWTFAKE_C10 canary to stderr; raw stream contains it; sanitised excerpt does not",
        program: sh.clone(),
        args: vec![s("secret_stderr.sh")],
        limits: lim(5000, 1000, MIB, None),
        check: |o| if o.canary_in_raw_stderr && !o.stderr.excerpt_sanitised.contains("PWTFAKE_") && o.stderr.excerpt_sanitised.contains("[REDACTED-CANARY]") && o.exit_code == Some(0) { Ok(()) } else { Err("expected canary sanitised".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-SPAWN-FAIL/missing",
        expect: "nonexistent executable: spawn error NotFound; no process created",
        program: s("does-not-exist"),
        args: vec![],
        limits: lim(5000, 1000, MIB, None),
        check: |o| if o.spawn_error.as_deref().map_or(false, |e| e.starts_with("NotFound")) && o.pid == 0 && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected NotFound".into()) },
    });
    cases.push(Case {
        id: "FX-PROC-SPAWN-FAIL/not-executable",
        expect: "fixture file without execute permission run directly: spawn error PermissionDenied; no process created",
        program: s("exit_code.sh"),
        args: vec!["0".into()],
        limits: lim(5000, 1000, MIB, None),
        check: |o| if o.spawn_error.as_deref().map_or(false, |e| e.starts_with("PermissionDenied")) && o.pid == 0 && o.process_tree_exited == Some(true) { Ok(()) } else { Err("expected PermissionDenied".into()) },
    });

    let mut json_cases = Vec::new();
    let (mut pass, mut fail) = (0, 0);
    for (i, c) in cases.iter().enumerate() {
        let o = run_case(c);
        let mut verdict = (c.check)(&o);
        if c.id.starts_with("FX-PROC-EXIT-CODES/") && i <= 3 && verdict.is_ok() && o.exit_code != Some(i as i32) {
            verdict = Err(format!("exit code {:?} != {i}", o.exit_code));
        }
        let status = if verdict.is_ok() { pass += 1; "pass" } else { fail += 1; "fail" };
        println!("{status:4} {:32} reason={} method={} code={} signal={} tree_exited={} elapsed_ms={}",
            c.id, o.reason, o.method, jo(o.exit_code), jo(o.term_signal), jo(o.process_tree_exited), o.elapsed_ms);
        json_cases.push(format!(
            concat!("{{\"id\":{},\"expect\":{},\"status\":{},\"detail\":{},",
                "\"limits\":{{\"timeout_ms\":{},\"grace_ms\":{},\"cleanup_ms\":{},\"max_output_bytes\":{},\"cancel_after_ms\":{}}},",
                "\"spawn_error\":{},\"pid\":{},\"pgid_observed\":{},\"harness_pid\":{},\"harness_pgid\":{},",
                "\"exit_code\":{},\"term_signal\":{},\"termination\":{{\"reason\":{},\"method\":{},\"process_tree_exited\":{}}},",
                "\"signals_sent\":{},\"leftover_after_child_exit\":{},\"group_live_after_cleanup\":{},\"other_ns_processes_after\":{},",
                "\"reaped\":{},\"stdout\":{},\"stderr\":{},\"canary_in_raw_stderr\":{},\"elapsed_ms\":{}}}"),
            js(c.id), js(c.expect), js(status), js(&verdict.err().unwrap_or_default()),
            c.limits.timeout.as_millis(), c.limits.grace.as_millis(), c.limits.cleanup.as_millis(), c.limits.max_output_bytes,
            jo(c.limits.cancel_after.map(|d| d.as_millis())),
            o.spawn_error.as_deref().map(js).unwrap_or_else(|| "null".into()), o.pid, o.pgid_observed, o.harness_pid, o.harness_pgid,
            jo(o.exit_code), jo(o.term_signal), js(o.reason), js(o.method), jo(o.process_tree_exited),
            jl(&o.signals_sent), jl(&o.leftover_after_child_exit), jl(&o.group_live_after_cleanup), jl(&o.other_ns_processes_after),
            jl(&o.reaped), stream_json(&o.stdout), stream_json(&o.stderr), o.canary_in_raw_stderr, o.elapsed_ms
        ));
    }
    let doc = format!(
        "{{\"harness_pid\":{},\"harness_pgid\":{},\"harness_sid\":{},\"pass\":{pass},\"fail\":{fail},\"cases\":[\n{}\n]}}\n",
        me, unsafe { libc::getpgrp() }, unsafe { libc::getsid(0) }, json_cases.join(",\n")
    );
    let mut f = fs::File::create(&args[2]).expect("create results file");
    f.write_all(doc.as_bytes()).expect("write results");
    println!("SUMMARY pass={pass} fail={fail}");
    std::process::exit(if fail == 0 { 0 } else { 1 });
}

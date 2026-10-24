//! P00-T07 spike harness: Linux Secret Service behaviour through `keyring` 4.2.0 (v1 store).
//!
//! Spike code, not application code. `orchestrate` must run as PID 1 of a dedicated
//! user+net+mount+PID namespace prepared by `run/setup-and-run.sh` (tmpfs masks over the real
//! bus/keyring/service paths). Each case runs in its own process (`case <ID>`) because the
//! keyring store is initialised once per process and its result is cached.
//!
//! Synthetic secrets are generated in memory from /dev/urandom and passed to case processes
//! and gnome-keyring-daemon only through stdin. They are never written to argv, environment,
//! logs or results; results record booleans only.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const APP_DEADLINE: Duration = Duration::from_secs(10);
const CASE_KILL_AFTER: Duration = Duration::from_secs(30);
const DAEMON_READY: Duration = Duration::from_secs(10);
const OUTPUT_CAP: u64 = 64 * 1024;

/// Paths the setup script must have covered with an empty tmpfs.
const MASKS: &[&str] = &[
    "/run/user",
    "/run/dbus",
    "/home/kali/.local/share/keyrings",
    "/tmp/.X11-unix",
    "/usr/share/dbus-1/services",
    "/etc/dbus-1/session.d",
];

const DBUS_DAEMON: &str = "/usr/bin/dbus-daemon";
const DBUS_SEND: &str = "/usr/bin/dbus-send";
const GKD: &str = "/usr/bin/gnome-keyring-daemon";
const KILL: &str = "/usr/bin/kill";

// ---------------------------------------------------------------- minimal JSON output

#[derive(Clone)]
enum J {
    S(String),
    B(bool),
    N(i64),
    Null,
    A(Vec<J>),
    O(Vec<(String, J)>),
}

impl J {
    fn s(v: impl Into<String>) -> J {
        J::S(v.into())
    }
    fn render(&self, out: &mut String) {
        match self {
            J::S(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        c if (c as u32) < 0x20 || c == '\u{7f}' => {
                            out.push_str(&format!("\\u{:04x}", c as u32))
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            J::B(b) => out.push_str(if *b { "true" } else { "false" }),
            J::N(n) => out.push_str(&n.to_string()),
            J::Null => out.push_str("null"),
            J::A(v) => {
                out.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    x.render(out);
                }
                out.push(']');
            }
            J::O(v) => {
                out.push('{');
                for (i, (k, x)) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    J::S(k.clone()).render(out);
                    out.push(':');
                    x.render(out);
                }
                out.push('}');
            }
        }
    }
    fn to_string(&self) -> String {
        let mut s = String::new();
        self.render(&mut s);
        s
    }
}

fn obj(pairs: Vec<(&str, J)>) -> J {
    J::O(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

// ---------------------------------------------------------------- helpers

fn die(msg: &str) -> ! {
    eprintln!("t07: {msg}");
    std::process::exit(2);
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .unwrap_or_else(|_| die("cannot read /dev/urandom"));
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn mkdir700(p: &Path) {
    fs::create_dir_all(p).unwrap_or_else(|_| die(&format!("mkdir {}", p.display())));
    fs::set_permissions(p, fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|_| die("chmod 0700"));
}

/// Bounded read of a spike-owned file.
fn read_capped(p: &Path) -> Vec<u8> {
    let mut v = Vec::new();
    if let Ok(f) = fs::File::open(p) {
        let _ = f.take(OUTPUT_CAP).read_to_end(&mut v);
    }
    v
}

fn contains(hay: &[u8], needle: &str) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Recursively list regular files under `dir` (spike-owned trees only), skipping sockets.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => files_under(&p, out),
                Ok(t) if t.is_file() => out.push(p),
                _ => {}
            }
        }
    }
}

/// Live processes in this PID namespace other than PID 1, as (pid, comm, state).
fn ns_processes() -> Vec<(u32, String, String)> {
    let mut v = Vec::new();
    if let Ok(rd) = fs::read_dir("/proc") {
        for e in rd.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
            if pid == 1 {
                continue;
            }
            let comm = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let state = stat
                .rsplit_once(") ")
                .and_then(|(_, r)| r.split_whitespace().next())
                .unwrap_or("?")
                .to_string();
            v.push((pid, comm.trim().to_string(), state));
        }
    }
    v
}

// ---------------------------------------------------------------- environment contract

fn case_env(case_dir: &Path, xdg_runtime: bool, bus: Option<&str>) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("PATH".into(), "/usr/bin:/bin".into());
    m.insert("LANG".into(), "C.UTF-8".into());
    m.insert("HOME".into(), case_dir.join("home").display().to_string());
    if xdg_runtime {
        m.insert("XDG_RUNTIME_DIR".into(), case_dir.join("run").display().to_string());
    }
    m.insert("XDG_CONFIG_HOME".into(), case_dir.join("config").display().to_string());
    m.insert("XDG_DATA_HOME".into(), case_dir.join("data").display().to_string());
    m.insert("XDG_CACHE_HOME".into(), case_dir.join("cache").display().to_string());
    m.insert("DBUS_SYSTEM_BUS_ADDRESS".into(), "unix:path=/nonexistent".into());
    if let Some(addr) = bus {
        m.insert("DBUS_SESSION_BUS_ADDRESS".into(), addr.into());
    }
    m
}

// ---------------------------------------------------------------- preflight (orchestrator)

struct Check {
    name: String,
    ok: bool,
    detail: String,
}

fn check(v: &mut Vec<Check>, name: &str, ok: bool, detail: impl Into<String>) {
    v.push(Check { name: name.into(), ok, detail: detail.into() });
}

fn preflight(root: &Path) -> Vec<Check> {
    let mut c = Vec::new();
    check(&mut c, "pid1", std::process::id() == 1, format!("pid={}", std::process::id()));

    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let euid = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(2))
        .unwrap_or("?")
        .to_string();
    check(&mut c, "euid_in_namespace", euid == "0", format!("euid={euid} (so zbus fallback is /run/user/0/bus)"));

    let mountinfo = fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    for m in MASKS {
        // Last mount on the exact mount point wins.
        let line = mountinfo.lines().filter(|l| l.split(' ').nth(4) == Some(m)).last();
        let (is_tmpfs, private) = match line {
            Some(l) => {
                let (pre, post) = l.split_once(" - ").unwrap_or((l, ""));
                (post.starts_with("tmpfs "), !pre.contains("shared:"))
            }
            None => (false, false),
        };
        let empty = fs::read_dir(m).map(|mut d| d.next().is_none()).unwrap_or(false);
        check(
            &mut c,
            &format!("mask {m}"),
            is_tmpfs && private && empty,
            format!("tmpfs={is_tmpfs} private_propagation={private} empty={empty}"),
        );
    }
    let root_line = mountinfo.lines().find(|l| l.split(' ').nth(4) == Some("/"));
    let root_private = root_line.map(|l| !l.split(" - ").next().unwrap_or("").contains("shared:")).unwrap_or(false);
    check(&mut c, "root_mount_private_propagation", root_private, "");

    for p in ["/run/user/0/bus", "/run/user/1000/bus", "/run/dbus/system_bus_socket"] {
        check(&mut c, &format!("absent {p}"), !Path::new(p).exists(), "");
    }

    // Network namespace: only loopback.
    let netdev = fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let ifaces: Vec<String> = netdev
        .lines()
        .skip(2)
        .filter_map(|l| l.split(':').next().map(|s| s.trim().to_string()))
        .collect();
    check(&mut c, "netns_loopback_only", ifaces == ["lo"], format!("{ifaces:?}"));

    // Inherited environment: exactly what `env -i` + setup script provide.
    let mut keys: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    keys.sort();
    let allowed = ["LANG", "PATH"];
    let extra: Vec<&String> = keys.iter().filter(|k| !allowed.contains(&k.as_str())).collect();
    check(&mut c, "inherited_env_minimal", extra.is_empty(), format!("keys={keys:?}"));

    let mode = fs::metadata(root).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
    check(&mut c, "spike_root_0700", mode == 0o700, format!("{mode:o}"));
    check(
        &mut c,
        "spike_root_prefix",
        root.to_string_lossy().starts_with("/home/kali/.local/share/pwt-dev-tools/_spikes/t07."),
        root.display().to_string(),
    );
    for b in [DBUS_DAEMON, DBUS_SEND, GKD, KILL] {
        check(&mut c, &format!("exists {b}"), Path::new(b).is_file(), "");
    }
    c
}

// ---------------------------------------------------------------- daemons

struct Daemon {
    name: &'static str,
    child: Child,
}

fn stop_daemon(d: &mut Daemon) -> J {
    let pid = d.child.id();
    let already = matches!(d.child.try_wait(), Ok(Some(_)));
    let mut method = "already_exited";
    if !already {
        let _ = Command::new(KILL).args(["-TERM", &pid.to_string()]).env_clear().status();
        method = "term";
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(2) {
            if let Ok(Some(_)) = d.child.try_wait() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if !matches!(d.child.try_wait(), Ok(Some(_))) {
            let _ = d.child.kill();
            method = "kill";
        }
    }
    let st = d.child.wait().ok();
    obj(vec![
        ("daemon", J::s(d.name)),
        ("stop_method", J::s(method)),
        ("exit", J::s(format!("{st:?}"))),
    ])
}

fn write_bus_conf(bus_dir: &Path, services: &Path, socket: &Path) -> PathBuf {
    let conf = format!(
        "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\"\n \
         \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
         <busconfig>\n  <type>session</type>\n  <keep_umask/>\n  \
         <listen>unix:path={}</listen>\n  <auth>EXTERNAL</auth>\n  \
         <servicedir>{}</servicedir>\n  <policy context=\"default\">\n    \
         <allow send_destination=\"*\" eavesdrop=\"true\"/>\n    <allow eavesdrop=\"true\"/>\n    \
         <allow own=\"*\"/>\n  </policy>\n</busconfig>\n",
        socket.display(),
        services.display()
    );
    let p = bus_dir.join("bus.conf");
    fs::write(&p, conf).unwrap_or_else(|_| die("write bus.conf"));
    p
}

fn verify_bus_conf(p: &Path, services: &Path) -> (bool, String) {
    let s = fs::read_to_string(p).unwrap_or_default();
    let n_servicedir = s.matches("<servicedir>").count();
    let bad = ["standard_session_servicedirs", "standard_system_servicedirs", "<include", "systemd", "<user>"]
        .iter()
        .filter(|b| s.contains(**b))
        .count();
    let svc_empty = fs::read_dir(services).map(|mut d| d.next().is_none()).unwrap_or(false);
    let ok = n_servicedir == 1 && bad == 0 && svc_empty;
    (ok, format!("servicedirs={n_servicedir} forbidden_elements={bad} servicedir_empty={svc_empty}"))
}

fn dbus_send(addr: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(DBUS_SEND)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg(format!("--bus={addr}"))
        .args(["--print-reply", "--reply-timeout=2000", "--dest=org.freedesktop.DBus", "/org/freedesktop/DBus"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout[..out.stdout.len().min(OUTPUT_CAP as usize)]).into_owned())
}

/// Starts the private bus. Returns (daemon, address, checks).
fn start_bus(root: &Path, logs: &Path, tag: &str) -> Result<(Daemon, String, Vec<J>), String> {
    let bus_dir = root.join("bus").join(tag);
    let services = root.join("bus-services");
    mkdir700(&bus_dir);
    let socket = bus_dir.join("socket");
    let conf = write_bus_conf(&bus_dir, &services, &socket);
    let (conf_ok, conf_detail) = verify_bus_conf(&conf, &services);
    let mut checks = vec![obj(vec![("check", J::s("bus_conf")), ("ok", J::B(conf_ok)), ("detail", J::s(conf_detail))])];
    if !conf_ok {
        return Err("bus.conf verification failed".into());
    }
    let child = Command::new(DBUS_DAEMON)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .args(["--nofork", "--nopidfile", "--nosyslog", "--print-address=1"])
        .arg(format!("--config-file={}", conf.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(fs::File::create(logs.join(format!("dbus-daemon-{tag}.stderr"))).map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| format!("spawn dbus-daemon: {e}"))?;
    let mut d = Daemon { name: "dbus-daemon", child };
    // Read the printed address (one line) with a deadline.
    let mut stdout = d.child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut b = [0u8; 1];
        while buf.len() < 4096 {
            match stdout.read(&mut b) {
                Ok(1) if b[0] == b'\n' => break,
                Ok(1) => buf.push(b[0]),
                _ => break,
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    let printed = match rx.recv_timeout(DAEMON_READY) {
        Ok(s) => s,
        Err(_) => {
            stop_daemon(&mut d);
            return Err("dbus-daemon did not print an address".into());
        }
    };
    let expected = format!("unix:path={}", socket.display());
    let addr_ok = printed.starts_with(&expected);
    checks.push(obj(vec![
        ("check", J::s("bus_printed_address_is_private_socket")),
        ("ok", J::B(addr_ok)),
        ("detail", J::s(format!("prefix_match={addr_ok}"))),
    ]));
    if !addr_ok {
        stop_daemon(&mut d);
        return Err("unexpected bus address".into());
    }
    let activatable = dbus_send(&expected, &["org.freedesktop.DBus.ListActivatableNames"]);
    let act_ok = activatable
        .as_ref()
        .map(|s| s.matches("string \"").count() == 1 && s.contains("string \"org.freedesktop.DBus\""))
        .unwrap_or(false);
    checks.push(obj(vec![
        ("check", J::s("only_org.freedesktop.DBus_activatable")),
        ("ok", J::B(act_ok)),
        ("detail", J::s(activatable.map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_else(|| "dbus-send failed".into()))),
    ]));
    if !act_ok {
        stop_daemon(&mut d);
        return Err("activatable-name check failed".into());
    }
    Ok((d, expected, checks))
}

fn dbus_send_to(addr: &str, dest: &str, path: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(DBUS_SEND)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg(format!("--bus={addr}"))
        .args(["--print-reply", "--reply-timeout=2000"])
        .arg(format!("--dest={dest}"))
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout[..out.stdout.len().min(OUTPUT_CAP as usize)]).into_owned())
}

/// K04b: lock the default collection through the private bus (Secret Service `Lock`).
/// Returns (ok, detail) where detail holds object paths only.
fn lock_default_collection(addr: &str) -> (bool, String) {
    let alias = dbus_send_to(addr, "org.freedesktop.secrets", "/org/freedesktop/secrets",
        &["org.freedesktop.Secret.Service.ReadAlias", "string:default"]);
    let Some(alias) = alias else { return (false, "ReadAlias failed".into()) };
    let Some(path) = alias.split_whitespace().find(|w| w.starts_with("\"/org/freedesktop/secrets/collection/")) else {
        return (false, "no default collection path".into());
    };
    let path = path.trim_matches('"').to_string();
    let lock = dbus_send_to(addr, "org.freedesktop.secrets", "/org/freedesktop/secrets",
        &["org.freedesktop.Secret.Service.Lock", &format!("array:objpath:{path}")]);
    let locked_prop = dbus_send_to(addr, "org.freedesktop.secrets", &path,
        &["org.freedesktop.DBus.Properties.Get", "string:org.freedesktop.Secret.Collection", "string:Locked"]);
    let is_locked = locked_prop.as_deref().map(|s| s.contains("boolean true")).unwrap_or(false);
    (lock.is_some() && is_locked, format!("collection={path} lock_reply={} locked_property_true={is_locked}", lock.is_some()))
}

fn name_has_owner(addr: &str, name: &str) -> bool {
    dbus_send(addr, &["org.freedesktop.DBus.NameHasOwner", &format!("string:{name}")])
        .map(|s| s.contains("boolean true"))
        .unwrap_or(false)
}

fn start_gkd(case_dir: &Path, env: &BTreeMap<String, String>, logs: &Path, unlock: Option<&str>) -> Result<Daemon, String> {
    let mut cmd = Command::new(GKD);
    cmd.env_clear().envs(env).args(["--foreground", "--components=secrets"]);
    cmd.arg(format!("--control-directory={}", case_dir.join("keyring-ctl").display()));
    if unlock.is_some() {
        cmd.arg("--unlock");
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(fs::File::create(logs.join("gnome-keyring-daemon.stdout")).map_err(|e| e.to_string())?)
        .stderr(fs::File::create(logs.join("gnome-keyring-daemon.stderr")).map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| format!("spawn gnome-keyring-daemon: {e}"))?;
    {
        let mut stdin = child.stdin.take().unwrap();
        if let Some(pw) = unlock {
            stdin.write_all(pw.as_bytes()).map_err(|e| e.to_string())?;
        }
        // Dropping closes stdin (EOF ends the password read).
    }
    Ok(Daemon { name: "gnome-keyring-daemon", child })
}

// ---------------------------------------------------------------- K06 listener

fn start_silent_listener(root: &Path) -> Result<String, String> {
    let dir = root.join("bus").join("silent");
    mkdir700(&dir);
    let sock = dir.join("socket");
    let l = UnixListener::bind(&sock).map_err(|e| e.to_string())?;
    thread::spawn(move || {
        let mut held: Vec<UnixStream> = Vec::new();
        for s in l.incoming().flatten() {
            held.push(s); // accept and never reply
        }
    });
    Ok(format!("unix:path={}", sock.display()))
}

// ---------------------------------------------------------------- case process

fn describe(e: &keyring::Error, secrets: &[&str]) -> J {
    let (variant, detail) = match e {
        keyring::Error::PlatformFailure(p) => ("PlatformFailure", Some(p.to_string())),
        keyring::Error::NoStorageAccess(p) => ("NoStorageAccess", Some(p.to_string())),
        keyring::Error::NoEntry => ("NoEntry", None),
        keyring::Error::BadEncoding(_) => ("BadEncoding", None),
        keyring::Error::BadDataFormat(_, p) => ("BadDataFormat", Some(p.to_string())),
        keyring::Error::BadStoreFormat(s) => ("BadStoreFormat", Some(s.clone())),
        keyring::Error::TooLong(s, n) => ("TooLong", Some(format!("{s} {n}"))),
        keyring::Error::Invalid(a, b) => ("Invalid", Some(format!("{a}: {b}"))),
        keyring::Error::Ambiguous(v) => ("Ambiguous", Some(format!("{} entries", v.len()))),
        keyring::Error::NoDefaultStore => ("NoDefaultStore", None),
        keyring::Error::NotSupportedByStore(s) => ("NotSupportedByStore", Some(s.clone())),
        _ => ("Other(non_exhaustive)", None),
    };
    let detail = detail.map(|d| {
        let mut d: String = d.chars().take(300).collect();
        for s in secrets {
            if d.contains(s) {
                d = "[redacted: contained synthetic secret]".into();
            }
        }
        d
    });
    obj(vec![("variant", J::s(variant)), ("detail", detail.map(J::S).unwrap_or(J::Null))])
}

enum Op {
    StoreStatus,
    New,
    Set(usize),
    GetEq(usize),
    Delete,
    GetExpectNoEntry,
}

fn op_name(o: &Op) -> &'static str {
    match o {
        Op::StoreStatus => "store_status",
        Op::New => "entry_new",
        Op::Set(0) => "set_password(C)",
        Op::Set(_) => "set_password(C2)",
        Op::GetEq(0) => "get_password==C",
        Op::GetEq(_) => "get_password==C2",
        Op::Delete => "delete_credential",
        Op::GetExpectNoEntry => "get_password(expect NoEntry)",
    }
}

fn run_case(id: &str, case_dir: &Path, expect_bus: Option<&str>, expect_xdg: bool) {
    // Environment contract, checked before any library call.
    let want = case_env(case_dir, expect_xdg, expect_bus);
    let have: BTreeMap<String, String> = std::env::vars().collect();
    let env_ok = want == have;
    let addr_ok = match expect_bus {
        Some(a) => have.get("DBUS_SESSION_BUS_ADDRESS").map(|v| v == a && v.starts_with("unix:path=")).unwrap_or(false),
        None => !have.contains_key("DBUS_SESSION_BUS_ADDRESS"),
    };
    let fallback_absent = !Path::new("/run/user/0/bus").exists() && !Path::new("/run/user/1000/bus").exists();
    let mut res: Vec<(&str, J)> = vec![
        ("case", J::s(id)),
        ("env_exact", J::B(env_ok)),
        ("bus_address_bound_as_expected", J::B(addr_ok)),
        ("host_fallback_paths_absent", J::B(fallback_absent)),
    ];
    if !(env_ok && addr_ok && fallback_absent) {
        res.push(("harness_failure", J::B(true)));
        println!("{}", obj(res).to_string());
        std::process::exit(3);
    }

    let mut secrets = String::new();
    let _ = std::io::stdin().take(1024).read_to_string(&mut secrets);
    let creds: Vec<String> = secrets.split_whitespace().map(String::from).collect();
    let service = format!("pwt-p00-t07-{}", random_hex(8));

    let ops: Vec<Op> = match id {
        "K03" => vec![Op::StoreStatus, Op::New, Op::Set(0), Op::GetEq(0), Op::Set(1), Op::GetEq(1), Op::Delete, Op::GetExpectNoEntry],
        "K04" | "K04b" => vec![Op::StoreStatus, Op::New, Op::Set(0), Op::GetEq(0), Op::Delete],
        _ => vec![Op::StoreStatus, Op::New, Op::Set(0)],
    };

    let (tx, rx) = mpsc::channel::<(usize, J)>();
    let n_ops = ops.len();
    let creds_w = creds.clone();
    let worker = thread::spawn(move || {
        let secrets: Vec<&str> = creds_w.iter().map(|s| s.as_str()).collect();
        let mut entry: Option<keyring::Entry> = None;
        for (i, op) in ops.iter().enumerate() {
            let t = Instant::now();
            let (ok, err): (bool, Option<J>) = match op {
                Op::StoreStatus => match keyring::Entry::store_status() {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(describe(e, &secrets))),
                },
                Op::New => match keyring::Entry::new(&service, "synthetic") {
                    Ok(e) => {
                        entry = Some(e);
                        (true, None)
                    }
                    Err(e) => (false, Some(describe(&e, &secrets))),
                },
                Op::Set(k) => match (&entry, secrets.get(*k)) {
                    (Some(e), Some(c)) => match e.set_password(c) {
                        Ok(()) => (true, None),
                        Err(e) => (false, Some(describe(&e, &secrets))),
                    },
                    _ => (false, Some(J::s("skipped: no entry or no synthetic value"))),
                },
                Op::GetEq(k) => match (&entry, secrets.get(*k)) {
                    (Some(e), Some(c)) => match e.get_password() {
                        Ok(v) => (v == *c, if v == *c { None } else { Some(J::s("value mismatch")) }),
                        Err(e) => (false, Some(describe(&e, &secrets))),
                    },
                    _ => (false, Some(J::s("skipped: no entry"))),
                },
                Op::Delete => match &entry {
                    Some(e) => match e.delete_credential() {
                        Ok(()) => (true, None),
                        Err(e) => (false, Some(describe(&e, &secrets))),
                    },
                    None => (false, Some(J::s("skipped: no entry"))),
                },
                Op::GetExpectNoEntry => match &entry {
                    Some(e) => match e.get_password() {
                        Err(keyring::Error::NoEntry) => (true, None),
                        Ok(_) => (false, Some(J::s("unexpectedly present"))),
                        Err(e) => (false, Some(describe(&e, &secrets))),
                    },
                    None => (false, Some(J::s("skipped: no entry"))),
                },
            };
            let r = obj(vec![
                ("op", J::s(op_name(op))),
                ("returned", J::B(true)),
                ("ok", J::B(ok)),
                ("error", err.unwrap_or(J::Null)),
                ("elapsed_ms", J::N(t.elapsed().as_millis() as i64)),
            ]);
            if tx.send((i, r)).is_err() {
                return;
            }
            // Stop after a failed connect-stage op; later ops would only repeat the cached failure.
            if !ok && i < 2 {
                return;
            }
        }
    });

    let mut results = Vec::new();
    let mut deadline_fired = false;
    let mut timed_out_op = J::Null;
    for i in 0..n_ops {
        match rx.recv_timeout(APP_DEADLINE) {
            Ok((_, r)) => results.push(r),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                deadline_fired = true;
                timed_out_op = J::N(i as i64);
                break;
            }
        }
    }
    // Is the worker still blocked? Give it 1 s grace after the deadline; never claim cancellation.
    let grace = if deadline_fired { Duration::from_secs(1) } else { Duration::from_secs(2) };
    let t = Instant::now();
    while !worker.is_finished() && t.elapsed() < grace {
        thread::sleep(Duration::from_millis(20));
    }
    let worker_blocked = !worker.is_finished();
    res.push(("ops", J::A(results)));
    res.push(("app_deadline_fired", J::B(deadline_fired)));
    res.push(("timed_out_op_index", timed_out_op));
    res.push(("worker_still_blocked_at_exit", J::B(worker_blocked)));
    res.push((
        "cancellation",
        J::s(if worker_blocked {
            "leaked worker - not cancelled; ended only by case-process exit"
        } else {
            "no blocked worker"
        }),
    ));
    println!("{}", obj(res).to_string());
    let _ = std::io::stdout().flush();
    // Process exit ends any blocked worker thread (containment, not cancellation).
    std::process::exit(0);
}

// ---------------------------------------------------------------- orchestrator

struct CaseSpec {
    id: &'static str,
    bus: BusKind,
    xdg: bool,
    gkd: GkdKind,
}
#[derive(PartialEq)]
enum BusKind {
    None,
    Private,
    Silent,
}
#[derive(PartialEq)]
enum GkdKind {
    None,
    Unlocked,
    NoUnlock,
    UnlockedThenLocked,
}

fn run_one(root: &Path, exe: &Path, spec: &CaseSpec, secrets: &[String]) -> (J, Vec<PathBuf>) {
    let case_dir = root.join("cases").join(spec.id);
    for d in ["home", "run", "config", "data", "cache", "keyring-ctl", "out"] {
        mkdir700(&case_dir.join(d));
    }
    let logs = case_dir.join("out");
    let mut rec: Vec<(&str, J)> = vec![("case", J::s(spec.id))];
    let mut daemons: Vec<Daemon> = Vec::new();
    let mut checks: Vec<J> = Vec::new();
    let mut addr: Option<String> = None;

    let mut harness_err: Option<String> = None;
    match spec.bus {
        BusKind::None => {}
        BusKind::Private => match start_bus(root, &logs, spec.id) {
            Ok((d, a, c)) => {
                daemons.push(d);
                addr = Some(a);
                checks.extend(c);
            }
            Err(e) => harness_err = Some(e),
        },
        BusKind::Silent => match start_silent_listener(root) {
            Ok(a) => addr = Some(a),
            Err(e) => harness_err = Some(format!("silent listener: {e}")),
        },
    }
    let env = case_env(&case_dir, spec.xdg, addr.as_deref());

    if harness_err.is_none() && spec.gkd != GkdKind::None {
        let unlock = if spec.gkd == GkdKind::NoUnlock { None } else { Some(secrets[2].as_str()) };
        match start_gkd(&case_dir, &env, &logs, unlock) {
            Ok(d) => {
                daemons.push(d);
                let a = addr.clone().unwrap();
                let t = Instant::now();
                let mut owned = false;
                while t.elapsed() < DAEMON_READY {
                    if name_has_owner(&a, "org.freedesktop.secrets") {
                        owned = true;
                        break;
                    }
                    if let Ok(Some(_)) = daemons.last_mut().unwrap().child.try_wait() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                checks.push(obj(vec![
                    ("check", J::s("gkd_owns_org.freedesktop.secrets_on_private_bus")),
                    ("ok", J::B(owned)),
                    ("detail", J::s(format!("waited_ms={}", t.elapsed().as_millis()))),
                ]));
                let ctl = case_dir.join("keyring-ctl").join("control");
                checks.push(obj(vec![
                    ("check", J::s("gkd_control_socket_in_private_dir")),
                    ("ok", J::B(ctl.exists())),
                    ("detail", J::Null),
                ]));
                if !owned {
                    harness_err = Some("gnome-keyring-daemon did not own org.freedesktop.secrets".into());
                } else if spec.gkd == GkdKind::UnlockedThenLocked {
                    let (ok, detail) = lock_default_collection(&a);
                    checks.push(obj(vec![
                        ("check", J::s("default_collection_locked_via_private_bus")),
                        ("ok", J::B(ok)),
                        ("detail", J::s(detail)),
                    ]));
                    if !ok {
                        harness_err = Some("could not lock the default collection".into());
                    }
                }
            }
            Err(e) => harness_err = Some(e),
        }
    }
    rec.push(("setup_checks", J::A(checks)));

    if let Some(e) = &harness_err {
        rec.push(("harness_failure", J::s(e.clone())));
    } else {
        let out_f = logs.join("case.stdout");
        let err_f = logs.join("case.stderr");
        let mut cmd = Command::new(exe);
        cmd.env_clear().envs(&env).arg("case").arg(spec.id).arg(&case_dir);
        cmd.arg(addr.clone().unwrap_or_else(|| "-".into()));
        cmd.arg(if spec.xdg { "xdg" } else { "noxdg" });
        let child = cmd
            .stdin(Stdio::piped())
            .stdout(fs::File::create(&out_f).unwrap())
            .stderr(fs::File::create(&err_f).unwrap())
            .spawn();
        match child {
            Err(e) => rec.push(("harness_failure", J::s(format!("spawn case: {e}")))),
            Ok(mut child) => {
                {
                    let mut stdin = child.stdin.take().unwrap();
                    let _ = stdin.write_all(format!("{}\n{}\n", secrets[0], secrets[1]).as_bytes());
                }
                let t = Instant::now();
                let mut killed = false;
                let status = loop {
                    if let Ok(Some(s)) = child.try_wait() {
                        break Some(s);
                    }
                    if t.elapsed() > CASE_KILL_AFTER {
                        let _ = child.kill();
                        killed = true;
                        break child.wait().ok();
                    }
                    thread::sleep(Duration::from_millis(50));
                };
                let out = read_capped(&out_f);
                let parsed = String::from_utf8_lossy(&out).lines().last().unwrap_or("").to_string();
                rec.push(("case_process_killed_by_orchestrator", J::B(killed)));
                rec.push(("case_process_status", J::s(format!("{status:?}"))));
                rec.push(("case_elapsed_ms", J::N(t.elapsed().as_millis() as i64)));
                rec.push(("case_result_raw_json", J::s(parsed)));
            }
        }
    }

    let stops: Vec<J> = daemons.iter_mut().rev().map(stop_daemon).collect();
    rec.push(("daemon_stops", J::A(stops)));
    thread::sleep(Duration::from_millis(200));
    let leftover: Vec<J> = ns_processes()
        .into_iter()
        .filter(|(pid, _, _)| *pid != std::process::id())
        .map(|(p, c, s)| J::s(format!("{p} {c} {s}")))
        .collect();
    rec.push(("processes_left_after_case", J::A(leftover)));
    let mut scan = vec![logs.join("case.stdout"), logs.join("case.stderr")];
    files_under(&case_dir, &mut scan);
    (obj(rec), scan)
}

fn orchestrate(root: &Path, out: &Path) {
    let pf = preflight(root);
    let pf_ok = pf.iter().all(|c| c.ok);
    let pf_json = J::A(
        pf.iter()
            .map(|c| obj(vec![("check", J::s(c.name.clone())), ("ok", J::B(c.ok)), ("detail", J::s(c.detail.clone()))]))
            .collect(),
    );
    let mut top: Vec<(&str, J)> = vec![("preflight_ok", J::B(pf_ok)), ("preflight", pf_json)];
    if !pf_ok {
        top.push(("cases", J::s("not run: preflight failed")));
        fs::write(out.join("results.json"), obj(top).to_string()).unwrap();
        eprintln!("t07: preflight failed; no case run");
        std::process::exit(1);
    }
    if std::env::args().nth(4).as_deref() == Some("preflight-only") {
        top.push(("cases", J::s("not run: preflight-only mode")));
        fs::write(out.join("results.json"), obj(top).to_string()).unwrap();
        return;
    }
    mkdir700(&root.join("bus"));
    mkdir700(&root.join("bus-services"));
    mkdir700(&root.join("cases"));

    // C, C2 (credentials), U (unlock password): 32 random bytes each, hex, memory only.
    let secrets = vec![random_hex(32), random_hex(32), random_hex(32)];
    let exe = std::env::current_exe().unwrap_or_else(|_| die("current_exe"));
    let specs = [
        CaseSpec { id: "K01a", bus: BusKind::None, xdg: true, gkd: GkdKind::None },
        CaseSpec { id: "K01b", bus: BusKind::None, xdg: false, gkd: GkdKind::None },
        CaseSpec { id: "K02", bus: BusKind::Private, xdg: true, gkd: GkdKind::None },
        CaseSpec { id: "K03", bus: BusKind::Private, xdg: true, gkd: GkdKind::Unlocked },
        CaseSpec { id: "K04", bus: BusKind::Private, xdg: true, gkd: GkdKind::NoUnlock },
        CaseSpec { id: "K04b", bus: BusKind::Private, xdg: true, gkd: GkdKind::UnlockedThenLocked },
        CaseSpec { id: "K06", bus: BusKind::Silent, xdg: true, gkd: GkdKind::None },
    ];
    let mut cases = Vec::new();
    let mut scan_files = Vec::new();
    for s in &specs {
        let (r, f) = run_one(root, &exe, s, &secrets);
        cases.push(r);
        scan_files.extend(f);
    }

    // K05: synthetic values must not appear in app-owned or harness-captured files.
    // Private service storage (cases/*/data/keyrings) is reported separately.
    let mut app_hits = Vec::new();
    let mut svc_hits = Vec::new();
    let mut svc_files = 0;
    scan_files.sort();
    scan_files.dedup();
    for f in &scan_files {
        let svc = f.components().any(|c| c.as_os_str() == "keyrings");
        if svc {
            svc_files += 1;
        }
        let data = read_capped(f);
        for (label, s) in [("C", &secrets[0]), ("C2", &secrets[1]), ("U", &secrets[2])] {
            if contains(&data, s) {
                let rel = f.strip_prefix(root).unwrap_or(f).display().to_string();
                if svc {
                    svc_hits.push(J::s(format!("{label} in {rel}")));
                } else {
                    app_hits.push(J::s(format!("{label} in {rel}")));
                }
            }
        }
    }
    let k05 = obj(vec![
        ("case", J::s("K05")),
        ("files_scanned", J::N(scan_files.len() as i64)),
        ("app_owned_or_captured_hits", J::A(app_hits.clone())),
        ("service_storage_files", J::N(svc_files)),
        ("service_storage_plaintext_hits", J::A(svc_hits)),
        ("scan_cap_bytes_per_file", J::N(OUTPUT_CAP as i64)),
        ("pass", J::B(app_hits.is_empty())),
    ]);
    top.push(("cases", J::A(cases)));
    top.push(("k05", k05));
    let left: Vec<J> = ns_processes()
        .into_iter()
        .map(|(p, c, s)| J::s(format!("{p} {c} {s}")))
        .collect();
    top.push(("processes_left_at_end", J::A(left)));
    let text = obj(top).to_string();
    // Final guard: results never contain a synthetic value.
    let guard = secrets.iter().any(|s| text.contains(s.as_str()));
    let text = if guard { "{\"error\":\"results suppressed: contained a synthetic value\"}".to_string() } else { text };
    fs::write(out.join("results.json"), &text).unwrap_or_else(|_| die("write results"));
    println!("t07: done; results guard_triggered={guard}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("orchestrate") if args.len() >= 4 => {
            if std::process::id() != 1 {
                die("orchestrate must run as PID 1 of a dedicated PID namespace");
            }
            orchestrate(Path::new(&args[2]), Path::new(&args[3]));
        }
        Some("case") if args.len() == 6 => {
            let bus = if args[4] == "-" { None } else { Some(args[4].as_str()) };
            run_case(&args[2], Path::new(&args[3]), bus, args[5] == "xdg");
        }
        _ => die("usage: t07-keyring orchestrate <spike-root> <out-dir> [preflight-only] | (internal) case ..."),
    }
}

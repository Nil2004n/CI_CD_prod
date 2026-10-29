//! P02-T14 end-to-end failure scenarios for the headless CLI (`planning/P02-T01-request.md` §5.9, §6.3-§6.5,
//! §9 rows T07/T10/T12; closes L-T12-3 and, together with `crates/application/tests/dev_store_faults.rs`,
//! L-T12-2).
//!
//! Every scenario runs the built binary (`pwt-workbench-cli`) in a cleared environment against a private
//! (mode 0700) data directory under `std::env::temp_dir()`, removed at the end. Expected values (exit codes,
//! root names, problem codes, counts) are written out literally, not derived from the code under test; each
//! test says what would make it fail. All store content is synthetic and every marker string is recognisably
//! fake.
//!
//! The nine roots are damaged from the `*.min.json` fixtures of `crates/persistence/tests/fixtures/store`
//! (read-only use), so "valid before, invalid after" is shown for each root rather than assumed.
//!
//! Limits of this file: the `cli` crate has no dependency on `persistence`, so the same-second report scenario
//! here builds the reports and the log by hand; the version made by the real `StoreWriter` is in
//! `crates/application/tests/dev_store_faults.rs`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_pwt-workbench-cli");
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../persistence/tests/fixtures/store"
);

/// The nine roots: command-line name, file relative to the data directory, valid minimal fixture.
const ROOTS: [(&str, &str, &str); 9] = [
    ("config", "config.json", "config.min.json"),
    ("repositories", "repo.json", "repositories.min.json"),
    ("workflows", "workflows.json", "workflows.min.json"),
    ("approvals", "approvals.json", "approvals.min.json"),
    ("log", "log.json", "log.min.json"),
    ("suppressions", "suppressions.json", "suppressions.min.json"),
    ("rag_index", "rag/index.json", "rag_index.min.json"),
    (
        "conversations",
        "rag/conversations.json",
        "conversations.min.json",
    ),
    ("tool_manifest", "tools.json", "tool_manifest.min.json"),
];

/// The read bound of a store file (request §6.5): 64 MiB.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// A marker that must never reach any output stream.
const MARKER: &str = "T14-VALUE-MARKER-9f3a";

// ---------------------------------------------------------------------------------------------------
// helpers

/// A private temporary data directory, removed on drop.
struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("pwt-cli-p02f-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        make_private_dir(&root);
        Self { root }
    }

    fn dir(&self) -> &str {
        self.root.to_str().unwrap()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Writes `rel` (mode 0600).
    fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.path(rel);
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    /// Creates the private sub-directory `rel`.
    fn private_dir(&self, rel: &str) {
        make_private_dir(&self.path(rel));
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        // A test that made the directory read-only must not leave it undeletable.
        let _ = std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Creates `path` with mode exactly 0700 (the umask can only remove bits, so the mode is set again).
fn make_private_dir(path: &Path) {
    std::fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

/// The result of one process run.
struct Run {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    elapsed: Duration,
}

impl Run {
    fn out(&self) -> String {
        String::from_utf8(self.stdout.clone()).expect("stdout is UTF-8")
    }

    fn err(&self) -> String {
        String::from_utf8(self.stderr.clone()).expect("stderr is UTF-8")
    }

    /// Every stdout line parsed as JSON; panics if one does not parse.
    fn jsonl(&self) -> Vec<Value> {
        self.out()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("every stdout line is JSON"))
            .collect()
    }
}

/// Runs the binary with a cleared environment.
fn bin_os(args: &[OsString]) -> Run {
    let start = Instant::now();
    let output = Command::new(BIN)
        .args(args)
        .env_clear()
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("exit code, not a signal"),
        stdout: output.stdout,
        stderr: output.stderr,
        elapsed: start.elapsed(),
    }
}

fn bin(args: &[&str]) -> Run {
    let owned: Vec<OsString> = args.iter().map(OsString::from).collect();
    bin_os(&owned)
}

/// `store <sub> --data-dir <dir> <extra...>` for an arbitrary directory path.
fn store_at(dir: &Path, sub: &str, extra: &[&str]) -> Run {
    let mut args: Vec<OsString> = vec!["store".into(), sub.into(), "--data-dir".into()];
    args.push(dir.as_os_str().to_owned());
    args.extend(extra.iter().map(OsString::from));
    bin_os(&args)
}

fn store(tmp: &Tmp, sub: &str, extra: &[&str]) -> Run {
    store_at(&tmp.root, sub, extra)
}

fn init(tmp: &Tmp) {
    let run = store(tmp, "init", &[]);
    assert_eq!(run.code, 0, "init failed: {}", run.err());
}

/// `init`, plus the private `rag/` directory the two rag roots live in.
fn prepare(tmp: &Tmp) {
    init(tmp);
    tmp.private_dir("rag");
}

/// The record of `dev_event` `event` among `records`; panics unless there is exactly one.
fn one<'a>(records: &'a [Value], event: &str) -> &'a Value {
    let found: Vec<&Value> = records.iter().filter(|r| r["dev_event"] == event).collect();
    assert_eq!(found.len(), 1, "expected exactly one {event} record");
    found[0]
}

fn validated<'a>(records: &'a [Value], kind: &str) -> &'a Value {
    records
        .iter()
        .find(|r| r["dev_event"] == "StoreValidated" && r["kind"] == kind)
        .unwrap_or_else(|| panic!("no StoreValidated record for {kind}"))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|v| v.as_str().expect("a string").to_owned())
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// One entry of a directory tree, without following symlinks.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Node {
    File(Vec<u8>),
    Dir,
    Link(PathBuf),
}

/// Relative path -> entry for everything under `root`, except the top-level lock file (its diagnostic line
/// changes with every locking command).
fn tree(root: &Path) -> BTreeMap<String, Node> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Node>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel == ".app.lock" {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                out.insert(rel, Node::Link(std::fs::read_link(&path).unwrap()));
            } else if meta.is_dir() {
                out.insert(rel.clone(), Node::Dir);
                walk(base, &path, out);
            } else {
                out.insert(rel, Node::File(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Fails if either stream of `run` contains a raw control character (other than the line feed that ends
/// lines), a bidirectional control, a zero-width or separator code point, or is not UTF-8.
fn assert_no_raw_controls(run: &Run, context: &str) {
    for (stream, bytes) in [("stdout", &run.stdout), ("stderr", &run.stderr)] {
        let text = std::str::from_utf8(bytes)
            .unwrap_or_else(|_| panic!("{context}: {stream} is not UTF-8"));
        for ch in text.chars() {
            let bad = (ch.is_control() && ch != '\n')
                || matches!(
                    ch,
                    '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                        | '\u{feff}'
                );
            assert!(!bad, "{context}: raw U+{:04X} in {stream}", u32::from(ch));
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// 4: malformed JSON in every root

/// The ways a root file is damaged.
#[derive(Clone, Copy, Debug)]
enum Damage {
    Empty,
    Truncated,
    WrongStoreType,
    Bom,
    InvalidUtf8,
    DeepNesting,
    UnterminatedNesting,
    UnknownField,
}

const DAMAGES: [Damage; 8] = [
    Damage::Empty,
    Damage::Truncated,
    Damage::WrongStoreType,
    Damage::Bom,
    Damage::InvalidUtf8,
    Damage::DeepNesting,
    Damage::UnterminatedNesting,
    Damage::UnknownField,
];

/// `base` with the member `key` set to the marker.
fn with_marker(base: &Value, key: &str) -> Value {
    let mut doc = base.clone();
    doc[key] = json!(MARKER);
    doc
}

/// The damaged file content, and whether it carries the marker (so that the no-echo check can fail).
fn damaged(damage: Damage, root: &str, base: &Value) -> (Vec<u8>, bool) {
    let marked = |bytes: Vec<u8>| (bytes, true);
    match damage {
        Damage::Empty => (Vec::new(), false),
        Damage::Truncated => {
            let full = serde_json::to_vec_pretty(&with_marker(base, "aaa_marker")).unwrap();
            marked(full[..full.len() * 3 / 4].to_vec())
        }
        Damage::WrongStoreType => {
            let mut doc = with_marker(base, "aaa_marker");
            doc["store_type"] = json!(if root == "log" { "repositories" } else { "log" });
            marked(serde_json::to_vec(&doc).unwrap())
        }
        Damage::Bom => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend(serde_json::to_vec(&with_marker(base, "aaa_marker")).unwrap());
            marked(bytes)
        }
        Damage::InvalidUtf8 => {
            let mut bytes = format!("{{\"aaa_marker\":\"{MARKER}\",\"store_type\":\"").into_bytes();
            bytes.extend([0xff, 0xfe, 0xc3]);
            bytes.extend(b"\"}");
            marked(bytes)
        }
        Damage::DeepNesting => {
            let mut bytes = format!("{{\"aaa_marker\":\"{MARKER}\",\"x\":").into_bytes();
            bytes.extend(std::iter::repeat_n(b'[', 5000));
            bytes.extend(std::iter::repeat_n(b']', 5000));
            bytes.push(b'}');
            marked(bytes)
        }
        Damage::UnterminatedNesting => (vec![b'['; 100_000], false),
        Damage::UnknownField => {
            marked(serde_json::to_vec(&with_marker(base, "aaa_unknown_field")).unwrap())
        }
    }
}

/// The problem list `validate` and `show` must report for `damage`; `None` for the unknown field, whose
/// list is checked by content instead.
fn expected_problems(damage: Damage) -> Option<Value> {
    match damage {
        Damage::WrongStoreType => Some(json!([{
            "layer": "version",
            "code": "store_type_mismatch",
            "pointer": "/store_type",
            "rule": null
        }])),
        Damage::UnknownField => None,
        _ => Some(json!([{
            "layer": "json",
            "code": "syntax",
            "pointer": "",
            "rule": null
        }])),
    }
}

/// Fails if, for any of the nine roots and any of eight damages (empty file, truncated JSON, valid JSON of
/// the wrong `store_type`, a UTF-8 BOM, invalid UTF-8, 5000-deep nesting, an unterminated 100000-deep
/// array, an unknown member), `validate --root` and `show` (JSON and human) do not exit 0 with verdict
/// `invalid` and the literal problem list (layer `json`/`syntax`, `version`/`store_type_mismatch` at
/// `/store_type`, or a `schema`/`keyword_additional_properties` problem), if `show` prints a document, if
/// the marker value, the unknown member's name or the BOM reaches either stream, if the damaged file is
/// touched, or if the undamaged fixture of the same root is not valid (which would make "invalid" mean
/// nothing). A panic or a stack overflow on deep nesting shows as a signal exit and fails the exit-code
/// expectation.
#[test]
fn malformed_json_in_every_root_is_invalid_not_a_failure() {
    for (name, rel, fixture_name) in ROOTS {
        let base = fixture(fixture_name);
        let tmp = Tmp::new("malformed");
        prepare(&tmp);

        // Control: the undamaged fixture is valid, so the verdicts below are caused by the damage.
        tmp.write(rel, &serde_json::to_vec_pretty(&base).unwrap());
        let control = store(&tmp, "validate", &["--root", name, "--json"]);
        assert_eq!(control.code, 0, "{name}: {}", control.err());
        assert_eq!(
            validated(&control.jsonl(), name)["verdict"],
            "valid",
            "{name}: the control fixture must be valid"
        );

        for damage in DAMAGES {
            let context = format!("{name} {damage:?}");
            let (bytes, carries_marker) = damaged(damage, name, &base);
            if carries_marker {
                assert!(
                    contains(&bytes, MARKER.as_bytes()),
                    "{context}: the damaged file lost its marker, so the no-echo check proves nothing"
                );
            }
            tmp.write(rel, &bytes);

            let validate = store(&tmp, "validate", &["--root", name, "--json"]);
            assert_eq!(validate.code, 0, "{context}: {}", validate.err());
            let records = validate.jsonl();
            assert_eq!(records.len(), 1, "{context}");
            let rec = validated(&records, name);
            assert_eq!(rec["verdict"], "invalid", "{context}");
            assert_eq!(rec["mode"], "dev");

            let show_json = store(&tmp, "show", &[name, "--json"]);
            let show_human = store(&tmp, "show", &[name]);
            assert_eq!(show_json.code, 0, "{context}: {}", show_json.err());
            assert_eq!(show_human.code, 0, "{context}: {}", show_human.err());
            let shown_records = show_json.jsonl();
            let shown = one(&shown_records, "StoreShown");
            assert_eq!(shown["verdict"], "invalid", "{context}");
            assert_eq!(shown["document"], Value::Null, "{context}");
            assert_eq!(shown["problems"], rec["problems"], "{context}");
            assert!(
                show_human.out().starts_with(&format!("{name}: invalid")),
                "{context}: {}",
                show_human.out()
            );

            match expected_problems(damage) {
                Some(expected) => assert_eq!(rec["problems"], expected, "{context}"),
                None => {
                    let problems = rec["problems"].as_array().expect("problem list");
                    assert!(
                        problems.iter().any(|p| p["layer"] == "schema"
                            && p["code"] == "keyword_additional_properties"),
                        "{context}: {problems:?}"
                    );
                }
            }

            for run in [&validate, &show_json, &show_human] {
                for stream in [&run.stdout, &run.stderr] {
                    for needle in [
                        MARKER.as_bytes(),
                        b"aaa_marker",
                        b"aaa_unknown_field",
                        &[0xEF, 0xBB, 0xBF],
                        &[0xff, 0xfe],
                    ] {
                        assert!(
                            !contains(stream, needle),
                            "{context}: file content reached the output"
                        );
                    }
                }
                assert_no_raw_controls(run, &context);
            }
            assert_eq!(
                tmp.read(rel),
                bytes,
                "{context}: a read-only command rewrote the file"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// 5: too large

/// Creates a sparse file of exactly `len` bytes (no data is written).
fn sparse_file(path: &Path, len: u64) {
    let file = std::fs::File::create(path).unwrap();
    file.set_len(len).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

/// Fails if, for each of the nine roots, a file one byte over the 64 MiB bound is not reported as `invalid`
/// with exactly the `json`/`too_large` problem by `validate` and `show` (exit 0, no document, fast), if
/// `recover` does not list it as malformed (every root but the reserved, unmanaged `suppressions`) with exit 0 and leave it untouched, or if a file of exactly 64 MiB
/// is also called too large (the boundary is `>`: it is read and then fails to parse, `json`/`syntax`). The
/// 5 s limit per command is generous; reading the sparse file instead of refusing it by size would still pass
/// it, so the limit only catches a pathological regression.
#[test]
fn oversized_root_is_reported_too_large_without_reading_it() {
    let too_large = json!([{
        "layer": "json",
        "code": "too_large",
        "pointer": "",
        "rule": null
    }]);
    for (name, rel, _) in ROOTS {
        let tmp = Tmp::new("oversized");
        prepare(&tmp);
        sparse_file(&tmp.path(rel), MAX_BYTES + 1);

        let validate = store(&tmp, "validate", &["--root", name, "--json"]);
        assert_eq!(validate.code, 0, "{name}: {}", validate.err());
        let rec_all = validate.jsonl();
        let rec = validated(&rec_all, name);
        assert_eq!(rec["verdict"], "invalid", "{name}");
        assert_eq!(rec["problems"], too_large, "{name}");
        let show = store(&tmp, "show", &[name, "--json"]);
        assert_eq!(show.code, 0, "{name}: {}", show.err());
        let shown_records = show.jsonl();
        let shown = one(&shown_records, "StoreShown");
        assert_eq!(shown["verdict"], "invalid", "{name}");
        assert_eq!(shown["document"], Value::Null);
        assert_eq!(shown["problems"], too_large, "{name}");
        for run in [&validate, &show] {
            assert!(
                run.elapsed < Duration::from_secs(5),
                "{name}: took {:?}",
                run.elapsed
            );
        }

        let recover = store(&tmp, "recover", &["--json"]);
        assert_eq!(recover.code, 0, "{name}: {}", recover.err());
        let records = recover.jsonl();
        let reconciled = one(&records, "StoreReconciled");
        // `suppressions.json` is reserved and never written, so it is not a managed root: startup
        // reconciliation neither reads nor flags it (writer.rs MANAGED_ROOTS). Every other root is flagged.
        let flagged: Vec<&str> = if name == "suppressions" {
            vec![]
        } else {
            vec![name]
        };
        assert_eq!(
            strings(&reconciled["malformed_roots"]),
            flagged,
            "{name}: recover must list the oversized root as malformed"
        );
        assert_eq!(
            std::fs::metadata(tmp.path(rel)).unwrap().len(),
            MAX_BYTES + 1,
            "{name}: recover changed the oversized file"
        );
    }

    // The boundary itself: exactly 64 MiB is within the bound.
    let tmp = Tmp::new("oversized-bound");
    prepare(&tmp);
    sparse_file(&tmp.path("repo.json"), MAX_BYTES);
    let run = store(&tmp, "validate", &["--root", "repositories", "--json"]);
    assert_eq!(run.code, 0, "{}", run.err());
    let records = run.jsonl();
    assert_eq!(
        validated(&records, "repositories")["problems"],
        json!([{ "layer": "json", "code": "syntax", "pointer": "", "rule": null }]),
        "a file of exactly the bound must be read, not refused by size"
    );
}

// ---------------------------------------------------------------------------------------------------
// 6: stale lock

/// Fails if an existing `.app.lock` that nobody holds (left by a dead process, with a text or binary
/// diagnostic line) blocks any of `init`, `validate`, `show`, `recover` (each must exit 0), if its content
/// (a pid, a time, a marker, a raw ESC, 0xff bytes) reaches either stream, or if a later command cannot
/// take the lock again (a lock that is left held would exit 3).
#[test]
fn stale_lock_file_does_not_block_and_is_not_echoed() {
    let contents: [&[u8]; 2] = [
        b"pid=424242 utc=2001-01-01T00:00:00Z STALE-LOCK-MARKER\n",
        b"\x1b[31mSTALE-LOCK-MARKER\xff\xfe\x00 pid=424242",
    ];
    for (i, content) in contents.iter().enumerate() {
        let tmp = Tmp::new("stale");
        tmp.write(".app.lock", content);
        let commands: [(&str, &[&str]); 5] = [
            ("init", &[]),
            ("validate", &[]),
            ("show", &["log"]),
            ("recover", &[]),
            ("validate", &["--json"]),
        ];
        for (sub, extra) in commands {
            let run = store(&tmp, sub, extra);
            assert_eq!(run.code, 0, "content {i}, {sub}: {}", run.err());
            for stream in [&run.stdout, &run.stderr] {
                assert!(
                    !contains(stream, b"STALE-LOCK-MARKER") && !contains(stream, b"424242"),
                    "content {i}, {sub}: the lock file content was echoed"
                );
            }
            assert_no_raw_controls(&run, &format!("content {i}, {sub}"));
        }
        assert_eq!(
            tmp.names_without_lock(),
            [
                "approvals.json",
                "log.json",
                "repo.json",
                "tools.json",
                "workflows.json"
            ],
            "content {i}: init on a stale lock created the wrong files"
        );
    }
}

impl Tmp {
    /// The file names directly in the data directory, sorted, without the lock file.
    fn names_without_lock(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != ".app.lock")
            .collect();
        names.sort();
        names
    }
}

/// Fails if a lock file that is a symlink (to a file outside the store) or has a second hard link is
/// accepted for any command, if the failure is not exit 5 with the fixed rejection text and no path, or if
/// the outside file is written to (the diagnostic line must never be written through a link).
#[test]
fn lock_file_that_is_a_symlink_or_hard_linked_is_rejected_with_exit_5() {
    for variant in ["symlink", "hard link"] {
        let tmp = Tmp::new("lockfile");
        let outside = Tmp::new("lockfile-outside");
        outside.write("target.txt", b"OUTSIDE-CONTENT");
        let lock = tmp.path(".app.lock");
        if variant == "symlink" {
            symlink(outside.path("target.txt"), &lock).unwrap();
        } else {
            std::fs::hard_link(outside.path("target.txt"), &lock).unwrap();
        }
        for (sub, extra) in [
            ("init", &[][..]),
            ("validate", &[]),
            ("show", &["log"]),
            ("recover", &[]),
        ] {
            let run = store(&tmp, sub, extra);
            assert_eq!(run.code, 5, "{variant}, {sub}: {}", run.err());
            assert!(run.stdout.is_empty(), "{variant}, {sub}");
            let err = run.err();
            assert!(err.contains("data directory was rejected"), "{err}");
            assert!(!err.contains(tmp.dir()), "{variant}: path echoed");
        }
        assert_eq!(
            outside.read("target.txt"),
            b"OUTSIDE-CONTENT",
            "{variant}: the lock diagnostic was written through the link"
        );
        assert_eq!(tmp.names_without_lock(), Vec::<String>::new());
    }
}

// ---------------------------------------------------------------------------------------------------
// 7: lock held by a second process

const HOLDER_LOCK_ENV: &str = "PWT_T14_LOCK_HOLDER_FILE";
const HOLDER_READY_ENV: &str = "PWT_T14_LOCK_READY_FILE";

/// Not a scenario: the second process of
/// [`lock_held_by_a_second_process_exits_3_then_succeeds_after_release`]. When the environment names a lock
/// file it takes the exclusive lock, creates the ready file and blocks until its standard input is closed;
/// without the variable it returns at once (so a normal run of the suite counts it as passed and does
/// nothing).
#[test]
fn second_process_lock_holder_helper() {
    let Some(lock_path) = std::env::var_os(HOLDER_LOCK_ENV) else {
        return;
    };
    let ready = std::env::var_os(HOLDER_READY_ENV).expect("the parent names a ready file");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    file.try_lock().expect("the helper takes the lock first");
    std::fs::write(ready, b"held").unwrap();
    // Blocks until the parent closes the pipe (or dies, which closes it too).
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    drop(file);
}

/// The second process, killed on drop if the test fails before releasing it.
struct Holder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Holder {
    /// Starts the helper and waits (at most 20 s) until it holds the lock of `lock_file`.
    fn start(lock_file: &Path, ready: &Path) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "second_process_lock_holder_helper",
                "--test-threads=1",
            ])
            .env_clear()
            .env(HOLDER_LOCK_ENV, lock_file)
            .env(HOLDER_READY_ENV, ready)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the test binary re-executes");
        let stdin = child.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready.exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "the lock holder exited before it held the lock"
            );
            assert!(
                Instant::now() < deadline,
                "the lock holder never reported ready"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        Self { child, stdin }
    }

    /// Closes the helper's input and waits (at most 20 s) for it to exit successfully, which releases the
    /// lock.
    fn release(&mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "the lock holder failed: {status:?}");
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the lock holder did not exit after release"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Fails if, while a real second process holds the data-directory lock, any of `init`, `validate`, `show`,
/// `recover` and `recover --restore-previous` (human and JSON) exits other than 3, prints to stdout, omits
/// the fixed "locked by another instance" text, echoes the data-directory path, or changes the store; or if
/// the same commands do not succeed once the second process has exited (which would mean the lock was not
/// the cause). The helper proves it holds the lock by creating the ready file after `try_lock` succeeded,
/// and the wait is bounded, so a helper that never starts fails the test instead of hanging it.
#[test]
fn lock_held_by_a_second_process_exits_3_then_succeeds_after_release() {
    let tmp = Tmp::new("lock2");
    let aux = Tmp::new("lock2-aux");
    init(&tmp);
    let before = tree(&tmp.root);
    let mut holder = Holder::start(&tmp.path(".app.lock"), &aux.path("ready"));
    let commands: [(&str, &[&str]); 7] = [
        ("init", &[]),
        ("validate", &[]),
        ("show", &["log"]),
        ("recover", &[]),
        ("recover", &["--restore-previous", "log"]),
        ("validate", &["--json"]),
        ("init", &["--json"]),
    ];
    for (sub, extra) in commands {
        let run = store(&tmp, sub, extra);
        assert_eq!(run.code, 3, "store {sub} {extra:?}: {}", run.err());
        assert!(run.stdout.is_empty(), "store {sub} printed to stdout");
        let err = run.err();
        assert!(err.contains("locked by another instance"), "{err}");
        assert!(!err.contains(tmp.dir()), "the path was echoed: {err}");
    }
    assert_eq!(
        tree(&tmp.root),
        before,
        "a refused command changed the store"
    );
    holder.release();
    for (sub, extra) in commands {
        let run = store(&tmp, sub, extra);
        // `--restore-previous log` on a fresh store has no previous generation: exit 5, not 3.
        let expected = if extra.contains(&"--restore-previous") {
            5
        } else {
            0
        };
        assert_eq!(
            run.code,
            expected,
            "after release, store {sub} {extra:?}: {}",
            run.err()
        );
    }
}

// ---------------------------------------------------------------------------------------------------
// 8: leftover temp files

/// A report stem the application would generate.
const STEM: &str = "20261016T052500Z_demo-repo_00000000-0000-4000-8000-000000000014";

/// Fails if `recover` does not remove exactly the six leftover temp files of the application's pattern (two
/// in the data directory, two in `rag/`, two in `backup/`) and report that count; if any look-alike (upper-case
/// hex, 15 hex digits, a suffix, an unknown base name, another directory's base name, no leading dot), foreign
/// file, directory with a temp name, symlink with a temp name (valid target and dangling) is removed,
/// changed or followed (the file a symlink points at must survive unchanged); if `validate` or `show`
/// removes anything (they are read-only); or if a second `recover` finds anything to remove.
#[test]
fn leftover_temp_files_are_removed_and_look_alikes_survive() {
    let tmp = Tmp::new("temps");
    let outside = Tmp::new("temps-outside");
    prepare(&tmp);
    tmp.private_dir("backup");
    outside.write("target.txt", b"OUTSIDE-CONTENT");
    let target = outside.path("target.txt");

    let removed = [
        ".repo.json.tmp-0123456789abcdef".to_owned(),
        ".log.json.prev.tmp-fedcba9876543210".to_owned(),
        "rag/.index.json.tmp-0123456789abcdef".to_owned(),
        "rag/.conversations.json.prev.tmp-aaaaaaaaaaaaaaaa".to_owned(),
        format!("backup/.{STEM}.json.tmp-bbbbbbbbbbbbbbbb"),
        format!("backup/.{STEM}.md.prev.tmp-cccccccccccccccc"),
    ];
    let kept_files = [
        ".repo.json.tmp-0123456789abcdeF",
        ".repo.json.tmp-0123456789abcde",
        ".repo.json.tmp-0123456789abcdef.bak",
        ".unknown.json.tmp-0123456789abcdef",
        ".index.json.tmp-0123456789abcdef",
        "repo.json.tmp-0123456789abcdef",
        "notes.txt",
        ".hidden",
        "rag/.repo.json.tmp-0123456789abcdef",
        "rag/other.json",
        "backup/.notes.json.tmp-0123456789abcdef",
        "backup/.20261016T052500Z_demo-repo_not-a-uuid.json.tmp-0123456789abcdef",
    ];
    for name in removed.iter().map(String::as_str).chain(kept_files) {
        tmp.write(name, name.as_bytes());
    }
    let links = [
        ".workflows.json.tmp-1111111111111111",
        "rag/.conversations.json.tmp-2222222222222222",
    ];
    for link in links {
        symlink(&target, tmp.path(link)).unwrap();
    }
    let backup_link = format!("backup/.{STEM}.pdf.tmp-3333333333333333");
    symlink(&target, tmp.path(&backup_link)).unwrap();
    symlink(
        outside.path("does-not-exist"),
        tmp.path(".approvals.json.tmp-4444444444444444"),
    )
    .unwrap();
    tmp.private_dir(".tools.json.tmp-5555555555555555");
    tmp.write(".tools.json.tmp-5555555555555555/inner.txt", b"inner");

    let before = tree(&tmp.root);
    for name in &removed {
        assert!(before.contains_key(name), "{name} was not planted");
    }
    for sub in ["validate", "show"] {
        let extra: &[&str] = if sub == "show" { &["log"] } else { &[] };
        assert_eq!(store(&tmp, sub, extra).code, 0);
    }
    assert_eq!(
        tree(&tmp.root),
        before,
        "a read-only command removed or changed a file"
    );

    let run = store(&tmp, "recover", &["--json"]);
    assert_eq!(run.code, 0, "{}", run.err());
    let records = run.jsonl();
    let rec = one(&records, "StoreReconciled");
    assert_eq!(rec["temp_files_removed"], 6);
    assert_eq!(rec["malformed_roots"], json!([]));
    assert_eq!(rec["reports_without_manifest"], 0);

    let after = tree(&tmp.root);
    let mut expected = before.clone();
    for name in &removed {
        expected.remove(name);
    }
    assert_eq!(
        after, expected,
        "recover must remove exactly the six temp files and nothing else"
    );
    assert_eq!(
        outside.read("target.txt"),
        b"OUTSIDE-CONTENT",
        "a symlink was followed"
    );
    for link in links.iter().copied().chain([backup_link.as_str()]) {
        assert!(
            matches!(after.get(link), Some(Node::Link(t)) if t == &target),
            "{link}: the symlink was removed or replaced"
        );
    }

    let again = store(&tmp, "recover", &["--json"]);
    assert_eq!(again.code, 0);
    assert_eq!(
        one(&again.jsonl(), "StoreReconciled")["temp_files_removed"],
        0
    );
    assert_eq!(tree(&tmp.root), expected);
}

// ---------------------------------------------------------------------------------------------------
// 9: same-second reports

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn run_uuid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn report_file_name(n: u32) -> String {
    format!("20261016T052500Z_demo-repo_{}.json", run_uuid(0xa0 + n))
}

/// The manifest of report `n` (run and report IDs derived from `n`).
fn manifest(n: u32) -> Value {
    let name = report_file_name(n);
    json!({
        "report_id": run_uuid(0xb0 + n),
        "run_id": run_uuid(0xa0 + n),
        "repo_id": run_uuid(0x0a),
        "json_path": format!("backup/{name}"),
        "md_path": format!("backup/{}.md", name.trim_end_matches(".json")),
        "json_written": true,
        "md_written": false,
        "json_digest": DIGEST,
        "generated_at": "2026-10-16T05:25:00Z",
        "pdf_path": null,
        "pdf_written": false,
        "pdf_digest": null,
        "pdf_error_redacted": null,
        "pdf_renderer_version": null
    })
}

/// The report document of report `n`.
fn report_bytes(n: u32) -> Vec<u8> {
    let mut doc = fixture("report.min.json");
    doc["run_id"] = json!(run_uuid(0xa0 + n));
    doc["report_id"] = json!(run_uuid(0xb0 + n));
    serde_json::to_vec_pretty(&doc).unwrap()
}

/// Fails if two reports of one repository generated in the same UTC second (equal stamp and slug, run IDs
/// `...a1` and `...a2`) are not both recognised: `validate` must call the log valid, and `recover` must
/// report no report without a manifest, no manifest without a report, no temp file and no malformed root,
/// and change no file. It also fails if the counters are blind: with the second report file removed
/// `recover` must name exactly that report ID as a manifest without a report, and with an extra report file
/// that has no manifest it must count exactly one report without a manifest.
#[test]
fn same_second_reports_are_recognised_and_not_orphans() {
    let tmp = Tmp::new("same-second");
    prepare(&tmp);
    tmp.private_dir("backup");
    let (first, second) = (report_file_name(1), report_file_name(2));
    assert_ne!(first, second);
    assert_eq!(first[..26], second[..26], "same stamp and slug");
    let mut log = fixture("log.min.json");
    log["reports"] = json!([manifest(1), manifest(2)]);
    tmp.write("log.json", &serde_json::to_vec_pretty(&log).unwrap());
    tmp.write(&format!("backup/{first}"), &report_bytes(1));
    tmp.write(&format!("backup/{second}"), &report_bytes(2));
    let before = tree(&tmp.root);

    let validate = store(&tmp, "validate", &["--json"]);
    assert_eq!(validate.code, 0, "{}", validate.err());
    assert_eq!(
        validated(&validate.jsonl(), "log")["verdict"],
        "valid",
        "the hand-built log with two same-second manifests must validate"
    );

    let recover = store(&tmp, "recover", &["--json"]);
    assert_eq!(recover.code, 0, "{}", recover.err());
    let records = recover.jsonl();
    let rec = one(&records, "StoreReconciled");
    assert_eq!(rec["temp_files_removed"], 0);
    assert_eq!(rec["runs_interrupted"], json!([]));
    assert_eq!(rec["manifests_without_report"], json!([]));
    assert_eq!(rec["reports_without_manifest"], 0);
    assert_eq!(rec["malformed_roots"], json!([]));
    assert_eq!(tree(&tmp.root), before, "recover changed the store");

    // Blind-counter checks.
    std::fs::remove_file(tmp.path(&format!("backup/{second}"))).unwrap();
    let missing = store(&tmp, "recover", &["--json"]);
    let missing_records = missing.jsonl();
    let rec = one(&missing_records, "StoreReconciled");
    assert_eq!(
        strings(&rec["manifests_without_report"]),
        [run_uuid(0xb2)],
        "the missing report must be named by its report ID"
    );
    assert_eq!(rec["reports_without_manifest"], 0);
    tmp.write(&format!("backup/{second}"), &report_bytes(2));
    tmp.write(&format!("backup/{}", report_file_name(3)), &report_bytes(3));
    let orphan = store(&tmp, "recover", &["--json"]);
    let orphan_records = orphan.jsonl();
    let rec = one(&orphan_records, "StoreReconciled");
    assert_eq!(rec["reports_without_manifest"], 1);
    assert_eq!(rec["manifests_without_report"], json!([]));
    assert!(
        tmp.path(&format!("backup/{}", report_file_name(3)))
            .exists(),
        "an unreferenced report must be reported, never deleted"
    );
}

// ---------------------------------------------------------------------------------------------------
// 10: hostile data-directory paths

/// A directory named `name` inside a fresh temp root, mode 0700, and the root that holds it.
fn named_dir(name: &str) -> (Tmp, PathBuf) {
    let holder = Tmp::new("path");
    let dir = holder.root.join(name);
    make_private_dir(&dir);
    (holder, dir)
}

/// Fails if a private data directory whose name contains spaces, Unicode (Japanese, right-to-left text with an
/// override character, combining marks), a leading `-`, a newline, a tab, an ESC, quotes or shell
/// metacharacters is not usable by `init`, `validate`, `show` and `recover` (exit 0, five roots created), if the
/// output contains a raw control or bidirectional character, or if any part of the directory name appears in
/// the output at all (the path must not be echoed).
#[test]
fn hostile_data_directory_names_work_and_are_never_echoed() {
    let names = [
        "dir with spaces",
        "\u{30c7}\u{30fc}\u{30bf}-\u{30c7}\u{30a3}\u{30ec}\u{30af}\u{30c8}\u{30ea}",
        "\u{05e9}\u{05dc}\u{05d5}\u{05dd}-\u{202e}evilname",
        "e\u{0301}\u{0323}combiningmark",
        "-dashfirst",
        "line1\nline2",
        "tab\there",
        "\u{1b}[31mescname",
        "it's \"quoted\" $HOME $(x) ;|& `y`",
        "back\\slash",
    ];
    for name in names {
        let (holder, dir) = named_dir(name);
        let context = format!("{name:?}");
        let init = store_at(&dir, "init", &[]);
        assert_eq!(init.code, 0, "{context}: {}", init.err());
        let runs = [
            init,
            store_at(&dir, "validate", &["--json"]),
            store_at(&dir, "show", &["log"]),
            store_at(&dir, "recover", &[]),
            store_at(&dir, "init", &["--json"]),
        ];
        for run in &runs {
            assert_eq!(run.code, 0, "{context}: {}", run.err());
            assert_no_raw_controls(run, &context);
            for stream in [&run.stdout, &run.stderr] {
                assert!(!contains(stream, name.as_bytes()), "{context}: path echoed");
                // The visible part of the name, at least the part that is not a control character.
                for fragment in [
                    "evilname",
                    "escname",
                    "combiningmark",
                    "dashfirst",
                    "slash",
                    "quoted",
                ] {
                    if name.contains(fragment) {
                        assert!(
                            !contains(stream, fragment.as_bytes()),
                            "{context}: path echoed"
                        );
                    }
                }
            }
        }
        let names_in_dir = {
            let mut v: Vec<String> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            names_in_dir,
            [
                ".app.lock",
                "approvals.json",
                "log.json",
                "repo.json",
                "tools.json",
                "workflows.json"
            ],
            "{context}"
        );
        drop(holder);
    }
}

/// A directory of exactly `target_len` bytes of path under `base`, made of components of at most 200 bytes,
/// mode 0700 at the leaf.
fn deep_dir(base: &Path, target_len: usize) -> PathBuf {
    let mut path = base.to_path_buf();
    let mut len = path.as_os_str().len();
    while len < target_len {
        let remaining = target_len - len - 1;
        let mut segment = remaining.min(200);
        if remaining > 200 && remaining - 200 < 2 {
            segment = remaining - 2;
        }
        path.push("d".repeat(segment));
        len = path.as_os_str().len();
    }
    assert_eq!(len, target_len);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

/// Fails if a data directory whose path is 3900 bytes (well under `PATH_MAX`) is not fully usable (exit 0 for
/// all four commands); if a path of 4090 bytes, where the directory exists but `<dir>/.app.lock` cannot be
/// named, is not a clean storage failure (exit 4, fixed text, nothing printed to stdout, nothing created, no
/// path echoed); or if a path of 5000 bytes (cannot exist) is not rejected with exit 5 and the fixed text.
#[test]
fn data_directory_paths_near_and_over_path_max() {
    let holder = Tmp::new("long");
    let near = deep_dir(&holder.root, 3900);
    for (sub, extra) in [
        ("init", &[][..]),
        ("validate", &[]),
        ("show", &["log"]),
        ("recover", &[]),
    ] {
        let run = store_at(&near, sub, extra);
        assert_eq!(run.code, 0, "3900 bytes, {sub}: {}", run.err());
    }

    let edge_holder = Tmp::new("long-edge");
    let edge = deep_dir(&edge_holder.root, 4090);
    for (sub, extra) in [
        ("init", &[][..]),
        ("validate", &[]),
        ("show", &["log"]),
        ("recover", &[]),
    ] {
        let run = store_at(&edge, sub, extra);
        assert_eq!(run.code, 4, "4090 bytes, {sub}: {}", run.err());
        assert!(run.stdout.is_empty());
        let err = run.err();
        assert!(err.contains("a storage operation failed"), "{err}");
        assert!(!err.contains("ddddd"), "the path was echoed");
    }
    assert!(
        std::fs::read_dir(&edge).unwrap().next().is_none(),
        "a failed command created something in the over-long directory"
    );

    let impossible = PathBuf::from(format!("/{}", "x".repeat(5000)));
    for sub in ["init", "validate", "recover"] {
        let run = store_at(&impossible, sub, &[]);
        assert_eq!(run.code, 5, "5000 bytes, {sub}: {}", run.err());
        assert!(run.stdout.is_empty());
        assert!(run.err().contains("data directory was rejected"));
        assert!(!run.err().contains("xxxxx"), "the path was echoed");
    }
}

/// Fails if a data directory that is a symlink (to a private directory, with and without a trailing slash,
/// and dangling), or a real directory with any group or other permission bit (eight modes), is accepted by
/// any command; if the rejection is not exit 5 with the fixed text and no path; if the directory the symlink
/// points at, or the permissive directory, is written to; or if the same private directory, once it is mode
/// 0700, is not accepted (so the permission bits and not something else caused the rejection).
#[test]
fn symlinked_and_permissive_data_directories_exit_5() {
    let tmp = Tmp::new("reject");
    let real = tmp.path("real");
    make_private_dir(&real);
    let link = tmp.path("link");
    symlink(&real, &link).unwrap();
    let dangling = tmp.path("dangling");
    symlink(tmp.path("nowhere"), &dangling).unwrap();
    let with_slash = PathBuf::from(format!("{}/", link.display()));

    for dir in [&link, &with_slash, &dangling] {
        for sub in ["init", "validate", "recover"] {
            let run = store_at(dir, sub, &[]);
            assert_eq!(run.code, 5, "{} {sub}: {}", dir.display(), run.err());
            assert!(run.stdout.is_empty());
            let err = run.err();
            assert!(err.contains("data directory was rejected"), "{err}");
            assert!(!err.contains(tmp.dir()), "the path was echoed: {err}");
        }
        let show = store_at(dir, "show", &["log"]);
        assert_eq!(show.code, 5);
    }
    assert_eq!(
        std::fs::read_dir(&real).unwrap().count(),
        0,
        "a command followed the symlink and wrote into the target"
    );

    for mode in [0o710, 0o701, 0o750, 0o705, 0o770, 0o707, 0o755, 0o777] {
        let wide = tmp.path(&format!("wide-{mode:o}"));
        make_private_dir(&wide);
        std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(mode)).unwrap();
        for sub in ["init", "validate", "recover"] {
            let run = store_at(&wide, sub, &[]);
            assert_eq!(run.code, 5, "mode {mode:o}, {sub}: {}", run.err());
            assert!(run.stdout.is_empty());
            assert!(run.err().contains("data directory was rejected"));
        }
        assert_eq!(
            std::fs::read_dir(&wide).unwrap().count(),
            0,
            "mode {mode:o}: a rejected directory was written to"
        );
        std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            store_at(&wide, "init", &[]).code,
            0,
            "mode {mode:o}: the same directory at 0700 must be accepted"
        );
    }
}

/// Fails if a `--data-dir` value that is not valid UTF-8 is not a usage error (exit 2, empty stdout,
/// message "not valid UTF-8", the raw bytes not echoed), or if `--data-dir` with a value that starts with `-`
/// or is empty is not exit 2 either; or if any of them touches the file system.
#[test]
fn non_utf8_and_flag_like_data_dir_values_are_usage_errors() {
    let tmp = Tmp::new("utf8");
    let mut bad = tmp.root.clone().into_os_string().into_vec();
    bad.extend_from_slice(b"/\xff\xfe-dir");
    let run = bin_os(&[
        OsString::from("store"),
        OsString::from("init"),
        OsString::from("--data-dir"),
        OsString::from_vec(bad),
    ]);
    assert_eq!(run.code, 2, "{}", run.err());
    assert!(run.stdout.is_empty());
    assert!(run.err().contains("not valid UTF-8"));
    assert!(!contains(&run.stderr, &[0xff]), "raw bytes echoed");
    for value in ["-dash", ""] {
        let run = bin(&["store", "init", "--data-dir", value]);
        assert_eq!(run.code, 2, "{value:?}: {}", run.err());
        assert!(run.stdout.is_empty());
    }
    assert_eq!(tmp.names_without_lock(), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------------
// exit 4

/// Whether this process is subject to file permission checks. Inside a user namespace that maps the user to
/// root (as the development runner does) it is not, and a mode-000 file can still be read.
fn permissions_are_enforced() -> bool {
    let tmp = Tmp::new("probe");
    tmp.write("probe", b"x");
    std::fs::set_permissions(tmp.path("probe"), std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::File::open(tmp.path("probe")).is_err()
}

/// An initialised store without `tools.json` and with a `log.json` that holds one `running` run, so that
/// `init` has a file to create and `recover` has a log to rewrite.
fn seed_recoverable_store(tmp: &Tmp) {
    prepare(tmp);
    std::fs::remove_file(tmp.path("tools.json")).unwrap();
    let mut log = fixture("log.max.json");
    let mut run = log["runs"][0].clone();
    run["run_id"] = json!("00000000-0000-4000-8000-000000000901");
    run["execution"] = json!("running");
    run["created_at"] = json!("2026-10-16T05:25:00Z");
    run["started_at"] = json!("2026-10-16T05:25:00Z");
    run["finished_at"] = Value::Null;
    run["last_checkpoint_at"] = Value::Null;
    run["report_ids"] = json!([]);
    log["runs"] = json!([run]);
    log["events"] = json!([]);
    log["reports"] = json!([]);
    tmp.write("log.json", &serde_json::to_vec_pretty(&log).unwrap());
}

/// Fails (when permissions are enforced) if an unreadable root, or a read-only data directory in which
/// `init` must create files and `recover` must rewrite the log, does not give exit 4 with the fixed
/// storage-failure text and without the path, or if a refused command changes anything. When the process is
/// not subject to permission checks (the runner's user namespace maps the user to root) the test returns at
/// once and proves nothing; the same exit code is pinned there by the class mapping in
/// `application_storage_failures_map_to_exit_4` and by the injected faults in
/// `application/tests/dev_store_faults.rs`.
#[test]
fn real_storage_failure_exits_4_when_permissions_are_enforced() {
    // Control, run whether or not permissions are enforced: the seeded store really is rewritable, so the
    // exit 4 below can only come from the denied write.
    let control = Tmp::new("eacces-control");
    seed_recoverable_store(&control);
    let ok = store(&control, "recover", &["--json"]);
    assert_eq!(ok.code, 0, "{}", ok.err());
    assert_eq!(
        one(&ok.jsonl(), "StoreReconciled")["runs_interrupted"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the seeded log must have one run to interrupt"
    );
    if !permissions_are_enforced() {
        return;
    }
    // An unreadable root: validate fails at the first root it must read, show fails outright.
    let tmp = Tmp::new("eacces-read");
    init(&tmp);
    std::fs::set_permissions(
        tmp.path("repo.json"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    let validate = store(&tmp, "validate", &["--json"]);
    assert_eq!(validate.code, 4, "{}", validate.err());
    assert_eq!(
        validate.jsonl().len(),
        1,
        "the record of the root before the unreadable one is kept"
    );
    assert!(validate.err().contains("a storage operation failed"));
    assert!(!validate.err().contains(tmp.dir()));
    let show = store(&tmp, "show", &["repositories"]);
    assert_eq!(show.code, 4, "{}", show.err());
    assert!(show.stdout.is_empty());

    // A read-only data directory: nothing can be created in it.
    let ro = Tmp::new("eacces-write");
    seed_recoverable_store(&ro);
    let before = tree(&ro.root);
    std::fs::set_permissions(&ro.root, std::fs::Permissions::from_mode(0o500)).unwrap();
    for sub in ["init", "recover"] {
        let run = store(&ro, sub, &[]);
        assert_eq!(run.code, 4, "{sub}: {}", run.err());
        assert!(run.stdout.is_empty(), "{sub}");
        let err = run.err();
        assert!(err.contains("a storage operation failed"), "{err}");
        assert!(!err.contains(ro.dir()));
    }
    std::fs::set_permissions(&ro.root, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(tree(&ro.root), before, "a refused write changed the store");
}

/// Fails if the application's failure classes stop mapping to the CLI exit codes the storage scenarios
/// rely on: `Storage` and `Halted` (the classes the injected ENOSPC/EACCES/EINTR faults in
/// `application/tests/dev_store_faults.rs` produce at every write step) to 4, `Locked` to 3, and the
/// operation failures to 5. This is the CLI half of the (class, exit code) pair of L-T12-2.
#[test]
fn application_storage_failures_map_to_exit_4() {
    use application::dev_store::{DevError, FailureClass};
    use cli::commands::Exit;

    assert_eq!(DevError::Storage.class(), FailureClass::Storage);
    assert_eq!(Exit::for_failure(DevError::Storage.class()).code(), 4);
    assert_eq!(Exit::for_failure(DevError::Halted.class()).code(), 4);
    assert_eq!(Exit::for_failure(DevError::Locked.class()).code(), 3);
    assert_eq!(
        Exit::for_failure(DevError::DataDirRejected.class()).code(),
        5
    );
    // The text the CLI prints for a storage failure is the error's own fixed text.
    assert_eq!(DevError::Storage.to_string(), "a storage operation failed");
}

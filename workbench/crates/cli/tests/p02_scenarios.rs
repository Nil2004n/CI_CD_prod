//! P02-T12 scenario tests for the headless CLI (`planning/P02-T01-request.md` §5.9, §9 row T12, T01-D7,
//! T01-D8, T01-D12; plan §4.1). Almost every test runs the built binary (`pwt-workbench-cli`) in a
//! cleared environment against a private temporary data directory and asserts on its exit code and its
//! output; a few call [`cli::run`] in-process.
//!
//! Expected values (exit codes, root names, version strings, file sets) are written out literally and are
//! not derived from the code under test. Every test says what would make it fail. All store content is
//! synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_pwt-workbench-cli");

/// The five roots `store init` creates (T01-D7), by file name.
const INIT_FILES: [&str; 5] = [
    "approvals.json",
    "log.json",
    "repo.json",
    "tools.json",
    "workflows.json",
];

/// The same five roots by root name.
const INIT_ROOTS: [&str; 5] = [
    "repositories",
    "workflows",
    "approvals",
    "log",
    "tool_manifest",
];

/// The roots a fresh store does not have yet.
const ABSENT_ROOTS: [&str; 4] = ["config", "suppressions", "rag_index", "conversations"];

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
            std::env::temp_dir().join(format!("pwt-cli-p02-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        // The umask may have removed bits but never adds any; set the mode explicitly anyway.
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn dir(&self) -> &str {
        self.root.to_str().unwrap()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, bytes: &[u8]) {
        std::fs::write(self.path(rel), bytes).unwrap();
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    /// The file names directly in the data directory, sorted.
    fn names(&self) -> Vec<String> {
        names_in(&self.root)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The result of one process run.
struct Run {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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
    let output = Command::new(BIN)
        .args(args)
        .env_clear()
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("exit code, not a signal"),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

fn bin(args: &[&str]) -> Run {
    let owned: Vec<OsString> = args.iter().map(OsString::from).collect();
    bin_os(&owned)
}

/// `store <sub> --data-dir <tmp> <extra...>`.
fn store(tmp: &Tmp, sub: &str, extra: &[&str]) -> Run {
    let mut args = vec!["store", sub, "--data-dir", tmp.dir()];
    args.extend_from_slice(extra);
    bin(&args)
}

fn init(tmp: &Tmp) {
    let run = store(tmp, "init", &[]);
    assert_eq!(run.code, 0, "init failed: {}", run.err());
}

/// `store <sub> --data-dir <tmp> <extra...>`, re-run while it exits 3 (lock held), at most 10 s, 20 ms
/// apart; returns the first run that exits otherwise (the caller asserts on it) and the number of tries.
///
/// For use right after a test releases a lock it held through a descriptor of the test process. Sibling
/// tests run in parallel threads of this process and spawn the binary; between `fork` and `exec` the child
/// holds a copy of every descriptor of the test process (`O_CLOEXEC` ones are closed only by `exec`). A
/// `flock` lives until the last descriptor of its open file description is closed, so `drop` of the holder
/// does not release the lock while such a child exists, and a command started in that window correctly
/// exits 3. Only exit 3 is retried; any other code is returned at once. The product is not affected: it
/// never spawns a process while holding the lock.
fn store_after_release(tmp: &Tmp, sub: &str, extra: &[&str]) -> (Run, u32) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut tries = 1;
    loop {
        let run = store(tmp, sub, extra);
        if run.code != 3 || std::time::Instant::now() >= deadline {
            return (run, tries);
        }
        tries += 1;
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The record of `dev_event` `event` among `records`; panics unless there is exactly one.
fn one<'a>(records: &'a [Value], event: &str) -> &'a Value {
    let found: Vec<&Value> = records.iter().filter(|r| r["dev_event"] == event).collect();
    assert_eq!(found.len(), 1, "expected exactly one {event} record");
    found[0]
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|v| v.as_str().expect("a string").to_owned())
        .collect()
}

/// The `StoreValidated` record of root `kind`.
fn validated<'a>(records: &'a [Value], kind: &str) -> &'a Value {
    records
        .iter()
        .find(|r| r["dev_event"] == "StoreValidated" && r["kind"] == kind)
        .unwrap_or_else(|| panic!("no StoreValidated record for {kind}"))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------------------------------
// --version, help, features

/// Fails if any of the four contract versions or the crate version is missing, mislabelled or wrong, in
/// human or JSON form. The expected strings are literals: a bumped constant must be reflected here.
#[test]
fn version_prints_all_contract_versions() {
    let run = bin(&["--version"]);
    assert_eq!(run.code, 0);
    assert!(run.stderr.is_empty());
    let lines: Vec<String> = run.out().lines().map(str::to_owned).collect();
    assert_eq!(
        lines,
        [
            format!("pwt-workbench-cli {}", env!("CARGO_PKG_VERSION")),
            "application contract version: 1.1".to_owned(),
            "store schema version: 1.4".to_owned(),
            "wrapper protocol version: 1.1".to_owned(),
            "UI schema version: 1.1".to_owned(),
        ]
    );
    let json_run = bin(&["--json", "--version"]);
    assert_eq!(json_run.code, 0);
    let records = json_run.jsonl();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record["record"], "version");
    assert_eq!(record["crate_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(record["app_contract_version"], "1.1");
    assert_eq!(record["store_schema_version"], "1.4");
    assert_eq!(record["wrapper_protocol_version"], "1.1");
    assert_eq!(record["ui_schema_version"], "1.1");
}

/// Fails if `help` omits a command, a store command lacks its usage line, or no-argument invocation is not a
/// usage error.
#[test]
fn help_lists_every_command() {
    let run = bin(&["help"]);
    assert_eq!(run.code, 0);
    let out = run.out();
    for usage in [
        "pwt-workbench-cli --version",
        "pwt-workbench-cli help [command]",
        "pwt-workbench-cli features",
        "pwt-workbench-cli store init --data-dir <dir>",
        "pwt-workbench-cli store validate --data-dir <dir> [--root <root>]",
        "pwt-workbench-cli store show --data-dir <dir> <root>",
        "pwt-workbench-cli store recover --data-dir <dir> [--restore-previous <root>]",
    ] {
        assert!(out.contains(usage), "help lacks `{usage}`:\n{out}");
    }
    assert!(out.contains("exit codes: 0 executed"));
    // JSON form: one line per command, seven commands.
    let json_run = bin(&["--json", "help"]);
    assert_eq!(json_run.code, 0);
    let records = json_run.jsonl();
    let commands: Vec<&str> = records
        .iter()
        .map(|r| r["command"].as_str().unwrap())
        .collect();
    assert_eq!(
        commands,
        [
            "--version",
            "help",
            "features",
            "store init",
            "store validate",
            "store show",
            "store recover"
        ]
    );
    // No arguments at all: a usage error with the usage reminder, and nothing on stdout.
    let none = bin(&[]);
    assert_eq!(none.code, 2);
    assert!(none.stdout.is_empty());
    assert!(none.err().contains("usage:"));
}

/// Fails if `help <command>` prints other commands, `help store` omits one of the four, or an unknown
/// topic is not exit 2.
#[test]
fn help_for_one_command_and_unknown_topic() {
    let one_cmd = bin(&["help", "store", "init"]);
    assert_eq!(one_cmd.code, 0);
    let out = one_cmd.out();
    assert!(out.contains("store init --data-dir <dir>"));
    assert!(!out.contains("store validate"));
    assert!(!out.contains("exit codes:"));

    let group = bin(&["help", "store"]);
    assert_eq!(group.code, 0);
    let out = group.out();
    for name in [
        "store init",
        "store validate",
        "store show",
        "store recover",
    ] {
        assert!(out.contains(name), "help store lacks {name}");
    }
    assert!(!out.contains("pwt-workbench-cli features"));
    assert!(out.contains("roots: config, repositories"));

    let version = bin(&["help", "version"]);
    assert_eq!(version.code, 0);
    assert!(version.out().contains("--version"));

    for args in [&["help", "nonsense"][..], &["help", "store", "nonsense"]] {
        let run = bin(args);
        assert_eq!(run.code, 2, "{args:?}");
        assert!(run.stdout.is_empty());
        assert!(!run.err().contains("nonsense"), "usage error echoes input");
    }
}

/// Fails if `features` omits a row (command, scenario or mode), if a command has no row, or if the JSON form
/// stops being one parseable object per row.
#[test]
fn features_prints_the_index() {
    let run = bin(&["features"]);
    assert_eq!(run.code, 0);
    let out = run.out();
    for row in cli::features::FEATURES {
        assert!(out.contains(row.scenario), "{} missing", row.scenario);
        assert!(out.contains(row.command));
        assert!(out.contains(row.mode.as_str()));
    }
    let json_run = bin(&["features", "--json"]);
    assert_eq!(json_run.code, 0);
    let records = json_run.jsonl();
    assert_eq!(records.len(), cli::features::FEATURES.len());
    let mut commands = BTreeSet::new();
    for (record, row) in records.iter().zip(cli::features::FEATURES) {
        assert_eq!(record["record"], "feature");
        assert_eq!(record["scenario"], row.scenario);
        assert_eq!(record["command"], row.command);
        assert_eq!(record["mode"], row.mode.as_str());
        commands.insert(row.command.to_owned());
    }
    // One row per P02 command at least.
    for command in [
        "--version",
        "help",
        "features",
        "store init",
        "store validate",
        "store show",
        "store recover",
    ] {
        assert!(commands.contains(command), "no feature row for {command}");
    }
}

// ---------------------------------------------------------------------------------------------------
// store init, validate

/// Fails if `init` creates anything but the five collection roots (and the lock file), reports them wrongly,
/// or a second `init` changes a byte, creates a file or fails to report all five as already present.
#[test]
fn init_creates_exactly_five_roots_and_is_idempotent() {
    let tmp = Tmp::new("init");
    let first = store(&tmp, "init", &["--json"]);
    assert_eq!(first.code, 0, "{}", first.err());
    let records = first.jsonl();
    assert_eq!(records.len(), 1);
    let rec = one(&records, "StoreInitialised");
    assert_eq!(rec["mode"], "dev");
    let mut created = strings(&rec["created"]);
    created.sort();
    let mut expected: Vec<String> = INIT_ROOTS.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(created, expected);
    assert!(strings(&rec["already_present"]).is_empty());
    let mut files: Vec<String> = INIT_FILES.iter().map(|s| (*s).to_owned()).collect();
    files.push(".app.lock".to_owned());
    files.sort();
    assert_eq!(tmp.names(), files);

    let bytes_before: Vec<Vec<u8>> = INIT_FILES.iter().map(|f| tmp.read(f)).collect();
    let second = store(&tmp, "init", &["--json"]);
    assert_eq!(second.code, 0, "{}", second.err());
    let records = second.jsonl();
    let rec = one(&records, "StoreInitialised");
    assert!(strings(&rec["created"]).is_empty());
    let mut present = strings(&rec["already_present"]);
    present.sort();
    assert_eq!(present, expected);
    assert_eq!(tmp.names(), files);
    let bytes_after: Vec<Vec<u8>> = INIT_FILES.iter().map(|f| tmp.read(f)).collect();
    assert_eq!(bytes_before, bytes_after, "a second init rewrote a root");

    // Human mode names the roots too.
    let human = store(&tmp, "init", &[]);
    assert_eq!(human.code, 0);
    assert!(human.out().contains("already present: "));
}

/// Fails if `validate` reports the wrong verdict for an initialised or an absent root, if `--root` does not
/// restrict the output to one record, or if `validate` changes any file.
#[test]
fn validate_fresh_store_and_single_root() {
    let tmp = Tmp::new("validate");
    init(&tmp);
    let before: Vec<Vec<u8>> = INIT_FILES.iter().map(|f| tmp.read(f)).collect();
    let names_before = tmp.names();

    let run = store(&tmp, "validate", &["--json"]);
    assert_eq!(run.code, 0, "{}", run.err());
    let records = run.jsonl();
    assert_eq!(records.len(), 9, "one StoreValidated per root");
    for root in INIT_ROOTS {
        let rec = validated(&records, root);
        assert_eq!(rec["verdict"], "valid", "{root}");
        assert!(rec["problems"].as_array().unwrap().is_empty());
    }
    for root in ABSENT_ROOTS {
        assert_eq!(validated(&records, root)["verdict"], "missing", "{root}");
    }

    let single = store(&tmp, "validate", &["--root", "repositories", "--json"]);
    assert_eq!(single.code, 0);
    let records = single.jsonl();
    assert_eq!(records.len(), 1);
    assert_eq!(validated(&records, "repositories")["verdict"], "valid");

    let human = store(&tmp, "validate", &["--root", "config"]);
    assert_eq!(human.code, 0);
    assert_eq!(human.out().trim_end(), "config: missing");

    // Read-only: no new file, no changed root.
    assert_eq!(tmp.names(), names_before);
    let after: Vec<Vec<u8>> = INIT_FILES.iter().map(|f| tmp.read(f)).collect();
    assert_eq!(before, after, "validate changed a root");
}

/// Fails if a malformed root is not reported `invalid` with the JSON syntax problem, if the command exits
/// non-zero for it, if the other roots are affected, or if the malformed file is touched.
#[test]
fn malformed_root_is_invalid_and_exit_is_zero() {
    let tmp = Tmp::new("malformed");
    init(&tmp);
    let garbage = b"{\"schema_version\": \"1.4\", \"repositories\": [".to_vec();
    tmp.write("repo.json", &garbage);
    let run = store(&tmp, "validate", &["--json"]);
    assert_eq!(
        run.code,
        0,
        "an invalid store is still exit 0: {}",
        run.err()
    );
    let records = run.jsonl();
    let rec = validated(&records, "repositories");
    assert_eq!(rec["verdict"], "invalid");
    assert_eq!(rec["mode"], "dev");
    assert_eq!(
        rec["problems"],
        json!([{ "layer": "json", "code": "syntax", "pointer": "", "rule": null }])
    );
    assert_eq!(validated(&records, "log")["verdict"], "valid");
    assert_eq!(
        tmp.read("repo.json"),
        garbage,
        "validate repaired or rewrote the file"
    );
}

/// Fails if a root labelled schema 1.3 is accepted or reported with anything other than the version-gate
/// problem at `/schema_version`, or if `show` of it prints a document.
#[test]
fn schema_1_3_label_is_rejected_with_the_version_problem() {
    let tmp = Tmp::new("v13");
    init(&tmp);
    tmp.write(
        "repo.json",
        br#"{"schema_version":"1.3","store_type":"repositories","updated_at":"2026-10-16T05:25:00Z","repositories":[]}"#,
    );
    let run = store(&tmp, "validate", &["--root", "repositories", "--json"]);
    assert_eq!(run.code, 0);
    let records = run.jsonl();
    let rec = validated(&records, "repositories");
    assert_eq!(rec["verdict"], "invalid");
    assert_eq!(
        rec["problems"],
        json!([{
            "layer": "version",
            "code": "unsupported_version_older",
            "pointer": "/schema_version",
            "rule": null
        }])
    );
    let shown = store(&tmp, "show", &["repositories", "--json"]);
    assert_eq!(shown.code, 0);
    let records = shown.jsonl();
    let rec = one(&records, "StoreShown");
    assert_eq!(rec["verdict"], "invalid");
    assert_eq!(rec["document"], Value::Null);
}

// ---------------------------------------------------------------------------------------------------
// store show

/// Fails if `show` of a malformed root prints any of the file content (including a raw ESC byte), omits the
/// problem, prints a document, or exits non-zero; and if `show` of a valid root prints the raw bytes rather
/// than the validated, typed, re-serialised document.
#[test]
fn show_of_a_malformed_root_prints_problems_only() {
    let tmp = Tmp::new("show");
    init(&tmp);
    let mut raw = b"{\"k\": \"\x1b[31mRAW-BYTES-MARKER".to_vec();
    raw.extend_from_slice("\u{202e}tail".as_bytes());
    tmp.write("repo.json", &raw);
    for json_flag in [false, true] {
        let mut extra = vec!["repositories"];
        if json_flag {
            extra.push("--json");
        }
        let run = store(&tmp, "show", &extra);
        assert_eq!(run.code, 0, "{}", run.err());
        for stream in [&run.stdout, &run.stderr] {
            assert!(!contains(stream, b"RAW-BYTES-MARKER"));
            assert!(!contains(stream, b"tail"));
            assert!(!contains(stream, &[0x1b]));
            assert!(!contains(stream, "\u{202e}".as_bytes()));
        }
        let out = run.out();
        assert!(out.contains("syntax"), "problem missing: {out}");
        if json_flag {
            let records = run.jsonl();
            let rec = one(&records, "StoreShown");
            assert_eq!(rec["mode"], "dev");
            assert_eq!(rec["verdict"], "invalid");
            assert_eq!(rec["document"], Value::Null);
        } else {
            assert!(out.starts_with("repositories: invalid"));
        }
    }

    // A valid root written compactly with its members in a different order.
    let compact = br#"{"store_type":"log","reports":[],"events":[],"runs":[],"updated_at":"2026-10-16T05:25:00Z","schema_version":"1.4"}"#;
    tmp.write("log.json", compact);
    let run = store(&tmp, "show", &["log"]);
    assert_eq!(run.code, 0, "{}", run.err());
    let out = run.out();
    let parsed: Value = serde_json::from_str(&out).expect("human show output is the JSON document");
    assert_eq!(parsed, serde_json::from_slice::<Value>(compact).unwrap());
    assert_ne!(
        out.trim_end().as_bytes(),
        compact.as_slice(),
        "raw bytes were echoed"
    );
    let version_at = out.find("schema_version").expect("schema_version printed");
    let type_at = out.find("store_type").expect("store_type printed");
    assert!(
        version_at < type_at,
        "member order is the typed model's, not the file's"
    );
    let json_run = store(&tmp, "show", &["log", "--json"]);
    assert_eq!(json_run.code, 0);
    let records = json_run.jsonl();
    let rec = one(&records, "StoreShown");
    assert_eq!(rec["verdict"], "valid");
    assert_eq!(rec["document"]["store_type"], "log");
    assert_eq!(rec["document"]["schema_version"], "1.4");
    // Missing roots are reported as such.
    let missing = store(&tmp, "show", &["config", "--json"]);
    assert_eq!(missing.code, 0);
    assert_eq!(one(&missing.jsonl(), "StoreShown")["verdict"], "missing");
}

/// Fails if an ESC or bidi control character in a store file's text reaches standard output or standard
/// error unescaped (human, JSON), or if the escaped form is missing (which would mean the text was dropped
/// rather than escaped), or if the readable part of the marker disappears.
#[test]
fn seeded_marker_never_reaches_output_unescaped() {
    let tmp = Tmp::new("marker");
    init(&tmp);
    // The second repository of the P02 `repositories.max` fixture, with a seeded `last_error_redacted`.
    let marker = "update failed \u{1b}[31mSEEDMARK\u{202e}rtl\u{2066}iso\u{85}c1";
    let doc = json!({
        "schema_version": "1.4",
        "store_type": "repositories",
        "updated_at": "2026-10-16T05:25:00Z",
        "repositories": [{
            "repo_id": "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b",
            "display_name": "Example service",
            "slug": "example-2",
            "remote_url": "ssh://git@git.example.invalid:2222/org/repo.git",
            "canonical_remote": "git.example.invalid/org/repo",
            "local_path": "checkouts/0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b",
            "default_ref_kind": "tag",
            "default_ref_value": "v1.2.3",
            "registered_at": "2026-10-16T05:25:00Z",
            "last_update_at": "2026-10-16T05:25:00.123456789Z",
            "last_commit_oid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "checkout_state": "error",
            "last_error_redacted": marker,
            "inventory_revision": 18446744073709551615u64,
            "inventory_at": "2026-10-16T05:25:00Z",
            "files": []
        }]
    });
    let encoded = serde_json::to_string(&doc).unwrap();
    // The file itself holds the raw bidi characters (only ESC must be JSON-escaped); a control that is
    // absent from the file would make this test vacuous.
    assert!(contains(encoded.as_bytes(), "\u{202e}".as_bytes()));
    tmp.write("repo.json", encoded.as_bytes());

    let validate = store(&tmp, "validate", &["--root", "repositories", "--json"]);
    assert_eq!(validate.code, 0, "{}", validate.err());
    assert_eq!(
        validated(&validate.jsonl(), "repositories")["verdict"],
        "valid",
        "the seeded store must itself be valid, or the show path is not exercised"
    );
    for extra in [&["repositories"][..], &["repositories", "--json"]] {
        let run = store(&tmp, "show", extra);
        assert_eq!(run.code, 0, "{}", run.err());
        for stream in [&run.stdout, &run.stderr] {
            assert!(!contains(stream, &[0x1b]), "raw ESC reached output");
            assert!(
                !contains(stream, "\u{202e}".as_bytes()),
                "raw RLO reached output"
            );
            assert!(
                !contains(stream, "\u{2066}".as_bytes()),
                "raw LRI reached output"
            );
            assert!(
                !contains(stream, "\u{85}".as_bytes()),
                "raw NEL reached output"
            );
        }
        let out = run.out();
        assert!(out.contains("SEEDMARK"), "marker text lost: {out}");
        assert!(out.contains("<U+001B>"), "ESC not escaped as text: {out}");
        assert!(out.contains("<U+202E>"), "RLO not escaped as text: {out}");
        if extra.contains(&"--json") {
            let records = run.jsonl();
            let rec = one(&records, "StoreShown");
            let shown = rec["document"]["repositories"][0]["last_error_redacted"]
                .as_str()
                .unwrap();
            assert!(shown.contains("SEEDMARK"));
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// store recover

/// Pre-plants an interrupted-write state: a malformed `repo.json` with a valid `repo.json.prev`, two leftover
/// temp files of the application's exact pattern, and two files that look similar but are not the
/// application's (they must survive). Returns the bytes of the malformed file.
fn plant_interrupted_write(tmp: &Tmp) -> Vec<u8> {
    init(tmp);
    std::fs::copy(tmp.path("repo.json"), tmp.path("repo.json.prev")).unwrap();
    let garbage = b"{ interrupted".to_vec();
    tmp.write("repo.json", &garbage);
    tmp.write(".repo.json.tmp-0123456789abcdef", b"partial");
    tmp.write(".log.json.prev.tmp-fedcba9876543210", b"partial");
    tmp.write(".repo.json.tmp-not-hex", b"keep");
    tmp.write("notes.txt", b"keep");
    garbage
}

/// Fails if `recover` does not remove exactly the two exact-pattern temp files, leaves the look-alikes
/// alone, repairs or rotates the malformed root, omits it from the report, or exits non-zero.
#[test]
fn recover_removes_leftover_temp_files_and_reports_malformed_roots() {
    let tmp = Tmp::new("recover");
    let garbage = plant_interrupted_write(&tmp);
    let prev = tmp.read("repo.json.prev");
    let run = store(&tmp, "recover", &["--json"]);
    assert_eq!(run.code, 0, "{}", run.err());
    let records = run.jsonl();
    assert_eq!(records.len(), 1);
    let rec = one(&records, "StoreReconciled");
    assert_eq!(rec["mode"], "dev");
    assert_eq!(rec["temp_files_removed"], 2);
    assert_eq!(strings(&rec["malformed_roots"]), ["repositories"]);
    assert_eq!(rec["runs_interrupted"], json!([]));
    assert_eq!(rec["approvals_pending"], 0);

    let names = tmp.names();
    assert!(!names.contains(&".repo.json.tmp-0123456789abcdef".to_owned()));
    assert!(!names.contains(&".log.json.prev.tmp-fedcba9876543210".to_owned()));
    assert!(names.contains(&".repo.json.tmp-not-hex".to_owned()));
    assert!(names.contains(&"notes.txt".to_owned()));
    assert_eq!(
        tmp.read("repo.json"),
        garbage,
        "a malformed root was repaired"
    );
    assert_eq!(tmp.read("repo.json.prev"), prev, ".prev was changed");
    assert!(!names.contains(&"quarantine".to_owned()));

    // Human output reports the same counts.
    let human = store(&tmp, "recover", &[]);
    assert_eq!(human.code, 0);
    let out = human.out();
    assert!(out.contains("temp files removed: 0"));
    assert!(out.contains("malformed roots: repositories"));
}

/// Fails if `--restore-previous` does not put the previous generation back, does not move the malformed file
/// to `quarantine/` byte for byte, omits either record, or leaves the root invalid.
#[test]
fn restore_previous_restores_and_quarantines() {
    let tmp = Tmp::new("restore");
    let garbage = plant_interrupted_write(&tmp);
    let prev = tmp.read("repo.json.prev");
    let run = store(
        &tmp,
        "recover",
        &["--restore-previous", "repositories", "--json"],
    );
    assert_eq!(run.code, 0, "{}", run.err());
    let records = run.jsonl();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["dev_event"], "StoreReconciled");
    assert_eq!(records[0]["temp_files_removed"], 2);
    assert_eq!(strings(&records[0]["malformed_roots"]), ["repositories"]);
    assert_eq!(records[1]["dev_event"], "StoreRestored");
    assert_eq!(records[1]["mode"], "dev");
    assert_eq!(records[1]["kind"], "repositories");
    assert_eq!(records[1]["quarantined"], true);

    assert_eq!(
        tmp.read("repo.json"),
        prev,
        "the previous generation was not restored"
    );
    let quarantined = names_in(&tmp.path("quarantine"));
    assert_eq!(quarantined.len(), 1, "{quarantined:?}");
    assert!(quarantined[0].starts_with("repo.json."), "{quarantined:?}");
    assert_eq!(
        std::fs::read(tmp.path("quarantine").join(&quarantined[0])).unwrap(),
        garbage,
        "the malformed file must be kept, not deleted"
    );
    let after = store(&tmp, "validate", &["--root", "repositories", "--json"]);
    assert_eq!(
        validated(&after.jsonl(), "repositories")["verdict"],
        "valid"
    );
}

/// Fails if a failed restore hides the reconciliation that already happened, exits with anything other than
/// 5, echoes a path, or restores the reserved `suppressions.json`.
#[test]
fn restore_without_previous_generation_exits_5_and_keeps_the_reconciliation_record() {
    let tmp = Tmp::new("restore-fail");
    init(&tmp);
    let run = store(&tmp, "recover", &["--restore-previous", "log", "--json"]);
    assert_eq!(run.code, 5);
    let records = run.jsonl();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["dev_event"], "StoreReconciled");
    let err = run.err();
    assert!(err.contains("no previous generation"), "{err}");
    assert!(err.contains("root log"), "{err}");
    assert!(!err.contains(tmp.dir()));
    let reserved = store(&tmp, "recover", &["--restore-previous", "suppressions"]);
    assert_eq!(reserved.code, 5);
    assert!(reserved.err().contains("refused"));
    assert!(!tmp.names().contains(&"quarantine".to_owned()));
}

// ---------------------------------------------------------------------------------------------------
// exit codes

/// Fails if a store command does not exit 3 while another holder has the data-directory lock (for every
/// command that takes it), if the failure leaks the path, if stdout is not empty, or if the same command
/// does not succeed once the lock is released (which would mean the lock was not the cause).
#[test]
fn lock_held_by_another_holder_exits_3() {
    let tmp = Tmp::new("lock");
    init(&tmp);
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path(".app.lock"))
        .unwrap();
    held.try_lock().expect("the test takes the lock first");
    for (sub, extra) in [
        ("validate", &[][..]),
        ("show", &["log"]),
        ("recover", &[]),
        ("init", &[]),
    ] {
        let run = store(&tmp, sub, extra);
        assert_eq!(run.code, 3, "store {sub}: {}", run.err());
        assert!(run.stdout.is_empty(), "store {sub} printed to stdout");
        let err = run.err();
        assert!(err.contains("locked by another instance"), "{err}");
        assert!(!err.contains(tmp.dir()));
    }
    drop(held);
    let (free, tries) = store_after_release(&tmp, "validate", &[]);
    assert_eq!(free.code, 0, "after {tries} tries: {}", free.err());
}

/// Fails if a missing `--data-dir`, an unknown command, an unknown flag, a missing or unknown root, a
/// non-UTF-8 argument or a flag the command does not take is not exit 2 with an empty stdout and a message
/// that does not repeat the input.
#[test]
fn usage_errors_exit_2() {
    let tmp = Tmp::new("usage");
    let d = tmp.dir();
    let cases: Vec<Vec<&str>> = vec![
        vec!["store", "init"],
        vec!["store", "validate"],
        vec!["store", "show", "log"],
        vec!["store", "recover"],
        vec!["bogus-command-zz"],
        vec!["--bogus-flag-zz"],
        vec!["store", "bogus-sub-zz", "--data-dir", d],
        vec!["store", "init", "--data-dir", d, "--bogus-flag-zz"],
        vec!["store", "show", "--data-dir", d],
        vec!["store", "show", "--data-dir", d, "bogus-root-zz"],
        vec!["store", "show", "--data-dir", d, "report"],
        vec![
            "store",
            "validate",
            "--data-dir",
            d,
            "--root",
            "bogus-root-zz",
        ],
        vec!["store", "init", "--data-dir"],
        vec!["store", "init", "--data-dir", d, "--data-dir", d],
        vec!["store", "init", "--data-dir", d, "--root", "log"],
        vec![
            "store",
            "validate",
            "--data-dir",
            d,
            "--restore-previous",
            "log",
        ],
        vec!["features", "--data-dir", d],
        vec!["--version", "features"],
        vec!["store"],
    ];
    for args in cases {
        let run = bin(&args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.err());
        assert!(run.stdout.is_empty(), "{args:?} printed to stdout");
        let err = run.err();
        assert!(err.starts_with("error: "), "{args:?}: {err}");
        assert!(err.contains("usage:"));
        assert!(!err.contains("zz"), "{args:?} echoed input: {err}");
        assert!(!err.contains(d));
    }
    assert!(
        tmp.names().is_empty(),
        "a usage error touched the data directory"
    );

    let non_utf8 = bin_os(&[
        OsString::from("features"),
        OsString::from_vec(vec![0x66, 0xff, 0x6f]),
    ]);
    assert_eq!(non_utf8.code, 2);
    assert!(non_utf8.stdout.is_empty());
    assert!(!contains(&non_utf8.stderr, &[0xff]), "raw bytes echoed");
    assert!(non_utf8.err().contains("not valid UTF-8"));
}

/// Fails if a data directory that is missing, relative, too permissive or not a directory is not exit 5
/// with a fixed message, or if one of those cases creates anything.
#[test]
fn rejected_data_directory_exits_5() {
    let tmp = Tmp::new("reject");
    let missing = tmp.path("does-not-exist");
    let wide = tmp.path("wide");
    std::fs::DirBuilder::new()
        .mode(0o755)
        .create(&wide)
        .unwrap();
    std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(0o755)).unwrap();
    tmp.write("plain-file", b"x");
    let cases = [
        missing.to_str().unwrap().to_owned(),
        String::from("relative/dir"),
        wide.to_str().unwrap().to_owned(),
        tmp.path("plain-file").to_str().unwrap().to_owned(),
    ];
    for dir in &cases {
        for sub in ["init", "validate", "recover"] {
            let run = bin(&["store", sub, "--data-dir", dir]);
            assert_eq!(run.code, 5, "{sub} {dir}: {}", run.err());
            assert!(run.stdout.is_empty());
            let err = run.err();
            assert!(err.contains("data directory was rejected"), "{err}");
            assert!(!err.contains(dir.as_str()), "the path was echoed: {err}");
        }
    }
    assert!(!missing.exists());
    assert_eq!(
        names_in(&wide),
        Vec::<String>::new(),
        "a rejected directory was written to"
    );
}

/// Fails if the exit-code enum changes value, if a failure class maps to the wrong code, or if a `DevError`
/// can map to 0, 2 or the reserved 6. Exit 4 (storage failure or halted writer) cannot be provoked through
/// the file system as root in the sandbox, so its CLI mapping is pinned here and the storage failure itself
/// by `application::dev_store` unit tests with an injected fault.
#[test]
fn exit_codes_map_as_specified() {
    use application::dev_store::DevError;
    use cli::commands::Exit;
    use domain::store::StoreKind;

    let codes: Vec<u8> = Exit::ALL.iter().map(|e| e.code()).collect();
    assert_eq!(codes, [0, 2, 3, 4, 5, 6]);
    let expect = [
        (DevError::Locked, 3),
        (DevError::Storage, 4),
        (DevError::Halted, 4),
        (DevError::DataDirRejected, 5),
        (DevError::SchemaUnavailable, 5),
        (DevError::NoPreviousGeneration(StoreKind::Log), 5),
        (DevError::PreviousInvalid(StoreKind::Log), 5),
        (DevError::Malformed(StoreKind::Log), 5),
        (DevError::Refused, 5),
        (DevError::Unrepresentable, 5),
    ];
    for (error, code) in expect {
        assert_eq!(Exit::for_failure(error.class()).code(), code, "{error:?}");
    }
}

// ---------------------------------------------------------------------------------------------------
// --json

/// Fails if any stdout line of any command is not a JSON object, if a store command's record lacks
/// `"mode":"dev"` and a `dev_event`, if a real command's lines lack `record`, or if stderr is used for
/// results.
#[test]
fn json_lines_parse_and_store_records_carry_mode_dev() {
    let tmp = Tmp::new("json");
    let runs = [
        store(&tmp, "init", &["--json"]),
        store(&tmp, "validate", &["--json"]),
        store(&tmp, "show", &["log", "--json"]),
        store(&tmp, "recover", &["--json"]),
    ];
    let events = [
        "StoreInitialised",
        "StoreValidated",
        "StoreShown",
        "StoreReconciled",
    ];
    for (run, event) in runs.iter().zip(events) {
        assert_eq!(run.code, 0, "{event}: {}", run.err());
        assert!(run.stderr.is_empty(), "{event} wrote to stderr");
        let records = run.jsonl();
        assert!(!records.is_empty());
        for record in &records {
            assert!(record.is_object());
            assert_eq!(record["mode"], "dev", "{event}");
            assert_eq!(record["dev_event"], event);
        }
    }
    for args in [
        &["--json", "--version"][..],
        &["--json", "help"],
        &["--json", "features"],
    ] {
        let run = bin(args);
        assert_eq!(run.code, 0);
        for record in run.jsonl() {
            assert!(record.is_object());
            assert!(record["record"].is_string(), "{args:?}");
        }
    }
    // A failure leaves stdout empty (nothing to parse) and the message on stderr.
    let failing = bin(&["--json", "store", "validate", "--data-dir", "relative/dir"]);
    assert_eq!(failing.code, 5);
    assert!(failing.stdout.is_empty());
}

// ---------------------------------------------------------------------------------------------------
// in-process

/// Fails if `run` panics or exits 0 when standard output is closed, or loses a successful result's exit
/// code when it is not.
#[test]
fn a_closed_stdout_does_not_panic_and_is_exit_5() {
    struct Closed;
    impl std::io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    let args = |parts: &[&str]| -> Vec<OsString> {
        std::iter::once("pwt-workbench-cli")
            .chain(parts.iter().copied())
            .map(OsString::from)
            .collect()
    };
    let mut stderr = Vec::new();
    let code = cli::run(args(&["--version"]), std::io::empty(), Closed, &mut stderr);
    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", std::process::ExitCode::from(5))
    );
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("output could not be written")
    );

    let mut stdout = Vec::new();
    let ok = cli::run(
        args(&["--version"]),
        std::io::empty(),
        &mut stdout,
        std::io::sink(),
    );
    assert_eq!(
        format!("{ok:?}"),
        format!("{:?}", std::process::ExitCode::SUCCESS)
    );
    assert!(!stdout.is_empty());

    // A usage error with a closed stderr must not panic either.
    let usage = cli::run(args(&["bogus"]), std::io::empty(), std::io::sink(), Closed);
    assert_eq!(
        format!("{usage:?}"),
        format!("{:?}", std::process::ExitCode::from(2))
    );
}

/// Fails if the in-process entry point and the binary disagree on the exit code of the same lock conflict.
#[test]
fn in_process_run_reports_a_held_lock_as_exit_3() {
    let tmp = Tmp::new("inproc-lock");
    init(&tmp);
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path(".app.lock"))
        .unwrap();
    held.try_lock().expect("the test takes the lock first");
    let args: Vec<OsString> = [
        "pwt-workbench-cli",
        "store",
        "validate",
        "--data-dir",
        tmp.dir(),
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::run(args, std::io::empty(), &mut stdout, &mut stderr);
    assert_eq!(
        format!("{code:?}"),
        format!("{:?}", std::process::ExitCode::from(3))
    );
    assert!(stdout.is_empty());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("locked by another instance")
    );
}

// ---------------------------------------------------------------------------------------------------
// feature index staleness

/// Whether `sources` define a test function named `name`: the text `fn <name>(` preceded, within the three
/// lines above it, by `#[test]`.
fn defines_test(sources: &[String], name: &str) -> bool {
    let needle = format!("fn {name}(");
    sources.iter().any(|src| {
        let lines: Vec<&str> = src.lines().collect();
        lines.iter().enumerate().any(|(i, line)| {
            line.trim_start().starts_with(&needle)
                && lines[i.saturating_sub(3)..i]
                    .iter()
                    .any(|l| l.trim() == "#[test]")
        })
    })
}

/// The sources of every `tests/p02_*.rs` file of this crate.
fn test_sources() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with("p02_") && name.ends_with(".rs") {
            sources.push(std::fs::read_to_string(path).unwrap());
        }
    }
    sources
}

/// Fails if a feature-index row names a command that the real parser does not turn into that command, a
/// command missing from the help table, a scenario that is not a `#[test]` function in `tests/p02_*.rs`, or
/// a manual-pending row without a reason; and if the checker itself accepts a made-up test name.
#[test]
fn feature_index_rows_name_an_existing_command_and_test() {
    let sources = test_sources();
    assert!(!sources.is_empty());
    assert!(
        !defines_test(&sources, "no_such_scenario_function"),
        "the staleness checker accepts any name"
    );
    assert!(defines_test(&sources, "features_prints_the_index"));
    // A function that exists but is not a test must not count.
    let fake = vec![String::from("fn helper_only() {}\n")];
    assert!(!defines_test(&fake, "helper_only"));

    assert!(!cli::features::FEATURES.is_empty());
    for row in cli::features::FEATURES {
        let info = cli::commands::COMMANDS
            .iter()
            .find(|c| c.name == row.command)
            .unwrap_or_else(|| {
                panic!(
                    "row {} names an unknown command {}",
                    row.scenario, row.command
                )
            });
        let mut argv: Vec<OsString> = vec![OsString::from("pwt-workbench-cli")];
        argv.extend(row.command.split(' ').map(OsString::from));
        if info.data_dir {
            argv.push(OsString::from("--data-dir"));
            argv.push(OsString::from("/pwt-staleness-check"));
        }
        if row.command == "store show" {
            argv.push(OsString::from("log"));
        }
        let parsed = cli::args::parse(argv).unwrap_or_else(|e| {
            panic!(
                "row {}: the parser rejects `{}`: {e}",
                row.scenario, row.command
            )
        });
        assert_eq!(parsed.command.name(), row.command);
        assert!(
            defines_test(&sources, row.scenario),
            "row for `{}` names a missing test function `{}`",
            row.command,
            row.scenario
        );
        match row.mode {
            cli::features::FeatureMode::ManualPending => {
                assert!(row.reason.is_some(), "manual-pending row without a reason");
            }
            _ => assert!(
                row.reason.is_none(),
                "only manual-pending rows carry a reason"
            ),
        }
        assert!(!row.requirement.is_empty());
    }
}

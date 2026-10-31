//! P03-T10 scenario tests for the CLI increment (`planning/P03-T01-request.md` §5.7, §5.4 including the
//! 2026-10-31 amendment, §5.6, §7, §8 "T10"; lead decisions T01-D5 to T01-D7; plan §4.1 P03 row).
//!
//! Almost every test calls [`cli::run_session`] in-process with a [`cli::Session`] and in-memory streams, so
//! that one session can script `credential set`, `status` and `delete`; a few run the built binary
//! (`pwt-workbench-cli`) in a cleared environment to prove the process-level behaviour (exit codes, a key
//! that is held for one process only). Every test works in a private temporary data directory (mode 0700).
//!
//! Expected values (exit codes, texts, states) are written out literally and are not derived from the code
//! under test. Every test says what would make it fail. All data is synthetic; the "keys" are recognisably
//! fake (`pwt-test-FAKE-...`) and are never printed in an assertion message.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use application::child_env::HostEnv;
use cli::Session;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_pwt-workbench-cli");

/// The workflow of the persistence fixture; its file holds it at revision 1.
const FIXTURE_WORKFLOW: &str = "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d";
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../persistence/tests/fixtures/store"
);

/// A recognisably fake key (38 characters, printable ASCII).
const KEY: &str = "pwt-test-FAKE-9f2c41d07a5b83e6-keyvalue";
/// The part of [`KEY`] that no other text of these tests contains.
const KEY_CORE: &str = "9f2c41d07a5b83e6";

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
            std::env::temp_dir().join(format!("pwt-cli-p03-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn dir(&self) -> &str {
        self.root.to_str().unwrap()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.path(rel)).unwrap()
    }

    /// Replaces `workflows.json` by the fixture workflow at `revision`.
    fn seed_workflows(&self, revision: u64) {
        let text = std::fs::read_to_string(Path::new(FIXTURES).join("data-s9-workflows.json"))
            .expect("the persistence fixture exists");
        let mut doc: Value = serde_json::from_str(&text).unwrap();
        doc["workflows"][0]["revision"] = json!(revision);
        let path = self.path("workflows.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// Every regular file under the directory, relative, sorted.
    fn files(&self) -> Vec<String> {
        let mut found = Vec::new();
        walk(&self.root, &self.root, &mut found);
        found.sort();
        found
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn walk(base: &Path, dir: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(base, &path, found);
        } else {
            found.push(path.strip_prefix(base).unwrap().display().to_string());
        }
    }
}

/// The result of one invocation.
struct Run {
    /// `format!("{:?}", ExitCode)`; compared with the same rendering of the expected code.
    code: String,
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

    fn jsonl(&self) -> Vec<Value> {
        self.out()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).expect("every stdout line is JSON"))
            .collect()
    }

    /// The only record of the run.
    fn record(&self) -> Value {
        let records = self.jsonl();
        assert_eq!(records.len(), 1, "expected one record: {}", self.out());
        records.into_iter().next().unwrap()
    }

    fn is(&self, code: u8) -> bool {
        self.code == format!("{:?}", ExitCode::from(code))
    }
}

/// Asserts the exit code, with the (key-free) standard error as the message.
#[track_caller]
fn assert_exit(run: &Run, code: u8) {
    assert!(
        run.is(code),
        "expected exit {code}, got {}; stderr: {}",
        run.code,
        run.err()
    );
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    std::iter::once("pwt-workbench-cli")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect()
}

/// One in-process invocation within `session`, exactly once.
fn go_once(session: &mut Session, args: &[&str], stdin: &[u8], tty: bool) -> Run {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::run_session(session, os_args(args), stdin, tty, &mut stdout, &mut stderr);
    Run {
        code: format!("{code:?}"),
        stdout,
        stderr,
    }
}

/// One in-process invocation within `session`, re-run while it exits 3 (lock held), at most 10 s, 20 ms
/// apart; returns the first run that exits otherwise.
///
/// Sibling tests run in parallel threads of this process and spawn the binary. Between `fork` and `exec` a
/// child holds a copy of every descriptor of the test process (`O_CLOEXEC` descriptors are closed only by
/// `exec`), and a `flock` lives until the last descriptor of its open file description is closed. A command
/// started in that window after another command of the same data directory therefore sees the lock held
/// and correctly exits 3. Only exit 3 is retried, and only for commands whose input can be read again (the
/// input is a byte slice). The product is not affected: it never spawns a process while holding the lock.
/// Tests that expect exit 3 use [`go_once`].
fn go(session: &mut Session, args: &[&str], stdin: &[u8], tty: bool) -> Run {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let run = go_once(session, args, stdin, tty);
        if !run.is(3) || std::time::Instant::now() >= deadline {
            return run;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// One in-process invocation in a fresh session with an empty host environment and no input.
fn run(args: &[&str]) -> Run {
    go(&mut fresh(), args, b"", false)
}

fn fresh() -> Session {
    Session::new(HostEnv::empty())
}

/// `<args> --data-dir <tmp>`.
fn with_dir(tmp: &Tmp, args: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    all.push("--data-dir".to_owned());
    all.push(tmp.dir().to_owned());
    all
}

fn in_dir(session: &mut Session, tmp: &Tmp, args: &[&str], stdin: &[u8]) -> Run {
    let owned = with_dir(tmp, args);
    let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    go(session, &refs, stdin, false)
}

/// `<args> --data-dir <tmp>` in a fresh session.
fn dir_run(tmp: &Tmp, args: &[&str]) -> Run {
    in_dir(&mut fresh(), tmp, args, b"")
}

fn init(tmp: &Tmp) {
    let run = dir_run(tmp, &["store", "init"]);
    assert_exit(&run, 0);
}

/// A network request: `git_acquire`, one repository, one destination, one category. `op` makes requests
/// distinct (the operation name is part of the fingerprint).
fn net(op: &str) -> Value {
    json!({
        "capability": "git_acquire",
        "operation": op,
        "arguments": [{"name": "branch", "value": "main"}],
        "repo_ids": ["00000000-0000-4000-8000-000000000101"],
        "destinations": ["https://github.com/example/repo"],
        "resource_constraints": ["max_size_mb=100"],
        "data": ["repository_metadata"],
        "lifetime": "once"
    })
}

/// A request bound to the fixture workflow at `revision`, with the `workflow_revision` lifetime.
fn bound(op: &str, revision: u32) -> Value {
    let mut request = net(op);
    request["workflow"] = json!({"workflow_id": FIXTURE_WORKFLOW, "revision": revision});
    request["lifetime"] = json!("workflow_revision");
    request
}

/// `dev permission-check --json` of `request` in a session.
fn check_in(session: &mut Session, tmp: &Tmp, request: &Value) -> Run {
    check_bytes_in(session, tmp, &serde_json::to_vec(request).unwrap())
}

fn check_bytes_in(session: &mut Session, tmp: &Tmp, doc: &[u8]) -> Run {
    in_dir(session, tmp, &["--json", "dev", "permission-check"], doc)
}

fn check(tmp: &Tmp, request: &Value) -> Run {
    check_in(&mut fresh(), tmp, request)
}

/// The approval ID of a one-record `permission-check --json` result.
fn pending_id(run: &Run) -> String {
    assert_exit(run, 6);
    let record = run.record();
    assert_eq!(record["outcome"], "needs_approval");
    record["approval_id"].as_str().unwrap().to_owned()
}

/// `approvals list --json` as records.
fn list_json(tmp: &Tmp) -> Vec<Value> {
    let run = dir_run(tmp, &["--json", "approvals", "list"]);
    assert_exit(&run, 0);
    run.jsonl()
}

fn approval<'a>(list: &'a [Value], id: &str) -> &'a Value {
    list.iter()
        .find(|a| a["approval_id"] == id)
        .expect("the approval is listed")
}

/// Runs the binary in a cleared environment (plus `env`), feeding `stdin`.
fn bin(args: &[&str], stdin: &[u8], env: &[(&str, &str)]) -> Run {
    let mut child = Command::new(BIN)
        .args(args)
        .env_clear()
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin)
        .expect("the binary reads its input");
    let output = child.wait_with_output().expect("the binary finishes");
    Run {
        code: format!(
            "{:?}",
            ExitCode::from(u8::try_from(output.status.code().expect("an exit code")).unwrap())
        ),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

/// A reader that counts the bytes it hands out.
struct Counting<'a> {
    data: &'a [u8],
    read: usize,
}

impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.data.len().min(buf.len());
        buf[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        self.read += n;
        Ok(n)
    }
}

/// A writer that always fails, standing for a closed pipe.
struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

fn key_input() -> Vec<u8> {
    format!("{KEY}\n").into_bytes()
}

fn status_of(session: &mut Session) -> String {
    let run = go(session, &["--json", "credential", "status"], b"", false);
    assert_exit(&run, 0);
    run.record()["status"].as_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------------------------------
// approvals

/// Fails if `approvals list` on a store without approvals prints anything but the empty notice in human
/// mode, prints any record in JSON mode, fails, or creates a file (it is read-only).
#[test]
fn approvals_list_on_an_empty_store_prints_nothing() {
    let tmp = Tmp::new("list-empty");
    // No store at all: the data directory exists but holds no approvals.json.
    let none = dir_run(&tmp, &["approvals", "list"]);
    assert_exit(&none, 0);
    assert_eq!(none.out(), "no approvals\n");
    assert!(none.stderr.is_empty());
    init(&tmp);
    let before = tmp.files();
    let human = dir_run(&tmp, &["approvals", "list"]);
    assert_exit(&human, 0);
    assert_eq!(human.out(), "no approvals\n");
    let json_run = dir_run(&tmp, &["--json", "approvals", "list"]);
    assert_exit(&json_run, 0);
    assert!(json_run.stdout.is_empty());
    assert_eq!(tmp.files(), before);
}

/// Fails if a once grant is not 6 (pending recorded, ID printed) -> approve -> 0 (consumed) -> 6 again with
/// a new pending request, if the pending ID changes between identical requests, if approve does not move the
/// record to `approved` with the CLI actor, or if the second authorisation reuses the consumed grant.
#[test]
fn permission_check_once_grant_goes_6_approve_0_then_6_again() {
    let tmp = Tmp::new("once");
    init(&tmp);
    let request = net("git.clone");

    let first = check(&tmp, &request);
    let id = pending_id(&first);
    assert_eq!(first.record()["reason"], "no_grant");
    assert_eq!(first.err(), "error: approval required\n");
    // The same request again finds the same pending record.
    let again = check(&tmp, &request);
    assert_eq!(pending_id(&again), id);
    assert_eq!(again.record()["reason"], "already_pending");
    let listed = list_json(&tmp);
    assert_eq!(listed.len(), 1);
    assert_eq!(approval(&listed, &id)["status"], "pending");
    assert_eq!(approval(&listed, &id)["actor"], "requester");
    assert_eq!(
        approval(&listed, &id)["purpose"],
        "developer permission check"
    );

    let approve = dir_run(&tmp, &["approvals", "approve", &id]);
    assert_exit(&approve, 0);
    assert_eq!(approve.out(), format!("approval {id}: approved (once)\n"));
    let listed = list_json(&tmp);
    let record = approval(&listed, &id);
    assert_eq!(record["status"], "approved");
    assert_eq!(record["lifetime"], "once");
    assert_eq!(record["actor"], "local-user/cli");
    assert_eq!(record["consumed"], false);

    let permitted = check(&tmp, &request);
    assert_exit(&permitted, 0);
    let result = permitted.record();
    assert_eq!(result["outcome"], "permitted");
    assert_eq!(result["approval_id"], id.as_str());
    assert_eq!(result["lifetime"], "once");
    assert_eq!(result["consumed"], true);
    assert_eq!(approval(&list_json(&tmp), &id)["consumed"], true);

    // The grant is spent: asking again needs a new decision, and a new pending request is recorded.
    let spent = check(&tmp, &request);
    let new_id = pending_id(&spent);
    assert_ne!(new_id, id);
    let result = spent.record();
    assert_eq!(result["reason"], "mismatch");
    assert_eq!(result["dimension"], "consumption");
    assert_eq!(list_json(&tmp).len(), 2);
}

/// Fails if the human form of `permission-check` changes: the permitted line (with "consumed" for a once
/// grant) and the approval-required line (the pending ID and the reason) on standard output, the
/// one-line explanation on standard error.
#[test]
fn permission_check_human_output() {
    let tmp = Tmp::new("human");
    init(&tmp);
    let request = serde_json::to_vec(&net("git.clone")).unwrap();
    let mut session = fresh();
    let needs = in_dir(&mut session, &tmp, &["dev", "permission-check"], &request);
    assert_exit(&needs, 6);
    let out = needs.out();
    let id = out
        .trim_end()
        .strip_prefix("approval required: pending request ")
        .and_then(|rest| rest.strip_suffix(" (reason: no_grant)"))
        .expect("the approval-required line")
        .to_owned();
    assert_eq!(needs.err(), "error: approval required\n");
    assert_exit(&dir_run(&tmp, &["approvals", "approve", &id]), 0);
    let permitted = in_dir(&mut session, &tmp, &["dev", "permission-check"], &request);
    assert_exit(&permitted, 0);
    assert_eq!(
        permitted.out(),
        format!("permitted: approval {id} (lifetime once, consumed)\n")
    );
    assert!(permitted.stderr.is_empty());
}

/// Fails if `list` prints a destination other than `NormalisedDestination::canonical()` for an https record
/// (host lower-cased, trailing dot removed, default port explicit), or prints the ssh store form (which
/// drops the path, D01 L1) as if it were canonical, in either mode.
#[test]
fn approvals_list_shows_canonical_destinations_and_labels_the_ssh_store_form() {
    let tmp = Tmp::new("list-dest");
    init(&tmp);
    let mut request = net("git.clone");
    request["destinations"] = json!([
        "HTTPS://GitHub.COM./Example/Repo",
        "ssh://Git.Example.ORG./srv/repo.git"
    ]);
    let id = pending_id(&check(&tmp, &request));

    let human = dir_run(&tmp, &["approvals", "list"]);
    assert_exit(&human, 0);
    let out = human.out();
    assert!(
        out.contains("  destination: https://github.com:443/Example/Repo [canonical]\n"),
        "{out}"
    );
    assert!(
        out.contains(
            "  destination: git.example.org:22 [store form (an ssh path is not recorded)]\n"
        ),
        "{out}"
    );
    assert!(!out.contains("GitHub.COM"));
    assert!(!out.contains("/srv/repo.git"));
    assert!(out.starts_with(&format!("{id} pending git_acquire lifetime=once\n")));

    let listed = list_json(&tmp);
    let destinations = &approval(&listed, &id)["destinations"];
    assert_eq!(
        destinations,
        &json!([
            {"form": "canonical", "value": "https://github.com:443/Example/Repo"},
            {"form": "store", "value": "git.example.org:22"}
        ])
    );
}

/// Fails if a workflow-scope grant is not honoured at the revision it was approved for (0, and not
/// consumed: asked twice), if a later revision still honours it, if a request at the new revision does not
/// need a new decision (6, mismatch on the workflow revision), or if a request for the old revision is not
/// refused as stale (5).
#[test]
fn workflow_scope_grant_holds_until_the_workflow_revision_changes() {
    let tmp = Tmp::new("workflow");
    init(&tmp);
    tmp.seed_workflows(1);
    let request = bound("workflow.fetch", 1);
    let id = pending_id(&check(&tmp, &request));

    let approve = dir_run(&tmp, &["approvals", "approve", &id, "--scope", "workflow"]);
    assert_exit(&approve, 0);
    assert_eq!(
        approve.out(),
        format!("approval {id}: approved (workflow scope)\n")
    );
    let listed = list_json(&tmp);
    let record = approval(&listed, &id);
    assert_eq!(record["lifetime"], "workflow_revision");
    assert_eq!(record["workflow_revision"], 1);
    assert_eq!(record["run_id"], Value::Null);

    for _ in 0..2 {
        let permitted = check(&tmp, &request);
        assert_exit(&permitted, 0);
        let result = permitted.record();
        assert_eq!(result["outcome"], "permitted");
        assert_eq!(result["lifetime"], "workflow_revision");
        assert_eq!(result["consumed"], false);
    }
    assert_eq!(approval(&list_json(&tmp), &id)["consumed"], false);

    // The workflow moves to revision 2.
    tmp.seed_workflows(2);
    let stale = check(&tmp, &request);
    assert_exit(&stale, 5);
    assert_eq!(stale.record()["outcome"], "refused");
    assert_eq!(stale.record()["reason"], "stale_workflow_revision");
    let renewed = check(&tmp, &bound("workflow.fetch", 2));
    let new_id = pending_id(&renewed);
    assert_ne!(new_id, id);
    assert_eq!(renewed.record()["reason"], "mismatch");
    assert_eq!(renewed.record()["dimension"], "workflow_revision");
    // The old grant is still listed, but it authorises nothing at revision 2.
    assert_eq!(approval(&list_json(&tmp), &id)["status"], "approved");
}

/// Fails if `--scope workflow` approves a request that has no workflow binding, or one whose revision is no
/// longer the saved one (both must be exit 5 with the fixed message, the record staying pending), or if
/// an explicit `--scope once` on a workflow-bound request does not grant lifetime `once`.
#[test]
fn workflow_scope_is_unavailable_without_a_saved_workflow_at_the_revision() {
    let tmp = Tmp::new("scope-unavailable");
    init(&tmp);
    tmp.seed_workflows(1);
    let unbound = pending_id(&check(&tmp, &net("git.clone")));
    let at_one = pending_id(&check(&tmp, &bound("workflow.fetch", 1)));

    let refused = dir_run(
        &tmp,
        &["approvals", "approve", &unbound, "--scope", "workflow"],
    );
    assert_exit(&refused, 5);
    assert!(refused.stdout.is_empty());
    assert_eq!(
        refused.err(),
        "error: workflow scope needs a saved workflow at its current revision\n"
    );
    assert_eq!(approval(&list_json(&tmp), &unbound)["status"], "pending");

    tmp.seed_workflows(2);
    let stale = dir_run(
        &tmp,
        &["approvals", "approve", &at_one, "--scope", "workflow"],
    );
    assert_exit(&stale, 5);
    assert_eq!(approval(&list_json(&tmp), &at_one)["status"], "pending");

    // Explicit `once` works like the default.
    let once = dir_run(&tmp, &["approvals", "approve", &at_one, "--scope", "once"]);
    assert_exit(&once, 0);
    assert_eq!(
        approval(&list_json(&tmp), &at_one)["lifetime"],
        "once",
        "a once decision on a workflow request grants once"
    );
}

/// Fails if `deny` does not mark the record denied, if a decided approval can be decided again, if the
/// identical request is not then refused as previously denied (5, with the denied ID), or if the denial
/// text differs.
#[test]
fn deny_blocks_the_identical_request_and_a_decided_approval_cannot_be_decided_again() {
    let tmp = Tmp::new("deny");
    init(&tmp);
    let request = net("git.clone");
    let id = pending_id(&check(&tmp, &request));

    let deny = dir_run(&tmp, &["approvals", "deny", &id]);
    assert_exit(&deny, 0);
    assert_eq!(deny.out(), format!("approval {id}: denied\n"));
    let listed = list_json(&tmp);
    assert_eq!(approval(&listed, &id)["status"], "denied");
    assert_eq!(approval(&listed, &id)["actor"], "local-user/cli");

    for args in [
        &["approvals", "approve", id.as_str()][..],
        &["approvals", "deny", id.as_str()],
    ] {
        let again = dir_run(&tmp, args);
        assert_exit(&again, 5);
        assert!(again.stdout.is_empty());
        assert_eq!(again.err(), "error: the approval is not pending\n");
    }
    let blocked = check(&tmp, &request);
    assert_exit(&blocked, 5);
    let record = blocked.record();
    assert_eq!(record["outcome"], "refused");
    assert_eq!(record["reason"], "previously_denied");
    assert_eq!(record["approval_id"], id.as_str());
    assert_eq!(list_json(&tmp).len(), 1, "a refusal records nothing");
}

/// Fails if `revoke` does not end a grant (the next check must need approval again), if a pending, denied
/// or already revoked record can be revoked (5, fixed message), or if the revoker is not recorded.
#[test]
fn revoke_ends_a_grant_and_only_an_approved_grant_can_be_revoked() {
    let tmp = Tmp::new("revoke");
    init(&tmp);
    tmp.seed_workflows(1);
    let request = bound("workflow.fetch", 1);
    let id = pending_id(&check(&tmp, &request));

    let pending = dir_run(&tmp, &["approvals", "revoke", &id]);
    assert_exit(&pending, 5);
    assert_eq!(
        pending.err(),
        "error: only an approved grant can be revoked\n"
    );

    assert_exit(
        &dir_run(&tmp, &["approvals", "approve", &id, "--scope", "workflow"]),
        0,
    );
    assert_exit(&check(&tmp, &request), 0);

    let revoke = dir_run(&tmp, &["approvals", "revoke", &id]);
    assert_exit(&revoke, 0);
    assert_eq!(revoke.out(), format!("approval {id}: revoked\n"));
    let listed = list_json(&tmp);
    assert_eq!(approval(&listed, &id)["status"], "revoked");
    assert_eq!(approval(&listed, &id)["actor"], "local-user/cli");
    assert_exit(&dir_run(&tmp, &["approvals", "revoke", &id]), 5);

    // A revoked grant authorises nothing: a new pending request is recorded.
    let after = check(&tmp, &request);
    assert_ne!(pending_id(&after), id);
    assert_eq!(approval(&list_json(&tmp), &id)["status"], "revoked");
}

/// Fails if an unknown approval ID (a well-formed one) is anything but exit 5 with the fixed message for
/// approve, deny and revoke, or if it records an approval.
#[test]
fn unknown_approval_ids_exit_5() {
    let tmp = Tmp::new("not-found");
    init(&tmp);
    let id = "00000000-0000-4000-8000-0000000000aa";
    for sub in ["approve", "deny", "revoke"] {
        let run = dir_run(&tmp, &["approvals", sub, id]);
        assert_exit(&run, 5);
        assert!(run.stdout.is_empty(), "approvals {sub} printed to stdout");
        assert_eq!(run.err(), "error: no approval with that identifier\n");
    }
    assert!(list_json(&tmp).is_empty());
}

/// Fails if a store failure of an approvals or permission-check command is not classified like the store
/// commands: a missing or non-private data directory, and a malformed `approvals.json`, exit 5 with fixed
/// text; a store locked by another holder exits 3 for every approvals command and for permission-check; no
/// failure text echoes the path.
#[test]
fn approval_commands_report_store_failures_with_the_store_exit_codes() {
    let tmp = Tmp::new("failures");
    init(&tmp);
    let id = "00000000-0000-4000-8000-0000000000aa";
    let request = serde_json::to_vec(&net("git.clone")).unwrap();
    let all: [(&[&str], &[u8]); 5] = [
        (&["approvals", "list"], b""),
        (&["approvals", "approve", id], b""),
        (&["approvals", "deny", id], b""),
        (&["approvals", "revoke", id], b""),
        (&["dev", "permission-check"], &request),
    ];

    // Missing directory.
    let missing = tmp.path("no-such-dir");
    for (args, stdin) in all {
        let mut argv: Vec<&str> = args.to_vec();
        argv.extend(["--data-dir", missing.to_str().unwrap()]);
        let run = go(&mut fresh(), &argv, stdin, false);
        assert_exit(&run, 5);
        assert!(run.err().contains("the data directory was rejected"));
        assert!(!run.err().contains(tmp.dir()));
    }

    // Group- and world-accessible directory.
    let open = Tmp::new("failures-open");
    std::fs::set_permissions(&open.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    for (args, stdin) in all {
        let run = in_dir(&mut fresh(), &open, args, stdin);
        assert_exit(&run, 5);
        assert!(run.err().contains("the data directory was rejected"));
    }

    // Malformed ledger: fails closed, file untouched.
    let bad = Tmp::new("failures-malformed");
    init(&bad);
    std::fs::write(bad.path("approvals.json"), b"{ not json").unwrap();
    std::fs::set_permissions(
        bad.path("approvals.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    for (args, stdin) in all {
        let run = in_dir(&mut fresh(), &bad, args, stdin);
        assert_exit(&run, 5);
        assert_eq!(
            run.err(),
            "error: approval storage failed: the existing store file is malformed (root approvals)\n",
            "{args:?}"
        );
    }
    assert_eq!(bad.read("approvals.json"), b"{ not json");

    // Lock held by another holder.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path(".app.lock"))
        .unwrap();
    held.try_lock().expect("the test takes the lock first");
    for (args, stdin) in all {
        let owned = with_dir(&tmp, args);
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let run = go_once(&mut fresh(), &refs, stdin, false);
        assert_exit(&run, 3);
        assert!(run.err().contains("locked by another instance"), "{args:?}");
        assert!(!run.err().contains(tmp.dir()));
    }
    drop(held);
}

/// Fails if a usage mistake of the new commands is not exit 2 with an empty stdout and a message that
/// repeats none of the input: a malformed or missing approval ID, an invalid scope, `--scope` on deny, a
/// missing data directory, an unknown subcommand or group, flags of other commands, extra words.
#[test]
fn approval_and_dev_usage_errors_exit_2() {
    let tmp = Tmp::new("usage");
    let d = tmp.dir();
    let id = "00000000-0000-4000-8000-0000000000aa";
    let cases: Vec<Vec<&str>> = vec![
        vec!["approvals"],
        vec!["approvals", "bogus-sub-zz", "--data-dir", d],
        vec!["approvals", "list"],
        vec!["approvals", "list", "extra-word-zz", "--data-dir", d],
        vec!["approvals", "approve", "--data-dir", d],
        vec!["approvals", "approve", "not-an-id-zz", "--data-dir", d],
        vec![
            "approvals",
            "approve",
            "A0000000-0000-4000-8000-0000000000AA",
            "--data-dir",
            d,
        ],
        vec!["approvals", "approve", id, id, "--data-dir", d],
        vec![
            "approvals",
            "approve",
            id,
            "--data-dir",
            d,
            "--scope",
            "forever-zz",
        ],
        vec!["approvals", "approve", id, "--data-dir", d, "--scope"],
        vec![
            "approvals",
            "approve",
            id,
            "--data-dir",
            d,
            "--scope",
            "once",
            "--scope",
            "once",
        ],
        vec!["approvals", "deny", id, "--data-dir", d, "--scope", "once"],
        vec![
            "approvals",
            "revoke",
            id,
            "--data-dir",
            d,
            "--scope",
            "once",
        ],
        vec!["approvals", "list", "--data-dir", d, "--session"],
        vec!["approvals", "list", "--data-dir", d, "--root", "log"],
        vec!["approvals", "approve", id],
        vec!["dev"],
        vec!["dev", "bogus-sub-zz", "--data-dir", d],
        vec!["dev", "permission-check"],
        vec!["dev", "permission-check", "extra-word-zz", "--data-dir", d],
        vec![
            "dev",
            "permission-check",
            "--data-dir",
            d,
            "--scope",
            "once",
        ],
        vec!["dev", "env-preview", "--data-dir", d],
        vec!["dev", "env-preview", "no-such-tool-zz", "--data-dir", d],
        vec!["dev", "env-preview", "git"],
        vec!["dev", "env-preview", "git", "yamllint", "--data-dir", d],
        vec!["dev", "env-preview", "git", "--data-dir", d, "--session"],
        vec!["store", "init", "--data-dir", d, "--scope", "once"],
        vec!["features", "--session"],
        vec!["--version", "--scope", "once"],
    ];
    for args in cases {
        let run = run_args(&args);
        assert_exit(&run, 2);
        assert!(run.stdout.is_empty(), "{args:?} printed to stdout");
        let err = run.err();
        assert!(err.contains("usage:"), "{args:?}: {err}");
        for word in [
            "not-an-id-zz",
            "forever-zz",
            "no-such-tool-zz",
            "bogus-sub-zz",
            "extra-word-zz",
            id,
            d,
        ] {
            assert!(!err.contains(word), "{args:?} echoes `{word}`");
        }
    }
    assert!(
        tmp.files().is_empty(),
        "usage errors must not touch the directory"
    );
}

fn run_args(args: &[&str]) -> Run {
    go(&mut fresh(), args, b"", false)
}

// ---------------------------------------------------------------------------------------------------
// credential

/// Fails if `set` -> `status` -> `delete` in one session does not give `session_only` -> `session_only`
/// -> `absent`, if a second `set` does not replace the key, if a fresh session does not report `absent`
/// (the key is held for one session only), or if `set` fails to say that the key is held for this
/// invocation only.
#[test]
fn credential_lifecycle_in_one_session_and_absent_in_a_fresh_one() {
    let mut session = fresh();
    assert_eq!(status_of(&mut session), "absent");

    let set = go(
        &mut session,
        &["credential", "set", "--session"],
        &key_input(),
        false,
    );
    assert_exit(&set, 0);
    assert_eq!(
        set.out(),
        "credential: session_only\nthe key is held for this invocation only (process memory; never written to disk)\n"
    );
    assert!(set.stderr.is_empty());
    assert_eq!(status_of(&mut session), "session_only");
    let human = go(&mut session, &["credential", "status"], b"", false);
    assert_eq!(human.out(), "credential: session_only\n");

    // Replace.
    let second = format!("{KEY}-second\r\n");
    let replaced = go(
        &mut session,
        &["credential", "set", "--session"],
        second.as_bytes(),
        false,
    );
    assert_exit(&replaced, 0);
    assert_eq!(status_of(&mut session), "session_only");

    // Another session never sees it.
    assert_eq!(status_of(&mut fresh()), "absent");

    let delete = go(&mut session, &["credential", "delete"], b"", false);
    assert_exit(&delete, 0);
    assert_eq!(delete.out(), "credential: absent\n");
    assert_eq!(status_of(&mut session), "absent");
    // Deleting nothing is not an error.
    assert_exit(&go(&mut session, &["credential", "delete"], b"", false), 0);
    assert_eq!(status_of(&mut session), "absent");
}

/// Fails if `set` reads even one byte from a terminal, accepts it, or changes an existing key; if the
/// refusal is not exit 5 with the fixed message; or if the same key through a pipe is not read in full.
#[test]
fn credential_set_refuses_a_terminal_and_reads_nothing() {
    let mut session = fresh();
    let input = key_input();
    let mut reader = Counting {
        data: &input,
        read: 0,
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::run_session(
        &mut session,
        os_args(&["credential", "set", "--session"]),
        &mut reader,
        true,
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(5)));
    assert_eq!(reader.read, 0, "a terminal must not be read");
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "error: standard input is a terminal; pipe the key in (nothing was read)\n"
    );
    assert_eq!(status_of(&mut session), "absent");

    // An existing key survives the refusal.
    assert_exit(
        &go(
            &mut session,
            &["credential", "set", "--session"],
            &input,
            false,
        ),
        0,
    );
    let refused = go(
        &mut session,
        &["credential", "set", "--session"],
        &input,
        true,
    );
    assert_exit(&refused, 5);
    assert_eq!(status_of(&mut session), "session_only");

    // The same key through a pipe is read, and read in full.
    let mut piped = Counting {
        data: &input,
        read: 0,
    };
    let code = cli::run_session(
        &mut fresh(),
        os_args(&["credential", "set", "--session"]),
        &mut piped,
        false,
        std::io::sink(),
        std::io::sink(),
    );
    assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(0)));
    assert_eq!(piped.read, input.len());
}

/// Fails if any malformed key (empty, line break only, 15 bytes, 4 097 bytes, inner space, inner line
/// break, non-ASCII, control byte, NUL, a second line, invalid UTF-8) is accepted, is not exit 5 with the
/// fixed message for its class, leaves the status other than `absent`, or echoes any part of the input on
/// either stream.
#[test]
fn malformed_keys_are_refused_with_exit_5_and_never_echoed() {
    let long = format!("pwt-test-FAKE-{}", "x".repeat(4097 - 14));
    assert_eq!(long.len(), 4097);
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "empty",
            Vec::new(),
            "error: no key was provided on standard input",
        ),
        (
            "line break only",
            b"\n".to_vec(),
            "error: no key was provided on standard input",
        ),
        (
            "crlf only",
            b"\r\n".to_vec(),
            "error: no key was provided on standard input",
        ),
        (
            "15 bytes",
            b"pwt-test-FAKE-1".to_vec(),
            "error: the key is too short (16 to 4096 characters)",
        ),
        (
            "4097 bytes",
            long.clone().into_bytes(),
            "error: the key is too long (16 to 4096 characters)",
        ),
        (
            "4097 bytes and a line break",
            format!("{long}\n").into_bytes(),
            "error: the key is too long (16 to 4096 characters)",
        ),
        (
            "inner space",
            b"pwt-test-FAKE-inner space-1234".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "leading space",
            b" pwt-test-FAKE-0123456789".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "inner line break",
            b"pwt-test-FAKE-first\npwt-test-FAKE-second\n".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "non-ASCII",
            "pwt-test-FAKE-\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"
                .as_bytes()
                .to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "control byte",
            b"pwt-test-FAKE-\x07-0123456789".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "NUL",
            b"pwt-test-FAKE-\x00-0123456789".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
        (
            "invalid UTF-8",
            b"pwt-test-FAKE-\xff\xfe-0123456789".to_vec(),
            "error: the key must be printable ASCII without spaces or line breaks",
        ),
    ];
    for (name, input, message) in cases {
        let mut session = fresh();
        let run = go(
            &mut session,
            &["credential", "set", "--session"],
            &input,
            false,
        );
        assert_exit(&run, 5);
        assert!(run.stdout.is_empty(), "{name}: stdout");
        assert_eq!(run.err(), format!("{message}\n"), "{name}");
        assert!(!run.err().contains("FAKE"), "{name}: echoes input");
        assert_eq!(status_of(&mut session), "absent", "{name}");
    }
}

/// Fails if the accepted key forms regress: exactly 16 and exactly 4 096 bytes, with no line break, `\n`
/// or `\r\n`; and if a key longer than the read limit after a valid one is not rejected (the trailing
/// bytes must not be ignored).
#[test]
fn valid_key_forms_are_accepted() {
    let sixteen = "pwt-FAKE-0123456".to_owned();
    assert_eq!(sixteen.len(), 16);
    let max = format!("pwt-test-FAKE-{}", "y".repeat(4096 - 14));
    assert_eq!(max.len(), 4096);
    for (name, input) in [
        ("16 bytes, no line break", sixteen.clone()),
        ("16 bytes, LF", format!("{sixteen}\n")),
        ("16 bytes, CRLF", format!("{sixteen}\r\n")),
        ("4096 bytes", max.clone()),
        ("4096 bytes, LF", format!("{max}\n")),
        ("4096 bytes, CRLF", format!("{max}\r\n")),
    ] {
        let mut session = fresh();
        let run = go(
            &mut session,
            &["credential", "set", "--session"],
            input.as_bytes(),
            false,
        );
        assert_exit(&run, 0);
        assert_eq!(status_of(&mut session), "session_only", "{name}");
        assert!(!run.out().contains("FAKE"), "{name}: echoes the key");
    }
    let trailing = format!("{max}\n\n");
    let run = go(
        &mut fresh(),
        &["credential", "set", "--session"],
        trailing.as_bytes(),
        false,
    );
    assert_exit(&run, 5);
}

/// Fails if `credential set` without `--session`, a credential command with a data directory, `--session`
/// on status or delete, or extra words are accepted (exit 2, nothing read, nothing printed).
#[test]
fn credential_usage_errors_exit_2() {
    let tmp = Tmp::new("cred-usage");
    let cases: Vec<Vec<&str>> = vec![
        vec!["credential"],
        vec!["credential", "bogus-sub-zz"],
        vec!["credential", "set"],
        vec!["credential", "set", "--session", "extra-word-zz"],
        vec!["credential", "set", "--session", "--session"],
        vec!["credential", "set", "--session", "--data-dir", tmp.dir()],
        vec!["credential", "status", "--session"],
        vec!["credential", "delete", "--session"],
        vec!["credential", "status", "--data-dir", tmp.dir()],
        vec!["credential", "status", "extra-word-zz"],
        vec!["credential", "delete", "--scope", "once"],
    ];
    for args in cases {
        let input = key_input();
        let mut reader = Counting {
            data: &input,
            read: 0,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = cli::run_session(
            &mut fresh(),
            os_args(&args),
            &mut reader,
            false,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(
            format!("{code:?}"),
            format!("{:?}", ExitCode::from(2)),
            "{args:?}"
        );
        assert_eq!(reader.read, 0, "{args:?} read its input");
        assert!(stdout.is_empty(), "{args:?} printed to stdout");
        let err = String::from_utf8(stderr).unwrap();
        assert!(err.contains("usage:"), "{args:?}");
        assert!(!err.contains("extra-word-zz") && !err.contains("bogus-sub-zz"));
    }
}

/// Fails if the JSON form of the credential commands changes: one object with `mode`, `record` and
/// `status`; `set` also says `held` is "this invocation only"; no key material.
#[test]
fn credential_json_shapes() {
    let mut session = fresh();
    let set = go(
        &mut session,
        &["--json", "credential", "set", "--session"],
        &key_input(),
        false,
    );
    assert_exit(&set, 0);
    assert_eq!(
        set.record(),
        json!({"mode": "real", "record": "credential", "status": "session_only", "held": "this invocation only"})
    );
    let status = go(
        &mut session,
        &["--json", "credential", "status"],
        b"",
        false,
    );
    assert_eq!(
        status.record(),
        json!({"mode": "real", "record": "credential", "status": "session_only"})
    );
    let delete = go(
        &mut session,
        &["--json", "credential", "delete"],
        b"",
        false,
    );
    assert_eq!(
        delete.record(),
        json!({"mode": "real", "record": "credential", "status": "absent"})
    );
    for run in [&set, &status, &delete] {
        assert!(!run.out().contains(KEY_CORE));
    }
}

/// Fails if a key set in one process is visible to the next (it must be `absent`), if the binary does not
/// read the key from a pipe and say so, or if the key reaches the binary's output.
#[test]
fn binary_key_is_held_for_one_process_only() {
    let set = bin(&["credential", "set", "--session"], &key_input(), &[]);
    assert_exit(&set, 0);
    assert!(set.out().contains("held for this invocation only"));
    assert!(!set.out().contains(KEY_CORE) && !set.err().contains(KEY_CORE));
    let status = bin(&["credential", "status"], b"", &[]);
    assert_exit(&status, 0);
    assert_eq!(status.out(), "credential: absent\n");
    // The key given as an argument is not a thing: it is an unexpected word.
    let argument = bin(&["credential", "set", "--session", KEY], b"", &[]);
    assert_exit(&argument, 2);
    assert!(!argument.err().contains(KEY_CORE));
}

// ---------------------------------------------------------------------------------------------------
// seeded key sweep

/// Fails if the seeded key appears in any standard output, standard error or `Debug` text of the session,
/// or in any file under the data directory (every file is read, whatever its name), after a script that
/// sets the key, reads its status, previews every tool's environment with the key seeded in the host
/// environment (under innocent and key-like names, also embedded in longer values), checks a permission
/// request whose argument value is the key, lists, approves and re-checks, and deletes the key.
#[test]
fn seeded_key_appears_in_no_output_or_file() {
    let tmp = Tmp::new("sweep");
    let embedded = format!("prefix-{KEY}-suffix");
    let tz = format!("Etc/{KEY}");
    let host = HostEnv::from_pairs(
        [
            ("HOME", "/home/someone"),
            ("PATH", "/usr/local/bin"),
            ("INNOCENT_NOTE", KEY),
            ("MY_SETTING", embedded.as_str()),
            ("OPENROUTER_API_KEY", KEY),
            ("GITHUB_TOKEN", KEY),
            ("API_KEY", KEY),
            ("SECRET", KEY),
            ("AUTHORIZATION", KEY),
            ("TZ", tz.as_str()),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    );
    let mut session = Session::new(host);

    let mut streams: Vec<(String, Vec<u8>)> = Vec::new();
    let mut step = |session: &mut Session, args: &[&str], stdin: &[u8], expect: u8| {
        let run = go(session, args, stdin, false);
        assert_exit(&run, expect);
        streams.push((format!("stdout of {args:?}"), run.stdout.clone()));
        streams.push((format!("stderr of {args:?}"), run.stderr.clone()));
        run
    };

    let d = tmp.dir();
    step(&mut session, &["store", "init", "--data-dir", d], b"", 0);
    // Before the key: the host's TZ (a valid zone name that happens to contain the key's text) is
    // inherited; this proves the sweep below can see an inherited value at all.
    let before = step(
        &mut session,
        &["dev", "env-preview", "internal", "--data-dir", d],
        b"",
        0,
    );
    assert!(
        before.out().contains("  TZ [inherited] <masked>\n"),
        "the seeded TZ should be inherited before a key is set: {}",
        before.out()
    );
    step(
        &mut session,
        &["credential", "set", "--session"],
        &key_input(),
        0,
    );
    step(&mut session, &["credential", "status"], b"", 0);
    step(&mut session, &["--json", "credential", "status"], b"", 0);
    for tool in [
        "git",
        "yamllint",
        "gitleaks",
        "checkov",
        "actionlint",
        "kubeconform",
        "internal",
    ] {
        let human = step(
            &mut session,
            &["dev", "env-preview", tool, "--data-dir", d],
            b"",
            0,
        );
        assert!(
            !human.out().contains("TZ"),
            "{tool}: TZ holding the key was inherited"
        );
        step(
            &mut session,
            &["--json", "dev", "env-preview", tool, "--data-dir", d],
            b"",
            0,
        );
    }
    let mut request = net("git.clone");
    request["arguments"] = json!([{"name": "token-like", "value": KEY}]);
    let doc = serde_json::to_vec(&request).unwrap();
    let pending = step(
        &mut session,
        &["--json", "dev", "permission-check", "--data-dir", d],
        &doc,
        6,
    );
    let id = pending.record()["approval_id"].as_str().unwrap().to_owned();
    step(
        &mut session,
        &["approvals", "list", "--data-dir", d],
        b"",
        0,
    );
    step(
        &mut session,
        &["--json", "approvals", "approve", &id, "--data-dir", d],
        b"",
        0,
    );
    step(
        &mut session,
        &["--json", "dev", "permission-check", "--data-dir", d],
        &doc,
        0,
    );
    step(
        &mut session,
        &["--json", "approvals", "list", "--data-dir", d],
        b"",
        0,
    );
    step(&mut session, &["credential", "delete"], b"", 0);

    for (name, bytes) in &streams {
        assert!(!contains(bytes, KEY), "the key is in the {name}");
        assert!(
            !contains(bytes, KEY_CORE),
            "the key's core is in the {name}"
        );
    }
    let debug = format!("{session:?}");
    assert!(
        !debug.contains(KEY_CORE),
        "the key is in Session's Debug text"
    );

    let files = tmp.files();
    assert!(files.iter().any(|f| f == "approvals.json"));
    assert!(files.iter().any(|f| f == "log.json"));
    for file in files {
        let bytes = tmp.read(&file);
        assert!(
            !contains(&bytes, KEY),
            "the key is in the data-directory file {file}"
        );
        assert!(
            !contains(&bytes, KEY_CORE),
            "the key's core is in the data-directory file {file}"
        );
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

// ---------------------------------------------------------------------------------------------------
// dev env-preview

/// Fails if a profile's preview shows a value, omits a variable the profile sets, shows an origin other
/// than fixed / app-dir / inherited, lists a denied host variable, creates any file or directory, or does
/// not show the four tool directories under `<data-dir>/tools/<tool>/`. The expected variable sets are the
/// request §7 table, written out.
#[test]
fn env_preview_shows_names_and_origins_never_values() {
    let tmp = Tmp::new("env");
    let host = HostEnv::from_pairs(
        [
            ("HOME", "/home/host-value-zz"),
            ("PATH", "/host/path-zz"),
            ("TZ", "Europe/Berlin"),
            ("OPENROUTER_API_KEY", "pwt-test-FAKE-hostkey-zz"),
            ("INNOCENT", "innocent-value-zz"),
            ("LANG", "host-lang-zz"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    );
    let mut session = Session::new(host);
    let before = tmp.files();

    let expected: [(&str, &[(&str, &str)]); 7] = [
        (
            "git",
            &[
                ("GIT_ATTR_NOSYSTEM", "fixed"),
                ("GIT_CONFIG_GLOBAL", "fixed"),
                ("GIT_CONFIG_NOSYSTEM", "fixed"),
                ("GIT_TERMINAL_PROMPT", "fixed"),
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("XDG_CONFIG_HOME", "app-dir"),
            ],
        ),
        (
            "yamllint",
            &[
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
                ("XDG_CACHE_HOME", "app-dir"),
                ("XDG_CONFIG_HOME", "app-dir"),
            ],
        ),
        (
            "gitleaks",
            &[
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
            ],
        ),
        (
            "checkov",
            &[
                ("CKV_SKIP_PACKAGE_UPDATE_CHECK", "fixed"),
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
                ("XDG_CACHE_HOME", "app-dir"),
                ("XDG_CONFIG_HOME", "app-dir"),
            ],
        ),
        (
            "actionlint",
            &[
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
                ("XDG_CACHE_HOME", "app-dir"),
            ],
        ),
        (
            "kubeconform",
            &[
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
                ("XDG_CACHE_HOME", "app-dir"),
            ],
        ),
        (
            "internal",
            &[
                ("HOME", "app-dir"),
                ("LANG", "fixed"),
                ("PATH", "fixed"),
                ("TMPDIR", "app-dir"),
                ("TZ", "inherited"),
            ],
        ),
    ];
    for (tool, variables) in expected {
        let human = in_dir(&mut session, &tmp, &["dev", "env-preview", tool], b"");
        assert_exit(&human, 0);
        let out = human.out();
        let mut lines = out.lines();
        assert_eq!(lines.next(), Some(format!("tool: {tool}").as_str()));
        assert_eq!(lines.next(), Some("directories (derived, not created):"));
        for d in ["home", "config", "cache", "tmp"] {
            assert_eq!(
                lines.next(),
                Some(format!("  <data-dir>/tools/{tool}/{d}").as_str())
            );
        }
        assert_eq!(
            lines.next(),
            Some("environment (names and origins; values masked):")
        );
        let shown: Vec<String> = lines.map(str::to_owned).collect();
        let want: Vec<String> = variables
            .iter()
            .map(|(name, origin)| format!("  {name} [{origin}] <masked>"))
            .collect();
        assert_eq!(shown, want, "{tool}");
        for secret in [
            "host-value-zz",
            "host-path-zz",
            "pwt-test-FAKE-hostkey-zz",
            "innocent-value-zz",
            "host-lang-zz",
            "Berlin",
            tmp.dir(),
        ] {
            assert!(
                !out.contains(secret),
                "{tool}: the preview shows `{secret}`"
            );
        }
        assert!(human.stderr.is_empty());
    }
    assert_eq!(tmp.files(), before, "env-preview must not create anything");
    assert!(!tmp.path("tools").exists());
}

/// Fails if a host `TZ` that contains the session key is inherited (it must be dropped, while the same
/// value without a key set is kept), if a POSIX-form `TZ` is inherited, or if a key set in the same session
/// is not what the sanitiser checks.
#[test]
fn env_preview_drops_inherited_tz_holding_the_key() {
    let tmp = Tmp::new("env-tz");
    let tz = format!("Etc/{KEY}");
    let mut session = Session::new(HostEnv::from_pairs([(
        OsString::from("TZ"),
        OsString::from(tz),
    )]));
    let has_tz = |run: &Run| run.out().contains("  TZ [inherited] <masked>\n");

    let without_key = in_dir(&mut session, &tmp, &["dev", "env-preview", "internal"], b"");
    assert_exit(&without_key, 0);
    assert!(has_tz(&without_key), "{}", without_key.out());

    assert_exit(
        &go(
            &mut session,
            &["credential", "set", "--session"],
            &key_input(),
            false,
        ),
        0,
    );
    let with_key = in_dir(&mut session, &tmp, &["dev", "env-preview", "internal"], b"");
    assert_exit(&with_key, 0);
    assert!(!has_tz(&with_key));

    assert_exit(&go(&mut session, &["credential", "delete"], b"", false), 0);
    let deleted = in_dir(&mut session, &tmp, &["dev", "env-preview", "internal"], b"");
    assert!(has_tz(&deleted), "deleting the key must stop the filtering");

    let mut posix = Session::new(HostEnv::from_pairs([(
        OsString::from("TZ"),
        OsString::from("EST5EDT"),
    )]));
    let run = in_dir(&mut posix, &tmp, &["dev", "env-preview", "internal"], b"");
    assert!(!has_tz(&run), "a POSIX TZ form must be dropped");
}

/// Fails if a relative data directory (the tool directories must be absolute), a data directory that
/// contains `..`, or one whose name contains the session key is accepted (exit 5, fixed message, nothing
/// printed on stdout, and the key not echoed).
#[test]
fn env_preview_fails_closed_on_bad_directories() {
    let mut session = fresh();
    for dir in ["relative/dir", ".", "/abs/../escape"] {
        let run = go(
            &mut session,
            &["dev", "env-preview", "git", "--data-dir", dir],
            b"",
            false,
        );
        assert_exit(&run, 5);
        assert!(run.stdout.is_empty());
        assert_eq!(
            run.err(),
            "error: the data directory must be an absolute path without `..` for this command\n"
        );
        if dir.len() > 1 {
            assert!(!run.err().contains(dir));
        }
    }
    assert_exit(
        &go(
            &mut session,
            &["credential", "set", "--session"],
            &key_input(),
            false,
        ),
        0,
    );
    let keyed = format!("/tmp/{KEY}/data");
    let run = go(
        &mut session,
        &["dev", "env-preview", "git", "--data-dir", &keyed],
        b"",
        false,
    );
    assert_exit(&run, 5);
    assert!(run.stdout.is_empty());
    assert_eq!(
        run.err(),
        "error: a value of the environment would contain the session key; refused\n"
    );
    assert!(!run.err().contains(KEY_CORE));
}

/// Fails if the JSON form of `env-preview` changes: a header record (tool, the four directories relative to
/// `<data-dir>`, `directories_created` false, the variable count) and one record per variable with `name`,
/// `origin` and the masked `value`; no host value.
#[test]
fn env_preview_json_shape() {
    let tmp = Tmp::new("env-json");
    let host = HostEnv::from_pairs([(OsString::from("TZ"), OsString::from("Area/Zone"))]);
    let mut session = Session::new(host);
    let run = in_dir(
        &mut session,
        &tmp,
        &["--json", "dev", "env-preview", "internal"],
        b"",
    );
    assert_exit(&run, 0);
    let records = run.jsonl();
    assert_eq!(
        records[0],
        json!({
            "mode": "dev",
            "record": "env_preview",
            "tool": "internal",
            "directories": [
                "<data-dir>/tools/internal/home",
                "<data-dir>/tools/internal/config",
                "<data-dir>/tools/internal/cache",
                "<data-dir>/tools/internal/tmp"
            ],
            "directories_created": false,
            "variables": 5
        })
    );
    let vars: Vec<(String, String)> = records[1..]
        .iter()
        .map(|r| {
            assert_eq!(r["mode"], "dev");
            assert_eq!(r["record"], "env_var");
            assert_eq!(r["value"], "<masked>");
            (
                r["name"].as_str().unwrap().to_owned(),
                r["origin"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let want: Vec<(String, String)> = [
        ("HOME", "app-dir"),
        ("LANG", "fixed"),
        ("PATH", "fixed"),
        ("TMPDIR", "app-dir"),
        ("TZ", "inherited"),
    ]
    .iter()
    .map(|(n, o)| ((*n).to_owned(), (*o).to_owned()))
    .collect();
    assert_eq!(vars, want);
    assert!(!run.out().contains("Area/Zone"));
}

/// Fails if the binary does not use the process environment as the host snapshot (`TZ` must be inherited by
/// the internal profile) or lets a denied host variable through (`OPENROUTER_API_KEY`, `HTTPS_PROXY`).
#[test]
fn binary_env_preview_uses_the_process_environment() {
    let tmp = Tmp::new("env-bin");
    let run = bin(
        &["dev", "env-preview", "internal", "--data-dir", tmp.dir()],
        b"",
        &[
            ("TZ", "UTC"),
            ("OPENROUTER_API_KEY", "pwt-test-FAKE-binary-zz"),
            ("HTTPS_PROXY", "http://proxy.invalid:3128"),
        ],
    );
    assert_exit(&run, 0);
    let out = run.out();
    assert!(out.contains("  TZ [inherited] <masked>\n"), "{out}");
    assert!(!out.contains("OPENROUTER") && !out.contains("PROXY"));
    assert!(!out.contains("UTC") && !out.contains("binary-zz") && !out.contains("proxy.invalid"));
    assert!(!tmp.path("tools").exists());
}

// ---------------------------------------------------------------------------------------------------
// dev permission-check

/// Fails if a request that can never be granted is anything but exit 5 with its reason and nothing
/// recorded: raw secret values, a network capability without a destination, a local capability with one,
/// a deferred capability, a run lifetime without a run, `model_transmission` without an inference scope.
#[test]
fn refused_requests_exit_5_with_the_reason() {
    let tmp = Tmp::new("refused");
    init(&tmp);
    let mut raw = net("git.clone");
    raw["data"] = json!(["repository_metadata", "raw_secret_values"]);
    let mut no_destination = net("git.clone");
    no_destination["destinations"] = json!([]);
    let mut unexpected = net("render.local");
    unexpected["capability"] = json!("local_render");
    let mut deferred = net("docs.fetch");
    deferred["capability"] = json!("documentation_fetch");
    let mut deferred_embedding = net("embed.send");
    deferred_embedding["capability"] = json!("embedding_transmission");
    let mut deferred_verify = net("verify.live");
    deferred_verify["capability"] = json!("non_mutating_verification");
    let mut run_without_run = net("git.clone");
    run_without_run["lifetime"] = json!("run");
    let mut model = net("model.send");
    model["capability"] = json!("model_transmission");
    model["destinations"] = json!(["https://openrouter.ai/api"]);
    let mut workflow_lifetime = net("git.clone");
    workflow_lifetime["lifetime"] = json!("workflow_revision");
    let mut dup_args = net("git.clone");
    dup_args["arguments"] = json!([
        {"name": "branch", "value": "a"},
        {"name": "branch", "value": "b"}
    ]);
    let cases = [
        ("raw secret", raw, "raw_secret_requested"),
        ("no destination", no_destination, "missing_destination"),
        (
            "unexpected destination",
            unexpected,
            "unexpected_destination",
        ),
        ("documentation_fetch", deferred, "invalid_request"),
        (
            "embedding_transmission",
            deferred_embedding,
            "invalid_request",
        ),
        (
            "non_mutating_verification",
            deferred_verify,
            "invalid_request",
        ),
        (
            "run lifetime without a run",
            run_without_run,
            "lifetime_binding",
        ),
        (
            "workflow lifetime without a binding",
            workflow_lifetime,
            "lifetime_binding",
        ),
        ("model without a scope", model, "inference_scope"),
        ("duplicate argument names", dup_args, "invalid_request"),
    ];
    let before = tmp.read("approvals.json");
    for (name, request, reason) in cases {
        let run = check(&tmp, &request);
        assert_exit(&run, 5);
        let record = run.record();
        assert_eq!(record["outcome"], "refused", "{name}");
        assert_eq!(record["reason"], reason, "{name}");
        assert_eq!(run.err(), "error: the request was refused\n", "{name}");
        let human = in_dir(
            &mut fresh(),
            &tmp,
            &["dev", "permission-check"],
            &serde_json::to_vec(&request).unwrap(),
        );
        assert_exit(&human, 5);
        assert_eq!(human.out(), format!("refused: {reason}\n"), "{name}");
    }
    assert_eq!(
        tmp.read("approvals.json"),
        before,
        "a refusal records nothing"
    );
}

/// Fails if an invalid request document is anything but exit 5 with a fixed message and a `rejected`
/// record, or records anything: malformed JSON, empty input, an oversize document, trailing text, an
/// unknown member, an unknown capability, lifetime or data category, a malformed ID, a bad operation or
/// argument name, a bad destination (scheme, userinfo, port, wildcard, IPv6), a non-string argument value.
/// A document of exactly the size limit is read (and then rejected only on content).
#[test]
fn invalid_request_documents_exit_5_and_record_nothing() {
    let tmp = Tmp::new("invalid-docs");
    init(&tmp);
    let before = tmp.read("approvals.json");
    let with = |key: &str, value: Value| {
        let mut request = net("git.clone");
        request[key] = value;
        serde_json::to_vec(&request).unwrap()
    };
    let mut oversize = serde_json::to_vec(&net("git.clone")).unwrap();
    oversize.pop(); // drop the closing brace
    oversize.extend_from_slice(format!(",\"pad\":\"{}\"}}", "p".repeat(70_000)).as_bytes());
    assert!(oversize.len() > 65_536);

    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "malformed JSON",
            b"{ not json".to_vec(),
            "not_a_valid_request",
        ),
        ("empty input", Vec::new(), "not_a_valid_request"),
        ("not an object", b"[]".to_vec(), "not_a_valid_request"),
        ("oversize", oversize, "unreadable_or_too_large"),
        (
            "trailing text",
            [
                serde_json::to_vec(&net("git.clone")).unwrap(),
                b" trailing".to_vec(),
            ]
            .concat(),
            "not_a_valid_request",
        ),
        (
            "unknown member",
            with("surprise", json!(1)),
            "not_a_valid_request",
        ),
        (
            "unknown capability",
            with("capability", json!("run_anything")),
            "not_a_valid_request",
        ),
        (
            "unknown lifetime",
            with("lifetime", json!("forever")),
            "not_a_valid_request",
        ),
        (
            "unknown data category",
            with("data", json!(["secrets_please"])),
            "not_a_valid_request",
        ),
        (
            "malformed repo id",
            with("repo_ids", json!(["not-an-id"])),
            "not_a_valid_request",
        ),
        (
            "malformed run id",
            with("run_id", json!("nope")),
            "not_a_valid_request",
        ),
        (
            "non-string argument value",
            with("arguments", json!([{"name": "a", "value": 1}])),
            "not_a_valid_request",
        ),
        (
            "negative validity",
            with("validity_seconds", json!(-1)),
            "not_a_valid_request",
        ),
        (
            "bad operation name",
            with("operation", json!("Git Clone")),
            "invalid_name",
        ),
        (
            "empty operation name",
            with("operation", json!("")),
            "invalid_name",
        ),
        (
            "bad argument name",
            with("arguments", json!([{"name": "Branch!", "value": "x"}])),
            "invalid_name",
        ),
        (
            "destination without scheme",
            with("destinations", json!(["github.com/x"])),
            "invalid_destination",
        ),
        (
            "http destination",
            with("destinations", json!(["http://github.com/x"])),
            "invalid_destination",
        ),
        (
            "userinfo",
            with("destinations", json!(["https://user@github.com/x"])),
            "invalid_destination",
        ),
        (
            "port 0",
            with("destinations", json!(["https://github.com:0/x"])),
            "invalid_destination",
        ),
        (
            "wildcard",
            with("destinations", json!(["https://*.github.com/x"])),
            "invalid_destination",
        ),
        (
            "ipv6",
            with("destinations", json!(["https://[::1]/x"])),
            "invalid_destination",
        ),
        (
            "query",
            with("destinations", json!(["https://github.com/x?y=1"])),
            "invalid_destination",
        ),
    ];
    for (name, doc, reason) in cases {
        let run = check_bytes_in(&mut fresh(), &tmp, &doc);
        assert_exit(&run, 5);
        let record = run.record();
        assert_eq!(record["outcome"], "rejected", "{name}");
        assert_eq!(record["reason"], reason, "{name}");
        assert_eq!(
            run.err(),
            "error: the request document was rejected\n",
            "{name}"
        );
    }
    assert_eq!(
        tmp.read("approvals.json"),
        before,
        "a rejection records nothing"
    );

    // A terminal is never read.
    let input = serde_json::to_vec(&net("git.clone")).unwrap();
    let mut reader = Counting {
        data: &input,
        read: 0,
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let dir_args = with_dir(&tmp, &["--json", "dev", "permission-check"]);
    let refs: Vec<&str> = dir_args.iter().map(String::as_str).collect();
    let code = cli::run_session(
        &mut fresh(),
        os_args(&refs),
        &mut reader,
        true,
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(5)));
    assert_eq!(reader.read, 0);
    assert_eq!(tmp.read("approvals.json"), before);
}

/// Fails if the size bound is off by one: a document of exactly 65 536 bytes must reach the decoder (and
/// be accepted when valid), one byte more must be rejected as too large.
#[test]
fn request_size_limit_is_exact() {
    let tmp = Tmp::new("size-limit");
    init(&tmp);
    let base = serde_json::to_vec(&net("git.clone")).unwrap();
    let pad = |len: usize| {
        // Pad with insignificant whitespace before the closing brace.
        let mut doc = base.clone();
        let close = doc.pop().unwrap();
        assert_eq!(close, b'}');
        doc.extend(std::iter::repeat_n(b' ', len - doc.len() - 1));
        doc.push(b'}');
        assert_eq!(doc.len(), len);
        doc
    };
    let exact = check_bytes_in(&mut fresh(), &tmp, &pad(65_536));
    assert_exit(&exact, 6);
    assert_eq!(exact.record()["outcome"], "needs_approval");
    let over = check_bytes_in(&mut fresh(), &tmp, &pad(65_537));
    assert_exit(&over, 5);
    assert_eq!(over.record()["reason"], "unreadable_or_too_large");
    // An argument value near the bound fits the document bound and is fingerprinted (hashed, not stored).
    let mut large = net("git.clone-large");
    large["arguments"] = json!([{"name": "payload", "value": "x".repeat(60_000)}]);
    let run = check(&tmp, &large);
    assert_exit(&run, 6);
    assert!(
        !String::from_utf8(tmp.read("approvals.json"))
            .unwrap()
            .contains("xxxxxxxx"),
        "an argument value reached the store"
    );
}

/// Fails if the pending record of a permission check does not carry the fixed purpose and requester text,
/// the canonical destination, the request's categories and constraints, or if its validity (when the
/// request has one) is lost; and if an unreadable input stream is not reported as an unreadable document.
#[test]
fn permission_check_records_a_well_formed_pending_request() {
    let tmp = Tmp::new("record");
    init(&tmp);
    let mut request = net("git.clone");
    request["validity_seconds"] = json!(3600);
    request["destinations"] = json!(["https://GitHub.com/example/repo"]);
    let id = pending_id(&check(&tmp, &request));
    let listed = list_json(&tmp);
    let record = approval(&listed, &id);
    assert_eq!(record["status"], "pending");
    assert_eq!(record["capability"], "git_acquire");
    assert_eq!(record["purpose"], "developer permission check");
    assert_eq!(record["actor"], "requester");
    assert_eq!(record["data_categories"], json!(["repository_metadata"]));
    assert_eq!(record["resource_constraints"], json!(["max_size_mb=100"]));
    assert_eq!(
        record["repo_ids"],
        json!(["00000000-0000-4000-8000-000000000101"])
    );
    assert_eq!(
        record["destinations"],
        json!([{"form": "canonical", "value": "https://github.com:443/example/repo"}])
    );
    assert!(
        record["expires_at"].is_string(),
        "the validity must survive until the decision"
    );
    assert_eq!(record["decided_at"], Value::Null);
    assert_eq!(record["consumed"], false);
    // Argument values are never stored.
    let stored = String::from_utf8(tmp.read("approvals.json")).unwrap();
    assert!(
        !stored.contains("\"main\""),
        "an argument value reached the store"
    );

    // The event log has the request.
    let log = String::from_utf8(tmp.read("log.json")).unwrap();
    assert!(log.contains("ApprovalRequested"));

    // A reader that fails is an unreadable document.
    struct Failing;
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    let dir_args = with_dir(&tmp, &["--json", "dev", "permission-check"]);
    let refs: Vec<&str> = dir_args.iter().map(String::as_str).collect();
    let mut stdout = Vec::new();
    let code = cli::run_session(
        &mut fresh(),
        os_args(&refs),
        Failing,
        false,
        &mut stdout,
        std::io::sink(),
    );
    assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(5)));
    let record: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(record["reason"], "unreadable_or_too_large");
}

/// Fails if the JSON shape of the permission-check result changes: `mode` "dev", `record`
/// "permission_check", `outcome`, and only the members that apply (permitted: `approval_id`, `lifetime`,
/// `consumed`; needs_approval: `approval_id`, `reason`, and `dimension` for a mismatch; refused / rejected:
/// `reason`).
#[test]
fn permission_check_json_shapes() {
    let tmp = Tmp::new("check-json");
    init(&tmp);
    let request = net("git.clone");
    let pending = check(&tmp, &request);
    let id = pending_id(&pending);
    assert_eq!(
        pending.record(),
        json!({"mode": "dev", "record": "permission_check", "outcome": "needs_approval",
               "approval_id": id, "reason": "no_grant"})
    );
    assert_exit(&dir_run(&tmp, &["approvals", "approve", &id]), 0);
    let permitted = check(&tmp, &request);
    assert_eq!(
        permitted.record(),
        json!({"mode": "dev", "record": "permission_check", "outcome": "permitted",
               "approval_id": id, "lifetime": "once", "consumed": true})
    );
    let mismatch = check(&tmp, &request);
    let new_id = pending_id(&mismatch);
    assert_eq!(
        mismatch.record(),
        json!({"mode": "dev", "record": "permission_check", "outcome": "needs_approval",
               "approval_id": new_id, "reason": "mismatch", "dimension": "consumption"})
    );
    let mut raw = net("git.clone");
    raw["data"] = json!(["raw_secret_values"]);
    assert_eq!(
        check(&tmp, &raw).record(),
        json!({"mode": "dev", "record": "permission_check", "outcome": "refused",
               "reason": "raw_secret_requested"})
    );
    assert_eq!(
        check_bytes_in(&mut fresh(), &tmp, b"{").record(),
        json!({"mode": "dev", "record": "permission_check", "outcome": "rejected",
               "reason": "not_a_valid_request"})
    );
}

/// Fails if the binary does not map the three outcomes of `dev permission-check` to exit codes 6, 0 and 5
/// (the exit-code contract of the process, not only of the library entry point), or if the second run of a
/// once grant is not 6 again.
#[test]
fn binary_permission_check_exit_codes() {
    let tmp = Tmp::new("bin-check");
    let store = bin(&["store", "init", "--data-dir", tmp.dir()], b"", &[]);
    assert_exit(&store, 0);
    let doc = serde_json::to_vec(&net("git.clone")).unwrap();
    let args = ["--json", "dev", "permission-check", "--data-dir", tmp.dir()];
    let first = bin(&args, &doc, &[]);
    assert_exit(&first, 6);
    let id = first.record()["approval_id"].as_str().unwrap().to_owned();
    assert_exit(
        &bin(
            &["approvals", "approve", &id, "--data-dir", tmp.dir()],
            b"",
            &[],
        ),
        0,
    );
    assert_exit(&bin(&args, &doc, &[]), 0);
    assert_exit(&bin(&args, &doc, &[]), 6);
    assert_exit(&bin(&args, b"{", &[]), 5);
}

// ---------------------------------------------------------------------------------------------------
// FR-33

/// Fails if a data directory refused by FR-33 (running as root against a directory not owned by root) is
/// not exit 5 with the fixed message for every approvals command, or if the message names a path or user.
///
/// The refusal itself is produced by `persistence` from the effective user ID, which a test running as an
/// ordinary user cannot change and the CLI has no seam to inject (the file-system operations are not a CLI
/// input). The end-to-end part therefore covers the CLI's own step: the mapping of
/// `DevError::RootForeignDataDir` inside an approval failure to its exit code and text. The refusal in
/// `DataDir::new` is covered by `persistence/tests/p03_fr33.rs` and `application/tests/p03_fr33.rs`.
#[test]
fn fr33_refusal_maps_to_exit_5_with_the_fixed_message() {
    use application::approvals::ApprovalError;
    use application::dev_store::DevError;
    let (exit, text) =
        cli::commands::approval_failure(&ApprovalError::Store(DevError::RootForeignDataDir));
    assert_eq!(exit.code(), 5);
    assert_eq!(
        text,
        "error: approval storage failed: refusing to use a data directory not owned by root while running as root"
    );
    // The neighbouring classes keep their own codes.
    assert_eq!(
        cli::commands::approval_failure(&ApprovalError::Store(DevError::Locked))
            .0
            .code(),
        3
    );
    assert_eq!(
        cli::commands::approval_failure(&ApprovalError::Store(DevError::Storage))
            .0
            .code(),
        4
    );
    assert_eq!(
        cli::commands::approval_failure(&ApprovalError::Store(DevError::Halted))
            .0
            .code(),
        4
    );
    assert_eq!(
        cli::commands::approval_failure(&ApprovalError::NotFound)
            .0
            .code(),
        5
    );
}

// ---------------------------------------------------------------------------------------------------
// help, features, output failures

/// Fails if the bare `help` stops listing the nine new commands (and keeps the P02 listing),
/// if `help approvals`, `help credential` and `help dev` do not select exactly their commands with their
/// usage lines, if `help dev permission-check` selects more than that one command, if `help dev
/// env-preview` omits the tool list, or if the exit legend still calls exit 6 reserved.
#[test]
fn help_lists_the_p03_commands() {
    let all = run(&["help"]);
    assert_exit(&all, 0);
    let out = all.out();
    assert!(out.contains("pwt-workbench-cli store init --data-dir <dir>"));
    assert!(out.contains("pwt-workbench-cli dev permission-check --data-dir <dir>"));
    assert!(out.contains("pwt-workbench-cli credential set --session"));
    assert!(
        out.contains("tools: git, yamllint, gitleaks, checkov, actionlint, kubeconform, internal")
    );
    let bare_json: Vec<String> = run(&["--json", "help"])
        .jsonl()
        .iter()
        .map(|r| r["command"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(bare_json.len(), 16, "bare help lists every command");
    assert!(bare_json.contains(&"approvals revoke".to_owned()));
    assert!(out.contains("6 approval required"));
    assert!(!out.contains("(reserved)"));

    for (topic, usages) in [
        (
            "approvals",
            vec![
                "pwt-workbench-cli approvals list --data-dir <dir>",
                "pwt-workbench-cli approvals approve <id> --data-dir <dir> [--scope once|workflow]",
                "pwt-workbench-cli approvals deny <id> --data-dir <dir>",
                "pwt-workbench-cli approvals revoke <id> --data-dir <dir>",
            ],
        ),
        (
            "credential",
            vec![
                "pwt-workbench-cli credential set --session",
                "pwt-workbench-cli credential status",
                "pwt-workbench-cli credential delete",
            ],
        ),
        (
            "dev",
            vec![
                "pwt-workbench-cli dev env-preview <tool> --data-dir <dir>",
                "pwt-workbench-cli dev permission-check --data-dir <dir>",
            ],
        ),
    ] {
        let human = run(&["help", topic]);
        assert_exit(&human, 0);
        let text = human.out();
        for usage in &usages {
            assert!(text.contains(usage), "help {topic} lacks `{usage}`");
        }
        assert!(
            !text.contains("pwt-workbench-cli store init"),
            "help {topic}"
        );
        let json_run = run(&["--json", "help", topic]);
        assert_exit(&json_run, 0);
        let commands: Vec<String> = json_run
            .jsonl()
            .iter()
            .map(|r| r["usage"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(commands, usages, "help {topic}");
    }
    let one = run(&["help", "dev", "permission-check"]);
    assert_exit(&one, 0);
    assert!(one.out().contains("dev permission-check"));
    assert!(!one.out().contains("dev env-preview"));
    assert!(!one.out().contains("exit codes:"));
    let env = run(&["help", "dev", "env-preview"]);
    assert!(
        env.out()
            .contains("tools: git, yamllint, gitleaks, checkov, actionlint, kubeconform, internal")
    );
    let kinds: Vec<(String, String)> = run(&["--json", "help", "dev"])
        .jsonl()
        .iter()
        .map(|r| {
            (
                r["command"].as_str().unwrap().to_owned(),
                r["kind"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("dev env-preview".to_owned(), "dev".to_owned()),
            ("dev permission-check".to_owned(), "dev".to_owned())
        ]
    );
}

/// Fails if the feature index lacks a row for a P03 command, or labels one with a mode other than `real`
/// (approvals, credential) or `dev` (env-preview, permission-check).
#[test]
fn features_list_every_p03_command_with_its_mode() {
    let run = run(&["--json", "features"]);
    assert_exit(&run, 0);
    let records = run.jsonl();
    for (command, mode) in [
        ("approvals list", "real"),
        ("approvals approve", "real"),
        ("approvals deny", "real"),
        ("approvals revoke", "real"),
        ("credential set", "real"),
        ("credential status", "real"),
        ("credential delete", "real"),
        ("dev env-preview", "dev"),
        ("dev permission-check", "dev"),
    ] {
        let rows: Vec<&Value> = records.iter().filter(|r| r["command"] == command).collect();
        assert!(!rows.is_empty(), "no feature row for {command}");
        for row in rows {
            assert_eq!(row["mode"], mode, "{command}");
            assert!(row["scenario"].as_str().unwrap().len() > 3);
        }
    }
}

/// Fails if a closed standard output makes a new command that would have exited 0 exit anything but 5, or
/// panics (the output of the new commands goes through the same write-failure handling as the P02 ones).
#[test]
fn a_closed_stdout_makes_the_new_commands_exit_5() {
    let tmp = Tmp::new("closed");
    init(&tmp);
    for args in [
        vec!["credential", "status"],
        vec!["credential", "delete"],
        vec!["approvals", "list", "--data-dir", tmp.dir()],
        vec!["dev", "env-preview", "git", "--data-dir", tmp.dir()],
    ] {
        let mut stderr = Vec::new();
        let code = cli::run_session(
            &mut fresh(),
            os_args(&args),
            std::io::empty(),
            false,
            Closed,
            &mut stderr,
        );
        // `approvals list` of an empty store prints "no approvals", which also fails to be written.
        assert_eq!(
            format!("{code:?}"),
            format!("{:?}", ExitCode::from(5)),
            "{args:?}"
        );
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("output could not be written")
        );
    }
}

// ---------------------------------------------------------------------------------------------------
// P03-D02 (A1 to A3, A6)

/// Rewrites `approvals.json` through `edit` (the file keeps its private mode).
fn edit_approvals(tmp: &Tmp, edit: impl FnOnce(&mut Value)) {
    let mut doc: Value = serde_json::from_slice(&tmp.read("approvals.json")).unwrap();
    edit(&mut doc);
    std::fs::write(
        tmp.path("approvals.json"),
        serde_json::to_vec_pretty(&doc).unwrap(),
    )
    .unwrap();
}

/// The text planted in the stored-text tests: a line break followed by a forged "approved" line, a carriage
/// return, a tab, an ESC sequence and a right-to-left override.
const PLANTED: &str = "max_size_mb=100\nFORGED 00000000-0000-4000-8000-0000000000ff approved git_acquire\r\tESC\u{1b}[2J\u{202e}end";

/// Fails if a stored multi-line constraint, purpose or actor forges lines in the human `approvals list`
/// (P03-D02 A1): the view of one approval must stay exactly its own lines, the line break, carriage
/// return and tab must be shown as `<U+000A>`, `<U+000D>` and `<U+0009>`, and ESC and U+202E as escapes;
/// the JSON form must stay one record on one line. Without the `one_line` step of the renderer a
/// `FORGED` line would start in column 0 and the line count would grow.
#[test]
fn planted_control_characters_render_on_one_visible_line() {
    let tmp = Tmp::new("planted");
    init(&tmp);
    let id = pending_id(&check(&tmp, &net("git.clone")));
    edit_approvals(&tmp, |doc| {
        let record = &mut doc["approvals"][0];
        record["resource_constraints"] = json!([PLANTED]);
        record["purpose"] = json!(PLANTED);
        record["actor"] = json!(PLANTED);
    });

    let human = dir_run(&tmp, &["approvals", "list"]);
    assert_exit(&human, 0);
    let out = human.out();
    let lines: Vec<&str> = out.lines().collect();
    // header, workflow, destination, data, constraint, requested, purpose
    assert_eq!(lines.len(), 7, "{out}");
    assert!(lines[0].starts_with(&format!("{id} pending")));
    assert!(
        lines[1..].iter().all(|l| l.starts_with("  ")),
        "a line starts outside the approval view: {out}"
    );
    assert!(!out.contains('\r') && !out.contains('\t') && !out.contains('\u{1b}'));
    assert!(!out.contains('\u{202e}'));
    let constraint = lines
        .iter()
        .find(|l| l.starts_with("  constraint: "))
        .expect("the constraint line");
    assert_eq!(
        *constraint,
        "  constraint: max_size_mb=100<U+000A>FORGED 00000000-0000-4000-8000-0000000000ff approved git_acquire<U+000D><U+0009>ESC<U+001B>[2J<U+202E>end"
    );
    assert!(lines[5].contains("actor: max_size_mb=100<U+000A>FORGED"));
    assert!(lines[6].starts_with("  purpose: max_size_mb=100<U+000A>FORGED"));

    let json_run = dir_run(&tmp, &["--json", "approvals", "list"]);
    assert_exit(&json_run, 0);
    assert_eq!(json_run.out().lines().count(), 1);
    assert_eq!(json_run.jsonl().len(), 1);
}

/// Fails if `dev permission-check` accepts a resource constraint that is not one line of plain text
/// (P03-D02 A1): a line break, carriage return, tab, ESC, DEL, a C1 control, a zero-width space or a
/// right-to-left override must each be exit 5 `not_a_valid_request` with nothing recorded; or if the
/// bounds are off by one (16 entries and 512 bytes are accepted, 17 entries and 513 bytes are not).
/// Without the rejection in `dev_permission::decode` the planted request would be recorded as pending.
#[test]
fn permission_check_rejects_constraints_that_are_not_plain_one_line_text() {
    let tmp = Tmp::new("constraints");
    init(&tmp);
    let before = tmp.read("approvals.json");
    for (name, constraint) in [
        ("line break", "a\nb"),
        ("carriage return", "a\rb"),
        ("tab", "a\tb"),
        ("ESC", "a\u{1b}[2Jb"),
        ("DEL", "a\u{7f}b"),
        ("NUL", "a\u{0}b"),
        ("C1 control", "a\u{85}b"),
        ("zero-width space", "a\u{200b}b"),
        ("right-to-left override", "a\u{202e}b"),
        ("line separator", "a\u{2028}b"),
    ] {
        let mut request = net("git.clone");
        request["resource_constraints"] = json!(["fine", constraint]);
        let run = check(&tmp, &request);
        assert_exit(&run, 5);
        let record = run.record();
        assert_eq!(record["outcome"], "rejected", "{name}");
        assert_eq!(record["reason"], "not_a_valid_request", "{name}");
        assert_eq!(tmp.read("approvals.json"), before, "{name}: recorded");
    }
    // Bounds.
    let mut many = net("git.clone");
    many["resource_constraints"] = json!((0..17).map(|i| format!("c{i}")).collect::<Vec<_>>());
    assert_exit(&check(&tmp, &many), 5);
    let mut long = net("git.clone");
    long["resource_constraints"] = json!(["x".repeat(513)]);
    assert_exit(&check(&tmp, &long), 5);
    assert_eq!(tmp.read("approvals.json"), before);

    let mut sixteen = net("git.clone");
    sixteen["resource_constraints"] = json!((0..16).map(|i| format!("c{i}")).collect::<Vec<_>>());
    assert_exit(&check(&tmp, &sixteen), 6);
    let mut widest = net("git.clone");
    widest["resource_constraints"] = json!(["x".repeat(512)]);
    assert_exit(&check(&tmp, &widest), 6);
    assert_eq!(list_json(&tmp).len(), 2);
}

/// A `run`-lifetime request for run `n`.
fn run_request(n: u32) -> Value {
    let mut request = net("run.step");
    request["lifetime"] = json!("run");
    request["run_id"] = json!(format!("00000000-0000-4000-8000-{:012x}", 0x300 + n));
    request
}

/// Fails if a `run` grant is consumed or applies to another run (P03-D02 A2): 6, approve (the grant keeps
/// lifetime `run`), then 0 and 0 again with `consumed` false; run 2 needs its own decision (6, mismatch on
/// the run).
#[test]
fn run_lifetime_grant_holds_for_its_run_only() {
    let tmp = Tmp::new("run-grant");
    init(&tmp);
    let id = pending_id(&check(&tmp, &run_request(1)));
    assert_exit(&dir_run(&tmp, &["approvals", "approve", &id]), 0);
    let listed = list_json(&tmp);
    assert_eq!(approval(&listed, &id)["lifetime"], "run");
    assert_eq!(
        approval(&listed, &id)["run_id"],
        "00000000-0000-4000-8000-000000000301"
    );
    for _ in 0..2 {
        let permitted = check(&tmp, &run_request(1));
        assert_exit(&permitted, 0);
        let record = permitted.record();
        assert_eq!(record["lifetime"], "run");
        assert_eq!(record["consumed"], false);
        assert_eq!(record["approval_id"], id.as_str());
    }
    assert_eq!(approval(&list_json(&tmp), &id)["consumed"], false);
    let other = check(&tmp, &run_request(2));
    let other_id = pending_id(&other);
    assert_ne!(other_id, id);
    assert_eq!(other.record()["reason"], "mismatch");
    assert_eq!(other.record()["dimension"], "run");
}

/// A `model_transmission` request for `model`.
fn model_request(model: &str) -> Value {
    json!({
        "capability": "model_transmission",
        "operation": "model.send",
        "arguments": [],
        "destinations": ["https://openrouter.ai/api/v1"],
        "data": ["redacted_source"],
        "lifetime": "once",
        "inference_scope": {
            "provider_id": "openrouter",
            "model_id": model,
            "routing_policy": {
                "data_collection": "deny",
                "zdr": true,
                "allow_fallbacks": false,
                "only": ["openrouter"]
            },
            "source_ids": ["01234567-89ab-4cde-8f01-23456789abcd"],
            "redaction_version": "2026.10-a"
        }
    })
}

/// Fails if a model-transmission grant is not tied to its inference scope (P03-D02 A2): 6, approve, then the
/// approved scope is permitted (0, consumed); and, in a second store, the same operation for a different
/// model is not permitted (6, a new pending request, mismatch on the inference scope).
///
/// The last assertion records an existing behaviour, not a wish: while the pending request for the other
/// model exists, a request for the approved scope is held back as `already_pending` (rules R7 and R8 ignore
/// the inference scope; fail-closed, nothing is authorised). That is the owner question P03-D01 B1/H3, pinned
/// at application level by `p03_hostile.rs`.
#[test]
fn model_transmission_grant_is_tied_to_its_inference_scope() {
    let tmp = Tmp::new("model");
    init(&tmp);
    let id = pending_id(&check(&tmp, &model_request("example/model")));
    assert_exit(&dir_run(&tmp, &["approvals", "approve", &id]), 0);
    let permitted = check(&tmp, &model_request("example/model"));
    assert_exit(&permitted, 0);
    assert_eq!(permitted.record()["approval_id"], id.as_str());
    assert_eq!(permitted.record()["lifetime"], "once");
    assert_eq!(permitted.record()["consumed"], true);

    let other_tmp = Tmp::new("model-other");
    init(&other_tmp);
    let id = pending_id(&check(&other_tmp, &model_request("example/model")));
    assert_exit(&dir_run(&other_tmp, &["approvals", "approve", &id]), 0);
    let other = check(&other_tmp, &model_request("example/other-model"));
    let other_id = pending_id(&other);
    assert_ne!(other_id, id);
    assert_eq!(other.record()["reason"], "mismatch");
    assert_eq!(other.record()["dimension"], "inference_scope");
    assert_eq!(
        approval(&list_json(&other_tmp), &other_id)["status"],
        "pending"
    );
    assert_eq!(approval(&list_json(&other_tmp), &id)["consumed"], false);
    let held_back = check(&other_tmp, &model_request("example/model"));
    assert_eq!(pending_id(&held_back), other_id);
    assert_eq!(held_back.record()["reason"], "already_pending");
}

/// Fails if `approvals list` hides or mis-renders the stored inference scope (P03-D02 A3): provider, model,
/// the three routing-policy values, the number of sources (not their identifiers) and the redaction
/// version, in the human line and in the exact JSON object; and `null` for an approval without a scope.
#[test]
fn approvals_list_shows_the_inference_scope() {
    let tmp = Tmp::new("scope-view");
    init(&tmp);
    let model_id = pending_id(&check(&tmp, &model_request("example/model")));
    let plain_id = pending_id(&check(&tmp, &net("git.clone")));

    let human = dir_run(&tmp, &["approvals", "list"]);
    assert_exit(&human, 0);
    let out = human.out();
    assert!(
        out.contains("  inference scope: provider openrouter; model example/model; data_collection=deny zdr=true allow_fallbacks=false; sources: 1; redaction 2026.10-a\n"),
        "{out}"
    );
    assert_eq!(out.matches("inference scope:").count(), 1);
    assert!(!out.contains("01234567-89ab-4cde-8f01-23456789abcd"));

    let listed = list_json(&tmp);
    assert_eq!(
        approval(&listed, &model_id)["inference_scope"],
        json!({
            "provider_id": "openrouter",
            "model_id": "example/model",
            "routing_policy": {
                "data_collection": "deny",
                "zdr": true,
                "allow_fallbacks": false,
                "only": ["openrouter"]
            },
            "source_count": 1,
            "redaction_version": "2026.10-a"
        })
    );
    assert_eq!(approval(&listed, &plain_id)["inference_scope"], Value::Null);
    assert!(
        !dir_run(&tmp, &["--json", "approvals", "list"])
            .out()
            .contains("01234567-89ab-4cde-8f01-23456789abcd")
    );
}

/// Fails if the JSON record of `approvals approve`, `deny` or `revoke` differs from the exact object
/// (P03-D02 A2): `mode` "real", `record`, `approval_id`, the `decision` (approve only) and the `state`.
#[test]
fn approval_decision_json_records_have_an_exact_shape() {
    let tmp = Tmp::new("decision-json");
    init(&tmp);
    tmp.seed_workflows(1);
    let once = pending_id(&check(&tmp, &net("op.once")));
    let scoped = pending_id(&check(&tmp, &bound("op.scoped", 1)));
    let denied = pending_id(&check(&tmp, &net("op.denied")));

    let approve = dir_run(&tmp, &["--json", "approvals", "approve", &once]);
    assert_exit(&approve, 0);
    assert_eq!(
        approve.record(),
        json!({"mode": "real", "record": "approval_resolved", "approval_id": once,
               "decision": "approve_once", "state": "approved"})
    );
    let scope = dir_run(
        &tmp,
        &[
            "--json",
            "approvals",
            "approve",
            &scoped,
            "--scope",
            "workflow",
        ],
    );
    assert_exit(&scope, 0);
    assert_eq!(
        scope.record(),
        json!({"mode": "real", "record": "approval_resolved", "approval_id": scoped,
               "decision": "approve_workflow_scope", "state": "approved"})
    );
    let deny = dir_run(&tmp, &["--json", "approvals", "deny", &denied]);
    assert_exit(&deny, 0);
    assert_eq!(
        deny.record(),
        json!({"mode": "real", "record": "approval_resolved", "approval_id": denied,
               "decision": "deny", "state": "denied"})
    );
    let revoke = dir_run(&tmp, &["--json", "approvals", "revoke", &once]);
    assert_exit(&revoke, 0);
    assert_eq!(
        revoke.record(),
        json!({"mode": "real", "record": "approval_revoked", "approval_id": once,
               "state": "revoked"})
    );
}

/// Fails if a closed standard output changes the exit code of `dev permission-check` in the unsafe
/// direction (P03-D02 A6). Today's behaviour, pinned here: approval required keeps exit 6 and the pending
/// request is recorded; a permitted result whose record could not be written becomes exit 5 with the
/// output-lost message, although the once grant was consumed before the output was written. That is the safe
/// direction: a caller never sees "permitted" (exit 0) without having been told, and the spent grant cannot
/// be used again.
#[test]
fn a_closed_stdout_keeps_permission_check_in_the_safe_direction() {
    let tmp = Tmp::new("closed-check");
    init(&tmp);
    let doc = serde_json::to_vec(&net("git.clone")).unwrap();
    let args = with_dir(&tmp, &["dev", "permission-check"]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let closed = |doc: &[u8]| {
        let mut stderr = Vec::new();
        let code = cli::run_session(
            &mut fresh(),
            os_args(&refs),
            doc,
            false,
            Closed,
            &mut stderr,
        );
        (format!("{code:?}"), String::from_utf8(stderr).unwrap())
    };

    let (code, err) = closed(&doc);
    assert_eq!(code, format!("{:?}", ExitCode::from(6)));
    assert!(err.contains("error: approval required"));
    assert!(!err.contains("output could not be written"));
    let listed = list_json(&tmp);
    assert_eq!(listed.len(), 1, "the pending request is recorded");
    let id = listed[0]["approval_id"].as_str().unwrap().to_owned();

    assert_exit(&dir_run(&tmp, &["approvals", "approve", &id]), 0);
    let (code, err) = closed(&doc);
    assert_eq!(code, format!("{:?}", ExitCode::from(5)));
    assert!(err.contains("output could not be written"));
    assert_eq!(
        approval(&list_json(&tmp), &id)["consumed"],
        true,
        "the grant was consumed before the output was written"
    );
    // The spent grant cannot be used again.
    assert_exit(&check(&tmp, &net("git.clone")), 6);
}

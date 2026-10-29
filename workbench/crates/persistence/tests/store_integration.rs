//! Lead B2 integration tests (P02 wave 2): `StoreWriter` (T10) driven by the real `SchemaValidator` (T08)
//! with the full semantic rule set (T09), over the T08 fixtures. T10's own tests use fake validators; these
//! tests show the pieces agree: every write the writer makes, including reconciliation, is accepted by the
//! production validator. All data is synthetic; every test uses its own directory under
//! `CARGO_TARGET_TMPDIR`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use domain::states::RunState;
use domain::store::values::Slug;
use domain::store::{AnyStore, StoreKind};
use persistence::fs::RealFs;
use persistence::read::{SchemaValidator, decode};
use persistence::validate::{DocumentValidator, Verdict};
use persistence::writer::StoreWriter;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/store");
const TS: &str = "2026-10-16T05:25:00Z";

struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("integration-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn data_dir(&self) -> persistence::paths::DataDir {
        persistence::paths::DataDir::new(&RealFs, &self.root).unwrap()
    }

    fn write_root(&self, name: &str, doc: &Value) {
        let path = self.root.join(name);
        std::fs::write(&path, serde_json::to_vec(doc).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn validator() -> Arc<dyn DocumentValidator> {
    Arc::new(SchemaValidator::new())
}

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn typed(kind: StoreKind, doc: &Value) -> AnyStore {
    decode(
        kind,
        &serde_json::to_vec(doc).unwrap(),
        &SchemaValidator::new(),
    )
    .unwrap_or_else(|e| panic!("{kind:?} fixture must be fully valid: {e:?}"))
}

fn assert_valid(kind: StoreKind, any: &AnyStore) {
    let v = serde_json::to_value(any).unwrap();
    assert_eq!(
        SchemaValidator::new().validate(kind, &v),
        Verdict::Valid,
        "{kind:?}"
    );
}

#[test]
fn init_with_the_real_validator_creates_roots_it_accepts() {
    let tmp = Tmp::new("init");
    let (writer, report) =
        StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator()).expect("init");
    assert_eq!(report.created.len(), 5);
    for kind in report.created {
        let doc = writer.read(kind).expect("read back");
        assert_eq!(doc.kind(), kind);
        assert_valid(kind, &doc);
    }
}

#[test]
fn every_writable_fixture_round_trips_through_replace_and_read() {
    let cases = [
        (StoreKind::Config, "config"),
        (StoreKind::Repositories, "repositories"),
        (StoreKind::Workflows, "workflows"),
        (StoreKind::Approvals, "approvals"),
        (StoreKind::Log, "log"),
        (StoreKind::RagIndex, "rag_index"),
        (StoreKind::Conversations, "conversations"),
        (StoreKind::ToolManifest, "tool_manifest"),
    ];
    for (kind, stem) in cases {
        for size in ["min", "max"] {
            let tmp = Tmp::new(&format!("replace-{stem}-{size}"));
            let (mut writer, _) =
                StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator()).expect("init");
            let doc = typed(kind, &fixture(&format!("{stem}.{size}.json")));
            writer
                .replace(&doc)
                .unwrap_or_else(|e| panic!("{stem}.{size}: replace {e:?}"));
            assert_eq!(writer.read(kind).expect("read"), doc, "{stem}.{size}");
            // A second writer on the same directory sees the same validated document.
            drop(writer);
            let (writer, rec) =
                StoreWriter::open(tmp.data_dir(), Arc::new(RealFs), validator()).expect("reopen");
            assert!(rec.malformed_roots.is_empty(), "{stem}.{size}: {rec:?}");
            let after = writer.read(kind).expect("read after reopen");
            if rec.runs_interrupted.is_empty() {
                assert_eq!(after, doc, "{stem}.{size}");
            } else {
                // Reconciliation (request §6.5) interrupted in-flight runs of this log; everything else in
                // each run, and every other run, is unchanged.
                let (AnyStore::Log(before), AnyStore::Log(after)) = (&doc, &after) else {
                    panic!("{stem}.{size}: only a log has runs to interrupt");
                };
                assert_eq!(before.runs.len(), after.runs.len());
                for (b, a) in before.runs.iter().zip(&after.runs) {
                    if rec.runs_interrupted.contains(&b.run_id) {
                        assert_eq!(a.execution, RunState::Interrupted, "{stem}.{size}");
                        assert!(a.finished_at.is_some(), "{stem}.{size}");
                        let mut expected = b.clone();
                        expected.execution = RunState::Interrupted;
                        expected.finished_at = a.finished_at;
                        assert_eq!(
                            a, &expected,
                            "{stem}.{size}: only state and finish time change"
                        );
                    } else {
                        assert_eq!(a, b, "{stem}.{size}: untouched run");
                    }
                }
                assert_eq!(
                    (&before.events, &before.reports),
                    (&after.events, &after.reports)
                );
            }
        }
    }
}

#[test]
fn write_report_output_is_accepted_by_the_real_validator() {
    for size in ["min", "max"] {
        let tmp = Tmp::new(&format!("report-{size}"));
        let (mut writer, _) =
            StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator()).expect("init");
        let AnyStore::Report(report) =
            typed(StoreKind::Report, &fixture(&format!("report.{size}.json")))
        else {
            panic!("report fixture decodes as a report");
        };
        let slug = Slug::parse("demo-repo").unwrap();
        let manifest = writer
            .write_report(&report, &slug)
            .unwrap_or_else(|e| panic!("report.{size}: {e:?}"));
        assert!(manifest.json_written);
        // The written report file decodes through the production pipeline.
        let path = tmp.root.join(manifest.json_path.as_str());
        let bytes = std::fs::read(&path).expect("report file");
        let back = decode(StoreKind::Report, &bytes, &SchemaValidator::new())
            .expect("report file is valid");
        assert_eq!(back, AnyStore::Report(report.clone()));
        // The log with the appended manifest is valid as a whole.
        let log = writer.read(StoreKind::Log).expect("log");
        assert_valid(StoreKind::Log, &log);
        let AnyStore::Log(log) = log else {
            panic!("log")
        };
        assert_eq!(log.reports, vec![manifest]);
    }
}

/// A log holding one run per in-flight state, built from the `log.max` run template.
fn log_with_in_flight_runs() -> Value {
    let mut log = fixture("log.max.json");
    let template = log["runs"][0].clone();
    let states = [
        RunState::Queued,
        RunState::AwaitingApproval,
        RunState::Running,
        RunState::Cancelling,
    ];
    let runs: Vec<Value> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let mut run = template.clone();
            run["run_id"] = json!(format!("00000000-0000-4000-8000-0000000009{i:02}"));
            run["execution"] = serde_json::to_value(state).unwrap();
            run["created_at"] = json!(TS);
            let started = matches!(state, RunState::Running | RunState::Cancelling);
            run["started_at"] = if started { json!(TS) } else { Value::Null };
            run["finished_at"] = Value::Null;
            run["last_checkpoint_at"] = Value::Null;
            run["report_ids"] = json!([]);
            run
        })
        .collect();
    log["runs"] = Value::Array(runs);
    log["events"] = json!([]);
    log["reports"] = json!([]);
    log
}

#[test]
fn reconciliation_output_is_accepted_by_the_real_validator() {
    let tmp = Tmp::new("reconcile");
    let (writer, _) =
        StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator()).expect("init");
    drop(writer);
    let log = log_with_in_flight_runs();
    // Positive control: the planted log is valid as it stands.
    assert_eq!(
        SchemaValidator::new().validate(StoreKind::Log, &log),
        Verdict::Valid
    );
    tmp.write_root("log.json", &log);

    let (writer, rec) = StoreWriter::open(tmp.data_dir(), Arc::new(RealFs), validator())
        .expect("open reconciles with the production validator");
    assert_eq!(rec.runs_interrupted.len(), 4, "{rec:?}");
    let back = writer
        .read(StoreKind::Log)
        .expect("reconciled log is valid");
    let AnyStore::Log(back) = back else {
        panic!("log")
    };
    for run in &back.runs {
        assert_eq!(run.execution, RunState::Interrupted);
        assert!(run.finished_at.is_some());
    }
}

#[test]
fn restore_previous_with_the_real_validator() {
    let tmp = Tmp::new("restore");
    let (mut writer, _) =
        StoreWriter::init(tmp.data_dir(), Arc::new(RealFs), validator()).expect("init");
    let min = typed(StoreKind::Repositories, &fixture("repositories.min.json"));
    let max = typed(StoreKind::Repositories, &fixture("repositories.max.json"));
    writer.replace(&min).expect("first");
    writer.replace(&max).expect("second");
    drop(writer);
    std::fs::write(tmp.root.join("repo.json"), b"{not json").unwrap();

    let (mut writer, rec) =
        StoreWriter::open(tmp.data_dir(), Arc::new(RealFs), validator()).expect("open");
    assert_eq!(rec.malformed_roots, vec![StoreKind::Repositories]);
    let restored = writer
        .restore_previous(StoreKind::Repositories)
        .expect("restore");
    assert!(restored.quarantined);
    assert_eq!(
        writer.read(StoreKind::Repositories).expect("restored root"),
        min
    );
    writer.replace(&max).expect("root is unblocked");
}

//! `dev` store entry points for the CLI: init, validate, show, recover, and the [`DevRecord`] output type
//! (`planning/P02-T01-request.md` §5.9, T01-D7, T01-D8, T01-D12).
//!
//! These functions take an explicit data-directory path, never look one up, and return a [`DevOutcome`]:
//! the records produced before any failure plus the typed [`DevError`] that ended the run, if any. The CLI
//! maps [`DevError::class`] onto its exit codes and prints the records; it never touches the store itself.
//!
//! # Read-only commands
//!
//! [`validate`] and [`show`] take the data-directory lock (so a running instance is never raced) and read
//! each root through the persistence read pipeline ([`persistence::read::decode`]: size bound, JSON,
//! version gate, schema, typed decode, semantic rules) with the real [`SchemaValidator`]. They do no
//! reconciliation, write no store file and remove no temp file. The one side effect is the data-directory
//! lock itself: [`persistence::lock::acquire`] creates an empty `.app.lock` when it is missing and writes a
//! diagnostic line into it, exactly as every writer does.
//!
//! # Write commands
//!
//! [`init`] creates only the five collection roots (T01-D7). [`recover`] opens the store, which runs
//! startup reconciliation (request §6.5: leftover temp files of the application's exact pattern removed,
//! interrupted runs marked, everything else reported and left alone), and with a root named it also runs
//! `restore_previous` (the malformed file is moved to `quarantine/`, never deleted).
//!
//! # Text safety
//!
//! A record never carries a path, a file name or an instance value from a store document. Problem pointers
//! are [`SanitisedText`] built by persistence from schema property names; a shown document is the validated,
//! typed, re-serialised content, never the raw file bytes.

use std::path::Path;
use std::sync::Arc;

use domain::ids::{ReportId, RunId};
use domain::store::{AnyStore, StoreKind};
use domain::text::{SanitisedText, sanitise};
use persistence::error::StoreError;
use persistence::fs::{FsOps, RealFs};
use persistence::lock;
use persistence::paths::{DataDir, StorePath};
use persistence::read::{self, MAX_DOCUMENT_BYTES, SchemaValidator};
use persistence::recovery::ReconcileReport;
use persistence::validate::{
    DocumentValidator, Layer, Problem, ProblemCode, Problems, SchemaKeyword, VersionClass,
};
use persistence::writer::StoreWriter;
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

/// The roots `validate` inspects, in the order of `StoreKind::ALL` (reports in `backup/` are not roots).
pub const ROOT_KINDS: [StoreKind; 9] = [
    StoreKind::Config,
    StoreKind::Repositories,
    StoreKind::Workflows,
    StoreKind::Approvals,
    StoreKind::Log,
    StoreKind::Suppressions,
    StoreKind::RagIndex,
    StoreKind::Conversations,
    StoreKind::ToolManifest,
];

/// The command-line name of a root: its schema `store_type` (`repositories`, `rag_index`, `tool_manifest`,
/// …). The same text is used in JSON records.
pub fn root_name(kind: StoreKind) -> &'static str {
    kind.store_type()
}

/// The root whose [`root_name`] is `name`; `None` for anything else, including `report` (a report is not a
/// root).
pub fn parse_root(name: &str) -> Option<StoreKind> {
    ROOT_KINDS.into_iter().find(|k| root_name(*k) == name)
}

/// What validation found about one root.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DevVerdict {
    /// The file exists and passed every validation layer.
    Valid,
    /// The file exists and failed at least one layer; the problems say where.
    Invalid,
    /// The file does not exist (a store created by `init` has no `config.json` or `rag/*` yet).
    Missing,
}

impl DevVerdict {
    /// The wire string of the verdict.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Missing => "missing",
        }
    }
}

/// One validation problem as the CLI reports it: fixed-vocabulary layer and code, a sanitised pointer into
/// the document, and the semantic rule identifier if a rule found it. Never an instance value.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct DevProblem {
    /// The pipeline layer: `json`, `version`, `schema`, `typed`, `semantic` or `file`.
    pub layer: &'static str,
    /// The fixed problem code, for example `syntax` or `unsupported_version_older`.
    pub code: &'static str,
    /// JSON pointer of the location, built from schema property names only.
    pub pointer: SanitisedText,
    /// The semantic rule identifier (for example `D11-04`), if any.
    pub rule: Option<&'static str>,
}

impl DevProblem {
    /// The problem for a root that exists but is not a regular file (symlink, directory, device).
    fn not_regular_file() -> Self {
        Self {
            layer: "file",
            code: "not_regular_file",
            pointer: sanitise(""),
            rule: None,
        }
    }
}

/// The pipeline layer as a fixed string.
fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Json => "json",
        Layer::Version => "version",
        Layer::Schema => "schema",
        Layer::Typed => "typed",
        Layer::Semantic => "semantic",
    }
}

/// The problem code as a fixed string.
fn code_name(code: ProblemCode) -> &'static str {
    match code {
        ProblemCode::Syntax => "syntax",
        ProblemCode::TooLarge => "too_large",
        ProblemCode::NotObject => "not_object",
        ProblemCode::UnknownStoreType => "unknown_store_type",
        ProblemCode::UnsupportedVersion(VersionClass::Older) => "unsupported_version_older",
        ProblemCode::UnsupportedVersion(VersionClass::NewerOrUnknown) => {
            "unsupported_version_newer_or_unknown"
        }
        ProblemCode::UnsupportedVersion(VersionClass::Missing) => "unsupported_version_missing",
        ProblemCode::StoreTypeMismatch => "store_type_mismatch",
        ProblemCode::Keyword(SchemaKeyword::Type) => "keyword_type",
        ProblemCode::Keyword(SchemaKeyword::Required) => "keyword_required",
        ProblemCode::Keyword(SchemaKeyword::AdditionalProperties) => {
            "keyword_additional_properties"
        }
        ProblemCode::Keyword(SchemaKeyword::Const) => "keyword_const",
        ProblemCode::Keyword(SchemaKeyword::Enum) => "keyword_enum",
        ProblemCode::Keyword(SchemaKeyword::Format) => "keyword_format",
        ProblemCode::Keyword(SchemaKeyword::Pattern) => "keyword_pattern",
        ProblemCode::Keyword(SchemaKeyword::Minimum) => "keyword_minimum",
        ProblemCode::Keyword(SchemaKeyword::MinLength) => "keyword_min_length",
        ProblemCode::Keyword(SchemaKeyword::OneOf) => "keyword_one_of",
        ProblemCode::Keyword(SchemaKeyword::AnyOf) => "keyword_any_of",
        ProblemCode::Keyword(SchemaKeyword::Other) => "keyword_other",
        ProblemCode::TypedDecode => "typed_decode",
        ProblemCode::Rule => "rule",
    }
}

/// Converts persistence problems into [`DevProblem`]s and reports whether the list was truncated.
fn dev_problems(problems: &Problems) -> (Vec<DevProblem>, bool) {
    let items = problems
        .items
        .iter()
        .map(|p: &Problem| DevProblem {
            layer: layer_name(p.layer),
            code: code_name(p.code),
            pointer: p.pointer.clone(),
            rule: p.rule.map(|r| r.0),
        })
        .collect();
    (items, problems.truncated)
}

/// One output record of a `dev` store command. Never mixed into `AppEvent`.
///
/// Serialises as one JSON object `{"mode":"dev","dev_event":"<Variant>", …}`; root kinds are written as
/// their [`root_name`].
#[derive(Clone, PartialEq, Debug)]
pub enum DevRecord {
    /// `store init` finished.
    StoreInitialised {
        /// Roots written with empty defaults (never `config.json`, `rag/*` or `suppressions.json`).
        created: Vec<StoreKind>,
        /// Roots that already existed and were left unchanged (they may still be malformed).
        already_present: Vec<StoreKind>,
    },
    /// `store validate` result for one root.
    StoreValidated {
        /// The root.
        kind: StoreKind,
        /// Valid, invalid or missing.
        verdict: DevVerdict,
        /// Why the root is invalid; empty otherwise.
        problems: Vec<DevProblem>,
        /// Whether more problems were found than are listed.
        truncated: bool,
    },
    /// `store show` result for one root.
    StoreShown {
        /// The root.
        kind: StoreKind,
        /// Valid, invalid or missing.
        verdict: DevVerdict,
        /// The validated, typed, re-serialised document; `None` unless the verdict is valid.
        document: Option<serde_json::Value>,
        /// Why the root is invalid; empty otherwise.
        problems: Vec<DevProblem>,
        /// Whether more problems were found than are listed.
        truncated: bool,
    },
    /// `store recover` finished startup reconciliation (request §6.5). Counts and identifiers only.
    StoreReconciled {
        /// Leftover temp files of the application's exact pattern that were removed.
        temp_files_removed: u32,
        /// Runs that were in flight and were set to `interrupted`.
        runs_interrupted: Vec<RunId>,
        /// Approvals still pending (left unchanged).
        approvals_pending: u32,
        /// Manifests claiming a JSON report that is missing (reported, not changed).
        manifests_without_report: Vec<ReportId>,
        /// Report files without a manifest (reported, not changed).
        reports_without_manifest: u32,
        /// Roots found malformed or foreign (reported, never repaired).
        malformed_roots: Vec<StoreKind>,
    },
    /// `store recover --restore-previous` restored a root from its `.prev` generation.
    StoreRestored {
        /// The restored root.
        kind: StoreKind,
        /// Whether a current file existed and was moved to `quarantine/`.
        quarantined: bool,
    },
}

impl DevRecord {
    /// The `dev_event` name of this record.
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::StoreInitialised { .. } => "StoreInitialised",
            Self::StoreValidated { .. } => "StoreValidated",
            Self::StoreShown { .. } => "StoreShown",
            Self::StoreReconciled { .. } => "StoreReconciled",
            Self::StoreRestored { .. } => "StoreRestored",
        }
    }
}

/// The names of `kinds`, for JSON arrays.
fn names(kinds: &[StoreKind]) -> Vec<&'static str> {
    kinds.iter().map(|k| root_name(*k)).collect()
}

/// Starts the JSON object of a record: `mode` and `dev_event` first, then room for `fields` more entries.
fn begin<S: Serializer>(
    serializer: S,
    event: &'static str,
    fields: usize,
) -> Result<S::SerializeMap, S::Error> {
    let mut map = serializer.serialize_map(Some(fields.saturating_add(2)))?;
    map.serialize_entry("mode", "dev")?;
    map.serialize_entry("dev_event", event)?;
    Ok(map)
}

impl Serialize for DevRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map;
        match self {
            Self::StoreInitialised {
                created,
                already_present,
            } => {
                map = begin(serializer, self.event_name(), 2)?;
                map.serialize_entry("created", &names(created))?;
                map.serialize_entry("already_present", &names(already_present))?;
            }
            Self::StoreValidated {
                kind,
                verdict,
                problems,
                truncated,
            } => {
                map = begin(serializer, self.event_name(), 4)?;
                map.serialize_entry("kind", root_name(*kind))?;
                map.serialize_entry("verdict", verdict.as_str())?;
                map.serialize_entry("problems", problems)?;
                map.serialize_entry("truncated", truncated)?;
            }
            Self::StoreShown {
                kind,
                verdict,
                document,
                problems,
                truncated,
            } => {
                map = begin(serializer, self.event_name(), 5)?;
                map.serialize_entry("kind", root_name(*kind))?;
                map.serialize_entry("verdict", verdict.as_str())?;
                map.serialize_entry("document", document)?;
                map.serialize_entry("problems", problems)?;
                map.serialize_entry("truncated", truncated)?;
            }
            Self::StoreReconciled {
                temp_files_removed,
                runs_interrupted,
                approvals_pending,
                manifests_without_report,
                reports_without_manifest,
                malformed_roots,
            } => {
                map = begin(serializer, self.event_name(), 6)?;
                map.serialize_entry("temp_files_removed", temp_files_removed)?;
                map.serialize_entry("runs_interrupted", runs_interrupted)?;
                map.serialize_entry("approvals_pending", approvals_pending)?;
                map.serialize_entry("manifests_without_report", manifests_without_report)?;
                map.serialize_entry("reports_without_manifest", reports_without_manifest)?;
                map.serialize_entry("malformed_roots", &names(malformed_roots))?;
            }
            Self::StoreRestored { kind, quarantined } => {
                map = begin(serializer, self.event_name(), 2)?;
                map.serialize_entry("kind", root_name(*kind))?;
                map.serialize_entry("quarantined", quarantined)?;
            }
        }
        map.end()
    }
}

/// How the CLI should classify a failure (request §5.9 exit codes 3, 4 and 5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FailureClass {
    /// Another instance holds the data-directory lock (exit 3).
    Locked,
    /// A storage failure, or a writer halted by one (exit 4).
    Storage,
    /// Any other typed operation failure (exit 5).
    Operation,
}

/// Why a `dev` store command failed. `Display` is fixed English text: no path, file name or document value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DevError {
    /// Another instance holds the data-directory lock.
    Locked,
    /// A storage operation failed.
    Storage,
    /// Storage is halted after an earlier failure.
    Halted,
    /// The data directory was rejected: missing, not an absolute real directory, or with group/other
    /// permission bits or a foreign owner.
    DataDirRejected,
    /// The embedded schema could not be used, so nothing can be called valid (FR-15).
    SchemaUnavailable,
    /// `restore_previous` found no previous generation of the root.
    NoPreviousGeneration(StoreKind),
    /// The previous generation of the root is not a valid document, so it was not restored.
    PreviousInvalid(StoreKind),
    /// The root is malformed in a way that blocks the operation.
    Malformed(StoreKind),
    /// The operation was refused (for example restoring the reserved `suppressions.json`).
    Refused,
    /// A validated document could not be represented as JSON.
    Unrepresentable,
    /// Running as root against a data directory not owned by root (FR-33).
    RootForeignDataDir,
}

impl DevError {
    /// The class that selects the CLI exit code.
    pub fn class(&self) -> FailureClass {
        match self {
            Self::Locked => FailureClass::Locked,
            Self::Storage | Self::Halted => FailureClass::Storage,
            Self::DataDirRejected
            | Self::SchemaUnavailable
            | Self::NoPreviousGeneration(_)
            | Self::PreviousInvalid(_)
            | Self::Malformed(_)
            | Self::Refused
            | Self::Unrepresentable
            | Self::RootForeignDataDir => FailureClass::Operation,
        }
    }
}

impl std::fmt::Display for DevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Locked => "the data directory is locked by another instance",
            Self::Storage => "a storage operation failed",
            Self::Halted => "storage is halted after an earlier failure",
            Self::DataDirRejected => {
                "the data directory was rejected (it must be an existing private directory)"
            }
            Self::SchemaUnavailable => "the store schema is unavailable; nothing can be validated",
            Self::NoPreviousGeneration(_) => "the root has no previous generation to restore",
            Self::PreviousInvalid(_) => "the previous generation is not a valid document",
            Self::Malformed(_) => "the existing store file is malformed",
            Self::Refused => "the operation was refused",
            Self::Unrepresentable => "the document could not be represented as JSON",
            Self::RootForeignDataDir => {
                "refusing to use a data directory not owned by root while running as root"
            }
        };
        f.write_str(text)
    }
}

impl std::error::Error for DevError {}

impl From<StoreError> for DevError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Locked => Self::Locked,
            StoreError::Io { .. } => Self::Storage,
            StoreError::Halted => Self::Halted,
            StoreError::SchemaUnavailable => Self::SchemaUnavailable,
            StoreError::NotInitialised(kind) => Self::NoPreviousGeneration(kind),
            StoreError::Invalid(kind, _) => Self::PreviousInvalid(kind),
            StoreError::Malformed(kind) => Self::Malformed(kind),
            StoreError::PathRejected => Self::DataDirRejected,
            StoreError::ReportNameCollision => Self::Refused,
            StoreError::RootForeignDataDir => Self::RootForeignDataDir,
        }
    }
}

/// The result of one entry function: the records produced, in order, and the failure that ended the run, if
/// any. A failure after some records (for example a storage error on the third root) keeps the earlier
/// records, so the CLI reports what was actually done.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct DevOutcome {
    /// Records in the order they were produced.
    pub records: Vec<DevRecord>,
    /// The error that stopped the run, if any.
    pub failure: Option<DevError>,
}

impl DevOutcome {
    /// An outcome that failed before producing anything.
    fn failed(error: impl Into<DevError>) -> Self {
        Self {
            records: Vec::new(),
            failure: Some(error.into()),
        }
    }
}

/// What reading one root found.
enum RootRead {
    Missing,
    Valid(Box<AnyStore>),
    Invalid(Vec<DevProblem>, bool),
}

/// Reads and validates the root of `kind` without writing anything. A missing file is
/// [`RootRead::Missing`]; every document fault is [`RootRead::Invalid`]; an unusable schema or a storage
/// error is an `Err`.
fn read_root(
    fs: &dyn FsOps,
    dir: &DataDir,
    validator: &dyn DocumentValidator,
    kind: StoreKind,
) -> Result<RootRead, DevError> {
    let path = StorePath::root(kind).ok_or(DevError::Refused)?;
    let real = dir.join(&path);
    match fs.symlink_metadata(&real) {
        Ok(info) if info.is_regular() => {}
        Ok(_) => {
            return Ok(RootRead::Invalid(
                vec![DevProblem::not_regular_file()],
                false,
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RootRead::Missing),
        Err(_) => return Err(DevError::Storage),
    }
    let bytes = match fs.read(&real, MAX_DOCUMENT_BYTES) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RootRead::Missing),
        Err(e) if e.kind() == std::io::ErrorKind::FileTooLarge => {
            let problems =
                Problems::one(Problem::new(Layer::Json, ProblemCode::TooLarge, "", None));
            let (items, truncated) = dev_problems(&problems);
            return Ok(RootRead::Invalid(items, truncated));
        }
        Err(_) => return Err(DevError::Storage),
    };
    match read::decode(kind, &bytes, validator) {
        Ok(doc) => Ok(RootRead::Valid(Box::new(doc))),
        Err(StoreError::Invalid(_, problems)) => {
            let (items, truncated) = dev_problems(&problems);
            Ok(RootRead::Invalid(items, truncated))
        }
        Err(other) => Err(other.into()),
    }
}

/// Validates the data directory path and takes the data-directory lock for a read-only command.
fn lock_for_read(fs: &dyn FsOps, dir: &Path) -> Result<(DataDir, lock::StoreLock), DevError> {
    let data = DataDir::new(fs, dir)?;
    let held = lock::acquire(&data, fs)?;
    Ok((data, held))
}

/// The record for one validated root.
fn validated_record(kind: StoreKind, read: RootRead) -> DevRecord {
    match read {
        RootRead::Missing => DevRecord::StoreValidated {
            kind,
            verdict: DevVerdict::Missing,
            problems: Vec::new(),
            truncated: false,
        },
        RootRead::Valid(_) => DevRecord::StoreValidated {
            kind,
            verdict: DevVerdict::Valid,
            problems: Vec::new(),
            truncated: false,
        },
        RootRead::Invalid(problems, truncated) => DevRecord::StoreValidated {
            kind,
            verdict: DevVerdict::Invalid,
            problems,
            truncated,
        },
    }
}

/// [`init`] over an injected file system and validator: fault-injection seam for tests; production callers
/// use `init`/`validate`/`show`/`recover`.
pub fn init_with(
    fs: Arc<dyn FsOps>,
    validator: Arc<dyn DocumentValidator>,
    dir: &Path,
) -> DevOutcome {
    let data = match DataDir::new(&*fs, dir) {
        Ok(data) => data,
        Err(e) => return DevOutcome::failed(e),
    };
    match StoreWriter::init(data, fs, validator) {
        Ok((_writer, report)) => DevOutcome {
            records: vec![DevRecord::StoreInitialised {
                created: report.created,
                already_present: report.already_present,
            }],
            failure: None,
        },
        Err(e) => DevOutcome::failed(e),
    }
}

/// `store init`: creates the missing collection roots (`repo.json`, `workflows.json`, `approvals.json`,
/// `log.json`, `tools.json`) in the existing private directory `dir` and reports the created and the
/// already present ones. Running it again changes nothing. One [`DevRecord::StoreInitialised`].
pub fn init(dir: &Path) -> DevOutcome {
    init_with(Arc::new(RealFs), Arc::new(SchemaValidator::new()), dir)
}

/// [`validate`] over an injected file system and validator: fault-injection seam for tests; production
/// callers use `init`/`validate`/`show`/`recover`.
pub fn validate_with(
    fs: &dyn FsOps,
    validator: &dyn DocumentValidator,
    dir: &Path,
    root: Option<StoreKind>,
) -> DevOutcome {
    let (data, _held) = match lock_for_read(fs, dir) {
        Ok(pair) => pair,
        Err(e) => return DevOutcome::failed(e),
    };
    let kinds: Vec<StoreKind> = root.map_or_else(|| ROOT_KINDS.to_vec(), |k| vec![k]);
    let mut outcome = DevOutcome::default();
    for kind in kinds {
        match read_root(fs, &data, validator, kind) {
            Ok(read) => outcome.records.push(validated_record(kind, read)),
            Err(e) => {
                outcome.failure = Some(e);
                break;
            }
        }
    }
    outcome
}

/// `store validate`: read-only validation of every root, or only `root`. One [`DevRecord::StoreValidated`]
/// per root in [`ROOT_KINDS`] order; a missing root has verdict `missing`, an invalid one lists its problems,
/// and neither is a failure of the command. See the module documentation for what "read-only" includes.
pub fn validate(dir: &Path, root: Option<StoreKind>) -> DevOutcome {
    validate_with(&RealFs, &SchemaValidator::new(), dir, root)
}

/// [`show`] over an injected file system and validator: fault-injection seam for tests; production callers
/// use `init`/`validate`/`show`/`recover`.
pub fn show_with(
    fs: &dyn FsOps,
    validator: &dyn DocumentValidator,
    dir: &Path,
    kind: StoreKind,
) -> DevOutcome {
    let (data, _held) = match lock_for_read(fs, dir) {
        Ok(pair) => pair,
        Err(e) => return DevOutcome::failed(e),
    };
    let record = match read_root(fs, &data, validator, kind) {
        Ok(RootRead::Valid(doc)) => match serde_json::to_value(&*doc) {
            Ok(value) => DevRecord::StoreShown {
                kind,
                verdict: DevVerdict::Valid,
                document: Some(value),
                problems: Vec::new(),
                truncated: false,
            },
            Err(_) => return DevOutcome::failed(DevError::Unrepresentable),
        },
        Ok(RootRead::Missing) => DevRecord::StoreShown {
            kind,
            verdict: DevVerdict::Missing,
            document: None,
            problems: Vec::new(),
            truncated: false,
        },
        Ok(RootRead::Invalid(problems, truncated)) => DevRecord::StoreShown {
            kind,
            verdict: DevVerdict::Invalid,
            document: None,
            problems,
            truncated,
        },
        Err(e) => return DevOutcome::failed(e),
    };
    DevOutcome {
        records: vec![record],
        failure: None,
    }
}

/// `store show <root>`: read-only. A valid root yields its validated, typed, re-serialised document (never
/// the raw file bytes); an invalid root yields its problems only; a missing root yields verdict `missing`.
/// One [`DevRecord::StoreShown`].
pub fn show(dir: &Path, kind: StoreKind) -> DevOutcome {
    show_with(&RealFs, &SchemaValidator::new(), dir, kind)
}

/// The reconciliation report as a record.
fn reconciled_record(report: ReconcileReport) -> DevRecord {
    DevRecord::StoreReconciled {
        temp_files_removed: report.temp_files_removed,
        runs_interrupted: report.runs_interrupted,
        approvals_pending: report.approvals_pending,
        manifests_without_report: report.manifests_without_report,
        reports_without_manifest: report.reports_without_manifest,
        malformed_roots: report.malformed_roots,
    }
}

/// [`recover`] over an injected file system and validator: fault-injection seam for tests; production
/// callers use `init`/`validate`/`show`/`recover`.
pub fn recover_with(
    fs: Arc<dyn FsOps>,
    validator: Arc<dyn DocumentValidator>,
    dir: &Path,
    restore: Option<StoreKind>,
) -> DevOutcome {
    let data = match DataDir::new(&*fs, dir) {
        Ok(data) => data,
        Err(e) => return DevOutcome::failed(e),
    };
    let (mut writer, report) = match StoreWriter::open(data, fs, validator) {
        Ok(pair) => pair,
        Err(e) => return DevOutcome::failed(e),
    };
    let mut outcome = DevOutcome {
        records: vec![reconciled_record(report)],
        failure: None,
    };
    if let Some(kind) = restore {
        match writer.restore_previous(kind) {
            Ok(done) => outcome.records.push(DevRecord::StoreRestored {
                kind: done.kind,
                quarantined: done.quarantined,
            }),
            Err(StoreError::PathRejected) => outcome.failure = Some(DevError::Refused),
            Err(e) => outcome.failure = Some(e.into()),
        }
    }
    outcome
}

/// `store recover`: opens the store, which runs startup reconciliation (request §6.5, T01-D8), and reports
/// it as one [`DevRecord::StoreReconciled`]. With `restore_previous` set it then restores that root from its
/// `.prev` generation, moving the current file to `quarantine/`, and adds one [`DevRecord::StoreRestored`].
/// If the restore fails, the reconciliation record is still returned: its changes were already made.
pub fn recover(dir: &Path, restore_previous: Option<StoreKind>) -> DevOutcome {
    recover_with(
        Arc::new(RealFs),
        Arc::new(SchemaValidator::new()),
        dir,
        restore_previous,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use persistence::error::WriteStep;
    use persistence::fault::{FaultFs, FaultPlan};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A private temp directory removed on drop. Tests only.
    struct Tmp(PathBuf);

    impl Tmp {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("pwt-dev-store-{label}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn json(record: &DevRecord) -> String {
        serde_json::to_string(record).unwrap()
    }

    /// Fails if the field order, the `mode`/`dev_event` members or the kind spelling change.
    #[test]
    fn records_serialise_with_mode_and_event_first() {
        let created = DevRecord::StoreInitialised {
            created: vec![StoreKind::Repositories, StoreKind::ToolManifest],
            already_present: vec![StoreKind::Log],
        };
        assert_eq!(
            json(&created),
            r#"{"mode":"dev","dev_event":"StoreInitialised","created":["repositories","tool_manifest"],"already_present":["log"]}"#
        );
        let validated = DevRecord::StoreValidated {
            kind: StoreKind::RagIndex,
            verdict: DevVerdict::Invalid,
            problems: vec![DevProblem {
                layer: "version",
                code: "unsupported_version_older",
                pointer: sanitise("/schema_version"),
                rule: None,
            }],
            truncated: false,
        };
        assert_eq!(
            json(&validated),
            r#"{"mode":"dev","dev_event":"StoreValidated","kind":"rag_index","verdict":"invalid","problems":[{"layer":"version","code":"unsupported_version_older","pointer":"/schema_version","rule":null}],"truncated":false}"#
        );
        let restored = DevRecord::StoreRestored {
            kind: StoreKind::Log,
            quarantined: true,
        };
        assert_eq!(
            json(&restored),
            r#"{"mode":"dev","dev_event":"StoreRestored","kind":"log","quarantined":true}"#
        );
        let shown = DevRecord::StoreShown {
            kind: StoreKind::Log,
            verdict: DevVerdict::Missing,
            document: None,
            problems: Vec::new(),
            truncated: false,
        };
        assert_eq!(
            json(&shown),
            r#"{"mode":"dev","dev_event":"StoreShown","kind":"log","verdict":"missing","document":null,"problems":[],"truncated":false}"#
        );
        let reconciled = DevRecord::StoreReconciled {
            temp_files_removed: 2,
            runs_interrupted: Vec::new(),
            approvals_pending: 1,
            manifests_without_report: Vec::new(),
            reports_without_manifest: 0,
            malformed_roots: vec![StoreKind::Repositories],
        };
        assert_eq!(
            json(&reconciled),
            r#"{"mode":"dev","dev_event":"StoreReconciled","temp_files_removed":2,"runs_interrupted":[],"approvals_pending":1,"manifests_without_report":[],"reports_without_manifest":0,"malformed_roots":["repositories"]}"#
        );
    }

    /// Fails if a root name stops round-tripping or `report` becomes a root.
    #[test]
    fn root_names_round_trip_and_exclude_reports() {
        for kind in ROOT_KINDS {
            assert_eq!(parse_root(root_name(kind)), Some(kind));
        }
        assert_eq!(parse_root("report"), None);
        assert_eq!(parse_root("repo"), None);
        assert_eq!(parse_root(""), None);
    }

    /// Fails if a storage error is classified as an operation failure or a lock as anything but locked.
    #[test]
    fn store_errors_map_to_failure_classes() {
        let io = StoreError::Io {
            step: WriteStep::Read,
            kind: std::io::ErrorKind::Other,
        };
        assert_eq!(DevError::from(io).class(), FailureClass::Storage);
        assert_eq!(
            DevError::from(StoreError::Halted).class(),
            FailureClass::Storage
        );
        assert_eq!(
            DevError::from(StoreError::Locked).class(),
            FailureClass::Locked
        );
        for other in [
            StoreError::SchemaUnavailable,
            StoreError::PathRejected,
            StoreError::ReportNameCollision,
            StoreError::Malformed(StoreKind::Log),
            StoreError::NotInitialised(StoreKind::Log),
        ] {
            assert_eq!(DevError::from(other).class(), FailureClass::Operation);
        }
    }

    /// Fails if an injected storage failure while taking the lock is not reported as a storage failure.
    #[test]
    fn injected_lock_failure_is_a_storage_failure() {
        let tmp = Tmp::new("fault-lock");
        let fault = FaultFs::new(FaultPlan {
            step: WriteStep::Lock,
            nth: 1,
            error: std::io::ErrorKind::Other,
        });
        let outcome = validate_with(&fault, &SchemaValidator::new(), &tmp.0, None);
        assert!(fault.fired());
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.failure, Some(DevError::Storage));
    }

    /// Fails if an injected storage failure during `init` is hidden or misclassified.
    #[test]
    fn injected_write_failure_during_init_is_a_storage_failure() {
        let tmp = Tmp::new("fault-init");
        let fault = FaultFs::new(FaultPlan {
            step: WriteStep::CreateTemp,
            nth: 1,
            error: std::io::ErrorKind::Other,
        });
        let outcome = init_with(Arc::new(fault), Arc::new(SchemaValidator::new()), &tmp.0);
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.failure, Some(DevError::Storage));
    }

    /// Fails if `validate` writes anything but the lock file, or reports the wrong verdicts for a fresh
    /// store.
    #[test]
    fn validate_is_read_only_and_reports_missing_roots() {
        let tmp = Tmp::new("validate");
        let init_outcome = init(&tmp.0);
        assert_eq!(init_outcome.failure, None);
        let before = listing(&tmp.0);
        let outcome = validate(&tmp.0, None);
        assert_eq!(outcome.failure, None);
        assert_eq!(outcome.records.len(), ROOT_KINDS.len());
        let verdicts: Vec<(StoreKind, DevVerdict)> = outcome
            .records
            .iter()
            .filter_map(|r| match r {
                DevRecord::StoreValidated { kind, verdict, .. } => Some((*kind, *verdict)),
                _ => None,
            })
            .collect();
        for (kind, verdict) in verdicts {
            let expect = if matches!(
                kind,
                StoreKind::Config
                    | StoreKind::Suppressions
                    | StoreKind::RagIndex
                    | StoreKind::Conversations
            ) {
                DevVerdict::Missing
            } else {
                DevVerdict::Valid
            };
            assert_eq!(verdict, expect, "{}", root_name(kind));
        }
        assert_eq!(listing(&tmp.0), before);
    }

    fn listing(dir: &Path) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().unwrap().len(),
                )
            })
            // The lock file's diagnostic line is rewritten by every locking command.
            .filter(|(name, _)| name != ".app.lock")
            .collect();
        out.sort();
        out
    }
}

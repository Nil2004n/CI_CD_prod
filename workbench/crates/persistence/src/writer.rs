//! StoreWriter: single serialised writer, init/open/replace/write_report/restore_previous (request §5.6,
//! §6.3, §6.5).
//!
//! Frozen public surface (request §5.12): [`StoreWriter`] and its methods, [`InitReport`],
//! [`RestoreReport`].
//!
//! # Guarantees
//!
//! * **Single writer.** The writer holds the data-directory lock for its whole life ([`crate::lock`]); every
//!   mutating method takes `&mut self`, so writes are serialised without an internal mutex.
//! * **Validate before write.** Every document is serialised to a JSON value and passed to the injected
//!   [`DocumentValidator`] first. `Invalid` becomes [`StoreError::Invalid`], `SchemaUnavailable` becomes
//!   [`StoreError::SchemaUnavailable`] (FR-15: never treated as valid); in both cases nothing has been
//!   written. Reads go through [`crate::read::decode`], so a document is only ever handed out after the same
//!   validation.
//! * **Halt on storage errors (Data Schema §11.25).** Any [`StoreError::Io`] raised inside a mutating
//!   method, at whatever step, halts the writer: every later mutating call returns [`StoreError::Halted`]
//!   until the data directory is opened again. Reads keep working, because the last validated copy stays
//!   readable after an injected failure at every step (request §6.3). Validation failures, name collisions
//!   and refused paths are not storage errors and do not halt.
//! * **Roots that are not recognised are never overwritten.** A malformed or foreign root found by
//!   [`StoreWriter::open`] (or [`StoreWriter::init`]) is *blocked*: [`StoreWriter::replace`] refuses it with
//!   [`StoreError::Malformed`] and the file is never rotated into `.prev` (D02-9). Only
//!   [`StoreWriter::restore_previous`] moves it away, and only to `quarantine/`.
//! * **`suppressions.json` is never written** (reserved, RES-06): `replace` and `restore_previous` answer
//!   [`StoreError::PathRejected`]. A report is not a root: `read` and `replace` answer
//!   [`StoreError::PathRejected`] for [`StoreKind::Report`]; reports are written only by
//!   [`StoreWriter::write_report`].
//!
//! # Origin rules that are not the writer's job
//!
//! The writer persists exactly the typed values it is given. Redaction of free text (security L-3) happens
//! where a `RedactedText` is created, and credential references (security L-2) are created and checked by
//! the provider code of P03/P06; the writer neither redacts nor creates credential references, and only
//! checks what the schema and semantic layers can check.
//!
//! # Limits
//!
//! Cross-file operations are not one transaction (Data Schema §12): the report file and the `log.json`
//! manifest are two separate atomic steps. Directory checks are path based and not atomic with the later
//! writes (see [`crate::fs`]). Documents larger than [`crate::read::MAX_DOCUMENT_BYTES`] are refused on
//! write too, so the writer never produces a file its own reader would reject.

use std::path::Path;
use std::sync::Arc;

use domain::store::approval::ApprovalFile;
use domain::store::log::{LogFile, ReportManifest};
use domain::store::report::ReportFile;
use domain::store::repository::RepositoryFile;
use domain::store::tools::ToolManifestFile;
use domain::store::values::{AppRelPath, Digest, Slug};
use domain::store::workflow::WorkflowFile;
use domain::store::{
    AnyStore, ApprovalsStoreType, LogStoreType, RepositoriesStoreType, SchemaVersion, StoreKind,
    ToolManifestStoreType, WorkflowsStoreType,
};
use domain::time::UtcTimestamp;
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::atomic;
use crate::error::{StoreError, WriteStep};
use crate::fs::FsOps;
use crate::lock::{self, StoreLock};
use crate::paths::{DataDir, PRIVATE_DIR_MODE, StorePath, check_private_dir};
use crate::read;
use crate::recovery::{self, ReconcileReport};
use crate::report_name::{compact_utc, report_stem};
use crate::validate::{DocumentValidator, Layer, Problem, ProblemCode, Problems, Verdict};

/// The roots `init` creates (T01-D7): `repo.json`, `workflows.json`, `approvals.json`, `log.json` and
/// `tools.json`. Not `config.json` (its required limits and theme would have to be invented; P03 writes it
/// first), not `rag/*` (P17) and never `suppressions.json`.
const INIT_ROOTS: [StoreKind; 5] = [
    StoreKind::Repositories,
    StoreKind::Workflows,
    StoreKind::Approvals,
    StoreKind::Log,
    StoreKind::ToolManifest,
];

/// The roots the application reads and writes, in the order they are inspected at startup. Excludes
/// `suppressions.json` (reserved and never written, so there is nothing to protect or restore) and the
/// reports in `backup/` (immutable, never roots).
pub(crate) const MANAGED_ROOTS: [StoreKind; 8] = [
    StoreKind::Config,
    StoreKind::Repositories,
    StoreKind::Workflows,
    StoreKind::Approvals,
    StoreKind::Log,
    StoreKind::RagIndex,
    StoreKind::Conversations,
    StoreKind::ToolManifest,
];

/// Directory (inside the data directory) that receives root files moved aside by `restore_previous`.
const QUARANTINE_DIR: &str = "quarantine";

/// What `init` created (T01-D7: the five collection roots `repo`, `workflows`, `approvals`, `log`,
/// `tools`; never `config.json`, `rag/*` or `suppressions.json`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct InitReport {
    /// Roots written with empty defaults.
    pub created: Vec<StoreKind>,
    /// Roots that already existed and were left unchanged. A root in this list may still be malformed;
    /// in that case writes to it are refused ([`StoreError::Malformed`]) until `restore_previous`.
    pub already_present: Vec<StoreKind>,
}

/// What `restore_previous` did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RestoreReport {
    /// The root that was restored from its `.prev` generation.
    pub kind: StoreKind,
    /// Whether a current file existed and was moved to `quarantine/`.
    pub quarantined: bool,
}

/// The state of one root file as found on disk.
pub(crate) enum RootState {
    /// The file does not exist.
    Absent,
    /// The file exists and passed `read::decode` (boxed: documents are large next to the other variants).
    Valid(Box<AnyStore>),
    /// The file exists but is malformed, foreign (wrong `store_type`, unsupported version, not a regular
    /// file) or larger than the size bound.
    Malformed,
}

/// Everything a storage operation needs besides the mutable writer state; cheap to clone.
#[derive(Clone, Debug)]
pub(crate) struct Env {
    pub(crate) fs: Arc<dyn FsOps>,
    pub(crate) dir: DataDir,
    pub(crate) validator: Arc<dyn DocumentValidator>,
    /// The uid of the lock file this process holds, which stands for the uid of the process
    /// ([`crate::lock`]); directories the store uses must be owned by it.
    owner_uid: u32,
}

/// An I/O error at `step`, as the [`StoreError`] that halts the writer.
pub(crate) fn io_at(step: WriteStep) -> impl Fn(std::io::Error) -> StoreError {
    move |e| StoreError::Io {
        step,
        kind: e.kind(),
    }
}

/// A single fixed-text problem at the document root.
fn root_problem(layer: Layer, code: ProblemCode) -> Problems {
    Problems::one(Problem::new(layer, code, "", None))
}

/// The error for a document that cannot be represented as JSON (for example a non-finite float); it is
/// reported as a typed-layer fault, because no JSON value exists to validate.
fn unrepresentable(kind: StoreKind) -> StoreError {
    StoreError::Invalid(kind, root_problem(Layer::Typed, ProblemCode::TypedDecode))
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Env {
    /// Reads the regular file at `path` (a root of `kind` or its `.prev`), bounded by
    /// [`read::MAX_DOCUMENT_BYTES`].
    ///
    /// Errors: missing → [`StoreError::NotInitialised`]; present but not a regular file (symlink,
    /// directory, device) → [`StoreError::Malformed`]; larger than the bound → [`StoreError::Invalid`]
    /// with a `TooLarge` problem; any other I/O error → [`StoreError::Io`] at [`WriteStep::Read`].
    fn read_file(&self, kind: StoreKind, path: &Path) -> Result<Vec<u8>, StoreError> {
        match self.fs.symlink_metadata(path) {
            Ok(info) if info.is_regular() => {}
            Ok(_) => return Err(StoreError::Malformed(kind)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::NotInitialised(kind));
            }
            Err(e) => return Err(io_at(WriteStep::Read)(e)),
        }
        self.fs
            .read(path, read::MAX_DOCUMENT_BYTES)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => StoreError::NotInitialised(kind),
                std::io::ErrorKind::FileTooLarge => {
                    StoreError::Invalid(kind, root_problem(Layer::Json, ProblemCode::TooLarge))
                }
                _ => io_at(WriteStep::Read)(e),
            })
    }

    /// Reads and fully validates the root of `kind` (`Report` is not a root: `PathRejected`).
    pub(crate) fn load_root(&self, kind: StoreKind) -> Result<AnyStore, StoreError> {
        let path = StorePath::root(kind).ok_or(StoreError::PathRejected)?;
        let bytes = self.read_file(kind, &self.dir.join(&path))?;
        read::decode(kind, &bytes, &*self.validator)
    }

    /// Classifies the root of `kind` for startup: absent, valid, or malformed/foreign. A schema that is
    /// unavailable or a storage error is an error, never a classification.
    pub(crate) fn classify(&self, kind: StoreKind) -> Result<RootState, StoreError> {
        match self.load_root(kind) {
            Ok(doc) => Ok(RootState::Valid(Box::new(doc))),
            Err(StoreError::NotInitialised(_)) => Ok(RootState::Absent),
            Err(StoreError::Invalid(..) | StoreError::Malformed(_)) => Ok(RootState::Malformed),
            Err(e) => Err(e),
        }
    }

    /// Serialises `doc` (a document of `kind`), validates it and returns the exact bytes to write.
    ///
    /// Nothing is written here. Bytes are pretty-printed JSON with a trailing newline.
    fn render<T: Serialize>(&self, kind: StoreKind, doc: &T) -> Result<Vec<u8>, StoreError> {
        let value = serde_json::to_value(doc).map_err(|_| unrepresentable(kind))?;
        match self.validator.validate(kind, &value) {
            Verdict::Valid => {}
            Verdict::Invalid(problems) => return Err(StoreError::Invalid(kind, problems)),
            Verdict::SchemaUnavailable => return Err(StoreError::SchemaUnavailable),
        }
        let mut bytes = serde_json::to_vec_pretty(&value).map_err(|_| unrepresentable(kind))?;
        bytes.push(b'\n');
        if u64::try_from(bytes.len()).map_or(true, |n| n > read::MAX_DOCUMENT_BYTES) {
            return Err(StoreError::Invalid(
                kind,
                root_problem(Layer::Json, ProblemCode::TooLarge),
            ));
        }
        Ok(bytes)
    }

    /// Creates the directory holding `path` if it is missing and requires it to be private and owned by
    /// this process (R-D03-8). Does nothing for a file directly in the data directory.
    fn prepare_parent(&self, path: &StorePath) -> Result<(), StoreError> {
        atomic::ensure_parent(&*self.fs, &self.dir, path)?;
        if path.parent().is_some() {
            check_private_dir(&*self.fs, &self.dir.parent_of(path), self.owner_uid)?;
        }
        Ok(())
    }

    /// Writes `bytes` as the new content of the root of `kind` through the replace protocol.
    pub(crate) fn publish_root(
        &self,
        kind: StoreKind,
        bytes: &[u8],
        keep_previous: bool,
    ) -> Result<(), StoreError> {
        let path = StorePath::root(kind).ok_or(StoreError::PathRejected)?;
        self.prepare_parent(&path)?;
        atomic::write_replace(&*self.fs, &self.dir, &path, bytes, keep_previous)
    }

    /// Requires `path` to be a private real directory owned by this process, creating it (mode
    /// [`PRIVATE_DIR_MODE`]) if it is missing.
    fn ensure_private_dir(&self, path: &Path) -> Result<(), StoreError> {
        match self.fs.symlink_metadata(path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match self
                    .fs
                    .create_dir(WriteStep::CreateDir, path, PRIVATE_DIR_MODE)
                {
                    Ok(()) => {}
                    // Lost a creation race: acceptable only if the winner's directory passes the check.
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(io_at(WriteStep::CreateDir)(e)),
                }
                self.fs
                    .sync_dir(WriteStep::CreateDir, self.dir.as_path())
                    .map_err(io_at(WriteStep::CreateDir))?;
            }
            Err(e) => return Err(io_at(WriteStep::CreateDir)(e)),
        }
        check_private_dir(&*self.fs, path, self.owner_uid)
    }
}

/// The empty default document of one of the [`INIT_ROOTS`].
fn empty_root(kind: StoreKind, now: UtcTimestamp) -> Result<AnyStore, StoreError> {
    Ok(match kind {
        StoreKind::Repositories => AnyStore::Repositories(RepositoryFile {
            schema_version: SchemaVersion,
            store_type: RepositoriesStoreType,
            updated_at: now,
            repositories: Vec::new(),
        }),
        StoreKind::Workflows => AnyStore::Workflows(WorkflowFile {
            schema_version: SchemaVersion,
            store_type: WorkflowsStoreType,
            updated_at: now,
            workflows: Vec::new(),
        }),
        StoreKind::Approvals => AnyStore::Approvals(ApprovalFile {
            schema_version: SchemaVersion,
            store_type: ApprovalsStoreType,
            updated_at: now,
            approvals: Vec::new(),
        }),
        StoreKind::Log => AnyStore::Log(LogFile {
            schema_version: SchemaVersion,
            store_type: LogStoreType,
            updated_at: now,
            runs: Vec::new(),
            events: Vec::new(),
            reports: Vec::new(),
        }),
        StoreKind::ToolManifest => AnyStore::ToolManifest(ToolManifestFile {
            schema_version: SchemaVersion,
            store_type: ToolManifestStoreType,
            updated_at: now,
            tools: Vec::new(),
        }),
        _ => return Err(StoreError::PathRejected),
    })
}

/// The single writer of one data directory. Holds the lock for its lifetime; every mutating method takes
/// `&mut self`, so writes are serialised without an internal mutex. After any storage error the writer is
/// halted and refuses further writes until reopened (Data Schema §11.25).
///
/// See the module documentation for the full list of guarantees and limits.
#[derive(Debug)]
pub struct StoreWriter {
    env: Env,
    _lock: StoreLock,
    halted: bool,
    blocked: Vec<StoreKind>,
}

impl StoreWriter {
    /// Takes the lock on `dir` and builds a writer with nothing halted or blocked.
    fn lock_dir(
        dir: DataDir,
        fs: Arc<dyn FsOps>,
        validator: Arc<dyn DocumentValidator>,
    ) -> Result<Self, StoreError> {
        let lock = lock::acquire(&dir, &*fs)?;
        Ok(Self {
            env: Env {
                fs,
                dir,
                validator,
                owner_uid: lock.owner_uid(),
            },
            _lock: lock,
            halted: false,
            blocked: Vec::new(),
        })
    }

    /// Locks `dir`, runs startup reconciliation (request §6.5, T01-D8) and returns the writer.
    ///
    /// Under the lock, in this order: leftover temp files of the application's exact temp pattern are
    /// removed; every managed root is read and validated (a malformed or foreign root is *blocked*, never
    /// rewritten, and listed in [`ReconcileReport::malformed_roots`]); runs still `queued`,
    /// `awaiting_approval`, `running` or `cancelling` are set to `interrupted` with `finished_at` set and
    /// written through the replace protocol; pending approvals are counted and left alone; report
    /// manifests without a report file, and report files without a manifest, are reported and not changed.
    ///
    /// Errors: [`StoreError::Locked`] if another instance holds the directory, [`StoreError::PathRejected`]
    /// if the directory fails the ownership/mode checks, [`StoreError::SchemaUnavailable`] if an existing
    /// root cannot be validated, and [`StoreError::Io`] for a storage error (the lock is released and no
    /// writer is returned).
    pub fn open(
        dir: DataDir,
        fs: Arc<dyn FsOps>,
        v: Arc<dyn DocumentValidator>,
    ) -> Result<(Self, ReconcileReport), StoreError> {
        let mut writer = Self::lock_dir(dir, fs, v)?;
        let report = recovery::reconcile(&mut writer)?;
        Ok((writer, report))
    }

    /// Locks `dir` and creates the missing collection roots with empty, validated defaults.
    ///
    /// Creates exactly `repo.json`, `workflows.json`, `approvals.json`, `log.json` and `tools.json`, each
    /// with `updated_at` set to the current time. An existing root is left unchanged and reported in
    /// [`InitReport::already_present`]; `config.json`, `rag/*` and `suppressions.json` are never created
    /// (T01-D7). Existing managed roots are also read, and a malformed or foreign one is blocked exactly
    /// as in [`StoreWriter::open`]. Running `init` again is therefore a no-op that reports every root as
    /// present. If it fails part-way (for example a full disk), the roots created so far remain and a
    /// repeat run completes the rest.
    ///
    /// Errors: those of [`StoreWriter::open`], plus [`StoreError::Invalid`] / [`StoreError::SchemaUnavailable`]
    /// if an empty default does not validate.
    pub fn init(
        dir: DataDir,
        fs: Arc<dyn FsOps>,
        v: Arc<dyn DocumentValidator>,
    ) -> Result<(Self, InitReport), StoreError> {
        let mut writer = Self::lock_dir(dir, fs, v)?;
        let states = writer.scan_roots()?;
        let now = UtcTimestamp::now();
        let mut report = InitReport::default();
        for kind in INIT_ROOTS {
            let present = states
                .iter()
                .any(|(k, state)| *k == kind && !matches!(state, RootState::Absent));
            if present {
                report.already_present.push(kind);
                continue;
            }
            let bytes = writer.env.render(kind, &empty_root(kind, now)?)?;
            writer.env.publish_root(kind, &bytes, false)?;
            report.created.push(kind);
        }
        Ok((writer, report))
    }

    /// Reads every managed root, blocks the malformed ones and returns each root's state.
    pub(crate) fn scan_roots(&mut self) -> Result<Vec<(StoreKind, RootState)>, StoreError> {
        let mut states = Vec::with_capacity(MANAGED_ROOTS.len());
        for kind in MANAGED_ROOTS {
            let state = self.env.classify(kind)?;
            if matches!(state, RootState::Malformed) && !self.blocked.contains(&kind) {
                self.blocked.push(kind);
            }
            states.push((kind, state));
        }
        Ok(states)
    }

    /// A clone of the storage context, for reconciliation.
    pub(crate) fn env(&self) -> Env {
        self.env.clone()
    }

    /// Records a storage error: any [`StoreError::Io`] halts the writer. Other errors pass through.
    fn track<T>(&mut self, result: Result<T, StoreError>) -> Result<T, StoreError> {
        if matches!(result, Err(StoreError::Io { .. })) {
            self.halted = true;
        }
        result
    }

    fn ensure_running(&self) -> Result<(), StoreError> {
        if self.halted {
            Err(StoreError::Halted)
        } else {
            Ok(())
        }
    }

    /// The root path of a kind the application may write: not a report, not `suppressions.json`.
    fn writable_root(kind: StoreKind) -> Result<StorePath, StoreError> {
        if !kind.writable() {
            return Err(StoreError::PathRejected);
        }
        StorePath::root(kind).ok_or(StoreError::PathRejected)
    }

    /// Reads and fully validates the root of `kind` (`Report` is not a root: `PathRejected`).
    ///
    /// Works while the writer is halted, and also on a blocked root, where it returns the validation
    /// problems that caused the block. A missing file is [`StoreError::NotInitialised`].
    pub fn read(&self, kind: StoreKind) -> Result<AnyStore, StoreError> {
        self.env.load_root(kind)
    }

    /// Validates `doc` and replaces its root through the replace protocol (request §6.3): temp file,
    /// sync, previous generation kept as `<name>.prev`, rename, directory sync.
    ///
    /// The caller sets `updated_at`; the writer persists the document as given. Nothing is written if the
    /// document is invalid ([`StoreError::Invalid`]) or the schema is unavailable
    /// ([`StoreError::SchemaUnavailable`]). A root that does not exist yet is created (for example
    /// `config.json`, written first by P03).
    ///
    /// Errors: [`StoreError::Halted`] after an earlier storage error; [`StoreError::PathRejected`] for
    /// `suppressions.json` and reports; [`StoreError::Malformed`] if startup found the existing root
    /// malformed or foreign (restore it explicitly); [`StoreError::Io`] for a storage error, which halts
    /// the writer (the previous file stays intact, except after a directory-sync failure, when the new
    /// content is already in place).
    pub fn replace(&mut self, doc: &AnyStore) -> Result<(), StoreError> {
        self.ensure_running()?;
        let kind = doc.kind();
        Self::writable_root(kind)?;
        if self.blocked.contains(&kind) {
            return Err(StoreError::Malformed(kind));
        }
        let result = self
            .env
            .render(kind, doc)
            .and_then(|bytes| self.env.publish_root(kind, &bytes, true));
        self.track(result)
    }

    /// Validates and writes a new report under `backup/` (never overwrites) and returns its manifest.
    ///
    /// Order, chosen so that a failure leaves as little behind as possible:
    ///
    /// 1. the report is validated as a report document;
    /// 2. the current `log.json` is read, the manifest (`json_written = true`, `md_written = false`,
    ///    `pdf_written = false`, `md_path` / `pdf_path` reserved as `backup/<stem>.md` / `.pdf`,
    ///    `json_digest` = SHA-256 of the exact bytes written) is appended and the new log is validated;
    /// 3. `backup/<stem>.json` is written without overwriting (`<stem>` from
    ///    [`report_stem`](crate::report_name::report_stem));
    /// 4. the new `log.json` is written through the replace protocol.
    ///
    /// Validation of both documents therefore happens before the first byte is written. If step 4 fails
    /// with a storage error, the report file stays (reports are immutable) and the writer halts; at the
    /// next startup the file is reported as a report without a manifest and nothing is deleted or
    /// guessed. A report whose name already exists, on disk or in the log, is refused with
    /// [`StoreError::ReportNameCollision`] and the existing file is untouched.
    ///
    /// Errors: [`StoreError::Halted`]; [`StoreError::Malformed`] if `log.json` was found malformed;
    /// [`StoreError::NotInitialised`] if `log.json` does not exist (run `init`);
    /// [`StoreError::Invalid`] / [`StoreError::SchemaUnavailable`]; [`StoreError::ReportNameCollision`];
    /// [`StoreError::Io`] (halts).
    pub fn write_report(
        &mut self,
        report: &ReportFile,
        slug: &Slug,
    ) -> Result<ReportManifest, StoreError> {
        self.ensure_running()?;
        if self.blocked.contains(&StoreKind::Log) {
            return Err(StoreError::Malformed(StoreKind::Log));
        }
        let result = self.write_report_steps(report, slug);
        self.track(result)
    }

    fn write_report_steps(
        &self,
        report: &ReportFile,
        slug: &Slug,
    ) -> Result<ReportManifest, StoreError> {
        let env = &self.env;
        let report_bytes = env.render(StoreKind::Report, report)?;
        let stem = report_stem(report.generated_at, slug, report.run_id);
        let json_name = format!("{stem}.json");
        let json_path = StorePath::report(&json_name)?;
        let rel = |ext: &str| {
            AppRelPath::parse(&format!("backup/{stem}.{ext}"))
                .map_err(|_| unrepresentable(StoreKind::Log))
        };
        let manifest = ReportManifest {
            report_id: report.report_id,
            run_id: report.run_id,
            repo_id: report.repo_id,
            json_path: rel("json")?,
            md_path: rel("md")?,
            json_written: true,
            md_written: false,
            json_digest: Some(
                Digest::parse(&sha256_hex(&report_bytes))
                    .map_err(|_| unrepresentable(StoreKind::Log))?,
            ),
            generated_at: report.generated_at,
            pdf_path: Some(rel("pdf")?),
            pdf_written: false,
            pdf_digest: None,
            pdf_error_redacted: None,
            pdf_renderer_version: None,
        };
        let AnyStore::Log(mut log) = env.load_root(StoreKind::Log)? else {
            return Err(StoreError::Malformed(StoreKind::Log));
        };
        if log
            .reports
            .iter()
            .any(|m| m.json_path == manifest.json_path)
        {
            return Err(StoreError::ReportNameCollision);
        }
        log.reports.push(manifest.clone());
        log.updated_at = UtcTimestamp::now();
        let log_bytes = env.render(StoreKind::Log, &log)?;

        env.prepare_parent(&json_path)?;
        atomic::write_new(&*env.fs, &env.dir, &json_path, &report_bytes)?;
        env.publish_root(StoreKind::Log, &log_bytes, true)?;
        Ok(manifest)
    }

    /// Moves the current root of `kind` to `quarantine/` and restores the validated `.prev` copy.
    ///
    /// Steps, each failing without changing anything before the first one that mutates:
    ///
    /// 1. `<name>.prev` must exist and pass full validation as a `kind` document (missing →
    ///    [`StoreError::NotInitialised`], invalid → [`StoreError::Invalid`]; nothing is changed);
    /// 2. a current file, if there is one, is moved to `quarantine/<name>.<YYYYMMDDTHHMMSSZ>` without ever
    ///    overwriting: a hard link to the new name (which fails if the name is taken) followed by removal of
    ///    the old name. A name collision, for example two restores in the same second, is refused with
    ///    [`StoreError::PathRejected`] and nothing is moved. `quarantine/` is created private (mode
    ///    `0700`) and an existing one must be private and owned by this process. A current entry that is
    ///    neither a regular file nor a symlink (a directory) cannot be moved this way and is refused with
    ///    [`StoreError::PathRejected`];
    /// 3. the `.prev` bytes are written as the new root through the replace protocol;
    /// 4. the root is unblocked.
    ///
    /// A storage error after step 2 started halts the writer. If the current file was already moved, the
    /// root is then absent, `.prev` is still in place, and calling `restore_previous` again after
    /// reopening completes the job (with `quarantined = false`). Restoring a root that is not malformed
    /// is allowed (an explicit operator action): the valid current file is quarantined like any other.
    ///
    /// Errors: [`StoreError::Halted`]; [`StoreError::PathRejected`] for `suppressions.json`, reports, a
    /// collision or a directory in the way; the validation errors above; [`StoreError::Io`] (halts).
    pub fn restore_previous(&mut self, kind: StoreKind) -> Result<RestoreReport, StoreError> {
        self.ensure_running()?;
        let result = self.restore_steps(kind);
        if result.is_ok() {
            self.blocked.retain(|blocked| *blocked != kind);
        }
        self.track(result)
    }

    fn restore_steps(&self, kind: StoreKind) -> Result<RestoreReport, StoreError> {
        let env = &self.env;
        let path = Self::writable_root(kind)?;
        let prev_bytes = env.read_file(kind, &env.dir.join(&path.previous()))?;
        read::decode(kind, &prev_bytes, &*env.validator)?;

        let current = env.dir.join(&path);
        let quarantined = match env.fs.symlink_metadata(&current) {
            Ok(info) if info.is_regular() || info.is_symlink() => true,
            Ok(_) => return Err(StoreError::PathRejected),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(io_at(WriteStep::Read)(e)),
        };
        if quarantined {
            let quarantine = env.dir.as_path().join(QUARANTINE_DIR);
            env.ensure_private_dir(&quarantine)?;
            let name = format!("{}.{}", path.file_name(), compact_utc(UtcTimestamp::now()));
            let target = quarantine.join(name);
            match env.fs.hard_link(WriteStep::LinkPrevious, &current, &target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(StoreError::PathRejected);
                }
                Err(e) => return Err(io_at(WriteStep::LinkPrevious)(e)),
            }
            env.fs
                .sync_dir(WriteStep::SyncDir, &quarantine)
                .map_err(io_at(WriteStep::SyncDir))?;
            env.fs
                .remove_file(&current)
                .map_err(io_at(WriteStep::Replace))?;
        }
        env.publish_root(kind, &prev_bytes, false)?;
        Ok(RestoreReport { kind, quarantined })
    }

    /// Whether an earlier storage error halted this writer.
    pub fn is_halted(&self) -> bool {
        self.halted
    }
}

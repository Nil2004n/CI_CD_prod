//! Approval lifecycle over the P02 `StoreWriter` (Architecture §7/§10, Data §11 rules 12–14, UI §10; frozen
//! interface and decision table `planning/P03-T01-request.md` §5.4). Owner: W1, T08.
//!
//! Only [`ApprovalService::resolve`] and [`ApprovalService::revoke`], called with an explicit [`Actor`],
//! change an approval's status: repository text, chat answers and pending requests never grant anything.
//! [`ApprovalService::authorise`] is the only producer of an [`Authorisation`]; for a `once` grant it
//! durably records the consumption before returning, so an interruption can lose a grant but never use one
//! twice.

use std::path::Path;
use std::sync::Arc;

use domain::contract::{AppEvent, ApprovalDecision, ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{ApprovalId, RunId};
use domain::states::ApprovalState;
use domain::store::approval::{Approval, ApprovalFile};
use domain::store::log::LogFile;
use domain::store::{AnyStore, ApprovalsStoreType, LogStoreType, SchemaVersion, StoreKind};
use domain::text::RedactedText;
use domain::time::UtcTimestamp;
use persistence::error::StoreError;
use persistence::fs::{FsOps, RealFs};
use persistence::lock;
use persistence::paths::{DataDir, StorePath};
use persistence::read::{self, MAX_DOCUMENT_BYTES, SchemaValidator};
use persistence::writer::StoreWriter;

use crate::dev_store::DevError;
use crate::emit::{Clock, Emitter, IdSource, SystemClock, UrandomIds, to_store_event};
use crate::permission::{
    ArgError, Evaluation, EvaluationContext, NeedReason, PermissionRequest, RefusalReason,
    RequestedData, evaluate, fingerprint,
};

/// Who decides. The requester's own text is `FixedText::ActorRequester`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Actor {
    /// A local user through the developer CLI.
    LocalUserCli,
    /// A local user through the TUI.
    LocalUserTui,
}

/// Proof that an operation may run now. Produced only by [`ApprovalService::authorise`]; not `Clone`.
#[derive(Debug, PartialEq, Eq)]
pub struct Authorisation {
    approval_id: ApprovalId,
    lifetime: domain::contract::ApprovalLifetime,
    operation_fingerprint: domain::store::values::Digest,
}

impl Authorisation {
    /// The approval that authorised the operation.
    pub fn approval_id(&self) -> ApprovalId {
        self.approval_id
    }

    /// Its lifetime (a `once` grant is already consumed).
    pub fn lifetime(&self) -> domain::contract::ApprovalLifetime {
        self.lifetime
    }

    /// Fingerprint of the evaluated operation content (F1–F7).
    ///
    /// This digest excludes other authorization dimensions, including lifetime,
    /// workflow and run bindings, validity, purpose and inference scope. It is
    /// not proof of authorization or a complete request binding on its own.
    pub fn operation_fingerprint(&self) -> &domain::store::values::Digest {
        &self.operation_fingerprint
    }
}

/// Why an approval operation failed. `Display` is fixed English text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApprovalError {
    /// A store failure (classified like the `dev` store commands).
    Store(DevError),
    /// No approval with that ID.
    NotFound,
    /// The approval is not pending.
    NotPending,
    /// Only an approved grant can be revoked.
    NotRevocable,
    /// Workflow scope needs a saved workflow at its visible revision.
    WorkflowScopeUnavailable,
    /// The request can never be granted.
    Refused(RefusalReason),
    /// The request's arguments are malformed.
    Fingerprint(ArgError),
    /// An event could not be emitted.
    Emit,
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "approval storage failed: {e}"),
            Self::NotFound => f.write_str("no approval with that identifier"),
            Self::NotPending => f.write_str("the approval is not pending"),
            Self::NotRevocable => f.write_str("only an approved grant can be revoked"),
            Self::WorkflowScopeUnavailable => {
                f.write_str("workflow scope needs a saved workflow at its current revision")
            }
            Self::Refused(_) => f.write_str("the request can never be granted"),
            Self::Fingerprint(_) => f.write_str("the request's arguments are malformed"),
            Self::Emit => f.write_str("an identifier or an event could not be produced"),
        }
    }
}

impl std::error::Error for ApprovalError {}

/// The result of [`ApprovalService::request`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RequestOutcome {
    /// A valid grant already covers the request (nothing recorded).
    Permitted,
    /// A pending request exists or was recorded.
    Pending(ApprovalId),
    /// The request can never be granted (nothing recorded).
    Refused(RefusalReason),
}

/// The result of [`ApprovalService::authorise`].
#[derive(Debug, PartialEq, Eq)]
pub enum AuthOutcome {
    /// The operation may run now.
    Authorised(Authorisation),
    /// A decision is needed; the pending request's ID.
    NeedsApproval(NeedReason, ApprovalId),
    /// The request can never be granted.
    Refused(RefusalReason),
}

/// The approval lifecycle over one open store writer.
#[derive(Debug)]
pub struct ApprovalService<'a, C: Clock, I: IdSource> {
    writer: &'a mut StoreWriter,
    emitter: &'a mut Emitter<C, I>,
    clock: &'a dyn Clock,
    ids: &'a mut dyn IdSource,
}

/// The result of recording a pending request.
enum Created {
    /// A new pending record was written.
    New(ApprovalId),
    /// The request would produce a record the store rejects; nothing was written.
    Refused(RefusalReason),
}

impl<'a, C: Clock, I: IdSource> ApprovalService<'a, C, I> {
    /// A service writing through `writer`, emitting through `emitter`, timing with `clock` and drawing
    /// approval IDs from `ids`.
    pub fn new(
        writer: &'a mut StoreWriter,
        emitter: &'a mut Emitter<C, I>,
        clock: &'a dyn Clock,
        ids: &'a mut dyn IdSource,
    ) -> Self {
        Self {
            writer,
            emitter,
            clock,
            ids,
        }
    }

    /// Records a pending request unless a valid grant already covers it, an identical request is pending,
    /// or it is refused.
    ///
    /// A deferred capability is refused (`InvalidRequest`) before anything is read or written, because the
    /// store rejects approvals for it (rule D18-06a). Otherwise the outcome of [`evaluate`] is mapped:
    /// `Permitted` is `Permitted` and records nothing (a `once` grant is not consumed here; only
    /// [`ApprovalService::authorise`] consumes), `Refused` is `Refused`, an identical pending request is
    /// reused, and anything else records one new pending request and emits one `ApprovalRequested`.
    pub fn request(
        &mut self,
        req: &PermissionRequest,
        current_revision: Option<u32>,
    ) -> Result<RequestOutcome, ApprovalError> {
        if req.capability.is_deferred() {
            return Ok(RequestOutcome::Refused(RefusalReason::InvalidRequest));
        }
        let now = self.clock.now();
        let file = load_approvals(self.writer, now)?;
        let ctx = EvaluationContext {
            now,
            current_workflow_revision: current_revision,
        };
        match evaluate(req, &file.approvals, &ctx) {
            Evaluation::Permitted(_) => Ok(RequestOutcome::Permitted),
            Evaluation::Refused(reason) => Ok(RequestOutcome::Refused(reason)),
            Evaluation::NeedsApproval(NeedReason::AlreadyPending(id)) => {
                Ok(RequestOutcome::Pending(id))
            }
            Evaluation::NeedsApproval(_) => match self.create_pending(req, file, now)? {
                Created::New(id) => Ok(RequestOutcome::Pending(id)),
                Created::Refused(reason) => Ok(RequestOutcome::Refused(reason)),
            },
        }
    }

    /// Decides a pending request (decision table, request §5.4).
    ///
    /// `approve_once` approves with lifetime `once` (`run` for a `run` request); `approve_workflow_scope`
    /// approves with lifetime `workflow_revision` and `run_id = null`, and only when the request is bound
    /// to a workflow at `current_revision` (otherwise [`ApprovalError::WorkflowScopeUnavailable`] and
    /// nothing is written); `deny` denies. `decided_at` is the clock reading and `actor` the fixed text of
    /// `actor`. A grant's `expires_at` is `decided_at` plus the request's `validity_seconds`, which the
    /// pending record carries as the span from `requested_at` to its own `expires_at`.
    pub fn resolve(
        &mut self,
        id: ApprovalId,
        decision: ApprovalDecision,
        actor: Actor,
        current_revision: Option<u32>,
    ) -> Result<ApprovalState, ApprovalError> {
        let now = self.clock.now();
        let mut file = load_approvals(self.writer, now)?;
        let record = file
            .approvals
            .iter_mut()
            .find(|a| a.approval_id == id)
            .ok_or(ApprovalError::NotFound)?;
        if record.status != ApprovalState::Pending {
            return Err(ApprovalError::NotPending);
        }
        let state = match decision {
            ApprovalDecision::Deny => {
                record.status = ApprovalState::Denied;
                record.expires_at = None;
                ApprovalState::Denied
            }
            ApprovalDecision::ApproveOnce => {
                record.lifetime = match record.lifetime {
                    ApprovalLifetime::Run => ApprovalLifetime::Run,
                    ApprovalLifetime::Once | ApprovalLifetime::WorkflowRevision => {
                        ApprovalLifetime::Once
                    }
                };
                record.expires_at = grant_expiry(record.requested_at, record.expires_at, now)?;
                record.status = ApprovalState::Approved;
                ApprovalState::Approved
            }
            ApprovalDecision::ApproveWorkflowScope => {
                let at_current_revision = record.workflow_id.is_some()
                    && current_revision.is_some_and(|r| r >= 1)
                    && record.workflow_revision == current_revision.map(u64::from);
                if !at_current_revision {
                    return Err(ApprovalError::WorkflowScopeUnavailable);
                }
                record.lifetime = ApprovalLifetime::WorkflowRevision;
                record.run_id = None;
                record.expires_at = grant_expiry(record.requested_at, record.expires_at, now)?;
                record.status = ApprovalState::Approved;
                ApprovalState::Approved
            }
        };
        record.decided_at = Some(now);
        record.actor = actor_text(actor);
        record.consumed = false;
        self.write_approvals(file, now)?;
        self.emit(AppEvent::ApprovalResolved {
            approval_id: id,
            state,
        })?;
        Ok(state)
    }

    /// Revokes an approved grant. The record keeps its original `decided_at`; `actor` becomes the revoker.
    pub fn revoke(&mut self, id: ApprovalId, actor: Actor) -> Result<(), ApprovalError> {
        let now = self.clock.now();
        let mut file = load_approvals(self.writer, now)?;
        let record = file
            .approvals
            .iter_mut()
            .find(|a| a.approval_id == id)
            .ok_or(ApprovalError::NotFound)?;
        if record.status != ApprovalState::Approved {
            return Err(ApprovalError::NotRevocable);
        }
        record.status = ApprovalState::Revoked;
        record.actor = actor_text(actor);
        self.write_approvals(file, now)?;
        self.emit(AppEvent::ApprovalRevoked { approval_id: id })
    }

    /// Marks approved grants whose `expires_at` has passed as `expired`; returns how many.
    ///
    /// A grant whose `expires_at` equals the clock reading is expired (the evaluator treats it so). Pending,
    /// denied, revoked and already expired records and grants without `expires_at` are untouched. Emits no
    /// event (the contract has none).
    pub fn expire_due(&mut self) -> Result<u32, ApprovalError> {
        let now = self.clock.now();
        self.expire_where(now, |a| a.expires_at.is_some_and(|e| e <= now))
    }

    /// Marks every approved `run`-lifetime grant of `run_id` as `expired` (a run grant ends with its run);
    /// returns how many. Grants of other runs and of other lifetimes are untouched. Emits no event.
    pub fn expire_run(&mut self, run_id: RunId) -> Result<u32, ApprovalError> {
        let now = self.clock.now();
        self.expire_where(now, |a| {
            a.lifetime == ApprovalLifetime::Run && a.run_id == Some(run_id)
        })
    }

    /// Evaluates `req`; on a match returns an [`Authorisation`] (consuming a `once` grant durably first),
    /// otherwise records or reuses a pending request, or refuses.
    ///
    /// For a `once` grant the record is rewritten with `consumed = true` before `Authorised` is returned; a
    /// failed write is an `Err` and no [`Authorisation`] exists. After an interruption the grant is
    /// therefore either consumed (and never granted again) or unconsumed (and its caller never received
    /// `Authorised`). `run` and `workflow_revision` grants are not consumed. A request without a matching
    /// grant records (or reuses) a pending request and returns `NeedsApproval` with the evaluator's reason
    /// and that request's ID; a pending request never authorises.
    pub fn authorise(
        &mut self,
        req: &PermissionRequest,
        current_revision: Option<u32>,
    ) -> Result<AuthOutcome, ApprovalError> {
        if req.capability.is_deferred() {
            return Ok(AuthOutcome::Refused(RefusalReason::InvalidRequest));
        }
        let now = self.clock.now();
        let mut file = load_approvals(self.writer, now)?;
        let ctx = EvaluationContext {
            now,
            current_workflow_revision: current_revision,
        };
        match evaluate(req, &file.approvals, &ctx) {
            Evaluation::Permitted(grant) => {
                let approval_id = grant.approval_id();
                let lifetime = grant.lifetime();
                let operation_fingerprint = fingerprint(req).map_err(ApprovalError::Fingerprint)?;
                if lifetime == ApprovalLifetime::Once {
                    let record = file
                        .approvals
                        .iter_mut()
                        .find(|a| a.approval_id == approval_id)
                        .ok_or(ApprovalError::NotFound)?;
                    record.consumed = true;
                    self.write_approvals(file, now)?;
                }
                Ok(AuthOutcome::Authorised(Authorisation {
                    approval_id,
                    lifetime,
                    operation_fingerprint,
                }))
            }
            Evaluation::Refused(reason) => Ok(AuthOutcome::Refused(reason)),
            Evaluation::NeedsApproval(reason @ NeedReason::AlreadyPending(id)) => {
                Ok(AuthOutcome::NeedsApproval(reason, id))
            }
            Evaluation::NeedsApproval(reason) => match self.create_pending(req, file, now)? {
                Created::New(id) => Ok(AuthOutcome::NeedsApproval(reason, id)),
                Created::Refused(refusal) => Ok(AuthOutcome::Refused(refusal)),
            },
        }
    }

    /// Records `req` as a new pending approval in `file` and emits `ApprovalRequested`.
    ///
    /// `Refused(InvalidRequest)` (nothing written) if the record would break a store rule that the
    /// evaluator does not look at: a `model_transmission` request without a data category (D11-16), a
    /// workflow revision of 0 (D11-12) or a validity that leaves the representable time range. The pending
    /// record's `expires_at` is `requested_at + validity_seconds` when the request has a validity, so that
    /// the validity survives until the decision; it is not a grant end while the record is pending.
    fn create_pending(
        &mut self,
        req: &PermissionRequest,
        mut file: ApprovalFile,
        now: UtcTimestamp,
    ) -> Result<Created, ApprovalError> {
        let mut data_categories: Vec<DataCategory> = Vec::new();
        for item in &req.data {
            if let RequestedData::Category(c) = item
                && !data_categories.contains(c)
            {
                data_categories.push(*c);
            }
        }
        if (req.capability == domain::vocab::Capability::ModelTransmission
            && data_categories.is_empty())
            || req.workflow.as_ref().is_some_and(|w| w.revision == 0)
        {
            return Ok(Created::Refused(RefusalReason::InvalidRequest));
        }
        let expires_at = match req.validity_seconds {
            None => None,
            Some(seconds) => {
                match UtcTimestamp::from_unix(
                    now.unix_seconds().saturating_add(i64::from(seconds)),
                    now.subsec_nanos(),
                ) {
                    Ok(t) => Some(t),
                    Err(_) => return Ok(Created::Refused(RefusalReason::InvalidRequest)),
                }
            }
        };
        let operation_fingerprint = fingerprint(req).map_err(ApprovalError::Fingerprint)?;
        let approval_id =
            ApprovalId::from_random_bytes(self.ids.next_bytes().map_err(|_| ApprovalError::Emit)?);
        let mut repo_ids = Vec::new();
        for repo in &req.repo_ids {
            if !repo_ids.contains(repo) {
                repo_ids.push(*repo);
            }
        }
        let mut destinations = Vec::new();
        for destination in &req.destinations {
            let stored = destination.to_store();
            if !destinations.contains(&stored) {
                destinations.push(stored);
            }
        }
        let record = Approval {
            approval_id,
            status: ApprovalState::Pending,
            capability: req.capability,
            workflow_id: req.workflow.as_ref().map(|w| w.workflow_id),
            workflow_revision: req.workflow.as_ref().map(|w| u64::from(w.revision)),
            run_id: req.run_id,
            repo_ids,
            purpose: req.purpose.clone(),
            operation_fingerprint,
            destinations,
            resource_constraints: req.resource_constraints.clone(),
            data_categories,
            lifetime: req.lifetime,
            requested_at: now,
            decided_at: None,
            expires_at,
            actor: FixedText::ActorRequester.redacted(),
            consumed: false,
            inference_scope: req.inference_scope.clone(),
        };
        let event = AppEvent::ApprovalRequested {
            approval_id,
            capability: record.capability,
            purpose: record.purpose.clone(),
            workflow_id: record.workflow_id,
            workflow_revision: req.workflow.as_ref().map(|w| w.revision),
            repo_ids: record.repo_ids.clone(),
            destinations: record
                .destinations
                .iter()
                .map(domain::store::values::Destination::to_redacted)
                .collect(),
            data_categories: record.data_categories.clone(),
            lifetime: record.lifetime,
        };
        file.approvals.push(record);
        self.write_approvals(file, now)?;
        self.emit(event)?;
        Ok(Created::New(approval_id))
    }

    /// Expires every approved record `due` selects; one write if any, none otherwise.
    fn expire_where(
        &mut self,
        now: UtcTimestamp,
        due: impl Fn(&Approval) -> bool,
    ) -> Result<u32, ApprovalError> {
        let mut file = load_approvals(self.writer, now)?;
        let mut count = 0u32;
        for record in &mut file.approvals {
            if record.status == ApprovalState::Approved && due(record) {
                record.status = ApprovalState::Expired;
                count = count.saturating_add(1);
            }
        }
        if count > 0 {
            self.write_approvals(file, now)?;
        }
        Ok(count)
    }

    /// Writes `file` as the new `approvals.json` (`updated_at` = `now`).
    fn write_approvals(
        &mut self,
        mut file: ApprovalFile,
        now: UtcTimestamp,
    ) -> Result<(), ApprovalError> {
        file.updated_at = now;
        self.writer
            .replace(&AnyStore::Approvals(file))
            .map_err(store_error)
    }

    /// Emits `event` and appends its store form to `log.json`. A failure here leaves the approvals write
    /// that came before it in place; a storage failure halts the writer (reported by the next call).
    fn emit(&mut self, event: AppEvent) -> Result<(), ApprovalError> {
        let envelope = self.emitter.emit(event).map_err(|_| ApprovalError::Emit)?;
        let stored = to_store_event(&envelope).ok_or(ApprovalError::Emit)?;
        let now = self.clock.now().max(envelope.at);
        let mut log = match self.writer.read(StoreKind::Log) {
            Ok(AnyStore::Log(log)) => log,
            Ok(_) => return Err(ApprovalError::Emit),
            Err(StoreError::NotInitialised(_)) => LogFile {
                schema_version: SchemaVersion,
                store_type: LogStoreType,
                updated_at: now,
                runs: Vec::new(),
                events: Vec::new(),
                reports: Vec::new(),
            },
            Err(e) => return Err(store_error(e)),
        };
        log.events.push(stored);
        log.updated_at = now;
        self.writer
            .replace(&AnyStore::Log(log))
            .map_err(store_error)
    }
}

/// The classification of a store error: a document that fails validation is `Malformed`, everything else as
/// the `dev` store commands classify it.
fn store_error(e: StoreError) -> ApprovalError {
    match e {
        StoreError::Invalid(kind, _) => ApprovalError::Store(DevError::Malformed(kind)),
        other => ApprovalError::Store(DevError::from(other)),
    }
}

/// The current `approvals.json`; a root that does not exist yet reads as an empty ledger (the first write
/// creates it).
fn load_approvals(writer: &StoreWriter, now: UtcTimestamp) -> Result<ApprovalFile, ApprovalError> {
    match writer.read(StoreKind::Approvals) {
        Ok(AnyStore::Approvals(file)) => Ok(file),
        Ok(_) => Err(ApprovalError::Store(DevError::Refused)),
        Err(StoreError::NotInitialised(_)) => Ok(ApprovalFile {
            schema_version: SchemaVersion,
            store_type: ApprovalsStoreType,
            updated_at: now,
            approvals: Vec::new(),
        }),
        Err(e) => Err(store_error(e)),
    }
}

/// The fixed `actor` text of `actor`.
fn actor_text(actor: Actor) -> RedactedText {
    match actor {
        Actor::LocalUserCli => FixedText::ActorLocalUserCli.redacted(),
        Actor::LocalUserTui => FixedText::ActorLocalUserTui.redacted(),
    }
}

/// The end of a grant decided at `decided_at`: the span between `requested_at` and the pending record's
/// `expires_at` (the request's validity) added to `decided_at`; `None` if the request had no validity.
fn grant_expiry(
    requested_at: UtcTimestamp,
    pending_expires_at: Option<UtcTimestamp>,
    decided_at: UtcTimestamp,
) -> Result<Option<UtcTimestamp>, ApprovalError> {
    let Some(end) = pending_expires_at else {
        return Ok(None);
    };
    let span = end
        .unix_seconds()
        .saturating_sub(requested_at.unix_seconds());
    UtcTimestamp::from_unix(
        decided_at.unix_seconds().saturating_add(span),
        decided_at.subsec_nanos(),
    )
    .map(Some)
    .map_err(|_| ApprovalError::Refused(RefusalReason::InvalidRequest))
}

/// Opens the store in `dir` for writing (startup reconciliation included, like `dev store recover`).
fn open_writer(dir: &Path) -> Result<StoreWriter, ApprovalError> {
    let fs: Arc<dyn FsOps> = Arc::new(RealFs);
    let data = DataDir::new(&*fs, dir).map_err(store_error)?;
    let (writer, _report) =
        StoreWriter::open(data, fs, Arc::new(SchemaValidator::new())).map_err(store_error)?;
    Ok(writer)
}

/// CLI entry point: every approval in `dir`, in store order.
///
/// Read-only: takes the data-directory lock and reads `approvals.json` through the validating read pipeline;
/// no reconciliation, nothing written. A store without `approvals.json` lists nothing.
pub fn list(dir: &Path) -> Result<Vec<Approval>, ApprovalError> {
    let fs = RealFs;
    let data = DataDir::new(&fs, dir).map_err(store_error)?;
    let _held = lock::acquire(&data, &fs).map_err(store_error)?;
    let path =
        StorePath::root(StoreKind::Approvals).ok_or(ApprovalError::Store(DevError::Refused))?;
    let file = data.join(&path);
    // Classified exactly like the writer's read path (`persistence::writer`): a file that is not a regular
    // file (directory, symlink, device) is `Malformed`, one over the size bound is `Malformed` (the writer
    // path reports it as `Invalid`, which `store_error` maps to `Malformed`), a missing one lists nothing,
    // anything else is a storage failure.
    match fs.symlink_metadata(&file) {
        Ok(info) if info.is_regular() => {}
        Ok(_) => return Err(store_error(StoreError::Malformed(StoreKind::Approvals))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ApprovalError::Store(DevError::Storage)),
    }
    let bytes = match fs.read(&file, MAX_DOCUMENT_BYTES) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::FileTooLarge => {
            return Err(ApprovalError::Store(DevError::Malformed(
                StoreKind::Approvals,
            )));
        }
        Err(_) => return Err(ApprovalError::Store(DevError::Storage)),
    };
    match read::decode(StoreKind::Approvals, &bytes, &SchemaValidator::new()) {
        Ok(AnyStore::Approvals(file)) => Ok(file.approvals),
        Ok(_) => Err(ApprovalError::Store(DevError::Refused)),
        Err(e) => Err(store_error(e)),
    }
}

/// CLI entry point: decides approval `id` in `dir` (current revision read from `workflows.json`).
///
/// The revision is that of the approval's workflow in `workflows.json`; no workflow ID on the approval, no
/// such workflow or no `workflows.json` gives `None`. An unknown `id` is [`ApprovalError::NotFound`].
pub fn resolve(
    dir: &Path,
    id: ApprovalId,
    decision: ApprovalDecision,
    actor: Actor,
) -> Result<ApprovalState, ApprovalError> {
    let mut writer = open_writer(dir)?;
    let clock = SystemClock;
    let current_revision = {
        let file = load_approvals(&writer, clock.now())?;
        let workflow_id = file
            .approvals
            .iter()
            .find(|a| a.approval_id == id)
            .and_then(|a| a.workflow_id);
        match workflow_id {
            None => None,
            Some(workflow_id) => match writer.read(StoreKind::Workflows) {
                Ok(AnyStore::Workflows(workflows)) => workflows
                    .workflows
                    .iter()
                    .find(|w| w.workflow_id == workflow_id)
                    .and_then(|w| u32::try_from(w.revision).ok()),
                Ok(_) => return Err(ApprovalError::Store(DevError::Refused)),
                Err(StoreError::NotInitialised(_)) => None,
                Err(e) => return Err(store_error(e)),
            },
        }
    };
    let mut emitter = Emitter::new(SystemClock, UrandomIds);
    let mut ids = UrandomIds;
    ApprovalService::new(&mut writer, &mut emitter, &clock, &mut ids).resolve(
        id,
        decision,
        actor,
        current_revision,
    )
}

/// CLI entry point: revokes approval `id` in `dir`.
pub fn revoke(dir: &Path, id: ApprovalId, actor: Actor) -> Result<(), ApprovalError> {
    let mut writer = open_writer(dir)?;
    let clock = SystemClock;
    let mut emitter = Emitter::new(SystemClock, UrandomIds);
    let mut ids = UrandomIds;
    ApprovalService::new(&mut writer, &mut emitter, &clock, &mut ids).revoke(id, actor)
}

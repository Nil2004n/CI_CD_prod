//! Approval lifecycle over the P02 `StoreWriter` (Architecture §7/§10, Data §11 rules 12–14, UI §10; frozen
//! interface and decision table `planning/P03-T01-request.md` §5.4). Owner: W1, T08.
//!
//! Only [`ApprovalService::resolve`] and [`ApprovalService::revoke`], called with an explicit [`Actor`],
//! change an approval's status: repository text, chat answers and pending requests never grant anything.
//! [`ApprovalService::authorise`] is the only producer of an [`Authorisation`]; for a `once` grant it
//! durably records the consumption before returning, so an interruption can lose a grant but never use one
//! twice.

use std::path::Path;

use domain::contract::ApprovalDecision;
use domain::ids::ApprovalId;
use domain::states::ApprovalState;
use domain::store::approval::Approval;
use persistence::writer::StoreWriter;

use crate::dev_store::DevError;
use crate::emit::{Clock, Emitter, IdSource};
use crate::permission::{ArgError, NeedReason, PermissionRequest, RefusalReason};

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
        let _ = (self, f);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
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
    pub fn request(
        &mut self,
        req: &PermissionRequest,
        current_revision: Option<u32>,
    ) -> Result<RequestOutcome, ApprovalError> {
        let _ = (
            req,
            current_revision,
            &self.writer,
            &self.emitter,
            &self.clock,
            &self.ids,
        );
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
        }
    }

    /// Decides a pending request (decision table, request §5.4).
    pub fn resolve(
        &mut self,
        id: ApprovalId,
        decision: ApprovalDecision,
        actor: Actor,
        current_revision: Option<u32>,
    ) -> Result<ApprovalState, ApprovalError> {
        let _ = (id, decision, actor, current_revision);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
        }
    }

    /// Revokes an approved grant.
    pub fn revoke(&mut self, id: ApprovalId, actor: Actor) -> Result<(), ApprovalError> {
        let _ = (id, actor);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
        }
    }

    /// Marks approved grants whose `expires_at` has passed as `expired`; returns how many.
    pub fn expire_due(&mut self) -> Result<u32, ApprovalError> {
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
        }
    }

    /// Evaluates `req`; on a match returns an [`Authorisation`] (consuming a `once` grant durably first),
    /// otherwise records or reuses a pending request, or refuses.
    pub fn authorise(
        &mut self,
        req: &PermissionRequest,
        current_revision: Option<u32>,
    ) -> Result<AuthOutcome, ApprovalError> {
        let _ = (req, current_revision);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T08")
        }
    }
}

/// CLI entry point: every approval in `dir`, in store order.
pub fn list(dir: &Path) -> Result<Vec<Approval>, ApprovalError> {
    let _ = dir;
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T08")
    }
}

/// CLI entry point: decides approval `id` in `dir` (current revision read from `workflows.json`).
pub fn resolve(
    dir: &Path,
    id: ApprovalId,
    decision: ApprovalDecision,
    actor: Actor,
) -> Result<ApprovalState, ApprovalError> {
    let _ = (dir, id, decision, actor);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T08")
    }
}

/// CLI entry point: revokes approval `id` in `dir`.
pub fn revoke(dir: &Path, id: ApprovalId, actor: Actor) -> Result<(), ApprovalError> {
    let _ = (dir, id, actor);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T08")
    }
}

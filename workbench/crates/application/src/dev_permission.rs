//! `dev permission-check` (`planning/P03-T01-request.md` §5.7, T01-D6). Owner: W2, T10.
//!
//! Decodes one synthetic permission request (JSON on standard input, a `dev`-only DTO), evaluates it against
//! the approvals in the data directory and, when approval is needed, records a pending request. A `once`
//! grant that matches is consumed. The purpose recorded is always `FixedText::PurposeDeveloperCheck`.
//!
//! This module never evaluates a request itself and never inspects a grant: it builds a
//! [`PermissionRequest`] and hands it to [`ApprovalService::authorise`], whose result is the only decision
//! (D01 H1: only `approvals.rs` acts on a permitted evaluation).
//!
//! Resource constraints are bounded (16 entries of 512 bytes) and must be plain one-line text; anything else
//! is a `Decode` rejection (P03-D02 A1).
//!
//! The request document is a developer tool, not a stable interface. Unknown members, an unknown capability,
//! lifetime or data category and every malformed identifier are rejected. Argument values are hashed into the
//! operation fingerprint and never stored, printed or logged; the document is not echoed in any result.

use std::path::Path;
use std::sync::Arc;

use domain::contract::{ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{RepoId, RunId, WorkflowId};
use domain::store::provider::InferenceScope;
use domain::store::{AnyStore, StoreKind};
use domain::text::sanitise;
use domain::vocab::Capability;
use persistence::error::StoreError;
use persistence::fs::{FsOps, RealFs};
use persistence::paths::DataDir;
use persistence::read::SchemaValidator;
use persistence::writer::StoreWriter;
use serde::Deserialize;

use crate::approvals::{ApprovalError, ApprovalService, AuthOutcome};
use crate::dev_store::DevError;
use crate::emit::{Emitter, SystemClock, UrandomIds};
use crate::permission::{
    ArgName, NormalisedDestination, OperationArg, OperationName, PermissionRequest, RequestedData,
    WorkflowBinding,
};

/// Largest accepted request document, in bytes.
pub const MAX_REQUEST_BYTES: usize = 65_536;
/// Most resource constraints a request may carry (P03-D02 A1).
pub const MAX_CONSTRAINTS: usize = 16;
/// Longest resource constraint, in bytes (P03-D02 A1).
pub const MAX_CONSTRAINT_BYTES: usize = 512;

/// Why the request document was rejected before evaluation. Fieldless.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RequestDocError {
    /// Over [`MAX_REQUEST_BYTES`] or unreadable.
    Read,
    /// Not a valid request document.
    Decode,
    /// A destination failed normalisation.
    Destination,
    /// An operation or argument name failed validation.
    Name,
}

/// The outcome of one `dev permission-check`.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// The document was rejected.
    Rejected(RequestDocError),
    /// The evaluation result.
    Evaluated(AuthOutcome),
}

/// One operation argument of the request document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgDoc {
    name: String,
    value: String,
}

/// The workflow binding of the request document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowDoc {
    workflow_id: WorkflowId,
    revision: u32,
}

/// The one data item that is not a category: a request for raw secret values, which is always refused.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawSecretTag {
    RawSecretValues,
}

/// One entry of `data`: a disclosable category, or the (refused) raw secret values.
#[derive(Deserialize)]
#[serde(untagged)]
enum DataDoc {
    Category(DataCategory),
    Raw(RawSecretTag),
}

/// The request document (`dev`-only DTO). Members default to empty; `capability`, `operation` and `lifetime`
/// are required.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestDoc {
    capability: Capability,
    operation: String,
    #[serde(default)]
    arguments: Vec<ArgDoc>,
    #[serde(default)]
    workflow: Option<WorkflowDoc>,
    #[serde(default)]
    run_id: Option<RunId>,
    #[serde(default)]
    repo_ids: Vec<RepoId>,
    #[serde(default)]
    destinations: Vec<String>,
    #[serde(default)]
    resource_constraints: Vec<String>,
    #[serde(default)]
    data: Vec<DataDoc>,
    lifetime: ApprovalLifetime,
    #[serde(default)]
    validity_seconds: Option<u32>,
    #[serde(default)]
    inference_scope: Option<InferenceScope>,
}

/// Whether a constraint is acceptable text: at most [`MAX_CONSTRAINT_BYTES`] bytes, no character below
/// U+0020 (line breaks and tabs included), no U+007F, and nothing that `sanitise` would escape. A constraint
/// is stored and later shown on a line of its own, so it must be one visible line (P03-D02 A1).
fn constraint_is_plain(constraint: &str) -> bool {
    constraint.len() <= MAX_CONSTRAINT_BYTES
        && !constraint.chars().any(|c| c < '\u{20}' || c == '\u{7f}')
        && sanitise(constraint).as_str() == constraint
}

/// Turns the document bytes into a request: size bound, decoding, name and destination validation, and the
/// fixed purpose. Nothing is read from or written to disk.
fn decode(doc: &[u8]) -> Result<(PermissionRequest, Option<WorkflowId>), RequestDocError> {
    if doc.len() > MAX_REQUEST_BYTES {
        return Err(RequestDocError::Read);
    }
    let parsed: RequestDoc = serde_json::from_slice(doc).map_err(|_| RequestDocError::Decode)?;
    if parsed.resource_constraints.len() > MAX_CONSTRAINTS
        || !parsed
            .resource_constraints
            .iter()
            .all(|c| constraint_is_plain(c))
    {
        return Err(RequestDocError::Decode);
    }
    let operation = OperationName::parse(&parsed.operation).map_err(|_| RequestDocError::Name)?;
    let mut arguments = Vec::with_capacity(parsed.arguments.len());
    for argument in parsed.arguments {
        arguments.push(OperationArg {
            name: ArgName::parse(&argument.name).map_err(|_| RequestDocError::Name)?,
            value: argument.value,
        });
    }
    let mut destinations = Vec::with_capacity(parsed.destinations.len());
    for destination in &parsed.destinations {
        destinations.push(
            NormalisedDestination::parse(destination).map_err(|_| RequestDocError::Destination)?,
        );
    }
    let workflow_id = parsed.workflow.as_ref().map(|w| w.workflow_id);
    let request = PermissionRequest {
        capability: parsed.capability,
        operation,
        arguments,
        workflow: parsed.workflow.map(|w| WorkflowBinding {
            workflow_id: w.workflow_id,
            revision: w.revision,
        }),
        run_id: parsed.run_id,
        repo_ids: parsed.repo_ids,
        destinations,
        resource_constraints: parsed
            .resource_constraints
            .iter()
            .map(|c| sanitise(c))
            .collect(),
        data: parsed
            .data
            .into_iter()
            .map(|item| match item {
                DataDoc::Category(category) => RequestedData::Category(category),
                DataDoc::Raw(RawSecretTag::RawSecretValues) => RequestedData::RawSecretValues,
            })
            .collect(),
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: parsed.lifetime,
        validity_seconds: parsed.validity_seconds,
        inference_scope: parsed.inference_scope,
    };
    Ok((request, workflow_id))
}

/// The classification of a store error, as `approvals` classifies it: a document that fails validation is
/// `Malformed`, everything else as the `dev` store commands classify it.
fn store_error(error: StoreError) -> ApprovalError {
    match error {
        StoreError::Invalid(kind, _) => ApprovalError::Store(DevError::Malformed(kind)),
        other => ApprovalError::Store(DevError::from(other)),
    }
}

/// Runs `dev permission-check` against the store in `dir` with the request document `doc`.
///
/// The document is checked before the store is touched (size, decoding, names, destinations). The store is
/// then opened for writing (reconciliation included, like `approvals::resolve`), the request's workflow
/// revision is read from `workflows.json` (no workflow, no such workflow or no file gives none) and
/// [`ApprovalService::authorise`] decides: a matching `once` grant is consumed, a request without a matching
/// grant is recorded (or found) as pending.
///
/// Errors: a store failure, or arguments that cannot be fingerprinted, as [`ApprovalError`]. A rejected
/// document is not an error but [`CheckOutcome::Rejected`].
pub fn check(dir: &Path, doc: &[u8]) -> Result<CheckOutcome, ApprovalError> {
    let (request, workflow_id) = match decode(doc) {
        Ok(decoded) => decoded,
        Err(reason) => return Ok(CheckOutcome::Rejected(reason)),
    };
    let fs: Arc<dyn FsOps> = Arc::new(RealFs);
    let data = DataDir::new(&*fs, dir).map_err(store_error)?;
    let (mut writer, _report) =
        StoreWriter::open(data, fs, Arc::new(SchemaValidator::new())).map_err(store_error)?;
    let current_revision = match workflow_id {
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
    };
    let clock = SystemClock;
    let mut emitter = Emitter::new(SystemClock, UrandomIds);
    let mut ids = UrandomIds;
    let outcome = ApprovalService::new(&mut writer, &mut emitter, &clock, &mut ids)
        .authorise(&request, current_revision)?;
    Ok(CheckOutcome::Evaluated(outcome))
}

//! Pure checks for the additional 1.5 Git approval context.
//!
//! These checks do not replace the generic approval rules, schema validation,
//! destination normalization, or request reconstruction and authorization.

use domain::contract::{ApprovalLifetime, DataCategory};
use domain::store::approval15::{Approval, GitApprovalStage};
use domain::store::repository15::valid_target_reference;
use domain::vocab::Capability;

/// A fixed, input-free classification of an invalid Git approval context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitContextError {
    Capability,
    Reference,
    RepositoryScope,
    Binding,
    Destination,
    Lifetime,
    Constraints,
    DataCategories,
    Expiry,
}

/// Check the additional constraints on a non-null Git context.
///
/// Null contexts remain valid here for historical and non-Git records; they
/// cannot authorize a Git operation. The caller must enforce that separately.
pub fn validate_git_context(approval: &Approval) -> Result<(), GitContextError> {
    let Some(context) = &approval.git_approval_context else {
        return Ok(());
    };
    let expected = match context.stage {
        GitApprovalStage::Discover | GitApprovalStage::Clone => Capability::GitAcquire,
        GitApprovalStage::Update => Capability::GitUpdate,
    };
    if approval.capability != expected {
        return Err(GitContextError::Capability);
    }
    let reference_valid = match (context.stage, context.ref_kind, &context.ref_value) {
        (GitApprovalStage::Discover, None, None) => true,
        (GitApprovalStage::Clone | GitApprovalStage::Update, Some(kind), Some(value)) => {
            valid_target_reference(kind, value)
        }
        _ => false,
    };
    if !reference_valid {
        return Err(GitContextError::Reference);
    }
    if approval.repo_ids.as_slice() != [context.repo_id] {
        return Err(GitContextError::RepositoryScope);
    }
    if approval.workflow_id.is_some()
        || approval.workflow_revision.is_some()
        || approval.run_id.is_some()
        || approval.inference_scope.is_some()
    {
        return Err(GitContextError::Binding);
    }
    if approval.destinations.len() != 1 {
        return Err(GitContextError::Destination);
    }
    if approval.lifetime != ApprovalLifetime::Once {
        return Err(GitContextError::Lifetime);
    }
    if !approval.resource_constraints.is_empty() {
        return Err(GitContextError::Constraints);
    }
    if approval.data_categories.as_slice() != [DataCategory::RepositoryMetadata] {
        return Err(GitContextError::DataCategories);
    }
    if approval.expires_at.is_some() {
        return Err(GitContextError::Expiry);
    }
    Ok(())
}

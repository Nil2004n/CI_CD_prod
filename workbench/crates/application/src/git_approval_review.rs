//! Comparison token for the P04 Git approval review contract.
//!
//! This deterministic token is not secret and does not authenticate a user,
//! prove presentation, authorize an operation, or establish administrator consent.

use domain::{
    ids::ApprovalId,
    store::values::{Digest, ValueError},
};
use sha2::{Digest as _, Sha256};

/// The operation stage to be presented for review.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitReviewStage {
    Discover,
    Clone,
    Update,
}

impl GitReviewStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Clone => "clone",
            Self::Update => "update",
        }
    }
}

/// Compute the approved v1 comparison token for an already validated request.
/// Callers must reconstruct and verify the request under the required locks.
pub fn review_token(
    id: ApprovalId,
    stage: GitReviewStage,
    fingerprint: &Digest,
) -> Result<Digest, ValueError> {
    let mut hasher = Sha256::new();
    hasher.update(b"pwt-git-approval-review\0v1\0");
    hasher.update(id.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(stage.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(fingerprint.as_str().as_bytes());
    let text: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Digest::parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_token_matches_contract_vector_and_changes_with_each_binding() {
        let id = ApprovalId::parse("00000000-0000-4000-8000-000000000001").unwrap();
        let other_id = ApprovalId::parse("00000000-0000-4000-8000-000000000002").unwrap();
        let fingerprint = Digest::parse(&"0".repeat(64)).unwrap();
        let other_fingerprint = Digest::parse(&"1".repeat(64)).unwrap();
        let token = review_token(id, GitReviewStage::Clone, &fingerprint).unwrap();
        assert_eq!(
            token.as_str(),
            "c678c484d79f9d50efd0442ee3ae552e1eefd83653fa109c821c829c45fa99f1"
        );
        assert_ne!(
            token,
            review_token(other_id, GitReviewStage::Clone, &fingerprint).unwrap()
        );
        assert_ne!(
            token,
            review_token(id, GitReviewStage::Discover, &fingerprint).unwrap()
        );
        assert_ne!(
            token,
            review_token(id, GitReviewStage::Update, &fingerprint).unwrap()
        );
        assert_ne!(
            token,
            review_token(id, GitReviewStage::Clone, &other_fingerprint).unwrap()
        );
    }
}

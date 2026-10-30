//! Canonical operation fingerprint, encoding v1 (`planning/P03-T01-request.md` §5.3). Owner: W1, T05.

use domain::store::values::Digest;

use super::{ArgError, PermissionRequest};

/// Domain-separation prefix of encoding v1.
pub const FINGERPRINT_PREFIX: &[u8] = b"pwt-operation-fingerprint\x00v1\x00";

/// SHA-256 over the canonical encoding v1 of the request's operation content (fields F1–F7). Workflow
/// binding, run, lifetime, validity, purpose, inference scope, actor and timestamps are excluded; they are
/// matched as separate dimensions.
pub fn fingerprint(request: &PermissionRequest) -> Result<Digest, ArgError> {
    let _ = request;
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T05")
    }
}

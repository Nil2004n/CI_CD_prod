//! `dev permission-check` (`planning/P03-T01-request.md` §5.7, T01-D6). Owner: W2, T10.
//!
//! Decodes one synthetic permission request (JSON on standard input, a `dev`-only DTO), evaluates it against
//! the approvals in the data directory and, when approval is needed, records a pending request. A `once`
//! grant that matches is consumed. The purpose recorded is always `FixedText::PurposeDeveloperCheck`.

use std::path::Path;

use crate::approvals::{ApprovalError, AuthOutcome};

/// Largest accepted request document, in bytes.
pub const MAX_REQUEST_BYTES: usize = 65_536;

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

/// Runs `dev permission-check` against the store in `dir` with the request document `doc`.
pub fn check(dir: &Path, doc: &[u8]) -> Result<CheckOutcome, ApprovalError> {
    let _ = (dir, doc);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T10")
    }
}

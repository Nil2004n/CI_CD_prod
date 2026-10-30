//! The pure permission evaluator (`planning/P03-T01-request.md` §6, rows R1–R8 and G1–G14). Owner: W1, T05.

use domain::store::approval::Approval;

use super::{Evaluation, EvaluationContext, PermissionRequest};

/// Evaluates `request` against the recorded `approvals` (rules 12–16). Request-level checks R1–R8 first;
/// then a grant authorises only if every dimension G1–G14 matches. Never returns `Permitted` on a
/// fingerprint match alone.
pub fn evaluate(
    request: &PermissionRequest,
    approvals: &[Approval],
    ctx: &EvaluationContext,
) -> Evaluation {
    let _ = (request, approvals, ctx);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T05")
    }
}

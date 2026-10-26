//! State machines (P01-T07; frozen surface and transition tables `planning/P01-T01-request.md` §5.4,
//! including owner decisions T01-D1 and T01-D2).
//!
//! Transitions are keyed on the target state; self-transitions are rejected.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A rejected transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError<S> {
    /// State the transition started from.
    pub from: S,
    /// Requested target state.
    pub to: S,
}

impl<S: fmt::Debug> fmt::Display for TransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "transition not allowed: {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl<S: fmt::Debug> std::error::Error for TransitionError<S> {}

/// Run execution state (`$defs/Execution`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Persisted, not started.
    Queued,
    /// Waiting for a permission decision.
    AwaitingApproval,
    /// Executing tasks.
    Running,
    /// Cancellation requested, tasks stopping.
    Cancelling,
    /// Execution finished (not a clean security outcome).
    Completed,
    /// Useful results exist but coverage was lost.
    Partial,
    /// No usable target coverage.
    Failed,
    /// Graceful cancellation finished.
    Cancelled,
    /// Abnormal termination, recorded on recovery.
    Interrupted,
}

/// Check outcome (`$defs/Outcome`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// Not started.
    Pending,
    /// Executing.
    Running,
    /// Check ran with no findings.
    Passed,
    /// Check produced findings.
    Findings,
    /// Check failed to produce usable evidence.
    Failed,
    /// Capability not supported for this target.
    Unsupported,
    /// Not executed (e.g. approval denied).
    Skipped,
    /// Partial evidence only.
    Incomplete,
    /// Cancelled.
    Cancelled,
}

/// Approval state (`$defs/Approval.status`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    /// Awaiting the user's decision.
    Pending,
    /// Granted.
    Approved,
    /// Refused.
    Denied,
    /// Request withdrawn before a decision.
    Cancelled,
    /// Grant revoked.
    Revoked,
    /// Grant lifetime ended.
    Expired,
}

macro_rules! state_machine_api {
    ($name:ident, $task:literal) => {
        impl $name {
            /// Moves to `next` if the transition is allowed.
            pub fn transition(self, next: Self) -> Result<Self, TransitionError<Self>> {
                if self.can_transition(next) {
                    Ok(next)
                } else {
                    Err(TransitionError {
                        from: self,
                        to: next,
                    })
                }
            }

            /// Whether `self -> next` is an allowed transition.
            #[allow(clippy::todo, unused_variables)]
            pub fn can_transition(self, next: Self) -> bool {
                todo!($task)
            }

            /// Whether no transition leaves this state.
            #[allow(clippy::todo, unused_variables)]
            pub fn is_terminal(self) -> bool {
                todo!($task)
            }
        }
    };
}

impl RunState {
    /// Every value, in schema order.
    pub const ALL: [Self; 9] = [
        Self::Queued,
        Self::AwaitingApproval,
        Self::Running,
        Self::Cancelling,
        Self::Completed,
        Self::Partial,
        Self::Failed,
        Self::Cancelled,
        Self::Interrupted,
    ];
}

impl CheckOutcome {
    /// Every value, in schema order.
    pub const ALL: [Self; 9] = [
        Self::Pending,
        Self::Running,
        Self::Passed,
        Self::Findings,
        Self::Failed,
        Self::Unsupported,
        Self::Skipped,
        Self::Incomplete,
        Self::Cancelled,
    ];
}

impl ApprovalState {
    /// Every value, in schema order.
    pub const ALL: [Self; 6] = [
        Self::Pending,
        Self::Approved,
        Self::Denied,
        Self::Cancelled,
        Self::Revoked,
        Self::Expired,
    ];
}

state_machine_api!(RunState, "P01-T07");
state_machine_api!(CheckOutcome, "P01-T07");
state_machine_api!(ApprovalState, "P01-T07");

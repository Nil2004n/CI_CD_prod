//! Semantic validation, Data Schema §11 rules 1–25 and §18 rules 1–18 + 6a (request §7). Pure functions
//! over typed store records; no I/O, no clock, no randomness.
//!
//! Lead-owned dispatcher (P02 wave 2 preparation, request §5.12). The rule registry ([`rules`]) and the
//! rule bodies (`ids`, `workflow`, `approval`, `evidence`, `ai`) are W2's (T09). Every rule ID of request
//! §7 appears in the registry exactly once, with its status; rules that are not checkable on store records
//! in P02 carry no check function and name the phase that enforces them.

use domain::store::AnyStore;

use crate::validate::{Problems, RuleId};

mod ai;
mod approval;
mod evidence;
mod ids;
mod rules;
mod workflow;

/// How much of a rule P02 enforces on store records (request §7 statuses I/P/F/X).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RuleStatus {
    /// **I**: fully checked on store records.
    Implemented,
    /// **P**: the store-level part is checked; the rest is enforced in `rest_in`.
    Partial { rest_in: &'static str },
    /// **F**: producer-only; enforced where the records are produced, in `phase`.
    Producer { phase: &'static str },
    /// **X**: a policy statement, not checkable on records; `phase` names where it is honoured, if any.
    Policy { phase: Option<&'static str> },
}

/// One entry of the rule table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuleInfo {
    /// `D11-nn`, `D18-nn` or `D18-06a`.
    pub id: RuleId,
    pub status: RuleStatus,
    /// Short fixed English description (no document content).
    pub title: &'static str,
}

/// A rule body: inspects `doc` and pushes [`crate::validate::Layer::Semantic`] /
/// [`crate::validate::ProblemCode::Rule`] problems carrying the rule's ID. Must not panic.
pub(crate) type RuleFn = fn(doc: &AnyStore, out: &mut Problems);

/// A registry entry. `check` is `None` exactly for [`RuleStatus::Producer`] and [`RuleStatus::Policy`]
/// rules.
pub(crate) struct Rule {
    pub(crate) info: RuleInfo,
    pub(crate) check: Option<RuleFn>,
}

/// Every rule of request §7, in registry order.
pub fn rule_table() -> impl Iterator<Item = &'static RuleInfo> {
    rules::REGISTRY.iter().map(|r| &r.info)
}

/// Runs every registered check over `doc`, in registry order. An empty result means no semantic rule
/// found a problem; it does not mean the document is safe or complete.
pub fn check(doc: &AnyStore) -> Problems {
    let mut out = Problems::default();
    for rule in rules::REGISTRY {
        if let Some(f) = rule.check {
            f(doc, &mut out);
        }
    }
    out
}

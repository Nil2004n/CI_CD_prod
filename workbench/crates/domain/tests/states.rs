//! P01-T07 tests: state machines and assessment vocabulary
//! (`planning/P01-T01-request.md` §5.4 and §6.3 row T07).
//!
//! The expected transition tables below are written out literally and are not derived from the
//! implementation. The schema cross-check loads `schemas/workbench-store-v1.3.schema.json`.

// clippy.toml's allow-*-in-tests covers only `#[test]` functions, not the shared helpers below.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use domain::states::{ApprovalState, CheckOutcome, RunState, TransitionError};
use domain::vocab::{Assessment, Capability, Category, Coverage, Severity};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::fmt::Debug;

const SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.3.schema.json");

// ---------------------------------------------------------------------------------------------
// Independent expected tables (literal).
// ---------------------------------------------------------------------------------------------

use ApprovalState as A;
use CheckOutcome as C;
use RunState as R;

/// Run: queued, awaiting_approval, running, cancelling, completed, partial, failed, cancelled,
/// interrupted. 15 allowed pairs out of 81.
const RUN_ALLOWED: [(RunState, RunState); 15] = [
    (R::Queued, R::AwaitingApproval),
    (R::Queued, R::Running),
    (R::Queued, R::Cancelled),
    (R::Queued, R::Interrupted),
    (R::AwaitingApproval, R::Running),
    (R::AwaitingApproval, R::Cancelling),
    (R::AwaitingApproval, R::Interrupted),
    (R::Running, R::AwaitingApproval),
    (R::Running, R::Cancelling),
    (R::Running, R::Completed),
    (R::Running, R::Partial),
    (R::Running, R::Failed),
    (R::Running, R::Interrupted),
    (R::Cancelling, R::Cancelled),
    (R::Cancelling, R::Interrupted),
];

const RUN_TERMINAL: [RunState; 5] = [
    R::Completed,
    R::Partial,
    R::Failed,
    R::Cancelled,
    R::Interrupted,
];

const RUN_NON_TERMINAL: [RunState; 4] = [R::Queued, R::AwaitingApproval, R::Running, R::Cancelling];

/// Check: 12 allowed pairs out of 81.
const CHECK_ALLOWED: [(CheckOutcome, CheckOutcome); 12] = [
    (C::Pending, C::Running),
    (C::Pending, C::Skipped),
    (C::Pending, C::Unsupported),
    (C::Pending, C::Cancelled),
    (C::Pending, C::Incomplete),
    (C::Running, C::Passed),
    (C::Running, C::Findings),
    (C::Running, C::Failed),
    (C::Running, C::Unsupported),
    (C::Running, C::Skipped),
    (C::Running, C::Incomplete),
    (C::Running, C::Cancelled),
];

const CHECK_TERMINAL: [CheckOutcome; 7] = [
    C::Passed,
    C::Findings,
    C::Failed,
    C::Unsupported,
    C::Skipped,
    C::Incomplete,
    C::Cancelled,
];

const CHECK_NON_TERMINAL: [CheckOutcome; 2] = [C::Pending, C::Running];

/// Approval: 5 allowed pairs out of 36.
const APPROVAL_ALLOWED: [(ApprovalState, ApprovalState); 5] = [
    (A::Pending, A::Approved),
    (A::Pending, A::Denied),
    (A::Pending, A::Cancelled),
    (A::Approved, A::Revoked),
    (A::Approved, A::Expired),
];

const APPROVAL_TERMINAL: [ApprovalState; 4] = [A::Denied, A::Cancelled, A::Revoked, A::Expired];

const APPROVAL_NON_TERMINAL: [ApprovalState; 2] = [A::Pending, A::Approved];

// ---------------------------------------------------------------------------------------------
// Generic helpers.
// ---------------------------------------------------------------------------------------------

/// Checks every (from, to) pair of `all` against the literal `allowed` list; returns pair count.
fn check_table<S>(
    all: &[S],
    allowed: &[(S, S)],
    can: impl Fn(S, S) -> bool,
    trans: impl Fn(S, S) -> Result<S, TransitionError<S>>,
) -> usize
where
    S: Copy + PartialEq + Debug,
{
    let mut pairs = 0;
    for &from in all {
        for &to in all {
            pairs += 1;
            let expected = allowed.iter().any(|&(f, t)| f == from && t == to);
            assert_eq!(
                can(from, to),
                expected,
                "can_transition({from:?}, {to:?}) expected {expected}"
            );
            match trans(from, to) {
                Ok(next) => {
                    assert!(expected, "transition({from:?}, {to:?}) unexpectedly Ok");
                    assert_eq!(next, to, "transition must return the target state");
                }
                Err(err) => {
                    assert!(!expected, "transition({from:?}, {to:?}) unexpectedly Err");
                    assert_eq!(err.from, from);
                    assert_eq!(err.to, to);
                }
            }
        }
    }
    // The literal allowed list itself must be unique, in range and free of self-transitions.
    for (i, &(f, t)) in allowed.iter().enumerate() {
        assert_ne!(f, t, "self-transition in expected table: {f:?}");
        assert!(all.contains(&f) && all.contains(&t));
        for &(f2, t2) in &allowed[i + 1..] {
            assert!(
                !(f == f2 && t == t2),
                "duplicate expected pair {f:?}->{t:?}"
            );
        }
    }
    pairs
}

/// Checks `is_terminal` per value against literal lists and the outgoing-transition invariants.
fn check_terminals<S>(
    all: &[S],
    terminal: &[S],
    non_terminal: &[S],
    is_terminal: impl Fn(S) -> bool,
    can: impl Fn(S, S) -> bool,
) where
    S: Copy + PartialEq + Debug,
{
    assert_eq!(terminal.len() + non_terminal.len(), all.len());
    for &s in all {
        assert_eq!(
            terminal.contains(&s) as u8 + non_terminal.contains(&s) as u8,
            1,
            "{s:?} must be in exactly one expected list"
        );
    }
    for &s in terminal {
        assert!(is_terminal(s), "{s:?} must be terminal");
        assert!(
            all.iter().all(|&to| !can(s, to)),
            "terminal {s:?} has an outgoing transition"
        );
    }
    for &s in non_terminal {
        assert!(!is_terminal(s), "{s:?} must not be terminal");
        assert!(
            all.iter().any(|&to| can(s, to)),
            "non-terminal {s:?} has no outgoing transition"
        );
    }
}

/// Every value serialises to its `as_str` string and round-trips; unknown strings are rejected.
fn check_serde<S>(all: &[S], as_str: impl Fn(S) -> &'static str)
where
    S: Copy + PartialEq + Debug + Serialize + DeserializeOwned,
{
    for &v in all {
        let text = as_str(v);
        let json = serde_json::to_value(v).expect("serialise");
        assert_eq!(json, serde_json::Value::String(text.to_owned()), "{v:?}");
        let text_json = serde_json::to_string(&v).expect("serialise to string");
        assert_eq!(text_json, format!("\"{text}\""));
        let back: S = serde_json::from_str(&text_json).expect("deserialise");
        assert_eq!(back, v);
        let back_val: S = serde_json::from_value(json).expect("deserialise value");
        assert_eq!(back_val, v);
    }
    for bad in [
        "",
        " ",
        "bogus",
        "QUEUED",
        "Queued",
        "NoFindings",
        "no-findings",
        "awaiting-approval",
        " passed",
        "passed ",
        "\u{0}",
        "git_acquire_",
    ] {
        // Skip the rare case where a probe string is genuinely valid for this enum.
        if all.iter().any(|&v| as_str(v) == bad) {
            continue;
        }
        let quoted = serde_json::Value::String(bad.to_owned());
        assert!(
            serde_json::from_value::<S>(quoted).is_err(),
            "unknown string {bad:?} must not deserialise"
        );
    }
    for non_string in ["0", "1", "null", "true", "[]", "{}"] {
        assert!(
            serde_json::from_str::<S>(non_string).is_err(),
            "{non_string}"
        );
    }
}

/// ALL has no duplicates and unique `as_str` values.
fn check_all_unique<S>(all: &[S], as_str: impl Fn(S) -> &'static str)
where
    S: Copy + PartialEq + Debug,
{
    for (i, &a) in all.iter().enumerate() {
        for &b in &all[i + 1..] {
            assert_ne!(a, b, "duplicate in ALL");
            assert_ne!(as_str(a), as_str(b), "duplicate string in ALL");
        }
    }
}

fn schema_enum(pointer: &str) -> Vec<String> {
    let doc: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema parses");
    let node = doc
        .pointer(pointer)
        .unwrap_or_else(|| panic!("schema node {pointer} missing"));
    assert_eq!(node.get("type").and_then(|t| t.as_str()), Some("string"));
    node.get("enum")
        .and_then(|e| e.as_array())
        .unwrap_or_else(|| panic!("{pointer} has no enum array"))
        .iter()
        .map(|v| v.as_str().expect("enum value is a string").to_owned())
        .collect()
}

/// Cross-checks one Rust enum against the schema enum at `pointer`.
fn check_against_schema<S>(pointer: &str, all: &[S], as_str: impl Fn(S) -> &'static str)
where
    S: Copy + PartialEq + Debug + Serialize,
{
    let schema = schema_enum(pointer);
    let schema_set: BTreeSet<&str> = schema.iter().map(String::as_str).collect();
    assert_eq!(schema_set.len(), schema.len(), "schema enum has duplicates");

    assert_eq!(
        all.len(),
        schema.len(),
        "{pointer}: ALL length differs from schema"
    );
    check_all_unique(all, &as_str);

    let via_as_str: BTreeSet<&str> = all.iter().map(|&v| as_str(v)).collect();
    assert_eq!(
        via_as_str, schema_set,
        "{pointer}: as_str set differs from schema"
    );

    let via_serde: Vec<String> = all
        .iter()
        .map(|&v| {
            serde_json::to_value(v)
                .expect("serialise")
                .as_str()
                .expect("serialises to a string")
                .to_owned()
        })
        .collect();
    let via_serde_set: BTreeSet<&str> = via_serde.iter().map(String::as_str).collect();
    assert_eq!(
        via_serde_set, schema_set,
        "{pointer}: serde set differs from schema"
    );

    // Documented "schema order" for ALL.
    assert_eq!(
        via_serde, schema,
        "{pointer}: ALL order differs from schema order"
    );
}

// ---------------------------------------------------------------------------------------------
// State machines.
// ---------------------------------------------------------------------------------------------

#[test]
fn run_state_full_pair_table() {
    let pairs = check_table(
        &RunState::ALL,
        &RUN_ALLOWED,
        RunState::can_transition,
        RunState::transition,
    );
    assert_eq!(pairs, 81);
}

#[test]
fn check_outcome_full_pair_table() {
    let pairs = check_table(
        &CheckOutcome::ALL,
        &CHECK_ALLOWED,
        CheckOutcome::can_transition,
        CheckOutcome::transition,
    );
    assert_eq!(pairs, 81);
}

#[test]
fn approval_state_full_pair_table() {
    let pairs = check_table(
        &ApprovalState::ALL,
        &APPROVAL_ALLOWED,
        ApprovalState::can_transition,
        ApprovalState::transition,
    );
    assert_eq!(pairs, 36);
}

#[test]
fn owner_decided_transitions_are_present() {
    // T01-D1
    assert!(R::Cancelling.can_transition(R::Interrupted));
    // T01-D2
    for to in [C::Skipped, C::Unsupported, C::Cancelled, C::Incomplete] {
        assert!(C::Pending.can_transition(to), "pending -> {to:?}");
    }
}

#[test]
fn self_transitions_are_rejected() {
    for s in RunState::ALL {
        assert!(!s.can_transition(s));
        assert_eq!(s.transition(s), Err(TransitionError { from: s, to: s }));
    }
    for s in CheckOutcome::ALL {
        assert!(!s.can_transition(s));
        assert_eq!(s.transition(s), Err(TransitionError { from: s, to: s }));
    }
    for s in ApprovalState::ALL {
        assert!(!s.can_transition(s));
        assert_eq!(s.transition(s), Err(TransitionError { from: s, to: s }));
    }
}

#[test]
fn run_state_terminals() {
    check_terminals(
        &RunState::ALL,
        &RUN_TERMINAL,
        &RUN_NON_TERMINAL,
        RunState::is_terminal,
        RunState::can_transition,
    );
}

#[test]
fn check_outcome_terminals() {
    check_terminals(
        &CheckOutcome::ALL,
        &CHECK_TERMINAL,
        &CHECK_NON_TERMINAL,
        CheckOutcome::is_terminal,
        CheckOutcome::can_transition,
    );
}

#[test]
fn approval_state_terminals() {
    check_terminals(
        &ApprovalState::ALL,
        &APPROVAL_TERMINAL,
        &APPROVAL_NON_TERMINAL,
        ApprovalState::is_terminal,
        ApprovalState::can_transition,
    );
}

#[test]
fn transition_error_carries_from_and_to_and_displays_both() {
    let err = RunState::Completed
        .transition(RunState::Running)
        .expect_err("completed -> running is rejected");
    assert_eq!(err.from, RunState::Completed);
    assert_eq!(err.to, RunState::Running);
    let text = err.to_string();
    assert!(text.contains("Completed"), "{text}");
    assert!(text.contains("Running"), "{text}");

    let err = CheckOutcome::Passed
        .transition(CheckOutcome::Pending)
        .expect_err("passed -> pending is rejected");
    assert_eq!(
        (err.from, err.to),
        (CheckOutcome::Passed, CheckOutcome::Pending)
    );
    let text = err.to_string();
    assert!(
        text.contains("Passed") && text.contains("Pending"),
        "{text}"
    );

    let err = ApprovalState::Denied
        .transition(ApprovalState::Approved)
        .expect_err("denied -> approved is rejected");
    assert_eq!(
        (err.from, err.to),
        (ApprovalState::Denied, ApprovalState::Approved)
    );
    let text = err.to_string();
    assert!(
        text.contains("Denied") && text.contains("Approved"),
        "{text}"
    );

    // Usable as a boxed std error.
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    assert!(!boxed.to_string().is_empty());
}

#[test]
fn transition_ok_returns_target() {
    assert_eq!(R::Queued.transition(R::Running), Ok(R::Running));
    assert_eq!(C::Running.transition(C::Findings), Ok(C::Findings));
    assert_eq!(A::Approved.transition(A::Expired), Ok(A::Expired));
}

#[test]
fn state_serde_round_trip_and_unknown_strings() {
    check_serde(&RunState::ALL, state_run_str);
    check_serde(&CheckOutcome::ALL, state_check_str);
    check_serde(&ApprovalState::ALL, state_approval_str);
}

// States have no `as_str` in the frozen surface; use the serde string as the source of truth and
// compare with a literal list written here.
fn state_run_str(s: RunState) -> &'static str {
    match s {
        R::Queued => "queued",
        R::AwaitingApproval => "awaiting_approval",
        R::Running => "running",
        R::Cancelling => "cancelling",
        R::Completed => "completed",
        R::Partial => "partial",
        R::Failed => "failed",
        R::Cancelled => "cancelled",
        R::Interrupted => "interrupted",
    }
}

fn state_check_str(s: CheckOutcome) -> &'static str {
    match s {
        C::Pending => "pending",
        C::Running => "running",
        C::Passed => "passed",
        C::Findings => "findings",
        C::Failed => "failed",
        C::Unsupported => "unsupported",
        C::Skipped => "skipped",
        C::Incomplete => "incomplete",
        C::Cancelled => "cancelled",
    }
}

fn state_approval_str(s: ApprovalState) -> &'static str {
    match s {
        A::Pending => "pending",
        A::Approved => "approved",
        A::Denied => "denied",
        A::Cancelled => "cancelled",
        A::Revoked => "revoked",
        A::Expired => "expired",
    }
}

// ---------------------------------------------------------------------------------------------
// Schema cross-check.
// ---------------------------------------------------------------------------------------------

#[test]
fn schema_execution_matches_run_state() {
    check_against_schema("/$defs/Execution", &RunState::ALL, state_run_str);
}

#[test]
fn schema_outcome_matches_check_outcome() {
    check_against_schema("/$defs/Outcome", &CheckOutcome::ALL, state_check_str);
}

#[test]
fn schema_approval_status_matches_approval_state() {
    check_against_schema(
        "/$defs/Approval/properties/status",
        &ApprovalState::ALL,
        state_approval_str,
    );
}

#[test]
fn schema_capability_matches() {
    check_against_schema("/$defs/Capability", &Capability::ALL, Capability::as_str);
}

#[test]
fn schema_category_matches() {
    check_against_schema("/$defs/Category", &Category::ALL, Category::as_str);
}

#[test]
fn schema_severity_matches() {
    check_against_schema("/$defs/Severity", &Severity::ALL, Severity::as_str);
}

#[test]
fn schema_assessment_matches() {
    check_against_schema("/$defs/Assessment", &Assessment::ALL, Assessment::as_str);
}

#[test]
fn schema_coverage_matches() {
    check_against_schema("/$defs/Coverage", &Coverage::ALL, Coverage::as_str);
}

// ---------------------------------------------------------------------------------------------
// Vocabulary.
// ---------------------------------------------------------------------------------------------

#[test]
fn vocab_expected_counts() {
    assert_eq!(Capability::ALL.len(), 11);
    assert_eq!(Category::ALL.len(), 9);
    assert_eq!(Severity::ALL.len(), 6);
    assert_eq!(Assessment::ALL.len(), 3);
    assert_eq!(Coverage::ALL.len(), 3);
    assert_eq!(RunState::ALL.len(), 9);
    assert_eq!(CheckOutcome::ALL.len(), 9);
    assert_eq!(ApprovalState::ALL.len(), 6);
}

#[test]
fn vocab_serde_round_trip_and_unknown_strings() {
    check_serde(&Capability::ALL, Capability::as_str);
    check_serde(&Category::ALL, Category::as_str);
    check_serde(&Severity::ALL, Severity::as_str);
    check_serde(&Assessment::ALL, Assessment::as_str);
    check_serde(&Coverage::ALL, Coverage::as_str);
}

#[test]
fn capability_as_str_literals() {
    let expected = [
        (Capability::GitAcquire, "git_acquire"),
        (Capability::GitUpdate, "git_update"),
        (Capability::ExternalReference, "external_reference"),
        (Capability::SchemaFetch, "schema_fetch"),
        (Capability::VulnerabilityLookup, "vulnerability_lookup"),
        (Capability::DocumentationFetch, "documentation_fetch"),
        (Capability::ModelTransmission, "model_transmission"),
        (Capability::EmbeddingTransmission, "embedding_transmission"),
        (Capability::LocalRender, "local_render"),
        (
            Capability::NonMutatingVerification,
            "non_mutating_verification",
        ),
        (Capability::AppStorage, "app_storage"),
    ];
    assert_eq!(expected.len(), Capability::ALL.len());
    for (cap, text) in expected {
        assert_eq!(cap.as_str(), text);
    }
}

#[test]
fn capability_is_deferred_exact_set() {
    let deferred: BTreeSet<&str> = Capability::ALL
        .iter()
        .filter(|c| c.is_deferred())
        .map(|c| c.as_str())
        .collect();
    let expected: BTreeSet<&str> = [
        "documentation_fetch",
        "embedding_transmission",
        "non_mutating_verification",
    ]
    .into_iter()
    .collect();
    assert_eq!(deferred, expected);

    for cap in [
        Capability::GitAcquire,
        Capability::GitUpdate,
        Capability::ExternalReference,
        Capability::SchemaFetch,
        Capability::VulnerabilityLookup,
        Capability::ModelTransmission,
        Capability::LocalRender,
        Capability::AppStorage,
    ] {
        assert!(!cap.is_deferred(), "{cap:?} must not be deferred");
    }
    for cap in [
        Capability::DocumentationFetch,
        Capability::EmbeddingTransmission,
        Capability::NonMutatingVerification,
    ] {
        assert!(cap.is_deferred(), "{cap:?} must be deferred");
    }
}

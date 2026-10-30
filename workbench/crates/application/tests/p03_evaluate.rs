//! P03-T05 tests: the pure permission evaluator (`planning/P03-T01-request.md` §6: rows R1-R8 and G1-G14,
//! tie-break and `Mismatch` reporting).
//!
//! Approval records are built from the public `domain` types (struct literals of `Approval`, `FixedText`
//! for the texts). Expected outcomes are written out literally. Every test says what would make it fail.
//! All values are synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use application::permission::{
    ArgName, Evaluation, EvaluationContext, NeedReason, NormalisedDestination, OperationArg,
    OperationName, PermissionRequest, RefusalReason, RequestedData, ScopeDimension,
    WorkflowBinding, evaluate, fingerprint,
};
use domain::contract::{ApprovalLifetime, DataCategory, FixedText};
use domain::ids::{ApprovalId, RepoId, RunId, WorkflowId};
use domain::states::ApprovalState;
use domain::store::approval::Approval;
use domain::store::provider::InferenceScope;
use domain::store::values::Digest;
use domain::text::sanitise;
use domain::time::UtcTimestamp;
use domain::vocab::Capability;

const NOW: i64 = 1_800_000_000;

/// A one-field change to an approval record, used by the one-dimension-at-a-time tables.
type Mutator = Box<dyn Fn(&mut Approval)>;

fn id<T>(parse: fn(&str) -> Result<T, domain::ids::IdError>, n: u32) -> T {
    parse(&format!("00000000-0000-4000-8000-{n:012x}")).expect("id")
}

fn approval_id(n: u32) -> ApprovalId {
    id(ApprovalId::parse, n)
}

fn repo(n: u32) -> RepoId {
    id(RepoId::parse, 0x100 + n)
}

fn workflow(n: u32) -> WorkflowId {
    id(WorkflowId::parse, 0x200 + n)
}

fn run(n: u32) -> RunId {
    id(RunId::parse, 0x300 + n)
}

fn ts(offset: i64) -> UtcTimestamp {
    UtcTimestamp::from_unix(NOW + offset, 0).expect("timestamp")
}

fn dest(s: &str) -> NormalisedDestination {
    NormalisedDestination::parse(s).expect("destination")
}

fn scope(model: &str) -> InferenceScope {
    serde_json::from_value(serde_json::json!({
        "provider_id": "openrouter",
        "model_id": model,
        "routing_policy": {"data_collection": "deny", "zdr": true, "allow_fallbacks": false, "only": ["openrouter"]},
        "source_ids": ["01234567-89ab-4cde-8f01-23456789abcd"],
        "redaction_version": "2026.10-a"
    }))
    .expect("inference scope")
}

fn ctx(current: Option<u32>) -> EvaluationContext {
    EvaluationContext {
        now: ts(0),
        current_workflow_revision: current,
    }
}

fn bind(n: u32, revision: u32) -> Option<WorkflowBinding> {
    Some(WorkflowBinding {
        workflow_id: workflow(n),
        revision,
    })
}

/// A plain network request: `git_acquire`, one repository, one destination, one category, `once`, no
/// workflow, no run.
fn plain() -> PermissionRequest {
    PermissionRequest {
        capability: Capability::GitAcquire,
        operation: OperationName::parse("git.clone").expect("operation"),
        arguments: vec![OperationArg {
            name: ArgName::parse("branch").expect("name"),
            value: "main".to_owned(),
        }],
        workflow: None,
        run_id: None,
        repo_ids: vec![repo(1)],
        destinations: vec![dest("https://github.com/example/repo")],
        resource_constraints: vec![sanitise("max_size_mb=100")],
        data: vec![RequestedData::Category(DataCategory::RepositoryMetadata)],
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: ApprovalLifetime::Once,
        validity_seconds: None,
        inference_scope: None,
    }
}

/// A request that binds every context field: `model_transmission` with an inference scope, workflow 1 at
/// revision 3, run 1, two repositories, constraints and data.
fn rich() -> PermissionRequest {
    PermissionRequest {
        capability: Capability::ModelTransmission,
        operation: OperationName::parse("chat.send").expect("operation"),
        arguments: Vec::new(),
        workflow: bind(1, 3),
        run_id: Some(run(1)),
        repo_ids: vec![repo(1), repo(2)],
        destinations: vec![dest("https://openrouter.ai/api/v1")],
        resource_constraints: vec![sanitise("max_tokens=2048"), sanitise("max_requests=1")],
        data: vec![
            RequestedData::Category(DataCategory::RedactedSource),
            RequestedData::Category(DataCategory::RedactedFindings),
        ],
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: ApprovalLifetime::Once,
        validity_seconds: None,
        inference_scope: Some(scope("openai/gpt-4o:free")),
    }
}

fn categories(request: &PermissionRequest) -> Vec<DataCategory> {
    request
        .data
        .iter()
        .filter_map(|d| match d {
            RequestedData::Category(c) => Some(*c),
            RequestedData::RawSecretValues => None,
        })
        .collect()
}

/// A record that matches `request` exactly on every dimension, with the given status and lifetime. The
/// run id is copied from the request, the revision from its workflow binding.
fn record(
    request: &PermissionRequest,
    n: u32,
    status: ApprovalState,
    lifetime: ApprovalLifetime,
) -> Approval {
    Approval {
        approval_id: approval_id(n),
        status,
        capability: request.capability,
        workflow_id: request.workflow.map(|b| b.workflow_id),
        workflow_revision: request.workflow.map(|b| u64::from(b.revision)),
        run_id: request.run_id,
        repo_ids: request.repo_ids.clone(),
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        operation_fingerprint: fingerprint(request).expect("fingerprint"),
        destinations: request.destinations.iter().map(|d| d.to_store()).collect(),
        resource_constraints: request.resource_constraints.clone(),
        data_categories: categories(request),
        lifetime,
        requested_at: ts(-1000 + i64::from(n)),
        decided_at: (status != ApprovalState::Pending).then(|| ts(-900)),
        expires_at: None,
        actor: if status == ApprovalState::Pending {
            FixedText::ActorRequester.redacted()
        } else {
            FixedText::ActorLocalUserCli.redacted()
        },
        consumed: false,
        inference_scope: request.inference_scope.clone(),
    }
}

fn approved(request: &PermissionRequest, n: u32, lifetime: ApprovalLifetime) -> Approval {
    record(request, n, ApprovalState::Approved, lifetime)
}

fn permitted(e: Evaluation) -> (ApprovalId, ApprovalLifetime) {
    match e {
        Evaluation::Permitted(grant) => (grant.approval_id(), grant.lifetime()),
        other => panic!("expected Permitted, got {other:?}"),
    }
}

fn mismatch(n: u32, dimension: ScopeDimension) -> Evaluation {
    Evaluation::NeedsApproval(NeedReason::Mismatch {
        approval_id: approval_id(n),
        dimension,
    })
}

const NO_GRANT: Evaluation = Evaluation::NeedsApproval(NeedReason::NoGrant);

fn refused(reason: RefusalReason) -> Evaluation {
    Evaluation::Refused(reason)
}

fn other_digest(request: &PermissionRequest) -> Digest {
    let mut changed = request.clone();
    changed.operation = OperationName::parse("other.operation").expect("operation");
    fingerprint(&changed).expect("fingerprint")
}

// ---------------------------------------------------------------------------------------------------------
// Baseline.
// ---------------------------------------------------------------------------------------------------------

/// Fails if a record that matches on every dimension is not permitted, or if the result carries another
/// record or lifetime. Every negative test below is a one-dimension deviation from this.
#[test]
fn a_fully_matching_grant_is_permitted() {
    for request in [plain(), rich()] {
        let grant = approved(&request, 1, ApprovalLifetime::Once);
        assert_eq!(
            permitted(evaluate(
                &request,
                &[grant],
                &ctx(request.workflow.map(|b| b.revision))
            )),
            (approval_id(1), ApprovalLifetime::Once)
        );
    }
}

/// Fails if an empty store, or a store with only another capability's grant, is not `NoGrant`.
#[test]
fn no_record_means_no_grant() {
    let request = plain();
    assert_eq!(evaluate(&request, &[], &ctx(None)), NO_GRANT);
    let mut other = approved(&request, 1, ApprovalLifetime::Once);
    other.capability = Capability::GitUpdate;
    assert_eq!(evaluate(&request, &[other], &ctx(None)), NO_GRANT);
}

// ---------------------------------------------------------------------------------------------------------
// R1-R8.
// ---------------------------------------------------------------------------------------------------------

/// R1. Fails if raw secret values can be permitted, including under a grant recorded for the very same
/// request (rule 16), or if a request without them is refused.
#[test]
fn r1_raw_secret_values_are_always_refused() {
    let mut request = plain();
    request.data.push(RequestedData::RawSecretValues);
    let grant = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[grant], &ctx(None)),
        refused(RefusalReason::RawSecretRequested)
    );
    assert_eq!(
        evaluate(&request, &[], &ctx(None)),
        refused(RefusalReason::RawSecretRequested)
    );
    // Negative: the same request without the datum is not refused.
    assert_eq!(evaluate(&plain(), &[], &ctx(None)), NO_GRANT);
}

/// R2. Fails if the network/local split moves for any of the 11 capabilities.
#[test]
fn r2_destination_rules_per_capability() {
    for capability in Capability::ALL {
        let local = matches!(capability, Capability::LocalRender | Capability::AppStorage);
        let mut with_dest = plain();
        with_dest.capability = capability;
        if capability == Capability::ModelTransmission {
            with_dest.inference_scope = Some(scope("openai/gpt-4o:free"));
        }
        let mut without = with_dest.clone();
        without.destinations.clear();
        let (with_result, without_result) = (
            evaluate(&with_dest, &[], &ctx(None)),
            evaluate(&without, &[], &ctx(None)),
        );
        if local {
            assert_eq!(
                with_result,
                refused(RefusalReason::UnexpectedDestination),
                "{capability:?}"
            );
            assert_eq!(without_result, NO_GRANT, "{capability:?}");
        } else {
            assert_eq!(with_result, NO_GRANT, "{capability:?}");
            assert_eq!(
                without_result,
                refused(RefusalReason::MissingDestination),
                "{capability:?}"
            );
        }
    }
}

/// R3. Fails if `workflow_revision` without a binding or `run` without a run id is accepted, or if the
/// bound forms are refused.
#[test]
fn r3_lifetime_needs_its_binding() {
    let mut wr = plain();
    wr.lifetime = ApprovalLifetime::WorkflowRevision;
    assert_eq!(
        evaluate(&wr, &[], &ctx(Some(3))),
        refused(RefusalReason::LifetimeBinding)
    );
    wr.workflow = bind(1, 3);
    assert_eq!(evaluate(&wr, &[], &ctx(Some(3))), NO_GRANT);

    let mut by_run = plain();
    by_run.lifetime = ApprovalLifetime::Run;
    assert_eq!(
        evaluate(&by_run, &[], &ctx(None)),
        refused(RefusalReason::LifetimeBinding)
    );
    by_run.run_id = Some(run(1));
    assert_eq!(evaluate(&by_run, &[], &ctx(None)), NO_GRANT);

    // `once` needs neither.
    assert_eq!(evaluate(&plain(), &[], &ctx(None)), NO_GRANT);
}

/// R4. Fails if an unsaved workflow, an older or a newer saved revision is accepted, or if the equal
/// revision, or an unbound request with a saved revision in the context, is refused.
#[test]
fn r4_workflow_must_be_saved_at_the_bound_revision() {
    let mut request = plain();
    request.workflow = bind(1, 3);
    assert_eq!(
        evaluate(&request, &[], &ctx(None)),
        refused(RefusalReason::UnknownWorkflow)
    );
    assert_eq!(
        evaluate(&request, &[], &ctx(Some(2))),
        refused(RefusalReason::StaleWorkflowRevision)
    );
    assert_eq!(
        evaluate(&request, &[], &ctx(Some(4))),
        refused(RefusalReason::StaleWorkflowRevision)
    );
    assert_eq!(evaluate(&request, &[], &ctx(Some(3))), NO_GRANT);
    assert_eq!(evaluate(&plain(), &[], &ctx(Some(9))), NO_GRANT);
}

/// R5. Fails if `model_transmission` without a scope, or another capability with one, is accepted, or if
/// the correct pairing is refused.
#[test]
fn r5_inference_scope_only_for_model_transmission() {
    let mut missing = rich();
    missing.inference_scope = None;
    assert_eq!(
        evaluate(&missing, &[], &ctx(Some(3))),
        refused(RefusalReason::InferenceScope)
    );
    let mut extra = plain();
    extra.inference_scope = Some(scope("openai/gpt-4o:free"));
    assert_eq!(
        evaluate(&extra, &[], &ctx(None)),
        refused(RefusalReason::InferenceScope)
    );
    assert_eq!(evaluate(&rich(), &[], &ctx(Some(3))), NO_GRANT);
    assert_eq!(evaluate(&plain(), &[], &ctx(None)), NO_GRANT);
}

/// R6. Fails if duplicate argument names, an over-long value or a workflow revision of 0 is accepted, or
/// if valid arguments are refused.
#[test]
fn r6_invalid_arguments_are_refused() {
    let mut duplicate = plain();
    duplicate.arguments.push(duplicate.arguments[0].clone());
    assert_eq!(
        evaluate(&duplicate, &[], &ctx(None)),
        refused(RefusalReason::InvalidRequest)
    );

    let mut long = plain();
    long.arguments[0].value = "a".repeat(65_537);
    assert_eq!(
        evaluate(&long, &[], &ctx(None)),
        refused(RefusalReason::InvalidRequest)
    );

    let mut zero = plain();
    zero.workflow = bind(1, 0);
    assert_eq!(
        evaluate(&zero, &[], &ctx(Some(0))),
        refused(RefusalReason::InvalidRequest)
    );

    let mut fine = plain();
    fine.arguments.push(OperationArg {
        name: ArgName::parse("depth").expect("name"),
        value: "1".to_owned(),
    });
    assert_eq!(evaluate(&fine, &[], &ctx(None)), NO_GRANT);
}

/// R7. Fails if a denial is not honoured, if it applies when one of capability, fingerprint, workflow,
/// revision or run differs, or if a lifetime difference stops it (R7 does not compare the lifetime).
#[test]
fn r7_previously_denied() {
    let request = rich();
    let c = ctx(Some(3));
    let denied = |n: u32| record(&request, n, ApprovalState::Denied, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[denied(7)], &c),
        refused(RefusalReason::PreviouslyDenied(approval_id(7)))
    );
    // The first denial in store order is named.
    assert_eq!(
        evaluate(&request, &[denied(8), denied(7)], &c),
        refused(RefusalReason::PreviouslyDenied(approval_id(8)))
    );
    // Another lifetime on the denied record does not matter.
    let mut run_denied = denied(7);
    run_denied.lifetime = ApprovalLifetime::Run;
    assert_eq!(
        evaluate(&request, &[run_denied], &c),
        refused(RefusalReason::PreviouslyDenied(approval_id(7)))
    );
    // Negatives: one equal-condition broken at a time.
    let breakers: Vec<(&str, Mutator)> = vec![
        (
            "capability",
            Box::new(|a: &mut Approval| a.capability = Capability::GitAcquire),
        ),
        (
            "fingerprint",
            Box::new({
                let d = other_digest(&request);
                move |a: &mut Approval| a.operation_fingerprint = d.clone()
            }),
        ),
        (
            "workflow",
            Box::new(|a: &mut Approval| a.workflow_id = Some(workflow(2))),
        ),
        (
            "workflow none",
            Box::new(|a: &mut Approval| a.workflow_id = None),
        ),
        (
            "revision",
            Box::new(|a: &mut Approval| a.workflow_revision = Some(2)),
        ),
        ("run", Box::new(|a: &mut Approval| a.run_id = Some(run(2)))),
        ("run none", Box::new(|a: &mut Approval| a.run_id = None)),
    ];
    for (name, mutate) in &breakers {
        let mut a = denied(7);
        mutate(&mut a);
        assert_eq!(evaluate(&request, &[a], &c), NO_GRANT, "{name}");
    }
    // Other statuses are not denials.
    let mut revoked = denied(7);
    revoked.status = ApprovalState::Revoked;
    assert_eq!(evaluate(&request, &[revoked], &c), NO_GRANT);
}

/// R7 comes before the grant search: a denial of the identical request is not overridden by an approved
/// record. Fails if a matching approved record wins over a denial.
#[test]
fn r7_denial_wins_over_a_matching_grant() {
    let request = plain();
    let grant = approved(&request, 1, ApprovalLifetime::Once);
    let denied = record(&request, 2, ApprovalState::Denied, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[grant, denied], &ctx(None)),
        refused(RefusalReason::PreviouslyDenied(approval_id(2)))
    );
}

/// R8. Fails if an identical pending request is not reported, if it applies when capability, fingerprint,
/// workflow, revision, run or lifetime differs, or if a pending record is ever permitted.
#[test]
fn r8_already_pending() {
    let request = rich();
    let c = ctx(Some(3));
    let pending = |n: u32| record(&request, n, ApprovalState::Pending, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[pending(5)], &c),
        Evaluation::NeedsApproval(NeedReason::AlreadyPending(approval_id(5)))
    );
    assert_eq!(
        evaluate(&request, &[pending(6), pending(5)], &c),
        Evaluation::NeedsApproval(NeedReason::AlreadyPending(approval_id(6)))
    );
    let breakers: Vec<(&str, Mutator)> = vec![
        (
            "capability",
            Box::new(|a: &mut Approval| a.capability = Capability::GitAcquire),
        ),
        (
            "fingerprint",
            Box::new({
                let d = other_digest(&request);
                move |a: &mut Approval| a.operation_fingerprint = d.clone()
            }),
        ),
        (
            "workflow",
            Box::new(|a: &mut Approval| a.workflow_id = Some(workflow(2))),
        ),
        (
            "revision",
            Box::new(|a: &mut Approval| a.workflow_revision = Some(2)),
        ),
        ("run", Box::new(|a: &mut Approval| a.run_id = Some(run(2)))),
        (
            "lifetime",
            Box::new(|a: &mut Approval| a.lifetime = ApprovalLifetime::Run),
        ),
    ];
    for (name, mutate) in &breakers {
        let mut a = pending(5);
        mutate(&mut a);
        assert_eq!(evaluate(&request, &[a], &c), NO_GRANT, "{name}");
    }
}

/// R8 comes before the grant search, as the table orders it; and R7 comes before R8. Fails if a pending
/// record is skipped in favour of a grant, or if R7 and R8 swap places.
#[test]
fn r7_r8_order_before_the_grant_search() {
    let request = plain();
    let grant = approved(&request, 1, ApprovalLifetime::Once);
    let pending = record(&request, 2, ApprovalState::Pending, ApprovalLifetime::Once);
    let denied = record(&request, 3, ApprovalState::Denied, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[grant.clone(), pending.clone()], &ctx(None)),
        Evaluation::NeedsApproval(NeedReason::AlreadyPending(approval_id(2)))
    );
    assert_eq!(
        evaluate(&request, &[pending, denied], &ctx(None)),
        refused(RefusalReason::PreviouslyDenied(approval_id(3)))
    );
}

/// The request-level rows are checked in table order. Fails if any two adjacent rows swap.
#[test]
fn r1_to_r6_are_checked_in_order() {
    // R1 before R2: raw secret with no destination.
    let mut r = plain();
    r.destinations.clear();
    r.data.push(RequestedData::RawSecretValues);
    assert_eq!(
        evaluate(&r, &[], &ctx(None)),
        refused(RefusalReason::RawSecretRequested)
    );
    // R2 before R3: no destination and a run lifetime without run.
    let mut r = plain();
    r.destinations.clear();
    r.lifetime = ApprovalLifetime::Run;
    assert_eq!(
        evaluate(&r, &[], &ctx(None)),
        refused(RefusalReason::MissingDestination)
    );
    // R3 before R4: a run lifetime without run, and a workflow that is not saved.
    let mut r = plain();
    r.lifetime = ApprovalLifetime::Run;
    r.workflow = bind(1, 3);
    assert_eq!(
        evaluate(&r, &[], &ctx(None)),
        refused(RefusalReason::LifetimeBinding)
    );
    // R4 before R5: unknown workflow and a missing scope.
    let mut r = rich();
    r.inference_scope = None;
    assert_eq!(
        evaluate(&r, &[], &ctx(None)),
        refused(RefusalReason::UnknownWorkflow)
    );
    // R5 before R6: a missing scope and a duplicate argument.
    let mut r = rich();
    r.inference_scope = None;
    r.arguments = vec![
        OperationArg {
            name: ArgName::parse("a").expect("name"),
            value: "1".to_owned(),
        },
        OperationArg {
            name: ArgName::parse("a").expect("name"),
            value: "2".to_owned(),
        },
    ];
    assert_eq!(
        evaluate(&r, &[], &ctx(Some(3))),
        refused(RefusalReason::InferenceScope)
    );
}

// ---------------------------------------------------------------------------------------------------------
// G1-G14.
// ---------------------------------------------------------------------------------------------------------

/// G1. Fails if a record in any state other than `approved` can authorise, or if the status is reported as
/// a mismatch dimension (such a record is not a candidate).
#[test]
fn g1_only_approved_records_authorise() {
    let request = plain();
    for status in [
        ApprovalState::Pending,
        ApprovalState::Denied,
        ApprovalState::Cancelled,
        ApprovalState::Revoked,
        ApprovalState::Expired,
    ] {
        let mut r = record(&request, 1, status, ApprovalLifetime::Once);
        // Even a record carrying a decision time and no consumption must not authorise.
        r.decided_at = Some(ts(-10));
        let e = evaluate(&request, &[r], &ctx(None));
        assert!(
            !matches!(e, Evaluation::Permitted(_)),
            "{status:?} authorised"
        );
        match status {
            ApprovalState::Pending => assert_eq!(
                e,
                Evaluation::NeedsApproval(NeedReason::AlreadyPending(approval_id(1)))
            ),
            ApprovalState::Denied => {
                assert_eq!(e, refused(RefusalReason::PreviouslyDenied(approval_id(1))))
            }
            _ => assert_eq!(e, NO_GRANT, "{status:?}"),
        }
    }
    // A pending record of another lifetime is no R8 hit and still not a grant.
    let other_lifetime = record(&request, 1, ApprovalState::Pending, ApprovalLifetime::Run);
    assert_eq!(evaluate(&request, &[other_lifetime], &ctx(None)), NO_GRANT);
    // Positive.
    let ok = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, &[ok], &ctx(None))).0,
        approval_id(1)
    );
}

/// G2. Fails if a grant of another capability is considered (it must not even be reported as a mismatch),
/// or if the same capability is not.
#[test]
fn g2_capability_must_be_equal() {
    let request = plain();
    for capability in Capability::ALL {
        if capability == request.capability {
            continue;
        }
        let mut other = approved(&request, 1, ApprovalLifetime::Once);
        other.capability = capability;
        assert_eq!(
            evaluate(&request, &[other], &ctx(None)),
            NO_GRANT,
            "{capability:?}"
        );
    }
    // A same-capability record is the closest even when another capability is listed first.
    let mut other = approved(&request, 1, ApprovalLifetime::Once);
    other.capability = Capability::GitUpdate;
    let mut same = approved(&request, 2, ApprovalLifetime::Once);
    same.consumed = true;
    assert_eq!(
        evaluate(&request, &[other, same], &ctx(None)),
        mismatch(2, ScopeDimension::Consumption)
    );
}

/// G3. Fails if a grant for another operation (other fingerprint) is accepted, or if equal fingerprints
/// are not required to be the only thing that matches.
#[test]
fn g3_fingerprint_must_be_equal() {
    let request = plain();
    let mut grant = approved(&request, 1, ApprovalLifetime::Once);
    grant.operation_fingerprint = other_digest(&request);
    assert_eq!(
        evaluate(&request, &[grant], &ctx(None)),
        mismatch(1, ScopeDimension::Fingerprint)
    );

    // A grant recorded for a request that differs in an operation argument.
    let mut sibling = request.clone();
    sibling.arguments[0].value = "dev".to_owned();
    let sibling_grant = approved(&sibling, 2, ApprovalLifetime::Once);
    assert_eq!(
        evaluate(&request, &[sibling_grant], &ctx(None)),
        mismatch(2, ScopeDimension::Fingerprint)
    );
}

/// G4. Fails if the workflow id is not compared, in either direction (bound grant for an unbound request,
/// unbound grant for a bound request, another workflow), or if a `workflow_revision` grant with a null
/// workflow id gets past G4 (both null is allowed only for `once` and `run`).
#[test]
fn g4_workflow_must_be_equal() {
    let mut request = plain();
    request.workflow = bind(1, 3);
    let c = ctx(Some(3));
    let ok = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, std::slice::from_ref(&ok), &c)).0,
        approval_id(1)
    );

    let mut other = ok.clone();
    other.workflow_id = Some(workflow(2));
    assert_eq!(
        evaluate(&request, &[other], &c),
        mismatch(1, ScopeDimension::Workflow)
    );
    let mut unbound = ok;
    unbound.workflow_id = None;
    assert_eq!(
        evaluate(&request, &[unbound], &c),
        mismatch(1, ScopeDimension::Workflow)
    );

    let unbound_request = plain();
    let mut bound_grant = approved(&unbound_request, 1, ApprovalLifetime::Once);
    bound_grant.workflow_id = Some(workflow(1));
    assert_eq!(
        evaluate(&unbound_request, &[bound_grant], &ctx(None)),
        mismatch(1, ScopeDimension::Workflow)
    );

    // Both null is allowed only for `once` and `run`: a `workflow_revision` grant without a workflow id
    // fails G4 whatever the request has (here an unbound request; G4 precedes G5 and G12 in table order).
    let wr_null = approved(&unbound_request, 2, ApprovalLifetime::WorkflowRevision);
    assert_eq!(wr_null.workflow_id, None);
    assert_eq!(
        evaluate(&unbound_request, std::slice::from_ref(&wr_null), &ctx(None)),
        mismatch(2, ScopeDimension::Workflow)
    );
    // The same record against a bound request fails G4 as well (workflow differs).
    assert_eq!(
        evaluate(&request, &[wr_null], &c),
        mismatch(2, ScopeDimension::Workflow)
    );
    // `once` and `run` grants with both ids null still pass G4 for an unbound request.
    let once_null = approved(&unbound_request, 3, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&unbound_request, &[once_null], &ctx(None))),
        (approval_id(3), ApprovalLifetime::Once)
    );
    let mut run_request = plain();
    run_request.run_id = Some(run(1));
    let run_null = approved(&run_request, 4, ApprovalLifetime::Run);
    assert_eq!(run_null.workflow_id, None);
    assert_eq!(
        permitted(evaluate(&run_request, &[run_null], &ctx(None))),
        (approval_id(4), ApprovalLifetime::Run)
    );
}

/// G5 and rule 14. Fails if a grant for an older revision matches while the workflow has moved on, if a
/// missing grant revision matches a bound request, or if the equal revision does not match. A `workflow
/// _revision` grant is the case that matters most.
#[test]
fn g5_revision_must_equal_the_saved_revision() {
    let mut at3 = plain();
    at3.workflow = bind(1, 3);
    let wr_grant = approved(&at3, 1, ApprovalLifetime::WorkflowRevision);
    assert_eq!(
        permitted(evaluate(
            &at3,
            std::slice::from_ref(&wr_grant),
            &ctx(Some(3))
        )),
        (approval_id(1), ApprovalLifetime::WorkflowRevision)
    );

    // The workflow is saved again as revision 4 and the same operation is requested at revision 4.
    let mut at4 = plain();
    at4.workflow = bind(1, 4);
    assert_eq!(
        evaluate(&at4, std::slice::from_ref(&wr_grant), &ctx(Some(4))),
        mismatch(1, ScopeDimension::WorkflowRevision),
        "a stale grant never matches even while listed"
    );
    // A request still bound to revision 3 is refused before any grant is looked at (R4).
    assert_eq!(
        evaluate(&at3, &[wr_grant], &ctx(Some(4))),
        refused(RefusalReason::StaleWorkflowRevision)
    );

    // A grant without a revision for a bound request.
    let mut no_revision = approved(&at3, 2, ApprovalLifetime::Once);
    no_revision.workflow_revision = None;
    assert_eq!(
        evaluate(&at3, &[no_revision], &ctx(Some(3))),
        mismatch(2, ScopeDimension::WorkflowRevision)
    );
    // A revision on a grant for an unbound request.
    let plain_request = plain();
    let mut with_revision = approved(&plain_request, 3, ApprovalLifetime::Once);
    with_revision.workflow_revision = Some(3);
    assert_eq!(
        evaluate(&plain_request, &[with_revision], &ctx(None)),
        mismatch(3, ScopeDimension::WorkflowRevision)
    );
}

/// G6. Fails if the run is not compared for a `run` grant, if a `once` grant with a run does not have to
/// match it, or if a `once` grant without a run stops matching.
#[test]
fn g6_run_rules() {
    let mut request = plain();
    request.run_id = Some(run(1));
    request.lifetime = ApprovalLifetime::Run;
    let run_grant = approved(&request, 1, ApprovalLifetime::Run);
    assert_eq!(
        permitted(evaluate(
            &request,
            std::slice::from_ref(&run_grant),
            &ctx(None)
        )),
        (approval_id(1), ApprovalLifetime::Run)
    );

    let mut other_run = run_grant.clone();
    other_run.run_id = Some(run(2));
    assert_eq!(
        evaluate(&request, &[other_run], &ctx(None)),
        mismatch(1, ScopeDimension::Run)
    );

    // once grants.
    let once_with = approved(&request, 2, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(
            &request,
            std::slice::from_ref(&once_with),
            &ctx(None)
        ))
        .0,
        approval_id(2)
    );
    let mut once_other = once_with.clone();
    once_other.run_id = Some(run(2));
    assert_eq!(
        evaluate(&request, &[once_other], &ctx(None)),
        mismatch(2, ScopeDimension::Run)
    );
    let mut once_without = once_with.clone();
    once_without.run_id = None;
    assert_eq!(
        permitted(evaluate(&request, &[once_without], &ctx(None))).0,
        approval_id(2)
    );

    // A once grant bound to a run does not serve a request without a run.
    let unrun = plain();
    let mut grant = approved(&unrun, 3, ApprovalLifetime::Once);
    grant.run_id = Some(run(1));
    assert_eq!(
        evaluate(&unrun, &[grant], &ctx(None)),
        mismatch(3, ScopeDimension::Run)
    );

    // A run grant without a run on a request with one.
    let mut no_run = run_grant;
    no_run.run_id = None;
    assert_eq!(
        evaluate(&request, &[no_run], &ctx(None)),
        mismatch(1, ScopeDimension::Run)
    );
}

/// The one-dimension-at-a-time table for G3-G14 on the rich request. Fails if any dimension is not
/// compared (the record still has an equal fingerprint, so only the explicit check can catch G4-G14) or
/// reports the wrong dimension. This is also the "never permitted from a fingerprint match alone" test.
#[test]
fn every_dimension_is_checked_beyond_the_fingerprint() {
    let request = rich();
    let c = ctx(Some(3));
    let base = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, std::slice::from_ref(&base), &c)).0,
        approval_id(1)
    );
    assert_eq!(
        base.operation_fingerprint,
        fingerprint(&request).expect("fingerprint")
    );

    let cases: Vec<(ScopeDimension, Mutator)> = vec![
        (
            ScopeDimension::Fingerprint,
            Box::new({
                let d = other_digest(&request);
                move |a: &mut Approval| a.operation_fingerprint = d.clone()
            }),
        ),
        (
            ScopeDimension::Workflow,
            Box::new(|a: &mut Approval| a.workflow_id = Some(workflow(2))),
        ),
        (
            ScopeDimension::WorkflowRevision,
            Box::new(|a: &mut Approval| a.workflow_revision = Some(2)),
        ),
        (
            ScopeDimension::Run,
            Box::new(|a: &mut Approval| a.run_id = Some(run(2))),
        ),
        (
            ScopeDimension::Repos,
            Box::new(|a: &mut Approval| a.repo_ids = vec![repo(1)]),
        ),
        (
            ScopeDimension::Repos,
            Box::new(|a: &mut Approval| a.repo_ids.push(repo(3))),
        ),
        (
            ScopeDimension::Destinations,
            Box::new(|a: &mut Approval| {
                a.destinations = vec![dest("https://example.com").to_store()];
            }),
        ),
        (
            ScopeDimension::Destinations,
            Box::new(|a: &mut Approval| a.destinations.clear()),
        ),
        (
            ScopeDimension::ResourceConstraints,
            Box::new(|a: &mut Approval| {
                a.resource_constraints.push(sanitise("extra"));
            }),
        ),
        (
            ScopeDimension::ResourceConstraints,
            Box::new(|a: &mut Approval| a.resource_constraints.clear()),
        ),
        (
            ScopeDimension::DataCategories,
            Box::new(|a: &mut Approval| {
                a.data_categories.push(DataCategory::AppMetadata);
            }),
        ),
        (
            ScopeDimension::DataCategories,
            Box::new(|a: &mut Approval| a.data_categories.truncate(1)),
        ),
        (
            ScopeDimension::InferenceScope,
            Box::new(|a: &mut Approval| a.inference_scope = None),
        ),
        (
            ScopeDimension::InferenceScope,
            Box::new(|a: &mut Approval| {
                a.inference_scope = Some(scope("anthropic/claude-3-haiku"));
            }),
        ),
        (
            ScopeDimension::Expiry,
            Box::new(|a: &mut Approval| a.expires_at = Some(ts(0))),
        ),
        (
            ScopeDimension::Consumption,
            Box::new(|a: &mut Approval| a.consumed = true),
        ),
    ];
    for (i, (dimension, mutate)) in cases.iter().enumerate() {
        let mut grant = base.clone();
        mutate(&mut grant);
        assert_eq!(
            evaluate(&request, &[grant], &c),
            mismatch(1, *dimension),
            "case {i} ({dimension:?})"
        );
    }
}

/// G7-G10. Fails if the sets are compared as ordered lists or with duplicates counted: the same members in
/// another order, or repeated, still match.
#[test]
fn g7_to_g10_compare_as_sets() {
    let request = rich();
    let c = ctx(Some(3));
    let mut grant = approved(&request, 1, ApprovalLifetime::Once);
    grant.repo_ids = vec![repo(2), repo(1), repo(2)];
    grant.resource_constraints = vec![
        sanitise("max_requests=1"),
        sanitise("max_tokens=2048"),
        sanitise("max_requests=1"),
    ];
    grant.data_categories = vec![
        DataCategory::RedactedFindings,
        DataCategory::RedactedSource,
        DataCategory::RedactedFindings,
    ];
    assert_eq!(
        permitted(evaluate(&request, &[grant], &c)).0,
        approval_id(1)
    );

    // Destinations: two spellings of one destination on the request, listed in a different order.
    let mut two = rich();
    two.destinations = vec![
        dest("https://openrouter.ai/api/v1"),
        dest("HTTPS://OpenRouter.ai.:443/api/v1"),
        dest("https://example.com"),
    ];
    let mut grant = approved(&two, 2, ApprovalLifetime::Once);
    grant.destinations.reverse();
    grant.destinations.push(grant.destinations[0].clone());
    assert_eq!(permitted(evaluate(&two, &[grant], &c)).0, approval_id(2));
}

/// G8. Fails if destinations are compared by the fingerprint's full canonical form instead of the store
/// form for `ssh` (the store form has no path), or if an `https` path is ignored.
#[test]
fn g8_destinations_use_the_store_form() {
    let mut request = plain();
    request.capability = Capability::GitUpdate;
    request.destinations = vec![dest("ssh://git.example.org/team/repo.git")];
    let grant = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(grant.destinations[0].as_str(), "git.example.org:22");
    assert_eq!(
        permitted(evaluate(&request, std::slice::from_ref(&grant), &ctx(None))).0,
        approval_id(1)
    );
    // Another path has another fingerprint (F5 uses the canonical form), so the grant does not match.
    let mut other = request.clone();
    other.destinations = vec![dest("ssh://git.example.org/team/other.git")];
    assert_eq!(
        evaluate(&other, &[grant], &ctx(None)),
        mismatch(1, ScopeDimension::Fingerprint)
    );

    let mut https = plain();
    https.destinations = vec![dest("https://github.com/example/repo")];
    let mut tampered = approved(&https, 2, ApprovalLifetime::Once);
    tampered.destinations = vec![dest("https://github.com/example/other").to_store()];
    assert_eq!(
        evaluate(&https, &[tampered], &ctx(None)),
        mismatch(2, ScopeDimension::Destinations)
    );
}

/// G11. Fails if the inference scope is not compared with `==`: a grant without a scope, with another
/// model, or another routing policy must not match.
#[test]
fn g11_inference_scope() {
    let request = rich();
    let c = ctx(Some(3));
    let ok = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, std::slice::from_ref(&ok), &c)).0,
        approval_id(1)
    );
    let mut none = ok.clone();
    none.inference_scope = None;
    assert_eq!(
        evaluate(&request, &[none], &c),
        mismatch(1, ScopeDimension::InferenceScope)
    );
    let mut model = ok.clone();
    model.inference_scope = Some(scope("openai/gpt-4o-mini"));
    assert_eq!(
        evaluate(&request, &[model], &c),
        mismatch(1, ScopeDimension::InferenceScope)
    );
    let mut sources = ok;
    let mut changed = scope("openai/gpt-4o:free");
    changed.source_ids.clear();
    sources.inference_scope = Some(changed);
    assert_eq!(
        evaluate(&request, &[sources], &c),
        mismatch(1, ScopeDimension::InferenceScope)
    );
}

/// A `workflow_revision` grant stores `run_id = null` and serves its workflow revision in every run; a
/// grant that carries a run stays tied to it (G6 strictness). Fails if a null-run workflow grant stops
/// serving another run, or if a grant carrying run 1 is accepted in run 2.
#[test]
fn workflow_revision_grant_without_run_serves_every_run() {
    let mut run1 = plain();
    run1.workflow = bind(1, 3);
    run1.run_id = Some(run(1));
    let mut run2 = run1.clone();
    run2.run_id = Some(run(2));
    let c = ctx(Some(3));

    let mut grant = approved(&run1, 1, ApprovalLifetime::WorkflowRevision);
    grant.run_id = None;
    for request in [&run1, &run2] {
        assert_eq!(
            permitted(evaluate(request, std::slice::from_ref(&grant), &c)),
            (approval_id(1), ApprovalLifetime::WorkflowRevision)
        );
    }

    let tied = approved(&run1, 2, ApprovalLifetime::WorkflowRevision);
    assert_eq!(tied.run_id, Some(run(1)));
    assert_eq!(
        permitted(evaluate(&run1, std::slice::from_ref(&tied), &c)).0,
        approval_id(2)
    );
    assert_eq!(
        evaluate(&run2, &[tied], &c),
        mismatch(2, ScopeDimension::Run)
    );
}

/// G12. Fails if a `run` grant without a run id matches a request without a run (G6 alone would accept that
/// malformed record), if a `workflow_revision` grant without workflow and revision matches an unbound
/// request (reported at G4), or if the well-formed forms stop matching.
#[test]
fn g12_lifetime_completeness() {
    let request = plain();
    let run_grant = approved(&request, 1, ApprovalLifetime::Run);
    assert_eq!(run_grant.run_id, None);
    assert_eq!(
        evaluate(&request, std::slice::from_ref(&run_grant), &ctx(None)),
        mismatch(1, ScopeDimension::Lifetime)
    );
    // A `workflow_revision` grant without a workflow id already fails G4 (it never reaches G12).
    let wr_grant = approved(&request, 2, ApprovalLifetime::WorkflowRevision);
    assert_eq!(
        evaluate(&request, &[wr_grant], &ctx(None)),
        mismatch(2, ScopeDimension::Workflow)
    );
    // A workflow id without a revision is incomplete too.
    let mut bound = plain();
    bound.workflow = bind(1, 3);
    let mut half = approved(&bound, 3, ApprovalLifetime::WorkflowRevision);
    half.workflow_revision = None;
    assert_eq!(
        evaluate(&bound, &[half], &ctx(Some(3))),
        mismatch(3, ScopeDimension::WorkflowRevision)
    );
    // Positives.
    let mut by_run = plain();
    by_run.run_id = Some(run(1));
    let run_ok = approved(&by_run, 4, ApprovalLifetime::Run);
    assert_eq!(
        permitted(evaluate(&by_run, &[run_ok], &ctx(None))).1,
        ApprovalLifetime::Run
    );
    let wr_ok = approved(&bound, 5, ApprovalLifetime::WorkflowRevision);
    assert_eq!(
        permitted(evaluate(&bound, &[wr_ok], &ctx(Some(3)))).1,
        ApprovalLifetime::WorkflowRevision
    );
    // A once grant is complete without any binding.
    let once = approved(&request, 6, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, &[once], &ctx(None))).1,
        ApprovalLifetime::Once
    );
}

/// G13. Fails if `expires_at` is ignored, if an equal-to-now expiry still authorises, or if `null` or a
/// future expiry does not.
#[test]
fn g13_expiry() {
    let request = plain();
    let make = |expires_at: Option<UtcTimestamp>| {
        let mut a = approved(&request, 1, ApprovalLifetime::Once);
        a.expires_at = expires_at;
        a
    };
    assert_eq!(
        permitted(evaluate(&request, &[make(None)], &ctx(None))).0,
        approval_id(1)
    );
    assert_eq!(
        permitted(evaluate(&request, &[make(Some(ts(1)))], &ctx(None))).0,
        approval_id(1)
    );
    assert_eq!(
        evaluate(&request, &[make(Some(ts(0)))], &ctx(None)),
        mismatch(1, ScopeDimension::Expiry)
    );
    assert_eq!(
        evaluate(&request, &[make(Some(ts(-1)))], &ctx(None)),
        mismatch(1, ScopeDimension::Expiry)
    );
    // Sub-second resolution: one nanosecond after now is still valid, one before is not.
    let after = UtcTimestamp::from_unix(NOW, 1).expect("timestamp");
    assert_eq!(
        permitted(evaluate(&request, &[make(Some(after))], &ctx(None))).0,
        approval_id(1)
    );
    let before = UtcTimestamp::from_unix(NOW - 1, 999_999_999).expect("timestamp");
    assert_eq!(
        evaluate(&request, &[make(Some(before))], &ctx(None)),
        mismatch(1, ScopeDimension::Expiry)
    );
    // The status is still `approved` on an expired record: only the expiry stops it.
    assert_eq!(make(Some(ts(-1))).status, ApprovalState::Approved);
}

/// G14. Fails if a consumed `once` grant authorises again, if an unconsumed one does not, or if the flag
/// is applied to other lifetimes (their grants are not consumed).
#[test]
fn g14_consumption() {
    let request = plain();
    let mut consumed = approved(&request, 1, ApprovalLifetime::Once);
    consumed.consumed = true;
    assert_eq!(
        evaluate(&request, &[consumed], &ctx(None)),
        mismatch(1, ScopeDimension::Consumption)
    );
    let fresh = approved(&request, 1, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, &[fresh], &ctx(None))).0,
        approval_id(1)
    );

    let mut by_run = plain();
    by_run.run_id = Some(run(1));
    let mut run_grant = approved(&by_run, 2, ApprovalLifetime::Run);
    run_grant.consumed = true;
    assert_eq!(
        permitted(evaluate(&by_run, &[run_grant], &ctx(None))).1,
        ApprovalLifetime::Run
    );
}

/// Fails if a `pending`, `cancelled`, `revoked` or `expired` record that otherwise matches everything is
/// ever permitted, in any lifetime.
#[test]
fn non_approved_statuses_never_authorise_in_any_lifetime() {
    let mut request = rich();
    request.lifetime = ApprovalLifetime::Once;
    for lifetime in [
        ApprovalLifetime::Once,
        ApprovalLifetime::Run,
        ApprovalLifetime::WorkflowRevision,
    ] {
        for status in [
            ApprovalState::Pending,
            ApprovalState::Denied,
            ApprovalState::Cancelled,
            ApprovalState::Revoked,
            ApprovalState::Expired,
        ] {
            let r = record(&request, 1, status, lifetime);
            let e = evaluate(&request, &[r], &ctx(Some(3)));
            assert!(
                !matches!(e, Evaluation::Permitted(_)),
                "{status:?}/{lifetime:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Tie-break and Mismatch reporting.
// ---------------------------------------------------------------------------------------------------------

fn tie_request() -> PermissionRequest {
    let mut r = plain();
    r.workflow = bind(1, 3);
    r.run_id = Some(run(1));
    r
}

/// Several matching grants: `workflow_revision`, then `run`, then `once`. Fails if the winner depends on
/// the store order or if the preference order changes.
#[test]
fn tie_break_prefers_wider_non_consuming_lifetimes() {
    let request = tie_request();
    let c = ctx(Some(3));
    let wr = approved(&request, 1, ApprovalLifetime::WorkflowRevision);
    let by_run = approved(&request, 2, ApprovalLifetime::Run);
    let once = approved(&request, 3, ApprovalLifetime::Once);
    let orders: [[&Approval; 3]; 6] = [
        [&wr, &by_run, &once],
        [&wr, &once, &by_run],
        [&by_run, &wr, &once],
        [&by_run, &once, &wr],
        [&once, &wr, &by_run],
        [&once, &by_run, &wr],
    ];
    for order in orders {
        let list: Vec<Approval> = order.iter().map(|a| (*a).clone()).collect();
        assert_eq!(
            permitted(evaluate(&request, &list, &c)),
            (approval_id(1), ApprovalLifetime::WorkflowRevision)
        );
        let without_wr: Vec<Approval> = list
            .iter()
            .filter(|a| a.lifetime != ApprovalLifetime::WorkflowRevision)
            .cloned()
            .collect();
        assert_eq!(
            permitted(evaluate(&request, &without_wr, &c)),
            (approval_id(2), ApprovalLifetime::Run)
        );
        let only_once: Vec<Approval> = list
            .iter()
            .filter(|a| a.lifetime == ApprovalLifetime::Once)
            .cloned()
            .collect();
        assert_eq!(
            permitted(evaluate(&request, &only_once, &c)),
            (approval_id(3), ApprovalLifetime::Once)
        );
    }
    // A non-matching wider grant does not win (here: expired).
    let mut dead = wr.clone();
    dead.expires_at = Some(ts(-5));
    assert_eq!(
        permitted(evaluate(
            &request,
            &[dead, by_run.clone(), once.clone()],
            &c
        ))
        .0,
        approval_id(2)
    );
}

/// Among equal lifetimes: earliest `requested_at`, then the smallest approval id. Fails if store order,
/// latest request or the largest id decides.
#[test]
fn tie_break_earliest_requested_then_approval_id() {
    let request = plain();
    let mut early = approved(&request, 9, ApprovalLifetime::Once);
    early.requested_at = ts(-5000);
    let mut late = approved(&request, 1, ApprovalLifetime::Once);
    late.requested_at = ts(-100);
    for list in [
        vec![early.clone(), late.clone()],
        vec![late.clone(), early.clone()],
    ] {
        assert_eq!(
            permitted(evaluate(&request, &list, &ctx(None))).0,
            approval_id(9)
        );
    }
    let mut a = approved(&request, 5, ApprovalLifetime::Once);
    let mut b = approved(&request, 4, ApprovalLifetime::Once);
    a.requested_at = ts(-300);
    b.requested_at = ts(-300);
    for list in [vec![a.clone(), b.clone()], vec![b, a]] {
        assert_eq!(
            permitted(evaluate(&request, &list, &ctx(None))).0,
            approval_id(4)
        );
    }
}

/// `Mismatch` names the record whose first failing dimension is latest in table order, whatever the store
/// order. Fails if the earliest failure, the first record or the last record is reported instead.
#[test]
fn mismatch_reports_the_closest_record() {
    let request = rich();
    let c = ctx(Some(3));
    let mut far = approved(&request, 1, ApprovalLifetime::Once);
    far.operation_fingerprint = other_digest(&request); // fails at Fingerprint (G3)
    let mut near = approved(&request, 2, ApprovalLifetime::Once);
    near.expires_at = Some(ts(-1)); // fails at Expiry (G13)
    let mut middle = approved(&request, 3, ApprovalLifetime::Once);
    middle.workflow_id = Some(workflow(2)); // fails at Workflow (G4)
    let orders = [
        vec![far.clone(), near.clone(), middle.clone()],
        vec![near.clone(), far.clone(), middle.clone()],
        vec![middle.clone(), far.clone(), near.clone()],
        vec![middle, near, far],
    ];
    for list in orders {
        assert_eq!(
            evaluate(&request, &list, &c),
            mismatch(2, ScopeDimension::Expiry)
        );
    }
}

/// Ties in the failing dimension go to the first record in store order. Fails if the last record, or the
/// smallest id, is reported instead.
#[test]
fn mismatch_ties_go_to_store_order() {
    let request = plain();
    let mut a = approved(&request, 7, ApprovalLifetime::Once);
    a.consumed = true;
    let mut b = approved(&request, 3, ApprovalLifetime::Once);
    b.consumed = true;
    assert_eq!(
        evaluate(&request, &[a.clone(), b.clone()], &ctx(None)),
        mismatch(7, ScopeDimension::Consumption)
    );
    assert_eq!(
        evaluate(&request, &[b, a], &ctx(None)),
        mismatch(3, ScopeDimension::Consumption)
    );
}

/// A non-approved record never becomes the reported mismatch, even when it would be "closer". Fails if a
/// revoked record that matches every later dimension is named.
#[test]
fn mismatch_ignores_records_that_are_not_approved() {
    let request = plain();
    let revoked = record(&request, 1, ApprovalState::Revoked, ApprovalLifetime::Once);
    let mut far = approved(&request, 2, ApprovalLifetime::Once);
    far.operation_fingerprint = other_digest(&request);
    assert_eq!(
        evaluate(&request, &[revoked, far], &ctx(None)),
        mismatch(2, ScopeDimension::Fingerprint)
    );
}

/// A valid grant wins over a closer-looking mismatch in the same list. Fails if a mismatch is reported
/// while a grant matches.
#[test]
fn a_matching_grant_beats_mismatching_records() {
    let request = plain();
    let mut stale = approved(&request, 1, ApprovalLifetime::Once);
    stale.consumed = true;
    let good = approved(&request, 2, ApprovalLifetime::Once);
    assert_eq!(
        permitted(evaluate(&request, &[stale, good], &ctx(None))).0,
        approval_id(2)
    );
}

/// The dimension enumeration and its order are the reporting order. Fails if `ALL` or the derived order
/// of `ScopeDimension` drifts from the G1-G14 table.
#[test]
fn scope_dimension_order_is_the_table_order() {
    use ScopeDimension::*;
    assert_eq!(
        ScopeDimension::ALL,
        [
            Status,
            Capability,
            Fingerprint,
            Workflow,
            WorkflowRevision,
            Run,
            Repos,
            Destinations,
            ResourceConstraints,
            DataCategories,
            InferenceScope,
            Lifetime,
            Expiry,
            Consumption
        ]
    );
    assert!(ScopeDimension::ALL.windows(2).all(|w| w[0] < w[1]));
}

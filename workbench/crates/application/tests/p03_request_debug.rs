//! P03-D01 item 1 (M1): `Debug` of a permission request never shows argument values. Lead-owned.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test helpers outside #[test] functions"
)]

use application::permission::{
    ArgName, NormalisedDestination, OperationArg, OperationName, PermissionRequest, RequestedData,
};
use domain::contract::{ApprovalLifetime, DataCategory, FixedText};
use domain::vocab::Capability;

const SEEDED: &str = "pwt-test-FAKE-argument-value-7f3a9c";

fn request() -> PermissionRequest {
    PermissionRequest {
        capability: Capability::GitAcquire,
        operation: OperationName::parse("git.clone").expect("op"),
        arguments: vec![OperationArg {
            name: ArgName::parse("token-like").expect("name"),
            value: SEEDED.to_owned(),
        }],
        workflow: None,
        run_id: None,
        repo_ids: Vec::new(),
        destinations: vec![NormalisedDestination::parse("https://example.com").expect("dest")],
        resource_constraints: Vec::new(),
        data: vec![RequestedData::Category(DataCategory::RepositoryMetadata)],
        purpose: FixedText::PurposeDeveloperCheck.redacted(),
        lifetime: ApprovalLifetime::Once,
        validity_seconds: None,
        inference_scope: None,
    }
}

/// Fails if `OperationArg` (or `PermissionRequest` through it) prints the value again, e.g. if the
/// derived `Debug` is restored.
#[test]
fn request_debug_never_contains_an_argument_value() {
    let req = request();
    for text in [
        format!("{req:?}"),
        format!("{req:#?}"),
        format!("{:?}", req.arguments[0]),
    ] {
        assert!(!text.contains(SEEDED), "argument value leaked into Debug");
        assert!(
            !text.contains("7f3a9c"),
            "part of the argument value leaked into Debug"
        );
        assert!(
            text.contains("token-like"),
            "the argument name is expected in Debug"
        );
    }
}

/// Fails if the byte length is dropped or wrong (the name and length are the documented content).
#[test]
fn argument_debug_shows_name_and_length_only() {
    let arg = &request().arguments[0];
    assert_eq!(
        format!("{arg:?}"),
        format!(
            "OperationArg {{ name: {:?}, value_bytes: {} }}",
            arg.name,
            SEEDED.len()
        )
    );
}

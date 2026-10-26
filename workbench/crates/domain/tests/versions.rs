//! P01-D04b tests: the independent contract version constants (`domain::versions`) against their
//! literal expected values and against the canonical JSON schemas under `schemas/`.
//!
//! The schema files are read with `include_str!` at compile time; nothing is fetched or executed.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use domain::versions::{
    APP_CONTRACT_VERSION, STORE_SCHEMA_VERSION, UI_SCHEMA_VERSION, WRAPPER_PROTOCOL_VERSION,
};
use serde_json::Value;

const STORE_SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.3.schema.json");
const WRAPPER_REQUEST_SCHEMA: &str =
    include_str!("../../../../schemas/wrapper-request-v1.1.schema.json");

/// Collects the `const` of every `properties.<key>` entry anywhere in the document, with its path.
fn property_consts(node: &Value, key: &str, path: &str, out: &mut Vec<(String, Option<Value>)>) {
    match node {
        Value::Object(map) => {
            if let Some(Value::Object(props)) = map.get("properties")
                && let Some(Value::Object(prop)) = props.get(key)
            {
                out.push((
                    format!("{path}/properties/{key}"),
                    prop.get("const").cloned(),
                ));
            }
            for (k, v) in map {
                property_consts(v, key, &format!("{path}/{k}"), out);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                property_consts(v, key, &format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn consts_in(schema: &str, key: &str) -> Vec<(String, Option<Value>)> {
    let doc: Value = serde_json::from_str(schema).expect("schema parses as JSON");
    let mut out = Vec::new();
    property_consts(&doc, key, "", &mut out);
    out
}

fn assert_all_const(found: &[(String, Option<Value>)], expected: &str, at_least: usize) {
    assert!(
        found.len() >= at_least,
        "expected at least {at_least} occurrences, found {}",
        found.len()
    );
    for (path, value) in found {
        assert_eq!(
            value.as_ref().and_then(Value::as_str),
            Some(expected),
            "const at {path}"
        );
    }
}

#[test]
fn version_constants_have_the_expected_literal_values() {
    assert_eq!(STORE_SCHEMA_VERSION, "1.3");
    assert_eq!(WRAPPER_PROTOCOL_VERSION, "1.1");
    assert_eq!(UI_SCHEMA_VERSION, "1.0");
    assert_eq!(APP_CONTRACT_VERSION, "1.0");
}

#[test]
fn app_contract_version_is_the_same_value_through_both_paths() {
    assert_eq!(
        domain::versions::APP_CONTRACT_VERSION,
        domain::contract::APP_CONTRACT_VERSION
    );
}

#[test]
fn store_schema_version_matches_every_schema_version_const_in_the_store_schema() {
    // Ten store file types each pin `schema_version` (verified by reading the schema).
    let found = consts_in(STORE_SCHEMA, "schema_version");
    assert_all_const(&found, STORE_SCHEMA_VERSION, 10);
}

#[test]
fn store_schema_version_matches_every_schema_version_const_in_the_wrapper_request_schema() {
    let found = consts_in(WRAPPER_REQUEST_SCHEMA, "schema_version");
    assert_all_const(&found, STORE_SCHEMA_VERSION, 1);
}

#[test]
fn wrapper_protocol_version_matches_every_protocol_version_const_in_both_schemas() {
    for schema in [STORE_SCHEMA, WRAPPER_REQUEST_SCHEMA] {
        let found = consts_in(schema, "protocol_version");
        assert_all_const(&found, WRAPPER_PROTOCOL_VERSION, 1);
    }
}

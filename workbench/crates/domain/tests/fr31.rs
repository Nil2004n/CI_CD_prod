//! FR-31 enforcement (P02-T13; `planning/P02-T01-request.md` §6.6, §9; Appendix A
//! `planning/P02-T01-fr31-classification.md`). All data is synthetic.
//!
//! The classification table is `domain::store::classes::TABLE`. These tests prove it complete against the
//! embedded store schema, equal to Appendix A, and in agreement with the Rust field type and the
//! `deserialize_with` helper of every store model field, and they check that the read-back of class R and S
//! text re-escapes. The compile-time guards (no `Deserialize` for `RedactedText`, no `String` in a typed
//! field, `store_read` is `pub(crate)`) are `compile_fail` doctests of `domain/src/store/classes.rs`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::collections::{BTreeMap, BTreeSet};

use domain::contract::AdapterId;
use domain::store::classes::{self, Class, ROW_COUNT, Row, TABLE};
use domain::store::evidence::SourceRef;
use domain::store::log::EventType;
use domain::store::plan::{FileRecord, RendererSetting};
use domain::store::suppression::Suppression;
use domain::store::values as v;
use serde_json::{Value, json};

const SCHEMA: &str = include_str!("../../../../schemas/workbench-store-v1.4.schema.json");
const APPENDIX: &str = include_str!("../../../../planning/P02-T01-fr31-classification.md");

/// Every `domain::store` module that defines models, with its source. `mod.rs` (envelope types), `values.rs`
/// (newtypes) and `classes.rs` (this table) define no model with a plain-string property.
const MODEL_SOURCES: [(&str, &str); 13] = [
    ("approval", include_str!("../src/store/approval.rs")),
    ("config", include_str!("../src/store/config.rs")),
    ("conversation", include_str!("../src/store/conversation.rs")),
    ("evidence", include_str!("../src/store/evidence.rs")),
    ("log", include_str!("../src/store/log.rs")),
    ("plan", include_str!("../src/store/plan.rs")),
    ("provider", include_str!("../src/store/provider.rs")),
    ("rag", include_str!("../src/store/rag.rs")),
    ("report", include_str!("../src/store/report.rs")),
    ("repository", include_str!("../src/store/repository.rs")),
    ("suppression", include_str!("../src/store/suppression.rs")),
    ("tools", include_str!("../src/store/tools.rs")),
    ("workflow", include_str!("../src/store/workflow.rs")),
];

/// `$defs` that no store root references (request T01-D2); classified with the wrapper protocol in P06.
const WRAPPER_DEFS: [&str; 6] = [
    "WrapperRequest",
    "WrapperControl",
    "TerminationRecord",
    "StreamRecord",
    "DescendantCleanup",
    "TerminationPolicy",
];

/// `$defs` that are themselves a plain string. They carry no property and are used through `$ref`, so no
/// property of a model is "plain" because of them. `RelativePath` is the one: the `RelPath` newtype.
const DEF_LEVEL_STRINGS: [&str; 1] = ["RelativePath"];

// --- schema walker ----------------------------------------------------------------------------------------

fn schema() -> Value {
    serde_json::from_str(SCHEMA).expect("the store schema is JSON")
}

fn defs(schema: &Value) -> &serde_json::Map<String, Value> {
    schema["$defs"].as_object().expect("schema has $defs")
}

fn collect_refs(node: &Value, out: &mut BTreeSet<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    if let Some(name) = value.as_str().and_then(|s| s.strip_prefix("#/$defs/")) {
                        out.insert(name.to_owned());
                    }
                } else {
                    collect_refs(value, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs(item, out);
            }
        }
        _ => {}
    }
}

/// The `$defs` reachable from the ten store roots (the schema's top-level `oneOf`).
fn reachable_defs(schema: &Value) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    collect_refs(&schema["oneOf"], &mut roots);
    let mut seen = BTreeSet::new();
    let mut todo: Vec<String> = roots.into_iter().collect();
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let def = defs(schema)
            .get(&name)
            .unwrap_or_else(|| panic!("dangling $ref to {name}"));
        let mut refs = BTreeSet::new();
        collect_refs(def, &mut refs);
        todo.extend(refs);
    }
    seen
}

/// Appendix A's definition of a plain string: `type: string` without `enum`, `const`, `format`, `pattern` or
/// `$ref`; or a nullable / `oneOf` union containing one; or a string map (`additionalProperties`); or an array
/// of one (`items`).
fn is_plain(node: &Value) -> bool {
    let Some(map) = node.as_object() else {
        return false;
    };
    if map.get("type").and_then(Value::as_str) == Some("string")
        && !["enum", "const", "format", "pattern", "$ref"]
            .iter()
            .any(|k| map.contains_key(*k))
    {
        return true;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(members) = map.get(key).and_then(Value::as_array)
            && members.iter().any(is_plain)
        {
            return true;
        }
    }
    if map.get("type").and_then(Value::as_str) == Some("object")
        && map.get("additionalProperties").is_some_and(is_plain)
    {
        return true;
    }
    map.get("type").and_then(Value::as_str) == Some("array")
        && map.get("items").is_some_and(is_plain)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Scalar,
    Array,
    Map,
}

/// Container shape of a plain-string property, read from the schema node.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Shape {
    nullable: bool,
    kind: Kind,
}

fn shape_of(node: &Value) -> Shape {
    let members = node
        .get("anyOf")
        .or_else(|| node.get("oneOf"))
        .and_then(Value::as_array);
    let nullable = members.is_some_and(|m| {
        m.iter()
            .any(|x| x.get("type").and_then(Value::as_str) == Some("null"))
    });
    let core = members
        .and_then(|m| m.iter().find(|x| is_plain(x)))
        .unwrap_or(node);
    let kind = match core.get("type").and_then(Value::as_str) {
        Some("array") => Kind::Array,
        Some("object") => Kind::Map,
        _ => Kind::Scalar,
    };
    Shape { nullable, kind }
}

/// The Appendix A "Shape" cell for `shape`.
fn shape_cell(shape: Shape) -> String {
    let base = match shape.kind {
        Kind::Scalar => "string",
        Kind::Array => "array",
        Kind::Map => "map (values; keys `Identifier`)",
    };
    if shape.nullable {
        format!("{base}, nullable")
    } else {
        base.to_owned()
    }
}

type Walked = BTreeMap<(String, String), Shape>;

/// Every plain-string property of every reachable `$def`, with its shape.
fn plain_properties(schema: &Value) -> Walked {
    let mut out = Walked::new();
    for name in reachable_defs(schema) {
        let Some(props) = defs(schema)[&name]
            .get("properties")
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (prop, node) in props {
            if is_plain(node) {
                out.insert((name.clone(), prop.clone()), shape_of(node));
            }
        }
    }
    out
}

/// Differences between the walked schema and a classification table.
#[derive(Debug, PartialEq, Eq)]
struct Gaps {
    /// `Def.property` of plain strings without a row.
    unclassified: Vec<String>,
    /// `Def.property` of rows without a plain-string property.
    stale: Vec<String>,
}

fn gaps(walked: &Walked, table: &[Row]) -> Gaps {
    let rows: BTreeSet<(String, String)> = table
        .iter()
        .map(|r| (r.0.to_owned(), r.1.to_owned()))
        .collect();
    Gaps {
        unclassified: walked
            .keys()
            .filter(|k| !rows.contains(*k))
            .map(|(d, p)| format!("{d}.{p}"))
            .collect(),
        stale: rows
            .iter()
            .filter(|k| !walked.contains_key(*k))
            .map(|(d, p)| format!("{d}.{p}"))
            .collect(),
    }
}

/// A copy of `schema` with `node` added as property `prop` of `def`.
fn with_property(schema: &Value, def: &str, prop: &str, node: Value) -> Value {
    let mut copy = schema.clone();
    copy["$defs"][def]["properties"][prop] = node;
    copy
}

/// A copy of `schema` without property `prop` of `def`.
fn without_property(schema: &Value, def: &str, prop: &str) -> Value {
    let mut copy = schema.clone();
    copy["$defs"][def]["properties"]
        .as_object_mut()
        .expect("def has properties")
        .remove(prop)
        .unwrap_or_else(|| panic!("{def}.{prop} exists"));
    copy
}

// --- completeness ------------------------------------------------------------------------------------------

/// Fails if the number of reachable `$defs` is not 57, if any wrapper def becomes reachable, or if a def
/// other than the six wrapper defs is not reachable (request T01-D2: 63 defs, 6 of them wrapper-only).
#[test]
fn the_store_roots_reach_57_defs_and_no_wrapper_def() {
    let schema = schema();
    let reachable = reachable_defs(&schema);
    assert_eq!(reachable.len(), 57, "reachable defs: {reachable:?}");
    for wrapper in WRAPPER_DEFS {
        assert!(!reachable.contains(wrapper), "{wrapper} became reachable");
    }
    let all: BTreeSet<String> = defs(&schema).keys().cloned().collect();
    let unreachable: BTreeSet<String> = all.difference(&reachable).cloned().collect();
    let expected: BTreeSet<String> = WRAPPER_DEFS.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(
        unreachable, expected,
        "unreachable defs other than wrapper defs"
    );
}

/// Fails if any plain-string property of a reachable def has no table row, if any row names a def or
/// property that is not a plain string of a reachable def, or if the totals differ from Appendix A
/// (105 rows: E 6, N 66, R 27, S 6).
#[test]
fn every_plain_string_property_is_classified_and_every_row_names_one() {
    let walked = plain_properties(&schema());
    assert_eq!(
        gaps(&walked, &TABLE),
        Gaps {
            unclassified: Vec::new(),
            stale: Vec::new()
        }
    );
    assert_eq!(walked.len(), 105, "plain-string properties in the schema");
    assert_eq!(TABLE.len(), 105);
    assert_eq!(ROW_COUNT, 105);
    assert_eq!(classes::count(Class::ClosedEnum), 6, "E");
    assert_eq!(classes::count(Class::Newtype), 66, "N");
    assert_eq!(classes::count(Class::Redacted), 27, "R");
    assert_eq!(classes::count(Class::Sanitised), 6, "S");
    let letters: usize = Class::ALL.iter().map(|c| classes::count(*c)).sum();
    assert_eq!(letters, 105, "every row has one of the four classes");
}

/// Fails if two rows name the same property, if `lookup` or `rows_of` disagree with the table, or if the
/// 24 validated newtypes of Appendix A are not exactly the class N type names.
#[test]
fn the_table_has_unique_rows_and_consistent_helpers() {
    let keys: BTreeSet<(&str, &str)> = TABLE.iter().map(|r| (r.0, r.1)).collect();
    assert_eq!(keys.len(), TABLE.len(), "duplicate row");
    for row in TABLE {
        assert_eq!(classes::lookup(row.0, row.1), Some(row));
    }
    assert_eq!(classes::lookup("Workflow", "not_a_property"), None);
    assert_eq!(classes::lookup("NotADef", "name"), None);
    for class in Class::ALL {
        let from_iter = classes::rows_of(class).count();
        assert_eq!(from_iter, classes::count(class), "{class:?}");
        assert!(classes::rows_of(class).all(|r| r.2 == class));
    }
    assert_eq!(
        Class::ALL.map(Class::letter),
        ['E', 'N', 'R', 'S'],
        "Appendix A letters"
    );
    let n_types: BTreeSet<&str> = classes::rows_of(Class::Newtype).map(|r| r.3).collect();
    assert_eq!(n_types.len(), 24, "Appendix A: 24 validated newtypes used");
    assert_eq!(
        n_types,
        newtypes()
            .iter()
            .map(|(name, _)| *name)
            .collect::<BTreeSet<_>>()
    );
}

/// Real Rust type name of a type (last path segment).
fn type_name<T>() -> &'static str {
    std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .expect("type_name is never empty")
}

/// The 24 validated newtypes of Appendix A, each with the name Rust reports for the imported type. Fails to
/// compile if a newtype is removed from `store::values`; the equality test above fails if a class N row
/// names a type that is not listed.
fn newtypes() -> [(&'static str, &'static str); 24] {
    [
        ("AppRelPath", type_name::<v::AppRelPath>()),
        ("CanonicalRemote", type_name::<v::CanonicalRemote>()),
        ("CredentialRef", type_name::<v::CredentialRef>()),
        ("Destination", type_name::<v::Destination>()),
        ("EventCode", type_name::<v::EventCode>()),
        ("FrameworkId", type_name::<v::FrameworkId>()),
        ("GitRefName", type_name::<v::GitRefName>()),
        ("IanaTimezone", type_name::<v::IanaTimezone>()),
        ("Identifier", type_name::<v::Identifier>()),
        ("LocalPath", type_name::<v::LocalPath>()),
        ("LocalTime", type_name::<v::LocalTime>()),
        ("ModelId", type_name::<v::ModelId>()),
        ("PathPattern", type_name::<v::PathPattern>()),
        ("PolicyId", type_name::<v::PolicyId>()),
        ("ProviderId", type_name::<v::ProviderId>()),
        ("RedactionVersion", type_name::<v::RedactionVersion>()),
        ("RelPath", type_name::<v::RelPath>()),
        ("RemoteUrl", type_name::<v::RemoteUrl>()),
        ("RendererId", type_name::<v::RendererId>()),
        ("SchemaResourceId", type_name::<v::SchemaResourceId>()),
        ("Slug", type_name::<v::Slug>()),
        ("SpdxExpression", type_name::<v::SpdxExpression>()),
        ("ToolName", type_name::<v::ToolName>()),
        ("VersionLabel", type_name::<v::VersionLabel>()),
    ]
}

/// Fails if a listed newtype name differs from the name of the Rust type it stands for (a typo in the list,
/// or a rename in `store::values`).
#[test]
fn the_newtype_list_names_real_types() {
    for (name, actual) in newtypes() {
        assert_eq!(name, actual, "listed name vs Rust type name");
    }
}

/// Fails if the class E type names are not real closed enums that reject unknown wire values.
#[test]
fn class_e_types_are_closed_enums() {
    let e_types: BTreeSet<&str> = classes::rows_of(Class::ClosedEnum).map(|r| r.3).collect();
    assert_eq!(e_types, BTreeSet::from(["AdapterId", "EventType"]));
    assert_eq!(type_name::<AdapterId>(), "AdapterId");
    assert_eq!(type_name::<EventType>(), "EventType");
    assert!(serde_json::from_value::<AdapterId>(json!("not-an-adapter")).is_err());
    assert!(serde_json::from_value::<EventType>(json!("not-an-event")).is_err());
    assert!(serde_json::from_value::<AdapterId>(json!("yamllint")).is_ok());
    assert!(EventType::from_wire(EventType::ALL[0].as_str()).is_some());
}

// --- negative controls for the walker ----------------------------------------------------------------------

fn plain_string() -> Value {
    json!({"type": "string"})
}

/// Fails if the walker misses a synthetic plain-string property (reporting it as classified), in any of the
/// four shapes Appendix A covers, or if it reports properties that Appendix A excludes (`enum`, `const`,
/// `format`, `pattern`, `$ref`).
#[test]
fn a_synthetic_extra_plain_string_property_is_reported_unclassified() {
    let base = schema();
    let cases = [
        ("string", plain_string()),
        (
            "nullable",
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
        ),
        (
            "array",
            json!({"type": "array", "items": {"type": "string"}}),
        ),
        (
            "map",
            json!({"type": "object", "additionalProperties": {"type": "string"}}),
        ),
        (
            "nullable via oneOf",
            json!({"oneOf": [{"type": "string"}, {"type": "null"}]}),
        ),
        (
            "array of nullable strings",
            json!({"type": "array", "items": {"anyOf": [{"type": "string"}, {"type": "null"}]}}),
        ),
    ];
    for (label, node) in cases {
        let modified = with_property(&base, "Workflow", "zz_synthetic", node);
        assert_eq!(
            gaps(&plain_properties(&modified), &TABLE),
            Gaps {
                unclassified: vec![String::from("Workflow.zz_synthetic")],
                stale: Vec::new()
            },
            "{label}"
        );
    }
    let excluded = [
        json!({"type": "string", "enum": ["a"]}),
        json!({"type": "string", "const": "a"}),
        json!({"type": "string", "format": "uuid"}),
        json!({"type": "string", "pattern": "^a$"}),
        json!({"$ref": "#/$defs/Digest"}),
        json!({"type": "integer"}),
        json!({"type": "array", "items": {"type": "string", "enum": ["a"]}}),
        json!({"type": "object", "additionalProperties": {"type": "string", "format": "uuid"}}),
    ];
    for node in excluded {
        let modified = with_property(&base, "Workflow", "zz_synthetic", node.clone());
        assert_eq!(
            gaps(&plain_properties(&modified), &TABLE).unclassified,
            Vec::<String>::new(),
            "must not be reported: {node}"
        );
    }
}

/// Fails if removing any single row from the table is not reported as exactly that unclassified property.
#[test]
fn removing_any_table_row_is_reported_as_a_gap() {
    let walked = plain_properties(&schema());
    for skip in 0..TABLE.len() {
        let mut table = TABLE.to_vec();
        let removed = table.remove(skip);
        assert_eq!(
            gaps(&walked, &table),
            Gaps {
                unclassified: vec![format!("{}.{}", removed.0, removed.1)],
                stale: Vec::new()
            },
            "row {skip}"
        );
    }
}

/// Fails if a row naming a missing def or property, or a property that was deleted from the schema, is not
/// reported as stale.
#[test]
fn a_row_without_a_property_is_reported_stale() {
    let base = schema();
    let mut table = TABLE.to_vec();
    table.push(("Workflow", "zz_missing", Class::Redacted, "RedactedText"));
    table.push(("ZzNoSuchDef", "name", Class::Newtype, "Identifier"));
    assert_eq!(
        gaps(&plain_properties(&base), &table),
        Gaps {
            unclassified: Vec::new(),
            stale: vec![
                String::from("Workflow.zz_missing"),
                String::from("ZzNoSuchDef.name")
            ]
        }
    );
    let modified = without_property(&base, "Workflow", "name");
    assert_eq!(
        gaps(&plain_properties(&modified), &TABLE),
        Gaps {
            unclassified: Vec::new(),
            stale: vec![String::from("Workflow.name")]
        }
    );
    // A property that stops being a plain string (it becomes an enum) is stale too.
    let enumerated = with_property(
        &base,
        "Workflow",
        "name",
        json!({"type": "string", "enum": ["a"]}),
    );
    assert_eq!(
        gaps(&plain_properties(&enumerated), &TABLE).stale,
        vec![String::from("Workflow.name")]
    );
}

/// Fails if reachability is not honoured: a plain string of a wrapper def stays out of scope while no store
/// root reaches it, and comes into scope (reported unclassified) once a store def references it.
#[test]
fn reachability_decides_the_scope() {
    let base = schema();
    let unreachable = with_property(&base, "WrapperRequest", "zz_synthetic", plain_string());
    assert_eq!(
        gaps(&plain_properties(&unreachable), &TABLE).unclassified,
        Vec::<String>::new()
    );
    let reached = with_property(
        &unreachable,
        "Config",
        "zz_wrapper",
        json!({"$ref": "#/$defs/WrapperRequest"}),
    );
    assert!(reachable_defs(&reached).contains("WrapperRequest"));
    let found = gaps(&plain_properties(&reached), &TABLE).unclassified;
    assert!(
        found.iter().any(|s| s == "WrapperRequest.zz_synthetic"),
        "plain strings of a newly reachable def must surface: {found:?}"
    );
    assert!(
        found
            .iter()
            .all(|s| WRAPPER_DEFS.iter().any(|w| s.starts_with(&format!("{w}.")))),
        "only the newly reachable wrapper defs surface: {found:?}"
    );
}

// --- deep scan: no plain string hides where the walker does not look -----------------------------------------

fn plain_leaves(node: &Value, path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    if let Some(map) = node.as_object() {
        if map.get("type").and_then(Value::as_str) == Some("string")
            && !["enum", "const", "format", "pattern", "$ref"]
                .iter()
                .any(|k| map.contains_key(*k))
        {
            out.push(path.clone());
        }
        for (key, value) in map {
            path.push(key.clone());
            plain_leaves(value, path, out);
            path.pop();
        }
    } else if let Some(items) = node.as_array() {
        for (i, value) in items.iter().enumerate() {
            path.push(i.to_string());
            plain_leaves(value, path, out);
            path.pop();
        }
    }
}

/// Fails if a plain-string node exists in a reachable def at a position the property walker does not examine
/// (for example under `patternProperties`, `allOf`, `prefixItems` or a nested inline `properties`), because
/// the property walker, like Appendix A, only follows `anyOf`/`oneOf`, `items` and `additionalProperties`
/// from a property. Def-level aliases are listed in `DEF_LEVEL_STRINGS`.
#[test]
fn no_plain_string_hides_at_a_position_the_walker_does_not_follow() {
    let schema = schema();
    let walked: BTreeSet<(String, String)> = plain_properties(&schema).into_keys().collect();
    let mut deep = BTreeSet::new();
    for name in reachable_defs(&schema) {
        let mut leaves = Vec::new();
        plain_leaves(&defs(&schema)[&name], &mut Vec::new(), &mut leaves);
        for path in leaves {
            if path.is_empty() {
                assert!(
                    DEF_LEVEL_STRINGS.contains(&name.as_str()),
                    "{name} is a plain string at def level and is not listed"
                );
                continue;
            }
            assert!(
                path.len() >= 2 && path[0] == "properties",
                "{name}: plain string at an unexamined position {path:?}"
            );
            assert!(
                path[2..].iter().all(|seg| matches!(
                    seg.as_str(),
                    "anyOf" | "oneOf" | "items" | "additionalProperties"
                ) || seg.chars().all(|c| c.is_ascii_digit())),
                "{name}: plain string at an unexamined position {path:?}"
            );
            deep.insert((name.clone(), path[1].clone()));
        }
    }
    assert_eq!(deep, walked);
}

// --- Appendix A equality -------------------------------------------------------------------------------------

/// One parsed Appendix A table row.
#[derive(Debug, PartialEq, Eq)]
struct AppendixRow {
    number: usize,
    def: String,
    property: String,
    shape: String,
    class: char,
    type_name: String,
}

fn appendix_rows() -> Vec<AppendixRow> {
    let mut rows = Vec::new();
    for line in APPENDIX.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // `| # | def | prop | shape | class | type |` splits into 8 cells with empty first and last.
        if cells.len() != 8 {
            continue;
        }
        let Ok(number) = cells[1].parse::<usize>() else {
            continue;
        };
        let inner = |cell: &str| cell.trim_matches('`').to_owned();
        rows.push(AppendixRow {
            number,
            def: cells[2].to_owned(),
            property: inner(cells[3]),
            shape: cells[4].to_owned(),
            class: cells[5].chars().next().expect("class cell"),
            type_name: inner(cells[6])
                .split_whitespace()
                .next()
                .expect("type cell")
                .to_owned(),
        });
    }
    rows
}

/// Fails if the table differs from Appendix A in any row: order, def, property, class letter, type name, or
/// the shape cell (checked against the shape read from the schema).
#[test]
fn the_table_equals_appendix_a_row_for_row() {
    let appendix = appendix_rows();
    assert_eq!(appendix.len(), 105, "Appendix A rows");
    let walked = plain_properties(&schema());
    for (i, (row, app)) in TABLE.iter().zip(&appendix).enumerate() {
        assert_eq!(app.number, i + 1, "Appendix A numbering");
        assert_eq!(
            (row.0, row.1),
            (app.def.as_str(), app.property.as_str()),
            "row {}",
            i + 1
        );
        assert_eq!(row.2.letter(), app.class, "row {} class", i + 1);
        assert_eq!(row.3, app.type_name, "row {} type", i + 1);
        let shape = walked[&(row.0.to_owned(), row.1.to_owned())];
        assert_eq!(shape_cell(shape), app.shape, "row {} shape", i + 1);
    }
}

// --- model source parser -------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    /// The type with all whitespace removed.
    ty: String,
    /// Attributes in order, each without the surrounding `#[` `]`.
    attrs: Vec<String>,
}

/// Source with `//` comments (documentation comments included) cut off.
fn strip_comments(src: &str) -> String {
    src.lines()
        .map(|l| l.find("//").map_or(l, |i| &l[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Text after the opening delimiter up to the matching closer, and the remainder after it. Quotes are
/// respected.
fn take_balanced(s: &str, open: char, close: char) -> (&str, &str) {
    let mut depth = 1_usize;
    let mut in_str = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_str = !in_str,
            _ if in_str => {}
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return (&s[..i], &s[i + c.len_utf8()..]);
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced {open}{close}");
}

/// Splits a struct body at top-level commas.
fn split_fields(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start, mut in_str) = (0_i32, 0, false);
    for (i, c) in body.char_indices() {
        match c {
            '"' => in_str = !in_str,
            _ if in_str => {}
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

fn parse_field(item: &str) -> Option<Field> {
    let mut rest = item.trim();
    let mut attrs = Vec::new();
    while let Some(after) = rest.strip_prefix("#[") {
        let (attr, tail) = take_balanced(after, '[', ']');
        attrs.push(attr.split_whitespace().collect::<Vec<_>>().join(" "));
        rest = tail.trim_start();
    }
    if rest.is_empty() {
        return None;
    }
    let rest = rest
        .strip_prefix("pub(crate) ")
        .or_else(|| rest.strip_prefix("pub "))
        .unwrap_or(rest);
    let (name, ty) = rest.split_once(':')?;
    Some(Field {
        name: name.trim().trim_start_matches("r#").to_owned(),
        // Whitespace removed, and the trailing comma rustfmt leaves in a wrapped generic list dropped.
        ty: ty
            .split_whitespace()
            .collect::<String>()
            .replace(",>", ">")
            .replace(",)", ")"),
        attrs,
    })
}

/// `pub struct Name { fields }` definitions of `src`, in source order. Tuple and unit structs are skipped.
fn parse_structs(src: &str) -> Vec<(String, Vec<Field>)> {
    let text = strip_comments(src);
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("pub struct ") {
        let after = &rest[at + "pub struct ".len()..];
        let len = after
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let (name, tail) = after.split_at(len);
        rest = tail;
        let Some(body_start) = tail.trim_start().strip_prefix('{') else {
            continue;
        };
        let (body, remainder) = take_balanced(body_start, '{', '}');
        rest = remainder;
        out.push((
            name.to_owned(),
            split_fields(body)
                .into_iter()
                .filter_map(parse_field)
                .collect(),
        ));
    }
    out
}

/// All model structs of the store, by name. Fails (panics) if two modules define a struct of the same name.
fn models() -> BTreeMap<String, Vec<Field>> {
    let mut all = BTreeMap::new();
    for (module, src) in MODEL_SOURCES {
        for (name, fields) in parse_structs(src) {
            assert!(
                all.insert(name.clone(), fields).is_none(),
                "{name} defined twice (second in {module})"
            );
        }
    }
    all
}

/// Fails if the parser mishandles attributes, a multi-line type, a raw identifier, nested generics, a comment
/// or a tuple struct, which would silently drop or merge model fields.
#[test]
fn the_source_parser_reads_fields_attributes_and_types() {
    let src = r#"
/// Doc with `pub struct Fake { a: String }` in it.
#[derive(Debug)]
pub struct Sample {
    /// Docs.
    pub plain: RedactedText, // trailing comment, with a comma
    #[serde(deserialize_with = "crate::text::store_read::redacted_opt")]
    #[serde(other)]
    pub opt: Option<RedactedText>,
    pub r#type: BTreeMap<
        Identifier,
        RedactedText,
    >,
    pub nested: Vec<Option<(A, B)>>,
}
pub struct Tuple(String);
pub struct Last { pub only: u32 }
"#;
    let parsed = parse_structs(src);
    let names: Vec<&str> = parsed.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Sample", "Last"]);
    let fields = &parsed[0].1;
    let summary: Vec<(&str, &str, usize)> = fields
        .iter()
        .map(|f| (f.name.as_str(), f.ty.as_str(), f.attrs.len()))
        .collect();
    assert_eq!(
        summary,
        [
            ("plain", "RedactedText", 0),
            ("opt", "Option<RedactedText>", 2),
            ("type", "BTreeMap<Identifier,RedactedText>", 0),
            ("nested", "Vec<Option<(A,B)>>", 0),
        ]
    );
    assert_eq!(
        fields[1].attrs[0],
        "serde(deserialize_with = \"crate::text::store_read::redacted_opt\")"
    );
    assert_eq!(parsed[1].1[0].name, "only");
}

/// Fails if a store module is added without being listed in `MODEL_SOURCES` (its models would escape the
/// agreement checks), or if a module listed here is not declared.
#[test]
fn every_store_module_is_scanned_or_excluded() {
    let declared: BTreeSet<&str> = include_str!("../src/store/mod.rs")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub mod ")?.strip_suffix(';'))
        .collect();
    let mut covered: BTreeSet<&str> = MODEL_SOURCES.iter().map(|(m, _)| *m).collect();
    covered.extend(["classes", "values"]);
    covered.extend(V15_SOURCES.iter().map(|(name, _)| *name));
    assert_eq!(declared, covered);
}

const V15_SOURCES: [(&str, &str); 3] = [
    ("approval15", include_str!("../src/store/approval15.rs")),
    ("repository15", include_str!("../src/store/repository15.rs")),
    ("roots15", include_str!("../src/store/roots15.rs")),
];

/// Check every versioned model against its classified 1.4 counterpart, with
/// explicit assertions for each 1.5 change. This also preserves redaction helpers.
#[test]
fn version15_models_preserve_classified_fields() {
    let legacy = models();
    for (module, src) in &V15_SOURCES[..2] {
        let parsed = parse_structs(src);
        let expected_names: &[&str] = if *module == "approval15" {
            &["GitApprovalContext", "Approval", "ApprovalFile"]
        } else {
            &["Repository", "RepositoryFile"]
        };
        assert_eq!(
            parsed.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            expected_names
        );
        for (name, mut fields) in parsed {
            if name == "GitApprovalContext" {
                assert_eq!(
                    fields
                        .iter()
                        .map(|f| (f.name.as_str(), f.ty.as_str()))
                        .collect::<Vec<_>>(),
                    [
                        ("stage", "GitApprovalStage"),
                        ("repo_id", "RepoId"),
                        ("ref_kind", "Option<RefKind>"),
                        ("ref_value", "Option<GitRefName>")
                    ]
                );
                assert!(fields.iter().all(|f| f.attrs.is_empty()));
                continue;
            }
            let extra = match name.as_str() {
                "Approval" => Some(("git_approval_context", "Option<GitApprovalContext>")),
                "Repository" => Some(("last_git_version", "Option<VersionLabel>")),
                _ => None,
            };
            if let Some((key, ty)) = extra {
                let index = fields
                    .iter()
                    .position(|f| f.name == key)
                    .expect("1.5 field");
                let f = fields.remove(index);
                assert_eq!(f.ty, ty);
                assert!(f.attrs.is_empty());
            }
            if name == "Repository" {
                for key in ["default_ref_kind", "default_ref_value"] {
                    let f = fields
                        .iter_mut()
                        .find(|f| f.name == key)
                        .expect("reference field");
                    f.ty =
                        f.ty.strip_prefix("Option<")
                            .and_then(|s| s.strip_suffix('>'))
                            .expect("nullable reference")
                            .to_owned();
                }
            }
            assert_eq!(fields, legacy[&name], "{module}::{name}");
        }
    }
    let roots = V15_SOURCES[2].1;
    assert_eq!(roots.matches("root!(").count(), 6);
    assert!(roots.contains("pub schema_version: SchemaVersion15"));
    assert!(roots.contains("pub $field: $field_type"));
}

// --- class / type agreement ----------------------------------------------------------------------------------

/// The element type a row stands for: the text type for classes R and S, the listed type otherwise.
fn element_type(row: Row) -> &'static str {
    match row.2 {
        Class::Redacted => "RedactedText",
        Class::Sanitised => "SanitisedText",
        Class::Newtype | Class::ClosedEnum => row.3,
    }
}

/// The Rust type the schema shape requires around `element`.
fn expected_type(shape: Shape, element: &str) -> String {
    let inner = match shape.kind {
        Kind::Scalar => element.to_owned(),
        Kind::Array => format!("Vec<{element}>"),
        Kind::Map => format!("BTreeMap<Identifier,{element}>"),
    };
    if shape.nullable {
        format!("Option<{inner}>")
    } else {
        inner
    }
}

/// The `store_read` helper a class R field of this shape must use.
fn expected_helper(shape: Shape) -> Option<&'static str> {
    match (shape.nullable, shape.kind) {
        (false, Kind::Scalar) => Some("redacted"),
        (true, Kind::Scalar) => Some("redacted_opt"),
        (false, Kind::Array) => Some("redacted_vec"),
        (false, Kind::Map) => Some("redacted_map"),
        (true, Kind::Array | Kind::Map) => None,
    }
}

/// The `deserialize_with` target of a field, if it has one.
fn deserialize_with(field: &Field) -> Option<&str> {
    field.attrs.iter().find_map(|a| {
        let (_, tail) = a.split_once("deserialize_with = \"")?;
        tail.split_once('"').map(|(target, _)| target)
    })
}

/// Why a model field disagrees with its row, or `Ok`.
fn check_row(row: Row, shape: Shape, field: &Field) -> Result<(), String> {
    let label = format!("{}.{}", row.0, row.1);
    if matches!(row.2, Class::Redacted | Class::Sanitised) && row.3 != element_type(row) {
        return Err(format!(
            "{label}: row type {} is not {}",
            row.3,
            element_type(row)
        ));
    }
    let want = expected_type(shape, element_type(row));
    if field.ty != want {
        return Err(format!(
            "{label}: field type `{}`, expected `{want}`",
            field.ty
        ));
    }
    let helper = deserialize_with(field);
    match row.2 {
        Class::Redacted => {
            let name = expected_helper(shape)
                .ok_or_else(|| format!("{label}: no store_read helper exists for this shape"))?;
            let want_helper = format!("crate::text::store_read::{name}");
            if helper != Some(want_helper.as_str()) {
                return Err(format!(
                    "{label}: deserialize_with is {helper:?}, expected `{want_helper}`"
                ));
            }
        }
        Class::Sanitised | Class::Newtype | Class::ClosedEnum => {
            if let Some(h) = helper {
                return Err(format!("{label}: unexpected deserialize_with `{h}`"));
            }
        }
    }
    let forbidden = field
        .attrs
        .iter()
        .find(|a| a.contains("default") || a.contains("skip") || a.contains("flatten"));
    if let Some(attr) = forbidden {
        return Err(format!("{label}: forbidden serde attribute `{attr}`"));
    }
    if matches!(row.2, Class::Redacted)
        && field
            .attrs
            .iter()
            .filter(|a| a.contains("deserialize_with"))
            .count()
            != 1
    {
        return Err(format!(
            "{label}: expected exactly one deserialize_with attribute"
        ));
    }
    Ok(())
}

/// Fails if any of the 105 model fields differs from its row: the Rust type (element type per class,
/// container per schema shape), the `store_read` helper of a class R field (exact name, one attribute), or
/// a class S/N/E field carries a `deserialize_with`, `default`, `skip` or `flatten` attribute. Covers all 27 R
/// rows, the 6 S rows, 66 N rows and 6 E rows.
#[test]
fn every_row_matches_the_rust_type_and_helper_of_its_field() {
    let models = models();
    let walked = plain_properties(&schema());
    let mut checked: BTreeMap<Class, usize> = BTreeMap::new();
    for row in TABLE {
        let fields = models
            .get(row.0)
            .unwrap_or_else(|| panic!("no model struct {}", row.0));
        let field = fields
            .iter()
            .find(|f| f.name == row.1)
            .unwrap_or_else(|| panic!("{}.{} is not a model field", row.0, row.1));
        let shape = walked[&(row.0.to_owned(), row.1.to_owned())];
        if let Err(why) = check_row(row, shape, field) {
            panic!("{why}");
        }
        *checked.entry(row.2).or_default() += 1;
    }
    assert_eq!(
        checked,
        BTreeMap::from([
            (Class::ClosedEnum, 6),
            (Class::Newtype, 66),
            (Class::Redacted, 27),
            (Class::Sanitised, 6),
        ])
    );
}

/// Fails if `check_row` accepts a mismatch: a bare `String`, the wrong text type, the wrong newtype, the
/// wrong container, a missing, wrong or doubled helper, a helper on a class that must not have one, or an
/// extra `default` attribute.
#[test]
fn the_row_check_rejects_mismatches() {
    let plain = Shape {
        nullable: false,
        kind: Kind::Scalar,
    };
    let nullable = Shape {
        nullable: true,
        kind: Kind::Scalar,
    };
    let array = Shape {
        nullable: false,
        kind: Kind::Array,
    };
    let map = Shape {
        nullable: false,
        kind: Kind::Map,
    };
    let helper = |n: &str| format!("serde(deserialize_with = \"crate::text::store_read::{n}\")");
    let field = |ty: &str, attrs: Vec<String>| Field {
        name: String::from("f"),
        ty: ty.to_owned(),
        attrs,
    };
    let r: Row = ("D", "f", Class::Redacted, "RedactedText");
    let s: Row = ("D", "f", Class::Sanitised, "SanitisedText");
    let n: Row = ("D", "f", Class::Newtype, "Slug");
    let e: Row = ("D", "f", Class::ClosedEnum, "AdapterId");

    // Accepted controls, so the rejections below are caused by the mismatch alone.
    assert!(check_row(r, plain, &field("RedactedText", vec![helper("redacted")])).is_ok());
    assert!(
        check_row(
            r,
            nullable,
            &field("Option<RedactedText>", vec![helper("redacted_opt")])
        )
        .is_ok()
    );
    assert!(
        check_row(
            r,
            array,
            &field("Vec<RedactedText>", vec![helper("redacted_vec")])
        )
        .is_ok()
    );
    assert!(
        check_row(
            r,
            map,
            &field(
                "BTreeMap<Identifier,RedactedText>",
                vec![helper("redacted_map")]
            )
        )
        .is_ok()
    );
    assert!(check_row(s, array, &field("Vec<SanitisedText>", Vec::new())).is_ok());
    assert!(check_row(n, nullable, &field("Option<Slug>", Vec::new())).is_ok());
    assert!(check_row(e, plain, &field("AdapterId", Vec::new())).is_ok());

    let rejected = [
        check_row(r, plain, &field("String", vec![helper("redacted")])),
        check_row(r, plain, &field("SanitisedText", vec![helper("redacted")])),
        check_row(r, plain, &field("RedactedText", Vec::new())),
        check_row(
            r,
            plain,
            &field("RedactedText", vec![helper("redacted_opt")]),
        ),
        check_row(
            r,
            nullable,
            &field("Option<RedactedText>", vec![helper("redacted")]),
        ),
        check_row(
            r,
            nullable,
            &field("RedactedText", vec![helper("redacted_opt")]),
        ),
        check_row(
            r,
            array,
            &field("Vec<RedactedText>", vec![helper("redacted")]),
        ),
        check_row(
            r,
            map,
            &field(
                "BTreeMap<Identifier,RedactedText>",
                vec![helper("redacted")],
            ),
        ),
        check_row(
            r,
            plain,
            &field("RedactedText", vec![helper("redacted"), helper("redacted")]),
        ),
        check_row(
            r,
            plain,
            &field(
                "RedactedText",
                vec![helper("redacted"), String::from("serde(default)")],
            ),
        ),
        check_row(s, plain, &field("RedactedText", Vec::new())),
        check_row(s, plain, &field("String", Vec::new())),
        check_row(s, plain, &field("SanitisedText", vec![helper("redacted")])),
        check_row(s, array, &field("SanitisedText", Vec::new())),
        check_row(n, plain, &field("String", Vec::new())),
        check_row(n, plain, &field("Identifier", Vec::new())),
        check_row(n, plain, &field("Option<Slug>", Vec::new())),
        check_row(n, array, &field("Slug", Vec::new())),
        check_row(n, plain, &field("Slug", vec![String::from("serde(skip)")])),
        check_row(e, plain, &field("String", Vec::new())),
        check_row(e, plain, &field("EventType", Vec::new())),
        check_row(
            r,
            Shape {
                nullable: true,
                kind: Kind::Array,
            },
            &field("Option<Vec<RedactedText>>", Vec::new()),
        ),
        check_row(
            ("D", "f", Class::Redacted, "SanitisedText"),
            plain,
            &field("RedactedText", vec![helper("redacted")]),
        ),
    ];
    for (i, result) in rejected.iter().enumerate() {
        assert!(result.is_err(), "mismatch case {i} was accepted");
    }
}

/// Fails if a model has a `RedactedText` field with no class R row, or a `SanitisedText` field with no class
/// S row, in any store module (a new free-text field cannot slip in unclassified), or if a class R or S row
/// names a field that does not use its text type.
#[test]
fn every_text_field_of_every_model_has_its_row() {
    let models = models();
    let mut redacted = BTreeSet::new();
    let mut sanitised = BTreeSet::new();
    for (name, fields) in &models {
        for field in fields {
            if field.ty.contains("RedactedText") {
                redacted.insert(format!("{name}.{}", field.name));
            }
            if field.ty.contains("SanitisedText") {
                sanitised.insert(format!("{name}.{}", field.name));
            }
        }
    }
    let rows = |class| -> BTreeSet<String> {
        classes::rows_of(class)
            .map(|r| format!("{}.{}", r.0, r.1))
            .collect()
    };
    assert_eq!(redacted, rows(Class::Redacted));
    assert_eq!(sanitised, rows(Class::Sanitised));
    assert_eq!(redacted.len(), 27);
    assert_eq!(sanitised.len(), 6);
}

// --- seal behaviour ------------------------------------------------------------------------------------------

/// Control, bidi, zero-width, separator and invisible code points around ordinary text, written as JSON
/// escapes so the document itself is plain ASCII. TAB and LF are kept by the escape set.
const HOSTILE_JSON: &str = r"a\u001bb\u202ec\u2028d\u200be\u00adf\tg\nh";
const HOSTILE_ESCAPED: &str = "a<U+001B>b<U+202E>c<U+2028>d<U+200B>e<U+00AD>f\tg\nh";

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const TS: &str = "2026-10-16T05:25:00Z";

fn uid(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

/// A document with `placeholder` replaced by the hostile JSON escapes inside string values.
fn plant(template: &str) -> String {
    assert!(template.contains("@@"), "template has a placeholder");
    template.replace("@@", HOSTILE_JSON)
}

/// Serialised JSON text must carry the escaped form and no raw dangerous code point.
fn assert_serialised_is_safe(json_text: &str) {
    assert!(json_text.contains("<U+001B>"), "{json_text}");
    assert!(json_text.contains("<U+202E>"), "{json_text}");
    for raw in ['\u{1b}', '\u{202e}', '\u{2028}', '\u{200b}', '\u{ad}'] {
        assert!(
            !json_text.contains(raw),
            "raw U+{:04X} in output",
            u32::from(raw)
        );
    }
    for escaped in ["\\u001b", "\\u202e", "\\u2028", "\\u200b", "\\u00ad"] {
        assert!(
            !json_text.contains(escaped),
            "JSON escape {escaped} in output"
        );
    }
}

/// Fails if a plain class R field (`Suppression.rule_id`, `.reason`, `.actor`) keeps an ESC, bidi override,
/// separator, zero-width or soft-hyphen character from the JSON, keeps escaping non-idempotent across a
/// second round trip, or serialises a raw dangerous code point.
#[test]
fn seal_plain_redacted_fields_are_re_escaped_on_read() {
    let doc = plant(&format!(
        r#"{{"suppression_id":"{}","repo_id":"{}","finding_fingerprint":"{DIGEST}","rule_id":"@@",
            "reason":"@@","created_at":"{TS}","expires_at":null,"actor":"@@","enabled":true}}"#,
        uid(1),
        uid(2)
    ));
    assert!(
        doc.contains(r"\u001b"),
        "the input really contains an ESC escape"
    );
    let model: Suppression = serde_json::from_str(&doc).expect("decodes");
    for text in [&model.rule_id, &model.reason, &model.actor] {
        assert_eq!(text.as_str(), HOSTILE_ESCAPED);
    }
    let out = serde_json::to_string(&model).expect("serialises");
    assert_serialised_is_safe(&out);
    let again: Suppression = serde_json::from_str(&out).expect("round trip");
    assert_eq!(again, model, "escaping is idempotent");
}

/// Fails if a nullable class R field (`SourceRef.resource_identifier`) is not re-escaped, or `null` stops
/// decoding to `None`.
#[test]
fn seal_nullable_redacted_fields_are_re_escaped_on_read() {
    let build = |value: &str| {
        format!(
            r#"{{"source_id":"{}","kind":"external_reference","repo_id":null,"commit_oid":null,
                "run_id":null,"file_path":null,"artifact_id":null,"document_index":null,
                "line_start":null,"line_end":null,"resource_identifier":{value},
                "content_digest":null,"retrieved_at":"{TS}","approval_id":null,
                "redaction_version":"redact-1","stale":false}}"#,
            uid(3)
        )
    };
    let doc = build(&plant("\"@@\""));
    assert!(
        doc.contains(r"\u202e"),
        "the input really contains a bidi escape"
    );
    let model: SourceRef = serde_json::from_str(&doc).expect("decodes");
    assert_eq!(
        model.resource_identifier.as_ref().map(|t| t.as_str()),
        Some(HOSTILE_ESCAPED)
    );
    let out = serde_json::to_string(&model).expect("serialises");
    assert_serialised_is_safe(&out);
    assert_eq!(
        serde_json::from_str::<SourceRef>(&out).expect("round trip"),
        model
    );
    let none: SourceRef = serde_json::from_str(&build("null")).expect("null decodes");
    assert!(none.resource_identifier.is_none());
}

/// Fails if the values of the class R string map (`RendererSetting.parameters_redacted`) are not re-escaped
/// on read, or the keys stop being validated `Identifier`s.
#[test]
fn seal_map_redacted_values_are_re_escaped_on_read() {
    let doc = plant(
        r#"{"framework":"helm","renderer_id":"helm","input_paths":[],
            "parameters_redacted":{"release":"@@","namespace":"default"}}"#,
    );
    assert!(
        doc.contains(r"\u001b"),
        "the input really contains an ESC escape"
    );
    let model: RendererSetting = serde_json::from_str(&doc).expect("decodes");
    assert_eq!(model.parameters_redacted.len(), 2);
    let values: BTreeMap<&str, &str> = model
        .parameters_redacted
        .iter()
        .map(|(k, t)| (k.as_str(), t.as_str()))
        .collect();
    assert_eq!(values["release"], HOSTILE_ESCAPED);
    assert_eq!(values["namespace"], "default");
    let out = serde_json::to_string(&model).expect("serialises");
    assert_serialised_is_safe(&out);
    assert_eq!(
        serde_json::from_str::<RendererSetting>(&out).expect("round trip"),
        model
    );
    let bad_key = r#"{"framework":"helm","renderer_id":"helm","input_paths":[],
        "parameters_redacted":{"bad key\u001b":"x"}}"#;
    assert!(serde_json::from_str::<RendererSetting>(bad_key).is_err());
}

/// Fails if the set of container shapes among the class R rows changes. The seal tests above cover the plain,
/// nullable and map shapes; a class R array has a helper (`redacted_vec`) but no field uses it yet, so a new
/// one needs its own seal case here.
#[test]
fn the_redacted_shapes_present_are_the_ones_the_seal_tests_cover() {
    let walked = plain_properties(&schema());
    let shapes: BTreeSet<Shape> = classes::rows_of(Class::Redacted)
        .map(|r| walked[&(r.0.to_owned(), r.1.to_owned())])
        .collect();
    let covered = BTreeSet::from([
        Shape {
            nullable: false,
            kind: Kind::Scalar,
        },
        Shape {
            nullable: true,
            kind: Kind::Scalar,
        },
        Shape {
            nullable: false,
            kind: Kind::Map,
        },
    ]);
    assert_eq!(shapes, covered);
}

/// Fails if a class S field (`FileRecord.reason`) keeps unsafe code points from the JSON (the nullable shape
/// is the representative; the array shape uses the same `SanitisedText` decoder).
#[test]
fn seal_sanitised_fields_are_re_sanitised_on_read() {
    let doc = plant(&format!(
        r#"{{"file_id":"{}","path":"ci.yml","byte_size":0,"content_digest":null,
            "commit_oid":"0123456789abcdef0123456789abcdef01234567","detected_framework":null,
            "discovery_state":"excluded","reason":"@@"}}"#,
        uid(4)
    ));
    let model: FileRecord = serde_json::from_str(&doc).expect("decodes");
    assert_eq!(
        model.reason.as_ref().map(|t| t.as_str()),
        Some(HOSTILE_ESCAPED)
    );
    let out = serde_json::to_string(&model).expect("serialises");
    assert_serialised_is_safe(&out);
}

//! P00-T06 spike harness: saphyr-parser events -> spike composer -> JSON projection -> jsonschema.
//! Spike code only. Must run as PID 1 of a dedicated PID/network namespace (see evidence record).
//! Usage: t06-yaml <fixtures-dir> <repo-root> <scratch-dir (must not exist)> <results.json> <j16-cases.json>

mod compose;
mod project;

use compose::{compose, ComposeError, Node, Scalar, SPIKE_LIMITS};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() && c != '\n' { format!("\\u{{{:x}}}", c as u32) } else { c.to_string() }).collect::<String>().chars().take(300).collect()
}

fn pos_json(p: compose::Pos) -> Value {
    json!({"byte": p.byte, "line": p.line, "col": p.col})
}

/// Outcome category of the YAML pipeline for one input (all documents).
fn pipeline(bytes: &[u8]) -> (String, Value, Vec<Value>) {
    let t = Instant::now();
    let c = compose(bytes, &SPIKE_LIMITS);
    let mut projected = Vec::new();
    let mut detail = json!({"documents_composed": c.docs.len(), "alias_events": c.alias_events, "expanded_nodes": c.expanded_nodes});
    let category = match &c.error {
        Some(ComposeError::InvalidUtf8) => "input_rejected:invalid_utf8".to_string(),
        Some(ComposeError::Limit(which, d)) => { detail["limit"] = json!(d); format!("limit_exceeded:{which}") }
        Some(ComposeError::Syntax { info, at }) => { detail["error"] = json!(clean(info)); detail["at"] = pos_json(*at); "syntax_error".into() }
        Some(ComposeError::DuplicateKey { key, first, second }) => { detail["key"] = json!(clean(key)); detail["first"] = pos_json(*first); detail["second"] = pos_json(*second); "policy_rejected:duplicate_key".into() }
        Some(ComposeError::AliasCycle { anchor, at }) => { detail["anchor"] = json!(anchor); detail["at"] = pos_json(*at); "policy_rejected:alias_cycle".into() }
        Some(ComposeError::UndefinedAlias { anchor, at }) => { detail["anchor"] = json!(anchor); detail["at"] = pos_json(*at); "policy_rejected:undefined_alias".into() }
        Some(ComposeError::TagValueMismatch { tag, value, at }) => { detail["tag"] = json!(tag); detail["value"] = json!(clean(value)); detail["at"] = pos_json(*at); "policy_rejected:tag_value_mismatch".into() }
        None if c.docs.is_empty() => "no_documents".into(),
        None => "composed".into(),
    };
    for d in &c.docs {
        projected.push(match project::project(d) {
            Ok(v) => json!({"projected": v}),
            Err(u) => json!({"unsupported_for_validation": u.reason, "at": pos_json(u.at)}),
        });
    }
    detail["elapsed_ms"] = json!(t.elapsed().as_millis() as u64);
    (category, detail, projected)
}

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<String>>>);
impl jsonschema::Retrieve for Recorder {
    fn retrieve(&self, uri: &jsonschema::Uri<String>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.0.lock().unwrap().push(uri.as_str().to_string());
        Err("spike retriever: retrieval refused".into())
    }
}

/// Validate with Draft 2020-12; formats asserted only when requested.
fn check(schema: &Value, inst: &Value, formats: bool, rec: Option<&Recorder>) -> (String, Value) {
    let mut o = jsonschema::options().with_draft(jsonschema::Draft::Draft202012).should_validate_formats(formats);
    if let Some(r) = rec {
        o = o.with_retriever(r.clone());
    }
    match o.build(schema) {
        Err(e) => {
            let m = clean(&e.to_string());
            let kind = format!("{:?}", e.kind());
            let unavailable = kind.starts_with("Referencing") || m.contains("feature or a custom resolver is required") || m.contains("spike retriever");
            (if unavailable { "schema_unavailable" } else { "schema_build_error" }.into(), json!({"error": m, "kind": clean(&kind)}))
        }
        Ok(v) => {
            let errs: Vec<Value> = v.iter_errors(inst).map(|e| json!({"path": e.instance_path().to_string(), "error": clean(&e.to_string())})).collect();
            if errs.is_empty() { ("schema_valid".into(), json!({})) } else { ("schema_invalid".into(), json!({"errors": errs})) }
        }
    }
}

struct Run {
    rows: Vec<Value>,
    j16: Vec<Value>,
    pass: usize,
    fail: usize,
    observed_only: usize,
}

impl Run {
    fn row(&mut self, id: &str, layer: &str, expected: &str, observed: &str, detail: Value) {
        let status = if expected == "observation" { self.observed_only += 1; "observation" } else if expected == observed { self.pass += 1; "pass" } else { self.fail += 1; "fail" };
        println!("{status:11} {id:34} expected={expected} observed={observed}");
        self.rows.push(json!({"id": id, "layer": layer, "expected": expected, "observed": observed, "status": status, "detail": detail}));
    }
    fn j(&mut self, id: &str, schema: &Value, inst: &Value, formats: bool, expected: &str) -> String {
        let (verdict, detail) = check(schema, inst, formats, None);
        self.j16.push(json!({"id": id, "schema": schema, "instance": inst, "formats": formats, "rust": verdict}));
        self.row(id, "validator", expected, &verdict, detail);
        verdict
    }
}

fn yaml_case(r: &mut Run, id: &str, bytes: &[u8], expected: &str) -> (String, Value, Vec<Value>) {
    let (cat, detail, proj) = pipeline(bytes);
    let first_unsupported = proj.iter().find_map(|p| p.get("unsupported_for_validation").and_then(|v| v.as_str()).map(|s| s.split(':').next().unwrap_or("").to_string()));
    let observed = if cat == "composed" { match &first_unsupported { Some(u) => format!("unsupported_for_validation:{u}"), None => "projected".into() } } else { cat.clone() };
    let mut d = detail.clone();
    d["documents"] = json!(proj);
    r.row(id, "yaml", expected, &observed, d);
    (observed, detail, proj)
}

fn md_json_blocks(md: &str) -> Vec<Value> {
    md.split("```json\n").skip(1).filter_map(|b| b.split("\n```").next()).filter_map(|b| serde_json::from_str(b).ok()).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 { eprintln!("usage: t06-yaml <fixtures> <repo-root> <scratch> <results.json> <j16-cases.json>"); std::process::exit(2); }
    if std::process::id() != 1 { eprintln!("refusing to run: pid {} is not 1 (dedicated PID namespace required)", std::process::id()); std::process::exit(3); }
    let (fx, repo, scratch) = (PathBuf::from(&a[1]), PathBuf::from(&a[2]), PathBuf::from(&a[3]));
    if scratch.exists() { eprintln!("refusing: scratch exists"); std::process::exit(2); }
    fs::create_dir_all(&scratch).expect("scratch");
    let y = |n: &str| fs::read(fx.join("yaml").join(n)).expect(n);
    let sch = |n: &str| -> Value { serde_json::from_slice(&fs::read(fx.join("schema").join(n)).expect(n)).expect(n) };
    let mut r = Run { rows: vec![], j16: vec![], pass: 0, fail: 0, observed_only: 0 };

    // ---------------- YAML cases ----------------
    let (_, _, p) = yaml_case(&mut r, "Y01-valid", &y("y01_valid.yaml"), "projected");
    let exp: Value = serde_json::from_slice(&y("y01_valid.expected.json")).unwrap();
    r.row("Y01-values-exact", "projection", "equal", if p.first().and_then(|d| d.get("projected")) == Some(&exp) { "equal" } else { "different" }, json!({}));
    let (_, _, p) = yaml_case(&mut r, "Y02-nested", &y("y02_nested.yaml"), "projected");
    let exp: Value = serde_json::from_slice(&y("y02_nested.expected.json")).unwrap();
    r.row("Y02-values-exact", "projection", "equal", if p.first().and_then(|d| d.get("projected")) == Some(&exp) { "equal" } else { "different" }, json!({}));
    for (id, f, lines) in [("Y03a-unclosed-flow", "y03a_unclosed_flow.yaml", vec![2usize, 3]), ("Y03b-bad-indent", "y03b_bad_indent.yaml", vec![3]), ("Y03c-tab-indent", "y03c_tab_indent.yaml", vec![2])] {
        let (o, d, _) = yaml_case(&mut r, id, &y(f), "syntax_error");
        let line = d["at"]["line"].as_u64().unwrap_or(0) as usize;
        r.row(&format!("{id}-line"), "parser", "line_in_declared_set", if o == "syntax_error" && lines.contains(&line) { "line_in_declared_set" } else { "line_outside_declared_set" }, json!({"reported": d["at"], "declared_lines": lines}));
    }
    let s04 = sch("y04_schema.json");
    let (_, _, p) = yaml_case(&mut r, "Y04-multidoc", &y("y04_multidoc.yaml"), "projected");
    let verdicts: Vec<String> = p.iter().map(|d| d.get("projected").map(|v| check(&s04, v, true, None).0).unwrap_or_else(|| "not_projected".into())).collect();
    r.row("Y04-per-doc-verdicts", "pipeline", "schema_valid,schema_invalid,schema_valid", &verdicts.join(","), json!({"documents": p.len()}));
    let (_, d, _) = yaml_case(&mut r, "Y05-multidoc-syntax", &y("y05_multidoc_syntax.yaml"), "syntax_error");
    r.row("Y05-docs-before-error-kept", "composer", "1", &d["documents_composed"].to_string(), json!({"note": "documents after the error are not assessed"}));
    for (id, f) in [("Y06a-dup-top", "y06a_dup_top.yaml"), ("Y06b-dup-nested", "y06b_dup_nested.yaml"), ("Y06c-dup-quoted-vs-plain", "y06c_dup_quoted.yaml")] {
        yaml_case(&mut r, id, &y(f), "policy_rejected:duplicate_key");
    }
    yaml_case(&mut r, "Y07-int-vs-string-key", &y("y07_int_vs_str_key.yaml"), "unsupported_for_validation:non_string_key");
    // Y08: scalar resolution checked on composed nodes (before projection).
    let c = compose(&y("y08_scalars.yaml"), &SPIKE_LIMITS);
    let expect: Vec<(&str, Scalar)> = vec![
        ("yes_", Scalar::Str("yes".into())), ("no_", Scalar::Str("no".into())), ("on_", Scalar::Str("on".into())), ("off_", Scalar::Str("off".into())),
        ("True_", Scalar::Bool(true)), ("TRUE_", Scalar::Bool(true)), ("tilde", Scalar::Null), ("null_", Scalar::Null), ("empty", Scalar::Null),
        ("octal", Scalar::Int(15)), ("hex", Scalar::Int(31)), ("leading_zero", Scalar::Int(17)), ("underscore", Scalar::Str("1_000".into())),
        ("exp", Scalar::Float(1000.0)), ("inf", Scalar::Float(f64::INFINITY)), ("neginf", Scalar::Float(f64::NEG_INFINITY)), ("nan", Scalar::Float(f64::NAN)),
        ("quoted_true", Scalar::Str("true".into())), ("quoted_num", Scalar::Str("123".into())), ("date_like", Scalar::Str("2026-10-24".into())),
    ];
    let mut table = Vec::new();
    let mut all_ok = c.error.is_none();
    if let Some(Node::Map { entries, .. }) = c.docs.first().map(|d| &**d) {
        for (k, want) in &expect {
            let got = entries.iter().find(|(kk, _)| matches!(&**kk, Node::Scalar { v: Scalar::Str(s), .. } if s == k)).map(|(_, v)| match &**v { Node::Scalar { v, .. } => v.clone(), _ => Scalar::Str("<collection>".into()) });
            let ok = match (&got, want) { (Some(Scalar::Float(g)), Scalar::Float(w)) if w.is_nan() => g.is_nan(), (Some(g), w) => g == w, (None, _) => false };
            all_ok &= ok;
            table.push(json!({"key": k, "expected": format!("{want:?}"), "got": format!("{got:?}"), "ok": ok}));
        }
    } else { all_ok = false; }
    r.row("Y08-core-schema-scalars", "composer", "all_match", if all_ok { "all_match" } else { "mismatch" }, json!({"yaml": "1.2 core schema", "table": table}));
    yaml_case(&mut r, "Y09a-inf", b"a: .inf\n", "unsupported_for_validation:non_finite_float(inf)");
    yaml_case(&mut r, "Y09b-nan", b"a: .nan\n", "unsupported_for_validation:non_finite_float(nan)");
    yaml_case(&mut r, "Y09c-int-above-u64", b"a: 18446744073709551616\n", "unsupported_for_validation:integer_outside_lossless_range(i64/u64)");
    yaml_case(&mut r, "Y09d-int-above-i128", b"a: 340282366920938463463374607431768211456\n", "unsupported_for_validation:integer_outside_lossless_range(i64/u64)");
    yaml_case(&mut r, "Y09e-float-overflow", b"a: 1e400\n", "unsupported_for_validation:float_literal_overflows_f64");
    yaml_case(&mut r, "Y09f-u64-max-ok", b"a: 18446744073709551615\n", "projected");
    let (_, _, p) = yaml_case(&mut r, "Y10-alias-ok", &y("y10_alias_ok.yaml"), "projected");
    let pv = p.first().and_then(|d| d.get("projected")).cloned().unwrap_or(Value::Null);
    r.row("Y10-alias-copies-equal", "projection", "equal", if pv["first"] == pv["base"] && pv["third"] == pv["base"] && pv["base"].is_object() { "equal" } else { "different" }, json!({}));
    // Generated bombs / limits.
    let mut bomb = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
    for i in 1..9 { bomb += &format!("a{i}: &a{i} [{}]\n", vec![format!("*a{}", i - 1); 10].join(", ")); }
    yaml_case(&mut r, "Y11-alias-bomb", bomb.as_bytes(), "limit_exceeded:expanded_nodes");
    let (o, _, _) = yaml_case(&mut r, "Y12a-seq-cycle", &y("y12a_cycle_seq.yaml"), "observation");
    r.row("Y12a-not-pass", "composer", "rejected", if o == "policy_rejected:alias_cycle" || o == "syntax_error" { "rejected" } else { "accepted" }, json!({"observed": o}));
    let (o, _, _) = yaml_case(&mut r, "Y12b-map-cycle", &y("y12b_cycle_map.yaml"), "observation");
    r.row("Y12b-not-pass", "composer", "rejected", if o == "policy_rejected:alias_cycle" || o == "syntax_error" { "rejected" } else { "accepted" }, json!({"observed": o}));
    let (o, _, _) = yaml_case(&mut r, "Y13-undefined-alias", &y("y13_undefined_alias.yaml"), "observation");
    r.row("Y13-not-pass", "parser/composer", "rejected", if o == "policy_rejected:undefined_alias" || o == "syntax_error" { "rejected" } else { "accepted" }, json!({"observed": o}));
    yaml_case(&mut r, "Y14a-depth-65", format!("a: {}{}\n", "[".repeat(65), "]".repeat(65)).as_bytes(), "limit_exceeded:depth");
    yaml_case(&mut r, "Y14b-depth-10000", format!("a: {}{}\n", "[".repeat(10000), "]".repeat(10000)).as_bytes(), "limit_exceeded:depth");
    let mut big = String::from("a: \""); big += &"x".repeat(1 << 20); big += "\"\n";
    yaml_case(&mut r, "Y15a-input-bytes", big.as_bytes(), "limit_exceeded:input_bytes");
    yaml_case(&mut r, "Y15b-101-documents", "---\na: 1\n".repeat(101).as_bytes(), "limit_exceeded:documents");
    yaml_case(&mut r, "Y15c-scalar-bytes", format!("a: {}\n", "y".repeat((64 << 10) + 1)).as_bytes(), "limit_exceeded:scalar_bytes");
    let per_doc = format!("---\nb: &b 1\nl: [{}]\n", vec!["*b"; 25].join(", "));
    yaml_case(&mut r, "Y15d-alias-budget-across-docs", per_doc.repeat(50).as_bytes(), "limit_exceeded:alias_events");
    // Y16: positions with multibyte text on the target line; plus BOM + CRLF variant.
    let u = y("y16_unicode.yaml");
    for (id, bytes) in [("Y16a-unicode-lf", u.clone()), ("Y16b-unicode-bom-crlf", { let mut v = vec![0xEF, 0xBB, 0xBF]; v.extend(String::from_utf8(u.clone()).unwrap().replace('\n', "\r\n").into_bytes()); v })] {
        let text = String::from_utf8(bytes.clone()).unwrap();
        let byte = text.find("TARGET_VALUE").unwrap();
        let line_start = text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = text[..byte].matches('\n').count() + 1;
        let char_col1 = text[line_start..byte].chars().count() + 1;
        let byte_col1 = byte - line_start + 1;
        let c = compose(&bytes, &SPIKE_LIMITS);
        let found = c.docs.first().and_then(|d| match &**d { Node::Map { entries, .. } => entries.iter().find(|(k, _)| matches!(&**k, Node::Scalar { v: Scalar::Str(s), .. } if s.starts_with("ключ"))).map(|(_, v)| v.at()), _ => None });
        let first_key = c.docs.first().and_then(|d| match &**d { Node::Map { entries, .. } => entries.first().and_then(|(k, _)| match &**k { Node::Scalar { v: Scalar::Str(s), .. } => Some(s.clone()), _ => None }), _ => None });
        let obs = match found {
            Some(p) => {
                let unit = if p.col == char_col1 { "char_1based" } else if p.col + 1 == char_col1 { "char_0based" } else if p.col == byte_col1 { "byte_1based" } else if p.col + 1 == byte_col1 { "byte_0based" } else { "unknown" };
                json!({"reported": pos_json(p), "expected_byte": byte, "expected_line": line, "char_col_1based": char_col1, "byte_col_1based": byte_col1, "col_unit": unit, "first_key": first_key})
            }
            None => json!({"error": "target not found"}),
        };
        let ok = obs["reported"]["byte"] == json!(byte) && obs["reported"]["line"] == json!(line) && obs["col_unit"] != json!("unknown") && first_key.as_deref() == Some("名前");
        r.row(id, "parser", "byte_and_line_exact+col_unit_identified+bom_not_in_key", if ok { "byte_and_line_exact+col_unit_identified+bom_not_in_key" } else { "mismatch" }, obs);
    }
    yaml_case(&mut r, "Y17a-invalid-utf8", b"a: \xff\xfe\n", "input_rejected:invalid_utf8");
    let (o, d, p) = yaml_case(&mut r, "Y17b-nul-byte", b"a: b\x00c\n", "observation");
    r.row("Y17b-not-silently-projected", "parser/composer", "rejected", if o == "syntax_error" || o.starts_with("input_rejected") { "rejected" } else { "accepted" }, json!({"observed": o, "detail": d, "documents": p}));
    let (_, _, p) = yaml_case(&mut r, "Y18a-core-tags", &y("y18a_core_tags.yaml"), "projected");
    r.row("Y18a-core-tag-values", "composer", "s=\"123\",i=7", &format!("s={},i={}", p.first().map(|d| d["projected"]["s"].to_string()).unwrap_or_default(), p.first().map(|d| d["projected"]["i"].to_string()).unwrap_or_default()), json!({}));
    for (id, f, tag) in [("Y18b-custom-tag", "y18b_custom_tag.yaml", "!Ref"), ("Y18c-python-tag", "y18c_python_tag.yaml", "!!python/object:os.system"), ("Y18d-binary-tag", "y18d_binary_tag.yaml", "!!binary")] {
        let (_, _, p) = yaml_case(&mut r, id, &y(f), "unsupported_for_validation:tag");
        let reason = p.first().and_then(|d| d["unsupported_for_validation"].as_str()).unwrap_or("").to_string();
        r.row(&format!("{id}-name"), "composer", &format!("tag:{tag}"), &reason, json!({"note": "tags are recorded, never instantiated"}));
    }
    for (id, f, exp) in [("Y19a-int-key", "y19a_int_key.yaml", "non_string_key"), ("Y19b-bool-key", "y19b_bool_key.yaml", "non_string_key"), ("Y19c-null-key", "y19c_null_key.yaml", "non_string_key"), ("Y19d-complex-key", "y19d_complex_key.yaml", "complex_key(collection used as key)")] {
        yaml_case(&mut r, id, &y(f), &format!("unsupported_for_validation:{exp}"));
    }
    yaml_case(&mut r, "Y20-merge-key", &y("y20_merge_key.yaml"), "unsupported_for_validation:merge_key(<<)");
    let s_obj = json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"});
    for (id, bytes, exp) in [("Y21a-empty", Vec::new(), "no_documents"), ("Y21b-comments-only", b"# only a comment\n".to_vec(), "no_documents")] {
        yaml_case(&mut r, id, &bytes, exp);
    }
    let (_, _, p) = yaml_case(&mut r, "Y21c-explicit-empty-doc", b"---\n", "projected");
    let v = p.first().map(|d| check(&s_obj, &d["projected"], true, None).0).unwrap_or_else(|| "no_document".into());
    r.row("Y21c-null-doc-vs-object-schema", "pipeline", "schema_invalid", &v, json!({}));

    // ---------------- JSON Schema cases ----------------
    let j01 = sch("j01_object.json");
    r.j("J01-valid", &j01, &json!({"name": "a", "replicas": 1}), true, "schema_valid");
    r.j("J01-missing-required", &j01, &json!({"name": "a"}), true, "schema_invalid");
    r.j("J01-additional-property", &j01, &json!({"name": "a", "replicas": 1, "x": 1}), true, "schema_invalid");
    r.j("J01-wrong-type", &j01, &json!({"name": "a", "replicas": "1"}), true, "schema_invalid");
    let j02 = sch("j02_enum_const.json");
    r.j("J02-valid", &j02, &json!({"mode": "a", "v": "1.3"}), true, "schema_valid");
    r.j("J02-enum", &j02, &json!({"mode": "c", "v": "1.3"}), true, "schema_invalid");
    r.j("J02-const", &j02, &json!({"mode": "a", "v": "1.2"}), true, "schema_invalid");
    let j03 = sch("j03_arrays.json");
    r.j("J03-valid", &j03, &json!({"p": ["a", 1, true], "u": [1, 2]}), true, "schema_valid");
    r.j("J03-prefixItems", &j03, &json!({"p": [1, 1]}), true, "schema_invalid");
    r.j("J03-items", &j03, &json!({"p": ["a", 1, "x"]}), true, "schema_invalid");
    r.j("J03-minItems", &j03, &json!({"p": ["a"]}), true, "schema_invalid");
    r.j("J03-uniqueItems", &j03, &json!({"u": [1, 1]}), true, "schema_invalid");
    let j04 = sch("j04_datetime.json");
    r.j("J04-valid-on", &j04, &json!("2026-10-24T05:25:00Z"), true, "schema_valid");
    r.j("J04-bad-date-on", &j04, &json!("2026-13-40T00:00:00Z"), true, "schema_invalid");
    r.j("J04-no-T-on", &j04, &json!("2026-10-24 05:25"), true, "schema_invalid");
    r.j("J04-bad-date-off", &j04, &json!("2026-13-40T00:00:00Z"), false, "schema_valid");
    r.j("J04-no-T-off", &j04, &json!("2026-10-24 05:25"), false, "schema_valid");
    let j05 = sch("j05_uuid.json");
    r.j("J05-valid-on", &j05, &json!("526a9739-68cb-5ef8-8c3a-0fda8a207475"), true, "schema_valid");
    r.j("J05-malformed-on", &j05, &json!("526a9739-68cb-5ef8-8c3a"), true, "schema_invalid");
    r.j("J05-malformed-off", &j05, &json!("526a9739-68cb-5ef8-8c3a"), false, "schema_valid");
    let j06 = sch("j06_pattern.json");
    r.j("J06-40-hex", &j06, &json!("a".repeat(40)), true, "schema_valid");
    r.j("J06-64-hex", &j06, &json!("0".repeat(64)), true, "schema_valid");
    r.j("J06-uppercase", &j06, &json!("A".repeat(40)), true, "schema_invalid");
    r.j("J06-39-hex", &j06, &json!("a".repeat(39)), true, "schema_invalid");
    let j07 = sch("j07_local_ref.json");
    r.j("J07-valid", &j07, &json!([1, 2]), true, "schema_valid");
    r.j("J07-invalid", &j07, &json!([1, -1]), true, "schema_invalid");
    // J08/J09: external references. Default retriever (features off) and an instrumented retriever.
    let canary_path = scratch.join("canary-schema.json");
    fs::write(&canary_path, br#"{"type": "string", "description": "PWT_T06_FILE_CANARY"}"#).unwrap();
    let refs = [("J08-http-ref", json!({"$ref": "https://example.invalid/s.json"}), "`resolve-http` feature or a custom resolver is required"),
                ("J09-file-ref", json!({"$ref": format!("file://{}", canary_path.display())}), "`resolve-file` feature or a custom resolver is required")];
    for (id, schema, disabled_msg) in &refs {
        let (v, d) = check(schema, &json!("x"), true, None);
        let path = if d["error"].as_str().unwrap_or("").contains(disabled_msg) { "disabled_path" } else { "other_error" };
        r.row(&format!("{id}-default"), "validator", "schema_unavailable+disabled_path", &format!("{v}+{path}"), d.clone());
        let rec = Recorder::default();
        let (v2, d2) = check(schema, &json!("x"), true, Some(&rec));
        let calls = rec.0.lock().unwrap().clone();
        r.row(&format!("{id}-instrumented"), "validator", "schema_unavailable+retriever_called", &format!("{v2}+{}", if calls.is_empty() { "retriever_not_called" } else { "retriever_called" }), json!({"retriever_calls": calls, "detail": d2}));
    }
    // J10: other drafts under forced Draft 2020-12 (observation only).
    for (id, s) in [("J10-draft07-array-items", json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "array", "items": [{"type": "string"}]})),
                    ("J10-draft201909", json!({"$schema": "https://json-schema.org/draft/2019-09/schema", "type": "string"}))] {
        let (v, d) = check(&s, &json!(["a", 1]), true, None);
        r.row(id, "validator", "observation", &v, d);
    }
    // J11–J13: current contract examples from the specification.
    let md = fs::read_to_string(repo.join("05_Database_Data_Schema.md")).unwrap();
    let blocks = md_json_blocks(&md);
    let store: Value = serde_json::from_slice(&fs::read(repo.join("schemas/workbench-store-v1.3.schema.json")).unwrap()).unwrap();
    let wctl: Value = serde_json::from_slice(&fs::read(repo.join("schemas/wrapper-control-v1.1.schema.json")).unwrap()).unwrap();
    let mut wc_example = Value::Null;
    for b in blocks.iter().skip(1) {
        if let Some(st) = b.get("store_type").and_then(|v| v.as_str()) {
            r.j(&format!("J11-store-1.3-{st}"), &store, b, true, "schema_valid");
            let mut old = b.clone(); old["schema_version"] = json!("1.2");
            r.j(&format!("J12-store-{st}-labelled-1.2"), &store, &old, true, "schema_invalid");
        } else if b.get("protocol_version").is_some() { wc_example = b.clone(); }
    }
    r.j("J13-wrapper-control-1.1", &wctl, &wc_example, true, "schema_valid");
    let mut old = wc_example.clone(); old["protocol_version"] = json!("1.0");
    r.j("J13-wrapper-control-labelled-1.0", &wctl, &old, true, "schema_invalid");
    let mut missing = wc_example.clone(); missing.as_object_mut().map(|m| m.remove("output_streams"));
    r.j("J13-wrapper-control-missing-output_streams", &wctl, &missing, true, "schema_invalid");
    // J14: YAML -> projection -> validation equals JSON-input verdicts.
    let (_, _, p) = pipeline(&y("y04_multidoc.yaml"));
    let via_yaml: Vec<String> = p.iter().map(|d| check(&s04, &d["projected"], true, None).0).collect();
    let via_json: Vec<String> = [json!({"name": "a", "replicas": 1}), json!({"name": "b", "replicas": "not-a-number"}), json!({"name": "c", "replicas": 2})].iter().map(|v| check(&s04, v, true, None).0).collect();
    r.row("J14-yaml-equals-json-verdicts", "pipeline", "equal", if via_yaml == via_json { "equal" } else { "different" }, json!({"yaml": via_yaml, "json": via_json}));
    // J15: valid YAML + unavailable schema is never a pass.
    let (_, _, p) = pipeline(&y("y01_valid.yaml"));
    let (v, _) = check(&refs[0].1, &p[0]["projected"], true, None);
    r.row("J15-valid-yaml-unavailable-schema", "pipeline", "schema_unavailable", &v, json!({}));

    // ---------------- outputs, cleanup, process check ----------------
    let canary_leak = r.rows.iter().any(|row| row.to_string().contains("PWT_T06_FILE_CANARY"));
    r.row("J09-canary-content-not-in-output", "validator", "absent", if canary_leak { "present" } else { "absent" }, json!({}));
    fs::write(&a[5], serde_json::to_string_pretty(&r.j16).unwrap()).unwrap();
    fs::remove_dir_all(&scratch).ok();
    let others: Vec<String> = fs::read_dir("/proc").unwrap().flatten().filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<u32>().ok())).filter(|p| *p != 1).map(|p| fs::read_to_string(format!("/proc/{p}/comm")).unwrap_or_default().trim().to_string() + &format!("({p})")).collect();
    let cleanup = format!("{}+{}", if scratch.exists() { "scratch_present" } else { "scratch_removed" }, if others.is_empty() { "no_other_processes" } else { "other_processes" });
    r.row("CLEANUP", "harness", "scratch_removed+no_other_processes", &cleanup, json!({"other_processes": others}));
    let doc = json!({"limits": {"input_bytes": SPIKE_LIMITS.input_bytes, "documents_per_input": SPIKE_LIMITS.documents, "depth": SPIKE_LIMITS.depth, "alias_events_per_input": SPIKE_LIMITS.alias_events, "expanded_nodes_per_input": SPIKE_LIMITS.expanded_nodes, "scalar_bytes": SPIKE_LIMITS.scalar_bytes},
        "pass": r.pass, "fail": r.fail, "observations": r.observed_only, "cases": r.rows});
    fs::write(&a[4], serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    println!("SUMMARY pass={} fail={} observations={}", r.pass, r.fail, r.observed_only);
    std::process::exit(if r.fail == 0 { 0 } else { 1 });
}

//! JSON projection (P00-T06 spike): decides whether a composed YAML node can be represented
//! losslessly as `serde_json::Value`. Anything else is `unsupported_for_validation` with a precise
//! reason; nothing is coerced. (`serde_json` is built without `arbitrary_precision`.)

use crate::compose::{Node, Pos, Scalar};
use serde_json::{Map, Number, Value};

pub struct Unsupported {
    pub reason: String,
    pub at: Pos,
}

pub fn project(n: &Node) -> Result<Value, Unsupported> {
    let un = |reason: String, at: Pos| Err(Unsupported { reason, at });
    match n {
        Node::Scalar { unsupported_tag: Some(t), at, .. } | Node::Seq { unsupported_tag: Some(t), at, .. } | Node::Map { unsupported_tag: Some(t), at, .. } => {
            un(format!("tag:{t}"), *at)
        }
        Node::Scalar { v, at, .. } => match v {
            Scalar::Null => Ok(Value::Null),
            Scalar::Bool(b) => Ok(Value::Bool(*b)),
            Scalar::Str(s) => Ok(Value::String(s.clone())),
            Scalar::Int(i) => {
                if let Ok(v) = i64::try_from(*i) {
                    Ok(Value::Number(v.into()))
                } else if let Ok(v) = u64::try_from(*i) {
                    Ok(Value::Number(v.into()))
                } else {
                    un(format!("integer_outside_lossless_range(i64/u64):{i}"), *at)
                }
            }
            Scalar::BigInt(s) => un(format!("integer_outside_lossless_range(i64/u64):{s}"), *at),
            Scalar::FloatOverflow(s) => un(format!("float_literal_overflows_f64:{s}"), *at),
            Scalar::Float(f) => match Number::from_f64(*f) {
                Some(num) => Ok(Value::Number(num)),
                None if f.is_nan() => un("non_finite_float(nan):not_a_json_number".into(), *at),
                None => un("non_finite_float(inf):not_a_json_number".into(), *at),
            },
        },
        Node::Seq { items, .. } => items.iter().map(|i| project(i)).collect::<Result<Vec<_>, _>>().map(Value::Array),
        Node::Map { entries, .. } => {
            let mut m = Map::new();
            for (k, v) in entries {
                let key = match &**k {
                    Node::Scalar { v: Scalar::Str(s), unsupported_tag: None, at } => {
                        if s == "<<" {
                            return un("merge_key(<<):merge semantics not applied".into(), *at);
                        }
                        s.clone()
                    }
                    Node::Scalar { at, v, .. } => return un(format!("non_string_key:{v:?}"), *at),
                    other => return un("complex_key(collection used as key)".into(), other.at()),
                };
                if m.contains_key(&key) {
                    return un(format!("key_collision_after_projection:{key}"), k.at());
                }
                m.insert(key, project(v)?);
            }
            Ok(Value::Object(m))
        }
    }
}

//! Spike-owned composer over saphyr-parser events (P00-T06). Not a general YAML library.
//!
//! Responsibilities: document separation, limits (enforced per input so multiple documents cannot
//! bypass them), anchors/aliases with cycle detection and an expansion budget checked *before*
//! materialisation, duplicate-key detection on the lossless entry list, YAML 1.2 core-schema scalar
//! resolution, tag policy. Projection to JSON lives in `project.rs`.

use saphyr_parser::{Event, Parser, ScalarStyle, Span};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Limits {
    pub input_bytes: usize,
    pub documents: usize,
    pub depth: usize,
    pub alias_events: usize,
    pub expanded_nodes: u64,
    pub scalar_bytes: usize,
}

pub const SPIKE_LIMITS: Limits = Limits {
    input_bytes: 1 << 20,
    documents: 100,
    depth: 64,
    alias_events: 1000,
    expanded_nodes: 100_000,
    scalar_bytes: 64 << 10,
};

#[derive(Clone, Copy, Debug)]
pub struct Pos {
    pub byte: usize,
    pub line: usize,
    pub col: usize,
}

fn pos(s: &Span) -> Pos {
    Pos { byte: s.start.index(), line: s.start.line(), col: s.start.col() }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i128),
    /// Integer literal outside i128 (kept as text; never coerced).
    BigInt(String),
    Float(f64),
    /// Finite float literal that overflows f64 (kept as text; never coerced to infinity).
    FloatOverflow(String),
    Str(String),
}

#[derive(Debug)]
pub enum Node {
    Scalar { v: Scalar, unsupported_tag: Option<String>, at: Pos },
    Seq { items: Vec<Rc<Node>>, unsupported_tag: Option<String>, at: Pos },
    Map { entries: Vec<(Rc<Node>, Rc<Node>)>, unsupported_tag: Option<String>, at: Pos },
}

impl Node {
    pub fn at(&self) -> Pos {
        match self {
            Node::Scalar { at, .. } | Node::Seq { at, .. } | Node::Map { at, .. } => *at,
        }
    }
}

#[derive(Debug)]
pub enum ComposeError {
    InvalidUtf8,
    Limit(&'static str, String),
    Syntax { info: String, at: Pos },
    DuplicateKey { key: String, first: Pos, second: Pos },
    AliasCycle { anchor: usize, at: Pos },
    UndefinedAlias { anchor: usize, at: Pos },
    TagValueMismatch { tag: String, value: String, at: Pos },
}

pub struct Composed {
    /// Documents successfully composed, in order (index 0 = document 1).
    pub docs: Vec<Rc<Node>>,
    /// Error that stopped composition (later documents not assessed).
    pub error: Option<ComposeError>,
    pub alias_events: usize,
    pub expanded_nodes: u64,
}

enum Frame {
    Seq { items: Vec<Rc<Node>>, anchor: usize, tag: Option<String>, at: Pos },
    Map { entries: Vec<(Rc<Node>, Rc<Node>)>, pending_key: Option<Rc<Node>>, anchor: usize, tag: Option<String>, at: Pos },
}

fn tag_name(t: &saphyr_parser::Tag) -> String {
    format!("{}{}", t.handle, t.suffix)
}

/// Core-schema tag suffix if the tag is a YAML core tag (`!!x` or `tag:yaml.org,2002:x`).
fn core_tag(t: &saphyr_parser::Tag) -> Option<&str> {
    if t.handle == "!!" || t.handle == "tag:yaml.org,2002:" {
        Some(t.suffix.as_str())
    } else {
        None
    }
}

fn all(s: &str, f: impl Fn(char) -> bool) -> bool {
    !s.is_empty() && s.chars().all(f)
}

fn parse_core_int(s: &str) -> Option<Result<i128, ()>> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (radix, digits) = if let Some(o) = s.strip_prefix("0o") {
        if !all(o, |c| ('0'..='7').contains(&c)) { return None; }
        (8, o)
    } else if let Some(x) = s.strip_prefix("0x") {
        if !all(x, |c| c.is_ascii_hexdigit()) { return None; }
        (16, x)
    } else {
        if !all(body, |c| c.is_ascii_digit()) { return None; }
        (10, body)
    };
    let neg = neg && radix == 10;
    Some(i128::from_str_radix(digits, radix).map(|v| if neg { -v } else { v }).map_err(|_| ()))
}

fn parse_core_float(s: &str) -> Option<f64> {
    match s {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => return Some(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => return Some(f64::NEG_INFINITY),
        ".nan" | ".NaN" | ".NAN" => return Some(f64::NAN),
        _ => {}
    }
    // [-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?
    let b = s.strip_prefix(['-', '+']).unwrap_or(s);
    let (mant, exp) = match b.find(['e', 'E']) {
        Some(i) => (&b[..i], Some(&b[i + 1..])),
        None => (b, None),
    };
    let mant_ok = match mant.split_once('.') {
        Some((a, f)) => (a.is_empty() && all(f, |c| c.is_ascii_digit())) || (all(a, |c| c.is_ascii_digit()) && f.chars().all(|c| c.is_ascii_digit())),
        None => all(mant, |c| c.is_ascii_digit()),
    };
    let exp_ok = exp.map_or(true, |e| all(e.strip_prefix(['-', '+']).unwrap_or(e), |c| c.is_ascii_digit()));
    if mant_ok && exp_ok { s.parse::<f64>().ok() } else { None }
}

/// YAML 1.2 core schema resolution for an untagged plain scalar.
pub fn resolve_plain(s: &str) -> Scalar {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Scalar::Null,
        "true" | "True" | "TRUE" => return Scalar::Bool(true),
        "false" | "False" | "FALSE" => return Scalar::Bool(false),
        _ => {}
    }
    match parse_core_int(s) {
        Some(Ok(v)) => return Scalar::Int(v),
        Some(Err(())) => return Scalar::BigInt(s.to_string()),
        None => {}
    }
    if let Some(f) = parse_core_float(s) {
        if f.is_infinite() && !s.to_ascii_lowercase().contains("inf") {
            return Scalar::FloatOverflow(s.to_string());
        }
        return Scalar::Float(f);
    }
    Scalar::Str(s.to_string())
}

/// Canonical identity of a key for duplicate detection (resolved type + value; structural for collections).
fn key_identity(n: &Node) -> String {
    match n {
        Node::Scalar { v, .. } => match v {
            Scalar::Null => "null".into(),
            Scalar::Bool(b) => format!("bool:{b}"),
            Scalar::Int(i) => format!("int:{i}"),
            Scalar::BigInt(s) => format!("int:{s}"),
            Scalar::Float(f) => format!("float:{f:?}"),
            Scalar::FloatOverflow(s) => format!("float:{s}"),
            Scalar::Str(s) => format!("str:{s}"),
        },
        Node::Seq { items, .. } => format!("seq[{}]", items.iter().map(|i| key_identity(i)).collect::<Vec<_>>().join(",")),
        Node::Map { entries, .. } => format!(
            "map{{{}}}",
            entries.iter().map(|(k, v)| format!("{}={}", key_identity(k), key_identity(v))).collect::<Vec<_>>().join(",")
        ),
    }
}

pub fn compose(bytes: &[u8], l: &Limits) -> Composed {
    let mut out = Composed { docs: Vec::new(), error: None, alias_events: 0, expanded_nodes: 0 };
    macro_rules! fail {
        ($e:expr) => {{
            out.error = Some($e);
            return out;
        }};
    }
    if bytes.len() > l.input_bytes {
        fail!(ComposeError::Limit("input_bytes", format!("{} > {}", bytes.len(), l.input_bytes)));
    }
    let Ok(text) = std::str::from_utf8(bytes) else { fail!(ComposeError::InvalidUtf8) };
    let mut anchors: HashMap<usize, (Rc<Node>, u64)> = HashMap::new();
    let mut open_anchors: HashSet<usize> = HashSet::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut sizes: Vec<u64> = Vec::new(); // expanded node count accumulated per open frame
    let mut doc_root: Option<Rc<Node>> = None;
    for ev in Parser::new_from_str(text) {
        let (event, span) = match ev {
            Ok(x) => x,
            Err(e) => {
                let m = e.marker();
                fail!(ComposeError::Syntax { info: e.info().to_string(), at: Pos { byte: m.index(), line: m.line(), col: m.col() } })
            }
        };
        let at = pos(&span);
        let is_seq = matches!(event, Event::SequenceStart(..));
        // Each completed node, with its expanded size, is attached to the parent below.
        let completed: Option<(Rc<Node>, u64, usize)> = match event {
            Event::StreamStart | Event::StreamEnd | Event::Nothing => None,
            Event::DocumentStart(_) => {
                if out.docs.len() >= l.documents {
                    fail!(ComposeError::Limit("documents", format!("more than {}", l.documents)));
                }
                None
            }
            Event::DocumentEnd => {
                out.docs.push(doc_root.take().unwrap_or_else(|| Rc::new(Node::Scalar { v: Scalar::Null, unsupported_tag: None, at })));
                None
            }
            Event::Alias(id) => {
                out.alias_events += 1;
                if out.alias_events > l.alias_events {
                    fail!(ComposeError::Limit("alias_events", format!("more than {}", l.alias_events)));
                }
                if open_anchors.contains(&id) {
                    fail!(ComposeError::AliasCycle { anchor: id, at });
                }
                let Some((node, size)) = anchors.get(&id).cloned() else { fail!(ComposeError::UndefinedAlias { anchor: id, at }) };
                Some((node, size, 0))
            }
            Event::Scalar(value, style, anchor, tag) => {
                if value.len() > l.scalar_bytes {
                    fail!(ComposeError::Limit("scalar_bytes", format!("{} > {}", value.len(), l.scalar_bytes)));
                }
                let (v, unsupported_tag) = match tag.as_deref() {
                    None if style == ScalarStyle::Plain => (resolve_plain(&value), None),
                    None => (Scalar::Str(value.to_string()), None),
                    Some(t) if t.handle == "!" && t.suffix.is_empty() => (Scalar::Str(value.to_string()), None),
                    Some(t) => match core_tag(t) {
                        Some("str") => (Scalar::Str(value.to_string()), None),
                        Some(k @ ("int" | "float" | "bool" | "null")) => {
                            let r = resolve_plain(&value);
                            let ok = matches!((k, &r), ("int", Scalar::Int(_) | Scalar::BigInt(_)) | ("float", Scalar::Float(_) | Scalar::FloatOverflow(_)) | ("float", Scalar::Int(_)) | ("bool", Scalar::Bool(_)) | ("null", Scalar::Null));
                            if !ok {
                                fail!(ComposeError::TagValueMismatch { tag: tag_name(t), value: value.to_string(), at });
                            }
                            let r = match (k, r) { ("float", Scalar::Int(i)) => Scalar::Float(i as f64), (_, r) => r };
                            (r, None)
                        }
                        _ => (Scalar::Str(value.to_string()), Some(tag_name(t))),
                    },
                };
                Some((Rc::new(Node::Scalar { v, unsupported_tag, at }), 1, anchor))
            }
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                if stack.len() + 1 > l.depth {
                    fail!(ComposeError::Limit("depth", format!("> {}", l.depth)));
                }
                if anchor != 0 {
                    open_anchors.insert(anchor);
                }
                let tag = tag.as_deref().and_then(|t| match core_tag(t) {
                    Some("seq" | "map") => None,
                    _ => Some(tag_name(t)),
                });
                stack.push(if is_seq {
                    Frame::Seq { items: Vec::new(), anchor, tag, at }
                } else {
                    Frame::Map { entries: Vec::new(), pending_key: None, anchor, tag, at }
                });
                sizes.push(1);
                None
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let size = sizes.pop().unwrap_or(1);
                match stack.pop() {
                    Some(Frame::Seq { items, anchor, tag, at }) => Some((Rc::new(Node::Seq { items, unsupported_tag: tag, at }), size, anchor)),
                    Some(Frame::Map { entries, anchor, tag, at, .. }) => {
                        // Duplicate keys are detected on the lossless entry list, before any map exists.
                        let mut seen: HashMap<String, Pos> = HashMap::new();
                        for (k, _) in &entries {
                            let id = key_identity(k);
                            if let Some(first) = seen.get(&id) {
                                fail!(ComposeError::DuplicateKey { key: id, first: *first, second: k.at() });
                            }
                            seen.insert(id, k.at());
                        }
                        Some((Rc::new(Node::Map { entries, unsupported_tag: tag, at }), size, anchor))
                    }
                    None => None,
                }
            }
        };
        if let Some((node, size, anchor)) = completed {
            if anchor != 0 {
                open_anchors.remove(&anchor);
                anchors.insert(anchor, (node.clone(), size));
            }
            // Expansion budget checked before the aliased subtree is materialised anywhere.
            out.expanded_nodes = out.expanded_nodes.saturating_add(if sizes.is_empty() { size } else { 0 });
            if let Some(parent) = sizes.last_mut() {
                *parent = parent.saturating_add(size);
                let total = out.expanded_nodes.saturating_add(sizes.iter().fold(0u64, |a, b| a.saturating_add(*b)));
                if total > l.expanded_nodes {
                    fail!(ComposeError::Limit("expanded_nodes", format!("> {}", l.expanded_nodes)));
                }
            } else if out.expanded_nodes > l.expanded_nodes {
                fail!(ComposeError::Limit("expanded_nodes", format!("> {}", l.expanded_nodes)));
            }
            match stack.last_mut() {
                Some(Frame::Seq { items, .. }) => items.push(node),
                Some(Frame::Map { entries, pending_key, .. }) => match pending_key.take() {
                    None => *pending_key = Some(node),
                    Some(k) => entries.push((k, node)),
                },
                None => doc_root = Some(node),
            }
        }
    }
    out
}

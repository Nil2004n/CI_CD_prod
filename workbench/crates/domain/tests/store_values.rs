//! P02-T05a validated store-value newtypes (`planning/P02-T01-request.md` §5.2). All inputs are synthetic.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use domain::store::values::{
    AppRelPath, CanonicalRemote, CredentialRef, Destination, Digest, EventCode, FrameworkId,
    GitOid, GitRefName, IanaTimezone, Identifier, LocalPath, LocalTime, ModelId, PathPattern,
    PolicyId, ProviderId, RedactionVersion, RelPath, RemoteUrl, RendererId, SchemaResourceId, Slug,
    SpdxExpression, ToolName, ValueError, ValueKind, ValueReason, VersionLabel,
};
use domain::text::sanitise;
use serde_json::Value;

use ValueReason::{Character, Empty, Forbidden, Syntax, TooLong};

/// One newtype under test, behind plain function pointers so every type runs the same checks.
struct Entry {
    name: &'static str,
    kind: ValueKind,
    max: usize,
    /// A string of exactly the given length that is valid for the type (for lengths >= `min_fill`).
    fill: fn(usize) -> String,
    /// Short valid samples.
    valid: &'static [&'static str],
    /// Prefix used when inserting forbidden characters (short, so multi-byte characters still fit).
    base: &'static str,
    parse: fn(&str) -> Result<String, ValueError>,
    decode: fn(Value) -> Result<String, serde_json::Error>,
    /// Parse, display, serialise and deserialise agree on the input.
    roundtrip: fn(&str) -> bool,
}

macro_rules! entry {
    ($ty:ident, $max:expr, $fill:expr, $base:expr, $valid:expr) => {
        Entry {
            name: stringify!($ty),
            kind: ValueKind::$ty,
            max: $max,
            fill: $fill,
            valid: $valid,
            base: $base,
            parse: |s| $ty::parse(s).map(|v| v.as_str().to_owned()),
            decode: |v| serde_json::from_value::<$ty>(v).map(|t| t.as_str().to_owned()),
            roundtrip: |s| match $ty::parse(s) {
                Ok(v) => {
                    v.as_str() == s
                        && v.to_string() == s
                        && serde_json::to_value(&v).ok() == Some(Value::String(s.to_owned()))
                        && serde_json::from_value::<$ty>(Value::String(s.to_owned())).ok()
                            == Some(v)
                }
                Err(_) => false,
            },
        }
    };
}

fn fill_a(n: usize) -> String {
    "a".repeat(n)
}

fn with_prefix(prefix: &str, n: usize) -> String {
    format!("{prefix}{}", "a".repeat(n.saturating_sub(prefix.len())))
}

fn fill_url(n: usize) -> String {
    with_prefix("https://host/", n)
}

fn fill_canonical(n: usize) -> String {
    with_prefix("host/", n)
}

fn fill_zone(n: usize) -> String {
    with_prefix("Area/", n)
}

fn fill_time(n: usize) -> String {
    with_prefix("12:30", n).replace('a', "0")
}

/// A valid `CredentialRef` of exactly `n` bytes: 64-byte `a.a.` segments separated by `:` (P02-D03 O1
/// caps a segment at 64 bytes; `.` breaks the key-like run). Valid for `n` whose last index is not a `:`.
fn fill_locator(n: usize) -> String {
    (0..n)
        .map(|i| match (i % 65, i % 2) {
            (64, _) => ':',
            (_, 0) => 'a',
            _ => '.',
        })
        .collect()
}

fn entries() -> Vec<Entry> {
    vec![
        entry!(
            Digest,
            64,
            fill_a,
            "a",
            &[
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ]
        ),
        entry!(
            GitOid,
            64,
            fill_a,
            "a",
            &[
                "0123456789abcdef0123456789abcdef01234567",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ]
        ),
        entry!(
            RelPath,
            4096,
            fill_a,
            "a",
            &[
                "a",
                "dir/file.yaml",
                ".github/workflows/ci.yml",
                "dir name/x y.yml",
                "\u{fc}n\u{ef}/\u{e7}od\u{e9}.yaml",
                "1:b/c",
                "dir/a:b",
            ]
        ),
        entry!(
            AppRelPath,
            4096,
            fill_a,
            "a",
            &["repos/a1b2", "backup/20261027T101500Z_repo_run.json", "x",]
        ),
        entry!(
            LocalPath,
            4096,
            fill_a,
            "a",
            &["/var/lib/pwt", "data/x", "/a", "rel/path"]
        ),
        entry!(
            PathPattern,
            1024,
            fill_a,
            "a",
            &[
                "**/*.yaml",
                "charts/*/values.yaml",
                "[ab]*.yml",
                "a?.yml",
                "[!a]x",
                "[^a-z]*.yaml",
                "dir name/*.yml",
                "*.yaml",
                "**",
                ".github/**/*.yml",
            ]
        ),
        entry!(
            GitRefName,
            255,
            fill_a,
            "a",
            &[
                "main",
                "feature/x-1",
                "v1.2.3",
                "release/2026.10",
                "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678",
                "hot_fix",
            ]
        ),
        entry!(
            RemoteUrl,
            2048,
            fill_url,
            "a",
            &[
                "https://github.com/org/repo.git",
                "https://example.com:8443/a/b",
                "ssh://git@host.example/org/repo.git",
                "ssh://host:22/org/r",
                "git@github.com:org/repo.git",
                "https://[2001:db8::1]/r",
                "https://192.168.0.1/r",
                "https://host/a%20b",
                "git@[::1]:org/r",
                "ssh://git@[2001:db8::1]:2222/r",
            ]
        ),
        entry!(
            CanonicalRemote,
            2048,
            fill_canonical,
            "a",
            &[
                "github.com/org/repo",
                "example.com/a/b/c",
                "192.168.0.1/x",
                "[2001:db8::1]/x",
            ]
        ),
        entry!(Slug, 48, fill_a, "a", &["repo", "my-repo-2", "a", "0"]),
        entry!(
            LocalTime,
            5,
            fill_time,
            "1",
            &["00:00", "23:59", "09:05", "12:30"]
        ),
        entry!(
            IanaTimezone,
            64,
            fill_zone,
            "a",
            &[
                "UTC",
                "Europe/Berlin",
                "America/Argentina/Buenos_Aires",
                "Etc/GMT+5",
                "Asia/Ho_Chi_Minh",
                "Asia/Kolkata",
            ]
        ),
        entry!(
            Destination,
            2048,
            fill_url,
            "a",
            &[
                "api.example.com",
                "example.com:443",
                "https://openrouter.ai/api/v1",
                "https://example.com",
                "https://example.com/",
                "https://example.com:8443/x",
                "10.0.0.1:80",
                "[::1]:8080",
                "[::1]",
                "localhost",
            ]
        ),
        entry!(
            RedactionVersion,
            128,
            fill_a,
            "a",
            &["v1", "2026.10-a", "rules:1.2"]
        ),
        entry!(
            VersionLabel,
            128,
            fill_a,
            "a",
            &["1.2.3", "v1.2.3+build.5", "git-2.43.0"]
        ),
        entry!(
            ToolName,
            128,
            fill_a,
            "a",
            &["yamllint", "docker-compose", "rust-schema", "gitleaks"]
        ),
        entry!(
            FrameworkId,
            128,
            fill_a,
            "a",
            &["github-actions", "kubernetes", "docker-compose", "k8s"]
        ),
        entry!(
            RendererId,
            128,
            fill_a,
            "a",
            &["helm", "helm:3.14", "Kustomize"]
        ),
        entry!(
            SchemaResourceId,
            128,
            fill_a,
            "a",
            &[
                "github-workflow",
                "bundled:schemas/github-workflow.json",
                "user/schema@2",
            ]
        ),
        entry!(
            ModelId,
            256,
            fill_a,
            "a",
            &[
                "anthropic/claude-sonnet-5",
                "openai/gpt-4o:free",
                "vendor/model@2026-10",
            ]
        ),
        entry!(
            ProviderId,
            128,
            fill_a,
            "a",
            &["openrouter", "anthropic", "Azure"]
        ),
        entry!(
            CredentialRef,
            256,
            fill_locator,
            "a",
            &[
                "openrouter-key",
                "keychain:pwt.openrouter",
                "pwt.openrouter.default",
                "skip-1",
            ]
        ),
        entry!(
            PolicyId,
            128,
            fill_a,
            "a",
            &["yamllint.default", "parser:sarif-v2", "p_1"]
        ),
        entry!(
            SpdxExpression,
            256,
            fill_a,
            "a",
            &[
                "MIT",
                "Apache-2.0 OR MIT",
                "(MIT OR Apache-2.0) AND BSD-3-Clause",
                "GPL-2.0+",
                "LicenseRef-Vendor-1",
            ]
        ),
        entry!(
            EventCode,
            128,
            fill_a,
            "a",
            &["storage.failed", "run_finished", "e-01"]
        ),
        entry!(
            Identifier,
            128,
            fill_a,
            "a",
            &["dark", "a:b/c@1+2._-", "x86_64", "Dark"]
        ),
    ]
}

/// Compile-time guard: adding a `ValueKind` variant breaks this match until `ALL_KINDS` and `entries()` follow.
fn kind_index(k: ValueKind) -> usize {
    match k {
        ValueKind::Digest => 0,
        ValueKind::GitOid => 1,
        ValueKind::RelPath => 2,
        ValueKind::AppRelPath => 3,
        ValueKind::LocalPath => 4,
        ValueKind::PathPattern => 5,
        ValueKind::GitRefName => 6,
        ValueKind::RemoteUrl => 7,
        ValueKind::CanonicalRemote => 8,
        ValueKind::Slug => 9,
        ValueKind::LocalTime => 10,
        ValueKind::IanaTimezone => 11,
        ValueKind::Destination => 12,
        ValueKind::RedactionVersion => 13,
        ValueKind::VersionLabel => 14,
        ValueKind::ToolName => 15,
        ValueKind::FrameworkId => 16,
        ValueKind::RendererId => 17,
        ValueKind::SchemaResourceId => 18,
        ValueKind::ModelId => 19,
        ValueKind::ProviderId => 20,
        ValueKind::CredentialRef => 21,
        ValueKind::PolicyId => 22,
        ValueKind::SpdxExpression => 23,
        ValueKind::EventCode => 24,
        ValueKind::Identifier => 25,
    }
}

fn entry_named<'a>(all: &'a [Entry], name: &str) -> &'a Entry {
    all.iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no entry named {name}"))
}

fn reason_of(e: &Entry, input: &str) -> Option<ValueReason> {
    (e.parse)(input).err().map(|err| err.reason)
}

/// Code points that must be rejected everywhere: NUL, C0, DEL, C1, bidi, zero-width, line separators.
const FORBIDDEN: &[char] = &[
    '\u{0000}', '\u{0001}', '\u{0009}', '\u{000A}', '\u{000D}', '\u{001B}', '\u{001F}', '\u{007F}',
    '\u{0080}', '\u{0085}', '\u{009F}', '\u{061C}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{200E}',
    '\u{200F}', '\u{2028}', '\u{2029}', '\u{202A}', '\u{202E}', '\u{2060}', '\u{2066}', '\u{2069}',
    '\u{FEFF}',
];

#[test]
fn there_is_one_entry_per_value_kind() {
    let all = entries();
    assert_eq!(all.len(), 26);
    let mut seen = [false; 26];
    for e in &all {
        let i = kind_index(e.kind);
        assert!(!seen[i], "{} listed twice", e.name);
        seen[i] = true;
        assert_eq!(format!("{:?}", e.kind), e.name);
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn valid_samples_parse_and_roundtrip() {
    for e in entries() {
        assert!(!e.valid.is_empty(), "{}: no samples", e.name);
        for s in e.valid {
            assert_eq!((e.parse)(s).as_deref().ok(), Some(*s), "{}: {s}", e.name);
            assert!((e.roundtrip)(s), "{}: round trip of {s}", e.name);
            let decoded = (e.decode)(Value::String((*s).to_owned()));
            assert_eq!(decoded.ok().as_deref(), Some(*s), "{}: decode {s}", e.name);
        }
    }
}

#[test]
fn maximum_length_is_accepted_and_one_more_is_rejected() {
    for e in entries() {
        let at_max = (e.fill)(e.max);
        assert_eq!(at_max.len(), e.max, "{}: filler length", e.name);
        assert!(
            (e.parse)(&at_max).is_ok(),
            "{}: max length rejected",
            e.name
        );
        assert!((e.roundtrip)(&at_max), "{}: max length round trip", e.name);
        let over = (e.fill)(e.max + 1);
        assert_eq!(over.len(), e.max + 1, "{}: filler length", e.name);
        assert_eq!(reason_of(&e, &over), Some(TooLong), "{}: max+1", e.name);
        let decoded = (e.decode)(Value::String(over));
        assert!(decoded.is_err(), "{}: max+1 decoded", e.name);
    }
}

#[test]
fn empty_is_rejected_by_every_type() {
    for e in entries() {
        assert_eq!(reason_of(&e, ""), Some(Empty), "{}", e.name);
        assert!(
            (e.decode)(Value::String(String::new())).is_err(),
            "{}",
            e.name
        );
    }
}

#[test]
fn controls_bidi_and_zero_width_are_rejected_by_every_type() {
    for e in entries() {
        for &c in FORBIDDEN {
            for input in [
                c.to_string(),
                format!("{}{c}", e.base),
                format!("{c}{}", e.base),
                format!("{}{c}{}", e.base, e.base),
            ] {
                assert_eq!(
                    reason_of(&e, &input),
                    Some(Character),
                    "{}: U+{:04X} in {input:?}",
                    e.name,
                    u32::from(c)
                );
                let decoded = (e.decode)(Value::String(input));
                assert!(
                    decoded.is_err(),
                    "{}: U+{:04X} decoded",
                    e.name,
                    u32::from(c)
                );
            }
        }
    }
}

#[test]
fn the_forbidden_character_set_equals_the_text_escape_set_plus_tab_and_lf() {
    // RelPath accepts every other character, so it exposes the common rule exactly.
    let mut rejected = 0usize;
    for cp in 0..=0x0010_FFFFu32 {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let input = format!("1{c}b");
        let escaped = sanitise(&input).as_str() != input;
        let expected_reject = escaped || c == '\t' || c == '\n' || c == '\\';
        let got = RelPath::parse(&input);
        assert_eq!(got.is_err(), expected_reject, "U+{cp:04X}");
        if got.is_err() {
            rejected += 1;
        }
    }
    // 224 escape-set code points (82 P01 + 142 P02-D03 O8) + TAB + LF + backslash.
    assert_eq!(rejected, 227);
}

#[test]
fn errors_name_the_type_and_never_contain_the_input() {
    const MARKER: &str = "PLANTEDMARKER";
    for e in entries() {
        let input = format!("{MARKER}\u{0000}");
        let err = (e.parse)(&input).err();
        let Some(err) = err else {
            panic!("{}: planted input accepted", e.name);
        };
        assert_eq!(err.kind, e.kind, "{}", e.name);
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains(MARKER), "{}: {text}", e.name);
            assert!(text.contains(e.name), "{}: {text}", e.name);
        }
        // Serde: from a Value and from JSON text.
        let from_value = (e.decode)(Value::String(input.clone()));
        let msg = from_value.err().map(|x| x.to_string()).unwrap_or_default();
        assert!(!msg.is_empty(), "{}: decode accepted", e.name);
        assert!(!msg.contains(MARKER), "{}: {msg}", e.name);
        // A valid-looking marker rejected for another reason must not be echoed either.
        let long = format!("{MARKER}{}", "x".repeat(e.max + 8));
        let text = (e.parse)(&long)
            .err()
            .map(|x| x.to_string())
            .unwrap_or_default();
        assert!(!text.contains(MARKER), "{}: {text}", e.name);
    }
}

#[test]
fn non_string_json_is_rejected_without_echo() {
    let samples = [
        ("9876543", "9876543"),
        ("-9876543", "9876543"),
        ("3.14159", "3.14159"),
        ("true", "true"),
        ("null", "null"),
        ("[\"PLANTEDMARKER\"]", "PLANTEDMARKER"),
        ("{\"PLANTEDMARKER\":1}", "PLANTEDMARKER"),
    ];
    for e in entries() {
        for (json, needle) in samples {
            // Dispatch through serde_json::from_str so the real deserializer path is used.
            let msg = from_str_error(e.name, json);
            assert!(!msg.is_empty(), "{}: {json} accepted", e.name);
            if needle != "null" && needle != "true" {
                assert!(!msg.contains(needle), "{}: {msg}", e.name);
            }
        }
    }
}

/// Error text of `serde_json::from_str::<Type>(json)`; empty if it decoded.
fn from_str_error(name: &str, json: &str) -> String {
    macro_rules! by_name {
        ($($ty:ident),* $(,)?) => {
            match name {
                $(stringify!($ty) => serde_json::from_str::<$ty>(json)
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_default(),)*
                other => panic!("unknown type {other}"),
            }
        };
    }
    by_name!(
        Digest,
        GitOid,
        RelPath,
        AppRelPath,
        LocalPath,
        PathPattern,
        GitRefName,
        RemoteUrl,
        CanonicalRemote,
        Slug,
        LocalTime,
        IanaTimezone,
        Destination,
        RedactionVersion,
        VersionLabel,
        ToolName,
        FrameworkId,
        RendererId,
        SchemaResourceId,
        ModelId,
        ProviderId,
        CredentialRef,
        PolicyId,
        SpdxExpression,
        EventCode,
        Identifier,
    )
}

#[test]
fn json_text_decodes_and_encodes_through_parse() {
    let slug = serde_json::from_str::<Slug>("\"my-repo\"").expect("valid slug");
    assert_eq!(slug.as_str(), "my-repo");
    assert!(serde_json::from_str::<Slug>("\"My-Repo\"").is_err());
    let json = serde_json::to_string(&Slug::parse("a-b").expect("valid slug")).expect("serialise");
    assert_eq!(json, "\"a-b\"");
}

#[test]
fn types_are_ordered_hashable_and_comparable() {
    let a = Slug::parse("a").expect("valid slug");
    let b = Slug::parse("b").expect("valid slug");
    assert!(a < b);
    assert_eq!(a, Slug::parse("a").expect("valid slug"));
    let set: std::collections::HashSet<Slug> = [a, b].into_iter().collect();
    assert_eq!(set.len(), 2);
}

/// `(type name, input, expected reason)` for the per-type rules. A dense table, kept on long lines.
#[rustfmt::skip]
fn rejections() -> Vec<(&'static str, String, ValueReason)> {
    let mut v: Vec<(&'static str, String, ValueReason)> = Vec::new();
    let mut add = |name: &'static str, inputs: &[&str], reason: ValueReason| {
        for i in inputs {
            v.push((name, (*i).to_owned(), reason));
        }
    };

    add("Digest", &["A".repeat(64).as_str()], Character);
    add("Digest", &["a".repeat(63).as_str(), "0"], Syntax);
    add("Digest", &[format!("{}g", "a".repeat(63)).as_str()], Character);
    add("GitOid", &["a".repeat(39).as_str(), "a".repeat(41).as_str(), "a".repeat(63).as_str()], Syntax);
    add("GitOid", &["A".repeat(40).as_str(), "x".repeat(40).as_str()], Character);

    let rel_syntax = [
        "/a",
        "/",
        "C:/a",
        "c:",
        "C:foo",
        ".",
        "..",
        "a/./b",
        "a/../b",
        "../a",
        "./a",
        "a//b",
        "a/",
        "a/b/",
    ];
    for name in ["RelPath", "AppRelPath"] {
        add(name, &rel_syntax, Syntax);
        add(name, &["a\\b", "C:\\x", "\\a", "a/b\\"], Character);
    }

    add("LocalPath", &["/", "/a/../b", "..", "a/..", "/a//b", "/a/", "/a/./b", "./a", "C:foo"], Syntax);
    add("LocalPath", &["C:\\x", "a\\b", "/a\\b"], Character);

    add("PathPattern", &["{a,b}", "*.{yml,yaml}", "$x", "a`b", "a;b", "a|b", "a&b", "a<b", "a>b", "a\\b", "~/x", "x(y)", "%41", "a'b", "a\"b", "a#b", "a:b"], Character);
    add("PathPattern", &["/abs/*.yaml", "/etc", "../x", "a/../b", "..", "a//b", "a/", "[ab", "[]", "[!]", "***", "a]b", "a!b", "a^b", "[a[b]", "[a*]", "[a!b]"], Syntax);

    add("GitRefName", &["a b", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", " main"], Character);
    add("GitRefName", &["a..b", "a@{b", "@{", "-x", "-", "a.", "a/", "x.lock", "a/b.lock", "a.lock/b", "/a", "a//b", ".hidden", "a/.b", "@"], Syntax);

    add("RemoteUrl", &["https://user:pass@host/x", "https://user@host/x", "https://host/x?token=abc", "https://host/x?", "https://host/x#frag", "https://host/x#", "https://us%40er:p@host/x", "https://us%40er@host/x", "ssh://us%40er@host/x", "ssh://user:pw@host/x", "git:pw@host:path", "us%40er@host:path", "git@host:path?x=1", "git@host:path#f", "ssh://git@host/x?y"], Forbidden);
    add("RemoteUrl", &["http://host/x", "ftp://h/x", "file:///etc", "host/path", "https://host", "https://host/", "https://-bad.com/x", "https://host:0/x", "https://host:99999/x", "https://host:/x", "https://host/%zz", "https://host/a%2", "git@host", "https://[::1", "https://[zz]/x", "ssh://@host/x", "ssh://host", "ssh://user@host:22", "https:///x", "https://a..b/x", "HTTPS://host/x", "git@:path", "git@[::1]path"], Syntax);
    add("RemoteUrl", &["https://ho st/x", "https://h\u{f4}st/x", "https://host/a\\b", "https://host:abc/x", "https://ex_ample.com/x", "https://host/ x", "git@h\u{f4}st:p"], Character);

    add("CanonicalRemote", &["https://github.com/org/repo", "git@github.com/x", "github.com:443/x", "github.com/org/repo?x=1", "github.com/org#f", "ssh://github.com/x", "user:pw@github.com/x"], Forbidden);
    add("CanonicalRemote", &["GitHub.com/org/repo", "github.com", "github.com/", "-a.com/x", "/x", "[2001:DB8::1]/x", "[zz]/x", "gith..ub.com/x", "github.com/a%zz"], Syntax);
    add("CanonicalRemote", &["gith ub.com/x", "git_hub.com/x", "github.com/a b", "github.com/a\\b", "h\u{f4}st.com/x"], Character);

    add("Slug", &["-a", "a-", "a--b", "-"], Syntax);
    add("Slug", &["Repo", "a_b", "a b", "a.b", "\u{fc}"], Character);

    add("LocalTime", &["24:00", "12:60", "1:30", "123:0", "99:99", "12:3", "::::"], Syntax);
    add("LocalTime", &["12-30", "ab:cd", "12:3a", "1 :30", "\u{663}\u{663}:"], Character);
    add("LocalTime", &["12:30:"], TooLong);

    add("IanaTimezone", &["utc", "Europe", "GMT", "Europe//Berlin", "/Europe/Berlin", "Europe/Berlin/", "a/b/c/d", "/"], Syntax);
    add("IanaTimezone", &["Europe/Ber lin", "Europe\\Berlin", "Europe/Bérlin", "Europe/Berlin?", "Europe/Berlin:1"], Character);

    add("Destination", &["https://user:pw@h/x", "https://u@h", "https://h/x?q=1", "https://h/x#f", "h@x", "h?x", "h#x", "user:pw@h:443"], Forbidden);
    add("Destination", &["h/path", "http://h", "ftp://h", "::1", "h:0", "h:", "https://", "https://h:0", "https://h//x%zz", "-h", "[::1", "[zz]:1"], Syntax);
    add("Destination", &["h:abc", "ho st", "https://ho st", "h\u{f4}st", "h_st"], Character);

    // Token family: shared charset.
    let token_bad = ["a b", "a\\b", "a;b", "\u{fc}n\u{ef}", "a#b", "a?b", "a,b", "a=b", "a|b", "a'b", "a\"b", "a(b)", "a*b"];
    for name in [
        "RedactionVersion",
        "VersionLabel",
        "RendererId",
        "SchemaResourceId",
        "ModelId",
        "ProviderId",
        "CredentialRef",
        "PolicyId",
        "Identifier",
        "ToolName",
        "FrameworkId",
        "EventCode",
    ] {
        add(name, &token_bad, Character);
    }
    // SPDX allows space and parentheses, nothing else beyond the token set.
    add("SpdxExpression", &["MIT;", "MIT|x", "MIT,Apache", "\u{fc}", "MIT\tOR", "a\\b", "MIT{1}", "a=b"], Character);
    // Lowercase-only types.
    for name in ["ToolName", "FrameworkId", "EventCode"] {
        add(name, &["Upper", "aB", "a:b", "a/b", "a@b", "a+b"], Character);
    }

    add("CredentialRef", &["sk-abc", "SK-abc", "Sk-live", "sk-or-v1-0123456789abcdef0123456789abcdef0123456789abcdef", "0123456789abcdef0123456789abcdef0123456789", "a".repeat(32).as_str(), "A_".repeat(16).as_str(), "x-".repeat(16).as_str(), "key:abcdefghijklmnopqrstuvwxyz0123456789"], Forbidden);
    v
}

#[test]
fn per_type_rejection_rules() {
    let all = entries();
    let cases = rejections();
    assert!(cases.len() > 300, "only {} cases", cases.len());
    for (name, input, expected) in &cases {
        let e = entry_named(&all, name);
        assert_eq!(
            reason_of(e, input),
            Some(*expected),
            "{name}: {input:?} expected {expected:?}"
        );
        assert!(
            (e.decode)(Value::String(input.clone())).is_err(),
            "{name}: {input:?}"
        );
    }
    // Every type has at least one targeted rejection beyond the common rule.
    for e in &all {
        assert!(
            cases.iter().any(|(n, _, _)| *n == e.name),
            "{}: no targeted case",
            e.name
        );
    }
}

#[test]
fn remote_url_requires_no_credentials_query_or_fragment() {
    // The request's named cases, spelled out.
    for input in [
        "https://user:pass@host/x",
        "https://host/x?token=abc",
        "https://host/x#frag",
        "https://us%40er@host/x",
    ] {
        assert_eq!(
            RemoteUrl::parse(input).map_err(|e| e.reason),
            Err(Forbidden),
            "{input}"
        );
    }
    assert!(RemoteUrl::parse("https://host/x").is_ok());
}

#[test]
fn credential_reference_key_heuristics() {
    // 31-character runs are fine, 32 are not; `.` and `:` break a run. (`/`, `@`, `+` also break a run but
    // are no longer in the locator charset: P02-D03 O1.)
    let run31 = "a".repeat(31);
    assert!(CredentialRef::parse(&run31).is_ok());
    assert!(CredentialRef::parse(&format!("{run31}.{run31}")).is_ok());
    assert!(CredentialRef::parse(&format!("{run31}:{run31}:{run31}")).is_ok());
    assert_eq!(
        CredentialRef::parse(&format!("{run31}:{run31}/{run31}")).map_err(|e| e.reason),
        Err(Character)
    );
    assert_eq!(
        CredentialRef::parse(&"a".repeat(32)).map_err(|e| e.reason),
        Err(Forbidden)
    );
    // `-` and `_` extend a run.
    assert_eq!(
        CredentialRef::parse(&format!(
            "{}-{}_{}",
            "a".repeat(15),
            "b".repeat(8),
            "c".repeat(8)
        ))
        .map_err(|e| e.reason),
        Err(Forbidden)
    );
    // A 40-character key-like run.
    assert_eq!(
        CredentialRef::parse("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN").map_err(|e| e.reason),
        Err(Forbidden)
    );
    // `sk-` prefix only at the start.
    assert_eq!(
        CredentialRef::parse("sk-x").map_err(|e| e.reason),
        Err(Forbidden)
    );
    assert!(CredentialRef::parse("task-x").is_ok());
    assert!(CredentialRef::parse("openrouter:sk").is_ok());
}

#[test]
fn local_time_boundaries() {
    for ok in ["00:00", "00:59", "23:00", "23:59"] {
        assert!(LocalTime::parse(ok).is_ok(), "{ok}");
    }
    for bad in ["24:00", "23:60", "00:6O"] {
        assert!(LocalTime::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn timezone_accepts_syntax_only() {
    // tzdb membership is checked in P14: a well-formed but unknown zone passes here.
    assert!(IanaTimezone::parse("Nowhere/Land").is_ok());
    assert!(IanaTimezone::parse("Etc/GMT-14").is_ok());
}

#[test]
fn path_types_are_distinct_types_with_equal_rules() {
    let rel = RelPath::parse("a/b.yaml").map(|p| p.to_string());
    let app = AppRelPath::parse("a/b.yaml").map(|p| p.to_string());
    assert_eq!(rel.ok(), app.ok());
    // Distinct error kinds identify the rejecting type.
    assert_eq!(
        RelPath::parse("/a").map_err(|e| e.kind),
        Err(ValueKind::RelPath)
    );
    assert_eq!(
        AppRelPath::parse("/a").map_err(|e| e.kind),
        Err(ValueKind::AppRelPath)
    );
    assert_eq!(
        LocalPath::parse("a/../b").map_err(|e| e.kind),
        Err(ValueKind::LocalPath)
    );
}

// --- P02-D02 -----------------------------------------------------------------------------------------------

#[test]
fn rel_path_length_bound_is_in_bytes_not_characters() {
    // D02-5a: 4096 bytes. U+00E9 is 2 bytes, U+20AC is 3 bytes.
    const BOUND: usize = 4096;
    let two_byte_at = "\u{e9}".repeat(BOUND / 2);
    let three_byte_at = format!("{}a", "\u{20ac}".repeat((BOUND - 1) / 3));
    for at in [&two_byte_at, &three_byte_at] {
        assert_eq!(at.len(), BOUND);
        assert!(at.chars().count() < BOUND);
        assert!(RelPath::parse(at).is_ok(), "exactly the bound is accepted");
        let json = serde_json::from_value::<RelPath>(Value::String(at.clone()));
        assert!(json.is_ok());
    }

    // One byte over, with far fewer characters than the bound.
    let two_byte_over = format!("{two_byte_at}a");
    let three_byte_over = "\u{20ac}".repeat(BOUND / 3 + 1);
    assert_eq!(two_byte_over.len(), BOUND + 1);
    assert_eq!(three_byte_over.len(), BOUND + 2);
    for over in [&two_byte_over, &three_byte_over] {
        assert!(over.chars().count() < BOUND);
        assert_eq!(
            RelPath::parse(over).map_err(|e| e.reason),
            Err(TooLong),
            "over the byte bound"
        );
        let json = serde_json::from_value::<RelPath>(Value::String(over.clone()));
        assert!(json.is_err());
    }
}

#[test]
fn ascii_only_types_reject_non_ascii_input() {
    // D02-5b: a token type (ModelId) and Slug.
    for input in [
        "vendor/m\u{f6}del",
        "\u{fc}n\u{ef}",
        "model\u{20ac}",
        "\u{65e5}\u{672c}",
    ] {
        assert_eq!(
            ModelId::parse(input).map_err(|e| e.reason),
            Err(Character),
            "ModelId {input:?}"
        );
        assert!(serde_json::from_value::<ModelId>(Value::String(input.to_owned())).is_err());
    }
    for input in ["rep\u{f6}", "\u{fc}", "\u{65e5}\u{672c}", "a-\u{e9}"] {
        assert_eq!(
            Slug::parse(input).map_err(|e| e.reason),
            Err(Character),
            "Slug {input:?}"
        );
        assert!(serde_json::from_value::<Slug>(Value::String(input.to_owned())).is_err());
    }
}

// --- P02-D03 (W1: O1, O4, O5, O6, O7, O9, O10) --------------------------------------------------------------

fn reason<T>(r: Result<T, ValueError>) -> Option<ValueReason> {
    r.err().map(|e| e.reason)
}

#[test]
fn d03_o1_credential_ref_is_a_lowercase_locator() {
    for ok in [
        "pwt:openrouter:default",
        "os-keyring.pwt",
        "session:1",
        "a",
        "0",
        "a_b:c_d",
        "x:-y",
        "x:.y",
    ] {
        assert!(CredentialRef::parse(ok).is_ok(), "{ok}");
    }
    // 64-byte segments are the maximum, in the first and in a later position.
    let seg64 = "a.".repeat(32);
    assert_eq!(seg64.len(), 64);
    assert!(CredentialRef::parse(&seg64).is_ok());
    assert!(CredentialRef::parse(&format!("{seg64}:{seg64}")).is_ok());

    // An AWS-secret-like shape (synthetic) has `/`, `+` and uppercase: Character.
    for bad in [
        "abcdEFGH/ijkl+MNOPqrst/uvwxYZ0123456789abcd",
        "Upper",
        "aB",
        "a/b",
        "a+b",
        "a@b",
        "a=b",
    ] {
        assert_eq!(reason(CredentialRef::parse(bad)), Some(Character), "{bad}");
        assert!(serde_json::from_value::<CredentialRef>(Value::String(bad.to_owned())).is_err());
    }
    // Segment shape: empty, over 64 bytes, or a first segment that does not start with [a-z0-9].
    let seg65 = format!("{}a", "a.".repeat(32));
    assert_eq!(seg65.len(), 65);
    for bad in [
        ":a".to_owned(),
        "a:".to_owned(),
        "a::b".to_owned(),
        ":".to_owned(),
        "_a".to_owned(),
        ".a".to_owned(),
        seg65.clone(),
        format!("x:{seg65}"),
        format!("{seg65}:x"),
    ] {
        assert_eq!(reason(CredentialRef::parse(&bad)), Some(Syntax), "{bad}");
    }
    // The key checks come first and stay Forbidden.
    assert_eq!(reason(CredentialRef::parse("sk-abc")), Some(Forbidden));
    assert_eq!(
        reason(CredentialRef::parse(&"a".repeat(65))),
        Some(Forbidden)
    );
}

#[test]
fn d03_o4_remote_url_rejects_option_like_values() {
    for bad in [
        "-oProxyCommand=x@host:path",
        "-https://host/x",
        "-git@host:p",
        "ssh://-user@host/p",
        "-user@host:p",
        "git@host:-path",
        "git@[::1]:-path",
        "https://host/-x",
        "https://host:8443/-x",
        "ssh://git@host/-x",
        "ssh://host/-x",
    ] {
        assert_eq!(reason(RemoteUrl::parse(bad)), Some(Syntax), "{bad}");
        assert!(serde_json::from_value::<RemoteUrl>(Value::String(bad.to_owned())).is_err());
    }
    for ok in [
        "https://host/x-y",
        "https://host/a/-b",
        "ssh://git@host/a/-b",
        "git@host:a/-b",
        "git@host:x-",
        "ssh://us-er@host/p",
        "us-er@host:p",
    ] {
        assert!(RemoteUrl::parse(ok).is_ok(), "{ok}");
    }
}

#[test]
fn d03_o4_token_types_reject_leading_dash_and_dot_tokens() {
    let all = entries();
    for name in [
        "RedactionVersion",
        "VersionLabel",
        "ToolName",
        "FrameworkId",
        "RendererId",
        "SchemaResourceId",
        "ModelId",
        "ProviderId",
        "CredentialRef",
        "PolicyId",
        "SpdxExpression",
        "EventCode",
        "Identifier",
    ] {
        let e = entry_named(&all, name);
        for bad in ["-x", "-", "--x", "-1.0", ".", ".."] {
            assert_eq!(reason_of(e, bad), Some(Syntax), "{name}: {bad}");
            assert!(
                (e.decode)(Value::String(bad.to_owned())).is_err(),
                "{name}: {bad}"
            );
        }
        // Only a leading `-` and the whole values `.` and `..` are rejected as Syntax.
        for ok in ["x-", "a.b", "a..b", "x-y", "1-x"] {
            assert!(reason_of(e, ok).is_none(), "{name}: {ok}");
        }
    }
    // Path types keep accepting a leading `-` (repository file names).
    assert!(RelPath::parse("-file").is_ok());
    assert!(RelPath::parse("dir/-file").is_ok());
    assert!(AppRelPath::parse("-file").is_ok());
    assert!(LocalPath::parse("-file").is_ok());
    assert!(LocalPath::parse("/-file").is_ok());
    assert!(PathPattern::parse("-*.yml").is_ok());
}

/// Hosts that must be rejected as non-canonical (O5), as `Syntax`.
const NON_CANONICAL_HOSTS: &[&str] = &[
    "127.1",
    "2130706433",
    "0x7f.0.0.1",
    "017.0.0.1",
    "127.0.0.01",
    "1.2.3.4.5",
    "0X7F.0.0.1",
    "host.0x1",
    "1",
    "Example.com",
    "EXAMPLE.COM",
    "exAmple.com",
    "[2001:DB8::1]",
    "[::FFFF:1.2.3.4]",
];

/// Hosts that are accepted (O5).
const CANONICAL_HOSTS: &[&str] = &[
    "127.0.0.1",
    "10.0.0.255",
    "0.0.0.0",
    "example.com",
    "a1.b2",
    "localhost",
    "x-1.y",
    "[2001:db8::1]",
    "[::1]",
];

fn remote_url_forms(host: &str) -> [String; 4] {
    [
        format!("https://{host}/r"),
        format!("ssh://git@{host}/r"),
        format!("ssh://{host}:22/r"),
        format!("git@{host}:r"),
    ]
}

#[test]
fn d03_o5_hosts_must_be_canonical_in_all_host_bearing_types() {
    for host in NON_CANONICAL_HOSTS {
        for url in remote_url_forms(host) {
            assert_eq!(reason(RemoteUrl::parse(&url)), Some(Syntax), "{url}");
        }
        let dests = [
            (*host).to_owned(),
            format!("{host}:443"),
            format!("https://{host}"),
            format!("https://{host}/r"),
            format!("https://{host}:8443/r"),
        ];
        for d in dests {
            assert_eq!(reason(Destination::parse(&d)), Some(Syntax), "{d}");
        }
        let canon = format!("{host}/x");
        assert_eq!(
            reason(CanonicalRemote::parse(&canon)),
            Some(Syntax),
            "{canon}"
        );
    }
    for host in CANONICAL_HOSTS {
        for url in remote_url_forms(host) {
            assert!(RemoteUrl::parse(&url).is_ok(), "{url}");
        }
        assert!(Destination::parse(host).is_ok(), "{host}");
        assert!(Destination::parse(&format!("{host}:443")).is_ok(), "{host}");
        assert!(Destination::parse(&format!("https://{host}/r")).is_ok());
        let canon = format!("{host}/x");
        assert!(CanonicalRemote::parse(&canon).is_ok(), "{canon}");
    }
}

#[test]
fn d03_o5_leading_zero_octets_are_rejected_explicitly() {
    // Whatever std does with a leading-zero octet, the value must equal the address's Display form.
    for host in ["017.0.0.1", "127.0.0.01", "127.000.000.001", "00.0.0.0"] {
        for url in remote_url_forms(host) {
            assert_eq!(reason(RemoteUrl::parse(&url)), Some(Syntax), "{url}");
        }
        assert_eq!(reason(Destination::parse(host)), Some(Syntax), "{host}");
        assert_eq!(
            reason(CanonicalRemote::parse(&format!("{host}/x"))),
            Some(Syntax),
            "{host}"
        );
    }
    assert!(Destination::parse("0.0.0.0").is_ok());
    assert!(Destination::parse("10.0.0.255").is_ok());
}

#[test]
fn d03_o6_ssh_and_scp_user_is_at_most_32_bytes_and_not_key_like() {
    let u32b = "a.".repeat(16);
    let u33b = format!("{u32b}a");
    assert_eq!((u32b.len(), u33b.len()), (32, 33));
    let forms: [fn(&str) -> String; 2] =
        [|u| format!("ssh://{u}@host/r"), |u| format!("{u}@host:r")];
    for form in forms {
        assert!(RemoteUrl::parse(&form(&u32b)).is_ok(), "32 bytes");
        assert_eq!(reason(RemoteUrl::parse(&form(&u33b))), Some(TooLong));
        // A long user of one run is TooLong first; a 32-byte run is key-like.
        assert_eq!(
            reason(RemoteUrl::parse(&form(&"a".repeat(40)))),
            Some(TooLong)
        );
        assert_eq!(
            reason(RemoteUrl::parse(&form(&"a".repeat(32)))),
            Some(Forbidden)
        );
        let dashed = format!("{}-{}", "a".repeat(16), "b".repeat(15));
        assert_eq!(dashed.len(), 32);
        assert_eq!(reason(RemoteUrl::parse(&form(&dashed))), Some(Forbidden));
        assert!(RemoteUrl::parse(&form(&"a".repeat(31))).is_ok());
        assert_eq!(reason(RemoteUrl::parse(&form("-u"))), Some(Syntax));
        // Existing rules stay.
        assert_eq!(reason(RemoteUrl::parse(&form("u:p"))), Some(Forbidden));
        assert_eq!(reason(RemoteUrl::parse(&form("u%41"))), Some(Forbidden));
        assert_eq!(reason(RemoteUrl::parse(&form("u+x"))), Some(Character));
    }
}

/// Parses `input` with the URL type selected by `idx` (0-2 `RemoteUrl`, 3 `Destination`, 4 `CanonicalRemote`).
fn url_reason(idx: usize, input: &str) -> Option<ValueReason> {
    match idx {
        0..=2 => reason(RemoteUrl::parse(input)),
        3 => reason(Destination::parse(input)),
        _ => reason(CanonicalRemote::parse(input)),
    }
}

fn url_with_escape(idx: usize, esc: &str) -> String {
    match idx {
        0 | 3 => format!("https://host/a{esc}b"),
        1 => format!("ssh://git@host/a{esc}b"),
        2 => format!("git@host:a{esc}b"),
        _ => format!("host/a{esc}b"),
    }
}

#[test]
fn d03_o7_percent_escapes_of_dangerous_bytes_are_forbidden() {
    for esc in [
        "%00", "%01", "%0a", "%0D", "%1f", "%1F", "%7F", "%7f", "%2f", "%2F", "%5c", "%5C", "%3f",
        "%3F", "%23", "%40",
    ] {
        for idx in 0..5 {
            let input = url_with_escape(idx, esc);
            assert_eq!(url_reason(idx, &input), Some(Forbidden), "{input}");
        }
    }
    for esc in ["%20", "%41", "%7e", "%80", "%25", "%2e", "%3A", "%ff"] {
        for idx in 0..5 {
            let input = url_with_escape(idx, esc);
            assert_eq!(url_reason(idx, &input), None, "{input}");
        }
    }
    // Malformed escapes stay Syntax.
    for esc in ["%", "%2!", "%zz", "%g0", "%0g"] {
        for idx in 0..5 {
            let input = url_with_escape(idx, esc);
            assert_eq!(url_reason(idx, &input), Some(Syntax), "{input}");
        }
    }
}

#[test]
fn d03_o9_spdx_expression_needs_an_alphanumeric_and_clean_spaces() {
    for ok in [
        "MIT",
        "(MIT OR Apache-2.0)",
        "Apache-2.0 WITH LLVM-exception",
        "(a)",
        "MIT AND (BSD-3-Clause OR ISC)",
    ] {
        assert!(SpdxExpression::parse(ok).is_ok(), "{ok}");
    }
    for bad in [
        "()",
        "(())",
        " ",
        "  ",
        " MIT",
        "MIT ",
        "MIT  OR Apache-2.0",
        "(MIT OR  Apache-2.0)",
        "+",
        "(+)",
        ".",
        "_:/@",
    ] {
        assert_eq!(reason(SpdxExpression::parse(bad)), Some(Syntax), "{bad:?}");
        assert!(serde_json::from_value::<SpdxExpression>(Value::String(bad.to_owned())).is_err());
    }
    // The charset rule still applies first.
    assert_eq!(reason(SpdxExpression::parse("MIT;")), Some(Character));
}

#[test]
fn d03_o10_path_pattern_allows_at_most_16_wildcards() {
    let at16 = [
        "?".repeat(16),
        format!("{}*", "*/".repeat(15)),
        "[a]".repeat(16),
        "*?".repeat(8),
        format!("{}**", "**/".repeat(15)),
        format!("{}{}{}", "[ab]".repeat(5), "?".repeat(5), "*/*/*/*/*/*"),
        "a*b*c*d*e*f*g*h*i*j*k*l*m*n*o*p*".to_owned(),
    ];
    let at17 = [
        "?".repeat(17),
        format!("{}*", "*/".repeat(16)),
        "[a]".repeat(17),
        format!("{}*", "*?".repeat(8)),
        format!("{}**", "**/".repeat(16)),
        format!("{}{}{}", "[ab]".repeat(5), "?".repeat(6), "*/*/*/*/*/*"),
        "a*b*c*d*e*f*g*h*i*j*k*l*m*n*o*p*q*".to_owned(),
    ];
    for ok in &at16 {
        assert!(PathPattern::parse(ok).is_ok(), "{ok}");
    }
    for bad in &at17 {
        assert_eq!(reason(PathPattern::parse(bad)), Some(Forbidden), "{bad}");
        assert!(serde_json::from_value::<PathPattern>(Value::String(bad.clone())).is_err());
    }
    // A run of `**` counts once, so many `**`-only patterns are not penalised.
    assert!(PathPattern::parse("**/**/a").is_ok());
    // Syntax errors outrank the count.
    assert_eq!(
        reason(PathPattern::parse(&format!("{}***", "?".repeat(20)))),
        Some(Syntax)
    );
}

#[test]
fn d03_new_forbidden_and_syntax_paths_never_echo_the_input() {
    // Lowercase marker so it stays valid for the lowercase-only CredentialRef charset.
    const MARKER: &str = "plantedmarker";
    let credential_inputs = [
        (format!("sk-{MARKER}"), Forbidden),
        (format!("{MARKER}:"), Syntax),
        (format!("{MARKER}::x"), Syntax),
        (format!("_{MARKER}"), Syntax),
        (format!("{MARKER}/x"), Character),
        (format!("-{MARKER}"), Syntax),
    ];
    for (input, expected) in &credential_inputs {
        let err = CredentialRef::parse(input).err();
        let Some(err) = err else {
            panic!("accepted: {input}");
        };
        assert_eq!(err.reason, *expected, "{input}");
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains(MARKER), "{text}");
        }
        let msg = serde_json::from_value::<CredentialRef>(Value::String(input.clone()))
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(!msg.is_empty() && !msg.contains(MARKER), "{msg}");
        let msg = serde_json::from_str::<CredentialRef>(&format!("\"{input}\""))
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(!msg.is_empty() && !msg.contains(MARKER), "{msg}");
    }

    let url_inputs = [
        (format!("https://host/{MARKER}%00"), Forbidden),
        (format!("https://host/{MARKER}%2F"), Forbidden),
        (format!("git@host:{MARKER}%40"), Forbidden),
        (format!("ssh://git@host/{MARKER}%5c"), Forbidden),
        (format!("https://host/{MARKER}%zz"), Syntax),
        (format!("https://host/-{MARKER}"), Syntax),
        (format!("-{MARKER}@host:p"), Syntax),
        (format!("https://{MARKER}.0x1/r"), Syntax),
        (format!("https://Host-{MARKER}/r"), Syntax),
    ];
    for (input, expected) in &url_inputs {
        let err = RemoteUrl::parse(input).err();
        let Some(err) = err else {
            panic!("accepted: {input}");
        };
        assert_eq!(err.reason, *expected, "{input}");
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains(MARKER), "{text}");
        }
        let msg = serde_json::from_str::<RemoteUrl>(&format!("\"{input}\""))
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(!msg.is_empty() && !msg.contains(MARKER), "{msg}");
    }
    for input in [
        format!("https://host/{MARKER}%00"),
        format!("https://{MARKER}.0x1/r"),
    ] {
        for text in [
            format!("{:?}", Destination::parse(&input).err()),
            format!("{:?}", CanonicalRemote::parse(&input).err()),
        ] {
            assert!(!text.contains(MARKER), "{text}");
        }
    }
}

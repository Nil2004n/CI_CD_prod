//! Validated newtypes for store values (FR-31 class N; Data Schema §4; `planning/P02-T01-request.md` §5.2).
//!
//! The error types below are lead-authored and frozen at B0. The newtypes themselves (one per row of the
//! request §5.2 table, each with `parse`, `as_str`, `Display`, `Serialize`, `Deserialize` through `parse`)
//! are implemented by W1 in wave 1a (T05a).
//!
//! Every newtype applies the common rule first: empty input, input longer than the type's byte bound and
//! any NUL, C0/C1 control, U+007F or code point of the `domain::text` escape set are rejected. Type-specific
//! rules follow. Errors are fieldless ([`ValueReason`]) and never contain the input.

use std::fmt;
use std::marker::PhantomData;
use std::net::Ipv6Addr;

use serde::de::{Deserializer, Visitor};

/// Which newtype rejected a value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueKind {
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
}

/// Why a value was rejected. Fieldless: errors never contain the input text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueReason {
    /// Empty where a value is required.
    Empty,
    /// Longer than the type's byte bound.
    TooLong,
    /// Contains a forbidden character (control, escape-set code point or outside the type's charset).
    Character,
    /// Characters are allowed but the structure is not (e.g. `..` segment, missing host).
    Syntax,
    /// Structurally valid but forbidden by policy (e.g. credentials in a URL, key-like credential reference).
    Forbidden,
}

/// A rejected value: the type and the reason, never the value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ValueError {
    pub kind: ValueKind,
    pub reason: ValueReason,
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {:?} value ({:?})", self.kind, self.reason)
    }
}

impl std::error::Error for ValueError {}

type Check = Result<(), ValueReason>;

/// Deserialises a JSON string through `parse`. Anything that is not a string fails with a fixed message,
/// and `parse` errors are mapped by the caller to texts without the input, so a failed decode never echoes
/// the value (FR-31). Shared with `provider.rs` for the constant markers.
pub(super) fn deserialize_parsed<'de, D, T, F>(d: D, parse: F) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: FnOnce(&str) -> Result<T, String>,
{
    struct StrVisitor<T, F>(F, PhantomData<fn() -> T>);

    impl<'de, T, F> Visitor<'de> for StrVisitor<T, F>
    where
        F: FnOnce(&str) -> Result<T, String>,
    {
        type Value = T;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<T, E> {
            (self.0)(v).map_err(E::custom)
        }

        fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<T, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<T, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<T, E> {
            Err(E::custom("expected a string"))
        }

        fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<T, E> {
            Err(E::custom("expected a string"))
        }
    }

    // `deserialize_any`, not `deserialize_str`: serde_json reports a wrongly typed number through
    // `invalid_type`, which quotes the number; with `deserialize_any` the visitor methods above run instead.
    d.deserialize_any(StrVisitor(parse, PhantomData))
}

macro_rules! string_newtype {
    ($(#[$doc:meta])* $name:ident, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Validates `s` and wraps it. The error never contains `s`.
            pub fn parse(s: &str) -> Result<Self, ValueError> {
                let check: fn(&str) -> Check = $check;
                match check(s) {
                    Ok(()) => Ok(Self(s.to_owned())),
                    Err(reason) => Err(ValueError {
                        kind: ValueKind::$name,
                        reason,
                    }),
                }
            }

            /// The validated text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserialize_parsed(deserializer, |s| Self::parse(s).map_err(|e| e.to_string()))
            }
        }
    };
}

// --- common rule ----------------------------------------------------------------------------------------

/// NUL, C0 (including TAB and LF), U+007F, C1 and every code point of the `domain::text` escape set.
fn forbidden_char(c: char) -> bool {
    matches!(
        c,
        '\u{0000}'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// Empty, over-long and forbidden-character checks shared by every type.
fn common(s: &str, max: usize) -> Check {
    if s.is_empty() {
        return Err(ValueReason::Empty);
    }
    if s.len() > max {
        return Err(ValueReason::TooLong);
    }
    if s.chars().any(forbidden_char) {
        return Err(ValueReason::Character);
    }
    Ok(())
}

/// `common` plus: every byte is printable ASCII (no space).
fn common_graphic(s: &str, max: usize) -> Check {
    common(s, max)?;
    if s.bytes().all(|b| b.is_ascii_graphic()) {
        Ok(())
    } else {
        Err(ValueReason::Character)
    }
}

// --- hashes ---------------------------------------------------------------------------------------------

fn lower_hex(s: &str, lengths: &[usize]) -> Check {
    common(s, 64)?;
    if !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(ValueReason::Character);
    }
    if lengths.contains(&s.len()) {
        Ok(())
    } else {
        Err(ValueReason::Syntax)
    }
}

string_newtype!(
    /// SHA-256 digest: 64 lowercase hex characters.
    Digest,
    |s| lower_hex(s, &[64])
);

string_newtype!(
    /// Git object ID: 40 (SHA-1) or 64 (SHA-256) lowercase hex characters.
    GitOid,
    |s| lower_hex(s, &[40, 64])
);

// --- paths ----------------------------------------------------------------------------------------------

const PATH_MAX: usize = 4096;

/// A segment of a relative or absolute path: not empty, not `.`, not `..`.
fn plain_segments(path: &str) -> Check {
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        Err(ValueReason::Syntax)
    } else {
        Ok(())
    }
}

/// Forward-slash relative path: no leading `/`, drive prefix, `\`, empty/`.`/`..` segment or trailing `/`.
fn rel_path(s: &str) -> Check {
    common(s, PATH_MAX)?;
    if s.contains('\\') {
        return Err(ValueReason::Character);
    }
    if s.starts_with('/') {
        return Err(ValueReason::Syntax);
    }
    let mut chars = s.chars();
    if let (Some(drive), Some(':')) = (chars.next(), chars.next())
        && drive.is_ascii_alphabetic()
    {
        return Err(ValueReason::Syntax);
    }
    plain_segments(s)
}

fn local_path(s: &str) -> Check {
    match s.strip_prefix('/') {
        Some(rest) => {
            common(s, PATH_MAX)?;
            if s.contains('\\') {
                return Err(ValueReason::Character);
            }
            plain_segments(rest)
        }
        None => rel_path(s),
    }
}

string_newtype!(
    /// Repository-relative path in forward-slash form (schema `RelativePath`).
    RelPath,
    rel_path
);

string_newtype!(
    /// Path relative to the app-data root (checkout directory, report files, retained artifacts).
    AppRelPath,
    rel_path
);

string_newtype!(
    /// Absolute (`/...`) or relative path without `..` segments; used only in `Config`.
    LocalPath,
    local_path
);

/// Whether `c` may appear anywhere in a path pattern (class contents included).
fn pattern_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '.' | '_' | ' ' | '-' | '*' | '?' | '[' | ']' | '!' | '^' | '/'
        )
}

/// One `/`-free segment of a pattern: `*`/`**`/`?` wildcards and non-nested, closed `[...]` classes.
fn pattern_segment(seg: &str) -> Check {
    let mut in_class = false;
    let mut class_len = 0usize;
    let mut negated = false;
    let mut stars = 0usize;
    for c in seg.chars() {
        if c != '*' {
            stars = 0;
        }
        if in_class {
            match c {
                ']' => {
                    if class_len == 0 {
                        return Err(ValueReason::Syntax);
                    }
                    in_class = false;
                }
                '[' | '*' | '?' => return Err(ValueReason::Syntax),
                '!' | '^' if class_len == 0 && !negated => negated = true,
                '!' | '^' => return Err(ValueReason::Syntax),
                _ => class_len += 1,
            }
        } else {
            match c {
                '[' => {
                    in_class = true;
                    class_len = 0;
                    negated = false;
                }
                ']' | '!' | '^' => return Err(ValueReason::Syntax),
                '*' => {
                    stars += 1;
                    if stars > 2 {
                        return Err(ValueReason::Syntax);
                    }
                }
                _ => {}
            }
        }
    }
    if in_class {
        Err(ValueReason::Syntax)
    } else {
        Ok(())
    }
}

fn path_pattern(s: &str) -> Check {
    common(s, 1024)?;
    if !s.chars().all(pattern_char) {
        return Err(ValueReason::Character);
    }
    if s.starts_with('/') {
        return Err(ValueReason::Syntax);
    }
    for seg in s.split('/') {
        if seg.is_empty() || seg == ".." {
            return Err(ValueReason::Syntax);
        }
        pattern_segment(seg)?;
    }
    Ok(())
}

string_newtype!(
    /// Glob subset for include/exclude patterns and schema bindings; never a shell expression.
    PathPattern,
    path_pattern
);

// --- git references and URLs ----------------------------------------------------------------------------

fn git_ref_name(s: &str) -> Check {
    common(s, 255)?;
    if s.chars()
        .any(|c| matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        return Err(ValueReason::Character);
    }
    if s == "@"
        || s.contains("..")
        || s.contains("@{")
        || s.starts_with('-')
        || s.ends_with('.')
        || s.ends_with('/')
        || s.ends_with(".lock")
    {
        return Err(ValueReason::Syntax);
    }
    if s.split('/')
        .any(|part| part.is_empty() || part.starts_with('.') || part.ends_with(".lock"))
    {
        return Err(ValueReason::Syntax);
    }
    Ok(())
}

string_newtype!(
    /// Branch, tag or ref name accepted by `git check-ref-format`.
    GitRefName,
    git_ref_name
);

const URL_MAX: usize = 2048;

/// DNS host name: dot-separated labels of `[A-Za-z0-9-]`, 1-63 bytes each, not starting or ending in `-`.
fn dns_name(host: &str) -> Check {
    if host.is_empty() || host.len() > 253 {
        return Err(ValueReason::Syntax);
    }
    for label in host.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(ValueReason::Syntax);
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(ValueReason::Character);
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(ValueReason::Syntax);
        }
    }
    Ok(())
}

fn ipv6_literal(addr: &str) -> Check {
    addr.parse::<Ipv6Addr>()
        .map(|_| ())
        .map_err(|_| ValueReason::Syntax)
}

/// Host without port: a DNS name or a bracketed IPv6 literal.
fn host_only(host: &str) -> Check {
    match host.strip_prefix('[') {
        Some(inner) => ipv6_literal(inner.strip_suffix(']').ok_or(ValueReason::Syntax)?),
        None => dns_name(host),
    }
}

fn port(p: &str) -> Check {
    if p.is_empty() || p.len() > 5 {
        return Err(ValueReason::Syntax);
    }
    if !p.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ValueReason::Character);
    }
    match p.parse::<u16>() {
        Ok(n) if n != 0 => Ok(()),
        _ => Err(ValueReason::Syntax),
    }
}

/// `host[:port]` where host is a DNS name or a bracketed IPv6 literal.
fn host_port(hp: &str) -> Check {
    if let Some(inner) = hp.strip_prefix('[') {
        let (addr, tail) = inner.split_once(']').ok_or(ValueReason::Syntax)?;
        ipv6_literal(addr)?;
        return match tail {
            "" => Ok(()),
            _ => port(tail.strip_prefix(':').ok_or(ValueReason::Syntax)?),
        };
    }
    match hp.split_once(':') {
        Some((host, p)) => {
            dns_name(host)?;
            port(p)
        }
        None => dns_name(hp),
    }
}

/// Path part of a URL (after the first `/`): not empty, no `\`, `%` only as a `%XX` escape.
fn url_path(p: &str) -> Check {
    if p.is_empty() {
        return Err(ValueReason::Syntax);
    }
    if p.contains('\\') {
        return Err(ValueReason::Character);
    }
    let b = p.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'%' {
            let hex = |off: usize| b.get(i + off).is_some_and(u8::is_ascii_hexdigit);
            if !(hex(1) && hex(2)) {
                return Err(ValueReason::Syntax);
            }
        }
    }
    Ok(())
}

/// User part of `user@host`. A `:` (password) or `%` (encoded userinfo) is a policy violation.
fn url_user(u: &str) -> Check {
    if u.contains(':') || u.contains('%') {
        return Err(ValueReason::Forbidden);
    }
    if u.is_empty() {
        return Err(ValueReason::Syntax);
    }
    if u.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(ValueReason::Character)
    }
}

fn no_query_or_fragment(s: &str) -> Check {
    if s.contains('?') || s.contains('#') {
        Err(ValueReason::Forbidden)
    } else {
        Ok(())
    }
}

fn remote_url(s: &str) -> Check {
    common_graphic(s, URL_MAX)?;
    no_query_or_fragment(s)?;
    if let Some(rest) = s.strip_prefix("https://") {
        let (authority, path) = rest.split_once('/').ok_or(ValueReason::Syntax)?;
        if authority.contains('@') {
            return Err(ValueReason::Forbidden);
        }
        host_port(authority)?;
        return url_path(path);
    }
    if let Some(rest) = s.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/').ok_or(ValueReason::Syntax)?;
        match authority.rsplit_once('@') {
            Some((user, hp)) => {
                url_user(user)?;
                host_port(hp)?;
            }
            None => host_port(authority)?,
        }
        return url_path(path);
    }
    if s.contains("://") {
        return Err(ValueReason::Syntax);
    }
    // scp-like: user@host:path
    let (user, rest) = s.split_once('@').ok_or(ValueReason::Syntax)?;
    url_user(user)?;
    let path = match rest.strip_prefix('[') {
        Some(inner) => {
            let (addr, tail) = inner.split_once(']').ok_or(ValueReason::Syntax)?;
            ipv6_literal(addr)?;
            tail.strip_prefix(':').ok_or(ValueReason::Syntax)?
        }
        None => {
            let (host, path) = rest.split_once(':').ok_or(ValueReason::Syntax)?;
            dns_name(host)?;
            path
        }
    };
    url_path(path)
}

string_newtype!(
    /// Credential-free remote: `https://host[:port]/path`, `ssh://[user@]host[:port]/path` or `user@host:path`.
    RemoteUrl,
    remote_url
);

fn canonical_remote(s: &str) -> Check {
    common_graphic(s, URL_MAX)?;
    no_query_or_fragment(s)?;
    if s.contains("://") {
        return Err(ValueReason::Forbidden);
    }
    let (host, path) = s.split_once('/').ok_or(ValueReason::Syntax)?;
    if host.contains('@') {
        return Err(ValueReason::Forbidden);
    }
    if host.starts_with('[') {
        host_only(host)?;
    } else {
        if host.contains(':') {
            return Err(ValueReason::Forbidden);
        }
        dns_name(host)?;
    }
    if host.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(ValueReason::Syntax);
    }
    url_path(path)
}

string_newtype!(
    /// `host/path` with a lowercase host and no scheme, userinfo, port, query or fragment.
    CanonicalRemote,
    canonical_remote
);

fn destination(s: &str) -> Check {
    common_graphic(s, URL_MAX)?;
    no_query_or_fragment(s)?;
    if let Some(rest) = s.strip_prefix("https://") {
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, Some(path)),
            None => (rest, None),
        };
        if authority.contains('@') {
            return Err(ValueReason::Forbidden);
        }
        host_port(authority)?;
        return path.filter(|p| !p.is_empty()).map_or(Ok(()), url_path);
    }
    if s.contains("://") || s.contains('/') {
        return Err(ValueReason::Syntax);
    }
    if s.contains('@') {
        return Err(ValueReason::Forbidden);
    }
    host_port(s)
}

string_newtype!(
    /// Network destination: `host[:port]` or an `https://` URL without userinfo, query or fragment.
    Destination,
    destination
);

// --- small syntaxes -------------------------------------------------------------------------------------

fn slug(s: &str) -> Check {
    common(s, 48)?;
    if !s
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    {
        return Err(ValueReason::Character);
    }
    if s.split('-').any(str::is_empty) {
        return Err(ValueReason::Syntax);
    }
    Ok(())
}

string_newtype!(
    /// Repository slug: lowercase alphanumeric groups separated by single hyphens.
    Slug,
    slug
);

fn local_time(s: &str) -> Check {
    common(s, 5)?;
    if !s.bytes().all(|b| b.is_ascii_digit() || b == b':') {
        return Err(ValueReason::Character);
    }
    let b = s.as_bytes();
    let digit = |i: usize| b.get(i).filter(|c| c.is_ascii_digit()).map(|c| c - b'0');
    match (digit(0), digit(1), b.get(2), digit(3), digit(4)) {
        (Some(h1), Some(h2), Some(b':'), Some(m1), Some(m2))
            if b.len() == 5 && h1 * 10 + h2 <= 23 && m1 * 10 + m2 <= 59 =>
        {
            Ok(())
        }
        _ => Err(ValueReason::Syntax),
    }
}

string_newtype!(
    /// Local wall-clock time `HH:MM`.
    LocalTime,
    local_time
);

fn iana_timezone(s: &str) -> Check {
    common(s, 64)?;
    if !s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-' | b'/'))
    {
        return Err(ValueReason::Character);
    }
    if s == "UTC" {
        return Ok(());
    }
    let parts = s.split('/').count();
    if (2..=3).contains(&parts) && s.split('/').all(|p| !p.is_empty()) {
        Ok(())
    } else {
        Err(ValueReason::Syntax)
    }
}

string_newtype!(
    /// IANA time-zone name by syntax only (`UTC` or `Area/Location[/Sub]`); tzdb membership is checked later.
    IanaTimezone,
    iana_timezone
);

// --- token family ---------------------------------------------------------------------------------------

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '@' | '+' | '-')
}

fn lower_token_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
}

fn spdx_char(c: char) -> bool {
    token_char(c) || matches!(c, ' ' | '(' | ')')
}

fn token(s: &str, max: usize, allowed: fn(char) -> bool) -> Check {
    common(s, max)?;
    if s.chars().all(allowed) {
        Ok(())
    } else {
        Err(ValueReason::Character)
    }
}

/// A credential reference is a locator, never key material (Data §11.23): no `sk-` prefix and no run of
/// 32 or more characters from `[A-Za-z0-9_-]`.
fn credential_ref(s: &str) -> Check {
    token(s, 256, token_char)?;
    if s.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("sk-")) {
        return Err(ValueReason::Forbidden);
    }
    let mut run = 0usize;
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            run += 1;
            if run >= 32 {
                return Err(ValueReason::Forbidden);
            }
        } else {
            run = 0;
        }
    }
    Ok(())
}

string_newtype!(
    /// Redaction rule-set version label.
    RedactionVersion,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// Adapter, tool, ruleset or renderer version label.
    VersionLabel,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// Tool name: lowercase `[a-z0-9._-]`.
    ToolName,
    |s| token(s, 128, lower_token_char)
);

string_newtype!(
    /// Framework identifier: lowercase `[a-z0-9._-]`.
    FrameworkId,
    |s| token(s, 128, lower_token_char)
);

string_newtype!(
    /// Renderer identifier.
    RendererId,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// Identifier of a bundled or user-supplied schema resource.
    SchemaResourceId,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// Model identifier (for example `vendor/model:variant`).
    ModelId,
    |s| token(s, 256, token_char)
);

string_newtype!(
    /// Provider identifier.
    ProviderId,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// Locator of a credential in the OS store or session; rejects anything that looks like a key.
    CredentialRef,
    credential_ref
);

string_newtype!(
    /// Policy or parser identifier.
    PolicyId,
    |s| token(s, 128, token_char)
);

string_newtype!(
    /// SPDX licence expression (token characters plus space and parentheses).
    SpdxExpression,
    |s| token(s, 256, spdx_char)
);

string_newtype!(
    /// Event code: lowercase `[a-z0-9._-]`.
    EventCode,
    |s| token(s, 128, lower_token_char)
);

string_newtype!(
    /// Generic machine identifier (theme, dialect, platform, native format, embedding key).
    Identifier,
    |s| token(s, 128, token_char)
);

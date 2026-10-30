//! Destination normalisation (C6 = 6a; `planning/P03-T01-request.md` §6). Owner: W1, T05.

use std::net::Ipv4Addr;
use std::str::FromStr;

use domain::store::values::{Destination, ValueReason};

/// Longest accepted destination input, in bytes.
pub const MAX_DESTINATION_BYTES: usize = 2048;

/// Accepted schemes; the scheme is always explicit in the input.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Scheme {
    /// `https`, default port 443.
    Https,
    /// `ssh`, default port 22.
    Ssh,
}

/// Why a destination was rejected. Fieldless: never echoes input.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DestinationError {
    /// Not of the form `scheme://host[:port][/path]`, or a bad label/path.
    Syntax,
    /// A scheme other than `https`/`ssh`, or none.
    Scheme,
    /// Userinfo (`@`) present.
    Userinfo,
    /// Non-ASCII host.
    NonAscii,
    /// An `xn--` label.
    Punycode,
    /// A `*` anywhere in the host.
    Wildcard,
    /// IPv6 or a non-canonical IPv4 form.
    IpLiteral,
    /// Port 0, over 65 535 or not decimal.
    Port,
    /// Query or fragment present.
    Query,
    /// Over [`MAX_DESTINATION_BYTES`].
    TooLong,
}

/// A destination in canonical form: lower-case ASCII host without a trailing dot, explicit port, optional
/// resource path.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NormalisedDestination {
    scheme: Scheme,
    host: String,
    port: u16,
    resource: Option<String>,
    /// The store form, validated once by the P02 `Destination` parser in [`NormalisedDestination::parse`],
    /// so that [`NormalisedDestination::to_store`] cannot fail and never bypasses that validation. It is a
    /// function of the four fields above, so the derived comparison and hash are unaffected by it.
    store: Destination,
}

impl Scheme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Self::Https => 443,
            Self::Ssh => 22,
        }
    }
}

/// Longest DNS host, in bytes, after the single trailing dot is removed.
const MAX_HOST_BYTES: usize = 253;
/// Longest DNS label, in bytes.
const MAX_LABEL_BYTES: usize = 63;
/// Most digits a port may be written with (leading zeros included).
const MAX_PORT_DIGITS: usize = 5;

impl NormalisedDestination {
    /// Parses and normalises `input` (rules: request §6 "Destination normalisation").
    ///
    /// Accepted shape: `scheme://host[:port][/path]` with scheme `https` or `ssh` (ASCII case-insensitive,
    /// stored lower-case). The host is lower-cased and exactly one trailing dot is removed; the default
    /// port (443, 22) becomes explicit. A bare `/` resource is the same resource as none and is dropped.
    ///
    /// Rejections map to the most specific [`DestinationError`]. When an input has several faults the
    /// first check in this order decides (the order is part of the contract, pinned by tests):
    ///
    /// 1. input over [`MAX_DESTINATION_BYTES`] -> `TooLong`;
    /// 2. no `://` or a scheme other than `https`/`ssh` -> `Scheme`;
    /// 3. `@` in the authority -> `Userinfo`;
    /// 4. `?` or `#` anywhere after the scheme -> `Query`;
    /// 5. empty authority -> `Syntax`; a non-ASCII byte in the authority -> `NonAscii`; an ASCII space or
    ///    control character in the authority -> `Syntax`;
    /// 6. `*` in the authority -> `Wildcard`;
    /// 7. a leading `[` (IPv6 literal) or more than one `:` (unbracketed IPv6) -> `IpLiteral`;
    /// 8. host faults: an `xn--` label -> `Punycode`; an empty host, an empty label (so also a second
    ///    trailing dot), a label over 63 bytes, a host over 253 bytes, a character outside `[a-z0-9-]` or a
    ///    label edge `-` -> `Syntax`; a numeric last label (all digits or `0x...`) that is not a canonical
    ///    dotted-quad IPv4 address -> `IpLiteral`;
    /// 9. an empty, non-decimal, zero or over-65 535 port, or one written with over five digits -> `Port`;
    /// 10. resource path faults (non-graphic or non-ASCII byte, `\`, a `%` that is not a `%XX` escape or that
    ///     decodes to a control character, `/`, `\`, `?`, `#` or `@`) -> `Syntax`;
    /// 11. a store form the P02 `Destination` parser rejects -> `TooLong` (length) or `Syntax`.
    pub fn parse(input: &str) -> Result<Self, DestinationError> {
        if input.len() > MAX_DESTINATION_BYTES {
            return Err(DestinationError::TooLong);
        }
        let (scheme_text, rest) = input.split_once("://").ok_or(DestinationError::Scheme)?;
        let scheme = if scheme_text.eq_ignore_ascii_case("https") {
            Scheme::Https
        } else if scheme_text.eq_ignore_ascii_case("ssh") {
            Scheme::Ssh
        } else {
            return Err(DestinationError::Scheme);
        };

        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at(authority_end);
        if authority.contains('@') {
            return Err(DestinationError::Userinfo);
        }
        if rest.contains(['?', '#']) {
            return Err(DestinationError::Query);
        }
        let (host_text, port_text) = split_authority(authority)?;
        let host = normalise_host(host_text)?;
        let port = match port_text {
            Some(text) => parse_port(text)?,
            None => scheme.default_port(),
        };
        let resource = parse_resource(tail)?;

        let store_text = match scheme {
            Scheme::Https => format!(
                "https://{host}:{port}{}",
                resource.as_deref().unwrap_or_default()
            ),
            Scheme::Ssh => format!("{host}:{port}"),
        };
        let store = Destination::parse(&store_text).map_err(|e| match e.reason {
            ValueReason::TooLong => DestinationError::TooLong,
            _ => DestinationError::Syntax,
        })?;
        Ok(Self {
            scheme,
            host,
            port,
            resource,
            store,
        })
    }

    /// The scheme.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The lower-case host.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The explicit port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The resource path (starting with `/`), if any.
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    /// `scheme://host:port[/path]`: the form hashed into the fingerprint.
    pub fn canonical(&self) -> String {
        format!(
            "{}://{}:{}{}",
            self.scheme.as_str(),
            self.host,
            self.port,
            self.resource.as_deref().unwrap_or_default()
        )
    }

    /// The store form: `https://host:port[/path]`, or `host:port` for `ssh` (the store grammar has no
    /// `ssh://`, so an `ssh` resource path is not part of the store form; the fingerprint keeps it).
    pub fn to_store(&self) -> Destination {
        self.store.clone()
    }
}

/// Splits the authority into host text and an optional port text (checks 5 to 7 of `parse`).
fn split_authority(authority: &str) -> Result<(&str, Option<&str>), DestinationError> {
    if authority.is_empty() {
        return Err(DestinationError::Syntax);
    }
    if !authority.is_ascii() {
        return Err(DestinationError::NonAscii);
    }
    if authority.bytes().any(|b| !b.is_ascii_graphic()) {
        return Err(DestinationError::Syntax);
    }
    if authority.contains('*') {
        return Err(DestinationError::Wildcard);
    }
    if authority.starts_with('[') || authority.matches(':').count() > 1 {
        return Err(DestinationError::IpLiteral);
    }
    Ok(match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    })
}

/// Lower-cases the host, removes exactly one trailing dot and validates the labels (check 8 of `parse`).
fn normalise_host(host_text: &str) -> Result<String, DestinationError> {
    let without_dot = host_text.strip_suffix('.').unwrap_or(host_text);
    let host = without_dot.to_ascii_lowercase();
    if host.is_empty() {
        return Err(DestinationError::Syntax);
    }
    if host.split('.').any(|label| label.starts_with("xn--")) {
        return Err(DestinationError::Punycode);
    }
    if host.len() > MAX_HOST_BYTES {
        return Err(DestinationError::Syntax);
    }
    for label in host.split('.') {
        let well_formed = !label.is_empty()
            && label.len() <= MAX_LABEL_BYTES
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !well_formed {
            return Err(DestinationError::Syntax);
        }
    }
    let last = host.rsplit('.').next().unwrap_or(&host);
    let numeric = last.bytes().all(|b| b.is_ascii_digit()) || last.starts_with("0x");
    if numeric {
        // Only the canonical dotted quad survives: `Ipv4Addr` rejects leading zeros, short forms, hex and
        // single-integer forms, and the `to_string` round trip leaves no other spelling.
        return match Ipv4Addr::from_str(&host) {
            Ok(addr) if addr.to_string() == host => Ok(host),
            _ => Err(DestinationError::IpLiteral),
        };
    }
    Ok(host)
}

/// Parses a decimal port in 1..=65535 (check 9 of `parse`).
fn parse_port(text: &str) -> Result<u16, DestinationError> {
    if text.is_empty() || text.len() > MAX_PORT_DIGITS || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(DestinationError::Port);
    }
    match text.parse::<u16>() {
        Ok(port) if port != 0 => Ok(port),
        _ => Err(DestinationError::Port),
    }
}

/// Validates the resource path (`tail` is empty or starts with `/`; check 10 of `parse`). A bare `/` is
/// dropped. The rules after the leading `/` are those of the P02 `url_path` check.
fn parse_resource(tail: &str) -> Result<Option<String>, DestinationError> {
    let Some(path) = tail.strip_prefix('/') else {
        return Ok(None);
    };
    if path.is_empty() {
        return Ok(None);
    }
    if path.bytes().any(|b| !b.is_ascii_graphic()) || path.contains('\\') {
        return Err(DestinationError::Syntax);
    }
    let bytes = path.as_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != b'%' {
            continue;
        }
        let nibble = |offset: usize| {
            bytes
                .get(i + offset)
                .and_then(|d| char::from(*d).to_digit(16))
                .and_then(|d| u8::try_from(d).ok())
        };
        let (Some(hi), Some(lo)) = (nibble(1), nibble(2)) else {
            return Err(DestinationError::Syntax);
        };
        let decoded = (hi << 4) | lo;
        if decoded < 0x20 || decoded == 0x7F || matches!(decoded, b'/' | b'\\' | b'?' | b'#' | b'@')
        {
            return Err(DestinationError::Syntax);
        }
    }
    Ok(Some(tail.to_owned()))
}

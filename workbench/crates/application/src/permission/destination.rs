//! Destination normalisation (C6 = 6a; `planning/P03-T01-request.md` §6). Owner: W1, T05.

use domain::store::values::Destination;

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
}

impl NormalisedDestination {
    /// Parses and normalises `input` (rules: request §6 "Destination normalisation").
    pub fn parse(input: &str) -> Result<Self, DestinationError> {
        let _ = input;
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T05")
        }
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
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T05")
        }
    }

    /// The store form: `https://host:port[/path]`, or `host:port` for `ssh` (the store grammar has no
    /// `ssh://`).
    pub fn to_store(&self) -> Destination {
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T05")
        }
    }
}

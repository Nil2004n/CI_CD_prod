//! Remote identity seam. Parsing is deliberately unavailable in this scaffold.

use domain::store::values::{CanonicalRemote, RemoteUrl, Slug};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteScheme {
    Https,
    Ssh,
}

pub struct DestinationParts<'a> {
    pub scheme: RemoteScheme,
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
}

pub struct RemoteSpec {
    remote: RemoteUrl,
    canonical: CanonicalRemote,
    slug: Slug,
    scheme: RemoteScheme,
    host: String,
    port: u16,
    path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteError {
    Empty,
    TooLong,
    Syntax,
    Scheme,
    Credentials,
    Host,
    InternalHost,
    Path,
    Unsupported,
}

impl RemoteSpec {
    pub fn parse(_input: &str) -> Result<Self, RemoteError> {
        Err(RemoteError::Unsupported)
    }

    pub fn remote_url(&self) -> &RemoteUrl {
        &self.remote
    }
    pub fn canonical_remote(&self) -> &CanonicalRemote {
        &self.canonical
    }
    pub fn slug(&self) -> &Slug {
        &self.slug
    }
    pub fn destination_parts(&self) -> DestinationParts<'_> {
        DestinationParts {
            scheme: self.scheme,
            host: &self.host,
            port: self.port,
            path: &self.path,
        }
    }
}

masked_debug!(DestinationParts<'_>, RemoteSpec);

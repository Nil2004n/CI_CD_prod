//! Validated remote identities. Host checks are lexical and do not resolve DNS.

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
    pub fn parse(input: &str) -> Result<Self, RemoteError> {
        if input.is_empty() {
            return Err(RemoteError::Empty);
        }
        if input.len() > 2048 {
            return Err(RemoteError::TooLong);
        }
        if input.starts_with('-')
            || !input.bytes().all(|b| b.is_ascii_graphic())
            || input
                .bytes()
                .any(|b| matches!(b, b'?' | b'#' | b'%' | b'\\'))
        {
            return Err(RemoteError::Syntax);
        }
        let (scheme, user, authority, path, scp) =
            if let Some(rest) = input.strip_prefix("https://") {
                let (authority, path) = rest.split_once('/').ok_or(RemoteError::Syntax)?;
                if authority.contains('@') {
                    return Err(RemoteError::Credentials);
                }
                (RemoteScheme::Https, None, authority, path, false)
            } else if let Some(rest) = input.strip_prefix("ssh://") {
                let (authority, path) = rest.split_once('/').ok_or(RemoteError::Syntax)?;
                let (user, host) = match authority.split_once('@') {
                    Some((u, h)) => (Some(u), h),
                    None => (None, authority),
                };
                (RemoteScheme::Ssh, user, host, path, false)
            } else {
                if input.contains("://") {
                    return Err(RemoteError::Scheme);
                }
                let (user, rest) = input.split_once('@').ok_or(RemoteError::Syntax)?;
                let (host, path) = rest.split_once(':').ok_or(RemoteError::Syntax)?;
                (RemoteScheme::Ssh, Some(user), host, path, true)
            };
        if let Some(u) = user
            && (u.is_empty()
                || u.len() > 32
                || u.starts_with('-')
                || !u
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')))
        {
            return Err(RemoteError::Credentials);
        }
        if path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.starts_with('-')
                || part.contains(['@', ':'])
        }) {
            return Err(RemoteError::Path);
        }
        let (raw_host, port) = match authority.split_once(':') {
            Some((h, p)) if !scp => {
                if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(RemoteError::Host);
                }
                let n = p.parse::<u16>().map_err(|_| RemoteError::Host)?;
                if n == 0 {
                    return Err(RemoteError::Host);
                }
                (h, n)
            }
            Some(_) => return Err(RemoteError::Host),
            None => (
                authority,
                if scheme == RemoteScheme::Https {
                    443
                } else {
                    22
                },
            ),
        };
        let host = raw_host
            .strip_suffix('.')
            .unwrap_or(raw_host)
            .to_ascii_lowercase();
        validate_host(&host)?;
        let authority = match user {
            Some(u) => format!("{u}@{host}"),
            None => host.clone(),
        };
        let default = if scheme == RemoteScheme::Https {
            443
        } else {
            22
        };
        let port_suffix = if port == default {
            String::new()
        } else {
            format!(":{port}")
        };
        let normalized = if scp {
            format!("{authority}:{path}")
        } else {
            let prefix = if scheme == RemoteScheme::Https {
                "https"
            } else {
                "ssh"
            };
            format!("{prefix}://{authority}{port_suffix}/{path}")
        };
        let remote = RemoteUrl::parse(&normalized).map_err(|_| RemoteError::Syntax)?;
        let canonical =
            CanonicalRemote::parse(&format!("{host}/{path}")).map_err(|_| RemoteError::Syntax)?;
        let mut slug = String::new();
        let mut separator = false;
        for b in path.rsplit('/').next().unwrap_or("").bytes() {
            if b.is_ascii_alphanumeric() {
                if separator && !slug.is_empty() && slug.len() < 47 {
                    slug.push('-');
                }
                separator = false;
                if slug.len() < 48 {
                    slug.push(char::from(b.to_ascii_lowercase()));
                }
            } else {
                separator = true;
            }
        }
        let slug = Slug::parse(&slug).map_err(|_| RemoteError::Path)?;
        Ok(Self {
            remote,
            canonical,
            slug,
            scheme,
            host,
            port,
            path: path.to_owned(),
        })
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

fn validate_host(host: &str) -> Result<(), RemoteError> {
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || label.starts_with("xn--")
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return Err(RemoteError::Host);
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(RemoteError::InternalHost);
    }
    let last = host.rsplit('.').next().unwrap_or(host);
    if last.bytes().all(|b| b.is_ascii_digit()) || last.starts_with("0x") {
        let ip = host
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| RemoteError::Host)?;
        if ip.to_string() != host {
            return Err(RemoteError::Host);
        }
        let [a, b, _, _] = ip.octets();
        if a == 0
            || a == 10
            || a == 127
            || a >= 224
            || (a == 169 && b == 254)
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
        {
            return Err(RemoteError::InternalHost);
        }
    }
    Ok(())
}

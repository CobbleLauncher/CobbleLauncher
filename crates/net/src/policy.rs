// SPDX-License-Identifier: MIT
//! Conservative public destination policy, based on IANA special-purpose
//! registries inspected 2026-10-21. Globally reachable special-purpose ranges
//! are rejected too. This is deliberately narrower than global IP routing.

use crate::NetError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use url::{Host, Url};

#[derive(Clone, Eq, PartialEq, Hash)]
pub(crate) struct Origin(pub String);

#[derive(Clone)]
pub(crate) struct Endpoint {
    pub url: Url,
    pub credential_path_safe: bool,
}

impl Endpoint {
    pub fn parse(raw: &str) -> Result<Self, NetError> {
        check_raw(raw)?;
        if raw
            .split_once("://")
            .and_then(|(_, tail)| tail.split(['/', '?', '#']).next())
            .is_some_and(|authority| authority.contains('@'))
        {
            return Err(NetError::InvalidUrl);
        }
        let url = Url::parse(raw).map_err(|_| NetError::InvalidUrl)?;
        if url.as_str().len() > 4096 {
            return Err(NetError::InvalidUrl);
        }
        check_url(&url)?;
        // WHATWG parsing accepts abbreviated/octal/hex IPv4. Refuse those even
        // if they happen to normalize to public addresses.
        if let Some(Host::Ipv4(ip)) = url.host() {
            let authority = raw
                .split_once("://")
                .ok_or(NetError::InvalidUrl)?
                .1
                .split(['/', '?', '#'])
                .next()
                .ok_or(NetError::InvalidUrl)?;
            let name = authority.split(':').next().ok_or(NetError::InvalidUrl)?;
            if name != ip.to_string() {
                return Err(NetError::InvalidUrl);
            }
        }
        let safe = raw_path_safe(raw) && path_safe(url.path());
        Ok(Self {
            url,
            credential_path_safe: safe,
        })
    }
    pub fn redirect(&self, location: &str) -> Result<Self, NetError> {
        check_raw(location)?;
        if location.starts_with("//") {
            return Self::parse(&format!("https:{location}"));
        }
        if Url::parse(location).is_ok() {
            return Self::parse(location);
        }
        let url = self.url.join(location).map_err(|_| NetError::InvalidUrl)?;
        let mut endpoint = Self::parse(url.as_str())?;
        endpoint.credential_path_safe &=
            raw_path_safe(location) && (!location.starts_with('?') || self.credential_path_safe);
        Ok(endpoint)
    }
    pub fn origin(&self) -> Origin {
        Origin(self.url.origin().ascii_serialization())
    }
    pub fn host(&self) -> String {
        match self.url.host().expect("validated host") {
            Host::Domain(domain) => domain.to_owned(),
            Host::Ipv4(ip) => ip.to_string(),
            Host::Ipv6(ip) => ip.to_string(),
        }
    }
    pub fn host_header(&self) -> String {
        match self.url.host().expect("validated host") {
            Host::Ipv6(ip) => format!("[{ip}]"),
            _ => self.host(),
        }
    }
    pub fn request_target(&self) -> String {
        let mut target = self.url.path().to_owned();
        if let Some(query) = self.url.query() {
            target.push('?');
            target.push_str(query);
        }
        target
    }
}

fn check_raw(raw: &str) -> Result<(), NetError> {
    if raw.is_empty()
        || raw.len() > 4096
        || raw
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err(NetError::InvalidUrl);
    }
    Ok(())
}
fn check_url(url: &Url) -> Result<(), NetError> {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(NetError::InvalidUrl);
    }
    match url.host().ok_or(NetError::InvalidUrl)? {
        Host::Domain(name) => {
            if !name.contains('.')
                || name.ends_with('.')
                || name == "localhost"
                || [".localhost", ".local", ".internal", ".home.arpa"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
            {
                return Err(NetError::ForbiddenDestination);
            }
        }
        Host::Ipv4(ip) if !public_ip(IpAddr::V4(ip)) => return Err(NetError::ForbiddenDestination),
        Host::Ipv6(ip) if !public_ip(IpAddr::V6(ip)) => return Err(NetError::ForbiddenDestination),
        _ => (),
    }
    Ok(())
}

pub(crate) fn path_safe(path: &str) -> bool {
    path.starts_with('/')
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte))
        && !path.contains("//")
        && !path.split('/').any(|part| part == "." || part == "..")
}
fn raw_path_safe(raw: &str) -> bool {
    let relative = if let Some((_, authority)) = raw.split_once("://") {
        authority.find('/').map_or("/", |at| &authority[at..])
    } else {
        raw
    };
    let path = relative.split(['?', '#']).next().unwrap_or("");
    (path.is_empty()
        || path_safe(path)
        || (!path.starts_with('/') && path_safe(&format!("/{path}"))))
        && !path.contains('%')
}

pub(crate) fn validate_addresses(addresses: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, NetError> {
    if addresses.is_empty() {
        return Err(NetError::Resolution);
    }
    if addresses.len() > 32 {
        return Err(NetError::ResolutionLimit);
    }
    if addresses
        .iter()
        .any(|address| address.port() != 443 || !public_ip(address.ip()))
    {
        return Err(NetError::ForbiddenDestination);
    }
    Ok(addresses)
}

pub(crate) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => public_v6(ip),
    }
}
fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192
            && ((b == 0 && (c == 0 || c == 2))
                || (b == 31 && c == 196)
                || (b == 52 && c == 193)
                || (b == 88 && c == 99)
                || b == 168
                || (b == 175 && c == 48)))
        || (a == 198 && ((18..20).contains(&b) || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}
fn public_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    (s[0] & 0xe000 == 0x2000)
        && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
        && s[0] != 0x2002
        && !(s[0] == 0x2620 && s[1] == 0x004f && s[2] == 0x8000)
        && !(s[0] == 0x3fff && s[1] & 0xf000 == 0)
        && !((s[4] == 0 || s[4] == 0x0200) && s[5] == 0x5efe)
}

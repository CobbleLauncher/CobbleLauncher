// SPDX-License-Identifier: MIT
use crate::NetError;
use crate::policy::{Endpoint, Origin, path_safe};
use http::header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue};

/// Opaque process-local credential reference. No secrets are returned by it.
#[derive(Clone)]
pub struct CredentialId {
    pub(crate) index: usize,
    pub(crate) identity: std::sync::Arc<()>,
}

/// Trusted Core registration only. No arbitrary header names, and deliberately
/// no Debug implementation. This is not a renderer/plugin credential API.
pub struct ProviderCredentials {
    pub bearer: Option<String>,
    pub api_key: Option<String>,
    pub cookie: Option<String>,
}

pub struct CredentialScope {
    origin: Origin,
    prefix: String,
}
impl CredentialScope {
    pub fn new(origin: &str, path_prefix: &str) -> Result<Self, NetError> {
        let endpoint = Endpoint::parse(origin)?;
        if path_prefix.len() > 1024
            || endpoint.url.path() != "/"
            || endpoint.url.query().is_some()
            || !path_safe(path_prefix)
        {
            return Err(NetError::CredentialScope);
        }
        let prefix = if path_prefix == "/" {
            "/".to_owned()
        } else {
            path_prefix.trim_end_matches('/').to_owned()
        };
        Ok(Self {
            origin: endpoint.origin(),
            prefix,
        })
    }
    fn matches(&self, endpoint: &Endpoint) -> bool {
        let path = endpoint.url.path();
        self.origin == endpoint.origin()
            && endpoint.credential_path_safe
            && path_safe(path)
            && (self.prefix == "/"
                || path == self.prefix
                || path
                    .strip_prefix(&self.prefix)
                    .is_some_and(|tail| tail.starts_with('/')))
    }
}

pub(crate) struct CredentialEntry {
    scope: CredentialScope,
    headers: HeaderMap,
}
impl CredentialEntry {
    pub fn new(scope: CredentialScope, secrets: ProviderCredentials) -> Result<Self, NetError> {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            (
                AUTHORIZATION,
                secrets.bearer.map(|value| format!("Bearer {value}")),
            ),
            (
                http::header::HeaderName::from_static("x-api-key"),
                secrets.api_key,
            ),
            (COOKIE, secrets.cookie),
        ] {
            if let Some(value) = value {
                if value.len() > 4096 {
                    return Err(NetError::CredentialScope);
                }
                let mut value =
                    HeaderValue::from_str(&value).map_err(|_| NetError::CredentialScope)?;
                value.set_sensitive(true);
                headers.insert(name, value);
            }
        }
        if headers.is_empty() {
            return Err(NetError::CredentialScope);
        }
        Ok(Self { scope, headers })
    }
    pub fn attach(&self, endpoint: &Endpoint, headers: &mut HeaderMap) {
        if self.scope.matches(endpoint) {
            headers.extend(self.headers.clone());
        }
    }
}

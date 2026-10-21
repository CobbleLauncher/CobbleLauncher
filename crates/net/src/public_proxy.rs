// SPDX-License-Identifier: MIT
//! Narrow, anonymous fallback for public loader downloads through the user's
//! existing proxy. Call only after the normal broker's connection/TLS failure;
//! this does not grant provider permissions or change OS proxy configuration.

use crate::{NetError, TimeoutPhase};
use curl::easy::{Easy, NetRc, SslVersion};
use std::time::Duration;
use url::Url;

pub async fn fetch_loader_via_system_proxy(url: &str, limit: u64) -> Result<Vec<u8>, NetError> {
    let url = public_loader_url(url)?;
    if limit > 64 * 1024 * 1024 {
        return Err(NetError::BodyLimit);
    }
    tokio::task::spawn_blocking(move || fetch(&url, limit))
        .await
        .map_err(|_| NetError::Connect)?
}

fn public_loader_url(value: &str) -> Result<Url, NetError> {
    let url = Url::parse(value).map_err(|_| NetError::InvalidUrl)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || !matches!(
            url.host_str(),
            Some("maven.neoforged.net" | "maven.minecraftforge.net" | "files.minecraftforge.net")
        )
    {
        return Err(NetError::ForbiddenDestination);
    }
    Ok(url)
}

fn existing_proxy() -> Result<String, NetError> {
    let configured = ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    let value = if let Some(value) = configured {
        value
    } else {
        let proxy = sysproxy::Sysproxy::get_system_proxy().map_err(|_| NetError::Connect)?;
        if !proxy.enable || proxy.host.is_empty() || proxy.port == 0 {
            return Err(NetError::Connect);
        }
        let mut url = Url::parse("http://localhost").map_err(|_| NetError::Connect)?;
        url.set_host(Some(&proxy.host))
            .map_err(|_| NetError::Connect)?;
        url.set_port(Some(proxy.port))
            .map_err(|_| NetError::Connect)?;
        url.to_string()
    };
    let proxy = Url::parse(&value).map_err(|_| NetError::Connect)?;
    if !matches!(proxy.scheme(), "http" | "https" | "socks5" | "socks5h")
        || proxy.host_str().is_none()
        || !proxy.username().is_empty()
        || proxy.password().is_some()
        || proxy.query().is_some()
        || proxy.fragment().is_some()
    {
        return Err(NetError::Connect);
    }
    Ok(proxy.to_string())
}

fn append_body(body: &mut Vec<u8>, data: &[u8], limit: u64) -> bool {
    if (body.len() as u64).saturating_add(data.len() as u64) > limit {
        return false;
    }
    body.extend_from_slice(data);
    true
}

fn fetch(url: &Url, limit: u64) -> Result<Vec<u8>, NetError> {
    let proxy = existing_proxy()?;
    let mut easy = Easy::new();
    let set = |_: curl::Error| NetError::Protocol;
    easy.url(url.as_str()).map_err(set)?;
    easy.proxy(&proxy).map_err(set)?;
    easy.noproxy("").map_err(set)?;
    easy.netrc(NetRc::Ignored).map_err(set)?;
    easy.follow_location(false).map_err(set)?;
    easy.ssl_verify_peer(true).map_err(set)?;
    easy.ssl_verify_host(true).map_err(set)?;
    easy.proxy_ssl_verify_peer(true).map_err(set)?;
    easy.proxy_ssl_verify_host(true).map_err(set)?;
    easy.ssl_version(SslVersion::Tlsv12).map_err(set)?;
    easy.connect_timeout(Duration::from_secs(15)).map_err(set)?;
    easy.timeout(Duration::from_secs(120)).map_err(set)?;
    easy.useragent("CobbleLauncher/0.1").map_err(set)?;
    let mut body = Vec::new();
    let mut exceeded = false;
    let outcome = {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                if append_body(&mut body, data, limit) {
                    Ok(data.len())
                } else {
                    exceeded = true;
                    Ok(0)
                }
            })
            .map_err(set)?;
        transfer.perform()
    };
    if exceeded {
        return Err(NetError::BodyLimit);
    }
    outcome.map_err(|error| {
        if error.is_operation_timedout() {
            NetError::Timeout(TimeoutPhase::Total)
        } else if error.is_couldnt_connect() {
            NetError::Connect
        } else if error.is_peer_failed_verification() || error.is_ssl_connect_error() {
            NetError::Tls
        } else {
            NetError::Protocol
        }
    })?;
    if easy.response_code().map_err(set)? != 200 {
        return Err(NetError::Protocol);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_loader_whitelist_is_exact() {
        for host in [
            "maven.neoforged.net",
            "maven.minecraftforge.net",
            "files.minecraftforge.net",
        ] {
            assert!(public_loader_url(&format!("https://{host}/file.jar.sha1")).is_ok());
        }
        for url in [
            "http://maven.neoforged.net/file",
            "https://maven.neoforged.net.evil.invalid/file",
            "https://user:secret@maven.neoforged.net/file",
            "https://127.0.0.1/file",
            "https://github.com/file",
            "https://maven.neoforged.net:8443/file",
        ] {
            assert!(public_loader_url(url).is_err());
        }
    }

    #[test]
    fn callback_cap_is_checked_before_extending() {
        let mut body = Vec::new();
        assert!(append_body(&mut body, b"ab", 3));
        assert!(append_body(&mut body, b"c", 3));
        assert!(!append_body(&mut body, b"d", 3));
        assert_eq!(body, b"abc");
        assert!(!append_body(&mut Vec::new(), b"a", 0));
    }
}

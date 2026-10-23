// SPDX-License-Identifier: MIT
//! Ephemeral direct-connect argv; never persisted into runtime preferences.
use std::net::{Ipv6Addr, SocketAddrV6};

pub fn supports_direct_connect(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    (parts.len() == 2 || parts.len() == 3)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && parts[0] == "1"
        && parts[1]
            .parse::<u32>()
            .is_ok_and(|minor| (20..=21).contains(&minor))
}
fn address_valid(address: &str) -> bool {
    if address.is_empty()
        || address.len() > 255
        || !address.is_ascii()
        || address
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return false;
    }
    if address.starts_with('[') {
        return address
            .parse::<SocketAddrV6>()
            .is_ok_and(|address| address.port() != 0)
            || address
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .is_some_and(|value| value.parse::<Ipv6Addr>().is_ok());
    }
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (address, None),
    };
    if port.is_some_and(|port| port.parse::<u16>().map_or(true, |port| port == 0)) {
        return false;
    }
    let host = host.strip_suffix('.').unwrap_or(host);
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}
pub fn direct_connect_arguments(version: &str, address: &str) -> Result<Vec<String>, String> {
    if !supports_direct_connect(version) {
        return Err("此版本暂不支持直接加入服务器，请复制地址后在游戏中连接".into());
    }
    if !address_valid(address) {
        return Err("服务器地址无效，请检查主机名与端口".into());
    }
    Ok(vec!["--quickPlayMultiplayer".into(), address.into()])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_gating_and_endpoint_validation_preserve_separate_argv() {
        for version in ["1.20", "1.20.6", "1.21.1"] {
            for address in [
                "localhost",
                "play.example.org:25565",
                "127.0.0.1:42",
                "[::1]",
                "[2001:db8::1]:25565",
            ] {
                assert_eq!(
                    direct_connect_arguments(version, address).unwrap(),
                    ["--quickPlayMultiplayer", address]
                );
            }
        }
        for version in ["1.19.4", "23w14a", "1.21-pre1", "1.22", "26.1", "custom"] {
            assert!(direct_connect_arguments(version, "localhost").is_err());
        }
        for address in [
            "",
            "--demo",
            "host:0",
            "host:65536",
            "http://host",
            "host/path",
            "a b",
            "host\n--demo",
            "user@host",
            "::1",
            "-host",
            "host:",
        ] {
            assert!(
                direct_connect_arguments("1.21.1", address).is_err(),
                "{address}"
            );
        }
    }
}

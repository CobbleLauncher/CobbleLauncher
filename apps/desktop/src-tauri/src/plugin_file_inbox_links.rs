// SPDX-License-Identifier: MIT
//! Recognize provider page references. This module never fetches or downloads a URL.
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinkRequest {
    pub url: String,
    pub game_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketLink {
    pub provider: &'static str,
    pub kind: &'static str,
    pub project: String,
    pub version: Option<String>,
}

fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

pub fn parse(value: &str) -> Result<MarketLink, String> {
    if value.len() > 2048 || value.trim() != value {
        return Err("FILE_INBOX_LINK_UNSUPPORTED".into());
    }
    let url = Url::parse(value).map_err(|_| "FILE_INBOX_LINK_UNSUPPORTED")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err("FILE_INBOX_LINK_UNSUPPORTED".into());
    }
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    let (provider, kind, project, version) = match (url.host_str(), parts.as_slice()) {
        (Some("modrinth.com" | "www.modrinth.com"), [category, project]) => {
            ("modrinth", modrinth_kind(category)?, *project, None)
        }
        (Some("modrinth.com" | "www.modrinth.com"), [category, project, "version", version]) => (
            "modrinth",
            modrinth_kind(category)?,
            *project,
            Some(*version),
        ),
        (Some("curseforge.com" | "www.curseforge.com"), ["minecraft", category, project]) => {
            ("curseforge", curseforge_kind(category)?, *project, None)
        }
        (
            Some("curseforge.com" | "www.curseforge.com"),
            ["minecraft", category, project, "files", version],
        ) => (
            "curseforge",
            curseforge_kind(category)?,
            *project,
            Some(*version),
        ),
        _ => return Err("FILE_INBOX_LINK_UNSUPPORTED".into()),
    };
    if !segment(project)
        || version.is_some_and(|v| !segment(v))
        || provider == "curseforge"
            && version.is_some_and(|v| !v.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("FILE_INBOX_LINK_UNSUPPORTED".into());
    }
    Ok(MarketLink {
        provider,
        kind,
        project: project.into(),
        version: version.map(str::to_owned),
    })
}

fn modrinth_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "mod" => Ok("mod"),
        "modpack" => Ok("modpack"),
        "resourcepack" => Ok("resource_pack"),
        "shader" => Ok("shader_pack"),
        _ => Err("FILE_INBOX_LINK_UNSUPPORTED".into()),
    }
}
fn curseforge_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "mc-mods" => Ok("mod"),
        "modpacks" => Ok("modpack"),
        _ => Err("FILE_INBOX_LINK_UNSUPPORTED".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_links_preserve_project_and_version() {
        assert_eq!(
            parse("https://modrinth.com/mod/sodium/version/abc-1").unwrap(),
            MarketLink {
                provider: "modrinth",
                kind: "mod",
                project: "sodium".into(),
                version: Some("abc-1".into())
            }
        );
        assert_eq!(
            parse("https://www.curseforge.com/minecraft/modpacks/a-pack/files/12345").unwrap(),
            MarketLink {
                provider: "curseforge",
                kind: "modpack",
                project: "a-pack".into(),
                version: Some("12345".into())
            }
        );
        assert_eq!(
            parse("https://modrinth.com/resourcepack/fresh")
                .unwrap()
                .kind,
            "resource_pack"
        );
    }
    #[test]
    fn unrelated_or_ambiguous_urls_are_rejected() {
        for value in [
            "http://modrinth.com/mod/sodium",
            "https://modrinth.com.evil.test/mod/sodium",
            "https://u@modrinth.com/mod/sodium",
            "https://modrinth.com/mod/sodium?download=1",
            "https://modrinth.com/mod/%2e%2e",
            "https://curseforge.com/minecraft/mc-mods/foo/files/latest",
            "https://curseforge.com/minecraft/modpacks/foo/download",
            "https://example.com/a.jar",
        ] {
            assert_eq!(
                parse(value).unwrap_err(),
                "FILE_INBOX_LINK_UNSUPPORTED",
                "{value}"
            );
        }
    }
}

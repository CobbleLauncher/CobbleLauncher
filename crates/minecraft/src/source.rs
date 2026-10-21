// SPDX-License-Identifier: MIT
//! Public Mojang file URL mappings, independently implemented from
//! https://bmclapidoc.bangbang93.com/ (usage section). Never maps auth APIs.
use sha1::{Digest, Sha1};
use std::fmt;
use url::Url;

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/";

#[derive(Clone, Debug)]
pub enum DownloadSource {
    Official,
    Bmclapi,
    /// Explicitly supplied BMCLAPI-compatible HTTPS base, not a provider claim.
    CustomBmclapi(Url),
}

#[derive(Clone, Copy, Debug)]
pub enum SourceError {
    InvalidUrl,
    InvalidMirror,
}
impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidUrl => "invalid public artifact URL",
            Self::InvalidMirror => {
                "mirror must be an HTTPS base without credentials, query or fragment"
            }
        })
    }
}
impl std::error::Error for SourceError {}

impl DownloadSource {
    pub fn custom(base: &str) -> Result<Self, SourceError> {
        let base = Url::parse(base).map_err(|_| SourceError::InvalidMirror)?;
        validate_base(&base)?;
        Ok(Self::CustomBmclapi(base))
    }
    /// Short nonsecret label for request identity/provenance. A custom base is
    /// hashed, so URL text is not copied into downloader labels or logs.
    pub fn id(&self) -> String {
        match self {
            Self::Official => "official".into(),
            Self::Bmclapi => "bmclapi".into(),
            Self::CustomBmclapi(base) => format!(
                "custom-bmclapi-{:x}",
                Sha1::digest(base.as_str().as_bytes())
            ),
        }
    }
    pub fn attribution(&self) -> &str {
        match self {
            Self::Official => "Mojang / Microsoft official downloads",
            Self::Bmclapi => "BMCLAPI (bangbang93) — https://bmclapidoc.bangbang93.com/",
            Self::CustomBmclapi(_) => "User-selected BMCLAPI-compatible mirror",
        }
    }
    /// The selected provider serves Mojang-format metadata and expected hashes.
    /// Unrecognized hosts and non-content paths are left untouched. This is a
    /// file-source adapter, not authorization for a mirror's cross-origin CDN.
    pub fn resolve(&self, official_url: &str) -> Result<String, SourceError> {
        let official = Url::parse(official_url).map_err(|_| SourceError::InvalidUrl)?;
        if matches!(self, Self::Official) {
            return Ok(official_url.into());
        }
        // Signed/authenticated endpoints never become mirror requests.
        if official.scheme() != "https"
            || !official.username().is_empty()
            || official.password().is_some()
            || official.port().is_some()
            || official.query().is_some()
            || official.fragment().is_some()
        {
            return Ok(official_url.into());
        }
        let path = official.path();
        let prefix = match official.host_str() {
            Some("resources.download.minecraft.net") => "/assets",
            Some("libraries.minecraft.net") => "/maven",
            Some("piston-meta.mojang.com" | "launchermeta.mojang.com")
                if path.starts_with("/v1/packages/") || path.starts_with("/mc/game/") =>
            {
                ""
            }
            Some("piston-data.mojang.com" | "launcher.mojang.com")
                if path.starts_with("/v1/objects/") || path.starts_with("/mc/game/") =>
            {
                ""
            }
            _ => return Ok(official_url.into()),
        };
        let mut base = match self {
            Self::Bmclapi => Url::parse(BMCLAPI).map_err(|_| SourceError::InvalidMirror)?,
            Self::CustomBmclapi(base) => {
                validate_base(base)?;
                base.clone()
            }
            Self::Official => unreachable!("handled above"),
        };
        let mapped_path = format!("{}{prefix}{path}", base.path().trim_end_matches('/'));
        base.set_path(&mapped_path);
        Ok(base.into())
    }
}

fn validate_base(base: &Url) -> Result<(), SourceError> {
    if base.scheme() != "https"
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(SourceError::InvalidMirror);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_known_public_content_routes() {
        for (input, path) in [
            (
                "https://libraries.minecraft.net/example/library.jar",
                "/maven/example/library.jar",
            ),
            (
                "https://resources.download.minecraft.net/ab/abcdef",
                "/assets/ab/abcdef",
            ),
            (
                "https://piston-data.mojang.com/v1/objects/hash/client.jar",
                "/v1/objects/hash/client.jar",
            ),
            (
                "https://piston-meta.mojang.com/v1/packages/hash/version.json",
                "/v1/packages/hash/version.json",
            ),
            (
                "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
                "/mc/game/version_manifest_v2.json",
            ),
        ] {
            assert_eq!(
                DownloadSource::Bmclapi.resolve(input).unwrap(),
                format!("https://bmclapi2.bangbang93.com{path}")
            );
            assert_eq!(DownloadSource::Official.resolve(input).unwrap(), input);
        }
        for input in [
            "https://login.microsoftonline.com/oauth2/token",
            "https://piston-meta.mojang.com/auth",
            "https://libraries.minecraft.net.evil.invalid/file",
            "https://libraries.minecraft.net/file?token=secret",
        ] {
            assert_eq!(DownloadSource::Bmclapi.resolve(input).unwrap(), input);
        }
    }

    #[test]
    fn custom_mirror_preserves_base_path_and_requires_explicit_https() {
        let source = DownloadSource::custom("https://mirror.example.invalid/minecraft/").unwrap();
        assert_eq!(
            source
                .resolve("https://libraries.minecraft.net/a.jar")
                .unwrap(),
            "https://mirror.example.invalid/minecraft/maven/a.jar"
        );
        assert!(source.id().starts_with("custom-bmclapi-"));
        for base in [
            "http://mirror.example.invalid/",
            "https://user:password@mirror.example.invalid/",
            "https://mirror.example.invalid/?token=secret",
            "https://mirror.example.invalid/#fragment",
        ] {
            assert!(DownloadSource::custom(base).is_err());
        }
    }
}

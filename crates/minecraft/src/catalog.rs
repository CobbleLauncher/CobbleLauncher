// SPDX-License-Identifier: MIT
use cobble_net::{Broker, NetError};
use serde::Deserialize;
use std::fmt;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Deserialize)]
pub struct VersionCatalog {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
    #[serde(other)]
    Other,
}

impl VersionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Snapshot => "snapshot",
            Self::OldAlpha => "old_alpha",
            Self::OldBeta => "old_beta",
            Self::Other => "other",
        }
    }
}

#[derive(Debug)]
pub enum CatalogError {
    Network(NetError),
    HttpStatus(u16),
    InvalidResponse,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(error) => write!(f, "could not fetch Minecraft versions: {error}"),
            Self::HttpStatus(status) => {
                write!(f, "Minecraft version service returned HTTP {status}")
            }
            Self::InvalidResponse => f.write_str("could not read the Minecraft version catalog"),
        }
    }
}
impl std::error::Error for CatalogError {}

impl VersionCatalog {
    /// Explicit user operation; never called during application bootstrap.
    /// Catalog entries are display data, not verified installation plans.
    pub async fn fetch(broker: &Broker) -> Result<Self, CatalogError> {
        let response = broker
            .get(MANIFEST_URL, None, 4 * 1024 * 1024)
            .await
            .map_err(CatalogError::Network)?;
        if response.metadata.status != 200 {
            return Err(CatalogError::HttpStatus(response.metadata.status));
        }
        Self::parse(&response.body)
    }

    fn parse(bytes: &[u8]) -> Result<Self, CatalogError> {
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|_| CatalogError::InvalidResponse)?;
        if catalog.versions.is_empty() {
            return Err(CatalogError::InvalidResponse);
        }
        Ok(catalog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_catalog_without_treating_display_entries_as_install_metadata() {
        let catalog = VersionCatalog::parse(br#"{
            "latest":{"release":"demo-release","snapshot":"demo-snapshot"},
            "versions":[
                {"id":"demo-snapshot","type":"snapshot","releaseTime":"2026-01-02T12:00:00Z","url":"https://example.invalid/snapshot.json"},
                {"id":"demo-release","type":"release","releaseTime":"2026-01-01T12:00:00Z","complianceLevel":1},
                {"id":"future","type":"future_kind","releaseTime":"2026-01-03T12:00:00Z"}
            ]
        }"#).unwrap();
        assert_eq!(catalog.latest.release, "demo-release");
        assert_eq!(catalog.latest.snapshot, "demo-snapshot");
        assert_eq!(catalog.versions[0].kind, VersionKind::Snapshot);
        assert_eq!(catalog.versions[1].kind, VersionKind::Release);
        assert_eq!(catalog.versions[2].kind, VersionKind::Other);
        assert!(
            VersionCatalog::parse(br#"{"latest":{"release":"x","snapshot":"x"},"versions":[]}"#)
                .is_err()
        );
        assert!(VersionCatalog::parse(b"not json").is_err());
    }
}

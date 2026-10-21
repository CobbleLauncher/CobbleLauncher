// SPDX-License-Identifier: MIT
use crate::{DownloadSource, MetadataError, SourceError, VersionMetadata};
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
    pub url: String,
    pub sha1: String,
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
    UnknownVersion,
    Metadata(MetadataError),
    Source(SourceError),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(error) => write!(f, "could not fetch Minecraft versions: {error}"),
            Self::HttpStatus(status) => {
                write!(f, "Minecraft version service returned HTTP {status}")
            }
            Self::InvalidResponse => f.write_str("could not read the Minecraft version catalog"),
            Self::UnknownVersion => {
                f.write_str("Minecraft version is absent from the selected catalog")
            }
            Self::Metadata(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CatalogError {}

impl VersionCatalog {
    pub async fn fetch_version(
        &self,
        broker: &Broker,
        id: &str,
    ) -> Result<VersionMetadata, CatalogError> {
        self.fetch_version_from(broker, id, &DownloadSource::Official)
            .await
    }
    /// Fetch from the selected provider, checking the version bytes against this
    /// catalog's SHA-1 record. A mirror catalog is explicitly trusted by its user;
    /// that is not an independent fetch of the official catalog.
    pub async fn fetch_version_from(
        &self,
        broker: &Broker,
        id: &str,
        source: &DownloadSource,
    ) -> Result<VersionMetadata, CatalogError> {
        let summary = self
            .versions
            .iter()
            .find(|version| version.id == id)
            .ok_or(CatalogError::UnknownVersion)?;
        let url = source.resolve(&summary.url).map_err(CatalogError::Source)?;
        let bytes = fetch_document(broker, &url, &summary.url, 8 * 1024 * 1024).await?;
        VersionMetadata::parse_verified(&bytes, id, &summary.sha1).map_err(CatalogError::Metadata)
    }
    /// Explicit user operation; never called during application bootstrap.
    /// Catalog entries are display data, not verified installation plans.
    pub async fn fetch(broker: &Broker) -> Result<Self, CatalogError> {
        Self::fetch_from(broker, &DownloadSource::Official).await
    }
    /// Try the selected provider first. A temporary mirror failure gets one
    /// official metadata attempt without changing the selected download source.
    pub async fn fetch_from(
        broker: &Broker,
        source: &DownloadSource,
    ) -> Result<Self, CatalogError> {
        let url = source.resolve(MANIFEST_URL).map_err(CatalogError::Source)?;
        let bytes = fetch_document(broker, &url, MANIFEST_URL, 4 * 1024 * 1024).await?;
        Self::parse(&bytes)
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

async fn get_document(broker: &Broker, url: &str, limit: u64) -> Result<Vec<u8>, CatalogError> {
    let response = broker
        .get(url, None, limit)
        .await
        .map_err(CatalogError::Network)?;
    if response.metadata.status != 200 {
        return Err(CatalogError::HttpStatus(response.metadata.status));
    }
    Ok(response.body)
}

fn temporary_failure(error: &CatalogError) -> bool {
    matches!(
        error,
        CatalogError::Network(
            NetError::Timeout(_) | NetError::Connect | NetError::Tls | NetError::Resolution
        ) | CatalogError::HttpStatus(429 | 500..=599)
    )
}

async fn fetch_document(
    broker: &Broker,
    selected: &str,
    official: &str,
    limit: u64,
) -> Result<Vec<u8>, CatalogError> {
    match get_document(broker, selected, limit).await {
        Err(error) if selected != official && temporary_failure(&error) => {
            get_document(broker, official, limit).await
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_mirror_failures_allow_one_official_attempt() {
        assert!(temporary_failure(&CatalogError::Network(
            NetError::Timeout(cobble_net::TimeoutPhase::Headers)
        )));
        assert!(temporary_failure(&CatalogError::HttpStatus(503)));
        assert!(!temporary_failure(&CatalogError::HttpStatus(403)));
        assert!(!temporary_failure(&CatalogError::Network(
            NetError::ForbiddenDestination
        )));
        assert!(!temporary_failure(&CatalogError::InvalidResponse));
    }

    #[tokio::test]
    async fn selected_source_errors_are_reported_before_catalog_or_version_network_requests() {
        let broker = Broker::new(cobble_net::BrokerConfig::default()).unwrap();
        let source = DownloadSource::CustomBmclapi(
            url::Url::parse("http://mirror.example.invalid/").unwrap(),
        );
        assert!(matches!(
            VersionCatalog::fetch_from(&broker, &source).await,
            Err(CatalogError::Source(_))
        ));
        let catalog = VersionCatalog {
            latest: LatestVersions {
                release: "fixture".into(),
                snapshot: "fixture".into(),
            },
            versions: vec![VersionSummary {
                id: "fixture".into(),
                kind: VersionKind::Release,
                release_time: "2026-01-01T00:00:00Z".into(),
                url: "https://piston-meta.mojang.com/v1/packages/fixture/fixture.json".into(),
                sha1: "0".repeat(40),
            }],
        };
        assert!(matches!(
            catalog
                .fetch_version_from(&broker, "fixture", &source)
                .await,
            Err(CatalogError::Source(_))
        ));
    }

    #[test]
    fn reads_catalog_without_treating_display_entries_as_install_metadata() {
        let catalog = VersionCatalog::parse(br#"{
            "latest":{"release":"demo-release","snapshot":"demo-snapshot"},
            "versions":[
                {"id":"demo-snapshot","type":"snapshot","releaseTime":"2026-01-02T12:00:00Z","url":"https://example.invalid/snapshot.json","sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                {"id":"demo-release","type":"release","releaseTime":"2026-01-01T12:00:00Z","url":"https://example.invalid/release.json","sha1":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","complianceLevel":1},
                {"id":"future","type":"future_kind","releaseTime":"2026-01-03T12:00:00Z","url":"https://example.invalid/future.json","sha1":"cccccccccccccccccccccccccccccccccccccccc"}
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

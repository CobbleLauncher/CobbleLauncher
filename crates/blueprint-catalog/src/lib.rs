// SPDX-License-Identifier: MIT
//! Pure read-only MCS API normalization. No HTTP, uploads, credentials or filesystem access.
//! Version-family filters aid discovery; they do not prove exact game/mod compatibility.
//! Original downloads are distinct from the provider's potentially converted viewer source.

use serde::{Deserialize, Serialize};
use url::Url;

pub const MCS_ORIGIN: &str = "https://www.mcschematic.top";
pub const MAX_API_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum CatalogError {
    UnsupportedGameVersion,
    InvalidQuery,
    InvalidMetadata,
    InvalidJson,
    InputTooLarge,
    UnsupportedFormat,
    RestrictedResource,
}

/// The API currently documents java-1.7.10 and java-1.12 through java-1.21.
pub fn java_version_family(release: &str) -> Result<String, CatalogError> {
    if release.len() > 32 {
        return Err(CatalogError::UnsupportedGameVersion);
    }
    if release == "1.7.10" {
        return Ok("java-1.7.10".into());
    }
    let parts: Vec<_> = release.split('.').collect();
    if !(2..=3).contains(&parts.len()) || parts[0] != "1" {
        return Err(CatalogError::UnsupportedGameVersion);
    }
    if parts.iter().any(|part| {
        part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
    }) {
        return Err(CatalogError::UnsupportedGameVersion);
    }
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| CatalogError::UnsupportedGameVersion)?;
    if !(12..=21).contains(&minor) {
        return Err(CatalogError::UnsupportedGameVersion);
    }
    Ok(format!("java-1.{minor}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewerFormat {
    Schem,
    Litematic,
}
impl ViewerFormat {
    fn api_filter(self) -> &'static str {
        match self {
            Self::Schem => "world_edit",
            Self::Litematic => "litematica",
        }
    }
    fn extension(self) -> &'static str {
        match self {
            Self::Schem => "schem",
            Self::Litematic => "litematic",
        }
    }
}

pub struct SearchQuery<'a> {
    pub java_release: &'a str,
    pub text: &'a str,
    pub format: Option<ViewerFormat>,
    pub mod_namespace: Option<&'a str>,
    pub cursor: Option<&'a str>,
    pub limit: u8,
}
impl SearchQuery<'_> {
    /// Use discovery's literal q filter: /search has no gameVersion or mod filter.
    /// Cursor values are opaque and bound by the service to the complete filter set.
    pub fn url(&self) -> Result<Url, CatalogError> {
        if !(1..=50).contains(&self.limit)
            || self.text.chars().count() > 200
            || self.text.chars().any(char::is_control)
            || self.cursor.is_some_and(|cursor| {
                cursor.is_empty() || cursor.len() > 8192 || cursor.chars().any(char::is_control)
            })
        {
            return Err(CatalogError::InvalidQuery);
        }
        // Empty release explicitly requests unfiltered discovery. A nonempty
        // unsupported release remains an error, so the caller can disclose fallback.
        let family = if self.java_release.is_empty() {
            None
        } else {
            Some(java_version_family(self.java_release)?)
        };
        let mut url = Url::parse(MCS_ORIGIN).expect("fixed HTTPS origin");
        url.set_path("/api/v1/blueprints");
        let mut query = url.query_pairs_mut();
        query.append_pair("extra", "none");
        if let Some(family) = family {
            query.append_pair("gameVersion", &family);
        }
        query.append_pair("limit", &self.limit.to_string());
        if !self.text.trim().is_empty() {
            query.append_pair("q", self.text.trim());
        }
        if let Some(format) = self.format {
            query.append_pair("format", format.api_filter());
        }
        if let Some(namespace) = self.mod_namespace {
            if namespace.is_empty()
                || namespace.len() > 64
                || !namespace.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
                })
            {
                return Err(CatalogError::InvalidQuery);
            }
            query.append_pair("mod", namespace);
        }
        if let Some(cursor) = self.cursor {
            query.append_pair("cursor", cursor);
        }
        drop(query);
        Ok(url)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Create,
    BuildingGadgets,
    Litematica,
    WorldEdit,
    BedrockEdition,
    Mcedit,
    #[serde(other)]
    Unknown,
}
impl SourceFormat {
    pub fn viewer_format(&self) -> Option<ViewerFormat> {
        match self {
            Self::WorldEdit => Some(ViewerFormat::Schem),
            Self::Litematica => Some(ViewerFormat::Litematic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    Original,
    Repost,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGameVersion {
    pub edition: String,
    pub release: Option<String>,
    pub data_version: Option<i32>,
    pub evidence: String,
    pub family: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryVersion {
    pub number: u32,
    pub source_format: SourceFormat,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSummary {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub attribution: Attribution,
    pub author: Author,
    pub current_version: SummaryVersion,
    pub source_game_version: Option<SourceGameVersion>,
    #[serde(default)]
    pub namespaces: Vec<String>,
    pub preview_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintPage {
    pub items: Vec<BlueprintSummary>,
    pub next_cursor: Option<String>,
    pub total_count: u64,
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CatalogError> {
    if bytes.len() > MAX_API_BYTES {
        return Err(CatalogError::InputTooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| CatalogError::InvalidJson)
}

pub fn parse_page(bytes: &[u8]) -> Result<BlueprintPage, CatalogError> {
    let page: BlueprintPage = decode(bytes)?;
    if page.items.len() > 50
        || page
            .next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() > 8192)
        || page.items.iter().any(|item| {
            !valid_uuid(&item.id)
                || item.current_version.number == 0
                || item.title.is_empty()
                || item.title.len() > 4096
                || item.namespaces.len() > 1024
        })
    {
        return Err(CatalogError::InvalidMetadata);
    }
    Ok(page)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintDetail {
    pub id: String,
    pub title: String,
    pub author: Author,
    pub attribution: Attribution,
    pub visibility: String,
    pub anonymously_readable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintVersion {
    pub blueprint_id: String,
    pub number: u32,
    pub source_format: SourceFormat,
    pub original_filename: String,
    pub original_source_byte_size: u64,
    pub download_url: String,
    pub validation_state: String,
}

#[derive(Debug, Serialize)]
pub struct Provenance {
    pub provider: &'static str,
    pub blueprint_id: String,
    pub version: u32,
    pub title: String,
    pub author: Author,
    /// "Original" is authorship metadata, not a redistribution license.
    pub attribution: Attribution,
    /// The public API supplies no per-blueprint license or trusted digest.
    pub license: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileProposal {
    pub request_url: String,
    /// Generated safe basename. Never authorize a path from originalFilename.
    pub suggested_filename: String,
    pub original_filename: String,
    pub declared_byte_size: u64,
    pub format: ViewerFormat,
    pub provenance: Provenance,
}

/// A proposal only: caller still authorizes download, redirect hosts, bounded bytes,
/// local destination and Viewer parsing. No hash or compatibility is invented.
pub fn original_file_proposal(
    detail_bytes: &[u8],
    version_bytes: &[u8],
) -> Result<FileProposal, CatalogError> {
    let detail: BlueprintDetail = decode(detail_bytes)?;
    let version: BlueprintVersion = decode(version_bytes)?;
    if !detail.anonymously_readable || detail.visibility != "public" {
        return Err(CatalogError::RestrictedResource);
    }
    if !valid_uuid(&detail.id)
        || detail.id != version.blueprint_id
        || version.number == 0
        || version.number > i32::MAX as u32
        || version.validation_state != "valid"
        || version.original_source_byte_size == 0
        || version.original_filename.len() > 1024
        || version
            .original_filename
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'))
    {
        return Err(CatalogError::InvalidMetadata);
    }
    let format = version
        .source_format
        .viewer_format()
        .ok_or(CatalogError::UnsupportedFormat)?;
    if !version
        .original_filename
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", format.extension()))
    {
        return Err(CatalogError::UnsupportedFormat);
    }
    let origin = Url::parse(MCS_ORIGIN).expect("fixed HTTPS origin");
    let expected = origin
        .join(&format!(
            "/api/v1/blueprints/{}/versions/{}/download",
            detail.id, version.number
        ))
        .expect("validated UUID and version");
    let supplied = origin
        .join(&version.download_url)
        .map_err(|_| CatalogError::InvalidMetadata)?;
    if supplied != expected {
        return Err(CatalogError::InvalidMetadata);
    }
    Ok(FileProposal {
        request_url: expected.to_string(),
        suggested_filename: format!("{}-v{}.{}", detail.id, version.number, format.extension()),
        original_filename: version.original_filename,
        declared_byte_size: version.original_source_byte_size,
        format,
        provenance: Provenance {
            provider: "mcschematic.top",
            blueprint_id: detail.id,
            version: version.number,
            title: detail.title,
            author: detail.author,
            attribution: detail.attribution,
            license: None,
            sha256: None,
        },
    })
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests;

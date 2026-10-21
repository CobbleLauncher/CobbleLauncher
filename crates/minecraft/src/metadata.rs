// SPDX-License-Identifier: MIT
//! Original parser for Mojang's public version JSON. No loader overlays or launch.
use cobble_store::SafeRelativePath;
use regex::Regex;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{collections::HashMap, fmt};

#[derive(Debug)]
pub enum MetadataError {
    InvalidMetadata,
    DigestMismatch,
    Unsupported(&'static str),
}
impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMetadata => f.write_str("invalid Minecraft version metadata"),
            Self::DigestMismatch => f.write_str("Minecraft version metadata SHA-1 mismatch"),
            Self::Unsupported(reason) => write!(f, "unsupported Minecraft metadata: {reason}"),
        }
    }
}
impl std::error::Error for MetadataError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    pub id: String,
    pub main_class: String,
    pub java_version: JavaVersion,
    pub asset_index: AssetIndex,
    downloads: VersionDownloads,
    libraries: Vec<Library>,
    #[serde(default)]
    logging: Logging,
    pub arguments: serde_json::Value,
    inherits_from: Option<String>,
    #[serde(skip)]
    source_bytes: Vec<u8>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}
#[derive(Debug, Deserialize)]
struct VersionDownloads {
    client: Option<Artifact>,
}
#[derive(Debug, Deserialize)]
struct Artifact {
    url: String,
    sha1: String,
    size: u64,
    path: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Library {
    downloads: Option<LibraryDownloads>,
    rules: Option<Vec<Rule>>,
    #[serde(default)]
    natives: HashMap<String, String>,
}
#[derive(Debug, Deserialize)]
struct LibraryDownloads {
    artifact: Option<Artifact>,
    #[serde(default)]
    classifiers: HashMap<String, Artifact>,
}
#[derive(Debug, Default, Deserialize)]
struct Logging {
    client: Option<ClientLogging>,
}
#[derive(Debug, Deserialize)]
struct ClientLogging {
    argument: Option<String>,
    file: LoggingFile,
}
#[derive(Debug, Deserialize)]
struct LoggingFile {
    id: String,
    url: String,
    sha1: String,
    size: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rule {
    action: Action,
    os: Option<OsRule>,
    #[serde(default)]
    features: HashMap<String, bool>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Allow,
    Disallow,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OsRule {
    name: Option<String>,
    arch: Option<String>,
    version: Option<String>,
}

/// OS/JVM architecture are explicit inputs. An unavailable OS version causes an
/// explicit unsupported error if a selected rule needs it.
#[derive(Debug)]
pub struct PlatformContext {
    pub os: String,
    pub arch: String,
    pub os_version: Option<String>,
    pub features: HashMap<String, bool>,
}
impl PlatformContext {
    pub fn current() -> Result<Self, MetadataError> {
        let os = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            "linux" => "linux",
            _ => return Err(MetadataError::Unsupported("operating system")),
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "x86" => "x86",
            "aarch64" => "aarch64",
            _ => return Err(MetadataError::Unsupported("architecture")),
        };
        Ok(Self {
            os: os.into(),
            arch: arch.into(),
            os_version: None,
            features: HashMap::new(),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Client,
    Library,
    Native,
    Logging,
    AssetIndex,
    Asset,
}
#[derive(Debug)]
pub struct PlannedFile {
    pub path: SafeRelativePath,
    pub url: String,
    pub size: u64,
    pub sha1: [u8; 20],
    pub kind: FileKind,
}
#[derive(Debug)]
pub struct DownloadPlan {
    pub version_id: String,
    pub java_major: u32,
    pub main_class: String,
    pub files: Vec<PlannedFile>,
    pub asset_index: AssetIndex,
}
pub(crate) fn parse_sha1(value: &str) -> Result<[u8; 20], MetadataError> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MetadataError::InvalidMetadata);
    }
    let mut output = [0; 20];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| MetadataError::InvalidMetadata)?;
    }
    Ok(output)
}
fn component(value: &str) -> Result<&str, MetadataError> {
    SafeRelativePath::parse(value).map_err(|_| MetadataError::InvalidMetadata)?;
    if value.contains('/') {
        return Err(MetadataError::InvalidMetadata);
    }
    Ok(value)
}
fn file(path: String, artifact: &Artifact, kind: FileKind) -> Result<PlannedFile, MetadataError> {
    Ok(PlannedFile {
        path: SafeRelativePath::parse(&path).map_err(|_| MetadataError::InvalidMetadata)?,
        url: artifact.url.clone(),
        size: artifact.size,
        sha1: parse_sha1(&artifact.sha1)?,
        kind,
    })
}
impl VersionMetadata {
    /// Verify the exact response bytes against the manifest before parsing.
    pub fn parse_verified(
        bytes: &[u8],
        expected_id: &str,
        expected_sha1: &str,
    ) -> Result<Self, MetadataError> {
        if Sha1::digest(bytes).as_slice() != parse_sha1(expected_sha1)? {
            return Err(MetadataError::DigestMismatch);
        }
        Self::parse_installed(bytes, expected_id)
    }
    /// Read metadata already verified and persisted by the installer.
    pub fn parse_installed(bytes: &[u8], expected_id: &str) -> Result<Self, MetadataError> {
        let raw: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| MetadataError::InvalidMetadata)?;
        if raw.get("inheritsFrom").is_some() {
            return Err(MetadataError::Unsupported("inherited profiles"));
        }
        if raw.get("minecraftArguments").is_some() || raw.get("arguments").is_none() {
            return Err(MetadataError::Unsupported("legacy arguments"));
        }
        let mut metadata: Self =
            serde_json::from_value(raw).map_err(|_| MetadataError::InvalidMetadata)?;
        if metadata.id != expected_id
            || metadata.main_class.is_empty()
            || metadata.java_version.major_version == 0
            || !metadata.arguments.is_object()
            || metadata.downloads.client.is_none()
        {
            return Err(MetadataError::InvalidMetadata);
        }
        metadata.source_bytes = bytes.to_vec();
        Ok(metadata)
    }
    pub fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }
    pub fn logging_argument(&self) -> Option<(&str, &str)> {
        let client = self.logging.client.as_ref()?;
        Some((client.argument.as_deref()?, &client.file.id))
    }
    pub fn download_plan(&self, platform: &PlatformContext) -> Result<DownloadPlan, MetadataError> {
        if self.inherits_from.is_some() {
            return Err(MetadataError::Unsupported("inherited profiles"));
        }
        if !matches!(platform.os.as_str(), "windows" | "osx" | "linux") {
            return Err(MetadataError::Unsupported("operating system"));
        }
        let id = component(&self.id)?;
        let client = self
            .downloads
            .client
            .as_ref()
            .ok_or(MetadataError::InvalidMetadata)?;
        let mut files = vec![file(
            format!("versions/{id}/{id}.jar"),
            client,
            FileKind::Client,
        )?];
        for library in &self.libraries {
            if !allowed(library.rules.as_deref(), platform)? {
                continue;
            }
            let downloads = library
                .downloads
                .as_ref()
                .ok_or(MetadataError::Unsupported(
                    "library without explicit downloads",
                ))?;
            if let Some(artifact) = &downloads.artifact {
                files.push(file(
                    format!(
                        "libraries/{}",
                        artifact
                            .path
                            .as_deref()
                            .ok_or(MetadataError::InvalidMetadata)?
                    ),
                    artifact,
                    FileKind::Library,
                )?);
            }
            if let Some(classifier) = library.natives.get(&platform.os) {
                let classifier = if classifier.contains("${arch}") {
                    let bits = match platform.arch.as_str() {
                        "amd64" | "x86_64" => "64",
                        "x86" => "32",
                        _ => {
                            return Err(MetadataError::Unsupported(
                                "native architecture placeholder",
                            ));
                        }
                    };
                    classifier.replace("${arch}", bits)
                } else {
                    classifier.clone()
                };
                let native = downloads
                    .classifiers
                    .get(&classifier)
                    .ok_or(MetadataError::Unsupported("native classifier unavailable"))?;
                files.push(file(
                    format!(
                        "libraries/{}",
                        native
                            .path
                            .as_deref()
                            .ok_or(MetadataError::InvalidMetadata)?
                    ),
                    native,
                    FileKind::Native,
                )?);
            }
        }
        if let Some(logging) = &self.logging.client {
            let log = &logging.file;
            files.push(file(
                format!("assets/log_configs/{}", component(&log.id)?),
                &Artifact {
                    url: log.url.clone(),
                    sha1: log.sha1.clone(),
                    size: log.size,
                    path: None,
                },
                FileKind::Logging,
            )?);
        }
        let asset = &self.asset_index;
        files.push(file(
            format!("assets/indexes/{}.json", component(&asset.id)?),
            &Artifact {
                url: asset.url.clone(),
                sha1: asset.sha1.clone(),
                size: asset.size,
                path: None,
            },
            FileKind::AssetIndex,
        )?);
        Ok(DownloadPlan {
            version_id: self.id.clone(),
            java_major: self.java_version.major_version,
            main_class: self.main_class.clone(),
            files,
            asset_index: asset.clone(),
        })
    }
}
pub(crate) fn allowed(
    rules: Option<&[Rule]>,
    platform: &PlatformContext,
) -> Result<bool, MetadataError> {
    let Some(rules) = rules else {
        return Ok(true);
    };
    let mut allowed = false;
    for rule in rules {
        if let Some(os) = &rule.os {
            if os.name.as_ref().is_some_and(|name| name != &platform.os)
                || os.arch.as_ref().is_some_and(|arch| arch != &platform.arch)
            {
                continue;
            }
            if let Some(pattern) = &os.version {
                let regex = Regex::new(pattern)
                    .map_err(|_| MetadataError::Unsupported("OS version regex"))?;
                let version = platform
                    .os_version
                    .as_ref()
                    .ok_or(MetadataError::Unsupported("OS version required by rule"))?;
                if !regex.is_match(version) {
                    continue;
                }
            }
        }
        if rule.features.iter().any(|(name, expected)| {
            platform.features.get(name).copied().unwrap_or(false) != *expected
        }) {
            continue;
        }
        allowed = matches!(rule.action, Action::Allow);
    }
    Ok(allowed)
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

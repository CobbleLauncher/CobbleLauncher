// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackProvider {
    Mrpack,
    CurseForge,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSource {
    pub provider: PackProvider,
    pub project_id: Option<String>,
}
/// Trusted retained manifest metadata, not a scan of the current instance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifestSnapshot {
    pub source: PackSource,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
    pub files: Vec<PackManifestFile>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifestFile {
    pub identity: String,
    pub version_id: Option<String>,
    pub path: String,
    pub size_bytes: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}
/// Native caller hashes scoped regular files. Omitted observations mean unknown;
/// explicit exists=false alone establishes that a path is absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedPackFile {
    pub path: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackLocalState {
    Clean,
    Modified,
    Missing,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFileChange {
    pub before: Option<PackManifestFile>,
    pub after: Option<PackManifestFile>,
    pub local_state: PackLocalState,
    pub preserve: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackIdentityRelation {
    Confirmed,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackUpdatePreview {
    pub identity_relation: PackIdentityRelation,
    pub current_version: String,
    pub target_version: String,
    pub runtime_changed: bool,
    pub added: Vec<PackFileChange>,
    pub removed: Vec<PackFileChange>,
    pub version_changed: Vec<PackFileChange>,
    pub unchanged: Vec<PackFileChange>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackUpdatePreviewError {
    MissingProvenance,
    SourceMismatch,
    InvalidManifest,
    InvalidObservation,
}
impl std::fmt::Display for PackUpdatePreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MissingProvenance => "Installed pack manifest provenance is unavailable",
            Self::SourceMismatch => "Selected pack source differs from installed pack",
            Self::InvalidManifest => "Invalid pack preview manifest",
            Self::InvalidObservation => "Invalid local pack observation",
        })
    }
}
impl std::error::Error for PackUpdatePreviewError {}

// SPDX-License-Identifier: MIT
//! Official Minecraft metadata and modern Vanilla artifact download planning.

mod catalog;
pub use catalog::{CatalogError, LatestVersions, VersionCatalog, VersionKind, VersionSummary};
mod metadata;
pub use metadata::{
    AssetIndex, DownloadPlan, FileKind, MetadataError, PlannedFile, PlatformContext,
    VersionMetadata,
};
mod java;
pub use java::{JavaError, JavaRuntime, find_java, probe_java};
mod source;
pub use source::{DownloadSource, SourceError};
mod settings;
pub use settings::{
    DownloadSettings, DownloadSettingsError, DownloadSourceChoice, read_download_settings,
    save_download_settings,
};

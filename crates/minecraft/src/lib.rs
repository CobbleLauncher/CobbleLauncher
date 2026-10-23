// SPDX-License-Identifier: MIT
//! Official Minecraft metadata and modern Vanilla artifact download planning.

mod catalog;
pub use catalog::{CatalogError, LatestVersions, VersionCatalog, VersionKind, VersionSummary};
mod launch;
mod metadata;
pub use launch::{native_artifact_matches, resolve_arguments, substitute};
pub use metadata::{
    AssetIndex, DownloadPlan, FileKind, MetadataError, PlannedFile, PlatformContext,
    VersionMetadata,
};
mod java;
pub use java::{JavaError, JavaRuntime, discover_java, find_java, probe_java, recommend_java};
mod java_settings;
pub use java_settings::{
    JavaSettings, read_java_settings, resolve_java, resolve_java_override, save_java_settings,
};
mod preferences;
pub use preferences::{
    JavaOverride, LaunchBehavior, LaunchOverrides, LauncherPreferences, PreferencesError,
    WindowSize, read_launcher_preferences, save_launcher_preferences, validate_advanced_runtime,
    validate_memory_mb,
};
mod source;
pub use source::{DownloadSource, PublicFetchError, SourceError};
mod settings;
pub use settings::{
    DownloadSettings, DownloadSettingsError, DownloadSourceChoice, read_download_settings,
    save_download_settings,
};

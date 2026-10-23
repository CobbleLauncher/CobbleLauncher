// SPDX-License-Identifier: MIT
//! Only manifest-indexed, bounded immutable static UI assets enter a package.
use crate::{PluginError, PluginManifest, digest, io_error, read_manifest, read_regular};
use cobble_plugin_api::{
    FRONTEND_ASSET_INDEX, FrontendAssetIndex, MAX_FRONTEND_INDEX_BYTES, frontend_asset_media_type,
};
use std::path::Path;
pub struct FrontendAssetBytes {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_INVALID_FRONTEND",
        "Frontend asset changed, is unsafe or is not indexed",
    )
}
fn regular_path(root: &Path, relative: &str) -> Result<(), PluginError> {
    let mut path = root.to_path_buf();
    let metadata = std::fs::symlink_metadata(&path).map_err(io_error)?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err(invalid());
    }
    let parts: Vec<_> = relative.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || matches!(*part, "." | "..") || part.contains(['\\', ':']) {
            return Err(invalid());
        }
        path.push(part);
        let metadata = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if linked(&metadata)
            || (index + 1 < parts.len() && !metadata.is_dir())
            || (index + 1 == parts.len() && !metadata.is_file())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn linked(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}
fn index(
    root: &Path,
    manifest: &PluginManifest,
) -> Result<(FrontendAssetIndex, Vec<u8>), PluginError> {
    let frontend = manifest.frontend.as_ref().ok_or_else(invalid)?;
    regular_path(root, FRONTEND_ASSET_INDEX)?;
    let bytes = read_regular(root, FRONTEND_ASSET_INDEX, MAX_FRONTEND_INDEX_BYTES)?;
    if digest(&bytes) != frontend.assets_sha256.to_ascii_lowercase() {
        return Err(invalid());
    }
    let index: FrontendAssetIndex = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    index.validate(frontend)?;
    Ok((index, bytes))
}
fn asset(
    root: &Path,
    path: &str,
    index: &FrontendAssetIndex,
) -> Result<FrontendAssetBytes, PluginError> {
    let media_type = frontend_asset_media_type(path).ok_or_else(invalid)?;
    let expected = index
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(invalid)?;
    regular_path(root, path)?;
    let bytes = read_regular(root, path, expected.bytes as usize)?;
    if bytes.len() as u64 != expected.bytes
        || digest(&bytes) != expected.sha256.to_ascii_lowercase()
    {
        return Err(invalid());
    }
    Ok(FrontendAssetBytes { bytes, media_type })
}
pub(crate) fn validate(root: &Path, manifest: &PluginManifest) -> Result<(), PluginError> {
    if manifest.frontend.is_none() {
        return Ok(());
    }
    let (index, _) = index(root, manifest)?;
    for file in &index.files {
        asset(root, &file.path, &index)?;
    }
    Ok(())
}
pub(crate) fn copy(
    source: &Path,
    destination: &Path,
    manifest: &PluginManifest,
) -> Result<(), PluginError> {
    if manifest.frontend.is_none() {
        return Ok(());
    }
    let (index, bytes) = index(source, manifest)?;
    for file in &index.files {
        let data = asset(source, &file.path, &index)?;
        let target = destination.join(&file.path);
        std::fs::create_dir_all(target.parent().ok_or_else(invalid)?).map_err(io_error)?;
        std::fs::write(target, data.bytes).map_err(io_error)?;
    }
    std::fs::write(destination.join(FRONTEND_ASSET_INDEX), bytes).map_err(io_error)?;
    Ok(())
}
/// Native caller additionally binds enabled plugin, view session and nonce. This
/// checks the exact reviewed manifest and requested asset on every uncached read.
pub fn read_frontend_asset(
    directory: &Path,
    expected: &PluginManifest,
    asset_path: &str,
) -> Result<FrontendAssetBytes, PluginError> {
    if read_manifest(directory)? != *expected {
        return Err(PluginError::new(
            "PLUGIN_CHANGED",
            "Frontend manifest changed",
        ));
    }
    expected.validate()?;
    let (index, _) = index(directory, expected)?;
    asset(directory, asset_path, &index)
}

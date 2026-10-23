// SPDX-License-Identifier: MIT
//! Declared inert package artwork. No remote URLs or filesystem paths reach guests.
use crate::{PluginError, PluginManifest, digest, io_error, read_regular};
use image::ImageDecoder;
use std::{io::Cursor, path::Path};
pub const MAX_ICON_BYTES: usize = 256 * 1024;
pub fn validate_icon_png(bytes: &[u8], expected_sha256: &str) -> Result<(), PluginError> {
    let invalid = || PluginError::new("PLUGIN_INVALID_PACKAGE", "Invalid or oversized PNG icon");
    if bytes.len() > MAX_ICON_BYTES || digest(bytes) != expected_sha256.to_ascii_lowercase() {
        return Err(invalid());
    }
    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes)).map_err(|_| invalid())?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return Err(invalid());
    }
    image::DynamicImage::from_decoder(decoder).map_err(|_| invalid())?;
    Ok(())
}
pub(crate) fn validate_package_assets(
    directory: &Path,
    manifest: &PluginManifest,
) -> Result<(), PluginError> {
    manifest.validate_metadata()?;
    crate::frontend_assets::validate(directory, manifest)?;
    if let Some(icon) = &manifest.icon {
        validate_icon_png(
            &read_regular(directory, &icon.file, MAX_ICON_BYTES)?,
            &icon.sha256,
        )?;
    }
    Ok(())
}
/// Copy only declared validated artwork; used by registry and SDK packaging.
pub fn copy_package_assets(
    source: &Path,
    destination: &Path,
    manifest: &PluginManifest,
) -> Result<(), PluginError> {
    validate_package_assets(source, manifest)?;
    crate::frontend_assets::copy(source, destination, manifest)?;
    if let Some(icon) = &manifest.icon {
        std::fs::write(
            destination.join(&icon.file),
            read_regular(source, &icon.file, MAX_ICON_BYTES)?,
        )
        .map_err(io_error)?;
    }
    Ok(())
}
pub(crate) fn read_icon(
    directory: &Path,
    manifest: &PluginManifest,
) -> Result<Option<Vec<u8>>, PluginError> {
    validate_package_assets(directory, manifest)?;
    manifest
        .icon
        .as_ref()
        .map(|icon| read_regular(directory, &icon.file, MAX_ICON_BYTES))
        .transpose()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginRegistry;
    use base64::Engine;
    const ICON: &[u8] = include_bytes!("../../../examples/modpack-inspector/icon.png");
    fn package(root: &Path, version: &str) -> PluginManifest {
        let bytes = b"\0asm\x0d\0\x01\0";
        let manifest:PluginManifest=serde_json::from_value(serde_json::json!({"id":"asset-fixture","name":"Asset fixture","version":version,"api_version":"0.1.0","publisher":"Original fixture","capabilities":["report"],"file_extensions":["txt"],"component_sha256":digest(bytes),"icon":{"file":"icon.png","sha256":digest(ICON)},"description":{"en":"Local fixture","zh-CN":"本地测试"},"tags":["fixture"]})).unwrap();
        std::fs::write(
            root.join("plugin.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(root.join("component.wasm"), bytes).unwrap();
        std::fs::write(root.join("icon.png"), ICON).unwrap();
        manifest
    }
    #[test]
    fn icons_survive_install_disabled_update_and_validate_real_resource() {
        let source = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().into());
        let manifest = package(source.path(), "1.0.0");
        registry.install_directory(source.path()).unwrap();
        assert_eq!(registry.icon(&manifest.id).unwrap().unwrap(), ICON);
        registry.set_enabled(&manifest.id, false).unwrap();
        package(source.path(), "2.0.0");
        let updated = registry.update_directory(source.path()).unwrap();
        assert!(!updated.enabled);
        assert_eq!(registry.icon(&manifest.id).unwrap().unwrap(), ICON);
        std::fs::remove_file(source.path().join("icon.png")).unwrap();
        assert!(crate::read_package(source.path()).is_err());
        std::fs::write(source.path().join("icon.png"), b"malformed").unwrap();
        assert!(crate::read_package(source.path()).is_err());
        assert_eq!(registry.icon(&manifest.id).unwrap().unwrap(), ICON);
        let mut unsafe_manifest = manifest;
        unsafe_manifest.icon.as_mut().unwrap().file = "../icon.png".into();
        assert!(unsafe_manifest.validate_metadata().is_err());
    }
    #[test]
    fn inline_catalog_png_requires_real_matching_bounded_icon() {
        let source = tempfile::tempdir().unwrap();
        let manifest = package(source.path(), "1.0.0");
        let mut entry = crate::catalog::CatalogPlugin {
            package: None,
            manifest,
            description: String::new(),
            component_url: "https://example.invalid/component.wasm".into(),
            project_url: None,
            icon_png_base64: Some(base64::engine::general_purpose::STANDARD.encode(ICON)),
        };
        assert_eq!(entry.icon_bytes().unwrap().unwrap(), ICON);
        let mut missing = entry.clone();
        missing.icon_png_base64 = None;
        assert!(!missing.supported());
        entry.icon_png_base64 = Some("invalid".into());
        assert!(entry.icon_bytes().is_err());
        entry.icon_png_base64 =
            Some(base64::engine::general_purpose::STANDARD.encode([0u8; 32769]));
        assert!(entry.icon_bytes().is_err());
        entry.icon_png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"bad PNG"));
        entry.manifest.icon.as_mut().unwrap().sha256 = digest(b"bad PNG");
        assert!(entry.icon_bytes().is_err());
    }
}

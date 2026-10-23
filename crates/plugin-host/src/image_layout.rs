// SPDX-License-Identifier: MIT
//! Metadata-only screenshot layout in a bounded owned worker.
use cobble_plugin_api::{
    ImageLayoutRequest, ImageSheet, MAX_COMPONENT_BYTES, PluginError, PluginManifest,
};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn run_image_layout(
    host: &Path,
    component: &[u8],
    manifest: &PluginManifest,
    request: &ImageLayoutRequest,
    cancel: Arc<AtomicBool>,
) -> Result<ImageSheet, PluginError> {
    request.validate(manifest)?;
    if component.len() > MAX_COMPONENT_BYTES {
        return Err(PluginError::new("PLUGIN_LIMIT", "Component exceeds budget"));
    }
    let bytes = serde_json::to_vec(request).map_err(|_| {
        PluginError::new("PLUGIN_INVALID_IMAGE_LAYOUT", "Invalid image descriptors")
    })?;
    if bytes.len() > 8192 {
        return Err(PluginError::new(
            "PLUGIN_LIMIT",
            "Image descriptors exceed budget",
        ));
    }
    super::report::run_worker(
        host,
        "--image-layout-stdio",
        vec![component.to_vec(), bytes],
        cancel,
        |sheet: ImageSheet| {
            sheet.validate(request)?;
            Ok(sheet)
        },
    )
}

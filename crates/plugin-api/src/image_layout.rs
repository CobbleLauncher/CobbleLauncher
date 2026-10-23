// SPDX-License-Identifier: MIT
//! Pure screenshot layout proposals; pixels and output paths stay with the host.
use crate::{PluginError, PluginManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const IMAGE_LAYOUT_API_VERSION: &str = "0.1.0";
pub const IMAGE_LAYOUT_INTERFACE: &str = "launcher:image-layout";
pub const MAX_LAYOUT_IMAGES: usize = 12;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutImage {
    pub id: String,
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub modified_at: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOrdering {
    Selected,
    Oldest,
    Newest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageLayoutRequest {
    pub locale: String,
    pub images: Vec<LayoutImage>,
    pub columns: u8,
    pub ordering: ImageOrdering,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageSheet {
    pub columns: u8,
    pub gap: u8,
    pub background: ImageRgb,
    pub image_ids: Vec<String>,
}
fn invalid(message: &str) -> PluginError {
    PluginError::new("PLUGIN_INVALID_IMAGE_LAYOUT", message)
}
impl ImageLayoutRequest {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        manifest.validate()?;
        if manifest.capabilities != ["image-layout"]
            || manifest.api_version != IMAGE_LAYOUT_API_VERSION
        {
            return Err(invalid(
                "An enabled image-layout implementation is required",
            ));
        }
        self.validate_bounds()
    }
    pub fn validate_bounds(&self) -> Result<(), PluginError> {
        let mut ids = BTreeSet::new();
        if !matches!(self.locale.as_str(), "en" | "zh-CN")
            || self.columns > 4
            || self.images.is_empty()
            || self.images.len() > MAX_LAYOUT_IMAGES
            || self.images.iter().any(|image| {
                image.id.is_empty()
                    || image.id.len() > 128
                    || !image
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                    || !ids.insert(&image.id)
                    || image.name.is_empty()
                    || image.name.len() > 160
                    || image
                        .name
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\'))
                    || !(1..=256).contains(&image.width)
                    || !(1..=256).contains(&image.height)
            })
        {
            return Err(invalid(
                "Expected 1–12 unique selected thumbnail descriptors and bounded layout options",
            ));
        }
        Ok(())
    }
}
impl ImageSheet {
    pub fn validate(&self, request: &ImageLayoutRequest) -> Result<(), PluginError> {
        request.validate_bounds()?;
        let expected: BTreeSet<_> = request.images.iter().map(|i| i.id.as_str()).collect();
        let actual: BTreeSet<_> = self.image_ids.iter().map(String::as_str).collect();
        if !(1..=4).contains(&self.columns)
            || self.gap > 32
            || self.image_ids.len() != request.images.len()
            || actual != expected
        {
            return Err(invalid(
                "Layout must include every selected image exactly once within bounded grid dimensions",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ImageLayoutRequest {
        ImageLayoutRequest {
            locale: "en".into(),
            images: vec![LayoutImage {
                id: "owned-1".into(),
                name: "screen.png".into(),
                width: 256,
                height: 144,
                modified_at: None,
            }],
            columns: 0,
            ordering: ImageOrdering::Selected,
        }
    }
    fn manifest() -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"sheet","name":"Sheet","publisher":"test","version":"0.1.0","api_version":"0.1.0","capabilities":["image-layout"],"file_extensions":[],"permissions":[{"permission":"launcher:images/selected/read"},{"permission":"launcher:images/output/propose"}],"component_sha256":"0".repeat(64)})).unwrap()
    }
    #[test]
    fn image_layout_requires_exact_workflow_and_selected_metadata_bounds() {
        let manifest = manifest();
        request().validate(&manifest).unwrap();
        let mut request = request();
        request.images[0].name = "../private.png".into();
        assert!(request.validate(&manifest).is_err());
        request.images[0].name = "screen.png".into();
        request.images[0].width = 257;
        assert!(request.validate(&manifest).is_err());
        request.images[0].width = 256;
        request.images.push(request.images[0].clone());
        assert!(request.validate(&manifest).is_err());
    }
    #[test]
    fn image_layout_output_cannot_select_other_resources_or_paths() {
        let mut sheet = ImageSheet {
            columns: 1,
            gap: 12,
            background: ImageRgb {
                red: 24,
                green: 24,
                blue: 24,
            },
            image_ids: vec!["owned-1".into()],
        };
        sheet.validate(&request()).unwrap();
        sheet.image_ids[0] = "foreign".into();
        assert!(sheet.validate(&request()).is_err());
        sheet.image_ids[0] = "owned-1".into();
        sheet.gap = 33;
        assert!(sheet.validate(&request()).is_err());
        assert!(serde_json::from_value::<ImageSheet>(serde_json::json!({"columns":1,"gap":0,"background":{"red":0,"green":0,"blue":0},"image_ids":["owned-1"],"path":"/tmp/out.png"})).is_err());
    }
}

// SPDX-License-Identifier: MIT
//! Public provider artwork only. No API key, renderer remote host, or active image format.
use base64::{Engine, engine::general_purpose::STANDARD};
use cobble_net::{Broker, BrokerConfig};
use image::{ImageFormat, imageops::FilterType};
use serde::Deserialize;
use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
    time::Duration,
};
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Icon,
    Preview,
}
fn normalize(bytes: &[u8], variant: Variant) -> Result<String, String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Image exceeds byte budget".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "Unsupported image")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("Unsupported image".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| "Invalid image")?;
    let (width, height) = match variant {
        Variant::Icon => (128, 128),
        Variant::Preview => (960, 600),
    };
    let image = image.resize(width, height, FilterType::Triangle);
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| "Invalid image")?;
    let bytes = output.into_inner();
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Image exceeds output budget".into());
    }
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}
#[tauri::command]
pub async fn get_market_image(
    provider: String,
    url: String,
    variant: Variant,
) -> Result<String, String> {
    if !cobble_content::media::valid_image_url(&provider, &url) {
        return Err("Unsupported image URL".into());
    }
    static BROKER: OnceLock<Result<Broker, cobble_net::NetError>> = OnceLock::new();
    static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = LIMIT
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Image unavailable")?;
    let broker = BROKER
        .get_or_init(|| {
            Broker::new(BrokerConfig {
                max_redirects: 0,
                total_timeout: Duration::from_secs(20),
                ..Default::default()
            })
        })
        .as_ref()
        .map_err(|_| "Image unavailable")?;
    let response = broker
        .get_public_metadata(&url, "market-public-images-v1", 8 * 1024 * 1024)
        .await
        .map_err(|_| "Image unavailable")?;
    if response.metadata.status != 200 {
        return Err("Image unavailable".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        normalize(&response.body, variant)
    })
    .await
    .map_err(|_| "Image unavailable")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn images_are_inert_bounded_thumbnails() {
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(320, 160)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let value = normalize(png.get_ref(), Variant::Icon).unwrap();
        let bytes = STANDARD
            .decode(value.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (128, 64));
        assert!(
            normalize(
                b"<svg xmlns='http://www.w3.org/2000/svg'></svg>",
                Variant::Preview
            )
            .is_err()
        );
        assert!(normalize(&vec![0; 8 * 1024 * 1024 + 1], Variant::Icon).is_err());
    }
}

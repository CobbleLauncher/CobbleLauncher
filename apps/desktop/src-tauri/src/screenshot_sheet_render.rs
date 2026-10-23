// SPDX-License-Identifier: MIT
use super::*;
use image::{
    DynamicImage, ImageFormat, Rgba,
    imageops::{FilterType, overlay},
};
use std::io::Cursor;
pub(super) fn render(
    sources: &[Source],
    request: &ImageLayoutRequest,
    sheet: &ImageSheet,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, u32, u32), String> {
    sheet.validate(request).map_err(|_| "SHEET_RENDER_FAILED")?;
    let columns = u32::from(sheet.columns);
    let rows = (sources.len() as u32).div_ceil(columns);
    let gap = u32::from(sheet.gap);
    let cell = 256
        .min((2048 - gap * (columns + 1)) / columns)
        .min((2048 - gap * (rows + 1)) / rows);
    let width = cell * columns + gap * (columns + 1);
    let height = cell * rows + gap * (rows + 1);
    if cell == 0 || width > 2048 || height > 2048 {
        return Err("SHEET_RENDER_FAILED".into());
    }
    let color = Rgba([
        sheet.background.red,
        sheet.background.green,
        sheet.background.blue,
        255,
    ]);
    let mut output = RgbaImage::from_pixel(width, height, color);
    for (index, id) in sheet.image_ids.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("CANCEL_REQUESTED".into());
        }
        let source = sources
            .iter()
            .find(|s| &s.descriptor.id == id)
            .ok_or("SHEET_RENDER_FAILED")?;
        let fitted = DynamicImage::ImageRgba8(source.pixels.clone())
            .resize(cell, cell, FilterType::Triangle)
            .to_rgba8();
        let x = gap + (index as u32 % columns) * (cell + gap) + (cell - fitted.width()) / 2;
        let y = gap + (index as u32 / columns) * (cell + gap) + (cell - fitted.height()) / 2;
        overlay(&mut output, &fitted, i64::from(x), i64::from(y));
    }
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(output)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|_| "SHEET_RENDER_FAILED")?;
    let bytes = bytes.into_inner();
    if bytes.len() > MAX_PNG {
        return Err("SHEET_RENDER_FAILED".into());
    }
    Ok((bytes, width, height))
}

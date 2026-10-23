// SPDX-License-Identifier: MIT
//! Local, bounded cube-face previews. This does not implement Minecraft block geometry.
use base64::Engine;
use cap_std::fs::Dir;
use image::{GenericImageView, ImageFormat, ImageReader, imageops::FilterType};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tauri::{AppHandle, WebviewWindow};
const PALETTE_LIMIT: usize = 1024;
const TEXTURE_LIMIT: usize = 256;
const FILE_LIMIT: u64 = 2 * 1024 * 1024;
const READ_BUDGET: u64 = 32 * 1024 * 1024;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    id: String,
    data_url: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureSet {
    source: &'static str,
    textures: Vec<Texture>,
    palette_faces: Vec<[Option<String>; 6]>,
    resolved_blocks: usize,
}
enum Source {
    Folder(Dir),
    Archive(zip::ZipArchive<File>),
}
struct Assets {
    sources: Vec<Source>,
    remaining: u64,
}
fn resource(value: &str) -> Option<(String, String)> {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    if namespace.is_empty()
        || matches!(namespace, "." | "..")
        || path.is_empty()
        || namespace.len() > 64
        || path.len() > 200
        || !namespace.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        })
        || path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
                })
        })
    {
        return None;
    }
    Some((namespace.into(), path.into()))
}
fn asset_path(id: &str, kind: &str, extension: &str) -> Option<String> {
    let (ns, name) = resource(id)?;
    Some(format!("assets/{ns}/{kind}/{name}.{extension}"))
}
impl Assets {
    fn bytes(&mut self, path: &str) -> Option<Vec<u8>> {
        for source in self.sources.iter_mut().rev() {
            let mut output = Vec::new();
            let success = match source {
                Source::Folder(dir) => match dir.open(path) {
                    Ok(file) => {
                        let len = file.metadata().ok()?.len();
                        if len > FILE_LIMIT || len > self.remaining {
                            return None;
                        }
                        self.remaining = self.remaining.saturating_sub(len);
                        file.take(FILE_LIMIT + 1).read_to_end(&mut output).is_ok()
                    }
                    Err(_) => false,
                },
                Source::Archive(archive) => match archive.by_name(path) {
                    Ok(file) => {
                        let len = file.size();
                        if len > FILE_LIMIT || len > self.remaining {
                            return None;
                        }
                        self.remaining = self.remaining.saturating_sub(len);
                        file.take(FILE_LIMIT + 1).read_to_end(&mut output).is_ok()
                    }
                    Err(_) => false,
                },
            };
            if success && output.len() as u64 <= FILE_LIMIT {
                return Some(output);
            }
        }
        None
    }
    fn json(&mut self, id: &str, kind: &str) -> Option<Value> {
        let path = asset_path(id, kind, "json")?;
        let bytes = self.bytes(&path)?;
        if bytes.len() > 256 * 1024 {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    }
}
fn archive(file: cap_std::fs::File) -> Option<Source> {
    let length = file.metadata().ok()?.len();
    if length > 512 * 1024 * 1024 {
        return None;
    }
    // Bound central-directory allocation before handing an archive to the ZIP parser.
    // ZIP64 packs are outside this preview's supported bounds.
    let mut file = file.into_std();
    let tail_len = length.min(65_557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64))).ok()?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail).ok()?;
    let start = (0..tail.len().saturating_sub(21)).rev().find(|offset| {
        tail[*offset..].starts_with(b"PK\x05\x06")
            && tail.get(*offset + 20..*offset + 22).is_some_and(|c| {
                *offset + 22 + u16::from_le_bytes([c[0], c[1]]) as usize == tail.len()
            })
    })?;
    let eocd = &tail[start..];
    let count = u16::from_le_bytes([eocd[10], eocd[11]]);
    let size = u32::from_le_bytes(eocd[12..16].try_into().ok()?);
    if eocd[4..8] != [0, 0, 0, 0] || count == u16::MAX || count > 50_000 || size > 8 * 1024 * 1024 {
        return None;
    }
    file.seek(SeekFrom::Start(0)).ok()?;
    let zip = zip::ZipArchive::new(file).ok()?;
    (zip.len() <= 50_000).then_some(Source::Archive(zip))
}
fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && !matches!(value, "." | "..")
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}
fn sources(directory: &Path, version: &str) -> Assets {
    let mut assets = Assets {
        sources: vec![],
        remaining: READ_BUDGET,
    };
    let Ok(game) = Dir::open_ambient_dir(directory, cap_std::ambient_authority()) else {
        return assets;
    };
    if component(version)
        && let Ok(file) = game.open(format!("versions/{version}/{version}.jar"))
        && let Some(source) = archive(file)
    {
        assets.sources.push(source)
    }
    let selected = game
        .open("options.txt")
        .ok()
        .and_then(|file| {
            if file.metadata().ok()?.len() > 1024 * 1024 {
                return None;
            }
            let mut text = String::new();
            file.take(1024 * 1024 + 1).read_to_string(&mut text).ok()?;
            text.lines()
                .find_map(|line| line.strip_prefix("resourcePacks:"))
                .and_then(|line| serde_json::from_str::<Vec<String>>(line).ok())
        })
        .unwrap_or_default();
    for selected in selected.iter().take(16) {
        let Some(name) = selected
            .strip_prefix("file/")
            .filter(|name| component(name))
        else {
            continue;
        };
        let path = format!("resourcepacks/{name}");
        if let Ok(dir) = game.open_dir(&path) {
            assets.sources.push(Source::Folder(dir))
        } else if let Ok(file) = game.open(&path)
            && let Some(source) = archive(file)
        {
            assets.sources.push(source)
        }
    }
    assets
}
fn state(value: &str) -> Option<(String, HashMap<String, String>)> {
    let (id, props) = match value.split_once('[') {
        Some((id, tail)) => (id, Some(tail.strip_suffix(']')?)),
        None => (value, None),
    };
    resource(id)?;
    let mut properties = HashMap::new();
    if let Some(props) = props {
        if props.len() > 1024 {
            return None;
        }
        for item in props.split(',').take(32) {
            let (k, v) = item.split_once('=')?;
            properties.insert(k.into(), v.into());
        }
    }
    Some((id.into(), properties))
}
fn condition(value: &Value, props: &HashMap<String, String>, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    let Some(object) = value.as_object() else {
        return false;
    };
    object.iter().all(|(key, value)| match key.as_str() {
        "OR" => value
            .as_array()
            .is_some_and(|a| a.len() <= 32 && a.iter().any(|v| condition(v, props, depth + 1))),
        "AND" => value
            .as_array()
            .is_some_and(|a| a.len() <= 32 && a.iter().all(|v| condition(v, props, depth + 1))),
        _ => value.as_str().is_some_and(|pattern| {
            props
                .get(key)
                .is_some_and(|actual| pattern.split('|').any(|part| part == actual))
        }),
    })
}
fn variant(blockstate: &Value, props: &HashMap<String, String>) -> Option<Value> {
    if let Some(variants) = blockstate["variants"].as_object() {
        for (key, value) in variants.iter().take(256) {
            if key.is_empty()
                || key.split(',').all(|pair| {
                    pair.split_once('=')
                        .is_some_and(|(k, v)| props.get(k).is_some_and(|actual| actual == v))
                })
            {
                return Some(
                    value
                        .as_array()
                        .and_then(|a| a.first())
                        .unwrap_or(value)
                        .clone(),
                );
            }
        }
    }
    for part in blockstate["multipart"].as_array()?.iter().take(64) {
        if part["when"].is_null() || condition(&part["when"], props, 0) {
            let value = &part["apply"];
            return Some(
                value
                    .as_array()
                    .and_then(|a| a.first())
                    .unwrap_or(value)
                    .clone(),
            );
        }
    }
    None
}
fn model(assets: &mut Assets, id: &str, seen: &mut Vec<String>) -> Option<Value> {
    if seen.len() >= 12 || seen.iter().any(|v| v == id) {
        return None;
    }
    seen.push(id.into());
    let child = assets.json(id, "models")?;
    let mut merged = child
        .get("parent")
        .and_then(Value::as_str)
        .and_then(|parent| model(assets, parent, seen))
        .unwrap_or_else(|| serde_json::json!({}));
    let object = child.as_object()?;
    for (key, value) in object {
        if key == "textures" {
            if !merged["textures"].is_object() {
                merged["textures"] = serde_json::json!({})
            }
            for (k, v) in value.as_object()?.iter().take(128) {
                merged["textures"][k] = v.clone()
            }
        } else {
            merged[key] = value.clone()
        }
    }
    Some(merged)
}
fn texture_ref(value: &str, textures: &Value) -> Option<String> {
    let mut next = value;
    let mut seen = Vec::new();
    for _ in 0..16 {
        if let Some(key) = next.strip_prefix('#') {
            if seen.contains(&key) {
                return None;
            }
            seen.push(key);
            next = textures[key].as_str()?
        } else {
            let (ns, path) = resource(next)?;
            return Some(format!("{ns}:{path}"));
        }
    }
    None
}
const FACE_NAMES: [&str; 6] = ["east", "west", "up", "down", "south", "north"];
fn faces(model: &Value) -> [Option<String>; 6] {
    let textures = &model["textures"];
    std::array::from_fn(|face| {
        if let Some(elements) = model["elements"].as_array() {
            for element in elements.iter().take(64) {
                if let Some(value) = element["faces"][FACE_NAMES[face]]["texture"].as_str() {
                    return texture_ref(value, textures);
                }
            }
        }
        let keys: &[&str] = match face {
            2 => &["top", "end", "all", "side", "particle"],
            3 => &["bottom", "end", "all", "side", "particle"],
            _ => &["side", "all", "particle"],
        };
        keys.iter().find_map(|key| {
            textures[*key]
                .as_str()
                .and_then(|value| texture_ref(value, textures))
        })
    })
}
fn rotate(mut faces: [Option<String>; 6], x: i64, y: i64) -> [Option<String>; 6] {
    // Quarter-turn blockstate orientation. UV rotations and noncube geometry remain out of scope.
    if x % 90 != 0 || y % 90 != 0 {
        return std::array::from_fn(|_| None);
    }
    for _ in 0..x.rem_euclid(360) / 90 {
        let old = faces;
        faces = [
            old[0].clone(),
            old[1].clone(),
            old[4].clone(),
            old[5].clone(),
            old[3].clone(),
            old[2].clone(),
        ]
    }
    for _ in 0..y.rem_euclid(360) / 90 {
        let old = faces;
        faces = [
            old[5].clone(),
            old[4].clone(),
            old[2].clone(),
            old[3].clone(),
            old[0].clone(),
            old[1].clone(),
        ]
    }
    faces
}
fn sanitize_png(bytes: &[u8]) -> Option<String> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    let (w, h) = image.dimensions();
    if w == 0 || h < w || h % w != 0 {
        return None;
    }
    let first = image
        .crop_imm(0, 0, w, w)
        .resize_exact(w.min(64), w.min(64), FilterType::Nearest);
    let mut clean = Cursor::new(Vec::new());
    first.write_to(&mut clean, ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(clean.into_inner())
    ))
}
fn collect(mut assets: Assets, palette: &[String]) -> TextureSet {
    let mut textures = Vec::new();
    let mut loaded: HashMap<String, bool> = HashMap::new();
    let mut palette_faces = Vec::new();
    let mut resolved_blocks = 0;
    for block in palette.iter().take(PALETTE_LIMIT) {
        let resolved = state(block).and_then(|(id, props)| {
            let blockstate = assets.json(&id, "blockstates")?;
            let choice = variant(&blockstate, &props)?;
            let id = choice["model"].as_str()?;
            let m = model(&mut assets, id, &mut vec![])?;
            Some(rotate(
                faces(&m),
                choice["x"].as_i64().unwrap_or(0),
                choice["y"].as_i64().unwrap_or(0),
            ))
        });
        let mut face_ids = resolved.unwrap_or_default();
        for face in &mut face_ids {
            if let Some(id) = face.clone() {
                let good = if let Some(good) = loaded.get(&id) {
                    *good
                } else if textures.len() < TEXTURE_LIMIT {
                    let data = asset_path(&id, "textures", "png")
                        .and_then(|path| assets.bytes(&path))
                        .and_then(|bytes| sanitize_png(&bytes));
                    let good = data.is_some();
                    if let Some(data_url) = data {
                        textures.push(Texture {
                            id: id.clone(),
                            data_url,
                        })
                    }
                    loaded.insert(id.clone(), good);
                    good
                } else {
                    false
                };
                if !good {
                    *face = None
                }
            }
        }
        if face_ids.iter().any(Option::is_some) {
            resolved_blocks += 1
        }
        palette_faces.push(face_ids);
    }
    palette_faces.resize_with(palette.len(), || std::array::from_fn(|_| None));
    TextureSet {
        source: if textures.is_empty() { "none" } else { "game" },
        textures,
        palette_faces,
        resolved_blocks,
    }
}
#[tauri::command]
pub async fn load_viewer_textures(
    app: AppHandle,
    window: WebviewWindow,
    webview: tauri::Webview,
    resource_id: String,
    game_id: String,
) -> Result<TextureSet, String> {
    if window.label() != "main" || webview.label() != "main" {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    let palette = crate::plugins::viewer_texture_palette(&resource_id)?;
    let directory = crate::games::game_directory(&app, &game_id)?;
    let version = crate::games::registered_game_version(&app, &game_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        collect(sources(&directory, &version), &palette)
    })
    .await
    .map_err(|_| "VIEWER_TEXTURES_UNAVAILABLE".to_owned())?;
    // A closed or revoked viewer may not receive a delayed response.
    crate::plugins::viewer_texture_palette(&resource_id)?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn asset_ids_and_pack_names_cannot_escape() {
        for id in [
            "../secret",
            "x:../a",
            "x:/a",
            "x:a\\b",
            "x:a//b",
            "https://host/a",
        ] {
            assert!(resource(id).is_none(), "{id}")
        }
        assert!(!component("../secret"));
        assert!(!component("C:secret"));
        assert_eq!(
            asset_path("minecraft:block/stone", "textures", "png").unwrap(),
            "assets/minecraft/textures/block/stone.png"
        );
    }
    #[test]
    fn variants_properties_inheritance_and_cycles_are_bounded() {
        let props = HashMap::from([("axis".into(), "y".into())]);
        let v = variant(
            &json!({"variants":{"axis=x":{"model":"x"},"axis=y":{"model":"y"}}}),
            &props,
        )
        .unwrap();
        assert_eq!(v["model"], "y");
        assert_eq!(
            texture_ref(
                "#side",
                &json!({"side":"#all","all":"minecraft:block/stone"})
            )
            .unwrap(),
            "minecraft:block/stone"
        );
        assert!(texture_ref("#a", &json!({"a":"#b","b":"#a"})).is_none());
        let f = faces(
            &json!({"textures":{"side":"minecraft:block/oak_log","end":"minecraft:block/oak_log_top"}}),
        );
        assert_eq!(f[0].as_deref(), Some("minecraft:block/oak_log"));
        assert_eq!(f[2].as_deref(), Some("minecraft:block/oak_log_top"));
    }
    #[test]
    fn png_is_reencoded_and_animated_strip_uses_first_frame() {
        let image = image::RgbaImage::from_fn(16, 32, |_, y| {
            if y < 16 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        });
        let mut bytes = Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let data = sanitize_png(bytes.get_ref()).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(data.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&raw).unwrap();
        assert_eq!(decoded.dimensions(), (16, 16));
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert!(sanitize_png(b"bad").is_none());
    }
    #[test]
    fn local_pack_overrides_vanilla_and_unknown_blocks_fallback() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        std::fs::create_dir_all(dir.join("resourcepacks/Chosen/assets/minecraft/blockstates"))
            .unwrap();
        std::fs::write(dir.join("options.txt"), "resourcePacks:[\"file/Chosen\"]\n").unwrap();
        std::fs::write(
            dir.join("resourcepacks/Chosen/assets/minecraft/blockstates/stone.json"),
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        )
        .unwrap();
        let mut a = sources(dir, "1.21.1");
        assert_eq!(
            a.json("minecraft:stone", "blockstates").unwrap()["variants"][""]["model"],
            "minecraft:block/stone"
        );
        let result = collect(a, &["minecraft:missing".into()]);
        assert_eq!(result.source, "none");
        assert!(result.palette_faces[0].iter().all(Option::is_none));
    }
    #[test]
    fn actual_local_jar_parent_model_and_selected_zip_texture_are_combined() {
        use std::io::Write;
        fn png(color: [u8; 4]) -> Vec<u8> {
            let mut output = Cursor::new(vec![]);
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                16,
                16,
                image::Rgba(color),
            ))
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
            output.into_inner()
        }
        fn zip(path: &Path, entries: Vec<(&str, Vec<u8>)>) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
            for (name, bytes) in entries {
                zip.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
                zip.write_all(&bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        let root = tempfile::tempdir().unwrap();
        zip(&root.path().join("versions/1.21.1/1.21.1.jar"),vec![
            ("assets/minecraft/blockstates/stone.json",br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#.to_vec()),
            ("assets/minecraft/models/block/stone.json",br##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##.to_vec()),
            ("assets/minecraft/models/block/cube_all.json",br##"{"textures":{"particle":"#all"},"elements":[{"faces":{"east":{"texture":"#all"},"west":{"texture":"#all"},"up":{"texture":"#all"},"down":{"texture":"#all"},"south":{"texture":"#all"},"north":{"texture":"#all"}}}]}"##.to_vec()),
            ("assets/minecraft/textures/block/stone.png",png([255,0,0,255])),
        ]);
        zip(
            &root.path().join("resourcepacks/Chosen.zip"),
            vec![(
                "assets/minecraft/textures/block/stone.png",
                png([0, 255, 0, 255]),
            )],
        );
        std::fs::write(
            root.path().join("options.txt"),
            "resourcePacks:[\"vanilla\",\"file/Chosen.zip\"]\n",
        )
        .unwrap();
        let mut palette = vec!["minecraft:stone".into(); PALETTE_LIMIT + 1];
        palette[1] = "minecraft:unknown".into();
        let result = collect(sources(root.path(), "1.21.1"), &palette);
        assert_eq!(result.source, "game");
        assert_eq!(result.textures.len(), 1);
        assert_eq!(result.palette_faces.len(), palette.len());
        assert!(result.palette_faces[1].iter().all(Option::is_none));
        assert!(
            result.palette_faces[PALETTE_LIMIT]
                .iter()
                .all(Option::is_none)
        );
        assert!(
            result.palette_faces[0]
                .iter()
                .all(|v| v.as_deref() == Some("minecraft:block/stone"))
        );
        let png = base64::engine::general_purpose::STANDARD
            .decode(
                result.textures[0]
                    .data_url
                    .strip_prefix("data:image/png;base64,")
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            image::load_from_memory(&png)
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [0, 255, 0, 255]
        );
    }
}

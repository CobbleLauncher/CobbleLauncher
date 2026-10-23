// SPDX-License-Identifier: MIT
//! Original bounded Sponge v1/v2/v3 and Litematic v4–v7 readers, using permissive fastnbt.
mod litematic;
use cobble_plugin_api::{
    MAX_BLOCKS, MaterialCount, PluginError, SceneBlock, SceneChunk, ViewerScene,
};
use fastnbt::Value;
use std::{collections::HashMap, io::Read};

pub struct ParsedScene {
    pub summary: ViewerScene,
    pub chunks: Vec<SceneChunk>,
}
pub fn inspect_bytes(data: &[u8]) -> Result<ViewerScene, PluginError> {
    Ok(parse_scene(data)?.summary)
}
pub fn parse_scene(data: &[u8]) -> Result<ParsedScene, PluginError> {
    let invalid = |message: &str| PluginError::new("VIEWER_INVALID_FILE", message);
    if data.len() > cobble_plugin_api::MAX_FILE_BYTES {
        return Err(invalid("Schematic exceeds 8 MiB"));
    }
    let mut decoded = Vec::new();
    if data.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(data)
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| invalid("Invalid gzip schematic"))?;
        if decoded.len() > 16 * 1024 * 1024 {
            return Err(invalid("Expanded schematic exceeds 16 MiB"));
        }
    } else {
        decoded.extend_from_slice(data);
    }
    let root: HashMap<String, Value> =
        fastnbt::from_bytes(&decoded).map_err(|_| invalid("Invalid NBT schematic"))?;
    if root.contains_key("Regions") {
        return litematic::parse(&root);
    }
    let root = match root.get("Schematic") {
        Some(Value::Compound(inner)) => inner,
        _ => &root,
    };
    let number = |map: &HashMap<String, Value>, key: &str| -> Result<u32, PluginError> {
        match map.get(key) {
            Some(Value::Int(n)) if *n >= 0 => Ok(*n as u32),
            Some(Value::Short(n)) => Ok(*n as u16 as u32),
            _ => Err(invalid("Missing numeric schematic field")),
        }
    };
    let version = number(root, "Version")?;
    if !matches!(version, 1..=3) {
        return Err(PluginError::new(
            "VIEWER_UNSUPPORTED_VERSION",
            "Supported Sponge schematic versions: 1, 2 and 3",
        ));
    }
    let (width, height, length) = (
        number(root, "Width")?,
        number(root, "Height")?,
        number(root, "Length")?,
    );
    let volume = (width as usize)
        .checked_mul(height as usize)
        .and_then(|v| v.checked_mul(length as usize))
        .filter(|v| *v > 0 && *v <= MAX_BLOCKS)
        .ok_or_else(|| invalid("Preview supports 1–1048576 blocks"))?;
    let blocks = if version == 3 {
        match root.get("Blocks") {
            Some(Value::Compound(blocks)) => blocks,
            _ => return Err(invalid("Missing Blocks compound")),
        }
    } else {
        root
    };
    let Some(Value::Compound(palette_map)) = blocks.get("Palette") else {
        return Err(PluginError::new(
            "VIEWER_UNSUPPORTED_VERSION",
            "A local block palette is required; global numeric block registries are unsupported",
        ));
    };
    if palette_map.is_empty() || palette_map.len() > 4096 {
        return Err(invalid("Invalid palette size"));
    }
    // The v1 specification permits sparse local palette indexes. Compact them
    // deterministically for the renderer without interpreting global numeric IDs.
    let mut encoded_palette = std::collections::BTreeMap::new();
    for (state, value) in palette_map {
        let Value::Int(index) = value else {
            return Err(invalid("Invalid palette index"));
        };
        if *index < 0
            || state.is_empty()
            || state.len() > 512
            || encoded_palette
                .insert(
                    *index as u32,
                    if state.split('[').next().unwrap_or(state).contains(':') {
                        state.clone()
                    } else {
                        format!("minecraft:{state}")
                    },
                )
                .is_some()
        {
            return Err(invalid("Invalid or duplicate palette index"));
        }
    }
    let encoded_ids: HashMap<u32, usize> = encoded_palette
        .keys()
        .enumerate()
        .map(|(dense, encoded)| (*encoded, dense))
        .collect();
    let palette = encoded_palette.into_values().collect::<Vec<_>>();
    let Some(Value::ByteArray(bytes)) = blocks.get(if version == 3 { "Data" } else { "BlockData" })
    else {
        return Err(invalid("Missing block data"));
    };
    let mut cursor = 0;
    let mut counts = vec![0u32; palette.len()];
    let mut chunks: Vec<SceneChunk> = Vec::new();
    for index in 0..volume {
        let mut id = 0u32;
        let mut complete = false;
        for shift in (0..35).step_by(7) {
            let byte = *bytes
                .get(cursor)
                .ok_or_else(|| invalid("Truncated block data"))? as u8;
            cursor += 1;
            if shift == 28 && byte > 15 {
                return Err(invalid("Overflowing block varint"));
            }
            id |= u32::from(byte & 127) << shift;
            if byte & 128 == 0 {
                complete = true;
                break;
            }
        }
        if !complete || !encoded_ids.contains_key(&id) {
            return Err(invalid("Invalid block palette reference"));
        }
        let id = encoded_ids[&id] as u32;
        let state = &palette[id as usize];
        if !matches!(
            state.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        ) {
            counts[id as usize] += 1;
            let index = index as u32;
            if chunks.last().is_none_or(|chunk| chunk.blocks.len() == 1024) {
                chunks.push(SceneChunk {
                    blocks: Vec::with_capacity(1024),
                });
            }
            chunks
                .last_mut()
                .expect("chunk allocated")
                .blocks
                .push(SceneBlock {
                    x: index % width,
                    y: index / (width * length),
                    z: (index / width) % length,
                    palette_index: id,
                });
        }
    }
    if cursor != bytes.len() {
        return Err(invalid("Trailing block data"));
    }
    let materials = palette
        .iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(state, count)| MaterialCount {
            state: state.clone(),
            count,
        })
        .collect();
    let scene = ViewerScene {
        format_version: version,
        width,
        height,
        length,
        palette,
        materials,
        chunk_count: chunks.len() as u32,
    };
    scene.validate()?;
    Ok(ParsedScene {
        summary: scene,
        chunks,
    })
}

#[cfg(target_arch = "wasm32")]
mod component {
    wit_bindgen::generate!({path:"../../contracts",world:"viewer-plugin"});
    struct SchematicViewer;
    static SCENE: std::sync::Mutex<Option<super::ParsedScene>> = std::sync::Mutex::new(None);
    impl exports::launcher::extensions::viewer::Guest for SchematicViewer {
        fn inspect(
            data: Vec<u8>,
        ) -> Result<
            exports::launcher::extensions::viewer::Scene,
            launcher::extensions::types::PluginError,
        > {
            use exports::launcher::extensions::viewer::{MaterialCount, Scene};
            let parsed = super::parse_scene(&data).map_err(|error| {
                launcher::extensions::types::PluginError {
                    code: error.code,
                    message: error.message,
                }
            })?;
            let scene = parsed.summary.clone();
            *SCENE.lock().unwrap() = Some(parsed);
            Ok(Scene {
                format_version: scene.format_version,
                width: scene.width,
                height: scene.height,
                length: scene.length,
                palette: scene.palette,
                materials: scene
                    .materials
                    .into_iter()
                    .map(|m| MaterialCount {
                        state: m.state,
                        count: m.count,
                    })
                    .collect(),
                chunk_count: scene.chunk_count,
            })
        }
        fn read_chunk(
            index: u32,
        ) -> Result<
            exports::launcher::extensions::viewer::Chunk,
            launcher::extensions::types::PluginError,
        > {
            use exports::launcher::extensions::viewer::{Block, Chunk};
            let guard = SCENE.lock().unwrap();
            let chunk = guard
                .as_ref()
                .and_then(|scene| scene.chunks.get(index as usize))
                .ok_or_else(|| launcher::extensions::types::PluginError {
                    code: "VIEWER_INVALID_CHUNK".into(),
                    message: "Chunk is outside this scene".into(),
                })?;
            Ok(Chunk {
                blocks: chunk
                    .blocks
                    .iter()
                    .map(|b| Block {
                        x: b.x,
                        y: b.y,
                        z: b.z,
                        palette_index: b.palette_index,
                    })
                    .collect(),
            })
        }
    }
    export!(SchematicViewer);
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn fixture(version: i32) -> Vec<u8> {
        let palette = Value::Compound(HashMap::from([
            ("minecraft:air".into(), Value::Int(0)),
            ("minecraft:stone".into(), Value::Int(1)),
        ]));
        let mut root = HashMap::from([
            ("Version".to_owned(), Value::Int(version)),
            ("Width".into(), Value::Short(2)),
            ("Height".into(), Value::Short(1)),
            ("Length".into(), Value::Short(2)),
        ]);
        if version == 3 {
            root.insert(
                "Blocks".into(),
                Value::Compound(HashMap::from([
                    ("Palette".into(), palette),
                    (
                        "Data".into(),
                        Value::ByteArray(fastnbt::ByteArray::new(vec![1, 0, 1, 1])),
                    ),
                ])),
            );
            fastnbt::to_bytes(&HashMap::from([("Schematic", Value::Compound(root))])).unwrap()
        } else {
            root.insert("Palette".into(), palette);
            root.insert(
                "BlockData".into(),
                Value::ByteArray(fastnbt::ByteArray::new(vec![1, 0, 1, 1])),
            );
            fastnbt::to_bytes(&root).unwrap()
        }
    }
    #[test]
    fn reads_v1_v2_v3_materials_and_coordinates() {
        for version in [1, 2, 3] {
            let parsed = parse_scene(&fixture(version)).unwrap();
            let scene = parsed.summary;
            assert_eq!(scene.materials[0].state, "minecraft:stone");
            assert_eq!(scene.materials[0].count, 3);
            assert_eq!(parsed.chunks[0].blocks[2].z, 1);
        }
    }
    #[test]
    fn sponge_v1_sparse_palette_and_global_registry_rejection() {
        let mut root: HashMap<String, Value> = fastnbt::from_bytes(&fixture(1)).unwrap();
        root.insert(
            "Palette".into(),
            Value::Compound(HashMap::from([
                ("air".into(), Value::Int(0)),
                ("stone".into(), Value::Int(128)),
            ])),
        );
        root.insert(
            "BlockData".into(),
            Value::ByteArray(fastnbt::ByteArray::new(vec![-128, 1, 0, -128, 1, -128, 1])),
        );
        let parsed = parse_scene(&fastnbt::to_bytes(&root).unwrap()).unwrap();
        assert_eq!(parsed.summary.format_version, 1);
        assert_eq!(parsed.summary.materials[0].count, 3);
        root.remove("Palette");
        assert_eq!(
            parse_scene(&fastnbt::to_bytes(&root).unwrap())
                .err()
                .unwrap()
                .code,
            "VIEWER_UNSUPPORTED_VERSION"
        );
    }
    #[test]
    fn rejects_legacy_and_corrupted_inputs() {
        assert!(inspect_bytes(&fixture(4)).is_err());
        assert!(inspect_bytes(b"not nbt").is_err());
    }
}

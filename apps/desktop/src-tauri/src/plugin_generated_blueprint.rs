// SPDX-License-Identifier: MIT
//! Restricted generated geometry; no paths, NBT input, game writes or network.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use fastnbt::Value;
use serde_json::json;
use std::fs;
use std::{collections::HashMap, io::Write};
use tauri::{AppHandle, Emitter};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRequest {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub length: u16,
    pub block: String,
    pub occupancy: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRequest {
    pub blueprint_id: String,
}
fn encode(args: &SaveRequest) -> Result<Vec<u8>, String> {
    let invalid = || "GENERATED_BLUEPRINT_INVALID".to_string();
    if ![args.width, args.height, args.length]
        .iter()
        .all(|n| (1..=128).contains(n))
        || args.name.trim().is_empty()
        || args.name.chars().count() > 80
        || args.name.chars().any(char::is_control)
        || ![
            "minecraft:stone",
            "minecraft:cobblestone",
            "minecraft:glass",
            "minecraft:oak_planks",
            "minecraft:white_concrete",
        ]
        .contains(&args.block.as_str())
    {
        return Err(invalid());
    }
    let volume = usize::from(args.width) * usize::from(args.height) * usize::from(args.length);
    if volume > cobble_plugin_api::MAX_BLOCKS {
        return Err(invalid());
    }
    let size = volume.div_ceil(8);
    if args.occupancy.len() != 4 * size.div_ceil(3) {
        return Err(invalid());
    }
    let bits = STANDARD.decode(&args.occupancy).map_err(|_| invalid())?;
    if bits.len() != size || (volume % 8 != 0 && bits[size - 1] >> (volume % 8) != 0) {
        return Err(invalid());
    }
    let count: usize = bits.iter().map(|n| n.count_ones() as usize).sum();
    if count == 0 || count > 262_144 {
        return Err(invalid());
    }
    // Sponge v2: x changes fastest, then z, then y. Palette indices are one-byte VarInts.
    let blocks = (0..volume)
        .map(|i| i8::from(bits[i / 8] & (1 << (i % 8)) != 0))
        .collect();
    let root = HashMap::from([
        ("Version", Value::Int(2)),
        ("DataVersion", Value::Int(3465)),
        ("Width", Value::Short(args.width as i16)),
        ("Height", Value::Short(args.height as i16)),
        ("Length", Value::Short(args.length as i16)),
        (
            "Offset",
            Value::IntArray(fastnbt::IntArray::new(vec![0, 0, 0])),
        ),
        ("PaletteMax", Value::Int(2)),
        (
            "Palette",
            Value::Compound(HashMap::from([
                ("minecraft:air".into(), Value::Int(0)),
                (args.block.clone(), Value::Int(1)),
            ])),
        ),
        (
            "BlockData",
            Value::ByteArray(fastnbt::ByteArray::new(blocks)),
        ),
        ("BlockEntities", Value::List(vec![])),
        ("Entities", Value::List(vec![])),
    ]);
    let nbt = fastnbt::to_bytes(&root).map_err(|_| invalid())?;
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&nbt).map_err(|_| invalid())?;
    gzip.finish().map_err(|_| invalid())
}
pub fn save(plugin_id: &str, args: SaveRequest) -> Result<serde_json::Value, String> {
    let bytes = encode(&args)?;
    let directory = root("blueprints").map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    fs::create_dir_all(&directory).map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    let blueprint = Blueprint {
        id: next_id(),
        name: args.name.trim().into(),
        plugin_id: String::new(),
        format: BlueprintFormat::Schem,
        favorite: false,
        has_thumbnail: false,
        source: Some(BlueprintSource {
            provider: format!("generated:{plugin_id}"),
            id: String::new(),
            version: 1,
            author: String::new(),
            attribution: String::new(),
            license: None,
        }),
    };
    let data_path = directory.join(format!("{}.schem", blueprint.id));
    let metadata_path = directory.join(format!("{}.json", blueprint.id));
    let persist = |path: &std::path::Path, bytes: &[u8]| -> Result<(), String> {
        let mut file = tempfile::NamedTempFile::new_in(&directory)
            .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
        file.write_all(bytes)
            .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
        file.as_file()
            .sync_all()
            .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
        file.persist_noclobber(path)
            .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
        Ok(())
    };
    persist(&data_path, &bytes)?;
    let metadata = serde_json::to_vec(&blueprint).map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    if let Err(error) = persist(&metadata_path, &metadata) {
        let _ = fs::remove_file(data_path);
        return Err(error);
    }
    Ok(json!({"blueprintId":blueprint.id,"name":blueprint.name}))
}
pub fn open(
    app: AppHandle,
    plugin_id: &str,
    args: OpenRequest,
) -> Result<serde_json::Value, String> {
    valid_id(&args.blueprint_id).map_err(|_| "GENERATED_BLUEPRINT_INVALID")?;
    let directory = root("blueprints").map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    let metadata = read_file(&directory.join(format!("{}.json", args.blueprint_id)))
        .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    let blueprint: Blueprint =
        serde_json::from_slice(&metadata).map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    if blueprint.id != args.blueprint_id
        || blueprint
            .source
            .as_ref()
            .is_none_or(|source| source.provider != format!("generated:{plugin_id}"))
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    app.emit_to(
        "main",
        "plugin-generated-blueprint-open",
        json!({"blueprint_id":args.blueprint_id,"plugin_id":plugin_id}),
    )
    .map_err(|_| "GENERATED_BLUEPRINT_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> SaveRequest {
        SaveRequest {
            name: "Sphere".into(),
            width: 2,
            height: 2,
            length: 2,
            block: "minecraft:stone".into(),
            occupancy: STANDARD.encode([0b10000001]),
        }
    }
    #[test]
    fn generated_sponge_roundtrip() {
        let bytes = encode(&request()).unwrap();
        let mut gzip = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut nbt = Vec::new();
        std::io::Read::read_to_end(&mut gzip, &mut nbt).unwrap();
        let root: HashMap<String, Value> = fastnbt::from_bytes(&nbt).unwrap();
        assert_eq!(root["DataVersion"], Value::Int(3465));
        assert_eq!(root["Version"], Value::Int(2));
        assert_eq!(
            root["BlockData"],
            Value::ByteArray(fastnbt::ByteArray::new(vec![1, 0, 0, 0, 0, 0, 0, 1]))
        );
    }
    #[test]
    fn rejects_unbounded_and_invalid() {
        let mut args = request();
        args.width = 129;
        assert!(encode(&args).is_err());
        args = request();
        args.block = "minecraft:command_block".into();
        assert!(encode(&args).is_err());
        args = request();
        args.occupancy = STANDARD.encode([0]);
        assert!(encode(&args).is_err());
        args = request();
        args.width = 1;
        args.height = 1;
        args.length = 1;
        assert!(encode(&args).is_err());
        args = request();
        args.width = 128;
        args.height = 128;
        args.length = 128;
        args.occupancy = STANDARD.encode(vec![255; 262144]);
        assert!(encode(&args).is_err());
    }
}

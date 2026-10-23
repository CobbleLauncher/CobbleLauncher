// SPDX-License-Identifier: MIT
//! Independently implemented from public format documentation, not Litematica code.
//! Sources and the deliberately supported subset are recorded in ../README.md.
use super::ParsedScene;
use cobble_plugin_api::{
    MAX_BLOCKS, MaterialCount, PluginError, SceneBlock, SceneChunk, ViewerScene,
};
use fastnbt::Value;
use std::collections::{BTreeMap, HashMap};

type Compound = HashMap<String, Value>;

fn invalid(message: &str) -> PluginError {
    PluginError::new("VIEWER_INVALID_FILE", message)
}
fn unsupported(message: &str) -> PluginError {
    PluginError::new("VIEWER_UNSUPPORTED_VERSION", message)
}
fn int(map: &Compound, key: &str) -> Result<i32, PluginError> {
    match map.get(key) {
        Some(Value::Int(value)) => Ok(*value),
        _ => Err(invalid("Missing Litematic integer field")),
    }
}
fn compound<'a>(map: &'a Compound, key: &str) -> Result<&'a Compound, PluginError> {
    match map.get(key) {
        Some(Value::Compound(value)) => Ok(value),
        _ => Err(invalid("Missing Litematic compound")),
    }
}
fn vector(map: &Compound, key: &str) -> Result<[i64; 3], PluginError> {
    let value = compound(map, key)?;
    Ok([
        i64::from(int(value, "x")?),
        i64::from(int(value, "y")?),
        i64::from(int(value, "z")?),
    ])
}

struct Region<'a> {
    nbt: &'a Compound,
    min: [i64; 3],
    max: [i64; 3],
    size: [usize; 3],
    volume: usize,
}

pub(super) fn parse(root: &Compound) -> Result<ParsedScene, PluginError> {
    let version = int(root, "Version")?;
    if !matches!(version, 4..=7) {
        return Err(unsupported("Supported Litematic versions: 4, 5, 6 and 7"));
    }
    // Version eras change game data and entity/tick metadata, not this block stream.
    // SubVersion 1 first appears in v6; unknown future subversions remain explicit.
    let max_subversion = if version <= 5 { 0 } else { 1 };
    if root.contains_key("SubVersion") && !(0..=max_subversion).contains(&int(root, "SubVersion")?)
    {
        return Err(unsupported("Unsupported Litematic subversion"));
    }
    let entries = compound(root, "Regions")?;
    if entries.is_empty() || entries.len() > 256 {
        return Err(invalid("Preview supports 1–256 Litematic regions"));
    }
    // Stable region ordering is useful for deterministic palette/chunk output.
    let entries: BTreeMap<_, _> = entries.iter().collect();
    let mut regions: Vec<Region<'_>> = Vec::new();
    let mut total = 0usize;
    let mut min = [i64::MAX; 3];
    let mut max = [i64::MIN; 3];
    for value in entries.values() {
        let Value::Compound(nbt) = value else {
            return Err(invalid("Invalid Litematic region"));
        };
        let position = vector(nbt, "Position")?;
        let signed = vector(nbt, "Size")?;
        let mut region = Region {
            nbt,
            min: [0; 3],
            max: [0; 3],
            size: [0; 3],
            volume: 1,
        };
        for axis in 0..3 {
            if signed[axis] == 0 {
                return Err(invalid("Litematic region has a zero dimension"));
            }
            region.size[axis] = signed[axis].unsigned_abs() as usize;
            region.volume = region
                .volume
                .checked_mul(region.size[axis])
                .filter(|v| *v <= MAX_BLOCKS)
                .ok_or_else(|| invalid("Preview exceeds 1048576 blocks"))?;
            region.min[axis] = position[axis]
                + if signed[axis] < 0 {
                    signed[axis] + 1
                } else {
                    0
                };
            region.max[axis] = region.min[axis] + region.size[axis] as i64;
            min[axis] = min[axis].min(region.min[axis]);
            max[axis] = max[axis].max(region.max[axis]);
        }
        total = total
            .checked_add(region.volume)
            .filter(|v| *v <= MAX_BLOCKS)
            .ok_or_else(|| invalid("Combined regions exceed 1048576 blocks"))?;
        if regions.iter().any(|other| {
            (0..3).all(|axis| {
                region.min[axis] < other.max[axis] && other.min[axis] < region.max[axis]
            })
        }) {
            return Err(unsupported(
                "Overlapping Litematic region boxes are not supported",
            ));
        }
        regions.push(region);
    }
    let mut dimensions = [0usize; 3];
    for axis in 0..3 {
        dimensions[axis] = usize::try_from(max[axis] - min[axis])
            .ok()
            .filter(|dimension| *dimension <= MAX_BLOCKS)
            .ok_or_else(|| invalid("Enclosing preview exceeds 1048576 blocks"))?;
    }
    dimensions
        .iter()
        .try_fold(1usize, |volume, dimension| {
            volume.checked_mul(*dimension).filter(|v| *v <= MAX_BLOCKS)
        })
        .ok_or_else(|| invalid("Enclosing preview exceeds 1048576 blocks"))?;
    let mut palette = Vec::new();
    let mut palette_ids = HashMap::new();
    let mut counts = Vec::<u32>::new();
    let mut chunks: Vec<SceneChunk> = Vec::new();
    for region in regions {
        let Some(Value::List(entries)) = region.nbt.get("BlockStatePalette") else {
            return Err(invalid("Missing Litematic block palette"));
        };
        if entries.is_empty() || entries.len() > 4096 {
            return Err(invalid("Invalid Litematic palette size"));
        }
        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let state = block_state(entry)?;
            let next = palette.len();
            let id = *palette_ids.entry(state.clone()).or_insert(next);
            if id == next {
                if palette.len() == 4096 {
                    return Err(invalid("Combined palette exceeds 4096 states"));
                }
                palette.push(state);
                counts.push(0);
            }
            ids.push(id);
        }
        if palette[ids[0]] != "minecraft:air" {
            return Err(invalid("Litematic palette must start with minecraft:air"));
        }
        let Some(Value::LongArray(words)) = region.nbt.get("BlockStates") else {
            return Err(invalid("Missing Litematic BlockStates"));
        };
        let bits = (usize::BITS - (entries.len() - 1).leading_zeros()).max(2) as usize;
        let expected = (region.volume * bits).div_ceil(64);
        if words.len() != expected {
            return Err(invalid("Truncated or trailing Litematic block states"));
        }
        let mask = (1u64 << bits) - 1;
        for index in 0..region.volume {
            let offset = index * bits;
            let word = offset / 64;
            let shift = offset % 64;
            let mut encoded = (words[word] as u64) >> shift;
            if shift + bits > 64 {
                encoded |= (words[word + 1] as u64) << (64 - shift);
            }
            let local_id = (encoded & mask) as usize;
            let id = *ids
                .get(local_id)
                .ok_or_else(|| invalid("Invalid Litematic palette reference"))?;
            if matches!(
                palette[id].as_str(),
                "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
            ) {
                continue;
            }
            counts[id] += 1;
            if chunks.last().is_none_or(|chunk| chunk.blocks.len() == 1024) {
                chunks.push(SceneChunk {
                    blocks: Vec::with_capacity(1024),
                });
            }
            let local = [
                index % region.size[0],
                index / (region.size[0] * region.size[2]),
                (index / region.size[0]) % region.size[2],
            ];
            chunks
                .last_mut()
                .expect("chunk allocated")
                .blocks
                .push(SceneBlock {
                    x: (region.min[0] - min[0] + local[0] as i64) as u32,
                    y: (region.min[1] - min[1] + local[1] as i64) as u32,
                    z: (region.min[2] - min[2] + local[2] as i64) as u32,
                    palette_index: id as u32,
                });
        }
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
    let summary = ViewerScene {
        format_version: version as u32,
        width: dimensions[0] as u32,
        height: dimensions[1] as u32,
        length: dimensions[2] as u32,
        palette,
        materials,
        chunk_count: chunks.len() as u32,
    };
    summary.validate()?;
    Ok(ParsedScene { summary, chunks })
}

fn block_state(value: &Value) -> Result<String, PluginError> {
    let Value::Compound(entry) = value else {
        return Err(invalid("Invalid Litematic palette entry"));
    };
    let Some(Value::String(name)) = entry.get("Name") else {
        return Err(invalid("Missing block state name"));
    };
    if name.is_empty() || name.len() > 512 || name.contains(['[', ']', ',', '=']) {
        return Err(invalid("Invalid Litematic block state name"));
    }
    let mut state = name.clone();
    if let Some(properties) = entry.get("Properties") {
        let Value::Compound(properties) = properties else {
            return Err(invalid("Invalid block state properties"));
        };
        let sorted: BTreeMap<_, _> = properties.iter().collect();
        let mut values = Vec::new();
        for (key, value) in sorted {
            let Value::String(value) = value else {
                return Err(invalid("Block state property must be text"));
            };
            if key.is_empty()
                || value.is_empty()
                || key.contains(['[', ']', ',', '='])
                || value.contains(['[', ']', ',', '='])
            {
                return Err(invalid("Invalid block state property"));
            }
            values.push(format!("{key}={value}"));
        }
        if !values.is_empty() {
            state.push_str(&format!("[{}]", values.join(",")));
        }
    }
    if state.len() > 512 {
        return Err(invalid("Block state exceeds 512 bytes"));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector_value(values: [i32; 3]) -> Value {
        Value::Compound(
            ["x", "y", "z"]
                .into_iter()
                .zip(values)
                .map(|(key, value)| (key.into(), Value::Int(value)))
                .collect(),
        )
    }
    fn state(name: &str) -> Value {
        Value::Compound(HashMap::from([("Name".into(), Value::String(name.into()))]))
    }
    fn region(position: [i32; 3], size: [i32; 3], palette: Vec<Value>, words: Vec<i64>) -> Value {
        Value::Compound(HashMap::from([
            ("Position".into(), vector_value(position)),
            ("Size".into(), vector_value(size)),
            ("BlockStatePalette".into(), Value::List(palette)),
            (
                "BlockStates".into(),
                Value::LongArray(fastnbt::LongArray::new(words)),
            ),
        ]))
    }
    fn root(regions: Vec<(&str, Value)>) -> Compound {
        HashMap::from([
            ("Version".into(), Value::Int(6)),
            ("SubVersion".into(), Value::Int(1)),
            (
                "Regions".into(),
                Value::Compound(
                    regions
                        .into_iter()
                        .map(|(key, value)| (key.into(), value))
                        .collect(),
                ),
            ),
        ])
    }
    fn fixture() -> Compound {
        root(vec![
            (
                "negative",
                region(
                    [10, 20, 30],
                    [-3, -2, -2],
                    vec![state("minecraft:air"), state("minecraft:stone")],
                    vec![0x55_5555],
                ),
            ),
            (
                "positive",
                region(
                    [12, 19, 29],
                    [1, 1, 1],
                    vec![state("minecraft:air"), state("minecraft:stone")],
                    vec![1],
                ),
            ),
        ])
    }
    #[test]
    fn supported_version_eras_preserve_signed_geometry_and_cross_word_blocks() {
        for version in [4, 5, 6, 7] {
            let mut palette = vec![state("minecraft:air")];
            palette.extend((1..26).map(|id| state(&format!("fixture:block_{id}"))));
            let mut nbt = root(vec![(
                "boundary",
                region(
                    [10, 0, 0],
                    [-13, 1, 1],
                    palette,
                    vec![i64::MIN + (1 << 60), 1],
                ),
            )]);
            nbt.insert("Version".into(), Value::Int(version));
            if version <= 5 {
                nbt.remove("SubVersion");
            }
            let parsed = super::super::parse_scene(&fastnbt::to_bytes(&nbt).unwrap()).unwrap();
            assert_eq!(parsed.summary.format_version, version as u32);
            assert_eq!(parsed.summary.width, 13);
            assert_eq!(parsed.chunks[0].blocks[0].x, 12);
            assert_eq!(parsed.summary.materials[0].state, "fixture:block_25");
            assert_eq!(parsed.summary.materials[0].count, 1);
            if version <= 5 {
                nbt.insert("SubVersion".into(), Value::Int(1));
                assert_eq!(
                    parse(&nbt).err().unwrap().code,
                    "VIEWER_UNSUPPORTED_VERSION"
                );
            }
        }
    }
    #[test]
    fn signed_multiregion_origins_and_materials_match_geometry() {
        let parsed = super::super::parse_scene(&fastnbt::to_bytes(&fixture()).unwrap()).unwrap();
        assert_eq!(
            (
                parsed.summary.width,
                parsed.summary.height,
                parsed.summary.length
            ),
            (5, 2, 2)
        );
        assert_eq!(parsed.summary.format_version, 6);
        assert_eq!(parsed.summary.materials[0].count, 13);
        let blocks = &parsed.chunks[0].blocks;
        assert_eq!((blocks[0].x, blocks[0].y, blocks[0].z), (0, 0, 0));
        assert_eq!((blocks[11].x, blocks[11].y, blocks[11].z), (2, 1, 1));
        assert_eq!((blocks[12].x, blocks[12].y, blocks[12].z), (4, 0, 0));
        assert_eq!(
            parsed
                .summary
                .materials
                .iter()
                .map(|m| m.count as usize)
                .sum::<usize>(),
            blocks.len()
        );
    }
    #[test]
    fn decodes_cross_word_five_bit_entry_and_signed_long() {
        let mut palette = vec![state("minecraft:air")];
        palette.extend((1..26).map(|id| state(&format!("fixture:block_{id}"))));
        // Golden stream: twelve zero IDs followed by ID25 (11001) at bit60.
        // Its low four bits occupy bits60..63; its high bit occupies next word bit0.
        let nbt = root(vec![(
            "boundary",
            region([0; 3], [13, 1, 1], palette, vec![i64::MIN + (1 << 60), 1]),
        )]);
        let parsed = parse(&nbt).unwrap();
        assert_eq!(parsed.summary.materials[0].state, "fixture:block_25");
        assert_eq!(parsed.summary.materials[0].count, 1);
        assert_eq!(parsed.chunks[0].blocks[0].x, 12);
    }
    #[test]
    fn sorts_properties_and_accepts_entity_metadata_without_rendering_it() {
        let mut nbt = fixture();
        let Value::Compound(regions) = nbt.get_mut("Regions").unwrap() else {
            unreachable!()
        };
        let Value::Compound(region) = regions.get_mut("positive").unwrap() else {
            unreachable!()
        };
        region.insert(
            "Entities".into(),
            Value::List(vec![Value::Compound(HashMap::new())]),
        );
        region.insert(
            "BlockStatePalette".into(),
            Value::List(vec![
                state("minecraft:air"),
                Value::Compound(HashMap::from([
                    ("Name".into(), Value::String("minecraft:oak_log".into())),
                    (
                        "Properties".into(),
                        Value::Compound(HashMap::from([
                            ("waterlogged".into(), Value::String("false".into())),
                            ("axis".into(), Value::String("x".into())),
                        ])),
                    ),
                ])),
            ]),
        );
        let parsed = parse(&nbt).unwrap();
        assert!(
            parsed
                .summary
                .palette
                .contains(&"minecraft:oak_log[axis=x,waterlogged=false]".into())
        );
        assert_eq!(parsed.chunks[0].blocks.len(), 13);
    }
    #[test]
    fn rejects_unsupported_versions_subversions_and_overlaps() {
        let mut nbt = fixture();
        nbt.insert("Version".into(), Value::Int(8));
        assert_eq!(
            parse(&nbt).err().unwrap().code,
            "VIEWER_UNSUPPORTED_VERSION"
        );
        nbt.insert("Version".into(), Value::Int(6));
        nbt.insert("SubVersion".into(), Value::Int(2));
        assert_eq!(
            parse(&nbt).err().unwrap().code,
            "VIEWER_UNSUPPORTED_VERSION"
        );
        nbt.remove("SubVersion");
        assert!(parse(&nbt).is_ok());
        let overlapping = root(vec![
            (
                "a",
                region([0; 3], [1; 3], vec![state("minecraft:air")], vec![0]),
            ),
            (
                "b",
                region([0; 3], [1; 3], vec![state("minecraft:air")], vec![0]),
            ),
        ]);
        assert!(
            parse(&overlapping)
                .err()
                .unwrap()
                .message
                .contains("Overlapping")
        );
    }
    #[test]
    fn rejects_truncated_trailing_and_invalid_palette_references() {
        for words in [vec![], vec![0, 0], vec![3]] {
            let nbt = root(vec![(
                "bad",
                region(
                    [0; 3],
                    [1; 3],
                    vec![state("minecraft:air"), state("minecraft:stone")],
                    words,
                ),
            )]);
            assert_eq!(parse(&nbt).err().unwrap().code, "VIEWER_INVALID_FILE");
        }
    }
    #[test]
    fn rejects_zero_huge_sparse_and_extreme_coordinates_without_overflow() {
        for (a_pos, a_size, b_pos) in [
            ([0; 3], [0, 1, 1], [3, 0, 0]),
            ([0; 3], [i32::MIN, 1, 1], [3, 0, 0]),
            ([0; 3], [1; 3], [i32::MAX, 0, 0]),
            ([i32::MIN; 3], [-2; 3], [i32::MAX; 3]),
        ] {
            let nbt = root(vec![
                (
                    "a",
                    region(a_pos, a_size, vec![state("minecraft:air")], vec![0]),
                ),
                (
                    "b",
                    region(b_pos, [1; 3], vec![state("minecraft:air")], vec![0]),
                ),
            ]);
            assert!(parse(&nbt).is_err());
        }
    }
    #[test]
    fn gzip_litematic_uses_existing_bounded_input_path() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(&fastnbt::to_bytes(&fixture()).unwrap())
            .unwrap();
        let parsed = super::super::parse_scene(&encoder.finish().unwrap()).unwrap();
        assert_eq!(parsed.summary.materials[0].count, 13);
    }
}

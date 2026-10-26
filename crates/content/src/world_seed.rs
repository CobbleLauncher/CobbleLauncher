// SPDX-License-Identifier: MIT
//! Bounded, read-only saved Java world generation metadata. Never live server state.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::Path};
const MAX_METADATA: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorldMetadata {
    pub schema_version: u32,
    pub world_id: String,
    pub source: &'static str,
    pub seed: Option<String>,
    pub seed_source: Option<&'static str>,
    pub version: Option<String>,
    pub data_version: Option<i32>,
    pub generator: WorldGeneratorMetadata,
    pub spawn: Option<WorldSpawn>,
    pub unavailable: Option<&'static str>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct WorldGeneratorMetadata {
    pub preset: Option<String>,
    pub custom: Option<bool>,
    pub hints: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct WorldSpawn {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
#[derive(Deserialize)]
struct Level {
    #[serde(rename = "Data")]
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    #[serde(rename = "RandomSeed")]
    random_seed: Option<i64>,
    #[serde(rename = "WorldGenSettings")]
    world_gen: Option<WorldGen>,
    #[serde(rename = "Version")]
    version: Option<Version>,
    #[serde(rename = "DataVersion")]
    data_version: Option<i32>,
    #[serde(rename = "generatorName")]
    generator_name: Option<String>,
    #[serde(rename = "SpawnX")]
    spawn_x: Option<i32>,
    #[serde(rename = "SpawnY")]
    spawn_y: Option<i32>,
    #[serde(rename = "SpawnZ")]
    spawn_z: Option<i32>,
}
#[derive(Deserialize)]
struct Version {
    #[serde(rename = "Name")]
    name: Option<String>,
}
#[derive(Deserialize)]
struct WorldGen {
    seed: Option<i64>,
    #[serde(default)]
    dimensions: BTreeMap<String, Dimension>,
}
#[derive(Deserialize)]
struct Dimension {
    generator: Option<Generator>,
}
#[derive(Deserialize)]
struct Generator {
    #[serde(rename = "type")]
    kind: Option<String>,
    settings: Option<fastnbt::Value>,
    biome_source: Option<BiomeSource>,
}
#[derive(Deserialize)]
struct BiomeSource {
    #[serde(rename = "type")]
    kind: Option<String>,
    preset: Option<String>,
}
fn clean(value: String) -> Option<String> {
    (!value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control))
        .then_some(value)
}
fn empty(world_id: &str, reason: &'static str) -> LocalWorldMetadata {
    LocalWorldMetadata {
        schema_version: 1,
        world_id: world_id.into(),
        source: "saved_metadata",
        seed: None,
        seed_source: None,
        version: None,
        data_version: None,
        generator: WorldGeneratorMetadata::default(),
        spawn: None,
        unavailable: Some(reason),
    }
}
/// Parse a bounded immutable gzip snapshot. Seed strings preserve every signed 64-bit value.
pub fn read_saved_world_metadata(mut input: impl Read, world_id: &str) -> LocalWorldMetadata {
    let mut read = || -> Result<Data, &'static str> {
        let mut compressed = Vec::new();
        input
            .by_ref()
            .take(MAX_METADATA as u64 + 1)
            .read_to_end(&mut compressed)
            .map_err(|_| "metadata_unavailable")?;
        if compressed.len() > MAX_METADATA {
            return Err("metadata_too_large");
        }
        let mut bytes = Vec::new();
        GzDecoder::new(compressed.as_slice())
            .take(MAX_METADATA as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "metadata_invalid")?;
        if bytes.len() > MAX_METADATA {
            return Err("metadata_too_large");
        }
        fastnbt::from_bytes::<Level>(&bytes)
            .map(|level| level.data)
            .map_err(|_| "metadata_invalid")
    };
    let data = match read() {
        Ok(data) => data,
        Err(reason) => return empty(world_id, reason),
    };
    let mut output = empty(world_id, "seed_missing");
    output.version = data
        .version
        .and_then(|version| version.name)
        .and_then(clean);
    output.data_version = data.data_version;
    output.spawn = match (data.spawn_x, data.spawn_y, data.spawn_z) {
        (Some(x), Some(y), Some(z)) => Some(WorldSpawn { x, y, z }),
        _ => None,
    };
    let modern = data.world_gen.is_some();
    if let Some(world_gen) = data.world_gen {
        if let Some(seed) = world_gen.seed {
            output.seed = Some(seed.to_string());
            output.seed_source = Some("world_gen_settings");
        }
        // Dimension generator references are hints, never proof of a default world preset.
        let mut custom = None;
        if world_gen.dimensions.len() <= 16 {
            for (dimension, value) in world_gen.dimensions {
                let Some(dimension) = clean(dimension) else {
                    continue;
                };
                let Some(generator) = value.generator else {
                    continue;
                };
                if dimension == "minecraft:overworld"
                    && generator.kind.as_deref() == Some("minecraft:noise")
                    && generator.biome_source.as_ref().is_some_and(|source| {
                        source.kind.as_deref() == Some("minecraft:multi_noise")
                            && source.preset.as_deref() == Some("minecraft:overworld")
                    })
                    && let Some(fastnbt::Value::String(settings)) = &generator.settings
                {
                    output.generator.preset = match settings.as_str() {
                        "minecraft:overworld" => Some("default".into()),
                        "minecraft:large_biomes" => Some("large_biomes".into()),
                        _ => None,
                    };
                }
                if !matches!(
                    dimension.as_str(),
                    "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
                ) {
                    custom = Some(true);
                }
                if generator.biome_source.as_ref().is_some_and(|source| {
                    source.kind.as_deref().is_some_and(|kind| {
                        !matches!(kind, "minecraft:multi_noise" | "minecraft:the_end")
                    }) || source.preset.as_deref().is_some_and(|preset| {
                        !matches!(preset, "minecraft:overworld" | "minecraft:nether")
                    })
                }) {
                    custom = Some(true);
                }
                if let Some(kind) = generator.kind.and_then(clean) {
                    if !matches!(
                        kind.as_str(),
                        "minecraft:noise" | "minecraft:flat" | "minecraft:debug"
                    ) {
                        custom = Some(true);
                    }
                    output
                        .generator
                        .hints
                        .push(format!("{dimension}:type={kind}"));
                }
                match generator.settings {
                    Some(fastnbt::Value::String(settings)) => {
                        if let Some(settings) = clean(settings) {
                            if !settings.starts_with("minecraft:") {
                                custom = Some(true);
                            }
                            output
                                .generator
                                .hints
                                .push(format!("{dimension}:settings={settings}"));
                        }
                    }
                    Some(fastnbt::Value::Compound(_)) => {
                        custom = Some(true);
                        output
                            .generator
                            .hints
                            .push(format!("{dimension}:inline_settings"));
                    }
                    _ => {}
                }
            }
        }
        output.generator.custom =
            custom.or_else(|| output.generator.preset.as_ref().map(|_| false));
    }
    if output.seed.is_none()
        && let Some(seed) = data.random_seed
    {
        output.seed = Some(seed.to_string());
        output.seed_source = Some("random_seed");
    }
    if let Some(preset) = data.generator_name.filter(|_| !modern).and_then(clean) {
        output.generator.custom = Some(!matches!(
            preset.as_str(),
            "default" | "flat" | "largeBiomes" | "amplified" | "default_1_1"
        ));
        output.generator.preset = Some(match preset.as_str() {
            "largeBiomes" => "large_biomes".into(),
            _ => preset,
        });
    }
    if output.seed.is_some() {
        output.unavailable = None;
    }
    output
}
/// Host-only lookup through the same direct-folder authorization as the world browser.
pub fn local_world_metadata(game_dir: &Path, world_id: &str) -> LocalWorldMetadata {
    if crate::worlds::world_directory(game_dir, world_id).is_err() {
        return empty(world_id, "metadata_unavailable");
    }
    let open = || -> std::io::Result<cap_std::fs::File> {
        let game = Dir::open_ambient_dir(game_dir, cap_std::ambient_authority())?;
        let saves = game.open_dir_nofollow("saves")?;
        let world = saves.open_dir_nofollow(world_id)?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        world.open_with("level.dat", &options)
    };
    let Ok(file) = open() else {
        return empty(world_id, "metadata_unavailable");
    };
    let Ok(before) = file.metadata() else {
        return empty(world_id, "metadata_unavailable");
    };
    if !before.is_file() || before.len() > MAX_METADATA as u64 {
        return empty(world_id, "metadata_too_large");
    };
    let mut output = read_saved_world_metadata(&file, world_id);
    if file.metadata().ok().is_none_or(|after| {
        after.len() != before.len() || after.modified().ok() != before.modified().ok()
    }) {
        output = empty(world_id, "metadata_changed");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastnbt::Value;
    use flate2::{Compression, write::GzEncoder};
    use std::{collections::HashMap, io::Write};
    fn compound(fields: Vec<(&str, Value)>) -> Value {
        Value::Compound(
            fields
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }
    fn fixture(data: Value) -> Vec<u8> {
        let root: HashMap<String, Value> = [("Data".into(), data)].into_iter().collect();
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        gzip.write_all(&fastnbt::to_bytes(&root).unwrap()).unwrap();
        gzip.finish().unwrap()
    }
    fn modern(settings: &str, source: &str) -> Value {
        compound(vec![
            ("seed", Value::Long(i64::MIN)),
            (
                "dimensions",
                compound(vec![(
                    "minecraft:overworld",
                    compound(vec![(
                        "generator",
                        compound(vec![
                            ("type", Value::String("minecraft:noise".into())),
                            ("settings", Value::String(settings.into())),
                            (
                                "biome_source",
                                compound(vec![
                                    ("type", Value::String("minecraft:multi_noise".into())),
                                    ("preset", Value::String(source.into())),
                                ]),
                            ),
                        ]),
                    )]),
                )]),
            ),
        ])
    }
    #[test]
    fn legacy_seed_keeps_i64_precision_and_saved_version_spawn() {
        let bytes = fixture(compound(vec![
            ("RandomSeed", Value::Long(i64::MAX)),
            (
                "Version",
                compound(vec![("Name", Value::String("1.12.2".into()))]),
            ),
            ("DataVersion", Value::Int(1343)),
            ("generatorName", Value::String("largeBiomes".into())),
            ("SpawnX", Value::Int(-123)),
            ("SpawnY", Value::Int(64)),
            ("SpawnZ", Value::Int(456)),
        ]));
        let result = read_saved_world_metadata(bytes.as_slice(), "World");
        assert_eq!(result.seed.as_deref(), Some("9223372036854775807"));
        assert_eq!(result.seed_source, Some("random_seed"));
        assert_eq!(result.version.as_deref(), Some("1.12.2"));
        assert_eq!(result.data_version, Some(1343));
        assert_eq!(result.spawn.unwrap().x, -123);
        assert_eq!(result.generator.preset.as_deref(), Some("large_biomes"));
        assert_eq!(result.source, "saved_metadata");
    }
    #[test]
    fn modern_seed_wins_and_preset_requires_matching_explicit_vanilla_references() {
        for (settings, preset) in [
            ("minecraft:overworld", "default"),
            ("minecraft:large_biomes", "large_biomes"),
        ] {
            let bytes = fixture(compound(vec![
                ("RandomSeed", Value::Long(7)),
                ("WorldGenSettings", modern(settings, "minecraft:overworld")),
            ]));
            let result = read_saved_world_metadata(bytes.as_slice(), "世界");
            assert_eq!(result.seed.as_deref(), Some("-9223372036854775808"));
            assert_eq!(result.seed_source, Some("world_gen_settings"));
            assert_eq!(result.generator.preset.as_deref(), Some(preset));
            assert_eq!(result.generator.custom, Some(false));
            assert!(result.spawn.is_none());
        }
        let bytes = fixture(compound(vec![
            (
                "WorldGenSettings",
                modern("minecraft:overworld", "custom:biomes"),
            ),
            ("generatorName", Value::String("default".into())),
        ]));
        let result = read_saved_world_metadata(bytes.as_slice(), "World");
        assert!(result.generator.preset.is_none());
        assert_eq!(result.generator.custom, Some(true));
    }
    #[test]
    fn unavailable_metadata_and_missing_seed_are_explicit() {
        assert_eq!(
            read_saved_world_metadata(&b"bad gzip"[..], "World").unavailable,
            Some("metadata_invalid")
        );
        let bytes = fixture(compound(vec![]));
        let missing = read_saved_world_metadata(bytes.as_slice(), "World");
        assert!(missing.seed.is_none());
        assert_eq!(missing.unavailable, Some("seed_missing"));
        assert_eq!(
            local_world_metadata(Path::new("."), "../private").unavailable,
            Some("metadata_unavailable")
        );
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        gzip.write_all(&vec![0; MAX_METADATA + 1]).unwrap();
        assert_eq!(
            read_saved_world_metadata(gzip.finish().unwrap().as_slice(), "World").unavailable,
            Some("metadata_too_large")
        );
    }
    #[test]
    fn local_metadata_uses_only_a_direct_regular_saved_world() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("saves/World");
        std::fs::create_dir_all(&world).unwrap();
        let bytes = fixture(compound(vec![("RandomSeed", Value::Long(123))]));
        std::fs::write(world.join("level.dat"), &bytes).unwrap();
        assert_eq!(
            local_world_metadata(root.path(), "World").seed.as_deref(),
            Some("123")
        );
        assert!(local_world_metadata(root.path(), "../World").seed.is_none());
        std::fs::remove_file(world.join("level.dat")).unwrap();
        std::fs::create_dir(world.join("level.dat")).unwrap();
        assert!(local_world_metadata(root.path(), "World").seed.is_none());
    }
    #[cfg(any(unix, windows))]
    #[test]
    #[cfg_attr(
        windows,
        ignore = "requires Windows Developer Mode or file-symlink privilege"
    )]
    fn linked_level_metadata_does_not_reveal_an_outside_seed() {
        let root = tempfile::tempdir().unwrap();
        let world = root.path().join("saves/World");
        std::fs::create_dir_all(&world).unwrap();
        let outside = root.path().join("outside.dat");
        std::fs::write(
            &outside,
            fixture(compound(vec![("RandomSeed", Value::Long(987654321))])),
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, world.join("level.dat")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, world.join("level.dat"))
            .expect("File symlink fixture requires Windows Developer Mode or symlink permission");
        let result = local_world_metadata(root.path(), "World");
        assert!(result.seed.is_none());
        assert_eq!(result.unavailable, Some("metadata_unavailable"));
    }
}

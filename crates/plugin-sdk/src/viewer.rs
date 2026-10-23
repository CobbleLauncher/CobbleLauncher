// SPDX-License-Identifier: MIT
use cobble_plugin_api::{PluginError, SceneChunk, ViewerScene};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

pub fn test(package: &Path, host: &Path, input: &Path, json: bool) -> Result<(), Box<dyn Error>> {
    let (manifest, component) = cobble_plugin_host::read_package(package)?;
    if manifest.capabilities != ["viewer"] {
        return Err(
            "Viewer test requires a viewer package; use test-report for report packages".into(),
        );
    }
    let extension = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !manifest
        .file_extensions
        .iter()
        .any(|allowed| allowed == &extension)
    {
        return Err("Input extension is not supported by this viewer package".into());
    }
    let mut data = Vec::new();
    fs::File::open(input)?
        .take(cobble_plugin_api::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > cobble_plugin_api::MAX_FILE_BYTES {
        return Err("Viewer input exceeds the size limit".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let mut session =
        cobble_plugin_host::PluginSession::open(host, &component, &data, cancel.clone())?;
    let scene = session.summary().clone();
    if !scene_version_matches_extension(&extension, scene.format_version) {
        return Err("Viewer format does not match the input extension".into());
    }
    let blocks = validate_pages(&scene, |index| session.read_chunk(index, cancel.clone()))?;
    session.close();
    if !session.has_exited()? {
        return Err("Viewer host did not exit after the test".into());
    }
    if json {
        println!(
            "{}",
            serde_json::json!({"report_version":1,"plugin_id":manifest.id,"api_version":manifest.api_version,"scene":scene,"validated_chunks":scene.chunk_count,"validated_blocks":blocks})
        );
    } else {
        println!(
            "{} × {} × {}; {} materials; {} chunks, {} blocks validated",
            scene.width,
            scene.height,
            scene.length,
            scene.materials.len(),
            scene.chunk_count,
            blocks
        );
    }
    Ok(())
}

fn scene_version_matches_extension(extension: &str, version: u32) -> bool {
    match extension {
        "schem" => matches!(version, 1..=3),
        "litematic" => matches!(version, 4..=7),
        _ => false,
    }
}

fn validate_pages(
    scene: &ViewerScene,
    mut read: impl FnMut(u32) -> Result<SceneChunk, PluginError>,
) -> Result<usize, PluginError> {
    scene.validate()?;
    let mut positions = HashSet::new();
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for index in 0..scene.chunk_count {
        let chunk = read(index).map_err(|error| {
            PluginError::new(&error.code, format!("Chunk {index}: {}", error.message))
        })?;
        scene.validate_chunk(&chunk)?;
        for block in chunk.blocks {
            if !positions.insert((block.x, block.y, block.z)) {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_REPLY",
                    format!(
                        "Chunk {index}: block coordinate was already returned by another chunk"
                    ),
                ));
            }
            *counts
                .entry(&scene.palette[block.palette_index as usize])
                .or_default() += 1;
        }
    }
    let expected: HashMap<_, _> = scene
        .materials
        .iter()
        .map(|material| (material.state.as_str(), material.count))
        .collect();
    if counts != expected {
        return Err(PluginError::new(
            "PLUGIN_INVALID_REPLY",
            "Paged blocks do not match the declared material counts",
        ));
    }
    Ok(positions.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_plugin_api::{MaterialCount, SceneBlock};
    fn scene() -> ViewerScene {
        ViewerScene {
            format_version: 2,
            width: 2048,
            height: 1,
            length: 1,
            palette: vec!["minecraft:stone".into()],
            materials: vec![MaterialCount {
                state: "minecraft:stone".into(),
                count: 2048,
            }],
            chunk_count: 2,
        }
    }
    fn chunk(index: u32) -> SceneChunk {
        SceneChunk {
            blocks: (index * 1024..(index + 1) * 1024)
                .map(|x| SceneBlock {
                    x,
                    y: 0,
                    z: 0,
                    palette_index: 0,
                })
                .collect(),
        }
    }
    #[test]
    fn cli_version_guard_keeps_sponge_and_litematic_versions_separate() {
        for version in 0..=8 {
            assert_eq!(
                scene_version_matches_extension("schem", version),
                (1..=3).contains(&version)
            );
            assert_eq!(
                scene_version_matches_extension("litematic", version),
                (4..=7).contains(&version)
            );
            assert!(!scene_version_matches_extension("txt", version));
        }
        assert!(!scene_version_matches_extension("schem", u32::MAX));
        assert!(!scene_version_matches_extension("litematic", u32::MAX));
    }
    #[test]
    fn visits_every_page_and_propagates_late_failure() {
        let mut visits = Vec::new();
        let error = validate_pages(&scene(), |index| {
            visits.push(index);
            if index == 1 {
                Err(PluginError::new("BROKEN_CHUNK", "parser failed"))
            } else {
                Ok(chunk(index))
            }
        })
        .unwrap_err();
        assert_eq!(visits, vec![0, 1]);
        assert_eq!(error.code, "BROKEN_CHUNK");
        assert!(error.message.contains("Chunk 1"));
    }
    #[test]
    fn rejects_duplicate_coordinates_between_pages() {
        let error = validate_pages(&scene(), |_| Ok(chunk(0))).unwrap_err();
        assert!(error.message.contains("another chunk"));
    }
    #[test]
    fn detects_missing_blocks_and_accepts_complete_output() {
        assert_eq!(
            validate_pages(&scene(), |index| Ok(chunk(index))).unwrap(),
            2048
        );
        let error = validate_pages(&scene(), |index| {
            let mut page = chunk(index);
            if index == 1 {
                page.blocks.pop();
            }
            Ok(page)
        })
        .unwrap_err();
        assert!(error.message.contains("material counts"));
    }
}

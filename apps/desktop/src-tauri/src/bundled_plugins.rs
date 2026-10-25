// SPDX-License-Identifier: MIT
use cobble_plugin_api::PluginManifest;
use std::path::PathBuf;

const BUNDLED_IDS: [&str; 30] = [
    "schematic-viewer",
    "crash-doctor",
    "modpack-inspector",
    "mcs-provider",
    "session-card",
    "config-doctor",
    "pack-metadata",
    "screenshot-sheet",
    "mod-conflicts",
    "resource-pack-stack",
    "world-dashboard",
    "world-chronicle",
    "command-pocket",
    "seed-compass",
    "coordinate-book",
    "material-planner",
    "trial-copy",
    "file-inbox",
    "ai-assistant",
    "anvil-planner",
    "geometry-builder",
    "play-manifest",
    "lang-doctor",
    "backup-auditor",
    "datapack-auditor",
    "serverbook-inspector",
    "screenshot-timeline",
    "controls-auditor",
    "storage-lens",
    "runtime-advisor",
];

fn package_path(id: &str) -> Result<PathBuf, String> {
    if !BUNDLED_IDS.contains(&id) {
        return Err("未知的内置插件".into());
    }
    let path = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .ok_or("无法找到插件目录")?
        .join("plugins")
        .join(id);
    #[cfg(debug_assertions)]
    let path = if path.exists() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../../target/{id}-package"))
    };
    Ok(path)
}

fn read_bundled(id: &str) -> Result<(PluginManifest, Vec<u8>), String> {
    let package = super::plugin_signatures::read_package(&package_path(id)?)
        .map_err(|error| error.to_string())?;
    if package.0.id != id {
        return Err("内置插件标识不匹配".into());
    }
    Ok(package)
}

pub(super) fn icon(id: &str) -> Result<Option<Vec<u8>>, String> {
    let (manifest, icon) =
        cobble_plugin_host::read_package_icon(&package_path(id)?).map_err(|e| e.to_string())?;
    if manifest.id != id {
        return Err("Bundled plugin ID differs".into());
    }
    Ok(icon)
}
#[tauri::command]
pub fn list_bundled_plugins() -> Result<Vec<PluginManifest>, String> {
    let installed = super::registry()?
        .list()
        .map_err(|error| error.to_string())?;
    let mut available = Vec::new();
    for id in BUNDLED_IDS {
        if !package_path(id)?.exists() {
            continue;
        }
        let manifest = read_bundled(id)?.0;
        if !installed.iter().any(|plugin| plugin.manifest == manifest) {
            available.push(manifest);
        }
    }
    Ok(available)
}

#[tauri::command]
pub fn preview_bundled_plugin(id: String) -> Result<PluginManifest, String> {
    let revision = super::begin_plugin_preview()?;
    read_bundled(&id)?;
    super::prepare_directory_package(package_path(&id)?, revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_selection_is_an_allowlist_not_a_path() {
        for id in [
            "../crash-doctor",
            "C:/plugins/crash-doctor",
            "unknown-viewer",
            "",
        ] {
            assert!(package_path(id).is_err());
        }
        for id in BUNDLED_IDS {
            assert!(
                package_path(id).unwrap().ends_with(id)
                    || package_path(id).unwrap().ends_with(format!("{id}-package"))
            );
        }
    }
}

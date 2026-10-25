// SPDX-License-Identifier: MIT
#[tauri::command]
pub fn query_launch_compatibility(
    minecraft_version: String,
    loader_kind: Option<String>,
    loader_version: Option<String>,
) -> cobble_content::Compatibility {
    cobble_content::query_compatibility(
        &minecraft_version,
        loader_kind.as_deref(),
        loader_version.as_deref(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

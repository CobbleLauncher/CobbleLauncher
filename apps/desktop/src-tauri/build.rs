// SPDX-License-Identifier: MIT
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "get_download_settings",
            "set_download_settings",
            "list_game_versions",
            "list_games",
            "default_game_directory",
            "pick_game_directory",
            "create_game",
            "retry_game",
        ]),
    ))
    .expect("failed to prepare desktop application");
}

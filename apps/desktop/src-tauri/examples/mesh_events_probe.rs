// SPDX-License-Identifier: MIT
//! Two-process native Mesh fixture. No production path or bridge overrides.
#![allow(dead_code)]
extern crate self as dirs;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
static ROOT: OnceLock<PathBuf> = OnceLock::new();
pub fn config_dir() -> Option<PathBuf> {
    ROOT.get().map(|p| p.join("config"))
}
pub fn data_local_dir() -> Option<PathBuf> {
    ROOT.get().map(|p| p.join("data"))
}
pub fn cache_dir() -> Option<PathBuf> {
    ROOT.get().map(|p| p.join("cache"))
}
pub fn desktop_dir() -> Option<PathBuf> {
    ROOT.get().map(|p| p.join("desktop"))
}
#[path = "../src/appearance.rs"]
mod appearance;
#[path = "../src/cache.rs"]
mod cache;
#[path = "../src/catalog_install.rs"]
mod catalog_install;
#[path = "../src/curseforge.rs"]
mod curseforge;
#[path = "../src/games.rs"]
mod games;
#[path = "../src/java.rs"]
mod java;
#[path = "../src/logging.rs"]
mod logging;
#[path = "../src/market_images.rs"]
mod market_images;
#[path = "../src/microsoft.rs"]
mod microsoft;
#[path = "../src/offline.rs"]
mod offline;
#[path = "../src/plugin_catalogs.rs"]
mod plugin_catalogs;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/preferences.rs"]
mod preferences;
#[path = "../src/seed_map.rs"]
mod seed_map;

fn owned_root(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = path.canonicalize()?;
    let temp = std::env::temp_dir().canonicalize()?;
    if !root.starts_with(&temp)
        || !root.ancestors().any(|p| {
            p.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .starts_with("cobble-plugin-webview-probe-mesh-")
            })
        })
    {
        return Err("explicit owned temporary Mesh probe root required".into());
    }
    Ok(root)
}
fn prepare(package: &Path, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package.join("plugin.json"))?)?;
    if manifest["id"] != "mesh-events-demo" {
        return Err("Ready check package required".into());
    }
    let registry = root.join("data/CobbleLauncher/plugins");
    std::fs::create_dir_all(&registry)?;
    cobble_plugin_host::PluginRegistry::new(registry.clone()).install_directory(package)?;
    std::fs::write(registry.join(".bundled-seeded"), b"1")?;
    let config = root.join("config/CobbleLauncher");
    std::fs::create_dir_all(&config)?;
    std::fs::write(config.join("games.json"), b"[]")?;
    // The caller explicitly opts the isolated fixture in; never touches user preferences.
    std::fs::write(
        config.join("mesh-enabled.json"),
        br#"{"schema_version":1,"enabled":true}"#,
    )?;
    Ok(())
}
#[tauri::command]
fn get_app_info(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    finish: Option<bool>,
) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("PROBE_CALLER_DENIED".into());
    }
    if finish == Some(true) {
        app.exit(0);
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = owned_root(&PathBuf::from(
        std::env::args_os().nth(1).ok_or("root argument required")?,
    ))?;
    ROOT.set(root.clone()).map_err(|_| "root initialized")?;
    let package = PathBuf::from(
        std::env::args_os()
            .nth(2)
            .ok_or("package argument required")?,
    );
    prepare(&package, &root)?;
    plugins::plugin_views::enable_isolated_probe()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(games::GameLibrary::load())
        .manage(games::tasks::TaskLibrary::load())
        .register_uri_scheme_protocol("cobble-probe", |_, _| {
            tauri::http::Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(
                    b"<!doctype html><html><body><p>Isolated Mesh plugin check</p></body></html>"
                        .to_vec(),
                )
                .unwrap()
        })
        .register_uri_scheme_protocol("cobble-plugin", plugins::plugin_views::serve_asset)
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            plugins::plugin_frontend_bridge::frontend_call,
            plugins::plugin_views::open_plugin_view,
            plugins::plugin_views::close_plugin_view,
            plugins::plugin_views::plugin_view_is_dirty,
            plugins::plugin_views::set_plugin_view_visible,
            games::mesh::app::mesh_start_app,
            games::mesh::app::mesh_app_snapshot,
            games::mesh::app::mesh_create_contact_invitation,
            games::mesh::app::mesh_accept_contact_invitation,
            games::mesh::app::mesh_stop_app
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(150));
                handle.exit(2);
            });
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External("http://cobble-probe.localhost/index.html".parse()?),
            )
            .title("Isolated Mesh plugin check")
            .inner_size(1000., 720.)
            .position(
                if root.file_name().is_some_and(|name| name == "b") {
                    420.
                } else {
                    0.
                },
                0.,
            )
            .focused(false)
            .data_directory(root.join("main-webview"))
            .build()?;
            Ok(())
        })
        .build(context)?;
    app.run(|_, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            games::mesh::shutdown();
        }
    });
    Ok(())
}

// SPDX-License-Identifier: MIT
//! Joined material preview proof using real bundled Vue, native commands, Wasm and GPU.
//! Run from the repository root with isolated target/blueprint-material-probe-* roots.
#![allow(dead_code)]
// Windows known-folder lookup ignores APPDATA overrides. This example-only
// adapter makes every included native module use the verified temporary roots.
extern crate self as dirs;
pub fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(Into::into)
}
pub fn data_local_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(Into::into)
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

use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
static EVENTS: Mutex<Vec<Value>> = Mutex::new(Vec::new());
#[tauri::command(rename = "save_blueprint_thumbnail")]
fn observed_thumbnail(id: String, png: String) -> Result<(), String> {
    plugins::blueprint_library::save_blueprint_thumbnail(id, png)?;
    EVENTS
        .lock()
        .unwrap()
        .push(json!({"command":"save_blueprint_thumbnail","success":true}));
    Ok(())
}
#[tauri::command(rename = "load_viewer_textures")]
async fn observed_textures(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    webview: tauri::Webview,
    resource_id: String,
    game_id: String,
) -> Result<plugins::viewer_textures::TextureSet, String> {
    let result =
        plugins::viewer_textures::load_viewer_textures(app, window, webview, resource_id, game_id)
            .await;
    let summary=result.as_ref().map(|set| { let value=serde_json::to_value(set).unwrap(); json!({"source":value["source"],"resolvedBlocks":value["resolvedBlocks"],"textures":value["textures"].as_array().map(Vec::len)}) }).map_err(Clone::clone);
    EVENTS
        .lock()
        .unwrap()
        .push(json!({"command":"load_viewer_textures","result":summary}));
    result
}
#[tauri::command]
fn get_app_info(app: tauri::AppHandle, proof: Option<Value>) -> Value {
    if let Some(mut proof) = proof {
        let path = data_root().join("CobbleLauncher/blueprints/material-proof.png");
        let persisted=std::fs::read(&path).ok().and_then(|b|image::load_from_memory(&b).ok()).map(|im| {
            let rgb=im.to_rgb8(); let colors:std::collections::HashSet<_>=rgb.pixels().map(|p|p.0).collect();
            let cyan=rgb.pixels().filter(|p|p[1]>p[0].saturating_add(35)&&p[2]>p[0].saturating_add(35)).count();
            let magenta=rgb.pixels().filter(|p|p[0]>p[1].saturating_add(35)&&p[2]>p[1].saturating_add(35)).count();
            json!({"width":rgb.width(),"height":rgb.height(),"distinctColors":colors.len(),"originalCyanPixels":cyan,"originalMagentaPixels":magenta,"bytes":std::fs::metadata(&path).unwrap().len()})
        });
        let events = EVENTS.lock().unwrap();
        let resolved = events
            .iter()
            .any(|e| e["result"]["Ok"]["resolvedBlocks"].as_u64().unwrap_or(0) > 0);
        proof["persisted"] = json!(persisted);
        proof["passed"] = json!(
            proof["passed"] == true
                && resolved
                && persisted
                    .as_ref()
                    .is_some_and(|p| p["distinctColors"].as_u64().unwrap_or(0) > 16
                        && p["originalCyanPixels"].as_u64().unwrap_or(0) > 20
                        && p["originalMagentaPixels"].as_u64().unwrap_or(0) > 20)
        );
        let report = json!({"ui":proof,"nativeEvents":*events,"fixture":"original generated checker textures; no network or game process"});
        std::fs::write(
            data_root().join("material-proof.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        eprintln!("MATERIAL_PROOF {report}");
        app.exit(if proof["passed"] == true { 0 } else { 3 });
    }
    let events = EVENTS.lock().unwrap();
    json!({"events":*events,"saves":events.iter().filter(|e|e["command"]=="save_blueprint_thumbnail").count()})
}
fn data_root() -> PathBuf {
    dirs::data_local_dir().unwrap()
}
fn isolated(path: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    let target = std::env::current_dir()?.join("target").canonicalize()?;
    if !path.starts_with(target)
        || !path.ancestors().any(|p| {
            p.file_name()
                .and_then(|p| p.to_str())
                .is_some_and(|p| p.starts_with("blueprint-material-probe-"))
        })
    {
        return Err("isolated target/blueprint-material-probe-* roots required".into());
    }
    Ok(path)
}
fn prepare(
    package: &Path,
    schematic: &Path,
    data: &Path,
    config: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = data.join("CobbleLauncher/plugins");
    std::fs::create_dir_all(&registry)?;
    cobble_plugin_host::PluginRegistry::new(registry.clone()).install_directory(package)?;
    std::fs::write(registry.join(".bundled-seeded"), b"1")?;
    let library = data.join("CobbleLauncher/blueprints");
    std::fs::create_dir_all(&library)?;
    std::fs::copy(schematic, library.join("material-proof.schem"))?;
    std::fs::write(
        library.join("material-proof.json"),
        serde_json::to_vec(
            &json!({"id":"material-proof","name":"Original material fixture","pluginId":"schematic-viewer","format":"schem","favorite":false,"hasThumbnail":false}),
        )?,
    )?;
    let game = data.join("fixture-game");
    let version = game.join("versions/1.21.1");
    std::fs::create_dir_all(&version)?;
    let mut jar = zip::ZipWriter::new(std::fs::File::create(version.join("1.21.1.jar"))?);
    let options = zip::write::SimpleFileOptions::default();
    for (block, color) in [("stone", [225, 35, 180]), ("oak_planks", [25, 210, 230])] {
        for (path, value) in [
            (
                format!("assets/minecraft/blockstates/{block}.json"),
                json!({"variants":{"":{"model":format!("minecraft:block/{block}")}}}),
            ),
            (
                format!("assets/minecraft/models/block/{block}.json"),
                json!({"textures":{"all":format!("minecraft:block/{block}")}}),
            ),
        ] {
            jar.start_file(path, options)?;
            jar.write_all(&serde_json::to_vec(&value)?)?;
        }
        let image = image::RgbImage::from_fn(16, 16, |x, y| {
            image::Rgb(if (x / 4 + y / 4) % 2 == 0 {
                color
            } else {
                [30, 40, 55]
            })
        });
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png)?;
        jar.start_file(
            format!("assets/minecraft/textures/block/{block}.png"),
            options,
        )?;
        jar.write_all(png.get_ref())?;
    }
    jar.finish()?;
    std::fs::create_dir_all(config.join("CobbleLauncher"))?;
    std::fs::write(
        config.join("CobbleLauncher/games.json"),
        serde_json::to_vec(
            &json!([{"id":"012345abcdef","name":"Original material assets","version":"1.21.1","directory":game,"status":"installed","error":null}]),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = isolated(data_root())?;
    let config = isolated(dirs::config_dir().ok_or("config")?)?;
    let args: Vec<_> = std::env::args().collect();
    prepare(
        Path::new(args.get(1).ok_or("package path")?),
        Path::new(args.get(2).ok_or("original schematic path")?),
        &data,
        &config,
    )?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.config_mut().identifier = "cobble-material-proof".into();
    let app = tauri::Builder::default()
        .manage(games::GameLibrary::load())
        .manage(games::tasks::TaskLibrary::load())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            observed_textures,
            observed_thumbnail,
            games::list_games,
            games::tasks::list_tasks,
            preferences::get_launcher_preferences,
            plugins::list_plugins,
            plugins::peek_pending_schematic,
            plugins::extension_contributions::list_extension_contributions,
            plugins::blueprint_viewers::get_blueprint_viewer_choices,
            plugins::list_blueprints,
            plugins::open_blueprint,
            plugins::read_viewer_chunk,
            plugins::close_viewer,
            plugins::blueprint_library::read_blueprint_thumbnail
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(55));
                eprintln!("MATERIAL_TIMEOUT");
                handle.exit(2);
            });
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Cobble material chain proof")
            .inner_size(1100., 800.)
            .data_directory(data.join("webview-profile"))
            .initialization_script(include_str!("support/material_driver.js"))
            .build()?;
            Ok(())
        })
        .build(context)?;
    app.run(|_, _| {});
    Ok(())
}

// SPDX-License-Identifier: MIT
//! Actual isolated native maintenance actions; no Minecraft, user library or OS dialog automation.
#![allow(dead_code)]
extern crate self as dirs;
pub fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(Into::into)
}
pub fn data_local_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(Into::into)
}
pub fn desktop_dir() -> Option<std::path::PathBuf> {
    data_local_dir().map(|p| p.join("synthetic-desktop"))
}
#[path = "support/maintenance_actions.rs"]
mod actions;
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
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn isolated(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    let temporary = std::env::temp_dir().canonicalize()?;
    path.ancestors()
        .find(|p| {
            p.starts_with(&temporary)
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("cobble-plugin-webview-probe-maintenance-"))
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| "fresh isolated maintenance probe root required".into())
}
fn prepare(base: &Path, packages: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let data = data_local_dir().ok_or("LOCALAPPDATA required")?;
    let config = config_dir().ok_or("APPDATA required")?;
    if isolated(&data)? != base || isolated(&config)? != base {
        return Err("probe roots must share one isolated parent".into());
    }
    let registry = data.join("CobbleLauncher/plugins");
    std::fs::create_dir_all(&registry)?;
    let registry = cobble_plugin_host::PluginRegistry::new(registry.clone());
    for id in [
        "trial-copy",
        "storage-lens",
        "config-doctor",
        "play-manifest",
        "file-inbox",
    ] {
        registry.install_directory(&packages.join(format!("{id}-package")))?;
    }
    std::fs::write(data.join("CobbleLauncher/plugins/.bundled-seeded"), b"1")?;
    let source = base.join("fixture-game");
    std::fs::create_dir_all(source.join("config"))?;
    std::fs::create_dir_all(source.join("saves/synthetic"))?;
    std::fs::create_dir_all(source.join("logs"))?;
    std::fs::create_dir_all(source.join("mods"))?;
    std::fs::write(source.join("config/synthetic.json"), b"{\"volume\":1}\n")?;
    std::fs::write(source.join("options.txt"), b"synthetic:true\n")?;
    std::fs::write(
        source.join("saves/synthetic/level.dat"),
        b"ORIGINAL_SYNTHETIC_WORLD",
    )?;
    std::fs::write(source.join("logs/synthetic.log"), b"SYNTHETIC_NOT_SHARED")?;
    std::fs::write(source.join("accounts.json"), b"SYNTHETIC_NOT_A_SECRET")?;
    std::fs::write(
        source.join("mods/unverified.jar"),
        b"SYNTHETIC_NOT_REDISTRIBUTABLE",
    )?;
    for folder in ["copies", "move-destination"] {
        std::fs::create_dir(base.join(folder))?;
    }
    std::fs::create_dir_all(config.join("CobbleLauncher"))?;
    std::fs::write(
        config.join("CobbleLauncher/games.json"),
        serde_json::to_vec(
            &json!([{"id":"a065f17e","name":"Synthetic maintenance game","version":"1.21.1","directory":source,"status":"installed","error":null,"runtime_overrides":{"java":{"mode":"automatic"},"memory_mb":2048,"jvm_args":[]}}]),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = isolated(&data_local_dir().ok_or("LOCALAPPDATA required")?)?;
    let packages = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("canonical plugin package parent required")?,
    );
    prepare(&base, &packages)?;
    plugins::plugin_views::enable_isolated_probe()?;
    plugins::plugin_confirm::enable_isolated_probe()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.config_mut().identifier = "cobble-maintenance-proof".into();
    let done = Arc::new(AtomicBool::new(false));
    let watcher = done.clone();
    let app=tauri::Builder::default()
        .manage(games::GameLibrary::load())
        .manage(games::tasks::TaskLibrary::load())
        .setup(move|app| {
            let window=tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::App("index.html".into()))
                .title("Isolated maintenance backend proof").visible(false)
                .data_directory(base.join("webview-profile"))
                .on_web_resource_request(|_,response| {
                    *response.status_mut()=tauri::http::StatusCode::OK;
                    *response.body_mut()=std::borrow::Cow::Owned(b"<!doctype html><html><body>Isolated native backend fixture</body></html>".to_vec());
                }).build()?;
            let handle=app.handle().clone();
            let timeout_handle=handle.clone();
            std::thread::spawn(move||{for _ in 0..900 {if watcher.load(Ordering::Acquire){return}std::thread::sleep(std::time::Duration::from_millis(100));}timeout_handle.exit(2);});
            tauri::async_runtime::spawn(async move {
                let result=actions::run(&handle,&window,&base).await;
                plugins::plugin_file_inbox::shutdown();
                for name in ["trial","storage","config","share","inbox"] {plugins::plugin_views::revoke_window(&format!("maintenance-probe-{name}"));}
                let passed=result.is_ok();
                let proof=match result {Ok(value)=>value,Err(error)=>json!({"passed":false,"error":error})};
                let proof=json!({"passed":passed,"checks":proof,"nativeBackend":true,"syntheticOnly":true,"nativeConfirmation":"debug-only isolated registry approval; real OS confirmation/picker not operated","actualPluginUi":false,"minecraftStarted":false,"ownedPid":std::process::id()});
                let written=std::fs::write(base.join("maintenance-native-proof.json"),serde_json::to_vec_pretty(&proof).unwrap());
                eprintln!("MAINTENANCE_NATIVE_PROOF {proof}");
                done.store(true,Ordering::Release);
                handle.exit(if passed&&written.is_ok(){0}else{3});
            });
            Ok(())
        }).build(context)?;
    app.run(|_, _| {});
    Ok(())
}

// SPDX-License-Identifier: MIT
//! Joined native Config Doctor workflow. Run with isolated APPDATA/LOCALAPPDATA.
//! The indexed test driver operates the real bundled UI and native broker/Wasm.
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
pub fn desktop_dir() -> Option<std::path::PathBuf> {
    data_local_dir().map(|root| root.join("synthetic-desktop"))
}
pub fn cache_dir() -> Option<std::path::PathBuf> {
    data_local_dir().map(|root| root.join("cache"))
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
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
static CHECKS: AtomicUsize = AtomicUsize::new(0);
static EVENTS: Mutex<Vec<Value>> = Mutex::new(Vec::new());
#[path = "support/coordinate_book_driver.rs"]
mod coordinate_book_driver;
#[path = "support/diagnostics_driver.rs"]
mod diagnostics_driver;
#[path = "support/inspection_driver.rs"]
mod inspection_driver;
#[path = "support/seed_compass_driver.rs"]
mod seed_compass_driver;
#[path = "support/upgraded_inspection_driver.rs"]
mod upgraded_inspection_driver;
fn upgraded(id: &Value) -> bool {
    id == "modpack-inspector" || id == "world-dashboard"
}

#[tauri::command(rename = "frontend_call")]
async fn observed_frontend_call(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    request: plugins::plugin_frontend_bridge::FrontendRequest,
) -> Result<Value, String> {
    use plugins::plugin_frontend_bridge::FrontendRequest::*;
    let method = match &request {
        MeshPeers(_) => "mesh.peers",
        MeshSubscribe(_) => "mesh.subscribe",
        MeshReceive(_) => "mesh.receive",
        MeshUnsubscribe(_) => "mesh.unsubscribe",
        MeshSend(_) => "mesh.send",
        TrialPreview(_) => "trialCopy.preview",
        TrialDestination(_) => "trialCopy.pickDestination",
        TrialStart(_) => "trialCopy.start",
        TrialTask(_) => "trialCopy.task",
        TrialCancel(_) => "trialCopy.cancel",
        TrialOpen(_) => "trialCopy.openGame",
        StorageOverview(_) => "storageWorkspace.overview",
        StorageDestination(_) => "storageWorkspace.pickDestination",
        StorageClean(_) => "storageWorkspace.cleanCache",
        StorageMove(_) => "storageWorkspace.startMove",
        StorageTask(_) => "storageWorkspace.task",
        StorageCancel(_) => "storageWorkspace.cancel",
        StorageRelease(_) => "storageWorkspace.releaseSource",
        StorageOpen(_) => "storageWorkspace.openSource",
        ConfigList(_) => "configWorkspace.list",
        ConfigRead(_) => "configWorkspace.read",
        ConfigBackup(_) => "configWorkspace.readBackup",
        ConfigWrite(_) => "configWorkspace.write",
        ConfigRestore(_) => "configWorkspace.restore",
        RecoverySnapshot(_) => "recovery.snapshot",
        RecoveryOpen(_) => "recovery.openRequest",
        PlayPreview(_) => "playManifest.preview",
        PlayExport(_) => "playManifest.export",
        InboxPick(_) => "fileInbox.pick",
        InboxPending(_) => "fileInbox.pending",
        InboxOpen(_) => "fileInbox.open",
        InboxRelease(_) => "fileInbox.release",
        LangPick(_) => "langPackage.pick",
        LangCheck(_) => "langPackage.check",
        LangDocument(_) => "langPackage.document",
        LangCancel(_) => "langPackage.cancel",

        TextExport(_) => "exports.text",
        ClipboardWrite(_) => "clipboard.writeText",
        ViewSetDirty(_) => "view.setDirty",
        HandoffReceivers(_) => "handoffs.receivers",
        HandoffSend(_) => "handoffs.send",
        HandoffTake(_) => "handoffs.take",
        DataRead(_) => "data.read",
        DataWrite(_) => "data.write",
        DataRemove(_) => "data.remove",
        ReportPick(_) => "report.pick",
        WorldMetadata(_) => "worlds.metadata",
        SeedMapCapabilities(_) => "seedMap.capabilities",
        SeedMapGenerate(_) => "seedMap.generate",
        SeedMapSearch(_) => "seedMap.search",
        SeedMapCancel(_) => "seedMap.cancel",
        Context(_) => "context",
        InspectionSnapshot(_) => "inspection.snapshot",
        InspectionCheck(_) => "inspection.check",
        List(_) => "configs.list",
        Read(_) => "configs.read",
        Check(_) => "report.check",
        Cancel(_) => "report.cancel",
        KeybindingsPreview(_) => "keybindings.preview",
        KeybindingsApply(_) => "keybindings.apply",
        KeybindingsRestore(_) => "keybindings.restore",
        DiagnosticsList(_) => "diagnostics.list",
        DiagnosticsRead(_) => "diagnostics.read",
        DiagnosticsCheck(_) => "diagnostics.check",
        DiagnosticsPick(_) => "diagnostics.pick",
        WorldSnapshotsList(_) => "worldSnapshots.list",
        WorldSnapshotsCompare(_) => "worldSnapshots.compare",
        WorldSnapshotsOpenRestore(_) => "worldSnapshots.openRestore",
        // Newer unrelated workflows are not observed by this joined fixture.
        _ => "other",
    };
    let request_id = match &request {
        SeedMapCancel(args) => Some(args.request_id.clone()),
        _ => None,
    };
    let requested_version = match &request {
        SeedMapGenerate(args) => Some(args.map.version.clone()),
        SeedMapSearch(args) => Some(args.search.version.clone()),
        _ => None,
    };
    let check = if matches!(&request, Check(_)) {
        CHECKS.fetch_add(1, Ordering::AcqRel) + 1
    } else {
        0
    };
    let label = webview.label().to_owned();
    let result = plugins::plugin_frontend_bridge::frontend_call(app, webview, request).await;
    let target_options = if method.starts_with("keybindings.") {
        std::fs::read_to_string(
            dirs::data_local_dir()
                .unwrap()
                .join("target-game/options.txt"),
        )
        .ok()
    } else {
        None
    };
    let summary = if method.starts_with("mesh.") {
        json!({"ok":result.is_ok()})
    } else {
        coordinate_book_driver::summarized_result(method, &result)
    };
    EVENTS
        .lock()
        .unwrap()
        .push(json!({"method":method,"requestId":request_id,"requestedVersion":requested_version,"check":check,"view":label,"result":summary,"fixtureTargetOptions":target_options}));
    result
}
#[tauri::command]
fn get_app_info(app: tauri::AppHandle, proof: Option<Value>) -> Value {
    if let Some(mut proof) = proof {
        if proof["pluginId"] == "world-dashboard" {
            let unchanged = std::fs::read(
                dirs::data_local_dir()
                    .unwrap()
                    .join("managed-game/saves/FixtureWorld/region/fixture.mca"),
            )
            .unwrap_or_default()
                == b"live synthetic generation remains untouched";
            proof["worldBytesUnchanged"] = json!(unchanged);
            proof["passed"] = json!(proof["passed"] == true && unchanged);
        }
        let target = dirs::data_local_dir()
            .unwrap()
            .join("target-game/options.txt");
        let restored = std::fs::read(&target).unwrap_or_default()
            == b"key_key.forward:key.keyboard.up\nkey_key.jump:key.keyboard.enter\nfov:0.5\n";
        proof["restoredBytesMatch"] = json!(restored);
        proof["passed"] = json!(proof["passed"] == true && restored);
        eprintln!(
            "JOINED_PROOF {}",
            json!({"ui":proof,"events":*EVENTS.lock().unwrap()})
        );
        app.exit(if proof["passed"] == true { 0 } else { 3 });
    }
    json!({"checks":CHECKS.load(Ordering::Acquire),"events":*EVENTS.lock().unwrap()})
}
fn isolated(path: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    if !path.starts_with(std::env::temp_dir().canonicalize()?)
        || !path.ancestors().any(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("cobble-plugin-webview-probe-"))
        })
    {
        return Err("isolated environment required".into());
    }
    Ok(path)
}
fn prepare(source: &Path, data: &Path, config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source_manifest: Value =
        serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    let diagnostics = source_manifest["id"] == "crash-doctor";
    let inspection = source_manifest["permissions"]
        .as_array()
        .is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node["permission"] == "launcher:game-inspection/read")
        });
    let package = data.join("driver-package");
    std::fs::create_dir_all(package.join("ui"))?;
    let index: Value =
        serde_json::from_slice(&std::fs::read(source.join("frontend-assets.json"))?)?;
    for asset in index["files"].as_array().ok_or("asset index")? {
        let path = asset["path"].as_str().ok_or("asset path")?;
        let target = package.join(path);
        std::fs::create_dir_all(target.parent().unwrap())?;
        std::fs::copy(source.join(path), target)?;
    }
    std::fs::copy(
        source.join("component.wasm"),
        package.join("component.wasm"),
    )?;
    let html = std::fs::read_to_string(package.join("ui/index.html"))?.replace(
        "</body>",
        "<script type=\"module\" src=\"./joined-driver.js\"></script></body>",
    );
    std::fs::write(package.join("ui/index.html"), html)?;
    std::fs::write(
        package.join("ui/joined-driver.js"),
        if source_manifest["id"] == "coordinate-book" {
            coordinate_book_driver::DRIVER
        } else if source_manifest["id"] == "seed-compass" {
            seed_compass_driver::DRIVER
        } else if upgraded(&source_manifest["id"]) {
            upgraded_inspection_driver::DRIVER
        } else if inspection {
            inspection_driver::DRIVER
        } else if diagnostics {
            diagnostics_driver::DRIVER
        } else {
            DRIVER
        },
    )?;
    let mut entries = index["files"].as_array().unwrap().clone();
    entries.push(json!({"path":"ui/joined-driver.js"}));
    for entry in &mut entries {
        let bytes = std::fs::read(package.join(entry["path"].as_str().unwrap()))?;
        entry["bytes"] = json!(bytes.len());
        entry["sha256"] = json!(cobble_plugin_host::digest(&bytes));
    }
    let index = serde_json::to_vec(&json!({"schema_version":1,"files":entries}))?;
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    if let Some(icon) = manifest["icon"]["file"].as_str() {
        std::fs::copy(source.join(icon), package.join(icon))?;
    }
    manifest["frontend"]["assets_sha256"] = json!(cobble_plugin_host::digest(&index));
    std::fs::write(package.join("frontend-assets.json"), index)?;
    std::fs::write(package.join("plugin.json"), serde_json::to_vec(&manifest)?)?;
    let registry = data.join("CobbleLauncher/plugins");
    std::fs::create_dir_all(&registry)?;
    cobble_plugin_host::PluginRegistry::new(registry.clone()).install_directory(&package)?;
    // Suppress unrelated bundled viewer discovery in this isolated test only.
    std::fs::write(registry.join(".bundled-seeded"), b"1")?;
    let game = data.join("managed-game");
    std::fs::create_dir_all(game.join("config/nested"))?;
    std::fs::write(
        game.join("config/nested/diagnostic.properties"),
        b"duplicate=one\nduplicate=two\nbad=\\u00ZZ\n",
    )?;
    std::fs::write(game.join("config/unsupported.bin"), b"not selected")?;
    if diagnostics {
        diagnostics_driver::prepare(&game)?;
    }
    std::fs::write(
        game.join("options.txt"),
        b"key_key.forward:key.keyboard.w\nkey_key.jump:key.keyboard.space\nfov:0.75\n",
    )?;
    if inspection {
        inspection_driver::prepare(&game)?;
    }
    if source_manifest["id"] == "seed-compass" {
        seed_compass_driver::prepare(&game)?;
    }
    let target = data.join("target-game");
    std::fs::create_dir_all(&target)?;
    std::fs::write(
        target.join("options.txt"),
        b"key_key.forward:key.keyboard.up\nkey_key.jump:key.keyboard.enter\nfov:0.5\n",
    )?;
    std::fs::create_dir_all(config.join("CobbleLauncher"))?;
    let game_id = if upgraded(&source_manifest["id"]) {
        upgraded_inspection_driver::game_id(data)
    } else {
        "012345abcdef".into()
    };
    let version = if source_manifest["id"] == "seed-compass" {
        seed_compass_driver::VERSION
    } else {
        "1.21.1"
    };
    std::fs::write(
        config.join("CobbleLauncher/games.json"),
        serde_json::to_vec(
            &json!([{"id":game_id,"name":"Joined fixture","version":version,"directory":game,"status":"installed","error":null},{"id":"fedcba543210","name":"Target fixture","version":"1.21.1","directory":target,"status":"installed","error":null}]),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = isolated(dirs::data_local_dir().ok_or("local data")?)?;
    let config = isolated(dirs::config_dir().ok_or("config")?)?;
    if std::env::args().nth(1).as_deref() == Some("--cleanup-fixture") {
        return upgraded_inspection_driver::cleanup(&data);
    }
    let source = PathBuf::from(std::env::args().nth(1).ok_or("package path")?);
    let source_manifest: Value =
        serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    let diagnostics = source_manifest["id"] == "crash-doctor";
    let inspection = source_manifest["permissions"]
        .as_array()
        .is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node["permission"] == "launcher:game-inspection/read")
        });
    let parent = if source_manifest["id"] == "coordinate-book" {
        coordinate_book_driver::PARENT.to_owned()
    } else if source_manifest["id"] == "seed-compass" {
        seed_compass_driver::PARENT.to_owned()
    } else if upgraded(&source_manifest["id"]) {
        upgraded_inspection_driver::parent(
            source_manifest["id"].as_str().ok_or("plugin id")?,
            &upgraded_inspection_driver::game_id(&data),
        )
    } else if inspection {
        inspection_driver::parent(source_manifest["id"].as_str().ok_or("plugin id")?)
    } else if diagnostics {
        diagnostics_driver::PARENT.to_owned()
    } else {
        PARENT.to_owned()
    };
    prepare(&source, &data, &config)?;
    if source_manifest["id"] == "coordinate-book" {
        coordinate_book_driver::install_sender(&source, &data)?;
    }
    let _fixture_credential = if source_manifest["id"] == "world-dashboard" {
        Some(upgraded_inspection_driver::snapshots(
            &data,
            &data.join("managed-game"),
        )?)
    } else {
        None
    };
    plugins::plugin_views::enable_isolated_probe()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app=tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(games::GameLibrary::load()).manage(games::tasks::TaskLibrary::load())
        .register_uri_scheme_protocol("cobble-probe",|_,_|tauri::http::Response::builder().header("Content-Type","text/html; charset=utf-8").body(b"<!doctype html><html><body>Joined Config Doctor native workflow</body></html>".to_vec()).unwrap())
        .register_uri_scheme_protocol("cobble-plugin",plugins::plugin_views::serve_asset)
        .invoke_handler(tauri::generate_handler![observed_frontend_call,get_app_info,games::list_games,plugins::plugin_views::open_plugin_view,plugins::plugin_views::close_plugin_view,plugins::plugin_views::plugin_view_is_dirty,plugins::plugin_views::set_plugin_view_visible,plugins::extension_settings::get_extension_settings,plugins::extension_settings::declarative::save_extension_settings_form])
        .setup(move|app|{
            games::sessions::watch_recovered(app.handle());
            let handle=app.handle().clone();std::thread::spawn(move||{std::thread::sleep(std::time::Duration::from_secs(55));eprintln!("JOINED_TIMEOUT {}",json!(*EVENTS.lock().unwrap()));handle.exit(2);});
            tauri::WebviewWindowBuilder::new(app,"main",tauri::WebviewUrl::External("http://cobble-probe.localhost/index.html".parse()?))
                .title("Cobble joined plugin workflow probe").inner_size(1000.,760.).data_directory(data.join("main-webview"))
                .initialization_script(parent.clone()).build()?;
            Ok(())
        }).build(context)?;
    app.run(|_, _| {});
    Ok(())
}
const DRIVER: &str = r#"
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test){for(let i=0;i<400;i++){if(test())return;await pause(25);}throw Error('driver timed out');}
await wait(()=>document.querySelector('#files li button'));
document.querySelector('#files li button').click();
await wait(()=>!document.querySelector('#source').hidden);
document.querySelector('#select-all').click();
document.querySelector('#check').click();
await wait(()=>!document.querySelector('#check').disabled);
document.querySelector('#bindings-tab').click();
const panel=document.querySelector('#bindings-panel'),select=panel.querySelectorAll('select')[1];
select.value='fedcba543210';select.dispatchEvent(new Event('change'));
const buttons=panel.querySelectorAll('button');buttons[0].click();
await wait(()=>panel.querySelectorAll('tbody tr').length===2);
buttons[1].click();await wait(()=>!buttons[2].hidden&&!buttons[2].disabled);
buttons[2].click();await wait(()=>buttons[2].hidden);
"#;
const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),pause=ms=>new Promise(r=>setTimeout(r,ms));let view;const proof={passed:false};
 try{
 view=(await invoke('open_plugin_view',{pluginId:'config-doctor',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
 let s;for(let i=0;i<1200;i++){s=await invoke('get_app_info');if(s.events.some(e=>e.method==='keybindings.restore'))break;await pause(25);}
 const find=m=>s.events.find(e=>e.method===m)?.result;
 proof.preview=find('keybindings.preview');proof.apply=find('keybindings.apply');proof.restore=find('keybindings.restore');proof.report=find('report.check');
 proof.passed=proof.preview?.Ok?.changes.length===2&&!!proof.apply?.Ok?.backupId&&typeof proof.restore?.Ok==='string'&&proof.report?.Ok?.report?.findings?.length>0;
 await invoke('close_plugin_view',{viewId:view});proof.closed=true;
 }catch(e){proof.error=String(e);}await invoke('get_app_info',{proof});
});
"#;

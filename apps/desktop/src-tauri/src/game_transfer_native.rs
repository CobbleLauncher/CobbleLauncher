// SPDX-License-Identifier: MIT
//! Host-owned one-time game handoff. Only registered stopped games can be offered.
use super::*;
use base64::Engine;
use cobble_core::game_transfer::{self as transfer, Selection, TransferManifest};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

struct Offer {
    game: workspace::GameSnapshot,
    manifest: TransferManifest,
    _lease: management::Operation,
    _world_locks: Vec<std::fs::File>,
    created: Instant,
    peer_id: Option<String>,
    state: String,
}
static OFFERS: OnceLock<Mutex<HashMap<String, Offer>>> = OnceLock::new();
struct FinishedOffer {
    summary: Value,
    finished: Instant,
}
static FINISHED_OFFERS: OnceLock<Mutex<HashMap<String, FinishedOffer>>> = OnceLock::new();
fn offers() -> &'static Mutex<HashMap<String, Offer>> {
    OFFERS.get_or_init(Default::default)
}
fn finished_offers() -> &'static Mutex<HashMap<String, FinishedOffer>> {
    FINISHED_OFFERS.get_or_init(Default::default)
}
fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "TRANSFER_UNAVAILABLE")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn expire(store: &mut HashMap<String, Offer>) {
    store.retain(|_, offer| {
        offer.state != "prepared" || offer.created.elapsed() < Duration::from_secs(300)
    });
}
fn terminal_state(state: &str) -> bool {
    matches!(state, "completed" | "expired" | "cancelled" | "rejected")
}
fn offer_summary(id: &str, offer: &Offer, state: &str, done: u64) -> Value {
    json!({"offerId":id,"gameId":offer.game.id,"gameName":offer.game.name,
        "peerId":offer.peer_id,"state":state,"bytesDone":done,
        "bytesTotal":offer.manifest.total_bytes})
}
fn finish_offer(id: &str, state: &str, done: u64) {
    let offer = offers().lock().ok().and_then(|mut store| store.remove(id));
    if let Some(offer) = offer {
        if let Ok(mut finished) = finished_offers().lock() {
            finished.retain(|_, item| item.finished.elapsed() < Duration::from_secs(3600));
            if finished.len() >= 32 {
                if let Some(oldest) = finished
                    .iter()
                    .min_by_key(|(_, item)| item.finished)
                    .map(|(id, _)| id.clone())
                {
                    finished.remove(&oldest);
                }
            }
            finished.insert(
                id.to_owned(),
                FinishedOffer {
                    summary: offer_summary(id, &offer, state, done),
                    finished: Instant::now(),
                },
            );
        }
    }
}
fn monitor_offer(id: String) {
    tauri::async_runtime::spawn(async move {
        for _ in 0..720 {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if !offers().lock().is_ok_and(|store| store.contains_key(&id)) {
                return;
            }
            let query_id = id.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                mesh::file_transfer_request(json!({"op":"app_file_status","transfer_id":query_id}))
            })
            .await;
            if let Ok(Ok(value)) = result {
                if let Some(state) = value["state"].as_str() {
                    if terminal_state(state) {
                        finish_offer(&id, state, value["bytes_transferred"].as_u64().unwrap_or(0));
                        return;
                    }
                }
            }
        }
        finish_offer(&id, "expired", 0);
    });
}

fn marker_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("TRANSFER_TARGET_INVALID".into());
    }
    Ok(dirs::config_dir()
        .ok_or("TRANSFER_UNAVAILABLE")?
        .join("CobbleLauncher/game-transfers")
        .join(format!("{id}.incomplete.json")))
}
fn origin_path(id: &str) -> Result<PathBuf, String> {
    Ok(marker_path(id)?.with_extension("origin.json"))
}
fn write_marker(game: &Game) -> Result<(), String> {
    let path = marker_path(&game.id)?;
    let origin = origin_path(&game.id)?;
    write_marker_files(&path, &origin, &game.id, &game.directory)
}
fn write_marker_files(path: &Path, origin: &Path, id: &str, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("TRANSFER_TARGET_INVALID")?)
        .map_err(|_| "TRANSFER_IO_FAILED")?;
    let mut record = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(origin)
        .map_err(|_| "TRANSFER_IO_FAILED")?;
    if serde_json::to_writer(&mut record, &json!({"game_id":id,"target":target})).is_err()
        || record.sync_all().is_err()
    {
        let _ = std::fs::remove_file(origin);
        return Err("TRANSFER_IO_FAILED".into());
    }
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => {
            let _ = std::fs::remove_file(origin);
            return Err("TRANSFER_IO_FAILED".into());
        }
    };
    if file.write_all(b"receiving").is_err() || file.sync_all().is_err() {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(origin);
        return Err("TRANSFER_IO_FAILED".into());
    }
    Ok(())
}
fn ensure_retry_allowed_path(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err("TRANSFER_INCOMPLETE".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("TRANSFER_INCOMPLETE".into()),
    }
}
/// Called by generic retry before any install work, including after app restart.
pub(super) fn ensure_install_retry_allowed(game: &Game) -> Result<(), String> {
    ensure_retry_allowed_path(&marker_path(&game.id)?)
}
fn verify_discard_origin(origin: &Path, id: &str, target: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(origin).map_err(|_| "TRANSFER_DISCARD_DENIED")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return Err("TRANSFER_DISCARD_DENIED".into());
    }
    let record: Value =
        serde_json::from_slice(&std::fs::read(origin).map_err(|_| "TRANSFER_DISCARD_DENIED")?)
            .map_err(|_| "TRANSFER_DISCARD_DENIED")?;
    if record["game_id"].as_str() != Some(id)
        || record["target"].as_str() != target.to_str()
        || target.file_name().and_then(|v| v.to_str()) != Some(id)
    {
        return Err("TRANSFER_DISCARD_DENIED".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn game_transfer_discard(app: AppHandle, game_id: String) -> Result<(), String> {
    let game = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|_| "TRANSFER_LIBRARY_UNAVAILABLE")?;
        library
            .games
            .iter()
            .find(|game| game.id == game_id && game.status == "failed")
            .cloned()
            .ok_or("TRANSFER_DISCARD_DENIED")?
    };
    let origin = origin_path(&game_id)?;
    verify_discard_origin(&origin, &game_id, &game.directory)?;
    let base = game.directory.parent().ok_or("TRANSFER_DISCARD_DENIED")?;
    let base = std::fs::canonicalize(base).map_err(|_| "TRANSFER_DISCARD_DENIED")?;
    let stage = base.join(format!(".cobble-transfer-incoming-{game_id}"));
    if let Ok(meta) = std::fs::symlink_metadata(&stage) {
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || std::fs::canonicalize(&stage)
                .ok()
                .and_then(|path| path.parent().map(Path::to_owned))
                != Some(base.clone())
        {
            return Err("TRANSFER_DISCARD_DENIED".into());
        }
        tauri::async_runtime::spawn_blocking(move || std::fs::remove_dir_all(stage))
            .await
            .map_err(|_| "TRANSFER_IO_FAILED")?
            .map_err(|_| "TRANSFER_IO_FAILED")?;
    }
    management::remove_game(app, game_id.clone(), true).await?;
    let _ = std::fs::remove_file(marker_path(&game_id)?);
    let _ = std::fs::remove_file(origin);
    Ok(())
}

#[tauri::command]
pub fn game_transfer_context(app: AppHandle, game_id: String) -> Result<Value, String> {
    let game = workspace::snapshot(&app, &game_id)?;
    let stopped = registered_game_stopped(&app, &game_id)?;
    Ok(
        json!({"gameId":game.id,"name":game.name,"version":game.version,
        "loaderKind":game.loader_kind,"loaderVersion":game.loader_version,"stopped":stopped,
        "meshAvailable":serde_json::to_value(mesh::mesh_availability()).map_err(|_| "TRANSFER_UNAVAILABLE")?["available"],
        "availableScopes":["saves","mods","resourcepacks","shaderpacks","config","options"]}),
    )
}

#[tauri::command]
pub async fn game_transfer_prepare(
    app: AppHandle,
    game_id: String,
    selection: Selection,
) -> Result<Value, String> {
    let offer_id = nonce()?;
    let timer_id = offer_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<Value, String> {
        let (game, lease) = workspace::acquire(&app, &game_id)?;
        let locks = if selection.saves { workspace::world_locks(&game.directory)? } else { Vec::new() };
        let manifest = transfer::prepare(&game.directory, &game.version,
            game.loader_kind.as_deref(), game.loader_version.as_deref(), selection, &AtomicBool::new(false))
            .map_err(|e| e.to_string())?;
        if manifest.files.len() > 8_000 || manifest.total_bytes > 16 * 1024 * 1024 * 1024 ||
            manifest.files.iter().any(|file| file.path.len() > 240) {
            return Err("TRANSFER_MANIFEST_TOO_LARGE".into());
        }
        let result = json!({"offerId":offer_id,"gameName":game.name,"version":game.version,
            "loaderKind":game.loader_kind,"loaderVersion":game.loader_version,
            "fileCount":manifest.files.len(),"totalBytes":manifest.total_bytes,"sharedFileNames":true});
        let mut store = offers().lock().map_err(|_| "TRANSFER_UNAVAILABLE")?;
        expire(&mut store);
        if store.len() >= 8 { return Err("TRANSFER_BUSY".into()); }
        store.insert(offer_id, Offer { game, manifest, _lease:lease, _world_locks:locks, created:Instant::now(), peer_id:None, state:"prepared".into() });
        Ok(result)
    }).await.map_err(|_| "TRANSFER_UNAVAILABLE".to_string())??;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(300)).await;
        if let Ok(mut store) = offers().lock() {
            if store
                .get(&timer_id)
                .is_some_and(|offer| offer.state == "prepared")
            {
                store.remove(&timer_id);
            }
        }
    });
    Ok(result)
}

#[tauri::command]
pub async fn game_transfer_send(offer_id: String, peer_id: String) -> Result<Value, String> {
    if peer_id.is_empty() || peer_id.len() > 128 {
        return Err("TRANSFER_PEER_INVALID".into());
    }
    let command = {
        let mut store = offers().lock().map_err(|_| "TRANSFER_UNAVAILABLE")?;
        expire(&mut store);
        let offer = store.get_mut(&offer_id).ok_or("TRANSFER_OFFER_EXPIRED")?;
        if offer.state != "prepared" {
            return Err("TRANSFER_ALREADY_SENT".into());
        }
        offer.state = "sending".into();
        let files = offer.manifest.files.iter().map(|file| {
            let id = sha2::Sha256::digest(file.path.as_bytes());
            let file_id = id.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let sha256 = file.sha256.iter().map(|b| format!("{b:02x}")).collect::<String>();
            json!({"file_id":file_id,"relative_path":file.path,"path":offer.game.directory.join(&file.path),
                "size":file.bytes,"sha256":sha256})
        }).collect::<Vec<_>>();
        json!({"op":"app_file_offer","peer_id":peer_id,"manifest_id":offer_id,"metadata":{
            "minecraft_version":offer.manifest.minecraft_version,
            "loader_kind":offer.manifest.loader_kind.as_deref().unwrap_or("none"),
            "loader_version":offer.manifest.loader_version.as_deref().unwrap_or(""),
            "game_name":offer.game.name.chars().take(80).collect::<String>()},"files":files})
    };
    let answer = tauri::async_runtime::spawn_blocking(move || mesh::file_transfer_request(command))
        .await
        .map_err(|_| "TRANSFER_UNAVAILABLE".to_owned())?;
    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            finish_offer(&offer_id, "expired", 0);
            return Err(error);
        }
    };
    let manifest_id = answer["manifest_id"]
        .as_str()
        .ok_or("TRANSFER_PROTOCOL_INVALID")?
        .to_owned();
    if !offers().lock().is_ok_and(|store| {
        store
            .get(&offer_id)
            .is_some_and(|offer| offer.state == "sending")
    }) {
        let _ =
            mesh::file_transfer_request(json!({"op":"app_file_cancel","transfer_id":manifest_id}));
        return Err("TRANSFER_CANCELLED".into());
    }
    let announce = json!({"op":"app_file_announce","peer_id":peer_id,"manifest_id":manifest_id});
    let announced =
        tauri::async_runtime::spawn_blocking(move || mesh::file_transfer_request(announce))
            .await
            .map_err(|_| "TRANSFER_UNAVAILABLE".to_owned())?;
    if let Err(error) = announced {
        let _ =
            mesh::file_transfer_request(json!({"op":"app_file_cancel","transfer_id":manifest_id}));
        finish_offer(&offer_id, "expired", 0);
        return Err(error);
    }
    if let Ok(mut store) = offers().lock() {
        if let Some(offer) = store.get_mut(&offer_id) {
            offer.peer_id = Some(peer_id);
            offer.state = "waiting".into();
        }
    }
    monitor_offer(offer_id.clone());
    Ok(
        json!({"manifestId":manifest_id,"totalFiles":answer["total_files"],
        "totalBytes":answer["total_bytes"],"state":"waiting"}),
    )
}

#[tauri::command]
pub fn game_transfer_cancel(offer_id: String) -> Result<(), String> {
    let mut store = offers().lock().map_err(|_| "TRANSFER_UNAVAILABLE")?;
    let offer = store.remove(&offer_id).ok_or("TRANSFER_OFFER_EXPIRED")?;
    if offer.peer_id.is_some() {
        drop(store);
        let _ = mesh::file_transfer_request(json!({"op":"app_file_cancel","transfer_id":offer_id}));
    }
    Ok(())
}

#[tauri::command]
pub async fn game_transfer_offers() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let result = mesh::file_transfer_request(json!({"op":"app_file_offers"}))?;
        let notices = result["offers"]
            .as_array()
            .or_else(|| result.as_array())
            .ok_or("TRANSFER_PROTOCOL_INVALID")?;
        let mut output = Vec::new();
        for notice in notices.iter().take(32) {
            let peer = notice["peer_id"]
                .as_str()
                .ok_or("TRANSFER_PROTOCOL_INVALID")?;
            let id = notice["manifest_id"]
                .as_str()
                .ok_or("TRANSFER_PROTOCOL_INVALID")?;
            let (manifest, game_name) = remote_manifest(peer, id)?;
            let scopes = manifest
                .files
                .iter()
                .filter_map(|file| file.path.split('/').next())
                .map(|scope| {
                    if scope == "options.txt" {
                        "options"
                    } else {
                        scope
                    }
                })
                .collect::<std::collections::BTreeSet<_>>();
            output.push(json!({"peerId":peer,"manifestId":id,"gameName":game_name,
                "version":manifest.minecraft_version,"loaderKind":manifest.loader_kind,
                "loaderVersion":manifest.loader_version,"fileCount":manifest.files.len(),
                "totalBytes":manifest.total_bytes,"scopes":scopes}));
        }
        Ok(json!(output))
    })
    .await
    .map_err(|_| "TRANSFER_UNAVAILABLE".to_string())?
}

#[tauri::command]
pub fn game_transfer_list_outgoing() -> Result<Value, String> {
    let mut store = offers().lock().map_err(|_| "TRANSFER_UNAVAILABLE")?;
    expire(&mut store);
    let mut output = store
        .iter()
        .map(|(id, offer)| offer_summary(id, offer, &offer.state, 0))
        .collect::<Vec<_>>();
    drop(store);
    if let Ok(mut finished) = finished_offers().lock() {
        finished.retain(|_, item| item.finished.elapsed() < Duration::from_secs(3600));
        output.extend(finished.values().map(|item| item.summary.clone()));
    }
    Ok(json!(output))
}

#[tauri::command]
pub async fn game_transfer_status(offer_id: String) -> Result<Value, String> {
    if let Ok(finished) = finished_offers().lock() {
        if let Some(item) = finished.get(&offer_id) {
            return Ok(item.summary.clone());
        }
    }
    {
        let mut store = offers().lock().map_err(|_| "TRANSFER_UNAVAILABLE")?;
        expire(&mut store);
        if !store.contains_key(&offer_id) {
            return Err("TRANSFER_OFFER_EXPIRED".into());
        }
    }
    let query = json!({"op":"app_file_status","transfer_id":offer_id});
    let result = tauri::async_runtime::spawn_blocking(move || mesh::file_transfer_request(query))
        .await
        .map_err(|_| "TRANSFER_UNAVAILABLE")??;
    let state = result["state"]
        .as_str()
        .ok_or("TRANSFER_PROTOCOL_INVALID")?;
    let display_state = match state {
        "offered" | "approved" | "pending" => "waiting",
        "transferring" => "transferring",
        "completed" => "completed",
        "expired" => "expired",
        "cancelled" => "cancelled",
        "rejected" => "rejected",
        _ => return Err("TRANSFER_PROTOCOL_INVALID".into()),
    };
    if terminal_state(state) {
        finish_offer(
            &offer_id,
            display_state,
            result["bytes_transferred"].as_u64().unwrap_or(0),
        );
        if let Ok(finished) = finished_offers().lock() {
            if let Some(item) = finished.get(&offer_id) {
                return Ok(item.summary.clone());
            }
        }
    }
    Ok(
        json!({"offerId":offer_id,"state":display_state,"bytesDone":result["bytes_transferred"],
        "bytesTotal":result["total_bytes"]}),
    )
}

fn parse_hex32(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("TRANSFER_PROTOCOL_INVALID".into());
    }
    let mut out = [0u8; 32];
    for (i, item) in out.iter_mut().enumerate() {
        *item = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| "TRANSFER_PROTOCOL_INVALID")?;
    }
    Ok(out)
}

fn remote_manifest(peer_id: &str, manifest_id: &str) -> Result<(TransferManifest, String), String> {
    let mut files = Vec::new();
    let mut metadata: Option<Value> = None;
    let mut total_files = None;
    let mut total_bytes = None;
    loop {
        let page = mesh::file_transfer_request(json!({"op":"app_file_manifest","peer_id":peer_id,
            "manifest_id":manifest_id,"offset":files.len()}))?;
        let count = page["total_files"]
            .as_u64()
            .ok_or("TRANSFER_PROTOCOL_INVALID")?;
        let bytes = page["total_bytes"]
            .as_u64()
            .ok_or("TRANSFER_PROTOCOL_INVALID")?;
        if count > 8_192
            || bytes > 16 * 1024 * 1024 * 1024
            || page["offset"].as_u64() != Some(files.len() as u64)
            || page["manifest_id"].as_str() != Some(manifest_id)
            || total_files.is_some_and(|old| old != count)
            || total_bytes.is_some_and(|old| old != bytes)
        {
            return Err("TRANSFER_PROTOCOL_INVALID".into());
        }
        total_files = Some(count);
        total_bytes = Some(bytes);
        let next = page["metadata"]
            .as_object()
            .ok_or("TRANSFER_PROTOCOL_INVALID")?;
        if metadata
            .as_ref()
            .is_some_and(|old| old != &page["metadata"])
        {
            return Err("TRANSFER_PROTOCOL_INVALID".into());
        }
        metadata = Some(Value::Object(next.clone()));
        let entries = page["files"]
            .as_array()
            .ok_or("TRANSFER_PROTOCOL_INVALID")?;
        if entries.len() > 64 || (entries.is_empty() && files.len() < count as usize) {
            return Err("TRANSFER_PROTOCOL_INVALID".into());
        }
        for value in entries {
            let path = value["relative_path"]
                .as_str()
                .ok_or("TRANSFER_PROTOCOL_INVALID")?;
            let id = value["file_id"]
                .as_str()
                .ok_or("TRANSFER_PROTOCOL_INVALID")?;
            let expected_id = sha2::Sha256::digest(path.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            if id != expected_id {
                return Err("TRANSFER_PROTOCOL_INVALID".into());
            }
            files.push(transfer::TransferFile {
                path: path.into(),
                bytes: value["size"].as_u64().ok_or("TRANSFER_PROTOCOL_INVALID")?,
                sha256: parse_hex32(
                    value["sha256"]
                        .as_str()
                        .ok_or("TRANSFER_PROTOCOL_INVALID")?,
                )?,
            });
        }
        if files.len() == count as usize {
            break;
        }
    }
    let metadata = metadata.ok_or("TRANSFER_PROTOCOL_INVALID")?;
    let manifest = TransferManifest {
        schema_version: 1,
        minecraft_version: metadata["minecraft_version"]
            .as_str()
            .ok_or("TRANSFER_PROTOCOL_INVALID")?
            .into(),
        loader_kind: metadata["loader_kind"]
            .as_str()
            .filter(|kind| *kind != "none")
            .map(str::to_owned),
        loader_version: metadata["loader_version"]
            .as_str()
            .filter(|version| !version.is_empty())
            .map(str::to_owned),
        files,
        total_bytes: total_bytes.unwrap_or(0),
    };
    transfer::validate(&manifest).map_err(|_| "TRANSFER_PROTOCOL_INVALID")?;
    let game_name = metadata["game_name"]
        .as_str()
        .unwrap_or("共享游戏")
        .chars()
        .take(80)
        .collect();
    Ok((manifest, game_name))
}

fn fresh_game(id: String, name: String, directory: PathBuf, manifest: &TransferManifest) -> Game {
    Game {
        id,
        name,
        last_launched_at: None,
        appearance_preset: game_appearance::default_preset(),
        icon_ref: None,
        cover_ref: None,
        pinned: false,
        version: manifest.minecraft_version.clone(),
        loader_kind: manifest.loader_kind.clone(),
        loader_version: manifest.loader_version.clone(),
        directory,
        profile_id: None,
        runtime_overrides: cobble_minecraft::LaunchOverrides::default(),
        download_source: None,
        status: "installing".into(),
        error: None,
        progress: None,
        launch_state: idle(),
        launch_error: None,
        log_path: None,
        content_installing: false,
        pack_name: None,
        pack_version: None,
        pack_source_id: None,
        pack_origin: None,
        pack_optional_files: Vec::new(),
        pack_mod_files: Vec::new(),
        launcher_import_id: None,
    }
}

fn local_candidate(app: &AppHandle, manifest: &TransferManifest) -> Option<PathBuf> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().ok()?;
    library
        .games
        .iter()
        .find(|game| {
            game.status == "installed"
                && game.launch_state == "idle"
                && !game.content_installing
                && game.version == manifest.minecraft_version
                && game.loader_kind == manifest.loader_kind
                && game.loader_version == manifest.loader_version
        })
        .map(|game| game.directory.clone())
}

fn receive_blocks(
    app: &AppHandle,
    task_id: &str,
    peer: &str,
    manifest_id: &str,
    manifest: &TransferManifest,
    staging: &Path,
    local: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let approved = mesh::file_transfer_request(json!({"op":"app_file_approve","peer_id":peer,
        "manifest_id":manifest_id,"transfer_id":manifest_id}))?;
    let transfer_id = approved["transfer_id"]
        .as_str()
        .ok_or("TRANSFER_PROTOCOL_INVALID")?;
    let missing = transfer::missing(manifest, local, staging, cancel).map_err(|e| e.to_string())?;
    let wanted = missing
        .paths
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let mut completed = manifest.total_bytes.saturating_sub(missing.bytes);
    for file in &manifest.files {
        if !wanted.contains(&file.path) {
            continue;
        }
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err("TRANSFER_CANCELLED".into());
        }
        let path = staging.join(&file.path);
        std::fs::create_dir_all(path.parent().ok_or("TRANSFER_PROTOCOL_INVALID")?)
            .map_err(|_| "TRANSFER_IO_FAILED")?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "TRANSFER_IO_FAILED")?;
        let mut digest = sha2::Sha256::new();
        let mut size = 0u64;
        let file_id = sha2::Sha256::digest(file.path.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let chunks = file.bytes.div_ceil(32 * 1024);
        for index in 0..chunks {
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Err("TRANSFER_CANCELLED".into());
            }
            let reply = mesh::file_transfer_request(
                json!({"op":"app_file_chunk","transfer_id":transfer_id,
                "file_id":file_id,"chunk_index":index}),
            )?;
            if reply["transfer_id"].as_str() != Some(transfer_id)
                || reply["file_id"].as_str() != Some(&file_id)
                || reply["chunk_index"].as_u64() != Some(index)
            {
                return Err("TRANSFER_PROTOCOL_INVALID".into());
            }
            let encoded = reply["data_base64"]
                .as_str()
                .ok_or("TRANSFER_PROTOCOL_INVALID")?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|_| "TRANSFER_PROTOCOL_INVALID")?;
            let advertised = parse_hex32(
                reply["chunk_sha256"]
                    .as_str()
                    .ok_or("TRANSFER_PROTOCOL_INVALID")?,
            )?;
            if bytes.is_empty()
                || bytes.len() > 32 * 1024
                || reply["size"].as_u64() != Some(bytes.len() as u64)
                || <[u8; 32]>::from(sha2::Sha256::digest(&bytes)) != advertised
                || size.saturating_add(bytes.len() as u64) > file.bytes
            {
                return Err("TRANSFER_VERIFY_FAILED".into());
            }
            output.write_all(&bytes).map_err(|_| "TRANSFER_IO_FAILED")?;
            digest.update(&bytes);
            size += bytes.len() as u64;
            completed = completed.saturating_add(bytes.len() as u64);
            tasks::external_progress(
                app,
                task_id,
                "receiving",
                completed as usize,
                manifest.total_bytes as usize,
            );
        }
        output.sync_all().map_err(|_| "TRANSFER_IO_FAILED")?;
        if size != file.bytes || <[u8; 32]>::from(digest.finalize()) != file.sha256 {
            return Err("TRANSFER_VERIFY_FAILED".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn game_transfer_accept(
    app: AppHandle,
    peer_id: String,
    manifest_id: String,
    name: String,
    use_default_directory: bool,
) -> Result<Value, String> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("TRANSFER_NAME_INVALID".into());
    }
    let peer = peer_id.clone();
    let id = manifest_id.clone();
    let (manifest, _) = tauri::async_runtime::spawn_blocking(move || remote_manifest(&peer, &id))
        .await
        .map_err(|_| "TRANSFER_UNAVAILABLE")??;
    let base = native_readiness_space::creation_directory(&app, use_default_directory)?;
    std::fs::create_dir_all(&base).map_err(|_| "TRANSFER_TARGET_INVALID")?;
    let game_id = next_pack_id()?;
    let game = fresh_game(game_id.clone(), name, base.join(&game_id), &manifest);
    let settings = cobble_minecraft::read_download_settings().map_err(|e| e.to_string())?;
    let preflight = native_readiness_space::estimate(
        &game.version,
        game.loader_kind.as_deref(),
        None,
        &game.directory,
        settings,
        &cobble_minecraft::JavaOverride::Automatic,
        cobble_core::OperationControl::new(Arc::new(|_| {})),
    )
    .await?;
    if !preflight.can_proceed {
        return Err("SPACE_INSUFFICIENT".into());
    }
    let (volume_id, _) =
        cobble_platform::disk_volume_identity(&base).ok_or("SPACE_VOLUME_UNAVAILABLE")?;
    let needed = manifest
        .total_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
        .and_then(|bytes| {
            bytes.checked_add(
                preflight
                    .volumes
                    .iter()
                    .find(|entry| entry.volume.id == volume_id)
                    .map_or(0, |entry| entry.additional_peak_bytes),
            )
        })
        .ok_or("SPACE_OVERFLOW")?;
    let free = cobble_platform::available_disk_space(&base).ok_or("SPACE_VOLUME_UNAVAILABLE")?;
    if free < needed {
        return Err("SPACE_INSUFFICIENT".into());
    }
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|_| "TRANSFER_LIBRARY_UNAVAILABLE")?;
        if library.games.iter().any(|g| g.id == game_id) {
            return Err("TRANSFER_TARGET_INVALID".into());
        }
        write_marker(&game)?;
        library.games.push(game.clone());
        if let Err(error) = save(&library.games) {
            library.games.pop();
            let _ = std::fs::remove_file(marker_path(&game_id)?);
            let _ = std::fs::remove_file(origin_path(&game_id)?);
            return Err(error);
        }
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let task_id =
        match tasks::register_external(&app, &game_id, "接收游戏", "game_transfer", cancel.clone())
        {
            Ok(id) => id,
            Err(error) => {
                update_game(
                    &app,
                    &game_id,
                    |game| {
                        game.status = "failed".into();
                        game.error = Some(error.clone());
                    },
                    true,
                );
                return Err(error);
            }
        };
    let result_id = task_id.clone();
    let return_game_id = game.id.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            tasks::wait_external_turn(&app, &task_id).await?;
            let local = local_candidate(&app, &manifest);
            let staging = base.join(format!(".cobble-transfer-incoming-{game_id}"));
            std::fs::create_dir(&staging).map_err(|_| "TRANSFER_TARGET_INVALID")?;
            let worker_app = app.clone();
            let worker_task = task_id.clone();
            let worker_peer = peer_id.clone();
            let worker_manifest = manifest_id.clone();
            let worker_game = game.clone();
            let install_cancel = cancel.clone();
            let outcome = tauri::async_runtime::spawn_blocking(move || {
                receive_blocks(
                    &worker_app,
                    &worker_task,
                    &worker_peer,
                    &worker_manifest,
                    &manifest,
                    &staging,
                    local.as_deref(),
                    &cancel,
                )?;
                tasks::external_progress(&worker_app, &worker_task, "verifying", 0, 0);
                transfer::receive(
                    &manifest,
                    local.as_deref(),
                    &staging,
                    &worker_game.directory,
                    &cancel,
                )
                .map_err(|e| e.to_string())?;
                std::fs::remove_file(marker_path(&worker_game.id)?)
                    .map_err(|_| "TRANSFER_MARKER_FAILED")?;
                let _ = std::fs::remove_dir_all(&staging);
                // Local completion follows final verification. If this notification
                // fails, the sender remains pending rather than falsely completing.
                let _ = mesh::file_transfer_request(
                    json!({"op":"app_file_complete","transfer_id":worker_manifest}),
                );
                Ok::<_, String>(())
            })
            .await
            .map_err(|_| "TRANSFER_IO_FAILED")?;
            outcome?;
            let progress_app = app.clone();
            let progress_id = task_id.clone();
            let control = cobble_core::OperationControl::new(Arc::new(move |progress| {
                tasks::external_progress(
                    &progress_app,
                    &progress_id,
                    &progress.phase,
                    progress.completed,
                    progress.total,
                );
            }));
            let watch_control = control.clone();
            let finished = Arc::new(AtomicBool::new(false));
            let watch_finished = finished.clone();
            tauri::async_runtime::spawn(async move {
                while !watch_finished.load(std::sync::atomic::Ordering::Acquire) {
                    if install_cancel.load(std::sync::atomic::Ordering::Acquire) {
                        watch_control.cancel();
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            });
            let preparation = async {
                let settings =
                    cobble_minecraft::read_download_settings().map_err(|e| e.to_string())?;
                let source = settings.download_source().map_err(|e| e.to_string())?;
                let installed = match (game.loader_kind.as_deref(), game.loader_version.as_deref())
                {
                    (None, None) => {
                        cobble_core::install_game_controlled(
                            &game.version,
                            &game.directory,
                            source,
                            control.clone(),
                        )
                        .await
                    }
                    (Some("fabric"), Some(loader)) => {
                        cobble_core::install_fabric_controlled(
                            &game.version,
                            loader,
                            &game.directory,
                            source,
                            control.clone(),
                        )
                        .await
                    }
                    (Some("forge" | "neoforge"), Some(loader)) => {
                        let kind = if game.loader_kind.as_deref() == Some("neoforge") {
                            cobble_core::ForgeKind::NeoForge
                        } else {
                            cobble_core::ForgeKind::Forge
                        };
                        cobble_core::install_forge_controlled(
                            &game.version,
                            loader,
                            kind,
                            &game.directory,
                            source,
                            control.clone(),
                        )
                        .await
                    }
                    _ => return Err("TRANSFER_LOADER_UNSUPPORTED".into()),
                }
                .map_err(|e| e.to_string())?;
                cobble_core::ensure_launch_java_controlled(
                    installed.java_major,
                    &cobble_minecraft::JavaOverride::Automatic,
                    control.clone(),
                )
                .await
                .map_err(|e| e.to_string())?;
                control.check_cancelled().map_err(|e| e.to_string())?;
                cobble_core::prepare_launch(&game.version, &game.directory)
                    .await
                    .map_err(|e| e.to_string())?;
                control.check_cancelled().map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            }
            .await;
            finished.store(true, std::sync::atomic::Ordering::Release);
            preparation?;
            update_game(
                &app,
                &game_id,
                |game| {
                    game.status = "installed".into();
                    game.error = None;
                },
                true,
            );
            Ok::<_, String>(())
        }
        .await;
        if let Err(error) = &result {
            update_game(
                &app,
                &game_id,
                |game| {
                    game.status = "failed".into();
                    game.error = Some(error.clone());
                },
                true,
            );
        }
        tasks::finish_external(&app, &task_id, result.map(|_| Some(game_id)));
    });
    Ok(json!({"taskId":result_id,"gameId":return_game_id}))
}

#[tauri::command]
pub async fn game_transfer_reject(peer_id: String, manifest_id: String) -> Result<Value, String> {
    if peer_id.is_empty() || manifest_id.is_empty() {
        return Err("TRANSFER_PROTOCOL_INVALID".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        mesh::file_transfer_request(json!({
        "op":"app_file_reject","peer_id":peer_id,"manifest_id":manifest_id}))
    })
    .await
    .map_err(|_| "TRANSFER_UNAVAILABLE")?
}

#[cfg(test)]
#[path = "game_transfer_native/tests.rs"]
mod tests;

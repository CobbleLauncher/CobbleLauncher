// SPDX-License-Identifier: MIT
//! Parent-private game adapters for selected immutable Inbox snapshots.
use super::*;
use serde_json::{Value, json};
use std::path::Path;
#[derive(Clone)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader: Option<String>,
    pub available: bool,
}
pub fn targets(app: &AppHandle, bound: Option<&str>) -> Result<Vec<Target>, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    if library.load_error.is_some() {
        return Err("FILE_INBOX_UNAVAILABLE".into());
    }
    let mut games: Vec<_> = library
        .games
        .iter()
        .filter(|g| g.status == "installed" && bound.is_none_or(|id| g.id == id))
        .collect();
    games.sort_by(|a, b| {
        b.last_launched_at
            .cmp(&a.last_launched_at)
            .then(a.name.cmp(&b.name))
    });
    Ok(games
        .into_iter()
        .take(256)
        .map(|g| Target {
            id: g.id.clone(),
            name: g.name.clone(),
            version: g.version.clone(),
            loader: g.loader_kind.clone(),
            available: g.launch_state == "idle" && !g.content_installing,
        })
        .collect())
}
/// Uses existing pinned MRPACK selection and import preview. Does not install.
pub fn mrpack(app: &AppHandle, source: &Path) -> Result<Value, String> {
    let id = next_pack_id()?;
    let archive = pack_archive_path(&id)?;
    fs::create_dir_all(archive.parent().ok_or("FILE_INBOX_UNAVAILABLE")?)
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    fs::copy(source, &archive).map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    match cobble_content::read_mrpack(&archive) {
        Ok(pack) => serde_json::to_value(store_pack_preview(app, id, pack, None)?)
            .map_err(|_| "FILE_INBOX_UNAVAILABLE".into()),
        Err(_) => {
            let _ = fs::remove_file(archive);
            Err("FILE_INBOX_FORMAT_INVALID".into())
        }
    }
}
/// Confirmation is owned by the main window, and a fresh existing management lease protects writes.
pub fn import(app: &AppHandle, game_id: &str, source: &Path, kind: &str) -> Result<Value, String> {
    let (game, _operation) =
        management::begin(app, game_id, true).map_err(|_| "FILE_INBOX_GAME_BUSY")?;
    match kind {
        "mod" => {
            cobble_content::import_mods(&game.directory, &[source.to_owned()])
                .map_err(|_| "FILE_INBOX_IMPORT_FAILED")?;
        }
        "resource-pack" | "shader-pack" => {
            let kind = if kind == "resource-pack" {
                cobble_content::LocalPackKind::ResourcePack
            } else {
                cobble_content::LocalPackKind::ShaderPack
            };
            cobble_content::import_local_packs(&game.directory, kind, &[source.to_owned()])
                .map_err(|_| "FILE_INBOX_IMPORT_FAILED")?;
        }
        _ => return Err("FILE_INBOX_HANDLER_INVALID".into()),
    }
    Ok(json!({"kind":"game-content","gameId":game_id,"contentKind":kind,"imported":true}))
}
/// Reuses the existing CurseForge pinned archive and unresolved-file preparation flow.
pub fn curseforge(app: &AppHandle, source: &Path) -> Result<Value, String> {
    let id = next_pack_id()?;
    let archive = curseforge_pack::archive(&id)?;
    fs::create_dir_all(archive.parent().ok_or("FILE_INBOX_UNAVAILABLE")?)
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    fs::copy(source, &archive).map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    match cobble_core::inspect_curseforge_pack(&archive) {
        Ok(pack) => serde_json::to_value(curseforge_pack::store(app, id, pack, None)?)
            .map_err(|_| "FILE_INBOX_UNAVAILABLE".into()),
        Err(_) => {
            let _ = fs::remove_file(archive);
            Err("FILE_INBOX_FORMAT_INVALID".into())
        }
    }
}

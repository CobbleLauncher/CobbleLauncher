// SPDX-License-Identifier: MIT
//! Crate-private metadata and stopped-instance leases for scoped host workspaces.
use super::*;
#[derive(Clone)]
pub struct GameSnapshot {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
    pub directory: PathBuf,
}
fn snapshot_of(game: &Game) -> GameSnapshot {
    GameSnapshot {
        id: game.id.clone(),
        name: game.name.clone(),
        version: game.version.clone(),
        loader_kind: game.loader_kind.clone(),
        loader_version: game.loader_version.clone(),
        directory: game.directory.clone(),
    }
}
pub fn snapshot(app: &AppHandle, id: &str) -> Result<GameSnapshot, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "PLUGIN_GAME_SCOPE")?;
    if library.load_error.is_some() {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    let game = library
        .games
        .iter()
        .find(|g| g.id == id && g.status == "installed")
        .ok_or("PLUGIN_GAME_SCOPE")?;
    Ok(snapshot_of(game))
}
pub fn acquire(app: &AppHandle, id: &str) -> Result<(GameSnapshot, management::Operation), String> {
    let (game, lease) = management::begin(app, id, true).map_err(|_| "PLUGIN_INSTANCE_BUSY")?;
    if sessions::recover(&game)
        .map_err(|_| "PLUGIN_INSTANCE_BUSY")?
        .is_some()
    {
        return Err("PLUGIN_INSTANCE_BUSY".into());
    }
    Ok((snapshot_of(&game), lease))
}
/// Hold existing Java world locks while reading consistent stopped-world data.
pub fn world_locks(source: &std::path::Path) -> Result<Vec<std::fs::File>, String> {
    let saves = source.join("saves");
    if !saves.try_exists().map_err(|_| "PLUGIN_WORLD_BUSY")? {
        return Ok(vec![]);
    }
    fn real(path: &std::path::Path, directory: bool) -> Result<(), String> {
        let m = fs::symlink_metadata(path).map_err(|_| "PLUGIN_WORLD_BUSY")?;
        if m.file_type().is_symlink() || (if directory { !m.is_dir() } else { !m.is_file() }) {
            return Err("PLUGIN_WORLD_BUSY".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("PLUGIN_WORLD_BUSY".into());
            }
        }
        Ok(())
    }
    real(&saves, true)?;
    let mut locks = vec![];
    for (count, entry) in fs::read_dir(saves)
        .map_err(|_| "PLUGIN_WORLD_BUSY")?
        .enumerate()
    {
        if count >= 1024 {
            return Err("PLUGIN_WORLD_BUSY".into());
        }
        let entry = entry.map_err(|_| "PLUGIN_WORLD_BUSY")?;
        let path = entry.path();
        let m = fs::symlink_metadata(&path).map_err(|_| "PLUGIN_WORLD_BUSY")?;
        if m.file_type().is_symlink() {
            return Err("PLUGIN_WORLD_BUSY".into());
        }
        if !m.is_dir() {
            continue;
        }
        real(&path, true)?;
        let path = path.join("session.lock");
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                real(&path, false)?;
                let f = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .map_err(|_| "PLUGIN_WORLD_BUSY")?;
                f.try_lock().map_err(|_| "PLUGIN_WORLD_BUSY")?;
                locks.push(f)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("PLUGIN_WORLD_BUSY".into()),
        }
    }
    Ok(locks)
}

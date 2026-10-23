// SPDX-License-Identifier: MIT
//! Saved-server IDs are checked against the current instance under its launch lock.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
pub struct SavedServer {
    id: String,
    name: String,
    address: String,
    can_connect: bool,
}
#[derive(Serialize)]
pub struct SavedServers {
    items: Vec<SavedServer>,
    direct_connect: bool,
}
fn entries(game: &Game) -> Result<Vec<SavedServer>, String> {
    let value = game_inspection::servers::saved(&game.directory)?;
    let items = value["items"].as_array().ok_or("无法读取服务器列表")?;
    Ok(items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let name = item["name"].as_str().unwrap_or_default().to_owned();
            let address = item["address"].as_str().unwrap_or_default().to_owned();
            let mut hash = Sha256::new();
            hash.update(index.to_be_bytes());
            hash.update(name.as_bytes());
            hash.update([0]);
            hash.update(address.as_bytes());
            let id = hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let can_connect =
                cobble_core::direct_connect_arguments(&game.version, &address).is_ok();
            SavedServer {
                id,
                name,
                address,
                can_connect,
            }
        })
        .collect())
}
#[tauri::command]
pub async fn list_game_servers(app: AppHandle, game_id: String) -> Result<SavedServers, String> {
    let game = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|_| "游戏库不可用")?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == game_id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" {
            return Err("请先完成游戏安装".into());
        }
        game.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        Ok(SavedServers {
            direct_connect: cobble_core::supports_direct_connect(&game.version),
            items: entries(&game)?,
        })
    })
    .await
    .map_err(|_| "服务器列表读取任务意外退出")?
}
pub(in crate::games) fn resolve(game: &Game, server_id: &str) -> Result<String, String> {
    let server = entries(game)?
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or("服务器列表已变化，请刷新后重试")?;
    cobble_core::direct_connect_arguments(&game.version, &server.address)?;
    Ok(server.address)
}
#[tauri::command]
pub fn start_game_server(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
    server_id: String,
) -> Result<Game, String> {
    runtime::start_game_target(app, library, id, Some(server_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_entry_ids_reject_changed_lists_and_never_modify_the_file() {
        let root = std::env::temp_dir().join(format!(
            "cobble-saved-servers-{}-{}",
            std::process::id(),
            next_pack_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let game: Game = serde_json::from_value(serde_json::json!({
            "id":"abc", "name":"Fixture", "version":"1.21.1", "directory":root,
            "status":"installed", "error":null
        }))
        .unwrap();
        let bytes = b"\x0a\x00\x00\x09\x00\x07servers\x0a\x00\x00\x00\x01\x08\x00\x04name\x00\x04Home\x08\x00\x02ip\x00\x09secret:42\x00\x00";
        let path = root.join("servers.dat");
        fs::write(&path, bytes).unwrap();
        let saved = entries(&game).unwrap();
        assert_eq!(saved.len(), 1);
        assert!(saved[0].can_connect);
        assert_eq!(resolve(&game, &saved[0].id).unwrap(), "secret:42");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let changed = bytes
            .windows(9)
            .position(|window| window == b"secret:42")
            .unwrap();
        let mut replacement = bytes.to_vec();
        replacement[changed..changed + 9].copy_from_slice(b"public:43");
        fs::write(&path, replacement).unwrap();
        assert!(resolve(&game, &saved[0].id).is_err());
        assert!(resolve(&game, "arbitrary-address").is_err());
        fs::write(&path, b"invalid NBT").unwrap();
        assert!(entries(&game).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

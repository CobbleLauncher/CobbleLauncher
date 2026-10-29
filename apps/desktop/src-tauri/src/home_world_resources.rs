// SPDX-License-Identifier: MIT
//! Read-only homepage metadata. No management lease, recovery, or renderer paths.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{io::Read, path::Path};

const MAX_WORLD_ICON_BYTES: u64 = 1024 * 1024;

fn game_directory(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "无法读取游戏列表")?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    Ok(game.directory.clone())
}

fn read_thumbnail(world: &Path) -> Result<Option<String>, String> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let icon = world.join("icon.png");
    let metadata = match fs::symlink_metadata(&icon) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        value => value.map_err(|_| "无法读取世界图标")?,
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_WORLD_ICON_BYTES
    {
        return Err("世界图标无效".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("世界图标无效".into());
        }
    }
    let parent = cap_std::fs::Dir::open_ambient_dir(world, cap_std::ambient_authority())
        .map_err(|_| "无法读取世界图标")?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with("icon.png", &options)
        .map_err(|_| "无法读取世界图标")?;
    let mut bytes = Vec::new();
    file.take(MAX_WORLD_ICON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取世界图标")?;
    if bytes.len() as u64 > MAX_WORLD_ICON_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("世界图标无效".into());
    }
    let png = crate::games::game_appearance::normalize_size(&bytes, 128, 128)?;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png)
    )))
}

#[derive(Serialize)]
pub struct HomeWorldSummary {
    thumbnail: Option<String>,
    last_backup: Option<String>,
    backup_unavailable: bool,
}

fn latest_backup(id: &str, folder_name: &str) -> Result<Option<String>, String> {
    let Some(repository) = repository(id, false)? else {
        return Ok(None);
    };
    let snapshots = repository
        .list(&cobble_core::world_snapshot_identity(folder_name))
        .map_err(|_| "无法读取世界备份")?;
    Ok(snapshots
        .into_iter()
        .next()
        .map(|snapshot| snapshot.created_at))
}

#[tauri::command]
pub async fn get_home_world_summary(
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<HomeWorldSummary, String> {
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let world = cobble_content::world_directory(&directory, &folder_name)
            .map_err(|_| "世界已不存在")?;
        let backup = latest_backup(&id, &folder_name);
        Ok(HomeWorldSummary {
            thumbnail: read_thumbnail(&world).ok().flatten(),
            backup_unavailable: backup.is_err(),
            last_backup: backup.ok().flatten(),
        })
    })
    .await
    .map_err(|_| "世界备份读取任务意外退出")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_is_optional_bounded_and_world_scoped() {
        let root = std::env::temp_dir().join(format!(
            "cobble-home-icon-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let world = root.join("saves/World");
        fs::create_dir_all(&world).unwrap();
        fs::write(world.join("level.dat"), b"fixture").unwrap();
        assert!(read_thumbnail(&world).unwrap().is_none());
        assert!(cobble_content::world_directory(&root, "../World").is_err());
        fs::write(world.join("icon.png"), b"invalid image").unwrap();
        assert!(read_thumbnail(&world).is_err());
        fs::write(
            world.join("icon.png"),
            vec![0; MAX_WORLD_ICON_BYTES as usize + 1],
        )
        .unwrap();
        assert!(read_thumbnail(&world).is_err());
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(64, 64)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        fs::write(world.join("icon.png"), png.into_inner()).unwrap();
        assert!(
            read_thumbnail(&world)
                .unwrap()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn absent_snapshot_repository_is_not_created_for_home() {
        let id = format!(
            "home-read-only-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = repository_root(&id).unwrap();
        assert!(!root.exists());
        assert!(latest_backup(&id, "World").unwrap().is_none());
        assert!(!root.exists());
    }
}

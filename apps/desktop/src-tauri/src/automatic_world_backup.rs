// SPDX-License-Identifier: MIT
//! Opt-in automatic backups; pending identity survives launcher interruption.
use super::*;
use serde::{Deserialize, Serialize};
static STATUS_WRITE: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Preference {
    #[default]
    Undecided,
    Enabled,
    Disabled,
}
#[derive(Default, Deserialize, Serialize)]
pub struct Status {
    preference: Preference,
    last_backup: Option<String>,
    last_error: Option<String>,
    #[serde(default)]
    world_backups: std::collections::BTreeMap<String, String>,
}
fn path(id: &str, pending: bool) -> Result<PathBuf, String> {
    let root = repository_root(id)?;
    Ok(root.parent().ok_or("备份目录无效")?.join(format!(
        "{id}.{}.json",
        if pending { "pending" } else { "automatic" }
    )))
}
fn read<T: serde::de::DeserializeOwned + Default>(path: &std::path::Path) -> Result<T, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "自动备份记录损坏".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(_) => Err("无法读取自动备份记录".into()),
    }
}
fn write<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("备份目录无效")?).map_err(|_| "无法创建备份目录")?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(value).map_err(|_| "无法保存自动备份记录")?;
    fs::write(&temporary, bytes).map_err(|_| "无法保存自动备份记录")?;
    fs::rename(temporary, path).map_err(|_| "无法保存自动备份记录".into())
}
#[tauri::command]
pub fn get_automatic_world_backup(app: AppHandle, game_id: String) -> Result<Status, String> {
    game_directory(&app, &game_id)?;
    read(&path(&game_id, false)?)
}
#[tauri::command]
pub fn set_automatic_world_backup(
    app: AppHandle,
    game_id: String,
    preference: Preference,
) -> Result<Status, String> {
    game_directory(&app, &game_id)?;
    let _write = STATUS_WRITE.lock().map_err(|_| "自动备份设置不可用")?;
    let file = path(&game_id, false)?;
    let mut status: Status = read(&file)?;
    status.preference = preference;
    write(&file, &status)?;
    if preference == Preference::Enabled {
        let game = {
            let state = app.state::<GameLibrary>();
            let library = state.0.lock().map_err(|_| "游戏库不可用")?;
            library
                .games
                .iter()
                .find(|game| game.id == game_id)
                .cloned()
                .ok_or("找不到这个游戏")?
        };
        if let Some(session) = sessions::recover(&game)? {
            record_pending(&session)?;
        }
    }
    Ok(status)
}
pub(in crate::games) fn record_pending(session: &sessions::Session) -> Result<(), String> {
    let status: Status = read(&path(&session.game_id, false)?)?;
    if status.preference == Preference::Enabled {
        write(&path(&session.game_id, true)?, &Some(session))?;
    }
    Ok(())
}
pub(in crate::games) fn record_error(game_id: &str, error: &str) {
    let Ok(_write) = STATUS_WRITE.lock() else {
        return;
    };
    let Ok(file) = path(game_id, false) else {
        return;
    };
    let Ok(mut status) = read::<Status>(&file) else {
        return;
    };
    status.last_error = Some(error.to_owned());
    if let Err(error) = write(&file, &status) {
        tracing::warn!(event = "automatic_backup_error_save_failed", game_id, %error);
    }
}
pub(in crate::games) async fn after_exit(app: &AppHandle, session: &sessions::Session) {
    if sessions::alive(session) {
        return;
    }
    let pending =
        path(&session.game_id, true).and_then(|file| read::<Option<sessions::Session>>(&file));
    if !matches!(pending, Ok(Some(_))) {
        return;
    }
    let result = tasks::run(
        app.clone(),
        tasks::TaskRequest::WorldAutomaticBackup {
            game_id: session.game_id.clone(),
            session: session.clone(),
        },
        "自动备份世界",
    )
    .await;
    if let Err(error) = result {
        let Ok(_write) = STATUS_WRITE.lock() else {
            return;
        };
        if let Ok(file) = path(&session.game_id, false)
            && let Ok(mut status) = read::<Status>(&file)
        {
            status.last_error = Some(error);
            let _ = write(&file, &status);
        }
    }
}
pub(crate) fn recover_pending(app: &AppHandle) {
    let games = {
        let state = app.state::<GameLibrary>();
        let Ok(library) = state.0.lock() else {
            return;
        };
        library.games.clone()
    };
    for game in games {
        let pending =
            path(&game.id, true).and_then(|file| read::<Option<sessions::Session>>(&file));
        if let Ok(Some(session)) = pending
            && session.directory == game.directory
            && session.game_id == game.id
            && !sessions::alive(&session)
        {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                after_exit(&app, &session).await;
            });
        }
    }
}
pub(in crate::games) async fn perform(
    app: &AppHandle,
    game_id: &str,
    session: &sessions::Session,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let pending_file = path(game_id, true)?;
    let pending: Option<sessions::Session> = read(&pending_file)?;
    let pending = pending.ok_or("自动备份会话已结束")?;
    if pending.game_id != game_id
        || pending.pid != session.pid
        || pending.started != session.started
        || pending.natives != session.natives
    {
        return Err("自动备份会话已变化".into());
    }
    let (game, operation) = management::begin_after_exit(app, &pending)?;
    let status_file = path(game_id, false)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let mut status: Status = read(&status_file)?;
        if status.preference != Preference::Enabled {
            fs::remove_file(pending_file).map_err(|_| "无法清除待处理备份")?;
            return Ok(());
        }
        control.check_cancelled().map_err(|e| e.to_string())?;
        let worlds =
            cobble_content::list_local_worlds(&game.directory).map_err(|e| e.to_string())?;
        if !worlds.is_empty() {
            let mut repository = repository(&game.id, true)?.ok_or("无法创建快照仓库")?;
            let mut changed = Vec::new();
            for world in worlds {
                let snapshot = cobble_core::create_automatic_world_snapshot_controlled(
                    &game.directory,
                    &world.folder_name,
                    &mut repository,
                    &control,
                )
                .map_err(|e| e.to_string())?;
                if let Some(snapshot) = snapshot {
                    status
                        .world_backups
                        .insert(world.folder_name.clone(), snapshot.created_at.clone());
                    status.last_backup = Some(snapshot.created_at);
                    changed.push(world.folder_name);
                }
            }
            // All worlds succeeded. Cancellation is accepted before retention starts;
            // once this commit begins it finishes without cancellation-induced deletion.
            control.check_cancelled().map_err(|e| e.to_string())?;
            for folder in changed {
                repository
                    .retain_automatic_controlled(
                        &cobble_core::world_snapshot_identity(&folder),
                        &Default::default(),
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        let _write = STATUS_WRITE.lock().map_err(|_| "自动备份设置不可用")?;
        let mut latest: Status = read(&status_file)?;
        latest.last_backup = status.last_backup;
        latest.world_backups = status.world_backups;
        latest.last_error = None;
        write(&status_file, &latest)?;
        fs::remove_file(pending_file).map_err(|_| "无法清除待处理备份")?;
        Ok(())
    })
    .await
    .map_err(|_| "自动备份任务意外退出")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_is_explicit_and_survives_state_replacement() {
        let root = std::env::temp_dir().join(format!(
            "cobble-auto-policy-{}-{}",
            std::process::id(),
            next_pack_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let file = root.join("policy.json");
        let initial: Status = read(&file).unwrap();
        assert!(initial.preference == Preference::Undecided);
        for preference in [Preference::Disabled, Preference::Enabled] {
            write(
                &file,
                &Status {
                    preference,
                    last_backup: Some("2026-10-23T00:00:00Z".into()),
                    last_error: None,
                    world_backups: Default::default(),
                },
            )
            .unwrap();
            let loaded: Status = read(&file).unwrap();
            assert!(loaded.preference == preference);
            assert!(loaded.last_backup.is_some());
        }
        fs::write(&file, b"invalid").unwrap();
        assert!(read::<Status>(&file).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

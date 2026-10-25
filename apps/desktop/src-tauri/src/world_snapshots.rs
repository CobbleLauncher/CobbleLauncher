// SPDX-License-Identifier: MIT
//! Native-only repository credentials and stopped-instance snapshot commands.
use super::*;
#[path = "world_snapshot_insights.rs"]
pub mod insights;
use cobble_core::{WorldSnapshot, WorldSnapshotRepository};
#[path = "automatic_world_backup.rs"]
pub mod automatic;
#[path = "world_snapshot_recovery.rs"]
pub mod recovery;
#[path = "world_snapshot_retention.rs"]
pub mod retention;

struct SnapshotActivity {
    operation: &'static str,
    game_id: String,
    started: std::time::Instant,
    outcome: &'static str,
}
impl SnapshotActivity {
    fn new(operation: &'static str, game_id: &str) -> Self {
        let safe_id = if !game_id.is_empty()
            && game_id.len() <= 128
            && game_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            game_id
        } else {
            "invalid"
        };
        Self {
            operation,
            game_id: safe_id.into(),
            started: std::time::Instant::now(),
            outcome: "failed",
        }
    }
    fn complete<T>(mut self, result: Result<T, String>) -> Result<T, String> {
        if result.is_ok() {
            self.outcome = "completed";
        }
        result
    }
    fn change(mut self, result: Result<bool, String>) -> Result<bool, String> {
        if let Ok(changed) = &result {
            self.outcome = if *changed { "completed" } else { "no_change" };
        }
        result
    }
}
impl Drop for SnapshotActivity {
    fn drop(&mut self) {
        tracing::info!(
            event = "world_snapshot_operation_finished",
            operation = self.operation,
            game_id = self.game_id.as_str(),
            outcome = self.outcome,
            duration_ms = self.started.elapsed().as_millis() as u64
        );
    }
}

fn repository_root(id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("游戏 ID 无效".into());
    }
    Ok(dirs::data_local_dir()
        .ok_or("无法找到本地数据目录")?
        .join("CobbleLauncher")
        .join("world-snapshots")
        .join(id))
}

fn repository(id: &str, create: bool) -> Result<Option<WorldSnapshotRepository>, String> {
    let root = repository_root(id)?;
    if !root.exists() && !create {
        return Ok(None);
    }
    if root.exists() {
        let metadata = fs::symlink_metadata(&root).map_err(|_| "无法读取快照仓库")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("快照仓库目录无效".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("快照仓库目录无效".into());
            }
        }
    }
    let entry =
        keyring::Entry::new("CobbleLauncher.WorldSnapshots", id).map_err(|_| "系统凭据库不可用")?;
    let password = match entry.get_password() {
        Ok(password) => password,
        Err(keyring::Error::NoEntry) if !root.exists() && create => {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).map_err(|_| "无法生成快照仓库凭据")?;
            let password: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            entry
                .set_password(&password)
                .map_err(|_| "无法保存快照仓库凭据")?;
            password
        }
        Err(_) => return Err("无法读取快照仓库凭据，请检查系统凭据库".into()),
    };
    if root.exists() {
        WorldSnapshotRepository::open(&root, &password)
    } else {
        fs::create_dir_all(root.parent().ok_or("快照仓库目录无效")?)
            .map_err(|_| "无法创建快照仓库目录")?;
        WorldSnapshotRepository::create(&root, &password)
    }
    .map(Some)
    .map_err(|_| "无法打开快照仓库".into())
}

#[tauri::command]
pub async fn list_world_snapshots(
    app: AppHandle,
    game_id: String,
    folder_name: String,
) -> Result<Vec<WorldSnapshot>, String> {
    let activity = SnapshotActivity::new("list", &game_id);
    let (game, operation) = management::begin(&app, &game_id, true)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::world_directory(&game.directory, &folder_name)
            .map_err(|e| e.to_string())?;
        let Some(repository) = repository(&game.id, false)? else {
            return Ok(vec![]);
        };
        repository
            .list(&cobble_core::world_snapshot_identity(&folder_name))
            .map_err(|_| "无法读取世界快照".into())
    })
    .await
    .map_err(|_| "快照读取任务意外退出")?;
    activity.complete(result)
}

#[tauri::command]
pub async fn create_world_snapshot(
    app: AppHandle,
    game_id: String,
    folder_name: String,
    name: Option<String>,
) -> Result<WorldSnapshot, String> {
    let activity = SnapshotActivity::new("create", &game_id);
    let result = match tasks::run(
        app,
        tasks::TaskRequest::WorldSnapshot {
            game_id,
            folder_name,
            name,
        },
        "创建世界快照",
    )
    .await?
    {
        tasks::TaskResult::Snapshot(snapshot) => Ok(snapshot),
        _ => Err("世界快照结果不可用".into()),
    };
    activity.complete(result)
}

pub(super) async fn perform_create(
    app: &AppHandle,
    game_id: &str,
    folder_name: &str,
    name: Option<String>,
    control: cobble_core::OperationControl,
) -> Result<WorldSnapshot, String> {
    let (game, operation) = management::begin(app, game_id, true)?;
    update_game(app, game_id, |_| {}, false);
    let folder_name = folder_name.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        control.check_cancelled().map_err(|e| e.to_string())?;
        let name = name.unwrap_or_default();
        let name = name.trim();
        if name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("快照名称最多 80 个字符".into());
        }
        // Validate before opening or initializing a repository.
        cobble_content::world_directory(&game.directory, &folder_name)
            .map_err(|e| e.to_string())?;
        let mut repository = repository(&game.id, true)?.ok_or("无法创建快照仓库")?;
        cobble_core::create_world_snapshot_controlled(
            &game.directory,
            &folder_name,
            &mut repository,
            name,
            &control,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "快照创建任务意外退出")?
}

#[tauri::command]
pub async fn restore_world_snapshot(
    app: AppHandle,
    game_id: String,
    folder_name: String,
    snapshot_id: String,
    replace_existing: bool,
) -> Result<worlds::WorldRestoreResult, String> {
    let activity = SnapshotActivity::new("restore", &game_id);
    if !replace_existing {
        return Err("请确认替换当前世界；替换前的世界会保留为恢复备份".into());
    }
    let result = match tasks::run(
        app,
        tasks::TaskRequest::WorldSnapshotRestore {
            game_id,
            folder_name,
            snapshot_id,
            operation_id: next_pack_id()?,
        },
        "恢复世界快照",
    )
    .await?
    {
        tasks::TaskResult::Worlds(result) => Ok(result),
        _ => Err("世界快照恢复结果不可用".into()),
    };
    activity.complete(result)
}

pub(super) async fn perform_restore(
    app: &AppHandle,
    game_id: &str,
    folder_name: &str,
    snapshot_id: &str,
    operation_id: &str,
    control: cobble_core::OperationControl,
) -> Result<worlds::WorldRestoreResult, String> {
    control.check_cancelled().map_err(|e| e.to_string())?;
    let (game, _operation) = management::begin(app, game_id, true)?;
    update_game(app, game_id, |_| {}, false);
    let id = game.id.clone();
    let repository = tauri::async_runtime::spawn_blocking(move || {
        repository(&id, false)?.ok_or_else(|| "没有世界快照".to_owned())
    })
    .await
    .map_err(|_| "快照读取任务意外退出")??;
    let outcome = cobble_core::restore_world_snapshot(
        &game.directory,
        folder_name,
        snapshot_id,
        repository,
        true,
        operation_id,
        control,
    )
    .await
    .map_err(|e| e.to_string())?;
    worlds::restore_result(game.directory, outcome).await
}

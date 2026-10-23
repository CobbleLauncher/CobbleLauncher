// SPDX-License-Identifier: MIT
//! Approval binds one exact repository revision; no renderer-supplied deletion IDs.
use super::*;
use cobble_core::{RetentionPlan, RetentionPolicy, RetentionPreview, RetentionResult};
use std::sync::OnceLock;

struct Pending {
    id: String,
    game_id: String,
    folder_name: String,
    plan: RetentionPlan,
    created: Instant,
}
static PREVIEWS: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
fn previews() -> &'static Mutex<HashMap<String, Pending>> {
    PREVIEWS.get_or_init(Default::default)
}
#[derive(Serialize)]
pub struct CleanupPreview {
    #[serde(rename = "previewId")]
    preview_id: String,
    #[serde(flatten)]
    preview: RetentionPreview,
}
#[tauri::command]
pub async fn preview_world_snapshot_cleanup(
    window: WebviewWindow,
    app: AppHandle,
    game_id: String,
    folder_name: String,
    policy: RetentionPolicy,
) -> Result<Option<CleanupPreview>, String> {
    if !(1..=1000).contains(&policy.keep_last) {
        return Err("保留数量需为 1–1000".into());
    }
    previews()
        .lock()
        .map_err(|_| "清理预览不可用")?
        .remove(window.label());
    let activity = SnapshotActivity::new("cleanup_preview", &game_id);
    let (game, operation) = management::begin(&app, &game_id, true)?;
    let pending = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::world_directory(&game.directory, &folder_name)
            .map_err(|_| "世界已不存在")?;
        let Some(repository) = repository(&game.id, false)? else {
            return Ok(None);
        };
        let plan = repository
            .retention_preview(&cobble_core::world_snapshot_identity(&folder_name), &policy)
            .map_err(|_| "无法预览快照清理")?;
        Ok::<_, String>(Some(Pending {
            id: next_pack_id()?,
            game_id,
            folder_name,
            plan,
            created: Instant::now(),
        }))
    })
    .await
    .map_err(|_| "快照清理预览意外退出")??;
    let result = if let Some(pending) = pending {
        let result = CleanupPreview {
            preview_id: pending.id.clone(),
            preview: pending.plan.preview.clone(),
        };
        let mut previews = previews().lock().map_err(|_| "清理预览不可用")?;
        previews.retain(|_, pending| pending.created.elapsed() < Duration::from_secs(900));
        if !previews.contains_key(window.label()) && previews.len() >= 4 {
            return Err("打开的清理预览过多，请稍后重试".into());
        }
        previews.insert(window.label().into(), pending);
        Some(result)
    } else {
        None
    };
    activity.complete(Ok(result))
}
#[tauri::command]
pub async fn apply_world_snapshot_cleanup(
    window: WebviewWindow,
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<RetentionResult, String> {
    if !confirmed {
        return Err("请先确认将永久移除预览中的版本".into());
    }
    let pending = {
        let mut previews = previews().lock().map_err(|_| "清理预览不可用")?;
        previews
            .get(window.label())
            .filter(|pending| {
                pending.id == preview_id && pending.created.elapsed() < Duration::from_secs(900)
            })
            .ok_or("清理预览已过期，请重新预览")?;
        previews.remove(window.label()).ok_or("清理预览已过期")?
    };
    let activity = SnapshotActivity::new("cleanup", &pending.game_id);
    let (game, operation) = management::begin(&app, &pending.game_id, true)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::world_directory(&game.directory, &pending.folder_name)
            .map_err(|_| "世界已不存在")?;
        let mut repository = repository(&game.id, false)?.ok_or("没有快照仓库")?;
        repository
            .apply_retention(pending.plan)
            .map_err(|_| "清理失败或版本已变化，请刷新后重新预览".to_owned())
    })
    .await
    .map_err(|_| "快照清理意外退出")?;
    activity.complete(result)
}

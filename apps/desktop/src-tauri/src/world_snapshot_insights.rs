// SPDX-License-Identifier: MIT
//! Read-only insights share the existing per-game stopped-operation lease.
use super::*;
#[tauri::command]
pub async fn get_world_snapshot_storage(
    app: AppHandle,
    game_id: String,
) -> Result<Option<cobble_core::RepositoryInsights>, String> {
    let (game, operation) = management::begin(&app, &game_id, true)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        repository(&game.id, false)?
            .map(|repo| {
                repo.storage_insights()
                    .map_err(|_| "无法读取备份存储统计".to_owned())
            })
            .transpose()
    })
    .await
    .map_err(|_| "备份统计任务意外退出".to_owned())?
}
#[tauri::command]
pub async fn compare_world_snapshots(
    app: AppHandle,
    game_id: String,
    folder_name: String,
    before_id: String,
    after_id: String,
    offset: usize,
    limit: usize,
) -> Result<cobble_core::SnapshotComparison, String> {
    compare_world_snapshots_filtered(
        app,
        FilteredComparison {
            game_id,
            folder_name,
            before_id,
            after_id,
            offset,
            limit,
            query: String::new(),
        },
    )
    .await
}
pub(crate) struct FilteredComparison {
    pub game_id: String,
    pub folder_name: String,
    pub before_id: String,
    pub after_id: String,
    pub offset: usize,
    pub limit: usize,
    pub query: String,
}
pub(crate) async fn compare_world_snapshots_filtered(
    app: AppHandle,
    request: FilteredComparison,
) -> Result<cobble_core::SnapshotComparison, String> {
    let FilteredComparison {
        game_id,
        folder_name,
        before_id,
        after_id,
        offset,
        limit,
        query,
    } = request;
    if limit == 0
        || limit > 100
        || offset > 200_000
        || query.chars().count() > 120
        || query.chars().any(char::is_control)
    {
        return Err("无效的快照比较范围".into());
    }
    let (game, operation) = management::begin(&app, &game_id, true)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::world_directory(&game.directory, &folder_name)
            .map_err(|_| "世界已不存在")?;
        let repo = repository(&game.id, false)?.ok_or("备份仓库不存在")?;
        repo.compare_snapshots_filtered(
            &cobble_core::world_snapshot_identity(&folder_name),
            &before_id,
            &after_id,
            cobble_core::SnapshotComparePage {
                offset,
                limit,
                query: &query,
            },
            &cobble_core::BackupControl::default(),
        )
        .map_err(|_| "无法比较世界快照".into())
    })
    .await
    .map_err(|_| "快照比较任务意外退出".to_owned())?
}

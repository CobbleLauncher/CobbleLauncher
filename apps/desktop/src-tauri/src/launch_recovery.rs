// SPDX-License-Identifier: MIT
//! Explicit recovery of launcher-managed Vanilla artifacts; never re-import packs.
use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    Login,
    Java,
    ManagedArtifacts,
    Unknown,
}
static FAILURES: OnceLock<Mutex<HashMap<String, FailureKind>>> = OnceLock::new();
static GENERATIONS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
pub fn begin_generation(id: &str) -> u64 {
    let mut generations = GENERATIONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let generation = generations.entry(id.into()).or_default();
    *generation += 1;
    *generation
}
pub fn same_generation(id: &str, generation: u64) -> bool {
    GENERATIONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        == Some(&generation)
}
pub fn record(id: &str, kind: FailureKind) {
    if let Ok(mut failures) = FAILURES.get_or_init(Default::default).lock() {
        failures.insert(id.into(), kind);
    }
}
pub fn classify(error: &(dyn std::error::Error + 'static)) -> FailureKind {
    if error
        .downcast_ref::<cobble_minecraft::JavaError>()
        .is_some()
    {
        return FailureKind::Java;
    }
    let message = error.to_string();
    if message.starts_with("Minecraft requires Java ")
        || message.starts_with("Java architecture ")
        || message == "Choose a Java executable"
    {
        return FailureKind::Java;
    }
    // These are exact Core preparation errors, never arbitrary log heuristics.
    if error.to_string().starts_with("missing installed file: ")
        || error.to_string() == "missing client artifact"
    {
        FailureKind::ManagedArtifacts
    } else {
        FailureKind::Unknown
    }
}
#[derive(Serialize)]
pub struct LaunchRecovery {
    kind: FailureKind,
    current_log: bool,
    can_repair: bool,
}
pub(crate) fn validate_current_failure(app: &AppHandle, id: &str) -> Result<(), String> {
    with_current_failure(app, id, |_| Ok(()))
}
pub(crate) fn with_current_failure<T>(
    app: &AppHandle,
    id: &str,
    read: impl FnOnce(&std::path::Path) -> Result<T, String>,
) -> Result<T, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.launch_error.is_none() || game.log_path.is_none() || game.launch_state != "idle" {
        return Err("本次启动没有可分析的失败日志".into());
    }
    read(&game.directory)
}
#[tauri::command]
pub fn get_launch_recovery(
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<LaunchRecovery, String> {
    let library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.launch_error.is_none() {
        return Err("没有启动失败记录".into());
    }
    let kind = FAILURES
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?
        .get(&id)
        .copied()
        .unwrap_or(FailureKind::Unknown);
    Ok(LaunchRecovery {
        kind,
        current_log: game.log_path.is_some(),
        can_repair: kind == FailureKind::ManagedArtifacts
            && game.launch_state == "idle"
            && !game.content_installing,
    })
}
#[tauri::command]
pub async fn open_current_launch_log(app: AppHandle, id: String) -> Result<(), String> {
    let path = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|g| g.id == id)
            .ok_or("找不到这个游戏")?;
        if game.launch_error.is_none() || game.log_path.is_none() {
            return Err("本次启动没有运行日志".into());
        }
        let path = game.directory.join(".cobble-logs/launcher.log");
        let root = game.directory.canonicalize().map_err(|e| e.to_string())?;
        if !path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root)
        {
            return Err("游戏日志超出游戏目录".into());
        }
        path
    };
    tauri::async_runtime::spawn_blocking(move || open::that(path).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn repair_game_artifacts(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<tasks::TaskSnapshot, String> {
    {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|g| g.id == id)
            .ok_or("找不到这个游戏")?;
        let kind = FAILURES
            .get_or_init(Default::default)
            .lock()
            .map_err(|e| e.to_string())?
            .get(&id)
            .copied();
        if game.status != "installed"
            || game.launch_state != "idle"
            || game.content_installing
            || kind != Some(FailureKind::ManagedArtifacts)
        {
            return Err("这个游戏当前无法修复".into());
        }
    }
    let task_id = tasks::register(
        &app,
        tasks::TaskRequest::GameRepair { game_id: id },
        "修复游戏文件",
    )?;
    let snapshot = tasks::list_tasks(app.clone())?
        .into_iter()
        .find(|task| task.id == task_id)
        .ok_or("找不到这个任务")?;
    tauri::async_runtime::spawn(async move {
        let _ = tasks::execute(app, task_id).await;
    });
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_exit_generation_cannot_match_a_new_launch() {
        let first = begin_generation("generation-regression-test");
        assert!(same_generation("generation-regression-test", first));
        let second = begin_generation("generation-regression-test");
        assert!(!same_generation("generation-regression-test", first));
        assert!(same_generation("generation-regression-test", second));
    }
    #[test]
    fn only_core_missing_artifacts_allow_repair() {
        let missing: Box<dyn std::error::Error> = "missing installed file: libraries/a.jar".into();
        assert_eq!(classify(missing.as_ref()), FailureKind::ManagedArtifacts);
        let unrelated: Box<dyn std::error::Error> = "missing Mod dependency".into();
        assert_eq!(classify(unrelated.as_ref()), FailureKind::Unknown);
        assert_eq!(
            classify(&cobble_minecraft::JavaError::NotFound { required_major: 21 }),
            FailureKind::Java
        );
    }
}

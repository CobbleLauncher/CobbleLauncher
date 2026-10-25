// SPDX-License-Identifier: MIT
//! Per-game runtime preferences, Java selection and process launch.
use super::{Game, GameLibrary, idle, save, sessions, update_game};
use serde::Serialize;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State, WebviewWindow};

#[derive(Serialize)]
pub struct LaunchReadiness {
    java: crate::java::JavaRuntimeInfo,
    required_java: u32,
    main_class: String,
    classpath_entries: usize,
}

pub(crate) fn recovery_overrides(
    app: &AppHandle,
    id: &str,
) -> Result<cobble_minecraft::LaunchOverrides, String> {
    app.state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "RECOVERY_LIBRARY_UNAVAILABLE".to_owned())?
        .games
        .iter()
        .find(|game| game.id == id)
        .map(|game| game.runtime_overrides.clone())
        .ok_or_else(|| "RECOVERY_GAME_NOT_FOUND".into())
}
pub(crate) fn recovery_has_failure(app: &AppHandle, id: &str) -> Result<bool, String> {
    Ok(app
        .state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "RECOVERY_LIBRARY_UNAVAILABLE")?
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("RECOVERY_GAME_NOT_FOUND")?
        .launch_error
        .is_some())
}

#[tauri::command]
pub async fn prepare_game_launch(
    id: String,
    library: State<'_, GameLibrary>,
) -> Result<LaunchReadiness, String> {
    let (version, directory, java_override) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" {
            return Err("请先完成游戏安装".into());
        }
        (
            game.version.clone(),
            game.directory.clone(),
            game.runtime_overrides.java.clone(),
        )
    };
    let launch = cobble_core::prepare_launch(&version, &directory)
        .await
        .map_err(|e| e.to_string())?;
    let java = cobble_minecraft::resolve_java_override(launch.required_java, &java_override)
        .await
        .map_err(|e| e.to_string())?;
    Ok(LaunchReadiness {
        java: java.into(),
        required_java: launch.required_java,
        main_class: launch.main_class,
        classpath_entries: launch.classpath_entries,
    })
}

#[tauri::command]
pub fn start_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Game, String> {
    start_game_target(app, library, id, None)
}
pub(in crate::games) fn start_game_target(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
    server_id: Option<String>,
) -> Result<Game, String> {
    start_game_inner(app, library, id, server_id, None)
}
pub(super) fn start_game_captured(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
    server_id: Option<String>,
    profile_id: String,
    executable: std::path::PathBuf,
    generation: u64,
) -> Result<Game, String> {
    start_game_inner(
        app,
        library,
        id,
        server_id,
        Some((profile_id, executable, generation)),
    )
}
fn record_immediate_failure(
    game: &mut Game,
    kind: super::launch_recovery::FailureKind,
    message: &str,
) {
    game.log_path = None;
    game.launch_error = Some(message.into());
    super::launch_recovery::record(&game.id, kind);
    let failed_id = game.id.clone();
    let loader = crate::plugins::plugin_recovery_workspace::loader_summary(
        game.version.clone(),
        game.loader_kind.clone(),
        game.loader_version.clone(),
    );
    let overrides = game.runtime_overrides.clone();
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| value.as_millis().try_into().ok())
        .unwrap_or(0);
    tauri::async_runtime::spawn(async move {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let runtime =
                crate::plugins::plugin_recovery_workspace::capture_runtime(&overrides, None).ok();
            crate::plugins::plugin_recovery_workspace::record_failed_attempt(
                &failed_id, at, runtime, loader,
            )
        })
        .await;
    });
}
fn start_game_inner(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
    server_id: Option<String>,
    captured: Option<(String, std::path::PathBuf, u64)>,
) -> Result<Game, String> {
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    if captured.as_ref().is_some_and(|(_, _, generation)| {
        !super::launch_recovery::same_generation(&id, *generation)
    }) {
        return Err("启动请求已过期".into());
    }
    let game = library
        .games
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    if game.launch_state != "idle" {
        return Err("这个游戏已经在启动或运行".into());
    }
    if game.content_installing {
        return Err("Mod 正在安装，请稍候".into());
    }
    let java_choice = match &captured {
        Some((_, executable, _)) => cobble_minecraft::JavaOverride::Custom {
            executable: executable.clone(),
        },
        None => match cobble_core::capture_java_choice(&game.runtime_overrides.java) {
            Ok(choice) => choice,
            Err(error) => {
                let message = error.to_string();
                record_immediate_failure(game, super::launch_recovery::FailureKind::Java, &message);
                return Err(message);
            }
        },
    };
    let server_address = match server_id
        .clone()
        .map(|server_id| super::game_servers::resolve(game, &server_id))
        .transpose()
    {
        Ok(address) => address,
        Err(error) => {
            record_immediate_failure(game, super::launch_recovery::FailureKind::Unknown, &error);
            return Err(error);
        }
    };
    // Capture the effective identity before spawning; later selection changes
    // only affect future launches.
    let profile = match cobble_auth::get_player_profile(
        captured
            .as_ref()
            .map(|(profile, _, _)| profile.as_str())
            .or(game.profile_id.as_deref()),
    ) {
        Ok(profile) => profile,
        Err(error) => {
            let message = error.to_string();
            record_immediate_failure(game, super::launch_recovery::FailureKind::Login, &message);
            return Err(message);
        }
    };
    game.launch_state = "preparing".into();
    let generation = super::launch_recovery::begin_generation(&id);
    game.launch_error = None;
    game.log_path = None;
    super::launch_recovery::record(&id, super::launch_recovery::FailureKind::Unknown);
    let snapshot = game.clone();
    drop(library);
    let mut launch = snapshot.clone();
    let recovery_overrides = snapshot.runtime_overrides.clone();
    let recovery_loader = crate::plugins::plugin_recovery_workspace::loader_summary(
        snapshot.version.clone(),
        snapshot.loader_kind.clone(),
        snapshot.loader_version.clone(),
    );
    let attempt_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| value.as_millis().try_into().ok())
        .unwrap_or(0);
    launch.runtime_overrides.java = java_choice;
    tauri::async_runtime::spawn(async move {
        let mut recovery_runtime = None;
        let result = async {
            let directory = launch.directory.clone();
            tauri::async_runtime::spawn_blocking(move || {
                cobble_core::recover_modrinth_update(&directory).map_err(|e| e.to_string())?;
                cobble_core::recover_world_operations(&directory).map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())??;
            let required = cobble_core::launch_java_requirement(&launch.version, &launch.directory)
                .map_err(|e| e.to_string())?;
            let compatibility = cobble_content::query_compatibility(
                &launch.version,
                launch.loader_kind.as_deref(),
                launch.loader_version.as_deref(),
                std::env::consts::OS,
                std::env::consts::ARCH,
            );
            if compatibility
                .java_major
                .is_some_and(|loader_required| loader_required != required)
            {
                return Err("JAVA_REQUIREMENT_CONFLICT".into());
            }
            let java = match cobble_minecraft::resolve_java_override(
                required,
                &launch.runtime_overrides.java,
            )
            .await
            {
                Ok(runtime) => runtime,
                Err(error)
                    if matches!(
                        launch.runtime_overrides.java,
                        cobble_minecraft::JavaOverride::Automatic
                    ) && matches!(
                        error.downcast_ref::<cobble_minecraft::JavaError>(),
                        Some(cobble_minecraft::JavaError::NotFound { .. })
                    ) =>
                {
                    super::launch_recovery::record(
                        &launch.id,
                        super::launch_recovery::FailureKind::Unknown,
                    );
                    match super::tasks::run(
                        app.clone(),
                        super::tasks::TaskRequest::GameJava {
                            game_id: launch.id.clone(),
                            required,
                            generation,
                            profile_id: profile.id.clone(),
                            server_id: server_id.clone(),
                        },
                        "Java",
                    )
                    .await?
                    {
                        super::tasks::TaskResult::Java(runtime) => runtime,
                        _ => return Err("JAVA_TASK_RESULT_INVALID".into()),
                    }
                }
                Err(error) => {
                    super::launch_recovery::record(
                        &launch.id,
                        super::launch_recovery::FailureKind::Java,
                    );
                    return Err(error.to_string().into());
                }
            };
            recovery_runtime = crate::plugins::plugin_recovery_workspace::capture_runtime(
                &recovery_overrides,
                Some(java.major),
            )
            .ok();
            launch.runtime_overrides.java = cobble_minecraft::JavaOverride::Custom {
                executable: java.executable,
            };
            let identity = cobble_auth::launch_identity(Some(&profile.id))
                .await
                .inspect_err(|_| {
                    super::launch_recovery::record(
                        &launch.id,
                        super::launch_recovery::FailureKind::Login,
                    );
                })?;
            let result = cobble_core::start_game_with_identity_and_server(
                &launch.version,
                &launch.directory,
                &identity,
                &launch.runtime_overrides,
                server_address.as_deref(),
            )
            .await;
            if let Err(error) = &result {
                super::launch_recovery::record(
                    &launch.id,
                    super::launch_recovery::classify(error.as_ref()),
                );
            }
            result
        }
        .await;
        match result {
            Ok(mut running) => {
                tracing::info!(event="game_started", game_id=%launch.id);
                let session = (|| {
                    let session = sessions::capture(
                        &launch,
                        running.child.id().ok_or("游戏进程已退出")?,
                        &running.natives_directory,
                    )?;
                    sessions::persist(&session)?;
                    if let Err(error) = super::world_snapshots::automatic::record_pending(&session)
                    {
                        tracing::warn!(event="automatic_backup_pending_failed", game_id=%launch.id, error=%error);
                        super::world_snapshots::automatic::record_error(&launch.id, &error);
                    }
                    Ok::<_, String>(session)
                })();
                let session = match session {
                    Ok(session) => session,
                    Err(error) => {
                        let _ = running.child.start_kill();
                        let _ = running.child.wait().await;
                        update_game(
                            &app,
                            &launch.id,
                            |game| {
                                game.launch_state = idle();
                                game.launch_error =
                                    Some(format!("无法保存游戏运行记录，已停止本次启动：{error}"));
                            },
                            true,
                        );
                        return;
                    }
                };
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        g.launch_state = "running".into();
                        g.log_path = Some(running.log_path);
                        g.last_launched_at = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .ok()
                            .and_then(|value| value.as_millis().try_into().ok());
                    },
                    true,
                );
                if running.launch_behavior == cobble_minecraft::LaunchBehavior::Minimize
                    && let Some(window) = app.get_webview_window("main")
                {
                    let _ = window.minimize();
                }
                let result = running.child.wait().await;
                tracing::info!(event="game_exited", game_id=%launch.id, success=result.as_ref().is_ok_and(|status|status.success()));
                if result.as_ref().is_ok_and(|status| status.success())
                    && super::launch_recovery::same_generation(&launch.id, generation)
                    && let (Some(summary), Some(started_at_ms), Some(ended_at_ms)) = (
                        recovery_runtime.clone(),
                        session.started.checked_mul(1000),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .ok()
                            .and_then(|value| value.as_millis().try_into().ok()),
                    )
                {
                    let game_id = launch.id.clone();
                    let loader = recovery_loader.clone();
                    let recorded = tauri::async_runtime::spawn_blocking(move || {
                        crate::plugins::plugin_recovery_workspace::record_clean_exit(
                            &game_id,
                            started_at_ms,
                            ended_at_ms,
                            summary,
                            loader,
                        )
                    })
                    .await;
                    if !matches!(recorded, Ok(Ok(()))) {
                        tracing::warn!(event="game_recovery_baseline_unavailable", game_id=%launch.id);
                    }
                }
                if !result.as_ref().is_ok_and(|status| status.success()) {
                    let game_id = launch.id.clone();
                    let loader = recovery_loader.clone();
                    let runtime = recovery_runtime.clone().or_else(|| {
                        crate::plugins::plugin_recovery_workspace::capture_runtime(
                            &recovery_overrides,
                            None,
                        )
                        .ok()
                    });
                    let recorded = tauri::async_runtime::spawn_blocking(move || {
                        crate::plugins::plugin_recovery_workspace::record_failed_attempt(
                            &game_id, attempt_at, runtime, loader,
                        )
                    })
                    .await;
                    if !matches!(recorded, Ok(Ok(()))) {
                        tracing::warn!(event="game_recovery_attempt_unavailable", game_id=%launch.id);
                    }
                }
                let exit_error = match &result {
                    Ok(status) if status.success() => None,
                    Ok(status) => Some(format!("游戏异常退出（{status}），请查看游戏日志")),
                    Err(error) => Some(error.to_string()),
                };
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        if super::launch_recovery::same_generation(&launch.id, generation) {
                            g.launch_error = exit_error;
                        }
                    },
                    true,
                );
                if result.is_ok() {
                    super::world_snapshots::automatic::after_exit(&app, &session).await;
                }
                let _ = sessions::clear(&session);
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        if super::launch_recovery::same_generation(&launch.id, generation) {
                            g.launch_state = idle();
                        }
                    },
                    true,
                );
            }
            Err(error) => {
                let game_id = launch.id.clone();
                let loader = recovery_loader;
                let runtime = recovery_runtime.or_else(|| {
                    crate::plugins::plugin_recovery_workspace::capture_runtime(
                        &recovery_overrides,
                        None,
                    )
                    .ok()
                });
                let recorded = tauri::async_runtime::spawn_blocking(move || {
                    crate::plugins::plugin_recovery_workspace::record_failed_attempt(
                        &game_id, attempt_at, runtime, loader,
                    )
                })
                .await;
                if !matches!(recorded, Ok(Ok(()))) {
                    tracing::warn!(event="game_recovery_attempt_unavailable", game_id=%launch.id);
                }
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        g.launch_state = idle();
                        g.launch_error = Some(error.to_string());
                    },
                    true,
                )
            }
        }
    });
    Ok(snapshot)
}

fn change_runtime(
    library: &GameLibrary,
    id: &str,
    change: impl FnOnce(&mut cobble_minecraft::LaunchOverrides),
) -> Result<Game, String> {
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter_mut()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    let previous = game.runtime_overrides.clone();
    change(&mut game.runtime_overrides);
    let snapshot = game.clone();
    if let Err(error) = save(&library.games) {
        library
            .games
            .iter_mut()
            .find(|game| game.id == id)
            .unwrap()
            .runtime_overrides = previous;
        return Err(error);
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn set_game_memory(
    library: State<'_, GameLibrary>,
    id: String,
    memory_mb: Option<u32>,
) -> Result<Game, String> {
    if let Some(memory) = memory_mb {
        cobble_minecraft::validate_memory_mb(memory).map_err(|e| e.to_string())?;
    }
    change_runtime(&library, &id, |settings| settings.memory_mb = memory_mb)
}

#[tauri::command]
pub fn set_game_advanced_runtime(
    library: State<'_, GameLibrary>,
    id: String,
    window_size: Option<cobble_minecraft::WindowSize>,
    jvm_args: Option<Vec<String>>,
    launch_behavior: Option<cobble_minecraft::LaunchBehavior>,
) -> Result<Game, String> {
    cobble_minecraft::validate_advanced_runtime(
        &window_size.clone().unwrap_or_default(),
        jvm_args.as_deref().unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    change_runtime(&library, &id, |settings| {
        settings.window_size = window_size;
        settings.jvm_args = jvm_args;
        settings.launch_behavior = launch_behavior;
    })
}

#[tauri::command]
pub fn set_game_java_mode(
    library: State<'_, GameLibrary>,
    id: String,
    mode: String,
) -> Result<Game, String> {
    let java = match mode.as_str() {
        "inherit" => cobble_minecraft::JavaOverride::Inherit,
        "automatic" => cobble_minecraft::JavaOverride::Automatic,
        _ => return Err("请选择自动选择或跟随默认 Java".into()),
    };
    change_runtime(&library, &id, |settings| settings.java = java)
}

#[tauri::command]
pub async fn pick_game_java_executable(
    window: WebviewWindow,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Option<Game>, String> {
    let (version, directory) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        (game.version.clone(), game.directory.clone())
    };
    let bytes = fs::read(directory.join(format!("versions/{version}/{version}.json")))
        .map_err(|e| e.to_string())?;
    let required = cobble_minecraft::VersionMetadata::parse_installed(&bytes, &version)
        .map_err(|e| e.to_string())?
        .java_version
        .major_version;
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择此游戏使用的 Java");
    #[cfg(windows)]
    let dialog = dialog.add_filter("Java (java.exe / javaw.exe)", &["exe"]);
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let java = cobble_minecraft::probe_java(file.path())
        .await
        .map_err(|e| format!("无法使用这个 Java：{e}"))?;
    if java.major != required {
        return Err(format!(
            "这个游戏需要 Java {required}，所选版本为 Java {}",
            java.major
        ));
    }
    change_runtime(&library, &id, |settings| {
        settings.java = cobble_minecraft::JavaOverride::Custom {
            executable: java.executable,
        }
    })
    .map(Some)
}

pub(crate) fn game_java_requirement(library: &GameLibrary, id: &str) -> Result<u32, String> {
    let (version, directory) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        (game.version.clone(), game.directory.clone())
    };
    let bytes = fs::read(directory.join(format!("versions/{version}/{version}.json")))
        .map_err(|e| e.to_string())?;
    Ok(
        cobble_minecraft::VersionMetadata::parse_installed(&bytes, &version)
            .map_err(|e| e.to_string())?
            .java_version
            .major_version,
    )
}

#[tauri::command]
pub async fn list_game_java_runtimes(
    library: State<'_, GameLibrary>,
    id: String,
    rescan: Option<bool>,
) -> Result<crate::java::JavaRuntimeList, String> {
    let required = game_java_requirement(&library, &id)?;
    crate::java::list_java_runtimes(Some(required), rescan).await
}

#[tauri::command]
pub async fn set_game_java_runtime(
    library: State<'_, GameLibrary>,
    id: String,
    runtime_id: String,
) -> Result<Game, String> {
    let required = game_java_requirement(&library, &id)?;
    let java = crate::java::resolve_discovered_runtime(&runtime_id).await?;
    if cobble_minecraft::recommend_java(
        std::slice::from_ref(&java),
        required,
        std::env::consts::ARCH,
    )
    .is_none()
    {
        return Err(format!(
            "请选择 Java {required} 且架构与此电脑匹配的运行环境"
        ));
    }
    change_runtime(&library, &id, |settings| {
        settings.java = cobble_minecraft::JavaOverride::Custom {
            executable: java.executable,
        }
    })
}

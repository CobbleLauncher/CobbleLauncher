// SPDX-License-Identifier: MIT
use cobble_minecraft::{JavaRuntime, JavaSettings};
use serde::Serialize;
use tauri::WebviewWindow;

// Opaque session identifiers authorize only discovered executables, never a
// renderer-supplied filesystem path.
static DISCOVERED: std::sync::OnceLock<std::sync::Mutex<Vec<JavaRuntime>>> =
    std::sync::OnceLock::new();

/// Inspect known runtime metadata without probing an executable or starting a scan.
pub(crate) fn cached_major(
    choice: &cobble_minecraft::JavaOverride,
    required: Option<u32>,
) -> Option<u32> {
    let selected = match choice {
        cobble_minecraft::JavaOverride::Custom { executable } => Some(executable.clone()),
        cobble_minecraft::JavaOverride::Inherit => {
            cobble_minecraft::read_java_settings().ok()?.executable
        }
        cobble_minecraft::JavaOverride::Automatic => None,
    };
    let runtimes = DISCOVERED.get()?.lock().ok()?;
    match selected {
        Some(path) => runtimes.iter().find(|runtime| runtime.executable == path),
        None => cobble_minecraft::recommend_java(&runtimes, required?, std::env::consts::ARCH),
    }
    .map(|runtime| runtime.major)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredJava {
    id: String,
    #[serde(flatten)]
    runtime: JavaRuntimeInfo,
    recommended: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntimeList {
    runtimes: Vec<DiscoveredJava>,
    required_major: Option<u32>,
    recommended_id: Option<String>,
}

fn runtime_id(java: &JavaRuntime) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    java.executable.hash(&mut hash);
    format!("java-{:016x}", hash.finish())
}

/// Reuses the existing session registry; this never saves global or game settings.
fn register_candidate(runtime: JavaRuntime, required: u32) -> Result<DiscoveredJava, String> {
    if cobble_minecraft::recommend_java(
        std::slice::from_ref(&runtime),
        required,
        std::env::consts::ARCH,
    )
    .is_none()
    {
        return Err(format!(
            "请选择 Java {required} 且架构与此电脑匹配的运行环境"
        ));
    }
    let id = runtime_id(&runtime);
    remember_discovered(std::slice::from_ref(&runtime))?;
    Ok(DiscoveredJava {
        id,
        runtime: runtime.into(),
        recommended: false,
    })
}

fn remember_discovered(runtimes: &[JavaRuntime]) -> Result<(), String> {
    let mut discovered = DISCOVERED
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "Java 列表不可用")?;
    for runtime in runtimes {
        if let Some(previous) = discovered
            .iter_mut()
            .find(|item| item.executable == runtime.executable)
        {
            *previous = runtime.clone();
        } else {
            discovered.push(runtime.clone());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pick_java_candidate(window: WebviewWindow) -> Result<Option<DiscoveredJava>, String> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择 Java 程序");
    #[cfg(windows)]
    let dialog = dialog.add_filter("Java (java.exe / javaw.exe)", &["exe"]);
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let runtime = cobble_minecraft::probe_java(file.path())
        .await
        .map_err(|error| format!("无法使用这个 Java 程序：{error}"))?;
    // Global preferences have no single game's required major. Keep the same
    // architecture validation and opaque registry used by game candidates.
    let major = runtime.major;
    register_candidate(runtime, major).map(Some)
}

#[tauri::command]
pub async fn pick_game_java_candidate(
    window: WebviewWindow,
    library: tauri::State<'_, crate::games::GameLibrary>,
    id: String,
) -> Result<Option<DiscoveredJava>, String> {
    let required = crate::games::runtime::game_java_requirement(&library, &id)?;
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择此游戏使用的 Java");
    #[cfg(windows)]
    let dialog = dialog.add_filter("Java (java.exe / javaw.exe)", &["exe"]);
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let runtime = cobble_minecraft::probe_java(file.path())
        .await
        .map_err(|error| format!("无法使用这个 Java 程序：{error}"))?;
    register_candidate(runtime, required).map(Some)
}

#[tauri::command]
pub async fn list_java_runtimes(
    required_major: Option<u32>,
    rescan: Option<bool>,
) -> Result<JavaRuntimeList, String> {
    if required_major == Some(0) {
        return Err("Java 版本必须大于零".into());
    }
    let mut runtimes = cobble_minecraft::discover_java(rescan.unwrap_or(false)).await;
    if let Some(path) = cobble_minecraft::read_java_settings()
        .map_err(|e| e.to_string())?
        .executable
        && let Ok(runtime) = cobble_minecraft::probe_java(&path).await
        && !runtimes
            .iter()
            .any(|java| java.executable == runtime.executable)
    {
        runtimes.push(runtime);
    }
    let recommended_id = required_major
        .and_then(|major| {
            cobble_minecraft::recommend_java(&runtimes, major, std::env::consts::ARCH)
        })
        .map(runtime_id);
    // A refresh must not revoke an unsaved picker candidate. The response is
    // the fresh scan; previously authorized IDs remain session-local and are
    // probed again by the save command before becoming a persisted choice.
    remember_discovered(&runtimes)?;
    Ok(JavaRuntimeList {
        runtimes: runtimes
            .into_iter()
            .map(|runtime| {
                let id = runtime_id(&runtime);
                DiscoveredJava {
                    recommended: recommended_id.as_ref() == Some(&id),
                    id,
                    runtime: runtime.into(),
                }
            })
            .collect(),
        required_major,
        recommended_id,
    })
}

#[tauri::command]
pub async fn select_java_runtime(runtime_id: String) -> Result<JavaSelection, String> {
    let runtime = resolve_discovered_runtime(&runtime_id).await?;
    cobble_minecraft::save_java_settings(&JavaSettings {
        executable: Some(runtime.executable.clone()),
    })
    .map_err(|e| e.to_string())?;
    Ok(JavaSelection::custom(runtime))
}

pub(crate) async fn resolve_discovered_runtime(id: &str) -> Result<JavaRuntime, String> {
    let path = DISCOVERED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|java| self::runtime_id(java) == id)
        .map(|java| java.executable.clone())
        .ok_or("Java 列表已变化，请重新扫描后选择")?;
    cobble_minecraft::probe_java(&path)
        .await
        .map_err(|e| format!("无法使用这个 Java 程序：{e}"))
}

#[derive(Serialize)]
pub struct JavaRuntimeInfo {
    version: String,
    major: u32,
    arch: String,
    vendor: String,
    executable: String,
}
impl From<JavaRuntime> for JavaRuntimeInfo {
    fn from(java: JavaRuntime) -> Self {
        let executable = java.executable.display().to_string();
        Self {
            version: java.version,
            major: java.major,
            arch: java.arch,
            vendor: java.vendor,
            executable: executable
                .strip_prefix(r"\\?\")
                .unwrap_or(&executable)
                .into(),
        }
    }
}

#[derive(Serialize)]
pub struct JavaSelection {
    mode: &'static str,
    executable: Option<String>,
    runtime: Option<JavaRuntimeInfo>,
}
impl JavaSelection {
    fn automatic() -> Self {
        Self {
            mode: "automatic",
            executable: None,
            runtime: None,
        }
    }
    fn custom(java: JavaRuntime) -> Self {
        let runtime = JavaRuntimeInfo::from(java);
        Self {
            mode: "custom",
            executable: Some(runtime.executable.clone()),
            runtime: Some(runtime),
        }
    }
}

#[tauri::command]
pub async fn get_java_settings() -> Result<JavaSelection, String> {
    let settings = cobble_minecraft::read_java_settings().map_err(|e| e.to_string())?;
    match settings.executable {
        None => Ok(JavaSelection::automatic()),
        Some(path) => cobble_minecraft::probe_java(&path)
            .await
            .map(JavaSelection::custom)
            .map_err(|e| format!("选定的 Java 无法使用，请重新选择或切换自动选择：{e}")),
    }
}

#[tauri::command]
pub fn use_automatic_java() -> Result<JavaSelection, String> {
    cobble_minecraft::save_java_settings(&JavaSettings::default()).map_err(|e| e.to_string())?;
    Ok(JavaSelection::automatic())
}

#[tauri::command]
pub async fn pick_java_executable(window: WebviewWindow) -> Result<Option<JavaSelection>, String> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择 Java 程序");
    #[cfg(windows)]
    let dialog = dialog.add_filter("Java (java.exe / javaw.exe)", &["exe"]);
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let runtime = cobble_minecraft::probe_java(file.path())
        .await
        .map_err(|e| format!("无法使用这个 Java 程序：{e}"))?;
    cobble_minecraft::save_java_settings(&JavaSettings {
        executable: Some(runtime.executable.clone()),
    })
    .map_err(|e| e.to_string())?;
    Ok(Some(JavaSelection::custom(runtime)))
}

#[cfg(test)]
mod candidate_tests {
    use super::*;
    #[test]
    fn readonly_java_candidate_registers_valid_ids_and_rejects_incompatible_metadata() {
        let path = std::env::temp_dir().join("cobble-readonly-candidate-java.exe");
        let runtime = JavaRuntime {
            executable: path.clone(),
            version: "21.0.1".into(),
            major: 21,
            arch: std::env::consts::ARCH.into(),
            vendor: "Fixture".into(),
        };
        let candidate = register_candidate(runtime.clone(), 21).unwrap();
        assert!(candidate.id.starts_with("java-"));
        assert!(
            DISCOVERED
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .iter()
                .any(|value| value.executable == path)
        );
        let count = DISCOVERED.get().unwrap().lock().unwrap().len();
        assert!(
            register_candidate(
                JavaRuntime {
                    major: 17,
                    ..runtime.clone()
                },
                21
            )
            .is_err()
        );
        assert!(
            register_candidate(
                JavaRuntime {
                    arch: "unsupported".into(),
                    ..runtime
                },
                21
            )
            .is_err()
        );
        assert_eq!(DISCOVERED.get().unwrap().lock().unwrap().len(), count);
        // Global selection has no game-specific major; a different probed
        // major is still a candidate, with the same host-issued executable ID.
        let global = JavaRuntime {
            executable: path.clone(),
            version: "17.0.1".into(),
            major: 17,
            arch: std::env::consts::ARCH.into(),
            vendor: "Fixture".into(),
        };
        let global_major = global.major;
        let global_candidate = register_candidate(global, global_major).unwrap();
        assert_eq!(global_candidate.id, candidate.id);
        assert_eq!(global_candidate.runtime.major, 17);
        remember_discovered(&[]).unwrap();
        assert!(
            DISCOVERED
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .iter()
                .any(|value| runtime_id(value) == candidate.id)
        );
        DISCOVERED
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .retain(|value| value.executable != path);
    }
}

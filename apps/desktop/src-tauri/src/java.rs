// SPDX-License-Identifier: MIT
use cobble_minecraft::{JavaRuntime, JavaSettings};
use serde::Serialize;
use tauri::WebviewWindow;

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

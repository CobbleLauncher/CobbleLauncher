// SPDX-License-Identifier: MIT
//! Local launcher diagnostics. Only controlled event fields are logged; neither
//! credential storage, game argv nor game/save directories enter this export.
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tauri::WebviewWindow;
fn directory() -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("无法找到日志目录")?
        .join("CobbleLauncher/logs"))
}
pub fn init() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let directory = directory().ok()?;
    let appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("cobble")
        .filename_suffix("log")
        .max_log_files(7)
        .build(directory)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            "cobble_desktop=info,cobble_core=info,cobble_net=info,cobble_plugin_host=info",
        )
        .with_ansi(false)
        .with_writer(writer)
        .try_init()
        .ok()?;
    tracing::info!(
        event = "launcher_started",
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH
    );
    std::panic::set_hook(Box::new(|info| {
        if let Some(location) = info.location() {
            tracing::error!(
                event = "launcher_panic",
                file = Path::new(location.file())
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown"),
                line = location.line()
            );
        }
    }));
    Some(guard)
}
#[tauri::command]
pub fn open_launcher_logs() -> Result<(), String> {
    let directory = directory()?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    open::that(directory).map_err(|error| error.to_string())
}
fn export(directory: &Path, output: std::fs::File) -> Result<(), String> {
    let mut archive = zip::ZipWriter::new(output);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let environment = serde_json::json!({"application":"Cobble Launcher","version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH});
    archive
        .start_file("environment.json", options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(
            serde_json::to_string_pretty(&environment)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    let mut logs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with("cobble.")
                && name.ends_with(".log")
                && entry.file_type().is_ok_and(|kind| kind.is_file())
            {
                logs.push(entry.path());
            }
        }
    }
    logs.sort();
    let skip = logs.len().saturating_sub(7);
    for path in logs.into_iter().skip(skip) {
        let mut file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
        let size = file.metadata().map_err(|error| error.to_string())?.len();
        file.seek(SeekFrom::Start(size.saturating_sub(2 * 1024 * 1024)))
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        archive
            .start_file(
                format!("logs/{}", path.file_name().unwrap().to_string_lossy()),
                options,
            )
            .map_err(|error| error.to_string())?;
        archive
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
    }
    archive.finish().map_err(|error| error.to_string())?;
    Ok(())
}
#[tauri::command]
pub async fn export_launcher_diagnostics(window: WebviewWindow) -> Result<bool, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导出诊断包")
        .set_file_name("cobble-diagnostics.zip")
        .add_filter("ZIP", &["zip"])
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let path = file.path().to_owned();
    let logs = directory()?;
    tauri::async_runtime::spawn_blocking(move || {
        let parent = path.parent().ok_or("保存位置无效")?;
        let temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        export(
            &logs,
            temporary.reopen().map_err(|error| error.to_string())?,
        )?;
        temporary
            .persist(&path)
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(true)
    })
    .await
    .map_err(|error| error.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_export_omits_accounts_and_unrelated_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("cobble.2026-10-22.log"),
            b"event=launcher_started",
        )
        .unwrap();
        std::fs::write(root.path().join("accounts.json"), b"fake-secret").unwrap();
        let output = tempfile::NamedTempFile::new().unwrap();
        export(root.path(), output.reopen().unwrap()).unwrap();
        let mut zip = zip::ZipArchive::new(output.reopen().unwrap()).unwrap();
        assert_eq!(zip.len(), 2);
        assert!(zip.by_name("environment.json").is_ok());
        assert!(zip.by_name("logs/cobble.2026-10-22.log").is_ok());
        assert!(zip.by_name("accounts.json").is_err());
    }
}

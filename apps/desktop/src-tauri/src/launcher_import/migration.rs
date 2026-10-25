// SPDX-License-Identifier: MIT
use super::*;
use std::path::Path;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    mode: String,
    source_retained: bool,
    recycled_roots: usize,
    retained_roots: usize,
}
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}
fn process_uses_source(name: &str, args: &[String], roots: &[PathBuf]) -> Result<bool, String> {
    if !matches!(
        name.to_ascii_lowercase().as_str(),
        "java" | "javaw" | "java.exe" | "javaw.exe"
    ) {
        return Ok(false);
    }
    if args.is_empty() {
        return Err("无法确认外部 Java 进程的游戏目录，请关闭来源游戏后重试迁移".into());
    }
    let paths = roots
        .iter()
        .map(|path| normalized(path))
        .collect::<Vec<_>>();
    Ok(args.iter().any(|arg| {
        let arg = arg.replace('\\', "/").to_ascii_lowercase();
        paths.iter().any(|root| arg.contains(root))
    }))
}
pub(super) fn ensure_stopped(
    candidate: &LauncherImportCandidate,
    content_id: &str,
) -> Result<(), String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let root = candidate
        .content_roots
        .iter()
        .find(|root| root.id == content_id)
        .ok_or("缺少来源内容目录")?;
    let mut roots = vec![root.directory.clone()];
    if let Some(runtime) = &candidate.runtime_source {
        roots.push(runtime.directory.clone());
    }
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
    for process in system.processes().values() {
        let java = matches!(
            process
                .name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            "java" | "javaw" | "java.exe" | "javaw.exe"
        );
        let mut args = process
            .cmd()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if java && args.is_empty() {
            return Err("无法确认外部 Java 进程的游戏目录，请关闭来源游戏后重试迁移".into());
        }
        if let Some(cwd) = process.cwd() {
            args.push(cwd.to_string_lossy().into_owned());
        } else if java && !args.iter().any(|arg| arg == "--gameDir") {
            return Err("无法确认外部 Java 进程的游戏目录，请关闭来源游戏后重试迁移".into());
        }
        if process_uses_source(&process.name().to_string_lossy(), &args, &roots)? {
            return Err("来源游戏正在运行，请关闭后再迁移；也可以改用复制导入".into());
        }
    }
    Ok(())
}
fn store(id: &str, result: ImportResult) -> Result<(), String> {
    let path = plan_path(id)?;
    let mut plan: ImportPlan = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    plan.result = Some(result);
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("缺少导入记录目录")?)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(temporary.as_file_mut(), &plan).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub(super) async fn finish(game: &Game, plan: &ImportPlan, control: cobble_core::OperationControl) {
    let Some(id) = game.launcher_import_id.clone() else {
        return;
    };
    let directory = game.directory.clone();
    let candidate = plan.candidate.clone();
    let content_id = plan.content_id.clone();
    let move_source = plan.move_source;
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let mut report = ImportResult {
            mode: if move_source { "move" } else { "copy" }.into(),
            source_retained: !move_source,
            recycled_roots: 0,
            retained_roots: 0,
        };
        report.source_retained = true;
        report.retained_roots = retained_count(&directory);
        // Persist a conservative result before any source-side action. If that
        // write fails the import remains a copy and no recycling is attempted.
        store(&id, report.clone())?;
        if move_source {
            let release =
                cobble_core::release_launcher_import_source(&directory, &control, |path| {
                    let expected = candidate
                        .content_roots
                        .iter()
                        .find(|root| root.id == content_id)
                        .ok_or("IMPORT_MOVE_SOURCE_CHANGED")?;
                    let actual =
                        fs::canonicalize(path.parent().ok_or("IMPORT_MOVE_SOURCE_CHANGED")?)?;
                    if actual != fs::canonicalize(&expected.directory)? {
                        return Err("IMPORT_MOVE_SOURCE_CHANGED".into());
                    }
                    ensure_stopped(&candidate, &content_id)
                        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
                    cobble_platform::recycle_directory(path).map_err(|e| e.into())
                });
            match release {
                Ok(result) => {
                    report.recycled_roots = result.recycled_roots;
                    report.retained_roots = result.retained_roots;
                    report.source_retained = result.retained_roots > 0 || result.issue.is_some();
                }
                Err(_) => {
                    report.source_retained = true;
                    let (released, retained) = receipt_counts(&directory);
                    report.recycled_roots = released;
                    report.retained_roots = retained;
                }
            }
        }
        store(&id, report)
    })
    .await;
}
fn retained_count(directory: &Path) -> usize {
    receipt_counts(directory).1
}
fn receipt_counts(directory: &Path) -> (usize, usize) {
    // Count only names from our completed import receipt, without reading foreign data.
    fs::read(directory.join(".cobble-import.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| {
            value["entries"].as_array().map(|entries| {
                let released = value["released_roots"].as_array().map_or(0, Vec::len);
                (released, entries.len().saturating_sub(released))
            })
        })
        .unwrap_or((0, 0))
}
fn merged_report(mut report: ImportResult, counts: (usize, usize)) -> ImportResult {
    if report.mode == "move" && counts.0 > report.recycled_roots {
        report.recycled_roots = counts.0;
        report.retained_roots = counts.1;
        report.source_retained = counts.1 > 0;
    }
    report
}
pub(super) fn reconcile_report(game: &Game, plan: &ImportPlan) -> Result<(), String> {
    if let (Some(id), Some(report)) = (game.launcher_import_id.as_deref(), plan.result.clone()) {
        store(id, merged_report(report, receipt_counts(&game.directory)))?;
    }
    Ok(())
}
pub(super) fn get_launcher_import_result(
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Option<ImportResult>, String> {
    let library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("游戏不存在")?;
    if game.status != "installed" {
        return Ok(None);
    }
    let Some(import_id) = &game.launcher_import_id else {
        return Ok(None);
    };
    let path = plan_path(import_id)?;
    let plan: ImportPlan = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (released, retained) = receipt_counts(&game.directory);
    let result = plan.result.unwrap_or_else(|| ImportResult {
        mode: if plan.move_source { "move" } else { "copy" }.into(),
        source_retained: true,
        recycled_roots: released,
        retained_roots: retained,
    });
    Ok(Some(merged_report(result, (released, retained))))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_running_source_and_unobservable_java_refuse_migration_without_stopping_processes() {
        let roots = vec![PathBuf::from("C:/Games/PCL/.minecraft")];
        assert!(
            process_uses_source(
                "javaw.exe",
                &[
                    "--gameDir".into(),
                    "C:\\Games\\PCL\\.minecraft\\versions\\Example".into()
                ],
                &roots
            )
            .unwrap()
        );
        assert!(
            !process_uses_source(
                "java.exe",
                &["-jar".into(), "C:/other/app.jar".into()],
                &roots
            )
            .unwrap()
        );
        assert!(process_uses_source("javaw.exe", &[], &roots).is_err());
        assert!(!process_uses_source("explorer.exe", &[], &roots).unwrap());
    }
    #[test]
    fn interrupted_native_result_reconstructs_counts_from_completed_core_receipt_without_more_recycling()
     {
        let report = ImportResult {
            mode: "move".into(),
            source_retained: true,
            recycled_roots: 0,
            retained_roots: 3,
        };
        let repaired = merged_report(report, (2, 1));
        assert_eq!(repaired.recycled_roots, 2);
        assert_eq!(repaired.retained_roots, 1);
        assert!(repaired.source_retained);
    }
}

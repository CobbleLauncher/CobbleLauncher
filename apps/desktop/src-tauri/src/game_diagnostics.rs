// SPDX-License-Identifier: MIT
//! Read-only diagnostic inventory rooted in a registered game, never caller paths.
use super::*;
use cap_std::fs::Dir;
use std::{io::Read, path::Path};
const MAX_ENTRIES: usize = 512;
const MAX_FILES: usize = 64;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub modified_at: Option<u64>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDocument {
    pub id: String,
    pub name: String,
    pub text: String,
    pub revision: String,
}
fn io() -> String {
    "DIAGNOSTIC_IO".into()
}
fn plain(dir: &Dir, name: &Path, directory: bool) -> Result<(), String> {
    let metadata = dir.symlink_metadata(name).map_err(|_| io())?;
    if metadata.file_type().is_symlink()
        || (if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        })
    {
        return Err(io());
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io());
        }
    }
    Ok(())
}
fn root(directory: &Path) -> Result<Dir, String> {
    Dir::open_ambient_dir(directory, cap_std::ambient_authority()).map_err(|_| io())
}
fn listing(root: &Dir) -> Result<Vec<DiagnosticFile>, String> {
    let mut files = vec![];
    let mut seen = 0;
    for folder in ["logs", "crash-reports", ".cobble-logs"] {
        if !root.try_exists(folder).map_err(|_| io())? {
            continue;
        }
        plain(root, Path::new(folder), true)?;
        let dir = root.open_dir(folder).map_err(|_| io())?;
        for entry in dir.entries().map_err(|_| io())? {
            seen += 1;
            if seen > MAX_ENTRIES {
                return Err("DIAGNOSTIC_INVENTORY_LIMIT".into());
            }
            let entry = entry.map_err(|_| io())?;
            let local = entry.file_name();
            let Some(name) = local.to_str() else {
                continue;
            };
            if !matches!(
                Path::new(name).extension().and_then(|value| value.to_str()),
                Some("log" | "txt")
            ) || plain(&dir, Path::new(name), false).is_err()
            {
                continue;
            }
            let metadata = dir.metadata(name).map_err(|_| io())?;
            if metadata.len() > cobble_diagnostics::MAX_INPUT_BYTES as u64 {
                continue;
            }
            let relative = format!("{folder}/{name}");
            files.push(DiagnosticFile {
                id: cobble_plugin_host::digest(relative.as_bytes()),
                name: relative,
                size: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.into_std().duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs()),
            });
        }
    }
    files.sort_by(|a, b| b.modified_at.cmp(&a.modified_at).then(a.name.cmp(&b.name)));
    files.truncate(MAX_FILES);
    Ok(files)
}
pub fn read_at(directory: &Path, id: &str) -> Result<DiagnosticDocument, String> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("DIAGNOSTIC_NOT_FOUND".into());
    }
    let root = root(directory)?;
    let file = listing(&root)?
        .into_iter()
        .find(|file| file.id == id)
        .ok_or("DIAGNOSTIC_NOT_FOUND")?;
    let (folder, name) = file.name.split_once('/').ok_or("DIAGNOSTIC_NOT_FOUND")?;
    plain(&root, Path::new(folder), true)?;
    let dir = root.open_dir(folder).map_err(|_| io())?;
    plain(&dir, Path::new(name), false)?;
    let mut bytes = vec![];
    dir.open(name)
        .map_err(|_| io())?
        .take(cobble_diagnostics::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io())?;
    if bytes.len() > cobble_diagnostics::MAX_INPUT_BYTES {
        return Err("DIAGNOSTIC_TOO_LARGE".into());
    }
    let revision = cobble_plugin_host::digest(&bytes);
    let raw = std::str::from_utf8(&bytes).map_err(|_| "DIAGNOSTIC_NOT_UTF8")?;
    let text = cobble_diagnostics::redact_log(raw);
    if text.len() > cobble_diagnostics::MAX_INPUT_BYTES {
        return Err("DIAGNOSTIC_TOO_LARGE".into());
    }
    Ok(DiagnosticDocument {
        id: file.id,
        name: file.name,
        text,
        revision,
    })
}
pub fn recent(app: &AppHandle, bound: Option<&str>) -> Result<Vec<serde_json::Value>, String> {
    let mut games = list_games(app.state::<GameLibrary>()).map_err(|_| io())?;
    games.retain(|game| bound.is_none_or(|id| id == game.id));
    games.sort_by(|a, b| {
        b.last_launched_at
            .cmp(&a.last_launched_at)
            .then(a.name.cmp(&b.name))
    });
    Ok(games.into_iter().take(64).map(|game| serde_json::json!({"id":game.id,"name":game.name,"version":game.version,"lastLaunchedAt":game.last_launched_at})).collect())
}
pub async fn list(app: AppHandle, game_id: String) -> Result<Vec<DiagnosticFile>, String> {
    let directory = game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || listing(&root(&directory)?))
        .await
        .map_err(|_| io())?
}
pub async fn read(
    app: AppHandle,
    game_id: String,
    id: String,
) -> Result<DiagnosticDocument, String> {
    let directory = game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || read_at(&directory, &id))
        .await
        .map_err(|_| io())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_fixed_diagnostic_roots_and_opaque_ids_are_readable() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("logs")).unwrap();
        fs::create_dir_all(temp.path().join("config")).unwrap();
        fs::write(
            temp.path().join("logs/latest.log"),
            "java.lang.OutOfMemoryError: Java heap space",
        )
        .unwrap();
        fs::write(temp.path().join("config/secret.txt"), "private").unwrap();
        let list = listing(&root(temp.path()).unwrap()).unwrap();
        assert_eq!(list.len(), 1);
        let doc = read_at(temp.path(), &list[0].id).unwrap();
        assert_eq!(doc.name, "logs/latest.log");
        assert!(read_at(temp.path(), "../config/secret.txt").is_err());
        assert!(
            read_at(
                temp.path(),
                &cobble_plugin_host::digest(b"config/secret.txt")
            )
            .is_err()
        );
        fs::write(temp.path().join("logs/latest.log"), "changed").unwrap();
        assert_ne!(
            doc.revision,
            read_at(temp.path(), &list[0].id).unwrap().revision
        );
    }
    #[test]
    fn inventory_is_bounded_and_rejects_binary_documents() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("crash-reports")).unwrap();
        fs::write(temp.path().join("crash-reports/binary.txt"), [255]).unwrap();
        let files = listing(&root(temp.path()).unwrap()).unwrap();
        assert_eq!(
            read_at(temp.path(), &files[0].id).err().as_deref(),
            Some("DIAGNOSTIC_NOT_UTF8")
        );
        fs::write(
            temp.path().join("crash-reports/oversized.txt"),
            vec![b'x'; cobble_diagnostics::MAX_INPUT_BYTES + 1],
        )
        .unwrap();
        assert_eq!(listing(&root(temp.path()).unwrap()).unwrap().len(), 1);
        for index in 0..MAX_ENTRIES {
            fs::write(temp.path().join(format!("crash-reports/{index}.log")), b"x").unwrap();
        }
        assert_eq!(
            listing(&root(temp.path()).unwrap()).err().as_deref(),
            Some("DIAGNOSTIC_INVENTORY_LIMIT")
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_roots_and_files_are_not_disclosed() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        symlink(outside.path(), temp.path().join("logs")).unwrap();
        assert!(listing(&root(temp.path()).unwrap()).is_err());
        fs::remove_file(temp.path().join("logs")).unwrap();
        fs::create_dir(temp.path().join("logs")).unwrap();
        symlink(
            outside.path().join("secret.txt"),
            temp.path().join("logs/secret.txt"),
        )
        .unwrap();
        assert!(listing(&root(temp.path()).unwrap()).unwrap().is_empty());
    }
}

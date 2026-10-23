// SPDX-License-Identifier: MIT
//! Copy saved blueprint bytes under the existing stopped-instance operation guard.
use super::*;
use std::io::Write;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintImportResult {
    destination: String,
}

#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintDuplicatePolicy {
    #[default]
    Error,
    KeepBoth,
}

#[tauri::command]
pub async fn open_blueprint_destination(app: AppHandle, id: String) -> Result<(), String> {
    let directory = game_directory(&app, &id)?.join("schematics");
    real_directory(&directory)?;
    tauri::async_runtime::spawn_blocking(move || {
        open::that(directory).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn import_blueprint(
    app: AppHandle,
    id: String,
    blueprint_id: String,
    duplicate_policy: Option<BlueprintDuplicatePolicy>,
) -> Result<BlueprintImportResult, String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let (name, bytes, format) = crate::plugins::saved_blueprint(&blueprint_id)?;
        let destination = copy_blueprint_with_policy(
            &game.directory,
            &name,
            &bytes,
            format,
            duplicate_policy.unwrap_or_default(),
        )?;
        Ok(BlueprintImportResult {
            destination: destination.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn real_directory(path: &std::path::Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked || !metadata.is_dir() {
        return Err("蓝图目标必须是普通目录".into());
    }
    Ok(())
}

#[cfg(test)]
fn copy_schematic(
    directory: &std::path::Path,
    name: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    copy_blueprint(
        directory,
        name,
        bytes,
        crate::plugins::BlueprintFormat::Schem,
    )
}

#[cfg(test)]
fn copy_blueprint(
    directory: &std::path::Path,
    name: &str,
    bytes: &[u8],
    format: crate::plugins::BlueprintFormat,
) -> Result<PathBuf, String> {
    copy_blueprint_with_policy(
        directory,
        name,
        bytes,
        format,
        BlueprintDuplicatePolicy::KeepBoth,
    )
}

fn copy_blueprint_with_policy(
    directory: &std::path::Path,
    name: &str,
    bytes: &[u8],
    format: crate::plugins::BlueprintFormat,
    policy: BlueprintDuplicatePolicy,
) -> Result<PathBuf, String> {
    real_directory(directory)?;
    let target = directory.join("schematics");
    match fs::create_dir(&target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    real_directory(&target)?;
    // Keep generated names portable, including when metadata has been edited locally.
    let extension = format.extension();
    let stem = if name
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        &name[..name.len() - extension.len() - 1]
    } else {
        name
    };
    let stem: String = stem
        .chars()
        .take(48)
        .map(|c| {
            if c.is_control() || r#"/\:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let stem = format!(
        "blueprint-{}",
        if stem.trim_end_matches(['.', ' ']).is_empty() {
            "saved"
        } else {
            stem.trim_end_matches(['.', ' '])
        }
    );
    let mut temporary =
        tempfile::NamedTempFile::new_in(&target).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    for suffix in 0..10_000 {
        let name = if suffix == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{suffix}.{extension}")
        };
        let destination = target.join(name);
        match temporary.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                if matches!(policy, BlueprintDuplicatePolicy::Error) {
                    return Err(
                        "BLUEPRINT_ALREADY_EXISTS: choose Keep both to import with a new name"
                            .into(),
                    );
                }
                temporary = error.file
            }
            Err(error) => return Err(error.error.to_string()),
        }
    }
    Err("同名蓝图过多，请清理 schematics 目录后重试".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_litematic_bytes_and_extension_even_after_rename() {
        let root = tempfile::tempdir().unwrap();
        let bytes = b"original litematic source bytes";
        let path = copy_blueprint(
            root.path(),
            "Renamed house",
            bytes,
            crate::plugins::BlueprintFormat::Litematic,
        )
        .unwrap();
        assert_eq!(path.extension().unwrap(), "litematic");
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn explicit_duplicate_stop_keeps_original_bytes() {
        let root = tempfile::tempdir().unwrap();
        let first = copy_schematic(root.path(), "house.schem", b"original").unwrap();
        let error = copy_blueprint_with_policy(
            root.path(),
            "house.schem",
            b"replacement",
            crate::plugins::BlueprintFormat::Schem,
            BlueprintDuplicatePolicy::Error,
        )
        .unwrap_err();
        assert!(error.starts_with("BLUEPRINT_ALREADY_EXISTS"));
        assert_eq!(fs::read(first).unwrap(), b"original");
        assert_eq!(
            fs::read_dir(root.path().join("schematics"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn copies_bytes_and_preserves_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let first = copy_schematic(root.path(), "house.schem", b"original").unwrap();
        let second = copy_schematic(root.path(), "house.schem", b"second").unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(first).unwrap(), b"original");
        assert_eq!(fs::read(second).unwrap(), b"second");
        assert_eq!(
            fs::read_dir(root.path().join("schematics"))
                .unwrap()
                .count(),
            2
        );
    }

    #[test]
    fn metadata_name_cannot_escape_schematics() {
        let root = tempfile::tempdir().unwrap();
        let path = copy_schematic(root.path(), "../../CON:bad.schem", b"data").unwrap();
        assert_eq!(path.parent().unwrap(), root.path().join("schematics"));
        assert_eq!(path.extension().unwrap(), "schem");
    }

    #[test]
    fn preserves_unicode_names_and_normalizes_extension() {
        let root = tempfile::tempdir().unwrap();
        let path = copy_schematic(root.path(), "小屋.SCHEM", b"data").unwrap();
        assert_eq!(path.file_name().unwrap(), "blueprint-小屋.schem");
    }

    #[test]
    fn rejects_non_directory_destination() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("schematics"), b"keep").unwrap();
        assert!(copy_schematic(root.path(), "test.schem", b"data").is_err());
        assert_eq!(fs::read(root.path().join("schematics")).unwrap(), b"keep");
    }
}

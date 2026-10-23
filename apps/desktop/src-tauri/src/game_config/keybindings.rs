// SPDX-License-Identifier: MIT
//! Explicit, revision-checked transfer of modern options.txt keybindings only.
use super::*;
use std::collections::BTreeMap;
const OPTIONS: &str = "options.txt";
fn options_id() -> String {
    digest(b"root-options-keybindings")
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingChange {
    key: String,
    before: String,
    after: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPreview {
    source_revision: String,
    target_revision: String,
    changes: Vec<BindingChange>,
    skipped: usize,
    compatible: usize,
    conflicts: Vec<BindingConflict>,
    skipped_bindings: Vec<SkippedBinding>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingConflict {
    key: String,
    other_key: String,
    value: String,
}
#[derive(Serialize)]
pub struct SkippedBinding {
    key: String,
    reason: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    backup_id: String,
    target_revision: String,
}
fn read_options(root: &Dir) -> Result<(Vec<u8>, String), String> {
    if root
        .metadata(OPTIONS)
        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        return Err("KEYBINDINGS_NOT_INITIALIZED".into());
    }
    let bytes = bounded(root, Path::new(OPTIONS))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "CONFIG_NOT_UTF8")?
        .to_string();
    if text.contains('\0') {
        return Err("CONFIG_NOT_UTF8".into());
    }
    Ok((bytes, text))
}
fn modern(value: &str) -> bool {
    let suffix = value
        .strip_prefix("key.keyboard.")
        .or_else(|| value.strip_prefix("key.mouse."));
    suffix.is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 80
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
    })
}
fn bindings(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.starts_with("key_") && result.insert(key.into(), value.into()).is_some() {
            return Err("KEYBINDINGS_DUPLICATE".into());
        }
    }
    Ok(result)
}
fn merge(source: &str, target: &str) -> Result<(String, Vec<BindingChange>, usize), String> {
    let source = bindings(source)?;
    let target_keys = bindings(target)?;
    let mut changes = Vec::new();
    let mut skipped = 0;
    for (key, after) in &source {
        let Some(before) = target_keys.get(key) else {
            skipped += 1;
            continue;
        };
        if !modern(after) || !modern(before) {
            skipped += 1;
            continue;
        }
        if before != after {
            changes.push(BindingChange {
                key: key.clone(),
                before: before.clone(),
                after: after.clone(),
            });
        }
    }
    let mut merged = String::new();
    for line in target.split_inclusive('\n') {
        let (body, ending) = if let Some(body) = line.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = line.strip_suffix('\n') {
            (body, "\n")
        } else {
            (line, "")
        };
        let plain = body.trim_start_matches('\u{feff}');
        let changed = plain
            .split_once(':')
            .and_then(|(key, _)| changes.iter().find(|change| change.key == key));
        if let Some(change) = changed {
            if body.starts_with('\u{feff}') {
                merged.push('\u{feff}');
            }
            merged.push_str(&change.key);
            merged.push(':');
            merged.push_str(&change.after);
        } else {
            merged.push_str(body);
        }
        merged.push_str(ending);
    }
    Ok((merged, changes, skipped))
}
fn preview(source: &Dir, target: &Dir) -> Result<TransferPreview, String> {
    let (source_bytes, source_text) = read_options(source)?;
    let (target_bytes, target_text) = read_options(target)?;
    let (_, changes, skipped) = merge(&source_text, &target_text)?;
    let source_bindings = bindings(&source_text)?;
    let target_bindings = bindings(&target_text)?;
    let mut skipped_bindings = Vec::new();
    let mut compatible = 0;
    let mut resulting = target_bindings.clone();
    for (key, value) in &source_bindings {
        match target_bindings.get(key) {
            None => skipped_bindings.push(SkippedBinding {
                key: key.clone(),
                reason: "missing-target",
            }),
            Some(before) if !modern(before) || !modern(value) => {
                skipped_bindings.push(SkippedBinding {
                    key: key.clone(),
                    reason: "unsupported-value",
                })
            }
            Some(_) => {
                compatible += 1;
                resulting.insert(key.clone(), value.clone());
            }
        }
    }
    let mut conflicts = Vec::new();
    for change in &changes {
        if change.after == "key.keyboard.unknown" {
            continue;
        }
        for (other_key, value) in &resulting {
            if other_key != &change.key && value == &change.after {
                conflicts.push(BindingConflict {
                    key: change.key.clone(),
                    other_key: other_key.clone(),
                    value: value.clone(),
                });
            }
        }
    }
    Ok(TransferPreview {
        source_revision: digest(&source_bytes),
        target_revision: digest(&target_bytes),
        changes,
        skipped,
        compatible,
        conflicts,
        skipped_bindings,
    })
}
fn apply(
    source: &Dir,
    target: &Dir,
    source_revision: &str,
    target_revision: &str,
) -> Result<TransferResult, String> {
    let (source_bytes, source_text) = read_options(source)?;
    let (target_bytes, target_text) = read_options(target)?;
    if digest(&source_bytes) != source_revision || digest(&target_bytes) != target_revision {
        return Err("CONFIG_CHANGED".into());
    }
    let (text, changes, _) = merge(&source_text, &target_text)?;
    if changes.is_empty() {
        return Err("KEYBINDINGS_NO_CHANGES".into());
    }
    write_existing(
        target,
        &options_id(),
        target,
        Path::new(OPTIONS),
        &target_bytes,
        &text,
        target_revision,
    )?;
    let backup_id = backups(target, &options_id())?
        .into_iter()
        .find(|backup| backup.id.ends_with(target_revision))
        .ok_or_else(failure)?
        .id;
    Ok(TransferResult {
        backup_id,
        target_revision: digest(text.as_bytes()),
    })
}
#[tauri::command]
pub async fn preview_game_keybindings(
    app: AppHandle,
    source_game_id: String,
    target_game_id: String,
) -> Result<TransferPreview, String> {
    if source_game_id == target_game_id {
        return Err("KEYBINDINGS_SAME_GAME".into());
    }
    let source = game_directory(&app, &source_game_id)?;
    let target = game_directory(&app, &target_game_id)?;
    tauri::async_runtime::spawn_blocking(move || preview(&root(&source)?, &root(&target)?))
        .await
        .map_err(|_| failure())?
}
#[tauri::command]
pub async fn apply_game_keybindings(
    app: AppHandle,
    source_game_id: String,
    target_game_id: String,
    source_revision: String,
    target_revision: String,
) -> Result<TransferResult, String> {
    if source_game_id == target_game_id {
        return Err("KEYBINDINGS_SAME_GAME".into());
    }
    let (source, source_operation) =
        management::begin(&app, &source_game_id, true).map_err(|_| "CONFIG_BUSY")?;
    let (target, target_operation) =
        management::begin(&app, &target_game_id, true).map_err(|_| "CONFIG_BUSY")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guards = (source_operation, target_operation);
        apply(
            &root(&source.directory)?,
            &root(&target.directory)?,
            &source_revision,
            &target_revision,
        )
    })
    .await
    .map_err(|_| failure())?
}
#[tauri::command]
pub async fn restore_game_keybindings(
    app: AppHandle,
    target_game_id: String,
    backup_id: String,
    target_revision: String,
) -> Result<String, String> {
    let (game, operation) =
        management::begin(&app, &target_game_id, true).map_err(|_| "CONFIG_BUSY")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let root = root(&game.directory)?;
        let id = options_id();
        if !backups(&root, &id)?
            .iter()
            .any(|backup| backup.id == backup_id)
        {
            return Err("CONFIG_BACKUP_NOT_FOUND".into());
        }
        let dir = backup_dir(&root, &id, false)?.ok_or("CONFIG_BACKUP_NOT_FOUND")?;
        let old = bounded(&dir, Path::new(&backup_id))?;
        if backup_id.split_once('-').map(|(_, hash)| hash) != Some(digest(&old).as_str()) {
            return Err("CONFIG_BACKUP_CHANGED".into());
        }
        let (current, _) = read_options(&root)?;
        let text = String::from_utf8(old).map_err(|_| "CONFIG_NOT_UTF8")?;
        write_existing(
            &root,
            &id,
            &root,
            Path::new(OPTIONS),
            &current,
            &text,
            &target_revision,
        )?;
        Ok(digest(text.as_bytes()))
    })
    .await
    .map_err(|_| failure())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_only_existing_modern_bindings_preserving_target() {
        let (text,changes,skipped)=merge("key_key.attack:key.mouse.right\nkey_key.missing:key.keyboard.a\nkey_key.jump:57\n","\u{feff}volume:0.2\r\nkey_key.attack:key.mouse.left\r\nkey_key.jump:key.keyboard.space\r\n").unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(skipped, 2);
        assert_eq!(
            text,
            "\u{feff}volume:0.2\r\nkey_key.attack:key.mouse.right\r\nkey_key.jump:key.keyboard.space\r\n"
        );
    }
    #[test]
    fn stale_write_and_restore_backup() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fs::write(a.path().join(OPTIONS), "key_key.jump:key.keyboard.a\n").unwrap();
        fs::write(
            b.path().join(OPTIONS),
            "key_key.jump:key.keyboard.space\nvolume:0.5\n",
        )
        .unwrap();
        let a = root(a.path()).unwrap();
        let b = root(b.path()).unwrap();
        let p = preview(&a, &b).unwrap();
        assert!(apply(&a, &b, "stale", &p.target_revision).is_err());
        assert!(backups(&b, &options_id()).unwrap().is_empty());
        let result = apply(&a, &b, &p.source_revision, &p.target_revision).unwrap();
        let backup = backup_dir(&b, &options_id(), false).unwrap().unwrap();
        assert_eq!(
            bounded(&backup, Path::new(&result.backup_id)).unwrap(),
            b"key_key.jump:key.keyboard.space\nvolume:0.5\n"
        );
        assert_eq!(
            read_options(&b).unwrap().1,
            "key_key.jump:key.keyboard.a\nvolume:0.5\n"
        );
    }
    #[test]
    fn rejects_duplicate_and_legacy_values() {
        assert!(bindings("key_key.jump:a\nkey_key.jump:b").is_err());
        assert!(!modern("57"));
        assert!(!modern("scancode.57"));
    }
    #[test]
    fn explains_skips_and_resulting_conflicts() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fs::write(
            a.path().join(OPTIONS),
            "key_key.jump:key.keyboard.w\nkey_mod.absent:key.keyboard.a\nkey_key.sneak:42\n",
        )
        .unwrap();
        fs::write(b.path().join(OPTIONS), "key_key.jump:key.keyboard.space\nkey_key.forward:key.keyboard.w\nkey_key.sneak:key.keyboard.left.shift\n").unwrap();
        let value = preview(&root(a.path()).unwrap(), &root(b.path()).unwrap()).unwrap();
        assert_eq!(value.compatible, 1);
        assert_eq!(value.skipped, 2);
        assert_eq!(value.skipped_bindings[0].reason, "unsupported-value");
        assert_eq!(value.skipped_bindings[1].reason, "missing-target");
        assert_eq!(value.conflicts.len(), 1);
        assert_eq!(value.conflicts[0].other_key, "key_key.forward");
    }
}

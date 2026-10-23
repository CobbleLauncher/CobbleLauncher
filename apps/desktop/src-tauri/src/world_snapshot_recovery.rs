// SPDX-License-Identifier: MIT
//! Explicit native recovery-file access. Credentials never cross IPC or enter errors.
use super::*;
use std::{
    io::{Read, Write},
    path::Path,
};

const MAX_RECOVERY_BYTES: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryKey {
    schema_version: u32,
    repository_id: String,
    password: String,
}

fn parse(bytes: &[u8], expected_id: &str) -> Result<RecoveryKey, String> {
    if bytes.len() > MAX_RECOVERY_BYTES {
        return Err("恢复密钥文件过大".into());
    }
    let key: RecoveryKey = serde_json::from_slice(bytes).map_err(|_| "恢复密钥格式无效")?;
    if key.schema_version != 1 || key.repository_id != expected_id {
        return Err("恢复密钥不属于这个快照仓库".into());
    }
    if key.password.len() != 64 || !key.password.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("恢复密钥格式无效".into());
    }
    Ok(key)
}

fn verified(key: &RecoveryKey, root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("本机没有这个快照仓库".into());
    }
    WorldSnapshotRepository::open(root, &key.password)
        .map(|_| ())
        .map_err(|_| "恢复密钥无法解锁这个快照仓库".into())
}

fn read_key(path: &Path, expected_id: &str) -> Result<RecoveryKey, String> {
    let file = fs::File::open(path).map_err(|_| "无法读取恢复密钥文件")?;
    if !file
        .metadata()
        .map_err(|_| "无法读取恢复密钥文件")?
        .is_file()
    {
        return Err("请选择普通密钥文件".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECOVERY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取恢复密钥文件")?;
    parse(&bytes, expected_id)
}

fn write_key(path: &Path, key: &RecoveryKey) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(key).map_err(|_| "无法生成恢复密钥文件")?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "无法保存恢复密钥，请选择新的文件名")?;
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
        return Err("无法保存恢复密钥文件".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn export_world_snapshot_recovery(
    window: WebviewWindow,
    app: AppHandle,
    game_id: String,
) -> Result<bool, String> {
    let activity = SnapshotActivity::new("recovery_export", &game_id);
    let (game, _operation) = management::begin(&app, &game_id, true)?;
    let id = game.id.clone();
    let key = tauri::async_runtime::spawn_blocking(move || {
        let entry = keyring::Entry::new("CobbleLauncher.WorldSnapshots", &id)
            .map_err(|_| "系统凭据库不可用")?;
        let password = entry.get_password().map_err(|_| "无法读取快照仓库凭据")?;
        let key = RecoveryKey {
            schema_version: 1,
            repository_id: id.clone(),
            password,
        };
        verified(&key, &repository_root(&id)?)?;
        Ok::<_, String>(key)
    })
    .await
    .map_err(|_| "无法读取恢复密钥")??;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导出恢复密钥（请妥善保密）")
        .add_filter("恢复密钥", &["json"])
        .set_file_name("cobble-world-recovery.json")
        .save_file()
        .await
    else {
        return activity.change(Ok(false));
    };
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || write_key(&path, &key))
        .await
        .map_err(|_| "恢复密钥导出任务意外退出")??;
    activity.change(Ok(true))
}

#[tauri::command]
pub async fn import_world_snapshot_recovery(
    window: WebviewWindow,
    app: AppHandle,
    game_id: String,
) -> Result<bool, String> {
    let activity = SnapshotActivity::new("recovery_import", &game_id);
    let (game, _operation) = management::begin(&app, &game_id, true)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导入恢复密钥")
        .add_filter("恢复密钥", &["json"])
        .pick_file()
        .await
    else {
        return activity.change(Ok(false));
    };
    let path = file.path().to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let key = read_key(&path, &game.id)?;
        let root = repository_root(&game.id)?;
        verified(&key, &root)?;
        let entry = keyring::Entry::new("CobbleLauncher.WorldSnapshots", &game.id)
            .map_err(|_| "系统凭据库不可用")?;
        if let Ok(existing) = entry.get_password()
            && WorldSnapshotRepository::open(&root, &existing).is_ok()
        {
            return Ok(false);
        }
        entry
            .set_password(&key.password)
            .map_err(|_| "无法保存快照仓库凭据")?;
        Ok(true)
    })
    .await
    .map_err(|_| "恢复密钥导入任务意外退出")?;
    activity.change(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    #[ignore = "Explicit Windows OS credential smoke test using a unique disposable namespace"]
    fn windows_vault_disposable_repository_roundtrip() {
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).unwrap_or_else(|_| panic!("random_generation_failed"));
        let unique: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let service = format!("CobbleLauncher.WorldSnapshots.Test.{unique}");
        let entry = keyring::Entry::new(&service, "disposable")
            .unwrap_or_else(|_| panic!("vault_entry_creation_failed"));
        assert!(
            matches!(entry.get_password(), Err(keyring::Error::NoEntry)),
            "disposable_namespace_not_empty"
        );
        struct OwnedCredential(keyring::Entry);
        impl Drop for OwnedCredential {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let owned = OwnedCredential(entry);
        getrandom::fill(&mut random).unwrap_or_else(|_| panic!("random_generation_failed"));
        let secret: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        owned
            .0
            .set_password(&secret)
            .unwrap_or_else(|_| panic!("vault_write_failed"));
        let recovered = owned
            .0
            .get_password()
            .unwrap_or_else(|_| panic!("vault_read_failed"));
        assert!(recovered == secret, "vault_roundtrip_mismatch");
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("repository");
        WorldSnapshotRepository::create(&root, &secret)
            .unwrap_or_else(|_| panic!("temporary_repository_create_failed"));
        assert!(
            WorldSnapshotRepository::open(&root, &recovered).is_ok(),
            "recovered_key_cannot_unlock_repository"
        );
        owned
            .0
            .delete_credential()
            .unwrap_or_else(|_| panic!("vault_cleanup_failed"));
        assert!(
            matches!(owned.0.get_password(), Err(keyring::Error::NoEntry)),
            "vault_cleanup_not_verified"
        );
    }

    #[test]
    fn malformed_or_wrong_repository_keys_never_echo_secret() {
        let secret = "ab".repeat(32);
        let valid = serde_json::to_vec(&RecoveryKey {
            schema_version: 1,
            repository_id: "game-a".into(),
            password: secret.clone(),
        })
        .unwrap();
        assert!(parse(&valid, "game-a").is_ok());
        let mismatch = parse(&valid, "game-b").err().unwrap();
        assert!(!mismatch.contains(&secret));
        for bytes in [
            format!("{{\"password\":\"{secret}\""),
            format!(
                "{{\"schema_version\":1,\"repository_id\":\"game-a\",\"password\":\"{secret}\",\"extra\":true}}"
            ),
            "x".repeat(MAX_RECOVERY_BYTES + 1),
        ] {
            let error = parse(bytes.as_bytes(), "game-a").err().unwrap();
            assert!(!error.contains(&secret));
        }
    }

    #[test]
    fn saved_keys_roundtrip_without_overwriting_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("key.json");
        let key = RecoveryKey {
            schema_version: 1,
            repository_id: "game-a".into(),
            password: "ab".repeat(32),
        };
        write_key(&path, &key).unwrap();
        assert_eq!(read_key(&path, "game-a").unwrap().password, key.password);
        let before = fs::read(&path).unwrap();
        assert!(write_key(&path, &key).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn wrong_password_cannot_verify_repository_and_error_is_sanitized() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("repository");
        WorldSnapshotRepository::create(&root, &"ab".repeat(32)).unwrap();
        let key = RecoveryKey {
            schema_version: 1,
            repository_id: "game-a".into(),
            password: "cd".repeat(32),
        };
        let error = verified(&key, &root).unwrap_err();
        assert!(!error.contains(&key.password));
        assert!(verified(&key, &directory.path().join("missing")).is_err());
    }
}

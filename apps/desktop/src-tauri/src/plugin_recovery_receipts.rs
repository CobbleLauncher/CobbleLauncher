// SPDX-License-Identifier: MIT
//! Native-only clean-exit receipts. Paths/arguments are never retained in receipts.
use super::RuntimeSummary;
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path, sync::Mutex};

static WRITES: Mutex<()> = Mutex::new(());
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Receipt {
    pub schema_version: u32,
    pub game_id: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub runtime: RuntimeSummary,
    #[serde(default)]
    pub loader: Option<super::LoaderSummary>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FailedAttempt {
    pub schema_version: u32,
    pub game_id: String,
    pub at: u64,
    pub runtime: Option<RuntimeSummary>,
    pub loader: super::LoaderSummary,
}
fn regular(path: &Path, directory: bool) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("RECOVERY_BASELINE_UNAVAILABLE".into()),
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("RECOVERY_BASELINE_UNAVAILABLE".into());
        }
    }
    if metadata.file_type().is_symlink()
        || (if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        })
    {
        return Err("RECOVERY_BASELINE_UNAVAILABLE".into());
    }
    Ok(true)
}
fn filename(game_id: &str) -> Result<String, String> {
    if game_id.is_empty() || game_id.len() > 128 || game_id.chars().any(char::is_control) {
        return Err("RECOVERY_GAME_SCOPE".into());
    }
    Ok(format!(
        "{}.json",
        cobble_plugin_host::digest(game_id.as_bytes())
    ))
}
pub(super) fn read_at(root: &Path, game_id: &str) -> Result<Option<Receipt>, String> {
    let name = filename(game_id)?;
    if !regular(root, true)? {
        return Ok(None);
    }
    let path = root.join(name);
    if !regular(&path, false)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    if bytes.len() > 4096 {
        return Err("RECOVERY_BASELINE_UNAVAILABLE".into());
    }
    let value: Receipt =
        serde_json::from_slice(&bytes).map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    if !matches!(value.schema_version, 1 | 2)
        || value.game_id != game_id
        || value.started_at == 0
        || value.ended_at < value.started_at
        || value.ended_at > 8_640_000_000_000_000
        || !value.runtime.valid()
        || value.loader.as_ref().is_some_and(|loader| !loader.valid())
    {
        return Err("RECOVERY_BASELINE_UNAVAILABLE".into());
    }
    Ok(Some(value))
}
pub(super) fn write_at(root: &Path, value: Receipt) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    let name = filename(&value.game_id)?;
    if value.started_at == 0
        || value.ended_at < value.started_at
        || value.ended_at > 8_640_000_000_000_000
        || !value.runtime.valid()
        || value.loader.as_ref().is_some_and(|loader| !loader.valid())
    {
        return Err("RECOVERY_BASELINE_INVALID".into());
    }
    if regular(root, true)? {
        regular(&root.join(&name), false)?;
    }
    if let Ok(Some(previous)) = read_at(root, &value.game_id)
        && (previous.started_at > value.started_at
            || previous.started_at == value.started_at && previous.ended_at > value.ended_at)
    {
        return Ok(());
    }
    if !regular(root, true)? {
        fs::create_dir(root).map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    }
    let bytes = serde_json::to_vec(&value).map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    if bytes.len() > 4096 {
        return Err("RECOVERY_BASELINE_INVALID".into());
    }
    let mut pending =
        tempfile::NamedTempFile::new_in(root).map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    use std::io::Write;
    pending
        .write_all(&bytes)
        .map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    pending
        .persist(root.join(name))
        .map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    Ok(())
}
pub(super) fn read_failed_at(root: &Path, game_id: &str) -> Result<Option<FailedAttempt>, String> {
    let name = format!("failed-{}", filename(game_id)?);
    if !regular(root, true)? || !regular(&root.join(&name), false)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(root.join(name))
        .map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    if bytes.len() > 4096 {
        return Err("RECOVERY_ATTEMPT_UNAVAILABLE".into());
    }
    let value: FailedAttempt =
        serde_json::from_slice(&bytes).map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    if value.schema_version != 1
        || value.game_id != game_id
        || value.at == 0
        || value.at > 8_640_000_000_000_000
        || value
            .runtime
            .as_ref()
            .is_some_and(|runtime| !runtime.valid())
        || !value.loader.valid()
    {
        return Err("RECOVERY_ATTEMPT_UNAVAILABLE".into());
    }
    Ok(Some(value))
}
pub(super) fn write_failed_at(root: &Path, value: FailedAttempt) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    if value.schema_version != 1
        || value.at == 0
        || value.at > 8_640_000_000_000_000
        || value
            .runtime
            .as_ref()
            .is_some_and(|runtime| !runtime.valid())
        || !value.loader.valid()
    {
        return Err("RECOVERY_ATTEMPT_INVALID".into());
    }
    let name = format!("failed-{}", filename(&value.game_id)?);
    if regular(root, true)? {
        regular(&root.join(&name), false)?;
    }
    if let Ok(Some(previous)) = read_failed_at(root, &value.game_id)
        && previous.at > value.at
    {
        return Ok(());
    }
    if !regular(root, true)? {
        fs::create_dir(root).map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    }
    let bytes = serde_json::to_vec(&value).map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    if bytes.len() > 4096 {
        return Err("RECOVERY_ATTEMPT_INVALID".into());
    }
    let mut pending =
        tempfile::NamedTempFile::new_in(root).map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    use std::io::Write;
    pending
        .write_all(&bytes)
        .map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    pending
        .persist(root.join(name))
        .map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    Ok(())
}
pub(super) fn clear_failed_at(root: &Path, game_id: &str, through: u64) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    let path = root.join(format!("failed-{}", filename(game_id)?));
    if read_failed_at(root, game_id)?.is_some_and(|attempt| attempt.at <= through)
        && regular(&path, false)?
    {
        fs::remove_file(path).map_err(|_| "RECOVERY_ATTEMPT_UNAVAILABLE")?;
    }
    Ok(())
}
pub(super) fn root() -> Result<std::path::PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("RECOVERY_BASELINE_UNAVAILABLE")?
        .join("CobbleLauncher/recovery-baselines"))
}
pub(super) fn prepare_root() -> Result<std::path::PathBuf, String> {
    let path = root()?;
    let base = path.parent().ok_or("RECOVERY_BASELINE_UNAVAILABLE")?;
    if !regular(base, true)? {
        fs::create_dir(base).map_err(|_| "RECOVERY_BASELINE_UNAVAILABLE")?;
    }
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn receipt(started: u64) -> Receipt {
        Receipt {
            schema_version: 1,
            game_id: "isolated-game".into(),
            started_at: started,
            ended_at: started + 10,
            runtime: RuntimeSummary {
                java_mode: "automatic".into(),
                selected_java_major: Some(21),
                memory_mb: 2048,
                jvm_argument_count: 0,
                settings_revision: "a".repeat(64),
            },
            loader: None,
        }
    }
    #[test]
    fn absent_baselines_are_unknown_and_older_completions_do_not_replace_newer() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("receipts");
        assert!(read_at(&root, "isolated-game").unwrap().is_none());
        write_at(&root, receipt(100)).unwrap();
        write_at(&root, receipt(50)).unwrap();
        assert_eq!(
            read_at(&root, "isolated-game").unwrap().unwrap().started_at,
            100
        );
        assert!(read_at(&root, "other-game").unwrap().is_none());
        write_at(&root, receipt(300)).unwrap();
        assert_eq!(
            read_at(&root, "isolated-game").unwrap().unwrap().started_at,
            300
        );
    }
    #[test]
    fn receipts_reject_wrong_identity_invalid_time_and_oversized_data() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("receipts");
        assert!(write_at(&root, receipt(0)).is_err());
        write_at(&root, receipt(100)).unwrap();
        let file = root.join(filename("isolated-game").unwrap());
        fs::write(&file, "x".repeat(4097)).unwrap();
        assert!(read_at(&root, "isolated-game").is_err());
        write_at(&root, receipt(200)).unwrap();
        assert_eq!(
            read_at(&root, "isolated-game").unwrap().unwrap().started_at,
            200
        );
        let mut wrong = receipt(100);
        wrong.game_id = "other-game".into();
        fs::write(&file, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(read_at(&root, "isolated-game").is_err());
    }
    #[test]
    fn failed_attempts_are_versioned_and_older_callbacks_cannot_replace_or_clear_newer_failures() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("receipts");
        let attempt = |at| FailedAttempt {
            schema_version: 1,
            game_id: "isolated-game".into(),
            at,
            runtime: Some(receipt(100).runtime),
            loader: super::super::loader_summary(
                "1.21.1".into(),
                Some("fabric".into()),
                Some("0.16".into()),
            ),
        };
        write_failed_at(&root, attempt(300)).unwrap();
        write_failed_at(&root, attempt(200)).unwrap();
        assert_eq!(
            read_failed_at(&root, "isolated-game").unwrap().unwrap().at,
            300
        );
        clear_failed_at(&root, "isolated-game", 250).unwrap();
        assert!(read_failed_at(&root, "isolated-game").unwrap().is_some());
        clear_failed_at(&root, "isolated-game", 300).unwrap();
        assert!(read_failed_at(&root, "isolated-game").unwrap().is_none());
    }
}

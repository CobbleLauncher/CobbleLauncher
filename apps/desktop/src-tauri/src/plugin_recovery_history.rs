// SPDX-License-Identifier: MIT
//! Bounded metadata adapter over the existing Mod operation receipts; no undo implementation.
use cap_std::fs::Dir;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Read, path::Path};
#[derive(Deserialize)]
struct StoredFile {
    name: String,
    before: Option<String>,
    after: Option<String>,
}
#[derive(Deserialize)]
struct StoredOperation {
    operation_id: String,
    created_at: u64,
    status: String,
    files: Vec<StoredFile>,
}
fn plain(dir: &Dir, name: &Path, folder: bool) -> Result<(), String> {
    let meta = dir
        .symlink_metadata(name)
        .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("RECOVERY_HISTORY_UNAVAILABLE".into());
        }
    }
    if meta.file_type().is_symlink()
        || (if folder {
            !meta.is_dir()
        } else {
            !meta.is_file()
        })
    {
        return Err("RECOVERY_HISTORY_UNAVAILABLE".into());
    }
    Ok(())
}
fn label(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect()
}
pub(super) fn read(directory: &Path) -> Result<Value, String> {
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())
        .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
    if !root
        .try_exists(".cobble-mod-backups")
        .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?
    {
        return Ok(json!({"available":true,"truncated":false,"items":[]}));
    }
    plain(&root, Path::new(".cobble-mod-backups"), true)?;
    let backups = root
        .open_dir(".cobble-mod-backups")
        .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
    let mut items = vec![];
    let mut seen = 0;
    let mut truncated = false;
    for entry in backups
        .entries()
        .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?
    {
        seen += 1;
        if seen > 128 {
            return Err("RECOVERY_HISTORY_LIMIT".into());
        }
        let name = entry
            .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?
            .file_name();
        let Some(id) = name.to_str() else { continue };
        if id.len() > 128
            || id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            continue;
        }
        plain(&backups, Path::new(id), true)?;
        let backup = backups
            .open_dir(id)
            .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
        if !backup
            .try_exists("history.json")
            .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?
        {
            continue;
        }
        plain(&backup, Path::new("history.json"), false)?;
        let mut bytes = vec![];
        backup
            .open("history.json")
            .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
        if bytes.len() > 65_536 {
            return Err("RECOVERY_HISTORY_LIMIT".into());
        }
        let value: StoredOperation =
            serde_json::from_slice(&bytes).map_err(|_| "RECOVERY_HISTORY_UNAVAILABLE")?;
        if value.operation_id != id || value.files.is_empty() {
            continue;
        }
        truncated |= value.files.len() > 64;
        let status = match value.status.as_str() {
            "applied" | "undone" | "undoing" | "installing" => value.status.as_str(),
            _ => "unknown",
        };
        items.push(json!({"id":id,"createdAt":value.created_at,"status":status,"files":value.files.iter().take(64).map(|file|json!({"name":label(&file.name),"before":file.before.as_deref().map(label),"after":file.after.as_deref().map(label)})).collect::<Vec<_>>()}));
    }
    items.sort_by_key(|item| std::cmp::Reverse(item["createdAt"].as_u64().unwrap_or_default()));
    truncated |= items.len() > 64;
    items.truncate(64);
    let mut files_left = 512;
    items.retain_mut(|item| {
        let files = item["files"]
            .as_array_mut()
            .expect("public operation files");
        if files_left == 0 {
            truncated = true;
            return false;
        }
        if files.len() > files_left {
            files.truncate(files_left);
            truncated = true;
        }
        files_left -= files.len();
        true
    });
    Ok(json!({"available":true,"truncated":truncated,"items":items}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_metadata_keeps_only_public_names_and_never_returns_internal_receipt_fields() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join(".cobble-mod-backups/install-test");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("history.json"), serde_json::to_vec(&json!({"operation_id":"install-test","created_at":100,"status":"applied","provider":"modrinth","originals":{"secret":"not-public"},"files":[{"name":"Example","before":"C:/private/old.jar","after":"new.jar"}]})).unwrap()).unwrap();
        let value = read(temp.path()).unwrap();
        assert_eq!(value["items"][0]["files"][0]["before"], "old.jar");
        assert!(!value.to_string().contains("not-public"));
        assert!(!value.to_string().contains("C:/private"));
    }
    #[test]
    fn large_retained_histories_keep_the_latest_records_with_a_total_file_budget() {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..10 {
            let id = format!("operation-{index}");
            let folder = temp.path().join(".cobble-mod-backups").join(&id);
            std::fs::create_dir_all(&folder).unwrap();
            let files = (0..64).map(|file| json!({"name":format!("Mod {file}"),"before":null,"after":format!("mod-{file}.jar")})).collect::<Vec<_>>();
            std::fs::write(folder.join("history.json"), serde_json::to_vec(&json!({"operation_id":id,"created_at":index+1,"status":"applied","files":files})).unwrap()).unwrap();
        }
        let value = read(temp.path()).unwrap();
        let items = value["items"].as_array().unwrap();
        assert_eq!(items[0]["createdAt"], 10);
        assert_eq!(value["truncated"], true);
        assert_eq!(
            items
                .iter()
                .map(|item| item["files"].as_array().unwrap().len())
                .sum::<usize>(),
            512
        );
    }
}

// SPDX-License-Identifier: MIT
//! Receiver-local, same-version keys/graphics only. No whole options.txt migration.
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
const MARKER: &str = ".cobble-personal-options.json";
const MAX_BYTES: usize = 16 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalOptions {
    pub schema_version: u8,
    pub source_version: String,
    pub target_version: String,
    pub options_version: u32,
    pub fields: BTreeMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct Capture {
    pub options: Option<PersonalOptions>,
    pub keys: usize,
    pub graphics: usize,
    pub skipped: usize,
    pub reason: Option<&'static str>,
    pub revision: String,
}
/// Shared with the existing initialized-target keybinding transfer.
pub fn modern_binding(value: &str) -> bool {
    value
        .strip_prefix("key.keyboard.")
        .or_else(|| value.strip_prefix("key.mouse."))
        .is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.len() <= 80
                && suffix.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._".contains(&byte)
                })
        })
}
/// Keep the legacy migration's duplicate handling and error code unchanged.
pub fn binding_values(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        if let Some((key, value)) = line.split_once(':')
            && key.starts_with("key_")
            && result.insert(key.into(), value.into()).is_some()
        {
            return Err("KEYBINDINGS_DUPLICATE".into());
        }
    }
    Ok(result)
}
fn release(value: &str) -> bool {
    if value.len() > 32 {
        return false;
    }
    let parts = value.split('.').collect::<Vec<_>>();
    matches!(parts.len(), 2 | 3)
        && parts[0] == "1"
        && parts[1].parse::<u32>().is_ok_and(|minor| minor >= 13)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
fn vanilla_key(key: &str) -> bool {
    matches!(
        key,
        "key_key.attack"
            | "key_key.use"
            | "key_key.forward"
            | "key_key.left"
            | "key_key.back"
            | "key_key.right"
            | "key_key.jump"
            | "key_key.sneak"
            | "key_key.sprint"
            | "key_key.drop"
            | "key_key.inventory"
            | "key_key.chat"
            | "key_key.playerlist"
            | "key_key.command"
            | "key_key.screenshot"
            | "key_key.togglePerspective"
            | "key_key.smoothCamera"
            | "key_key.fullscreen"
            | "key_key.spectatorOutlines"
            | "key_key.swapOffhand"
            | "key_key.advancements"
            | "key_key.loadToolbarActivator"
            | "key_key.saveToolbarActivator"
    ) || key
        .strip_prefix("key_key.hotbar.")
        .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}
fn graphic_key(key: &str) -> bool {
    matches!(
        key,
        "fullscreen"
            | "fov"
            | "renderDistance"
            | "simulationDistance"
            | "guiScale"
            | "maxFps"
            | "vsync"
            | "enableVsync"
    )
}
fn valid_field(key: &str, value: &str) -> bool {
    if vanilla_key(key) {
        return modern_binding(value);
    }
    match key {
        "fullscreen" | "vsync" | "enableVsync" => matches!(value, "true" | "false"),
        "fov" => value
            .parse::<f64>()
            .is_ok_and(|number| number.is_finite() && (-1.0..=1.0).contains(&number)),
        "renderDistance" => value
            .parse::<u32>()
            .is_ok_and(|number| (2..=32).contains(&number)),
        "simulationDistance" => value
            .parse::<u32>()
            .is_ok_and(|number| (5..=32).contains(&number)),
        "guiScale" => value.parse::<u32>().is_ok_and(|number| number <= 8),
        "maxFps" => value
            .parse::<u32>()
            .is_ok_and(|number| (10..=260).contains(&number)),
        _ => false,
    }
}
pub fn validate(plan: &PersonalOptions) -> Result<(), String> {
    if plan.schema_version != 1
        || plan.source_version != plan.target_version
        || !release(&plan.target_version)
        || plan.options_version == 0
        || plan.options_version > i32::MAX as u32
        || plan.fields.is_empty()
        || plan.fields.len() > 64
        || plan
            .fields
            .iter()
            .any(|(key, value)| !valid_field(key, value))
        || serde_json::to_vec(plan)
            .map_err(|_| "PERSONAL_OPTIONS_INVALID")?
            .len()
            > MAX_BYTES
    {
        return Err("PERSONAL_OPTIONS_INVALID".into());
    }
    Ok(())
}
pub fn validate_target(plan: &PersonalOptions, target_version: &str) -> Result<(), String> {
    validate(plan)?;
    if plan.target_version != target_version {
        return Err("PERSONAL_OPTIONS_TARGET_CHANGED".into());
    }
    Ok(())
}
fn encoded(plan: &PersonalOptions) -> Result<Vec<u8>, String> {
    validate(plan)?;
    let mut text = format!("version:{}\n", plan.options_version);
    for (key, value) in &plan.fields {
        text.push_str(key);
        text.push(':');
        text.push_str(value);
        text.push('\n');
    }
    if text.len() > MAX_BYTES {
        return Err("PERSONAL_OPTIONS_LIMIT".into());
    }
    Ok(text.into_bytes())
}
pub fn encoded_len(plan: &PersonalOptions) -> Result<u64, String> {
    Ok(encoded(plan)?.len() as u64)
}
pub fn capture_text(
    source_version: &str,
    target_version: &str,
    text: &str,
) -> Result<Capture, String> {
    if text.len() > 1024 * 1024 || text.contains('\0') {
        return Err("PERSONAL_OPTIONS_INVALID".into());
    }
    let bindings = binding_values(text)?;
    let mut selected = BTreeMap::new();
    let mut options_version = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key == "version" {
            if options_version
                .replace(
                    value
                        .parse::<u32>()
                        .map_err(|_| "PERSONAL_OPTIONS_INVALID")?,
                )
                .is_some()
            {
                return Err("PERSONAL_OPTIONS_DUPLICATE".into());
            }
        } else if graphic_key(key) && selected.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err("PERSONAL_OPTIONS_DUPLICATE".into());
        }
    }
    let total = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.starts_with("key_") || graphic_key(key))
        .count();
    let mut result = Capture {
        options: None,
        keys: 0,
        graphics: 0,
        skipped: total,
        reason: None,
        revision: format!("{:x}", Sha256::digest(text.as_bytes())),
    };
    if source_version != target_version {
        result.reason = Some("differentVersion");
        return Ok(result);
    }
    if !release(target_version)
        || options_version.is_none_or(|version| version == 0 || version > i32::MAX as u32)
    {
        result.reason = Some("unsupportedFormat");
        return Ok(result);
    }
    let mut fields = BTreeMap::new();
    for (key, value) in bindings {
        if valid_field(&key, &value) {
            result.keys += 1;
            fields.insert(key, value);
        }
    }
    for (key, value) in selected {
        if valid_field(&key, &value) {
            result.graphics += 1;
            fields.insert(key, value);
        }
    }
    result.skipped = total - result.keys - result.graphics;
    if fields.is_empty() {
        result.reason = Some("noCompatible");
        return Ok(result);
    }
    let plan = PersonalOptions {
        schema_version: 1,
        source_version: source_version.into(),
        target_version: target_version.into(),
        options_version: options_version.unwrap_or_default(),
        fields,
    };
    validate(&plan)?;
    result.options = Some(plan);
    Ok(result)
}
fn root(directory: &Path) -> Result<Dir, String> {
    let metadata =
        std::fs::symlink_metadata(directory).map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("PERSONAL_OPTIONS_LINK_DENIED".into());
        }
    }
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("PERSONAL_OPTIONS_LINK_DENIED".into());
    }
    Dir::open_ambient_dir(directory, cap_std::ambient_authority())
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE".into())
}
fn bytes(root: &Dir, name: &str, limit: u64) -> Result<Option<Vec<u8>>, String> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = match root.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("PERSONAL_OPTIONS_UNAVAILABLE".into()),
    };
    if !file
        .metadata()
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?
        .is_file()
    {
        return Err("PERSONAL_OPTIONS_UNAVAILABLE".into());
    }
    let mut bytes = vec![];
    std::io::Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    if bytes.len() as u64 > limit {
        return Err("PERSONAL_OPTIONS_LIMIT".into());
    }
    Ok(Some(bytes))
}
pub fn capture(
    directory: &Path,
    source_version: &str,
    target_version: &str,
) -> Result<Capture, String> {
    match bytes(&root(directory)?, "options.txt", 1024 * 1024)? {
        Some(bytes) => capture_text(
            source_version,
            target_version,
            std::str::from_utf8(&bytes).map_err(|_| "PERSONAL_OPTIONS_INVALID")?,
        ),
        None => Ok(Capture {
            options: None,
            keys: 0,
            graphics: 0,
            skipped: 0,
            reason: Some("missingOptions"),
            revision: "missing".into(),
        }),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u8,
    identity: String,
    complete: bool,
}
fn read_receipt(root: &Dir) -> Result<Option<Receipt>, String> {
    let Some(raw) = bytes(root, MARKER, 4096)? else {
        return Ok(None);
    };
    let receipt: Receipt =
        serde_json::from_slice(&raw).map_err(|_| "PERSONAL_OPTIONS_RECEIPT_INVALID")?;
    if receipt.schema_version != 1 || receipt.identity.len() != 64 {
        return Err("PERSONAL_OPTIONS_RECEIPT_INVALID".into());
    }
    Ok(Some(receipt))
}
fn write_receipt(root: &Dir, identity: &str, complete: bool) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root
        .open_with(format!("{MARKER}.tmp"), &options)
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    serde_json::to_writer(
        &mut file,
        &Receipt {
            schema_version: 1,
            identity: identity.into(),
            complete,
        },
    )
    .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    file.sync_all()
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    drop(file);
    root.rename(format!("{MARKER}.tmp"), root, MARKER)
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE".into())
}
pub fn ensure_complete(directory: &Path) -> Result<(), String> {
    let directory = Dir::open_ambient_dir(directory, cap_std::ambient_authority())
        .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    if read_receipt(&directory)?.is_some_and(|receipt| !receipt.complete) {
        return Err("PERSONAL_OPTIONS_INCOMPLETE".into());
    }
    Ok(())
}
pub fn install_controlled(
    directory: &Path,
    plan: &PersonalOptions,
    control: crate::OperationControl,
) -> Result<(), String> {
    let expected = encoded(plan)?;
    control
        .check_cancelled()
        .map_err(|error| error.to_string())?;
    let root = root(directory)?;
    let identity = format!("{:x}", Sha256::digest(&expected));
    let previous = read_receipt(&root)?;
    if previous
        .as_ref()
        .is_some_and(|receipt| receipt.identity != identity)
    {
        return Err("PERSONAL_OPTIONS_PLAN_CHANGED".into());
    }
    let existing = bytes(&root, "options.txt", MAX_BYTES as u64)?;
    if existing.as_ref().is_some_and(|bytes| bytes != &expected) {
        return Err("PERSONAL_OPTIONS_CONFLICT".into());
    }
    let temporary = format!(".cobble-personal-{}.tmp", &identity[..16]);
    if bytes(&root, &temporary, MAX_BYTES as u64)?.is_some()
        && previous.as_ref().is_none_or(|receipt| receipt.complete)
    {
        return Err("PERSONAL_OPTIONS_CONFLICT".into());
    }
    write_receipt(&root, &identity, false)?;
    if existing.is_none() {
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create(true)
            .truncate(true)
            .follow(FollowSymlinks::No);
        let mut file = root
            .open_with(&temporary, &options)
            .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
        file.write_all(&expected)
            .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
        file.sync_all()
            .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
        drop(file);
        (control.progress())(crate::InstallProgress {
            detail: None,
            phase: "personal-options".into(),
            completed: 0,
            total: 1,
            reused: 0,
        });
        control
            .check_cancelled()
            .map_err(|error| error.to_string())?;
        if bytes(&root, "options.txt", MAX_BYTES as u64)?.is_some() {
            return Err("PERSONAL_OPTIONS_CONFLICT".into());
        }
        root.rename(&temporary, &root, "options.txt")
            .map_err(|_| "PERSONAL_OPTIONS_UNAVAILABLE")?;
    }
    write_receipt(&root, &identity, true)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_only_same_version_supported_local_fields() {
        let source = "version:3955\nkey_key.forward:key.keyboard.up\nkey_key.attack:key.mouse.right\nkey_mod.unknown:key.keyboard.a\nkey_key.jump:57\nrenderDistance:12\nfov:0.5\nvsync:true\nlastServer:private.example\nresourcePacks:[private]\n";
        let captured = capture_text("1.21.1", "1.21.1", source).unwrap();
        assert_eq!(
            (captured.keys, captured.graphics, captured.skipped),
            (2, 3, 2)
        );
        let bytes = encoded(captured.options.as_ref().unwrap()).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("private"));
        assert!(!text.contains("key_mod"));
        assert!(text.contains("version:3955"));
        let vsync = capture_text(
            "1.21.1",
            "1.21.1",
            "version:3955\nenableVsync:false\nvsync:true\n",
        )
        .unwrap()
        .options
        .unwrap();
        assert_eq!(vsync.fields["enableVsync"], "false");
        assert_eq!(vsync.fields["vsync"], "true");
        assert!(!valid_field("enableVsync", "1"));
        assert!(
            capture_text("1.20.1", "1.21.1", source)
                .unwrap()
                .options
                .is_none()
        );
        assert!(
            capture_text("snapshot", "snapshot", source)
                .unwrap()
                .options
                .is_none()
        );
        assert!(
            capture_text("1.21.4", "1.21.4", source)
                .unwrap()
                .options
                .is_some()
        );
    }
    #[test]
    fn duplicate_and_unknown_or_invalid_fields_fail_closed() {
        assert!(binding_values("key_key.jump:a\nkey_key.jump:b").is_err());
        assert!(!modern_binding("57"));
        let mut plan = capture_text(
            "1.21.1",
            "1.21.1",
            "version:3955\nkey_key.jump:key.keyboard.space\n",
        )
        .unwrap()
        .options
        .unwrap();
        plan.fields.insert("lastServer".into(), "private".into());
        assert!(validate(&plan).is_err());
        assert!(
            capture_text("1.21.1", "1.21.1", "key_key.jump:key.keyboard.space\n")
                .unwrap()
                .options
                .is_none()
        );
    }
    #[test]
    fn cancelled_target_write_blocks_launch_then_retries_without_touching_source_or_player_edits() {
        let directory = std::env::temp_dir().join(format!(
            "cobble-personal-options-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("source");
        let target = directory.join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        let source_text=b"version:3955\nkey_key.forward:key.keyboard.up\nrenderDistance:10\nlastServer:private\n";
        std::fs::write(source.join("options.txt"), source_text).unwrap();
        let plan = capture(&source, "1.21.1", "1.21.1")
            .unwrap()
            .options
            .unwrap();
        let holder = std::sync::Arc::new(std::sync::Mutex::new(None::<crate::OperationControl>));
        let cloned = holder.clone();
        let control = crate::OperationControl::new(std::sync::Arc::new(move |_| {
            cloned.lock().unwrap().as_ref().unwrap().cancel()
        }));
        *holder.lock().unwrap() = Some(control.clone());
        assert!(install_controlled(&target, &plan, control).is_err());
        assert!(ensure_complete(&target).is_err());
        assert!(crate::environment_install::ensure_environment_complete(&target).is_err());
        install_controlled(
            &target,
            &plan,
            crate::OperationControl::new(std::sync::Arc::new(|_| {})),
        )
        .unwrap();
        ensure_complete(&target).unwrap();
        assert_eq!(
            std::fs::read(source.join("options.txt")).unwrap(),
            source_text
        );
        assert!(
            !String::from_utf8(std::fs::read(target.join("options.txt")).unwrap())
                .unwrap()
                .contains("private")
        );
        std::fs::write(target.join("options.txt"), b"player edit").unwrap();
        assert!(
            install_controlled(
                &target,
                &plan,
                crate::OperationControl::new(std::sync::Arc::new(|_| {}))
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(target.join("options.txt")).unwrap(),
            b"player edit"
        );
        let canonical = std::fs::canonicalize(&directory).unwrap();
        assert_eq!(
            canonical.parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(canonical).unwrap();
    }
}

// SPDX-License-Identifier: MIT
//! Read-only comparison against a retained, hash-bound original MRPACK.
use super::*;
use cap_std::fs::Dir;
use cobble_content::pack_update_preview::{
    self as domain, ObservedPackFile, PackManifestSnapshot, PackUpdatePreview,
};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const ARCHIVE_LIMIT: u64 = 512 * 1024 * 1024;
const FILE_LIMIT: u64 = 64 * 1024 * 1024;
const OBSERVATION_LIMIT: u64 = 512 * 1024 * 1024;
const FILE_COUNT: usize = 4096;
#[derive(Deserialize)]
struct Marker {
    name: String,
    version_id: String,
    archive_sha256: String,
    selected_optional: Vec<String>,
    complete: bool,
}
#[derive(Serialize)]
pub struct Context {
    supported: bool,
    reason: Option<&'static str>,
    provider: Option<&'static str>,
    name: Option<String>,
    installed_version: Option<String>,
    online_versions: bool,
    identity_relation: &'static str,
}
#[derive(Serialize)]
pub struct Preview {
    #[serde(flatten)]
    plan: PackUpdatePreview,
    target_name: String,
    excluded_optional_files: usize,
    observed_file_count: usize,
    unobserved_file_count: usize,
}
fn io_error() -> String {
    "PACK_UPDATE_IO".into()
}
fn game(app: &AppHandle, id: &str) -> Result<Game, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| io_error())?;
    if library.load_error.is_some() {
        return Err(io_error());
    }
    library
        .games
        .iter()
        .find(|g| g.id == id && g.status == "installed")
        .cloned()
        .ok_or("PACK_UPDATE_UNSUPPORTED".into())
}
fn plain_file(path: &Path, limit: u64) -> Result<fs::File, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| io_error())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io_error());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io_error());
        }
    }
    if metadata.len() > limit {
        return Err("PACK_UPDATE_ARCHIVE_TOO_LARGE".into());
    }
    fs::File::open(path).map_err(|_| io_error())
}
fn marker(game: &Game) -> Result<Marker, String> {
    let path = game.directory.join(".cobble-pack.json");
    let file = plain_file(&path, 64 * 1024)?;
    let marker: Marker =
        serde_json::from_reader(file.take(64 * 1024 + 1)).map_err(|_| "PACK_UPDATE_UNSUPPORTED")?;
    if !marker.complete
        || marker.archive_sha256.len() != 64
        || !marker.archive_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("PACK_UPDATE_UNSUPPORTED".into());
    }
    Ok(marker)
}
fn context(game: &Game) -> Context {
    let mut context = Context {
        supported: false,
        reason: Some("not_managed_pack"),
        provider: None,
        name: game.pack_name.clone(),
        installed_version: game.pack_version.clone(),
        online_versions: false,
        identity_relation: "unknown",
    };
    let Some(id) = &game.pack_source_id else {
        return context;
    };
    let Ok(archive) = pack_archive_path(id) else {
        context.reason = Some("source_unavailable");
        return context;
    };
    if archive.with_extension("curseforge.json").exists() {
        context.reason = Some("unsupported_format");
        return context;
    }
    let Ok(marker) = marker(game) else {
        context.reason = Some("incomplete_pack");
        return context;
    };
    match plain_file(&archive, ARCHIVE_LIMIT) {
        Ok(_) => {}
        Err(error) => {
            context.reason = Some(if error == "PACK_UPDATE_ARCHIVE_TOO_LARGE" {
                "archive_too_large"
            } else {
                "source_unavailable"
            });
            return context;
        }
    }
    context.supported = true;
    context.reason = None;
    context.provider = Some("mrpack");
    context.name = Some(marker.name);
    context.installed_version = Some(marker.version_id);
    context
}
fn archive(
    path: &Path,
    expected: Option<&str>,
    previous_optional: &[String],
) -> Result<(PackManifestSnapshot, String, usize), String> {
    let mut file = plain_file(path, ARCHIVE_LIMIT)?;
    if let Some(expected) = expected {
        let mut digest = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let mut read = 0;
        loop {
            let count = file.read(&mut buffer).map_err(|_| io_error())?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > ARCHIVE_LIMIT {
                return Err("PACK_UPDATE_ARCHIVE_TOO_LARGE".into());
            }
            digest.update(&buffer[..count]);
        }
        if !format!("{:x}", digest.finalize()).eq_ignore_ascii_case(expected) {
            return Err("PACK_UPDATE_SOURCE_CHANGED".into());
        }
        file.seek(SeekFrom::Start(0)).map_err(|_| io_error())?;
    }
    let pack = cobble_content::read_mrpack_file(file.try_clone().map_err(|_| io_error())?)
        .map_err(|_| "PACK_UPDATE_INVALID_ARCHIVE")?;
    if pack.files.len() + pack.overrides.len() > FILE_COUNT {
        return Err("PACK_UPDATE_TOO_MANY_FILES".into());
    }
    let selected: Vec<_> = pack
        .files
        .iter()
        .filter(|f| f.optional && previous_optional.contains(&f.path))
        .map(|f| f.path.clone())
        .collect();
    let excluded = pack.files.iter().filter(|f| f.optional).count() - selected.len();
    file.seek(SeekFrom::Start(0)).map_err(|_| io_error())?;
    let snapshot =
        domain::snapshot_mrpack_file(file, &selected).map_err(|_| "PACK_UPDATE_INVALID_ARCHIVE")?;
    Ok((snapshot, pack.name, excluded))
}
fn observation(root: &Dir, path: &str, budget: &mut u64) -> Option<ObservedPackFile> {
    if path
        .split('/')
        .next()
        .is_some_and(|part| part.eq_ignore_ascii_case("saves"))
    {
        return None;
    }
    let metadata = match root.symlink_metadata(path) {
        Ok(m) => m,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A user-disabled Mod is not declared missing; preserve its unknown path state.
            if path.starts_with("mods/")
                && root.try_exists(format!("{path}.disabled")).ok() == Some(true)
            {
                return None;
            }
            return Some(ObservedPackFile {
                path: path.into(),
                exists: false,
                size_bytes: 0,
                sha1: None,
                sha512: None,
            });
        }
        Err(_) => return None,
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > FILE_LIMIT
        || metadata.len() > *budget
    {
        return None;
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return None;
        }
    }
    // Reject linked parent components rather than reading another instance-owned subtree.
    let mut parent = std::path::PathBuf::new();
    let parts: Vec<_> = Path::new(path).components().collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        parent.push(part.as_os_str());
        let metadata = root.symlink_metadata(&parent).ok()?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return None;
        }
        #[cfg(windows)]
        {
            use cap_std::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return None;
            }
        }
    }
    let mut file = root.open(path).ok()?;
    let before = file.metadata().ok()?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut buffer = [0; 64 * 1024];
    let mut read = 0;
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        read += count as u64;
        *budget = budget.saturating_sub(count as u64);
        if read > FILE_LIMIT || read > before.len() {
            return None;
        }
        sha1.update(&buffer[..count]);
        sha512.update(&buffer[..count]);
    }
    let after = file.metadata().ok()?;
    if read != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return None;
    }
    Some(ObservedPackFile {
        path: path.into(),
        exists: true,
        size_bytes: read,
        sha1: Some(format!("{:x}", sha1.finalize())),
        sha512: Some(format!("{:x}", sha512.finalize())),
    })
}
fn compare(game: &Game, target: &Path) -> Result<Preview, String> {
    if !context(game).supported {
        return Err("PACK_UPDATE_UNSUPPORTED".into());
    }
    let marker = marker(game)?;
    let source = pack_archive_path(
        game.pack_source_id
            .as_deref()
            .ok_or("PACK_UPDATE_UNSUPPORTED")?,
    )?;
    let (installed, _, _) = archive(
        &source,
        Some(&marker.archive_sha256),
        &marker.selected_optional,
    )?;
    let (target, target_name, excluded_optional_files) =
        archive(target, None, &marker.selected_optional)?;
    let paths: BTreeSet<_> = installed
        .files
        .iter()
        .chain(&target.files)
        .map(|f| f.path.as_str())
        .collect();
    let root = Dir::open_ambient_dir(&game.directory, cap_std::ambient_authority())
        .map_err(|_| io_error())?;
    let mut budget = OBSERVATION_LIMIT;
    let observed: Vec<_> = paths
        .iter()
        .filter_map(|path| observation(&root, path, &mut budget))
        .collect();
    let unobserved_file_count = paths.len() - observed.len();
    let observed_file_count = observed.len();
    let plan = domain::preview_pack_update(Some(&installed), &target, &observed)
        .map_err(|_| "PACK_UPDATE_INVALID_ARCHIVE")?;
    Ok(Preview {
        plan,
        target_name,
        excluded_optional_files,
        observed_file_count,
        unobserved_file_count,
    })
}
#[tauri::command]
pub async fn get_game_pack_update_context(
    app: AppHandle,
    game_id: String,
) -> Result<Context, String> {
    let game = game(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || Ok(context(&game)))
        .await
        .map_err(|_| io_error())?
}
#[tauri::command]
pub async fn preview_game_pack_update(
    window: WebviewWindow,
    app: AppHandle,
    game_id: String,
) -> Result<Option<Preview>, String> {
    let game = game(&app, &game_id)?;
    if !context(&game).supported {
        return Err("PACK_UPDATE_UNSUPPORTED".into());
    }
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter("Modrinth pack", &["mrpack"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let target = file.path().to_owned();
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || compare(&game, &target))
        .await
        .map_err(|_| io_error())?;
    tracing::info!(
        event = "pack_update_preview_finished",
        outcome = if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
        duration_ms = started.elapsed().as_millis() as u64
    );
    result.map(Some)
}
#[cfg(test)]
#[path = "native_pack_update_preview_tests.rs"]
mod tests;

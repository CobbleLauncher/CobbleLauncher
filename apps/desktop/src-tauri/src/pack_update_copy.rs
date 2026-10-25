// SPDX-License-Identifier: MIT
//! Confirmed local MRPACK update into an independent, unpublished game directory.
use super::*;
#[path = "pack_update_json.rs"]
pub(super) mod json;
use cobble_content::pack_update_preview::{PackLocalState, PackUpdatePreview};
use cobble_core::storage_workspace as disk;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{Read, Write},
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default, Serialize)]
pub struct CopyPreview {
    plan_id: String,
    player_added: Vec<String>,
    migratable_paths: Vec<String>,
    json_configs: Vec<cobble_content::pack_update_preview::JsonConfigPreview>,
}
#[derive(Clone)]
struct Plan {
    game: Game,
    preview: PackUpdatePreview,
    archive: PathBuf,
    archive_hash: String,
    preview_hash: String,
    candidate: BTreeMap<String, disk::FileStamp>,
    player_added: Vec<String>,
    json_documents: BTreeMap<String, cobble_content::pack_update_preview::JsonMergeDocument>,
    created: Instant,
}
static PLANS: OnceLock<Mutex<HashMap<String, Plan>>> = OnceLock::new();
fn plans() -> &'static Mutex<HashMap<String, Plan>> {
    PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn code() -> String {
    "PACK_UPDATE_COPY_FAILED".into()
}
fn nonce() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| code())?;
    Ok(b.iter().map(|b| format!("{b:02x}")).collect())
}
fn hash(path: &Path) -> Result<String, String> {
    let m = fs::symlink_metadata(path).map_err(|_| code())?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > 512 * 1024 * 1024 {
        return Err("PACK_UPDATE_SOURCE_CHANGED".into());
    }
    let mut f = fs::File::open(path).map_err(|_| code())?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(|_| code())?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn preview_hash(preview: &PackUpdatePreview) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(preview).map_err(|_| code())?)
    ))
}
fn managed_paths(preview: &PackUpdatePreview) -> BTreeSet<String> {
    [
        &preview.added,
        &preview.removed,
        &preview.version_changed,
        &preview.unchanged,
    ]
    .into_iter()
    .flatten()
    .flat_map(|c| [c.before.as_ref(), c.after.as_ref()])
    .flatten()
    .map(|f| f.path.clone())
    .collect()
}
pub(super) fn local_candidates(
    game: &Game,
    preview: &PackUpdatePreview,
) -> Result<(BTreeMap<String, disk::FileStamp>, Vec<String>), String> {
    let cancel = AtomicBool::new(false);
    let managed = managed_paths(preview);
    let mut result = BTreeMap::new();
    let mut added = Vec::new();
    for top in [
        "mods",
        "config",
        "defaultconfigs",
        "resourcepacks",
        "shaderpacks",
        "kubejs",
        "scripts",
    ] {
        let path = game.directory.join(top);
        if !path.exists() {
            continue;
        }
        let inventory =
            disk::inventory(&path, &cancel).map_err(|_| "PACK_UPDATE_LOCAL_UNREADABLE")?;
        for (relative, stamp) in inventory.files {
            let name = format!("{top}/{}", relative.to_string_lossy().replace('\\', "/"));
            if !managed.contains(&name) {
                added.push(name.clone());
            }
            result.insert(name, stamp);
        }
    }
    added.sort();
    if result.len() > 10_000 {
        return Err("PACK_UPDATE_LOCAL_TOO_LARGE".into());
    }
    Ok((result, added))
}
pub(super) fn store_plan(
    game: &Game,
    archive: &Path,
    preview: &PackUpdatePreview,
    json_documents: &[json::PreparedJson],
) -> Result<CopyPreview, String> {
    let (candidate, player_added) = local_candidates(game, preview)?;
    let json_documents: BTreeMap<_, _> = json_documents
        .iter()
        .filter(|d| {
            candidate
                .get(&d.document.preview.path)
                .is_some_and(|stamp| stamp.sha256 == d.local_sha256)
        })
        .map(|d| (d.document.preview.path.clone(), d.document.clone()))
        .collect();
    let json_configs = json::previews(&json_documents);
    let id = nonce()?;
    let plan = Plan {
        game: game.clone(),
        preview: preview.clone(),
        archive: archive.to_owned(),
        archive_hash: hash(archive)?,
        preview_hash: preview_hash(preview)?,
        candidate,
        player_added: player_added.clone(),
        json_documents,
        created: Instant::now(),
    };
    let mut store = plans().lock().map_err(|_| code())?;
    store.retain(|_, p| p.created.elapsed() < Duration::from_secs(300));
    if store.len() >= 16 {
        return Err("PACK_UPDATE_BUSY".into());
    }
    let migratable_paths = plan.candidate.keys().cloned().collect();
    store.insert(id.clone(), plan);
    Ok(CopyPreview {
        plan_id: id,
        player_added,
        migratable_paths,
        json_configs,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyRequest {
    plan_id: String,
    name: String,
    preserve_paths: Vec<String>,
    #[serde(default)]
    replace_new_mods: Vec<ModReplacement>,
    #[serde(default)]
    include_worlds: bool,
    #[serde(default)]
    json_merges: Vec<json::MergeRequest>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModReplacement {
    old_path: String,
    new_path: Option<String>,
}
fn replacement_map(
    preview: &PackUpdatePreview,
    selected: &BTreeSet<String>,
    requested: &[ModReplacement],
    player_added: &[String],
) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut old: BTreeSet<String> = preview
        .removed
        .iter()
        .filter_map(|c| c.before.as_ref())
        .map(|f| &f.path)
        .filter(|p| p.starts_with("mods/") && p.ends_with(".jar") && selected.contains(*p))
        .cloned()
        .collect();
    old.extend(
        player_added
            .iter()
            .filter(|p| p.starts_with("mods/") && p.ends_with(".jar") && selected.contains(*p))
            .cloned(),
    );
    let new: BTreeSet<String> = preview
        .added
        .iter()
        .filter_map(|c| c.after.as_ref())
        .map(|f| &f.path)
        .filter(|p| p.starts_with("mods/") && p.ends_with(".jar"))
        .cloned()
        .collect();
    let mut result = BTreeMap::new();
    let mut claimed = BTreeSet::new();
    for entry in requested {
        if !old.contains(&entry.old_path)
            || result.contains_key(&entry.old_path)
            || entry
                .new_path
                .as_ref()
                .is_some_and(|p| !new.contains(p) || !claimed.insert(p.clone()))
        {
            return Err("PACK_UPDATE_SELECTION_INVALID".into());
        }
        result.insert(entry.old_path.clone(), entry.new_path.clone());
    }
    if !new.is_empty() && result.len() != old.len() {
        return Err("PACK_UPDATE_MOD_CHOICE_REQUIRED".into());
    }
    Ok(result)
}
fn requested_paths(
    plan: &Plan,
    request: &CopyRequest,
    preview: &PackUpdatePreview,
) -> Result<BTreeSet<String>, String> {
    let mut allowed: BTreeSet<String> = plan.player_added.iter().cloned().collect();
    for c in [
        &preview.added,
        &preview.removed,
        &preview.version_changed,
        &preview.unchanged,
    ]
    .into_iter()
    .flatten()
    {
        if c.local_state != PackLocalState::Clean && c.local_state != PackLocalState::Missing {
            if let Some(before) = &c.before {
                allowed.insert(before.path.clone());
            }
            if let Some(after) = &c.after {
                allowed.insert(after.path.clone());
            }
        }
    }
    let requested: BTreeSet<String> = request.preserve_paths.iter().cloned().collect();
    if requested.len() != request.preserve_paths.len()
        || !requested.is_subset(&allowed)
        || !requested.iter().all(|p| plan.candidate.contains_key(p))
    {
        return Err("PACK_UPDATE_SELECTION_INVALID".into());
    }
    Ok(requested)
}
fn check_plan(plan: &Plan) -> Result<PackUpdatePreview, String> {
    if hash(&plan.archive)? != plan.archive_hash {
        return Err("PACK_UPDATE_STALE".into());
    }
    let current = super::pack_update_preview::compare(&plan.game, &plan.archive)?;
    if preview_hash(&current.plan)? != plan.preview_hash {
        return Err("PACK_UPDATE_STALE".into());
    }
    let (files, added) = local_candidates(&plan.game, &current.plan)?;
    if files != plan.candidate || added != plan.player_added {
        return Err("PACK_UPDATE_STALE".into());
    }
    Ok(current.plan)
}
fn copy_local(
    source: &Path,
    target: &Path,
    relative: &str,
    stamp: &disk::FileStamp,
    cancel: &AtomicBool,
) -> Result<(), String> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    use cap_std::fs::{Dir, OpenOptions};
    cobble_store::SafeRelativePath::parse(relative).map_err(|_| code())?;
    for root in [source, target] {
        let meta = fs::symlink_metadata(root).map_err(|_| code())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
    }
    let src = Dir::open_ambient_dir(source, cap_std::ambient_authority()).map_err(|_| code())?;
    let dst = Dir::open_ambient_dir(target, cap_std::ambient_authority()).map_err(|_| code())?;
    let mut parent = PathBuf::new();
    for part in Path::new(relative).parent().ok_or_else(code)?.components() {
        parent.push(part.as_os_str());
        let meta = src
            .symlink_metadata(&parent)
            .map_err(|_| "PACK_UPDATE_STALE")?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
        if !dst.try_exists(&parent).map_err(|_| code())? {
            dst.create_dir(&parent).map_err(|_| code())?;
        }
        let meta = dst.symlink_metadata(&parent).map_err(|_| code())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
    }
    let mut read = OpenOptions::new();
    read.read(true).follow(FollowSymlinks::No);
    let mut input = src
        .open_with(relative, &read)
        .map_err(|_| "PACK_UPDATE_STALE")?;
    let meta = input.metadata().map_err(|_| code())?;
    if !meta.is_file() || meta.len() != stamp.bytes {
        return Err("PACK_UPDATE_STALE".into());
    }
    let temp = format!("{relative}.cobble-update-tmp");
    let mut write = OpenOptions::new();
    write
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut output = dst.open_with(&temp, &write).map_err(|_| code())?;
    let mut h = Sha256::new();
    let mut copied = 0u64;
    let mut b = [0; 65536];
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err("PACK_UPDATE_CANCELLED".into());
        }
        let n = input.read(&mut b).map_err(|_| code())?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > stamp.bytes {
            return Err("PACK_UPDATE_STALE".into());
        }
        h.update(&b[..n]);
        output.write_all(&b[..n]).map_err(|_| code())?;
    }
    if copied != stamp.bytes || h.finalize().as_slice() != stamp.sha256 {
        return Err("PACK_UPDATE_STALE".into());
    }
    output.sync_all().map_err(|_| code())?;
    drop(output);
    if let Ok(meta) = dst.symlink_metadata(relative) {
        if !meta.is_file() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
        dst.remove_file(relative).map_err(|_| code())?;
    }
    dst.rename(&temp, &dst, relative).map_err(|_| code())?;
    Ok(())
}
fn retain_archive(plan: &Plan, id: &str) -> Result<PathBuf, String> {
    let path = pack_archive_path(id)?;
    fs::create_dir_all(path.parent().ok_or_else(code)?).map_err(|_| code())?;
    let mut source = fs::File::open(&plan.archive).map_err(|_| code())?;
    let mut dest = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| code())?;
    let outcome: Result<(), String> = (|| {
        std::io::copy(&mut source, &mut dest).map_err(|_| code())?;
        dest.sync_all().map_err(|_| code())?;
        if hash(&path)? != plan.archive_hash {
            return Err("PACK_UPDATE_STALE".into());
        }
        Ok(())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&path);
    }
    outcome?;
    Ok(path)
}
fn sidecar(base: &Path, id: &str) -> PathBuf {
    base.join(format!(".cobble-pack-update-{id}.json"))
}
fn write_sidecar(
    base: &Path,
    id: &str,
    archive_id: &str,
    task_id: &str,
    source_id: &str,
) -> Result<PathBuf, String> {
    let path = sidecar(base, id);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| code())?;
    let data=serde_json::to_vec(&serde_json::json!({"taskId":task_id,"sourceGameId":source_id,"targetId":id,"archiveId":archive_id})).map_err(|_|code())?;
    file.write_all(&data).map_err(|_| code())?;
    file.sync_all().map_err(|_| code())?;
    Ok(path)
}
fn remove_owned_target(target: &Path) {
    if let Ok(meta) = fs::symlink_metadata(target) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return;
            }
        }
        if meta.is_dir() && !meta.file_type().is_symlink() {
            let _ = fs::remove_dir_all(target);
        }
    }
}
fn remove_new_file(target: &Path, relative: &str) -> Result<(), String> {
    use cap_std::fs::Dir;
    cobble_store::SafeRelativePath::parse(relative).map_err(|_| code())?;
    let root = fs::symlink_metadata(target).map_err(|_| code())?;
    if !root.is_dir() || root.file_type().is_symlink() || is_reparse_point(&root) {
        return Err("PACK_UPDATE_LINK_DENIED".into());
    }
    let dir = Dir::open_ambient_dir(target, cap_std::ambient_authority()).map_err(|_| code())?;
    let mut parent = PathBuf::new();
    for part in Path::new(relative).parent().ok_or_else(code)?.components() {
        parent.push(part.as_os_str());
        let meta = dir
            .symlink_metadata(&parent)
            .map_err(|_| "PACK_UPDATE_STALE")?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
    }
    let meta = dir
        .symlink_metadata(relative)
        .map_err(|_| "PACK_UPDATE_STALE")?;
    if !meta.is_file() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
        return Err("PACK_UPDATE_LINK_DENIED".into());
    }
    dir.remove_file(relative).map_err(|_| code())
}

fn is_reparse_point(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}
fn is_cap_reparse_point(meta: &cap_std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}
#[allow(clippy::too_many_arguments)]
fn publish(
    app: &AppHandle,
    plan: &Plan,
    id: &str,
    name: &str,
    target: &Path,
    archive_id: &str,
    preview: &PackUpdatePreview,
    selected: &BTreeSet<String>,
    replacements: &BTreeMap<String, Option<String>>,
) -> Result<String, String> {
    let mut game = plan.game.clone();
    let target_pack =
        cobble_content::read_mrpack(&pack_archive_path(archive_id)?).map_err(|_| code())?;
    game.id = id.into();
    game.name = name.into();
    game.directory = target.into();
    game.version = target_pack.minecraft_version;
    game.loader_kind = target_pack.loader_kind;
    game.loader_version = target_pack.loader_version;
    game.pack_name = Some(target_pack.name);
    game.pack_version = Some(target_pack.version_id);
    game.pack_source_id = Some(archive_id.into());
    game.pack_origin = None;
    game.pack_optional_files = plan
        .game
        .pack_optional_files
        .iter()
        .filter(|p| {
            target_pack
                .files
                .iter()
                .any(|f| f.optional && &f.path == *p)
        })
        .cloned()
        .collect();
    let mut managed_mods: BTreeSet<String> = preview
        .added
        .iter()
        .chain(&preview.version_changed)
        .chain(&preview.unchanged)
        .filter_map(|c| c.after.as_ref())
        .filter(|f| f.path.starts_with("mods/"))
        .map(|f| f.path.clone())
        .collect();
    for change in &preview.version_changed {
        if let (Some(before), Some(after)) = (&change.before, &change.after)
            && before.path != after.path
            && selected.contains(&before.path)
        {
            managed_mods.remove(&after.path);
            if before.path.starts_with("mods/") {
                managed_mods.insert(before.path.clone());
            }
        }
    }
    for (old, new) in replacements {
        if let Some(new) = new {
            managed_mods.remove(new);
        }
        if !plan.player_added.contains(old) {
            managed_mods.insert(old.clone());
        }
    }
    game.pack_mod_files = managed_mods.into_iter().collect();
    game.status = "installed".into();
    game.error = None;
    game.content_installing = false;
    game.progress = None;
    game.launch_state = idle();
    game.launch_error = None;
    game.log_path = None;
    game.last_launched_at = None;
    let state = app.state::<GameLibrary>();
    let mut lib = state.0.lock().map_err(|_| code())?;
    if lib
        .games
        .iter()
        .any(|g| g.id == id || g.directory == target)
    {
        return Err("PACK_UPDATE_STALE".into());
    }
    let mut games = lib.games.clone();
    games.push(game.clone());
    let path = library_file()?;
    fs::create_dir_all(path.parent().ok_or_else(code)?).map_err(|_| code())?;
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().ok_or_else(code)?).map_err(|_| code())?;
    temp.write_all(&serde_json::to_vec_pretty(&games).map_err(|_| code())?)
        .map_err(|_| code())?;
    temp.as_file().sync_all().map_err(|_| code())?;
    temp.persist(path).map_err(|_| code())?;
    lib.games = games;
    drop(lib);
    let _ = app.emit("game-install", &game);
    Ok(id.into())
}
#[tauri::command]
pub async fn start_game_pack_update_copy(
    app: AppHandle,
    request: CopyRequest,
) -> Result<String, String> {
    let plan = {
        let mut s = plans().lock().map_err(|_| code())?;
        s.remove(&request.plan_id)
            .ok_or("PACK_UPDATE_PLAN_EXPIRED")?
    };
    if plan.created.elapsed() > Duration::from_secs(300) {
        return Err("PACK_UPDATE_PLAN_EXPIRED".into());
    }
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("PACK_UPDATE_NAME_INVALID".into());
    }
    let selected = requested_paths(&plan, &request, &plan.preview)?;
    let json_merges = json::choices(&plan.json_documents, &request.json_merges, &selected)?;
    let replacements = replacement_map(
        &plan.preview,
        &selected,
        &request.replace_new_mods,
        &plan.player_added,
    )?;
    let id = nonce()?;
    let base = default_directory()?;
    fs::create_dir_all(&base).map_err(|_| code())?;
    let target = base.join(&id);
    if target.exists() {
        return Err("PACK_UPDATE_TARGET_EXISTS".into());
    }
    let archive_id = nonce()?;
    let cancel = Arc::new(AtomicBool::new(false));
    let task_id =
        tasks::register_pack_update_copy(&app, &plan.game.id, &id, &archive_id, cancel.clone())?;
    let sidecar = match write_sidecar(&base, &id, &archive_id, &task_id, &plan.game.id) {
        Ok(path) => path,
        Err(error) => {
            tasks::finish_external(&app, &task_id, Err(error.clone()));
            return Err(error);
        }
    };
    let return_id = task_id.clone();
    let name = name.to_owned();
    tauri::async_runtime::spawn(async move {
        let worker = async {
            tasks::wait_external_turn(&app, &task_id).await?;
            if plan.created.elapsed() > Duration::from_secs(300) {
                return Err("PACK_UPDATE_PLAN_EXPIRED".into());
            }
            let (current, _source_lease) = management::begin(&app, &plan.game.id, true)?;
            if current.directory != plan.game.directory
                || current.pack_source_id != plan.game.pack_source_id
            {
                return Err("PACK_UPDATE_STALE".into());
            }
            let preview = tauri::async_runtime::spawn_blocking({
                let plan = plan.clone();
                move || check_plan(&plan)
            })
            .await
            .map_err(|_| code())??;
            let archive = pack_archive_path(&archive_id)?;
            let settings = plan
                .game
                .download_source
                .clone()
                .map(Ok)
                .unwrap_or_else(cobble_minecraft::read_download_settings)
                .map_err(|_| code())?;
            let source = settings.download_source().map_err(|_| code())?;
            let progress_app = app.clone();
            let progress_id = task_id.clone();
            let control = cobble_core::OperationControl::new(Arc::new(move |p| {
                tasks::external_progress(
                    &progress_app,
                    &progress_id,
                    &p.phase,
                    p.completed,
                    p.total,
                )
            }));
            let watch = control.clone();
            let flag = cancel.clone();
            let watcher = tauri::async_runtime::spawn(async move {
                while !flag.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                watch.cancel();
            });
            let result = async {
                let target_pack = cobble_content::read_mrpack(&plan.archive).map_err(|_| code())?;
                let optional: Vec<String> = plan
                    .game
                    .pack_optional_files
                    .iter()
                    .filter(|p| {
                        target_pack
                            .files
                            .iter()
                            .any(|f| f.optional && &f.path == *p)
                    })
                    .cloned()
                    .collect();
                let worlds = plan.game.directory.join("saves");
                let world_inventory = if request.include_worlds && worlds.exists() {
                    let path = worlds.clone();
                    let flag = cancel.clone();
                    Some(
                        tauri::async_runtime::spawn_blocking(move || disk::inventory(&path, &flag))
                            .await
                            .map_err(|_| code())?
                            .map_err(|_| code())?,
                    )
                } else {
                    None
                };
                let selected_bytes = selected
                    .iter()
                    .try_fold(0u64, |sum, path| {
                        sum.checked_add(plan.candidate.get(path).ok_or("PACK_UPDATE_STALE")?.bytes)
                            .ok_or("SPACE_OVERFLOW")
                    })
                    .map_err(str::to_owned)?;
                let json_bytes = json_merges
                    .values()
                    .try_fold(0u64, |sum, bytes| sum.checked_add(bytes.len() as u64))
                    .ok_or("SPACE_OVERFLOW")?;
                let world_bytes = world_inventory
                    .as_ref()
                    .map_or(0, |inventory| inventory.bytes);
                let migration_bytes = selected_bytes
                    .checked_add(json_bytes)
                    .and_then(|n| n.checked_add(world_bytes))
                    .ok_or("SPACE_OVERFLOW")?;
                let retained_source = plan
                    .candidate
                    .values()
                    .try_fold(world_bytes, |sum, stamp| sum.checked_add(stamp.bytes))
                    .ok_or("SPACE_OVERFLOW")?;
                let archive_bytes = fs::metadata(&plan.archive).map_err(|_| code())?.len();
                let java = cobble_core::capture_java_choice(&plan.game.runtime_overrides.java)
                    .map_err(|e| e.to_string())?;
                let space = native_readiness_space::estimate_pack(
                    &target_pack,
                    &optional,
                    &target,
                    settings.clone(),
                    &java,
                    native_readiness_space::PackSpaceExtras {
                        archive_copy: Some((archive.clone(), archive_bytes)),
                        migration_bytes,
                        retained_source: Some((plan.game.directory.clone(), retained_source)),
                    },
                    control.clone(),
                )
                .await?;
                if !space.can_proceed {
                    return Err("SPACE_INSUFFICIENT".into());
                }
                control
                    .check_cancelled()
                    .map_err(|_| "PACK_UPDATE_CANCELLED")?;
                let archive = retain_archive(&plan, &archive_id)?;
                let installed = cobble_core::install_mrpack_controlled(
                    &archive,
                    &target,
                    source,
                    &optional,
                    control.clone(),
                )
                .await
                .map_err(|e| e.to_string())?;
                if cancel.load(Ordering::Acquire) {
                    return Err("PACK_UPDATE_CANCELLED".into());
                }
                // The old game is locked and rechecked before any migration.
                check_plan(&plan)?;
                for new in replacements.values().flatten() {
                    remove_new_file(&target, new)?;
                }
                for (number, path) in selected.iter().enumerate() {
                    if cancel.load(Ordering::Acquire) {
                        return Err("PACK_UPDATE_CANCELLED".into());
                    }
                    let stamp = plan.candidate.get(path).ok_or("PACK_UPDATE_STALE")?;
                    // A locally preserved renamed Mod replaces the new managed path.
                    for change in &preview.version_changed {
                        if let (Some(before), Some(after)) = (&change.before, &change.after)
                            && before.path == *path
                            && before.path != after.path
                        {
                            remove_new_file(&target, &after.path)?;
                        }
                    }
                    copy_local(&plan.game.directory, &target, path, stamp, &cancel)?;
                    tasks::external_progress(
                        &app,
                        &task_id,
                        "migrating",
                        number + 1,
                        selected.len(),
                    );
                }
                for (path, bytes) in &json_merges {
                    json::write(&target, path, bytes, &cancel)?;
                }
                if let Some(inventory) = &world_inventory {
                    disk::copy_verified(&worlds, &target.join("saves"), inventory, &cancel, |_| {})
                        .map_err(|_| code())?;
                }
                control
                    .check_cancelled()
                    .map_err(|_| "PACK_UPDATE_CANCELLED")?;
                cobble_core::prepare_launch(&installed.minecraft_version, &target)
                    .await
                    .map_err(|_| "PACK_UPDATE_NOT_PLAYABLE")?;
                control
                    .check_cancelled()
                    .map_err(|_| "PACK_UPDATE_CANCELLED")?;
                publish(
                    &app,
                    &plan,
                    &id,
                    &name,
                    &target,
                    &archive_id,
                    &preview,
                    &selected,
                    &replacements,
                )
            }
            .await;
            watcher.abort();
            if result.is_err() {
                remove_owned_target(&target);
                let _ = fs::remove_file(&archive);
            }
            result
        }
        .await;
        if worker.is_err() {
            remove_owned_target(&target);
            if let Ok(archive) = pack_archive_path(&archive_id) {
                let _ = fs::remove_file(archive);
            }
        }
        let _ = fs::remove_file(&sidecar);
        tasks::finish_external(&app, &task_id, worker.map(Some));
    });
    Ok(return_id)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_overrides_new_bytes_and_never_changes_source() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("config")).unwrap();
        fs::create_dir(target.path().join("config")).unwrap();
        fs::write(source.path().join("config/local.txt"), b"player").unwrap();
        fs::write(target.path().join("config/local.txt"), b"new pack").unwrap();
        let stamp = disk::inventory(&source.path().join("config"), &AtomicBool::new(false))
            .unwrap()
            .files
            .remove(Path::new("local.txt"))
            .unwrap();
        copy_local(
            source.path(),
            target.path(),
            "config/local.txt",
            &stamp,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            fs::read(source.path().join("config/local.txt")).unwrap(),
            b"player"
        );
        assert_eq!(
            fs::read(target.path().join("config/local.txt")).unwrap(),
            b"player"
        );
    }
    #[test]
    fn stale_or_cancelled_migration_does_not_replace_new_bytes() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("mods")).unwrap();
        fs::create_dir(target.path().join("mods")).unwrap();
        fs::write(source.path().join("mods/a.jar"), b"old").unwrap();
        fs::write(target.path().join("mods/a.jar"), b"new").unwrap();
        let stamp = disk::inventory(&source.path().join("mods"), &AtomicBool::new(false))
            .unwrap()
            .files
            .remove(Path::new("a.jar"))
            .unwrap();
        fs::write(source.path().join("mods/a.jar"), b"edited").unwrap();
        assert_eq!(
            copy_local(
                source.path(),
                target.path(),
                "mods/a.jar",
                &stamp,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
            "PACK_UPDATE_STALE"
        );
        assert_eq!(fs::read(target.path().join("mods/a.jar")).unwrap(), b"new");
        fs::write(source.path().join("mods/a.jar"), b"old").unwrap();
        let _ = fs::remove_file(target.path().join("mods/a.jar.cobble-update-tmp"));
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            copy_local(
                source.path(),
                target.path(),
                "mods/a.jar",
                &stamp,
                &cancelled
            )
            .unwrap_err(),
            "PACK_UPDATE_CANCELLED"
        );
        assert_eq!(fs::read(source.path().join("mods/a.jar")).unwrap(), b"old");
    }
    #[test]
    fn worlds_are_opt_in_and_renderer_cannot_supply_paths() {
        let request: CopyRequest = serde_json::from_value(
            serde_json::json!({"planId":"p","name":"copy","preservePaths":[]}),
        )
        .unwrap();
        assert!(!request.include_worlds);
        assert!(serde_json::from_value::<CopyRequest>(serde_json::json!({"planId":"p","name":"copy","preservePaths":[],"archive":"C:/foreign.mrpack"})).is_err());
    }
    #[test]
    fn removed_local_mod_requires_explicit_new_mod_decision() {
        let file = |path: &str| serde_json::json!({"identity":path,"version_id":null,"path":path,"size_bytes":3,"sha1":null,"sha512":null});
        let preview:PackUpdatePreview=serde_json::from_value(serde_json::json!({
            "identity_relation":"unknown","current_version":"old","target_version":"new","runtime_changed":false,
            "added":[{"before":null,"after":file("mods/new.jar"),"local_state":"missing","preserve":false}],
            "removed":[{"before":file("mods/old.jar"),"after":null,"local_state":"modified","preserve":true}],
            "version_changed":[],"unchanged":[]
        })).unwrap();
        let selected = BTreeSet::from(["mods/old.jar".into()]);
        assert_eq!(
            replacement_map(&preview, &selected, &[], &[]).unwrap_err(),
            "PACK_UPDATE_MOD_CHOICE_REQUIRED"
        );
        let keep_both = [ModReplacement {
            old_path: "mods/old.jar".into(),
            new_path: None,
        }];
        assert_eq!(
            replacement_map(&preview, &selected, &keep_both, &[]).unwrap()["mods/old.jar"],
            None
        );
        let replace = [ModReplacement {
            old_path: "mods/old.jar".into(),
            new_path: Some("mods/new.jar".into()),
        }];
        assert_eq!(
            replacement_map(&preview, &selected, &replace, &[]).unwrap()["mods/old.jar"],
            Some("mods/new.jar".into())
        );
        let invalid = [ModReplacement {
            old_path: "mods/old.jar".into(),
            new_path: Some("mods/foreign.jar".into()),
        }];
        assert_eq!(
            replacement_map(&preview, &selected, &invalid, &[]).unwrap_err(),
            "PACK_UPDATE_SELECTION_INVALID"
        );
        let player = ["mods/added.jar".into()];
        let selected = BTreeSet::from(["mods/added.jar".into()]);
        assert_eq!(
            replacement_map(&preview, &selected, &[], &player).unwrap_err(),
            "PACK_UPDATE_MOD_CHOICE_REQUIRED"
        );
    }
    #[test]
    fn replacement_removes_only_the_new_target_file() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir_all(source.path().join("mods")).unwrap();
        fs::create_dir_all(target.path().join("mods")).unwrap();
        fs::write(source.path().join("mods/new.jar"), b"source").unwrap();
        fs::write(target.path().join("mods/new.jar"), b"new pack").unwrap();
        remove_new_file(target.path(), "mods/new.jar").unwrap();
        assert!(!target.path().join("mods/new.jar").exists());
        assert_eq!(
            fs::read(source.path().join("mods/new.jar")).unwrap(),
            b"source"
        );
        assert!(remove_new_file(target.path(), "../mods/new.jar").is_err());
    }
    #[tokio::test]
    #[ignore = "uses the real Minecraft metadata/artifact downloader"]
    async fn real_mrpack_install_then_selected_local_migration() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("new.mrpack");
        let mut zip = zip::ZipWriter::new(fs::File::create(&pack).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","versionId":"new","name":"Update fixture","files":[],"dependencies":{"minecraft":"1.21.1"}}"#).unwrap();
        zip.start_file("overrides/config/settings.txt", options)
            .unwrap();
        zip.write_all(b"new pack").unwrap();
        zip.finish().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("fresh");
        fs::create_dir_all(source.join("config")).unwrap();
        fs::create_dir_all(source.join("saves/world")).unwrap();
        fs::write(source.join("config/settings.txt"), b"player edit").unwrap();
        fs::write(source.join("saves/world/level.dat"), b"synthetic world").unwrap();
        let result = cobble_core::install_mrpack_controlled(
            &pack,
            &target,
            cobble_minecraft::DownloadSource::Automatic,
            &[],
            cobble_core::OperationControl::new(Arc::new(|_| {})),
        )
        .await
        .unwrap();
        assert_eq!(result.minecraft_version, "1.21.1");
        assert_eq!(
            fs::read(target.join("config/settings.txt")).unwrap(),
            b"new pack"
        );
        assert!(!target.join("saves/world/level.dat").exists());
        let stamp = disk::inventory(&source.join("config"), &AtomicBool::new(false))
            .unwrap()
            .files
            .remove(Path::new("settings.txt"))
            .unwrap();
        copy_local(
            &source,
            &target,
            "config/settings.txt",
            &stamp,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            fs::read(target.join("config/settings.txt")).unwrap(),
            b"player edit"
        );
        assert_eq!(
            fs::read(source.join("config/settings.txt")).unwrap(),
            b"player edit"
        );
        let world_inventory =
            disk::inventory(&source.join("saves"), &AtomicBool::new(false)).unwrap();
        disk::copy_verified(
            &source.join("saves"),
            &target.join("saves"),
            &world_inventory,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(
            fs::read(target.join("saves/world/level.dat")).unwrap(),
            b"synthetic world"
        );
        cobble_core::prepare_launch("1.21.1", &target)
            .await
            .unwrap();
    }
}

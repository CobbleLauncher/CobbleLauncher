// SPDX-License-Identifier: MIT
//! Single-world ZIP staging and recoverable directory swaps, under a stopped-game lease.
use crate::{InstallProgress, OperationControl};
use cobble_content::{
    LocalWorld, MAX_WORLD_ARCHIVE_BYTES, inspect_world_archive, read_world_metadata,
    world_directory,
};
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const OPERATIONS: &str = ".cobble-world-operations";
const BACKUPS: &str = ".cobble-world-backups";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldImportOutcome {
    pub folder_name: String,
    pub backup_id: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct WorldReplacementBackup {
    pub backup_id: String,
    pub folder_name: String,
    pub world_name: String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Request {
    Import {
        archive: PathBuf,
        folder_name: String,
        replace: bool,
    },
    Restore {
        backup_id: String,
    },
    Snapshot {
        snapshot_id: String,
        folder_name: String,
        replace: bool,
    },
}
#[derive(Serialize, Deserialize)]
struct State {
    request: Request,
    folder_name: String,
    backup_id: Option<String>,
    created_at: u64,
    staging_complete: bool,
    committing: bool,
    complete: bool,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    folder_name: String,
    world_name: String,
    created_at: u64,
}

fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn real_dir(path: &Path) -> Result<PathBuf, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err("世界操作目录不能是链接或特殊文件".into());
    }
    Ok(dunce::canonicalize(path)?)
}
fn leaf(name: &str) -> Result<(), Error> {
    if name.len() > 200 || SafeRelativePath::parse(name)?.as_str().contains('/') {
        return Err("世界文件夹名称无效".into());
    }
    Ok(())
}
fn operation_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("世界操作 ID 无效".into());
    }
    Ok(())
}
fn exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn ensure_dir(path: &Path) -> Result<PathBuf, Error> {
    if !exists(path)? {
        fs::create_dir(path)?;
    }
    real_dir(path)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err("世界操作记录无效".into());
    }
    Ok(serde_json::from_reader(
        File::open(path)?.take(1024 * 1024),
    )?)
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Error> {
    let temporary = path.with_extension("json.tmp");
    if exists(&temporary)? {
        let metadata = fs::symlink_metadata(&temporary)?;
        if linked(&metadata) || !metadata.is_file() {
            return Err("世界操作临时记录无效".into());
        }
        fs::remove_file(&temporary)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    serde_json::to_writer(&mut file, value)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    Ok(())
}
fn world_metadata(path: &Path, folder: &str) -> Result<LocalWorld, Error> {
    real_dir(path)?;
    let level = path.join("level.dat");
    let metadata = fs::symlink_metadata(&level)?;
    if linked(&metadata) || !metadata.is_file() {
        return Err("世界必须包含普通 level.dat 文件".into());
    }
    Ok(read_world_metadata(File::open(level)?, folder))
}
pub(crate) fn lock_world(path: &Path) -> Result<Option<File>, Error> {
    if !exists(path)? {
        return Ok(None);
    }
    real_dir(path)?;
    let lock = path.join("session.lock");
    if !exists(&lock)? {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&lock)?;
    if linked(&metadata) || !metadata.is_file() {
        return Err("世界锁文件不能是链接".into());
    }
    let file = OpenOptions::new().read(true).write(true).open(lock)?;
    file.try_lock()
        .map_err(|_| "世界正在使用中，请先关闭游戏")?;
    #[cfg(unix)]
    rustix::fs::fcntl_lock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| "世界正在使用中，请先关闭游戏")?;
    Ok(Some(file))
}
fn validate_tree(path: &Path, depth: usize) -> Result<(), Error> {
    if depth > 128 {
        return Err("世界目录超过 128 层".into());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) {
            return Err("世界不能包含链接或重解析点".into());
        }
        if metadata.is_dir() {
            validate_tree(&entry.path(), depth + 1)?;
        } else if !metadata.is_file() {
            return Err("世界中有不支持的特殊文件".into());
        }
    }
    Ok(())
}
fn saves(game: &Path) -> Result<PathBuf, Error> {
    ensure_dir(&game.join("saves"))
}
fn backup_root(game: &Path, id: &str) -> Result<PathBuf, Error> {
    operation_id(id)?;
    real_dir(&game.join(BACKUPS))?;
    real_dir(&game.join(BACKUPS).join(id))
}
fn outcome(state: &State) -> WorldImportOutcome {
    WorldImportOutcome {
        folder_name: state.folder_name.clone(),
        backup_id: state.backup_id.clone(),
    }
}

fn existing_operation(game: &Path, id: &str) -> Result<Option<(PathBuf, State)>, Error> {
    operation_id(id)?;
    let root = game.join(OPERATIONS);
    if !exists(&root)? {
        return Ok(None);
    }
    real_dir(&root)?;
    let operation = root.join(id);
    if !exists(&operation)? {
        return Ok(None);
    }
    real_dir(&operation)?;
    if !exists(&operation.join("state.json"))? {
        return Ok(None);
    }
    let state: State = read_json(&operation.join("state.json"))?;
    leaf(&state.folder_name)?;
    Ok(Some((operation, state)))
}

pub fn world_operation_is_complete(game_dir: &Path, id: &str) -> Result<bool, Error> {
    let game = real_dir(game_dir)?;
    Ok(existing_operation(&game, id)?.is_some_and(|(_, state)| state.complete))
}

fn begin(
    game: &Path,
    id: &str,
    request: Request,
    folder: &str,
    replace: bool,
) -> Result<(PathBuf, State), Error> {
    operation_id(id)?;
    leaf(folder)?;
    let operations = ensure_dir(&game.join(OPERATIONS))?;
    let operation = operations.join(id);
    if let Some((operation, state)) = existing_operation(game, id)? {
        if state.request != request || state.folder_name != folder {
            return Err("这个世界操作 ID 属于不同请求".into());
        }
        return Ok((operation, state));
    }
    let target = saves(game)?.join(folder);
    let present = exists(&target)?;
    if present {
        if !replace {
            return Err("已存在同名世界，请使用新名称或明确选择替换".into());
        }
        world_directory(game, folder)?;
    } else if matches!(
        request,
        Request::Import { replace: true, .. } | Request::Snapshot { replace: true, .. }
    ) {
        return Err("选定的原世界已不存在，请重新选择".into());
    }
    let state = State {
        request,
        folder_name: folder.into(),
        backup_id: present.then(|| id.into()),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
        staging_complete: false,
        committing: false,
        complete: false,
    };
    if exists(&operation)? {
        real_dir(&operation)?;
        if fs::read_dir(&operation)?.any(|entry| match entry {
            Ok(entry) => entry.file_name() != "state.json.tmp",
            Err(_) => true,
        }) {
            return Err("世界操作目录缺少有效记录，未接管已有内容".into());
        }
    } else {
        fs::create_dir(&operation)?;
    }
    write_json(&operation.join("state.json"), &state)?;
    Ok((operation, state))
}

fn extract(archive: &Path, staging: &Path, control: &OperationControl) -> Result<(), Error> {
    control.check_cancelled()?;
    let before = fs::symlink_metadata(archive)?;
    if linked(&before) || !before.is_file() || before.len() > MAX_WORLD_ARCHIVE_BYTES {
        return Err("世界 ZIP 不是有效的普通文件".into());
    }
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    let plan = inspect_world_archive(
        &mut zip,
        archive
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported World"),
    )?;
    fs::create_dir(staging)?;
    let progress = control.progress();
    progress(InstallProgress {
        detail: None,
        phase: "world-extract".into(),
        completed: 0,
        total: plan.preview.file_count,
        reused: 0,
    });
    let mut completed = 0;
    for entry in plan.entries {
        control.check_cancelled()?;
        let path = staging.join(&entry.path);
        if entry.directory {
            fs::create_dir_all(path)?;
            continue;
        }
        fs::create_dir_all(path.parent().ok_or("世界文件路径无效")?)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut input = zip.by_index(entry.index)?;
        let mut copied = 0_u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            control.check_cancelled()?;
            let length = input.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            copied += length as u64;
            if copied > entry.size {
                return Err("世界 ZIP 解压大小与目录记录不一致".into());
            }
            output.write_all(&buffer[..length])?;
        }
        if copied != entry.size {
            return Err("世界 ZIP 文件不完整".into());
        }
        completed += 1;
        progress(InstallProgress {
            detail: None,
            phase: "world-extract".into(),
            completed,
            total: plan.preview.file_count,
            reused: 0,
        });
    }
    let after = fs::symlink_metadata(archive)?;
    if linked(&after)
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("解压期间源 ZIP 发生改变，请重试".into());
    }
    world_metadata(staging, "Imported World")?;
    control.check_cancelled()
}

fn prepare_backup(game: &Path, state: &State) -> Result<Option<PathBuf>, Error> {
    let Some(id) = &state.backup_id else {
        return Ok(None);
    };
    operation_id(id)?;
    let backups = ensure_dir(&game.join(BACKUPS))?;
    let backup = ensure_dir(&backups.join(id))?;
    let info = backup.join("backup.json");
    if !exists(&info)? {
        let old = world_metadata(&saves(game)?.join(&state.folder_name), &state.folder_name)?;
        write_json(
            &info,
            &Backup {
                folder_name: state.folder_name.clone(),
                world_name: old.name,
                created_at: state.created_at,
            },
        )?;
    } else {
        let info: Backup = read_json(&info)?;
        if info.folder_name != state.folder_name {
            return Err("世界备份记录与操作不一致".into());
        }
    }
    Ok(Some(backup.join("world")))
}

fn swap(game: &Path, operation: &Path, state: &mut State) -> Result<WorldImportOutcome, Error> {
    let target = saves(game)?.join(&state.folder_name);
    let stage = operation.join("stage");
    let backup = prepare_backup(game, state)?;
    let _target_lock = lock_world(&target)?;
    let _stage_lock = lock_world(&stage)?;
    let _backup_lock = match &backup {
        Some(path) => lock_world(path)?,
        None => None,
    };
    // Windows refuses to rename a directory with an open child, including our
    // own session.lock handle. Probe every world first, then close our handles;
    // an external game's open handle still prevents the directory rename.
    #[cfg(windows)]
    drop((_target_lock, _stage_lock, _backup_lock));
    if exists(&stage)? {
        real_dir(&stage)?;
        if exists(&target)? {
            let old = backup.as_ref().ok_or("导入目标出现同名世界，未覆盖")?;
            if exists(old)? {
                return Err("原世界备份与目标同时存在，请先检查文件夹".into());
            }
            world_directory(game, &state.folder_name)?;
            validate_tree(&target, 0)?;
            fs::rename(&target, old).map_err(|error| format!("移动原世界到备份失败：{error}"))?;
        }
        if let Some(old) = &backup
            && !exists(old)?
        {
            return Err("找不到原世界备份，未发布新世界".into());
        }
        fs::rename(&stage, &target).map_err(|error| format!("发布暂存世界失败：{error}"))?;
    } else if !exists(&target)? {
        return Err("世界交换缺少暂存目录和目标，请检查备份".into());
    }
    world_metadata(&target, &state.folder_name)?;
    state.complete = true;
    write_json(&operation.join("state.json"), state)?;
    Ok(outcome(state))
}

fn rollback(game: &Path, operation: &Path, state: &mut State) -> Result<(), Error> {
    let target = saves(game)?.join(&state.folder_name);
    let stage = operation.join("stage");
    let backup_world = if let Some(id) = &state.backup_id {
        operation_id(id)?;
        let backups = game.join(BACKUPS);
        if exists(&backups)? {
            real_dir(&backups)?;
        }
        let parent = backups.join(id);
        if exists(&parent)? {
            real_dir(&parent)?;
        }
        Some(parent.join("world"))
    } else {
        None
    };
    let _target_lock = lock_world(&target)?;
    let _stage_lock = lock_world(&stage)?;
    let _backup_lock = match &backup_world {
        Some(path) => lock_world(path)?,
        None => None,
    };
    #[cfg(windows)]
    drop((_target_lock, _stage_lock, _backup_lock));
    if let Some(backup) = &backup_world {
        if exists(backup)? {
            if exists(&target)? {
                if exists(&stage)? {
                    return Err("世界回滚遇到同名目录，请检查原世界备份".into());
                }
                fs::rename(&target, &stage)?;
            }
            fs::rename(backup, target)?;
        }
    } else if !exists(&stage)? && exists(&target)? {
        fs::rename(target, &stage)?;
    }
    if let Request::Restore { backup_id } = &state.request {
        let source = backup_root(game, backup_id)?.join("world");
        if exists(&stage)? && !exists(&source)? {
            fs::rename(&stage, &source)?;
        }
    }
    state.committing = false;
    state.complete = false;
    state.staging_complete = exists(&stage)?;
    write_json(&operation.join("state.json"), state)
}

fn prepare_restore_stage(game: &Path, operation: &Path, state: &mut State) -> Result<(), Error> {
    if state.staging_complete {
        return Ok(());
    }
    let Request::Restore { backup_id } = &state.request else {
        return Err("世界暂存尚未完成".into());
    };
    let source = backup_root(game, backup_id)?.join("world");
    let stage = operation.join("stage");
    if exists(&stage)? {
        if exists(&source)? {
            return Err("世界恢复同时存在两个源目录，未覆盖".into());
        }
        real_dir(&stage)?;
    } else {
        let _lock = lock_world(&source)?;
        world_metadata(&source, &state.folder_name)?;
        validate_tree(&source, 0)?;
        #[cfg(windows)]
        drop(_lock);
        fs::rename(source, stage)?;
    }
    state.staging_complete = true;
    write_json(&operation.join("state.json"), state)
}

fn commit(
    game: &Path,
    operation: &Path,
    state: &mut State,
    control: &OperationControl,
) -> Result<WorldImportOutcome, Error> {
    let progress = control.progress();
    progress(InstallProgress {
        detail: None,
        phase: "world-commit".into(),
        completed: 0,
        total: 1,
        reused: 0,
    });
    control.check_cancelled()?;
    state.committing = true;
    write_json(&operation.join("state.json"), state)?;
    let result = (|| {
        if !state.staging_complete {
            prepare_restore_stage(game, operation, state)?;
        }
        swap(game, operation, state)
    })();
    match result {
        Ok(result) => {
            progress(InstallProgress {
                detail: None,
                phase: "world-complete".into(),
                completed: 1,
                total: 1,
                reused: 0,
            });
            Ok(result)
        }
        Err(error) => {
            if let Err(restore) = rollback(game, operation, state) {
                return Err(format!(
                    "世界导入失败：{error}；回滚未完成：{restore}。原世界保留在备份中"
                )
                .into());
            }
            Err(error)
        }
    }
}

pub async fn import_world_archive(
    archive: &Path,
    game_dir: &Path,
    folder_name: &str,
    replace: bool,
    operation_id: &str,
    control: OperationControl,
) -> Result<WorldImportOutcome, Error> {
    let (archive, game, folder, id) = (
        archive.to_owned(),
        game_dir.to_owned(),
        folder_name.to_owned(),
        operation_id.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        control.check_cancelled()?;
        let game = real_dir(&game)?;
        let request = Request::Import {
            archive: archive.clone(),
            folder_name: folder.clone(),
            replace,
        };
        let (operation, mut state) = begin(&game, &id, request, &folder, replace)?;
        if state.complete {
            return Ok(outcome(&state));
        }
        let stage = operation.join("stage");
        if !state.staging_complete {
            let target = saves(&game)?.join(&folder);
            let source = dunce::canonicalize(&archive)?;
            if source.starts_with(&target) {
                return Err("源 ZIP 不能位于要替换的世界内".into());
            }
            let _lock = lock_world(&target)?;
            if exists(&stage)? {
                real_dir(&stage)?;
                fs::remove_dir_all(&stage)?;
            }
            if let Err(error) = extract(&archive, &stage, &control) {
                if exists(&stage)? {
                    let _ = fs::remove_dir_all(&stage);
                }
                return Err(error);
            }
            state.staging_complete = true;
            write_json(&operation.join("state.json"), &state)?;
        }
        commit(&game, &operation, &mut state, &control)
    })
    .await?
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_repository_snapshot(
    game_dir: &Path,
    folder_name: &str,
    snapshot_id: &str,
    replace: bool,
    operation_id: &str,
    control: &OperationControl,
    populate: impl FnOnce(&Path) -> Result<(), Error>,
) -> Result<WorldImportOutcome, Error> {
    control.check_cancelled()?;
    let game = real_dir(game_dir)?;
    let request = Request::Snapshot {
        snapshot_id: snapshot_id.into(),
        folder_name: folder_name.into(),
        replace,
    };
    let (operation, mut state) = begin(&game, operation_id, request, folder_name, replace)?;
    if state.complete {
        return Ok(outcome(&state));
    }
    if !state.staging_complete {
        let stage = operation.join("stage");
        let _lock = lock_world(&saves(&game)?.join(folder_name))?;
        if exists(&stage)? {
            real_dir(&stage)?;
            fs::remove_dir_all(&stage)?;
        }
        let prepared = populate(&stage).and_then(|()| {
            control.check_cancelled()?;
            validate_tree(&stage, 0)?;
            world_metadata(&stage, folder_name)?;
            Ok(())
        });
        if let Err(error) = prepared {
            if exists(&stage)? {
                let _ = fs::remove_dir_all(&stage);
            }
            return Err(error);
        }
        state.staging_complete = true;
        write_json(&operation.join("state.json"), &state)?;
    }
    commit(&game, &operation, &mut state, control)
}

pub fn list_world_replacement_backups(
    game_dir: &Path,
) -> Result<Vec<WorldReplacementBackup>, Error> {
    let game = real_dir(game_dir)?;
    let backups = game.join(BACKUPS);
    if !exists(&backups)? {
        return Ok(Vec::new());
    }
    real_dir(&backups)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(backups)? {
        let entry = entry?;
        real_dir(&entry.path())?;
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| "世界备份 ID 无效")?;
        operation_id(&id)?;
        if !exists(&entry.path().join("backup.json"))? || !exists(&entry.path().join("world"))? {
            continue;
        }
        let info: Backup = read_json(&entry.path().join("backup.json"))?;
        world_metadata(&entry.path().join("world"), &info.folder_name)?;
        result.push(WorldReplacementBackup {
            backup_id: id,
            folder_name: info.folder_name,
            world_name: info.world_name,
            created_at: info.created_at,
        });
    }
    result.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then(b.backup_id.cmp(&a.backup_id))
    });
    Ok(result)
}

pub async fn restore_world_replacement_backup(
    game_dir: &Path,
    backup_id: &str,
    operation_id: &str,
    control: OperationControl,
) -> Result<WorldImportOutcome, Error> {
    let (game, backup_id, id) = (
        game_dir.to_owned(),
        backup_id.to_owned(),
        operation_id.to_owned(),
    );
    tokio::task::spawn_blocking(move || {
        control.check_cancelled()?;
        let game = real_dir(&game)?;
        let request = Request::Restore {
            backup_id: backup_id.clone(),
        };
        let (operation, mut state) =
            if let Some((operation, state)) = existing_operation(&game, &id)? {
                if state.request != request {
                    return Err("这个世界操作 ID 属于不同请求".into());
                }
                (operation, state)
            } else {
                let backup = backup_root(&game, &backup_id)?;
                let info: Backup = read_json(&backup.join("backup.json"))?;
                begin(&game, &id, request, &info.folder_name, true)?
            };
        if state.complete {
            return Ok(outcome(&state));
        }
        commit(&game, &operation, &mut state, &control)
    })
    .await?
}

pub fn recover_world_operations(game_dir: &Path) -> Result<bool, Error> {
    let game = real_dir(game_dir)?;
    let operations = game.join(OPERATIONS);
    if !exists(&operations)? {
        return Ok(false);
    }
    real_dir(&operations)?;
    let mut recovered = false;
    for entry in fs::read_dir(operations)? {
        let entry = entry?;
        real_dir(&entry.path())?;
        if !exists(&entry.path().join("state.json"))? {
            continue;
        }
        let mut state: State = read_json(&entry.path().join("state.json"))?;
        leaf(&state.folder_name)?;
        if state.committing && !state.complete {
            let result = (|| {
                if !state.staging_complete {
                    prepare_restore_stage(&game, &entry.path(), &mut state)?;
                }
                swap(&game, &entry.path(), &mut state)
            })();
            match result {
                Ok(_) => recovered = true,
                Err(error) => {
                    rollback(&game, &entry.path(), &mut state)?;
                    return Err(error);
                }
            }
        }
    }
    Ok(recovered)
}

pub(crate) fn ensure_world_operations_complete(game_dir: &Path) -> Result<(), Error> {
    let operations = game_dir.join(OPERATIONS);
    if !exists(&operations)? {
        return Ok(());
    }
    real_dir(&operations)?;
    for entry in fs::read_dir(operations)? {
        let entry = entry?;
        real_dir(&entry.path())?;
        if exists(&entry.path().join("state.json"))? {
            let state: State = read_json(&entry.path().join("state.json"))?;
            if state.committing && !state.complete {
                return Err("世界替换尚未恢复，请先重试世界操作".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-world-restore-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("game")).unwrap();
            Self(path)
        }
        fn game(&self) -> PathBuf {
            self.0.join("game")
        }
        fn old_world(&self) {
            let world = self.game().join("saves/World");
            fs::create_dir_all(world.join("region")).unwrap();
            fs::write(world.join("level.dat"), b"recoverable old metadata").unwrap();
            fs::write(world.join("region/r.0.0.mca"), b"old world region").unwrap();
        }
        fn archive(&self, prefix: &str) -> PathBuf {
            let path = self.0.join("world.zip");
            let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
            for (name, bytes) in [
                ("level.dat", b"recoverable imported metadata".as_slice()),
                ("region/r.0.0.mca", b"imported region bytes"),
                ("session.lock", b"stale lock"),
            ] {
                writer
                    .start_file(
                        format!("{prefix}{name}"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn control() -> OperationControl {
        OperationControl::new(Arc::new(|_| {}))
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }
    fn cancel_when(phase: &'static str, completed: usize) -> OperationControl {
        let token = Arc::new(Mutex::new(None::<cobble_net::TransferCancellation>));
        let observed = token.clone();
        let control = OperationControl::new(Arc::new(move |event| {
            if event.phase == phase && event.completed == completed {
                observed.lock().unwrap().as_ref().unwrap().cancel();
            }
        }));
        *token.lock().unwrap() = Some(control.cancellation());
        control
    }

    #[test]
    fn repository_snapshot_publication_preserves_old_world_and_replays_without_population() {
        let fixture = Fixture::new();
        fixture.old_world();
        let result = publish_repository_snapshot(
            &fixture.game(),
            "World",
            "snapshot-a",
            true,
            "repository-restore",
            &control(),
            |stage| {
                fs::create_dir(stage)?;
                fs::write(stage.join("level.dat"), b"snapshot metadata")?;
                fs::write(stage.join("new.dat"), b"restored bytes")?;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.backup_id.as_deref(), Some("repository-restore"));
        assert_eq!(
            fs::read(fixture.game().join("saves/World/new.dat")).unwrap(),
            b"restored bytes"
        );
        assert_eq!(
            fs::read(
                fixture
                    .game()
                    .join(".cobble-world-backups/repository-restore/world/region/r.0.0.mca")
            )
            .unwrap(),
            b"old world region"
        );
        publish_repository_snapshot(
            &fixture.game(),
            "World",
            "snapshot-a",
            true,
            "repository-restore",
            &control(),
            |_| panic!("completed restore must not populate again"),
        )
        .unwrap();
        assert!(
            publish_repository_snapshot(
                &fixture.game(),
                "World",
                "different-snapshot",
                true,
                "repository-restore",
                &control(),
                |_| panic!("mismatched replay must not populate"),
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_repository_snapshot_stage_preserves_current_world() {
        let fixture = Fixture::new();
        fixture.old_world();
        assert!(
            publish_repository_snapshot(
                &fixture.game(),
                "World",
                "snapshot-b",
                true,
                "invalid-snapshot",
                &control(),
                |stage| {
                    fs::create_dir(stage)?;
                    fs::write(stage.join("unrelated.dat"), b"no level")?;
                    Ok(())
                },
            )
            .is_err()
        );
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        assert!(
            !fixture
                .game()
                .join(".cobble-world-operations/invalid-snapshot/stage")
                .exists()
        );
    }

    #[test]
    fn new_import_preserves_zip_bytes_and_completed_replay_never_rewrites_world() {
        for prefix in ["", "Enclosing world/"] {
            let fixture = Fixture::new();
            let archive = fixture.archive(prefix);
            let bytes = fs::read(&archive).unwrap();
            let runtime = runtime();
            let result = runtime
                .block_on(import_world_archive(
                    &archive,
                    &fixture.game(),
                    "Imported",
                    false,
                    "new",
                    control(),
                ))
                .unwrap();
            assert!(result.backup_id.is_none());
            let world = fixture.game().join("saves/Imported");
            assert_eq!(
                fs::read(world.join("region/r.0.0.mca")).unwrap(),
                b"imported region bytes"
            );
            assert!(!world.join("session.lock").exists());
            fs::write(world.join("region/r.0.0.mca"), b"later user edits").unwrap();
            assert_eq!(fs::read(&archive).unwrap(), bytes);
            fs::remove_file(&archive).unwrap();
            runtime
                .block_on(import_world_archive(
                    &archive,
                    &fixture.game(),
                    "Imported",
                    false,
                    "new",
                    control(),
                ))
                .unwrap();
            assert_eq!(
                fs::read(world.join("region/r.0.0.mca")).unwrap(),
                b"later user edits"
            );
            assert!(world_operation_is_complete(&fixture.game(), "new").unwrap());
            assert!(
                runtime
                    .block_on(import_world_archive(
                        &archive,
                        &fixture.game(),
                        "Other",
                        false,
                        "new",
                        control()
                    ))
                    .is_err()
            );
            assert!(
                runtime
                    .block_on(import_world_archive(
                        &archive,
                        &fixture.game(),
                        "../outside",
                        false,
                        "escape",
                        control()
                    ))
                    .is_err()
            );
        }
    }

    #[test]
    fn explicit_replacement_and_restore_preserve_both_world_generations() {
        let fixture = Fixture::new();
        fixture.old_world();
        let archive = fixture.archive("World/");
        let runtime = runtime();
        assert!(
            runtime
                .block_on(import_world_archive(
                    &archive,
                    &fixture.game(),
                    "World",
                    false,
                    "collision",
                    control()
                ))
                .is_err()
        );
        let imported = runtime
            .block_on(import_world_archive(
                &archive,
                &fixture.game(),
                "World",
                true,
                "replace",
                control(),
            ))
            .unwrap();
        assert_eq!(imported.backup_id.as_deref(), Some("replace"));
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"imported region bytes"
        );
        let backups = list_world_replacement_backups(&fixture.game()).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].backup_id, "replace");
        assert!(backups[0].created_at > 1_000_000_000_000);
        fs::write(
            fixture.game().join("saves/World/region/r.0.0.mca"),
            b"edits before restore",
        )
        .unwrap();
        let restored = runtime
            .block_on(restore_world_replacement_backup(
                &fixture.game(),
                "replace",
                "restore",
                control(),
            ))
            .unwrap();
        assert_eq!(restored.backup_id.as_deref(), Some("restore"));
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        assert_eq!(
            fs::read(
                fixture
                    .game()
                    .join(BACKUPS)
                    .join("restore/world/region/r.0.0.mca")
            )
            .unwrap(),
            b"edits before restore"
        );
        assert_eq!(
            list_world_replacement_backups(&fixture.game())
                .unwrap()
                .len(),
            1
        );
        fs::remove_dir_all(fixture.game().join(BACKUPS).join("replace")).unwrap();
        fs::write(
            fixture.game().join("saves/World/region/r.0.0.mca"),
            b"edits after restore",
        )
        .unwrap();
        runtime
            .block_on(restore_world_replacement_backup(
                &fixture.game(),
                "replace",
                "restore",
                control(),
            ))
            .unwrap();
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"edits after restore"
        );
    }

    #[test]
    fn cancellation_removes_owned_extraction_only_and_restore_cancel_keeps_backup_visible() {
        let fixture = Fixture::new();
        fixture.old_world();
        let archive = fixture.archive("");
        let runtime = runtime();
        let error = runtime
            .block_on(import_world_archive(
                &archive,
                &fixture.game(),
                "World",
                true,
                "cancel",
                cancel_when("world-extract", 1),
            ))
            .unwrap_err();
        assert!(error.is::<crate::OperationCancelled>());
        assert!(
            !fixture
                .game()
                .join(OPERATIONS)
                .join("cancel/stage")
                .exists()
        );
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        ensure_world_operations_complete(&fixture.game()).unwrap();
        runtime
            .block_on(import_world_archive(
                &archive,
                &fixture.game(),
                "World",
                true,
                "cancel",
                control(),
            ))
            .unwrap();
        let error = runtime
            .block_on(restore_world_replacement_backup(
                &fixture.game(),
                "cancel",
                "cancel-restore",
                cancel_when("world-commit", 0),
            ))
            .unwrap_err();
        assert!(error.is::<crate::OperationCancelled>());
        assert!(fixture.game().join(BACKUPS).join("cancel/world").exists());
        assert_eq!(
            list_world_replacement_backups(&fixture.game())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"imported region bytes"
        );
    }

    #[test]
    fn interrupted_renames_recover_and_ordinary_rollback_restores_previous_world() {
        for interrupted_at in 0..=2 {
            let fixture = Fixture::new();
            fixture.old_world();
            let archive = fixture.archive("");
            let game = fixture.game();
            let request = Request::Import {
                archive: archive.clone(),
                folder_name: "World".into(),
                replace: true,
            };
            let (operation, mut state) =
                begin(&game, "interrupted", request, "World", true).unwrap();
            extract(&archive, &operation.join("stage"), &control()).unwrap();
            state.staging_complete = true;
            state.committing = true;
            write_json(&operation.join("state.json"), &state).unwrap();
            let backup = prepare_backup(&game, &state).unwrap().unwrap();
            if interrupted_at >= 1 {
                fs::rename(game.join("saves/World"), &backup).unwrap();
            }
            if interrupted_at >= 2 {
                fs::rename(operation.join("stage"), game.join("saves/World")).unwrap();
            }
            assert!(ensure_world_operations_complete(&game).is_err());
            if interrupted_at == 2 {
                rollback(&game, &operation, &mut state).unwrap();
                assert_eq!(
                    fs::read(game.join("saves/World/region/r.0.0.mca")).unwrap(),
                    b"old world region"
                );
                ensure_world_operations_complete(&game).unwrap();
                commit(&game, &operation, &mut state, &control()).unwrap();
            } else {
                assert!(recover_world_operations(&game).unwrap());
            }
            assert_eq!(
                fs::read(game.join("saves/World/region/r.0.0.mca")).unwrap(),
                b"imported region bytes"
            );
            assert_eq!(
                fs::read(backup.join("region/r.0.0.mca")).unwrap(),
                b"old world region"
            );
            assert!(!recover_world_operations(&game).unwrap());
        }
    }

    #[test]
    fn rollback_refuses_running_published_world_without_moving_any_directory() {
        let fixture = Fixture::new();
        fixture.old_world();
        let archive = fixture.archive("");
        let game = fixture.game();
        let request = Request::Import {
            archive: archive.clone(),
            folder_name: "World".into(),
            replace: true,
        };
        let (operation, mut state) =
            begin(&game, "rollback-locked", request, "World", true).unwrap();
        extract(&archive, &operation.join("stage"), &control()).unwrap();
        state.staging_complete = true;
        state.committing = true;
        write_json(&operation.join("state.json"), &state).unwrap();
        let backup = prepare_backup(&game, &state).unwrap().unwrap();
        let target = game.join("saves/World");
        fs::rename(&target, &backup).unwrap();
        fs::rename(operation.join("stage"), &target).unwrap();
        fs::write(target.join("session.lock"), b"lock").unwrap();
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .open(target.join("session.lock"))
            .unwrap();
        held.try_lock().unwrap();
        assert!(rollback(&game, &operation, &mut state).is_err());
        assert_eq!(
            fs::read(target.join("region/r.0.0.mca")).unwrap(),
            b"imported region bytes"
        );
        assert_eq!(
            fs::read(backup.join("region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        assert!(!operation.join("stage").exists());
        drop(held);
        rollback(&game, &operation, &mut state).unwrap();
        assert_eq!(
            fs::read(target.join("region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        assert_eq!(
            fs::read(operation.join("stage/region/r.0.0.mca")).unwrap(),
            b"imported region bytes"
        );
    }

    #[test]
    fn active_session_lock_prevents_replacement_without_moving_world() {
        let fixture = Fixture::new();
        fixture.old_world();
        let lock = fixture.game().join("saves/World/session.lock");
        fs::write(&lock, b"lock").unwrap();
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock)
            .unwrap();
        held.try_lock().unwrap();
        let archive = fixture.archive("");
        assert!(
            runtime()
                .block_on(import_world_archive(
                    &archive,
                    &fixture.game(),
                    "World",
                    true,
                    "locked",
                    control()
                ))
                .is_err()
        );
        assert_eq!(
            fs::read(fixture.game().join("saves/World/region/r.0.0.mca")).unwrap(),
            b"old world region"
        );
        assert!(!fixture.game().join(BACKUPS).exists());
        drop(held);
        runtime()
            .block_on(import_world_archive(
                &archive,
                &fixture.game(),
                "World",
                true,
                "locked",
                control(),
            ))
            .unwrap();
    }
}

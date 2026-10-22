// SPDX-License-Identifier: MIT
//! Local launcher format inspection and independent gameplay-data migration.
//! Foreign launch arguments, accounts, hooks and runtime files are never used.
use crate::{InstallOutcome, InstallProgress};
use cobble_minecraft::DownloadSource;
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-import.json";
type Progress = Arc<dyn Fn(InstallProgress) + Send + Sync>;

#[derive(Clone, Serialize, Deserialize)]
pub struct LauncherContentRoot {
    pub id: String,
    pub label: String,
    pub has_saves: bool,
    pub directory: PathBuf,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LauncherImportCandidate {
    pub id: String,
    pub name: String,
    pub minecraft_version: Option<String>,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
    pub unsupported_reason: Option<String>,
    pub warnings: Vec<String>,
    pub content_roots: Vec<LauncherContentRoot>,
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

fn real_directory(path: &Path) -> Result<PathBuf, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err("导入目录必须是普通文件夹，不能是链接".into());
    }
    Ok(dunce::canonicalize(path)?)
}

fn leaf(name: &str) -> Result<(), Error> {
    if SafeRelativePath::parse(name)?.as_str().contains('/') {
        return Err("无效的导入文件名".into());
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err("版本元数据必须是小于 4 MiB 的普通 JSON 文件".into());
    }
    Ok(serde_json::from_reader(
        File::open(path)?.take(4 * 1024 * 1024 + 1),
    )?)
}

fn candidate(id: String, name: String) -> LauncherImportCandidate {
    LauncherImportCandidate {
        id,
        name,
        minecraft_version: None,
        loader_kind: None,
        loader_version: None,
        unsupported_reason: None,
        warnings: vec!["账号、Java、启动参数及启动钩子不会迁移，运行文件将重新下载".into()],
        content_roots: Vec::new(),
    }
}

fn supported_mc(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    let numeric = parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    numeric
        && match parts.as_slice() {
            ["1", minor, ..] => minor.parse::<u32>().is_ok_and(|v| v >= 17),
            [major, ..] => major.parse::<u32>().is_ok_and(|v| v >= 26),
            _ => false,
        }
}

fn runtime_supported(candidate: &mut LauncherImportCandidate) -> Result<(), Error> {
    let mc = candidate
        .minecraft_version
        .as_deref()
        .ok_or("无法确定 Minecraft 版本")?;
    leaf(mc)?;
    if !supported_mc(mc) {
        return Err("仅支持 Minecraft 1.17 及以后的正式版".into());
    }
    if let Some(kind) = candidate.loader_kind.as_deref() {
        let version = candidate
            .loader_version
            .as_deref()
            .ok_or("缺少加载器版本")?;
        leaf(version)?;
        match kind {
            "fabric" => {}
            "forge" | "neoforge" if mc == "1.21.1" => {}
            "forge" | "neoforge" => return Err("Forge / NeoForge 暂仅支持 Minecraft 1.21.1".into()),
            _ => return Err("暂不支持这个加载器".into()),
        }
    }
    Ok(())
}

fn set_loader(
    candidate: &mut LauncherImportCandidate,
    kind: &str,
    version: &str,
) -> Result<(), Error> {
    if let Some(old) = &candidate.loader_kind
        && (old != kind || candidate.loader_version.as_deref() != Some(version))
    {
        return Err("版本含有多个或冲突的加载器".into());
    }
    candidate.loader_kind = Some(kind.into());
    candidate.loader_version = Some(version.into());
    Ok(())
}

fn allowed(name: &str, include_saves: bool) -> bool {
    matches!(
        name,
        "mods"
            | "config"
            | "defaultconfigs"
            | "resourcepacks"
            | "shaderpacks"
            | "scripts"
            | "kubejs"
            | "schematics"
            | "schematic"
            | "journeymap"
            | "XaeroWaypoints"
            | "XaeroWorldMap"
            | "voxelmap"
            | "servers.dat"
            | "servers.dat_old"
    ) || (name.starts_with("options") && name.ends_with(".txt"))
        || (include_saves && name == "saves")
}

fn has_content(path: &Path) -> Result<bool, Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| allowed(name, true))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn content_root(id: &str, label: &str, path: &Path) -> Result<LauncherContentRoot, Error> {
    let directory = real_directory(path)?;
    let has_saves = match fs::symlink_metadata(directory.join("saves")) {
        Ok(metadata) => metadata.is_dir() && !linked(&metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    Ok(LauncherContentRoot {
        id: id.into(),
        label: label.into(),
        has_saves,
        directory,
    })
}

fn inspect_standard(root: &Path, id: &str) -> LauncherImportCandidate {
    let mut result = candidate(format!("version-{id}"), id.into());
    let inspection = (|| -> Result<(), Error> {
        leaf(id)?;
        let versions = real_directory(&root.join("versions"))?;
        let isolated = real_directory(&versions.join(id))?;
        let mut current = id.to_owned();
        let mut seen = BTreeSet::new();
        let mut nonvanilla_main = false;
        for depth in 0..16 {
            leaf(&current)?;
            if !seen.insert(current.clone()) {
                return Err("版本继承存在循环".into());
            }
            let folder = real_directory(&versions.join(&current))?;
            let value = read_json(&folder.join(format!("{current}.json")))?;
            if value["mainClass"]
                .as_str()
                .is_some_and(|main| main != "net.minecraft.client.main.Main")
            {
                nonvanilla_main = true;
            }
            for library in value["libraries"].as_array().into_iter().flatten() {
                let Some(name) = library["name"].as_str() else {
                    continue;
                };
                let parts: Vec<_> = name.split(':').collect();
                match parts.as_slice() {
                    ["net.fabricmc", "fabric-loader", version, ..] => {
                        set_loader(&mut result, "fabric", version)?
                    }
                    ["net.minecraftforge", "forge", version, ..] => {
                        let (mc, loader) = version.split_once('-').ok_or("无法解析 Forge 版本")?;
                        result.minecraft_version.get_or_insert_with(|| mc.into());
                        set_loader(&mut result, "forge", loader)?;
                    }
                    ["net.neoforged", "neoforge", version, ..] => {
                        set_loader(&mut result, "neoforge", version)?
                    }
                    [group, ..]
                        if group.starts_with("org.quiltmc")
                            || group.to_ascii_lowercase().contains("optifine")
                            || group.contains("liteloader") =>
                    {
                        return Err("暂不支持 Quilt、OptiFine 或 LiteLoader 版本迁移".into());
                    }
                    _ => {}
                }
            }
            match value["inheritsFrom"].as_str() {
                Some(parent) => current = parent.to_owned(),
                None => {
                    let base = value["id"].as_str().unwrap_or(&current);
                    if let Some(old) = &result.minecraft_version
                        && old != base
                    {
                        return Err("Minecraft 继承版本与加载器不一致".into());
                    }
                    result.minecraft_version = Some(base.into());
                    break;
                }
            }
            if depth == 15 {
                return Err("版本继承超过 16 层".into());
            }
        }
        if nonvanilla_main && result.loader_kind.is_none() {
            return Err("无法识别这个版本的自定义加载器".into());
        }
        runtime_supported(&mut result)?;
        if has_content(&isolated)? {
            result
                .content_roots
                .push(content_root("isolated", "版本独立目录", &isolated)?);
        }
        if has_content(root)? || result.content_roots.is_empty() {
            result
                .content_roots
                .push(content_root("shared", "共享游戏目录", root)?);
        }
        if result.content_roots.len() > 1 {
            result
                .warnings
                .push("独立目录与共享目录均有游戏内容，请选择实际使用的目录".into());
        }
        Ok(())
    })();
    if let Err(error) = inspection {
        result.unsupported_reason = Some(error.to_string());
    }
    result
}

fn inspect_prism(path: &Path) -> LauncherImportCandidate {
    let id = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut result = candidate(format!("instance-{id}"), id);
    let inspection = (|| -> Result<(), Error> {
        let root = real_directory(path)?;
        let pack = read_json(&root.join("mmc-pack.json"))?;
        let components = pack["components"]
            .as_array()
            .ok_or("实例缺少 components 元数据")?;
        for component in components {
            let uid = component["uid"].as_str().ok_or("实例组件缺少 uid")?;
            let version = component["version"].as_str().unwrap_or("");
            match uid {
                "net.minecraft" => {
                    if result.minecraft_version.replace(version.into()).is_some() {
                        return Err("实例包含重复 Minecraft 组件".into());
                    }
                }
                "net.fabricmc.fabric-loader" => set_loader(&mut result, "fabric", version)?,
                "net.minecraftforge" => set_loader(&mut result, "forge", version)?,
                "net.neoforged" => set_loader(&mut result, "neoforge", version)?,
                "net.fabricmc.intermediary"
                | "org.lwjgl"
                | "org.lwjgl3"
                | "net.minecraft.java"
                | "net.adoptium.java"
                | "com.azul.java"
                | "com.ibm.java" => {}
                _ => return Err(format!("暂不支持实例组件：{uid}").into()),
            }
        }
        runtime_supported(&mut result)?;
        let cfg = root.join("instance.cfg");
        if cfg.try_exists()? {
            let metadata = fs::symlink_metadata(&cfg)?;
            if linked(&metadata) || !metadata.is_file() || metadata.len() > 1024 * 1024 {
                return Err("实例名称设置不是普通文件".into());
            }
            let bytes = fs::read(cfg)?;
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Some(name) = line
                    .trim()
                    .strip_prefix("name=")
                    .filter(|s| !s.trim().is_empty())
                {
                    result.name = name.trim().chars().take(256).collect();
                    break;
                }
            }
        }
        for (id, name) in [("dot-minecraft", ".minecraft"), ("minecraft", "minecraft")] {
            let folder = root.join(name);
            if folder.try_exists()? {
                result.content_roots.push(content_root(id, name, &folder)?);
            }
        }
        if result.content_roots.is_empty() {
            result.content_roots.push(content_root(
                "instance",
                "实例目录（无独立内容目录）",
                &root,
            )?);
        }
        if result.content_roots.len() > 1 {
            result
                .warnings
                .push("实例中有两个内容目录，请选择实际使用的目录".into());
        }
        if root.join("patches").try_exists()? {
            result
                .warnings
                .push("自定义 patches 不会迁移，仅重新安装已识别的官方运行版本".into());
        }
        Ok(())
    })();
    if let Err(error) = inspection {
        result.unsupported_reason = Some(error.to_string());
    }
    result
}

/// Inspect direct local format files only; this operation never accesses network.
pub fn inspect_launcher_import(root: &Path) -> Result<Vec<LauncherImportCandidate>, Error> {
    let root = real_directory(root)?;
    let mut candidates = Vec::new();
    if root.join("mmc-pack.json").try_exists()? {
        candidates.push(inspect_prism(&root));
        return Ok(candidates);
    }
    let standard = if root.join("versions").try_exists()? {
        root.clone()
    } else {
        root.join(".minecraft")
    };
    if standard.join("versions").try_exists()? {
        real_directory(&standard)?;
        let versions = real_directory(&standard.join("versions"))?;
        for entry in fs::read_dir(versions)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if !linked(&metadata) && metadata.is_dir() {
                let id = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "版本名称必须是 UTF-8")?;
                if entry.path().join(format!("{id}.json")).try_exists()? {
                    candidates.push(inspect_standard(&standard, &id));
                }
            }
        }
    }
    let instances = if root.join("instances").try_exists()? {
        real_directory(&root.join("instances"))?
    } else {
        root.clone()
    };
    for entry in fs::read_dir(instances)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !linked(&metadata)
            && metadata.is_dir()
            && entry.path().join("mmc-pack.json").try_exists()?
        {
            candidates.push(inspect_prism(&entry.path()));
        }
    }
    candidates.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(candidates)
}

#[derive(Serialize, Deserialize)]
struct ImportState {
    minecraft_version: String,
    loader_kind: Option<String>,
    loader_version: Option<String>,
    source: PathBuf,
    content_id: String,
    include_saves: bool,
    entries: Vec<String>,
    staging_complete: bool,
    content_complete: bool,
    complete: bool,
}

const STAGING: &str = ".cobble-import-data";

fn world_locks(root: &Path, include_saves: bool) -> Result<Vec<File>, Error> {
    let mut locks = Vec::new();
    if !include_saves || !root.join("saves").try_exists()? {
        return Ok(locks);
    }
    let saves = real_directory(&root.join("saves"))?;
    for entry in fs::read_dir(saves)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) {
            return Err("世界目录不能是链接".into());
        }
        if metadata.is_dir() {
            let path = entry.path().join("session.lock");
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if linked(&metadata) || !metadata.is_file() {
                        return Err("世界锁文件不能是链接或特殊文件".into());
                    }
                    let file = OpenOptions::new().read(true).write(true).open(path)?;
                    file.try_lock()
                        .map_err(|_| "世界正在使用中，请先关闭原启动器中的游戏")?;
                    locks.push(file);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(locks)
}

fn selected_entries(root: &Path, include_saves: bool) -> Result<Vec<String>, Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if allowed(name, include_saves) {
            leaf(name)?;
            entries.push(name.into());
        }
    }
    entries.sort();
    Ok(entries)
}

fn write_state(target: &Path, state: &ImportState, new: bool) -> Result<(), Error> {
    let path = target.join(MARKER);
    if new {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        serde_json::to_writer_pretty(&mut file, state)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        return Ok(());
    }
    // Only this one marker is replaced. Gameplay files are individually moved.
    let temporary = target.join(".cobble-import.json.tmp");
    if let Ok(metadata) = fs::symlink_metadata(&temporary) {
        if linked(&metadata) || !metadata.is_file() {
            return Err("导入临时标记无效".into());
        }
        fs::remove_file(&temporary)?;
    }
    let result = (|| -> Result<(), Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, state)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn copy_entry(
    source: &Path,
    target: &Path,
    world_root: bool,
    depth: usize,
    control: &crate::OperationControl,
) -> Result<(), Error> {
    control.check_cancelled()?;
    if depth > 128 {
        return Err("导入内容目录超过 128 层".into());
    }
    let before = fs::symlink_metadata(source)?;
    if linked(&before) {
        return Err("导入内容不能含链接或重解析点".into());
    }
    if before.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("导入文件名必须是 UTF-8")?;
            leaf(name)?;
            if world_root && name == "session.lock" {
                continue;
            }
            copy_entry(
                &entry.path(),
                &target.join(name),
                depth == 0 && source.file_name().is_some_and(|name| name == "saves"),
                depth + 1,
                control,
            )?;
        }
    } else if before.is_file() {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        let mut input = File::open(source)?;
        let mut buffer = [0; 64 * 1024];
        let mut copied = 0;
        loop {
            control.check_cancelled()?;
            let length = input.read(&mut buffer)?;
            if length == 0 {
                break;
            }
            output.write_all(&buffer[..length])?;
            copied += length as u64;
        }
        let after = fs::symlink_metadata(source)?;
        if linked(&after)
            || copied != before.len()
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err("复制期间源文件发生改变，请关闭原游戏后重试".into());
        }
    } else {
        return Err("导入内容含不支持的特殊文件".into());
    }
    Ok(())
}

#[cfg(test)]
fn copy_content(target: &Path, state: &mut ImportState, progress: &Progress) -> Result<(), Error> {
    copy_content_controlled(
        target,
        state,
        &crate::OperationControl::new(progress.clone()),
    )
}

fn copy_content_controlled(
    target: &Path,
    state: &mut ImportState,
    control: &crate::OperationControl,
) -> Result<(), Error> {
    control.check_cancelled()?;
    let progress = control.progress();
    let staging = target.join(STAGING);
    if !state.staging_complete {
        let source = real_directory(&state.source)?;
        let _locks = world_locks(&source, state.include_saves)?;
        if staging.try_exists()? {
            real_directory(&staging)?;
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir(&staging)?;
        let result = (|| -> Result<(), Error> {
            // Re-evaluate only root names after an interrupted copy. No content digests.
            state.entries = selected_entries(&source, state.include_saves)?;
            progress(InstallProgress {
                phase: "import-copy".into(),
                completed: 0,
                total: state.entries.len(),
                reused: 0,
            });
            for (index, name) in state.entries.iter().enumerate() {
                copy_entry(&source.join(name), &staging.join(name), false, 0, control)?;
                progress(InstallProgress {
                    phase: "import-copy".into(),
                    completed: index + 1,
                    total: state.entries.len(),
                    reused: 0,
                });
            }
            control.check_cancelled()?;
            state.staging_complete = true;
            write_state(target, state, false)?;
            Ok(())
        })();
        if result.is_err() {
            state.staging_complete = false;
            let _ = fs::remove_dir_all(&staging);
        }
        result?;
    }
    real_directory(&staging)?;
    for name in &state.entries {
        control.check_cancelled()?;
        leaf(name)?;
        if !allowed(name, state.include_saves) {
            return Err("导入标记含不支持的内容路径".into());
        }
        let staged = staging.join(name);
        let destination = target.join(name);
        let staged_metadata = fs::symlink_metadata(&staged);
        let destination_metadata = fs::symlink_metadata(&destination);
        match (staged_metadata, destination_metadata) {
            (Ok(metadata), Err(error)) if error.kind() == io::ErrorKind::NotFound => {
                if linked(&metadata) {
                    return Err("暂存内容不能是链接".into());
                }
                // Operation lease keeps publication exclusive. Never rename over an existing path.
                fs::rename(&staged, &destination)?;
            }
            (Err(error), Ok(metadata)) if error.kind() == io::ErrorKind::NotFound => {
                if linked(&metadata) {
                    return Err("已迁移的目标内容不能是链接".into());
                }
                // A prior attempt already moved this root. Preserve any later edits.
            }
            (Ok(_), Ok(_)) => return Err(format!("导入目标已有内容，未覆盖：{name}").into()),
            (Err(error), _) => return Err(error.into()),
            (_, Err(error)) => return Err(error.into()),
        }
    }
    control.check_cancelled()?;
    state.content_complete = true;
    write_state(target, state, false)?;
    // Empty owned staging only; never remove published/user files.
    fs::remove_dir(staging)?;
    Ok(())
}

fn read_state(target: &Path) -> Result<ImportState, Error> {
    let state: ImportState = serde_json::from_value(read_json(&target.join(MARKER))?)?;
    if !supported_mc(&state.minecraft_version) {
        return Err("导入标记的 Minecraft 版本不受支持".into());
    }
    leaf(&state.minecraft_version)?;
    match (
        state.loader_kind.as_deref(),
        state.loader_version.as_deref(),
    ) {
        (None, None) => {}
        (Some("fabric"), Some(version)) => leaf(version)?,
        (Some("forge" | "neoforge"), Some(version)) if state.minecraft_version == "1.21.1" => {
            leaf(version)?
        }
        _ => return Err("导入标记的加载器不受支持".into()),
    }
    Ok(state)
}

fn initialize_target(target: &Path, state: &ImportState) -> Result<(), Error> {
    fs::create_dir(target)?;
    let result = (|| -> Result<(), Error> {
        // Register the still-empty owned directory before import content makes
        // it nonempty. Subsequent runtime installation resumes this same base.
        drop(crate::install::prepare_target(
            target,
            &state.minecraft_version,
            false,
        )?);
        write_state(target, state, true)
    })();
    if result.is_err() {
        for name in [MARKER, ".cobble-install.json", ".cobble-install.json.tmp"] {
            let _ = fs::remove_file(target.join(name));
        }
        // Do not recursively delete the target: unrelated files must survive.
        let _ = fs::remove_dir(target);
    }
    result
}

/// Host owns the selected candidate and stopped-instance operation lease.
/// Existing targets can only resume this same marked import; no foreign runtime is used.
pub async fn import_launcher_game(
    candidate: &LauncherImportCandidate,
    content_id: &str,
    target: &Path,
    source: DownloadSource,
    include_saves: bool,
    progress: Progress,
) -> Result<InstallOutcome, Error> {
    import_launcher_game_controlled(
        candidate,
        content_id,
        target,
        source,
        include_saves,
        crate::OperationControl::new(progress),
    )
    .await
}

pub async fn import_launcher_game_controlled(
    candidate: &LauncherImportCandidate,
    content_id: &str,
    target: &Path,
    source: DownloadSource,
    include_saves: bool,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;

    if let Some(reason) = &candidate.unsupported_reason {
        return Err(reason.clone().into());
    }
    let mut candidate = candidate.clone();
    runtime_supported(&mut candidate)?;
    let content = candidate
        .content_roots
        .iter()
        .find(|root| root.id == content_id)
        .ok_or("请选择要迁移的内容目录")?
        .directory
        .clone();
    let target_owned = target.to_owned();
    let content_id = content_id.to_owned();
    let initialization_control = control.clone();
    let state = tokio::task::spawn_blocking(move || -> Result<ImportState, Error> {
        if target_owned.try_exists()? {
            real_directory(&target_owned)?;
            let state = read_state(&target_owned)?;
            if Some(&state.minecraft_version) != candidate.minecraft_version.as_ref()
                || state.loader_kind != candidate.loader_kind
                || state.loader_version != candidate.loader_version
                || state.source != content
                || state.content_id != content_id
                || state.include_saves != include_saves
            {
                return Err("目标属于另一个导入任务，未修改任何内容".into());
            }
            if state.complete {
                return Err("这个导入任务已经完成".into());
            }
            return Ok(state);
        }
        let content = real_directory(&content)?;
        let parent = real_directory(target_owned.parent().ok_or("导入目标没有父目录")?)?;
        let name = target_owned
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("导入目标名称无效")?;
        leaf(name)?;
        let target = parent.join(name);
        if target.starts_with(&content) || content.starts_with(&target) {
            return Err("导入目标必须位于源目录之外".into());
        }
        let state = ImportState {
            minecraft_version: candidate.minecraft_version.unwrap(),
            loader_kind: candidate.loader_kind,
            loader_version: candidate.loader_version,
            source: content,
            content_id,
            include_saves,
            entries: Vec::new(),
            staging_complete: false,
            content_complete: false,
            complete: false,
        };
        initialization_control.check_cancelled()?;
        initialize_target(&target, &state)?;
        Ok(state)
    })
    .await??;
    finish_import(target, source, state, control).await
}

pub async fn resume_launcher_import(
    target: &Path,
    source: DownloadSource,
    progress: Progress,
) -> Result<InstallOutcome, Error> {
    resume_launcher_import_controlled(target, source, crate::OperationControl::new(progress)).await
}

pub async fn resume_launcher_import_controlled(
    target: &Path,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;

    let target = real_directory(target)?;
    let state = read_state(&target)?;
    if state.complete {
        return Err("这个导入任务已经完成".into());
    }
    finish_import(&target, source, state, control).await
}

async fn finish_import(
    target: &Path,
    source: DownloadSource,
    mut state: ImportState,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;
    if !state.content_complete {
        let directory = target.to_owned();
        let copy_control = control.clone();
        state = tokio::task::spawn_blocking(move || -> Result<ImportState, Error> {
            real_directory(&directory)?;
            copy_content_controlled(&directory, &mut state, &copy_control)?;
            Ok(state)
        })
        .await??;
    }
    let outcome = match (
        state.loader_kind.as_deref(),
        state.loader_version.as_deref(),
    ) {
        (None, None) => {
            crate::install_game_controlled(
                &state.minecraft_version,
                target,
                source,
                control.clone(),
            )
            .await?
        }
        (Some("fabric"), Some(loader)) => {
            crate::install_fabric_controlled(
                &state.minecraft_version,
                loader,
                target,
                source,
                control.clone(),
            )
            .await?
        }
        (Some(kind @ ("forge" | "neoforge")), Some(loader)) => {
            let kind = if kind == "forge" {
                crate::ForgeKind::Forge
            } else {
                crate::ForgeKind::NeoForge
            };
            crate::install_forge_controlled(
                &state.minecraft_version,
                loader,
                kind,
                target,
                source,
                control.clone(),
            )
            .await?
        }
        _ => return Err("不支持这个导入运行版本".into()),
    };
    control.check_cancelled()?;
    state.complete = true;
    write_state(target, &state, false)?;
    Ok(outcome)
}

pub(crate) fn ensure_import_complete(directory: &Path) -> Result<(), Error> {
    let path = directory.join(MARKER);
    if !path.try_exists()? {
        return Ok(());
    }
    let state = read_state(directory)?;
    if !state.complete {
        return Err("游戏导入尚未完成，请先重试导入".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-foreign-import-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, path: &str, bytes: &[u8]) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fn json(&self, path: &str, value: Value) {
            self.write(path, &serde_json::to_vec(&value).unwrap());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn progress() -> Progress {
        Arc::new(|_| {})
    }
    fn state(source: &Path, include_saves: bool) -> ImportState {
        ImportState {
            minecraft_version: "1.21.1".into(),
            loader_kind: None,
            loader_version: None,
            source: dunce::canonicalize(source).unwrap(),
            content_id: "shared".into(),
            include_saves,
            entries: Vec::new(),
            staging_complete: false,
            content_complete: false,
            complete: false,
        }
    }

    #[test]
    fn local_inheritance_finds_fabric_and_keeps_shared_and_isolated_choices_explicit() {
        let f = Fixture::new();
        f.json(
            ".minecraft/versions/1.21.1/1.21.1.json",
            json!({"id":"1.21.1","mainClass":"net.minecraft.client.main.Main"}),
        );
        f.json(".minecraft/versions/Fabric/Fabric.json", json!({"id":"Fabric","inheritsFrom":"1.21.1","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.14","url":"https://ignored.invalid"}],"arguments":{"jvm":["-Dforeign=ignored"]},"mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient"}));
        f.write(".minecraft/options.txt", b"shared");
        f.write(".minecraft/versions/Fabric/mods/example.jar", b"isolated");
        f.write(".minecraft/versions/Fabric/saves/World/level.dat", b"world");
        let candidates = inspect_launcher_import(&f.0).unwrap();
        let fabric = candidates.iter().find(|c| c.name == "Fabric").unwrap();
        assert!(
            fabric.unsupported_reason.is_none(),
            "{:?}",
            fabric.unsupported_reason
        );
        assert_eq!(fabric.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(fabric.loader_kind.as_deref(), Some("fabric"));
        assert_eq!(fabric.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(fabric.content_roots.len(), 2);
        assert!(
            fabric
                .content_roots
                .iter()
                .find(|r| r.id == "isolated")
                .unwrap()
                .has_saves
        );
        assert!(
            fabric
                .warnings
                .iter()
                .any(|warning| warning.contains("请选择"))
        );
        let restored: LauncherImportCandidate =
            serde_json::from_slice(&serde_json::to_vec(fabric).unwrap()).unwrap();
        assert_eq!(
            restored.content_roots[0].directory,
            fabric.content_roots[0].directory
        );
    }

    #[test]
    fn prism_component_format_supports_both_roots_and_rejects_unknown_loaders() {
        let f = Fixture::new();
        f.json("instances/Fabric/mmc-pack.json", json!({"formatVersion":1,"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.14"},{"uid":"org.lwjgl3","version":"3.3.3"}]}));
        f.write(
            "instances/Fabric/instance.cfg",
            b"[General]\nname=My Fabric Game\nPreLaunchCommand=never run\n",
        );
        f.write("instances/Fabric/.minecraft/options.txt", b"one");
        f.write("instances/Fabric/minecraft/options.txt", b"two");
        f.json("instances/Quilt/mmc-pack.json", json!({"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"org.quiltmc.quilt-loader","version":"0.28.0"}]}));
        f.json("instances/OldForge/mmc-pack.json", json!({"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.minecraftforge","version":"47.4.0"}]}));
        let candidates = inspect_launcher_import(&f.0).unwrap();
        let fabric = candidates
            .iter()
            .find(|c| c.name == "My Fabric Game")
            .unwrap();
        assert!(fabric.unsupported_reason.is_none());
        assert_eq!(fabric.content_roots.len(), 2);
        assert!(
            candidates
                .iter()
                .find(|c| c.name == "Quilt")
                .unwrap()
                .unsupported_reason
                .is_some()
        );
        assert!(
            candidates
                .iter()
                .find(|c| c.name == "OldForge")
                .unwrap()
                .unsupported_reason
                .as_deref()
                .unwrap()
                .contains("1.21.1")
        );
    }

    #[test]
    fn inheritance_cycles_missing_parents_traversal_and_unknown_main_class_remain_visible() {
        let f = Fixture::new();
        f.json(
            "versions/Cycle/Cycle.json",
            json!({"id":"Cycle","inheritsFrom":"Cycle"}),
        );
        f.json(
            "versions/Missing/Missing.json",
            json!({"id":"Missing","inheritsFrom":"1.21.1"}),
        );
        f.json(
            "versions/Escape/Escape.json",
            json!({"id":"Escape","inheritsFrom":"../outside"}),
        );
        f.json(
            "versions/Unknown/Unknown.json",
            json!({"id":"1.21.1","mainClass":"unknown.Loader"}),
        );
        let candidates = inspect_launcher_import(&f.0).unwrap();
        assert_eq!(candidates.len(), 4);
        assert!(candidates.iter().all(|c| c.unsupported_reason.is_some()));
        assert!(
            candidates
                .iter()
                .find(|c| c.name == "Cycle")
                .unwrap()
                .unsupported_reason
                .as_deref()
                .unwrap()
                .contains("循环")
        );
    }

    #[test]
    fn staging_copies_independent_gameplay_bytes_only_and_saves_require_opt_in() {
        let f = Fixture::new();
        for (name, bytes) in [
            ("source/mods/mod.jar", b"jar".as_slice()),
            ("source/config/mod.json", b"config"),
            ("source/optionsof.txt", b"options"),
            ("source/saves/World/level.dat", b"world"),
            ("source/saves/World/session.lock", b"lock"),
            ("source/accounts.json", b"secret"),
            ("source/libraries/library.jar", b"runtime"),
            ("source/instance.cfg", b"hook"),
        ] {
            f.write(name, bytes);
        }
        for name in ["target", "with-worlds"] {
            fs::create_dir(f.0.join(name)).unwrap();
        }
        let target = f.0.join("target");
        let mut first = state(&f.0.join("source"), false);
        write_state(&target, &first, true).unwrap();
        copy_content(&target, &mut first, &progress()).unwrap();
        assert!(first.content_complete);
        assert!(ensure_import_complete(&target).is_err());
        assert_eq!(fs::read(target.join("mods/mod.jar")).unwrap(), b"jar");
        for name in [
            "saves",
            "accounts.json",
            "libraries",
            "instance.cfg",
            STAGING,
        ] {
            assert!(!target.join(name).exists());
        }
        fs::write(target.join("mods/mod.jar"), b"changed").unwrap();
        assert_eq!(fs::read(f.0.join("source/mods/mod.jar")).unwrap(), b"jar");
        let target = f.0.join("with-worlds");
        let mut second = state(&f.0.join("source"), true);
        write_state(&target, &second, true).unwrap();
        copy_content(&target, &mut second, &progress()).unwrap();
        assert_eq!(
            fs::read(target.join("saves/World/level.dat")).unwrap(),
            b"world"
        );
        assert!(!target.join("saves/World/session.lock").exists());
        assert_eq!(
            fs::read(f.0.join("source/saves/World/session.lock")).unwrap(),
            b"lock"
        );
        second.complete = true;
        write_state(&target, &second, false).unwrap();
        ensure_import_complete(&target).unwrap();
    }

    #[test]
    fn initialized_import_content_can_resume_base_installer_but_cannot_launch() {
        let f = Fixture::new();
        f.write("source/mods/example.jar", b"local gameplay bytes");
        let target = f.0.join("target");
        let mut plan = state(&f.0.join("source"), false);
        initialize_target(&target, &plan).unwrap();
        copy_content(&target, &mut plan, &progress()).unwrap();
        drop(crate::install::prepare_target(&target, "1.21.1", false).unwrap());
        assert_eq!(
            fs::read(target.join("mods/example.jar")).unwrap(),
            b"local gameplay bytes"
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = runtime
            .block_on(crate::prepare_launch("1.21.1", &target))
            .unwrap_err();
        assert!(error.to_string().contains("导入尚未完成"));
        // Initialization never adopts an existing user directory.
        f.write("existing/user.txt", b"keep");
        assert!(initialize_target(&f.0.join("existing"), &plan).is_err());
        assert_eq!(fs::read(f.0.join("existing/user.txt")).unwrap(), b"keep");
    }

    #[test]
    fn partial_publication_resumes_without_overwriting_user_changes_or_target_collisions() {
        let f = Fixture::new();
        f.write("target/mods/changed.jar", b"user changed after move");
        f.write("target/.cobble-import-data/options.txt", b"staged options");
        let target = f.0.join("target");
        let mut plan = state(&target, false);
        plan.source = f.0.join("source-no-longer-present");
        plan.entries = vec!["mods".into(), "options.txt".into()];
        plan.staging_complete = true;
        write_state(&target, &plan, true).unwrap();
        copy_content(&target, &mut plan, &progress()).unwrap();
        assert_eq!(
            fs::read(target.join("mods/changed.jar")).unwrap(),
            b"user changed after move"
        );
        assert_eq!(
            fs::read(target.join("options.txt")).unwrap(),
            b"staged options"
        );
        fs::create_dir(target.join(STAGING)).unwrap();
        fs::write(
            target.join(STAGING).join("options.txt"),
            b"different staged options",
        )
        .unwrap();
        plan.content_complete = false;
        plan.entries = vec!["options.txt".into()];
        assert!(copy_content(&target, &mut plan, &progress()).is_err());
        assert_eq!(
            fs::read(target.join("options.txt")).unwrap(),
            b"staged options"
        );
        assert!(target.join(STAGING).join("options.txt").exists());
    }

    #[test]
    fn locked_world_refuses_copy_and_stale_partial_staging_is_recreated() {
        let f = Fixture::new();
        f.write("source/saves/World/session.lock", b"lock");
        f.write("source/saves/World/level.dat", b"world");
        let world_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.0.join("source/saves/World/session.lock"))
            .unwrap();
        world_lock.try_lock().unwrap();
        assert!(world_locks(&f.0.join("source"), true).is_err());
        assert!(world_locks(&f.0.join("source"), false).unwrap().is_empty());
        drop(world_lock);
        f.write("target/.cobble-import-data/stale.part", b"partial");
        let mut plan = state(&f.0.join("source"), true);
        write_state(&f.0.join("target"), &plan, true).unwrap();
        copy_content(&f.0.join("target"), &mut plan, &progress()).unwrap();
        assert!(!f.0.join("target/stale.part").exists());
        assert_eq!(
            fs::read(f.0.join("target/saves/World/level.dat")).unwrap(),
            b"world"
        );
    }

    #[test]
    fn cancelled_copy_removes_only_owned_staging_and_retries_from_existing_marker() {
        let fixture = Fixture::new();
        fixture.write("source/config/settings.json", b"configuration");
        fixture.write("source/mods/example.jar", b"mod bytes");
        let target = fixture.0.join("target");
        let mut plan = state(&fixture.0.join("source"), false);
        initialize_target(&target, &plan).unwrap();
        let token = Arc::new(std::sync::Mutex::new(
            None::<cobble_net::TransferCancellation>,
        ));
        let observed = token.clone();
        let control = crate::OperationControl::new(Arc::new(move |progress| {
            if progress.completed == 1 {
                observed.lock().unwrap().as_ref().unwrap().cancel();
            }
        }));
        *token.lock().unwrap() = Some(control.cancellation());
        assert!(
            copy_content_controlled(&target, &mut plan, &control)
                .unwrap_err()
                .is::<crate::OperationCancelled>()
        );
        assert!(!target.join(STAGING).exists());
        assert!(!target.join("config").exists());
        let mut retry = read_state(&target).unwrap();
        assert!(!retry.staging_complete && !retry.content_complete && !retry.complete);
        copy_content(&target, &mut retry, &progress()).unwrap();
        assert_eq!(
            fs::read(target.join("mods/example.jar")).unwrap(),
            b"mod bytes"
        );
        assert_eq!(
            fs::read(fixture.0.join("source/mods/example.jar")).unwrap(),
            b"mod bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn linked_content_is_refused_and_only_owned_staging_is_cleaned() {
        let f = Fixture::new();
        f.write("source/config/regular.json", b"config");
        f.write("outside/secret", b"secret");
        std::os::unix::fs::symlink(f.0.join("outside"), f.0.join("source/config/link")).unwrap();
        fs::create_dir(f.0.join("target")).unwrap();
        let mut plan = state(&f.0.join("source"), false);
        write_state(&f.0.join("target"), &plan, true).unwrap();
        assert!(copy_content(&f.0.join("target"), &mut plan, &progress()).is_err());
        assert!(!f.0.join("target").join(STAGING).exists());
        assert_eq!(fs::read(f.0.join("outside/secret")).unwrap(), b"secret");
    }

    #[cfg(windows)]
    #[test]
    fn windows_junction_content_is_refused_and_does_not_touch_external_files() {
        use std::os::windows::process::CommandExt;
        let f = Fixture::new();
        f.write("source/config/regular.json", b"config");
        f.write("outside/secret", b"secret");
        let junction = f.0.join("source/config/external");
        let result = std::process::Command::new("cmd")
            .args(["/D", "/C"])
            .raw_arg(format!(
                "mklink /J \"{}\" \"{}\"",
                junction.display(),
                f.0.join("outside").display()
            ))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::create_dir(f.0.join("target")).unwrap();
        let mut plan = state(&f.0.join("source"), false);
        write_state(&f.0.join("target"), &plan, true).unwrap();
        assert!(copy_content(&f.0.join("target"), &mut plan, &progress()).is_err());
        assert!(!f.0.join("target").join(STAGING).exists());
        assert_eq!(fs::read(f.0.join("outside/secret")).unwrap(), b"secret");
        fs::remove_dir(junction).unwrap();
    }
}

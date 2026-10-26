// SPDX-License-Identifier: MIT
//! One-time, host-authorized personal game data handoff.
//!
//! The caller holds a stopped-instance lease. Transport supplies verified candidate
//! directories; this module never reads Mesh events or authorizes renderer paths.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_store::SafeRelativePath;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MAX_FILES: usize = 100_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Selection {
    pub saves: bool,
    pub config: bool,
    pub options: bool,
    pub mods: bool,
    pub resourcepacks: bool,
    pub shaderpacks: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TransferFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TransferManifest {
    pub schema_version: u32,
    pub minecraft_version: String,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
    pub files: Vec<TransferFile>,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingFiles {
    pub paths: Vec<String>,
    pub bytes: u64,
}

fn fail(code: &'static str) -> Error {
    io::Error::other(code).into()
}
fn check(cancel: &AtomicBool) -> Result<(), Error> {
    if cancel.load(Ordering::Acquire) {
        Err(fail("TRANSFER_CANCELLED"))
    } else {
        Ok(())
    }
}
fn allowed(path: &str) -> bool {
    path == "options.txt"
        || [
            "saves/",
            "config/",
            "mods/",
            "resourcepacks/",
            "shaderpacks/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}
fn opened_root(path: &Path) -> Result<Dir, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(fail("TRANSFER_ROOT_INVALID"));
    }
    Ok(Dir::open_ambient_dir(path, cap_std::ambient_authority())?)
}
fn read_file(root: &Dir, path: &str) -> Result<cap_std::fs::File, Error> {
    let safe = SafeRelativePath::parse(path)?;
    let mut dir = root.try_clone()?;
    let parts = safe.as_str().split('/').collect::<Vec<_>>();
    for component in &parts[..parts.len() - 1] {
        dir = dir.open_dir_nofollow(component)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = dir.open_with(parts[parts.len() - 1], &options)?;
    if !file.metadata()?.is_file() {
        return Err(fail("TRANSFER_FILE_INVALID"));
    }
    Ok(file)
}
fn stamp(mut file: cap_std::fs::File, cancel: &AtomicBool) -> Result<(u64, [u8; 32]), Error> {
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 128 * 1024];
    loop {
        check(cancel)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| fail("TRANSFER_LIMIT"))?;
        if size > MAX_BYTES {
            return Err(fail("TRANSFER_LIMIT"));
        }
        digest.update(&buffer[..count]);
    }
    Ok((size, digest.finalize().into()))
}
fn collect_tree(
    root: &Dir,
    dir: &Dir,
    prefix: &str,
    files: &mut Vec<TransferFile>,
    cancel: &AtomicBool,
    depth: usize,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(fail("TRANSFER_LIMIT"));
    }
    for entry in dir.entries()? {
        check(cancel)?;
        let entry = entry?;
        let component = entry
            .file_name()
            .into_string()
            .map_err(|_| fail("TRANSFER_PATH_INVALID"))?;
        let path = format!("{prefix}/{component}");
        SafeRelativePath::parse(&path)?;
        let meta = dir.symlink_metadata(&component)?;
        if meta.file_type().is_symlink() {
            return Err(fail("TRANSFER_LINK_DENIED"));
        }
        if meta.is_dir() {
            collect_tree(
                root,
                &dir.open_dir_nofollow(&component)?,
                &path,
                files,
                cancel,
                depth + 1,
            )?;
        } else if meta.is_file() {
            // Minecraft's live world lock is ephemeral and is never handed off.
            if path.starts_with("saves/") && component == "session.lock" {
                continue;
            }
            let (bytes, sha256) = stamp(read_file(root, &path)?, cancel)?;
            files.push(TransferFile {
                path,
                bytes,
                sha256,
            });
            if files.len() > MAX_FILES {
                return Err(fail("TRANSFER_LIMIT"));
            }
        } else {
            return Err(fail("TRANSFER_SPECIAL_FILE"));
        }
    }
    Ok(())
}

/// The host must independently prove the selected game is stopped and lease it
/// until this scan ends. Only the three explicitly chosen personal-data scopes
/// are included; Java, accounts, tokens, logs and downloaded artifacts are not.
pub fn prepare(
    source: &Path,
    minecraft_version: &str,
    loader_kind: Option<&str>,
    loader_version: Option<&str>,
    selection: Selection,
    cancel: &AtomicBool,
) -> Result<TransferManifest, Error> {
    check(cancel)?;
    if minecraft_version.is_empty()
        || minecraft_version.len() > 128
        || !minecraft_version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._+".contains(&b))
    {
        return Err(fail("TRANSFER_VERSION_INVALID"));
    }
    if !selection.saves
        && !selection.config
        && !selection.options
        && !selection.mods
        && !selection.resourcepacks
        && !selection.shaderpacks
    {
        return Err(fail("TRANSFER_EMPTY_SELECTION"));
    }
    let root = opened_root(source)?;
    let mut files = Vec::new();
    for (name, selected) in [
        ("saves", selection.saves),
        ("config", selection.config),
        ("mods", selection.mods),
        ("resourcepacks", selection.resourcepacks),
        ("shaderpacks", selection.shaderpacks),
    ] {
        if !selected {
            continue;
        }
        match root.symlink_metadata(name) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
                collect_tree(
                    &root,
                    &root.open_dir_nofollow(name)?,
                    name,
                    &mut files,
                    cancel,
                    0,
                )?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(fail("TRANSFER_ROOT_INVALID")),
        }
    }
    if selection.options {
        match root.symlink_metadata("options.txt") {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                let (bytes, sha256) = stamp(read_file(&root, "options.txt")?, cancel)?;
                files.push(TransferFile {
                    path: "options.txt".into(),
                    bytes,
                    sha256,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(fail("TRANSFER_FILE_INVALID")),
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if files.is_empty() {
        return Err(fail("TRANSFER_EMPTY_SELECTION"));
    }
    let total_bytes = files
        .iter()
        .try_fold(0u64, |sum, file| sum.checked_add(file.bytes))
        .ok_or_else(|| fail("TRANSFER_LIMIT"))?;
    let manifest = TransferManifest {
        schema_version: 1,
        minecraft_version: minecraft_version.into(),
        loader_kind: loader_kind.map(str::to_owned),
        loader_version: loader_version.map(str::to_owned),
        files,
        total_bytes,
    };
    validate(&manifest)?;
    Ok(manifest)
}

pub fn validate(manifest: &TransferManifest) -> Result<(), Error> {
    if manifest.schema_version != 1
        || manifest.files.len() > MAX_FILES
        || manifest.total_bytes > MAX_BYTES
    {
        return Err(fail("TRANSFER_MANIFEST_INVALID"));
    }
    if manifest.minecraft_version.is_empty()
        || manifest.minecraft_version.len() > 128
        || !manifest
            .minecraft_version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._+".contains(&b))
    {
        return Err(fail("TRANSFER_MANIFEST_INVALID"));
    }
    match (
        manifest.loader_kind.as_deref(),
        manifest.loader_version.as_deref(),
    ) {
        (None, None) => {}
        (Some("fabric" | "forge" | "neoforge"), Some(version))
            if !version.is_empty()
                && version.len() <= 128
                && version
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._+".contains(&b)) => {}
        _ => return Err(fail("TRANSFER_MANIFEST_INVALID")),
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for file in &manifest.files {
        let safe = SafeRelativePath::parse(&file.path)?;
        if safe.as_str() != file.path
            || !allowed(&file.path)
            || file.path.split('/').any(|part| part == "session.lock")
            || !seen.insert(file.path.to_lowercase())
        {
            return Err(fail("TRANSFER_MANIFEST_INVALID"));
        }
        total = total
            .checked_add(file.bytes)
            .ok_or_else(|| fail("TRANSFER_LIMIT"))?;
    }
    if total != manifest.total_bytes {
        return Err(fail("TRANSFER_MANIFEST_INVALID"));
    }
    Ok(())
}

fn matching(root: &Dir, file: &TransferFile, cancel: &AtomicBool) -> bool {
    read_file(root, &file.path)
        .and_then(|input| stamp(input, cancel))
        .is_ok_and(|(size, digest)| size == file.bytes && digest == file.sha256)
}

/// Check a local verified-file candidate and a transport-owned incoming stage.
/// Missing entries stay explicit; neither candidate is trusted from its name.
pub fn missing(
    manifest: &TransferManifest,
    local: Option<&Path>,
    incoming: &Path,
    cancel: &AtomicBool,
) -> Result<MissingFiles, Error> {
    validate(manifest)?;
    let local = local.map(opened_root).transpose()?;
    let incoming = opened_root(incoming)?;
    let mut paths = Vec::new();
    let mut bytes = 0u64;
    for file in &manifest.files {
        check(cancel)?;
        if local
            .as_ref()
            .is_some_and(|root| matching(root, file, cancel))
            || matching(&incoming, file, cancel)
        {
            continue;
        }
        paths.push(file.path.clone());
        bytes += file.bytes;
    }
    Ok(MissingFiles { paths, bytes })
}

fn create_file(root: &Dir, path: &str) -> Result<cap_std::fs::File, Error> {
    let mut dir = root.try_clone()?;
    let parts = path.split('/').collect::<Vec<_>>();
    for component in &parts[..parts.len() - 1] {
        match dir.create_dir(component) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        dir = dir.open_dir_nofollow(component)?;
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    Ok(dir.open_with(parts[parts.len() - 1], &options)?)
}

/// Materialize only into a new, unregistered game-data directory. The caller
/// registers the game and prepares platform Java only after this returns Ok.
/// On error the partial directory remains unregistered for explicit recovery.
pub fn receive(
    manifest: &TransferManifest,
    local: Option<&Path>,
    incoming: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<u64, Error> {
    validate(manifest)?;
    let missing = missing(manifest, local, incoming, cancel)?;
    if !missing.paths.is_empty() {
        return Err(fail("TRANSFER_FILES_MISSING"));
    }
    let local_root = local.map(opened_root).transpose()?;
    let incoming_root = opened_root(incoming)?;
    let parent = target
        .parent()
        .ok_or_else(|| fail("TRANSFER_TARGET_INVALID"))?;
    let parent = fs::canonicalize(parent)?;
    let name = target
        .file_name()
        .ok_or_else(|| fail("TRANSFER_TARGET_INVALID"))?;
    if name == "." || name == ".." {
        return Err(fail("TRANSFER_TARGET_INVALID"));
    }
    let target = parent.join(name);
    for candidate in [Some(incoming), local].into_iter().flatten() {
        let candidate = fs::canonicalize(candidate)?;
        if target.starts_with(&candidate) || candidate.starts_with(&target) {
            return Err(fail("TRANSFER_TARGET_OVERLAP"));
        }
    }
    fs::create_dir(&target)?;
    let output = opened_root(&target)?;
    let mut reused = 0u64;
    for file in &manifest.files {
        check(cancel)?;
        let source = if let Some(root) = local_root
            .as_ref()
            .filter(|root| matching(root, file, cancel))
        {
            reused += file.bytes;
            root
        } else {
            &incoming_root
        };
        let mut input = read_file(source, &file.path)?;
        let mut written = create_file(&output, &file.path)?;
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 128 * 1024];
        loop {
            check(cancel)?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            size = size
                .checked_add(count as u64)
                .ok_or_else(|| fail("TRANSFER_LIMIT"))?;
            if size > file.bytes {
                return Err(fail("TRANSFER_VERIFY_FAILED"));
            }
            written.write_all(&buffer[..count])?;
            digest.update(&buffer[..count]);
        }
        written.sync_all()?;
        if size != file.bytes
            || <[u8; 32]>::from(digest.finalize()) != file.sha256
            || !matching(&output, file, cancel)
        {
            return Err(fail("TRANSFER_VERIFY_FAILED"));
        }
    }
    check(cancel)?;
    Ok(reused)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_data_only_and_reuse_then_complete() {
        let temp = std::env::temp_dir().join(format!(
            "cobble-transfer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&temp).unwrap();
        let source = temp.join("source");
        let local = temp.join("local");
        let incoming = temp.join("incoming");
        for root in [&source, &local, &incoming] {
            fs::create_dir_all(root.join("saves/World")).unwrap();
        }
        fs::create_dir_all(source.join("config")).unwrap();
        fs::write(source.join("saves/World/level.dat"), b"world").unwrap();
        fs::write(source.join("saves/World/session.lock"), b"lock").unwrap();
        fs::write(source.join("config/mod.toml"), b"config").unwrap();
        fs::write(source.join("options.txt"), b"options").unwrap();
        fs::write(source.join("accounts.json"), b"secret").unwrap();
        let cancel = AtomicBool::new(false);
        let manifest = prepare(
            &source,
            "1.21.1",
            Some("fabric"),
            Some("0.16.14"),
            Selection {
                saves: true,
                config: true,
                options: false,
                mods: false,
                resourcepacks: false,
                shaderpacks: false,
            },
            &cancel,
        )
        .unwrap();
        assert_eq!(
            manifest
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            vec!["config/mod.toml", "saves/World/level.dat"]
        );
        fs::write(local.join("saves/World/level.dat"), b"world").unwrap();
        assert_eq!(
            missing(&manifest, Some(&local), &incoming, &cancel)
                .unwrap()
                .paths,
            vec!["config/mod.toml"]
        );
        assert!(
            receive(
                &manifest,
                Some(&local),
                &incoming,
                &temp.join("not-created"),
                &cancel
            )
            .is_err()
        );
        fs::create_dir_all(incoming.join("config")).unwrap();
        fs::write(incoming.join("config/mod.toml"), b"config").unwrap();
        let target = temp.join("target");
        assert_eq!(
            receive(&manifest, Some(&local), &incoming, &target, &cancel).unwrap(),
            5
        );
        assert_eq!(
            fs::read(target.join("saves/World/level.dat")).unwrap(),
            b"world"
        );
        assert!(!target.join("accounts.json").exists());
        assert!(source.join("saves/World/level.dat").exists());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn untrusted_paths_and_changed_bytes_never_complete() {
        let temp = std::env::temp_dir().join(format!(
            "cobble-transfer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&temp).unwrap();
        let cancel = AtomicBool::new(false);
        let mut manifest = TransferManifest {
            schema_version: 1,
            minecraft_version: "1.21.1".into(),
            loader_kind: None,
            loader_version: None,
            files: vec![TransferFile {
                path: "../accounts.json".into(),
                bytes: 1,
                sha256: [0; 32],
            }],
            total_bytes: 1,
        };
        assert!(validate(&manifest).is_err());
        manifest.files[0].path = "saves/World/level.dat".into();
        validate(&manifest).unwrap();
        let incoming = temp.join("incoming");
        fs::create_dir_all(incoming.join("saves/World")).unwrap();
        fs::write(incoming.join("saves/World/level.dat"), b"changed").unwrap();
        assert_eq!(
            missing(&manifest, None, &incoming, &cancel).unwrap().paths,
            vec!["saves/World/level.dat"]
        );
        assert!(receive(&manifest, None, &incoming, &temp.join("target"), &cancel).is_err());
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn selected_mod_and_pack_files_keep_their_own_paths() {
        let temp = std::env::temp_dir().join(format!(
            "cobble-transfer-content-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = temp.join("source");
        for folder in ["mods", "resourcepacks", "shaderpacks", "logs"] {
            fs::create_dir_all(source.join(folder)).unwrap();
        }
        fs::write(source.join("mods/custom.jar"), b"own mod").unwrap();
        fs::write(source.join("resourcepacks/own.zip"), b"own resource pack").unwrap();
        fs::write(source.join("shaderpacks/own.zip"), b"own shader pack").unwrap();
        fs::write(source.join("logs/latest.log"), b"token-like fixture").unwrap();
        let manifest = prepare(
            &source,
            "1.21.1",
            Some("fabric"),
            Some("0.16.14"),
            Selection {
                saves: false,
                config: false,
                options: false,
                mods: true,
                resourcepacks: true,
                shaderpacks: true,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            manifest
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            [
                "mods/custom.jar",
                "resourcepacks/own.zip",
                "shaderpacks/own.zip"
            ]
        );
        assert_eq!(manifest.loader_kind.as_deref(), Some("fabric"));
        fs::remove_dir_all(temp).unwrap();
    }
}

// SPDX-License-Identifier: MIT
//! Verified independent instance copies. Caller holds the stopped-instance lease.
//! Source bytes are never deleted here, including after cancellation or publication failure.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
#[derive(Clone, Copy)]
struct Scan {
    selection: Option<bool>,
    entries: usize,
}
const INSTANCE_SCAN: Scan = Scan {
    selection: None,
    entries: 100_000,
};
const GAME_ROOT_SCAN: Scan = Scan {
    selection: None,
    entries: 1_000_000,
};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileStamp {
    pub bytes: u64,
    pub sha256: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub files: BTreeMap<PathBuf, FileStamp>,
    pub directories: Vec<PathBuf>,
    pub bytes: u64,
}
fn invalid(code: &str) -> io::Error {
    io::Error::other(code)
}
fn check(cancel: &AtomicBool) -> Result<(), Error> {
    if cancel.load(Ordering::Acquire) {
        Err(invalid("STORAGE_CANCELLED").into())
    } else {
        Ok(())
    }
}
fn linked(metadata: &fs::Metadata) -> bool {
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
fn root(path: &Path) -> Result<Dir, Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err(invalid("STORAGE_LINK_DENIED").into());
    }
    Ok(Dir::open_ambient_dir(path, cap_std::ambient_authority())?)
}
fn hash(mut file: cap_std::fs::File, cancel: &AtomicBool) -> Result<FileStamp, Error> {
    let mut digest = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; 128 * 1024];
    loop {
        check(cancel)?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(FileStamp {
        bytes,
        sha256: digest.finalize().into(),
    })
}
fn inventory_tree(
    dir: &Dir,
    relative: &Path,
    result: &mut Inventory,
    cancel: &AtomicBool,
    depth: usize,
    scan: Scan,
    entries: &mut usize,
) -> Result<(), Error> {
    if depth > 128 || result.files.len() > scan.entries {
        return Err(invalid("STORAGE_SCAN_LIMIT").into());
    }
    for entry in dir.entries()? {
        check(cancel)?;
        let entry = entry?;
        let name = entry.file_name();
        let path = relative.join(&name);
        *entries += 1;
        if *entries > scan.entries {
            return Err(invalid("STORAGE_SCAN_LIMIT").into());
        }
        if scan.selection.is_some_and(|include_saves| {
            let name = name.to_string_lossy();
            matches!(
                name.as_ref(),
                "logs"
                    | "crash-reports"
                    | ".cobble-logs"
                    | ".cobble-natives"
                    | ".cobble-installers"
                    | "temp"
                    | "tmp"
                    | "locks"
            ) || name.ends_with(".lock")
                || name.ends_with(".tmp")
                || name.ends_with(".part")
                || (depth == 0 && name == "saves" && !include_saves)
        }) {
            continue;
        }
        let metadata = dir.symlink_metadata(&name)?;
        // cap-std reports symlinks; Windows reparse directories are also rejected by nofollow open.
        if metadata.file_type().is_symlink() {
            return Err(invalid("STORAGE_LINK_DENIED").into());
        }
        if metadata.is_dir() {
            result.directories.push(path.clone());
            inventory_tree(
                &dir.open_dir_nofollow(&name)?,
                &path,
                result,
                cancel,
                depth + 1,
                scan,
                entries,
            )?;
        } else if metadata.is_file() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let stamp = hash(dir.open_with(&name, &options)?, cancel)?;
            result.bytes = result
                .bytes
                .checked_add(stamp.bytes)
                .ok_or_else(|| invalid("STORAGE_SIZE_OVERFLOW"))?;
            result.files.insert(path, stamp);
        } else {
            return Err(invalid("STORAGE_SPECIAL_FILE").into());
        }
    }
    Ok(())
}
pub fn inventory(path: &Path, cancel: &AtomicBool) -> Result<Inventory, Error> {
    inventory_selected(path, cancel, INSTANCE_SCAN)
}
/// Complete multi-instance root with a bounded one-million-entry inventory.
pub fn inventory_game_root(path: &Path, cancel: &AtomicBool) -> Result<Inventory, Error> {
    inventory_selected(path, cancel, GAME_ROOT_SCAN)
}
pub fn inventory_instance(
    path: &Path,
    include_saves: bool,
    cancel: &AtomicBool,
) -> Result<Inventory, Error> {
    inventory_selected(
        path,
        cancel,
        Scan {
            selection: Some(include_saves),
            ..INSTANCE_SCAN
        },
    )
}
fn inventory_selected(path: &Path, cancel: &AtomicBool, scan: Scan) -> Result<Inventory, Error> {
    let mut result = Inventory {
        files: BTreeMap::new(),
        directories: Vec::new(),
        bytes: 0,
    };
    inventory_tree(
        &root(path)?,
        Path::new(""),
        &mut result,
        cancel,
        0,
        scan,
        &mut 0,
    )?;
    result.directories.sort();
    Ok(result)
}
/// Size-only overview does not authorize deletion or migration.
pub fn size(path: &Path) -> Result<u64, Error> {
    size_bounded(path, INSTANCE_SCAN.entries)
}
pub fn size_game_root(path: &Path) -> Result<u64, Error> {
    size_bounded(path, GAME_ROOT_SCAN.entries)
}
fn size_bounded(path: &Path, limit: usize) -> Result<u64, Error> {
    fn visit(dir: &Dir, depth: usize, entries: &mut usize, limit: usize) -> Result<u64, Error> {
        if depth > 128 {
            return Err(invalid("STORAGE_SCAN_LIMIT").into());
        }
        let mut bytes = 0u64;
        for entry in dir.entries()? {
            let entry = entry?;
            *entries += 1;
            if *entries > limit {
                return Err(invalid("STORAGE_SCAN_LIMIT").into());
            }
            let name = entry.file_name();
            let metadata = dir.symlink_metadata(&name)?;
            if metadata.file_type().is_symlink() {
                return Err(invalid("STORAGE_LINK_DENIED").into());
            }
            let value = if metadata.is_dir() {
                visit(&dir.open_dir_nofollow(&name)?, depth + 1, entries, limit)?
            } else if metadata.is_file() {
                metadata.len()
            } else {
                return Err(invalid("STORAGE_SPECIAL_FILE").into());
            };
            bytes = bytes
                .checked_add(value)
                .ok_or_else(|| invalid("STORAGE_SIZE_OVERFLOW"))?;
        }
        Ok(bytes)
    }
    visit(&root(path)?, 0, &mut 0, limit)
}
/// Copies into a new exact target, verifies every file and rechecks source inventory.
/// Failure cleanup removes only the target directory created by this invocation.
pub fn copy_verified(
    source: &Path,
    target: &Path,
    expected: &Inventory,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), Error> {
    copy_selected(source, target, expected, cancel, progress, INSTANCE_SCAN)
}
pub fn copy_game_root_verified(
    source: &Path,
    target: &Path,
    expected: &Inventory,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), Error> {
    copy_selected(source, target, expected, cancel, progress, GAME_ROOT_SCAN)
}
pub fn copy_instance_verified(
    source: &Path,
    target: &Path,
    expected: &Inventory,
    include_saves: bool,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), Error> {
    copy_selected(
        source,
        target,
        expected,
        cancel,
        progress,
        Scan {
            selection: Some(include_saves),
            ..INSTANCE_SCAN
        },
    )
}
fn copy_selected(
    source: &Path,
    target: &Path,
    expected: &Inventory,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
    scan: Scan,
) -> Result<(), Error> {
    check(cancel)?;
    let source_path = dunce::canonicalize(source)?;
    let parent = dunce::canonicalize(
        target
            .parent()
            .ok_or_else(|| invalid("STORAGE_TARGET_INVALID"))?,
    )?;
    let target = parent.join(
        target
            .file_name()
            .ok_or_else(|| invalid("STORAGE_TARGET_INVALID"))?,
    );
    if target.starts_with(&source_path) || source_path.starts_with(&target) {
        return Err(invalid("STORAGE_TARGET_OVERLAP").into());
    }
    if inventory_selected(&source_path, cancel, scan)? != *expected {
        return Err(invalid("STORAGE_SOURCE_CHANGED").into());
    }
    fs::create_dir(&target)?;
    let result = (|| {
        let from = root(&source_path)?;
        let to = root(&target)?;
        let mut completed = 0;
        for directory in &expected.directories {
            check(cancel)?;
            to.create_dir(directory)?;
        }
        for (path, stamp) in &expected.files {
            check(cancel)?;
            let mut read = OpenOptions::new();
            read.read(true).follow(FollowSymlinks::No);
            let mut input = from.open_with(path, &read)?;
            let mut write = OpenOptions::new();
            write
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No);
            let mut output = to.open_with(path, &write)?;
            let mut buffer = [0; 128 * 1024];
            loop {
                check(cancel)?;
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
                completed += count as u64;
                progress(completed);
            }
            output.sync_all()?;
            drop(output);
            if hash(to.open_with(path, &read)?, cancel)? != *stamp {
                return Err(invalid("STORAGE_VERIFY_FAILED").into());
            }
        }
        if inventory_selected(&source_path, cancel, scan)? != *expected {
            return Err(invalid("STORAGE_SOURCE_CHANGED").into());
        }
        if inventory_selected(&target, cancel, scan)? != *expected {
            return Err(invalid("STORAGE_VERIFY_FAILED").into());
        }
        check(cancel)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&target);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    #[test]
    fn game_root_limits_reuse_the_engine_and_keep_instance_budget() {
        assert_eq!(INSTANCE_SCAN.entries, 100_000);
        assert_eq!(GAME_ROOT_SCAN.entries, 1_000_000);
        let f = Fixture::new();
        let source = f.0.join("source");
        let cancel = AtomicBool::new(false);
        assert!(
            inventory_selected(
                &source,
                &cancel,
                Scan {
                    entries: 2,
                    ..INSTANCE_SCAN
                }
            )
            .is_err()
        );
        assert!(size_bounded(&source, 2).is_err());
        let expected = inventory_game_root(&source, &cancel).unwrap();
        assert_eq!(size_game_root(&source).unwrap(), expected.bytes);
        let target = f.0.join("multi-game-root");
        copy_game_root_verified(&source, &target, &expected, &cancel, |_| {}).unwrap();
        assert_eq!(inventory_game_root(&target, &cancel).unwrap(), expected);
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-move-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("source/saves/world")).unwrap();
            fs::write(path.join("source/saves/world/level.dat"), b"world").unwrap();
            fs::write(path.join("source/options.txt"), b"options").unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn verified_copy_keeps_world_and_source_independent() {
        let f = Fixture::new();
        let cancel = AtomicBool::new(false);
        let source = f.0.join("source");
        let target = f.0.join("target");
        let expected = inventory(&source, &cancel).unwrap();
        copy_verified(&source, &target, &expected, &cancel, |_| {}).unwrap();
        assert_eq!(inventory(&target, &cancel).unwrap(), expected);
        fs::write(target.join("options.txt"), b"new").unwrap();
        assert_eq!(fs::read(source.join("options.txt")).unwrap(), b"options");
    }
    #[test]
    fn cancellation_and_changed_source_never_publish_or_destroy_source() {
        let f = Fixture::new();
        let cancel = AtomicBool::new(false);
        let source = f.0.join("source");
        let target = f.0.join("target");
        let expected = inventory(&source, &cancel).unwrap();
        let result = copy_verified(&source, &target, &expected, &cancel, |_| {
            cancel.store(true, Ordering::Release)
        });
        assert!(result.is_err());
        assert!(!target.exists());
        assert!(source.join("saves/world/level.dat").exists());
        cancel.store(false, Ordering::Release);
        fs::write(source.join("options.txt"), b"changed").unwrap();
        assert!(copy_verified(&source, &target, &expected, &cancel, |_| {}).is_err());
        assert!(!target.exists());
    }
    #[test]
    fn write_verification_failure_keeps_source_and_removes_only_new_copy() {
        let f = Fixture::new();
        let cancel = AtomicBool::new(false);
        let source = f.0.join("source");
        let target = f.0.join("target");
        let expected = inventory(&source, &cancel).unwrap();
        let result = copy_verified(&source, &target, &expected, &cancel, |_| {
            fs::write(target.join("options.txt"), b"fault").unwrap();
        });
        assert!(result.is_err());
        assert!(!target.exists());
        assert_eq!(fs::read(source.join("options.txt")).unwrap(), b"options");
    }
    #[test]
    fn trial_filter_omits_worlds_and_transients_but_complete_move_keeps_them() {
        let f = Fixture::new();
        let cancel = AtomicBool::new(false);
        let source = f.0.join("source");
        fs::create_dir(source.join("logs")).unwrap();
        fs::write(source.join("logs/latest.log"), b"log").unwrap();
        fs::write(source.join("pending.part"), b"partial").unwrap();
        let filtered = inventory_instance(&source, false, &cancel).unwrap();
        assert_eq!(filtered.files.len(), 1);
        let target = f.0.join("trial");
        copy_instance_verified(&source, &target, &filtered, false, &cancel, |_| {}).unwrap();
        assert!(!target.join("saves").exists());
        assert!(!target.join("logs").exists());
        assert_eq!(inventory(&source, &cancel).unwrap().files.len(), 4);
    }
    #[test]
    fn existing_target_and_nested_target_are_never_overwritten() {
        let f = Fixture::new();
        let cancel = AtomicBool::new(false);
        let source = f.0.join("source");
        let expected = inventory(&source, &cancel).unwrap();
        assert!(
            copy_verified(&source, &source.join("nested"), &expected, &cancel, |_| {}).is_err()
        );
        let target = f.0.join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"keep").unwrap();
        assert!(copy_verified(&source, &target, &expected, &cancel, |_| {}).is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"keep");
    }
}

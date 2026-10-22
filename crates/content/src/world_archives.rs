// SPDX-License-Identifier: MIT
//! Read-only, bounded single-world ZIP inspection. No extraction occurs here.
use crate::mrpack::Namespace;
use crate::{BoxError, LocalWorld, read_world_metadata};
use cobble_store::SafeRelativePath;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Seek},
    path::Path,
};

pub const MAX_WORLD_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct WorldArchivePreview {
    pub world: LocalWorld,
    pub suggested_folder_name: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

/// Host-only validated extraction positions, bound to the inspected ZIP handle.
pub struct WorldArchivePlan {
    pub preview: WorldArchivePreview,
    pub entries: Vec<WorldArchiveEntry>,
}
pub struct WorldArchiveEntry {
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub directory: bool,
}

pub fn preview_world_archive(path: &Path) -> Result<WorldArchivePreview, BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    let linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || metadata.file_attributes() & 0x400 != 0
    };
    if linked || !metadata.is_file() || metadata.len() > MAX_WORLD_ARCHIVE_BYTES {
        return Err("世界 ZIP 必须是小于 16 GiB 的普通文件".into());
    }
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    Ok(inspect_world_archive(
        &mut archive,
        path.file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Imported World"),
    )?
    .preview)
}

pub fn inspect_world_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    suggested_name: &str,
) -> Result<WorldArchivePlan, BoxError> {
    if archive.len() > MAX_ENTRIES {
        return Err("世界 ZIP 文件数量超过 50000".into());
    }
    archive.validate_header_consistency()?;
    let mut names = Namespace::default();
    let mut entries = Vec::new();
    let mut roots = Vec::new();
    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = std::str::from_utf8(entry.name_raw())?;
        let directory = entry.is_dir();
        if entry.encrypted() {
            return Err("暂不支持加密的世界 ZIP".into());
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != if directory { 0o040000 } else { 0o100000 } {
                return Err("世界 ZIP 不能包含链接或特殊文件".into());
            }
        }
        let path = name.strip_suffix('/').unwrap_or(name);
        let path = SafeRelativePath::parse(path)?;
        names.insert(path.as_str(), directory, Some(0))?;
        if entry.size() > MAX_FILE {
            return Err("世界 ZIP 中的单个文件超过 2 GiB".into());
        }
        total_bytes = total_bytes
            .checked_add(entry.size())
            .ok_or("世界 ZIP 解压大小无效")?;
        if total_bytes > MAX_WORLD_ARCHIVE_BYTES {
            return Err("世界 ZIP 解压后超过 16 GiB".into());
        }
        if !directory
            && (path.as_str() == "level.dat"
                || path.as_str().split('/').count() == 2 && path.as_str().ends_with("/level.dat"))
        {
            roots.push((
                index,
                path.as_str().strip_suffix("level.dat").unwrap().to_owned(),
            ));
        }
        entries.push(WorldArchiveEntry {
            index,
            path: path.as_str().into(),
            size: entry.size(),
            directory,
        });
    }
    let [(metadata_index, prefix)] = roots.as_slice() else {
        return Err(
            "请选择只包含一个世界的 ZIP：根目录或单层文件夹中必须有唯一的 level.dat".into(),
        );
    };
    let suggested = if prefix.is_empty() {
        suggested_name
    } else {
        prefix.trim_end_matches('/')
    };
    let suggested = if suggested.len() <= 200
        && !suggested.starts_with('.')
        && SafeRelativePath::parse(suggested).is_ok_and(|name| !name.as_str().contains('/'))
    {
        suggested
    } else {
        "Imported World"
    };
    let world = read_world_metadata(archive.by_index(*metadata_index)?, suggested);
    let mut selected = Vec::new();
    for mut entry in entries {
        if entry.directory && entry.path == prefix.trim_end_matches('/') {
            continue;
        }
        let path = entry
            .path
            .strip_prefix(prefix)
            .ok_or("世界 ZIP 中有世界文件夹之外的内容")?;
        if path.is_empty() {
            return Err("世界 ZIP 的根目录无效".into());
        }
        if path == "session.lock" {
            continue;
        }
        entry.path = path.into();
        selected.push(entry);
    }
    let file_count = selected.iter().filter(|entry| !entry.directory).count();
    let total_bytes = selected.iter().map(|entry| entry.size).sum();
    Ok(WorldArchivePlan {
        preview: WorldArchivePreview {
            world,
            suggested_folder_name: suggested.into(),
            file_count,
            total_bytes,
        },
        entries: selected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::{Cursor, Write};

    fn level() -> Vec<u8> {
        let bytes = fastnbt::to_bytes(&fastnbt::nbt!({"Data":{"LevelName":"Archive world","Version":{"Name":"1.21.1"},"GameType":1i32,"LastPlayed":123i64}})).unwrap();
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(&bytes).unwrap();
        gzip.finish().unwrap()
    }
    fn zip(entries: &[(&str, &[u8])]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip::ZipArchive::new(Cursor::new(zip.finish().unwrap().into_inner())).unwrap()
    }
    #[test]
    fn root_and_single_enclosing_folder_share_bounded_gzip_metadata() {
        let level = level();
        for prefix in ["", "Original world/"] {
            let level_path = format!("{prefix}level.dat");
            let region_path = format!("{prefix}region/r.0.0.mca");
            let lock_path = format!("{prefix}session.lock");
            let mut archive = zip(&[
                (&level_path, &level),
                (&region_path, b"region bytes"),
                (&lock_path, b"stale lock"),
            ]);
            let plan = inspect_world_archive(&mut archive, "selected-archive").unwrap();
            assert_eq!(plan.preview.world.name, "Archive world");
            assert_eq!(
                plan.preview.world.minecraft_version.as_deref(),
                Some("1.21.1")
            );
            assert!(!plan.preview.world.metadata_error);
            assert_eq!(plan.preview.file_count, 2);
            assert!(
                plan.entries
                    .iter()
                    .any(|entry| entry.path == "region/r.0.0.mca")
            );
            assert!(
                !plan
                    .entries
                    .iter()
                    .any(|entry| entry.path == "session.lock")
            );
            assert_eq!(
                plan.preview.suggested_folder_name,
                if prefix.is_empty() {
                    "selected-archive"
                } else {
                    "Original world"
                }
            );
        }
        let preview = inspect_world_archive(&mut zip(&[("level.dat", b"bad gzip")]), "Broken")
            .unwrap()
            .preview;
        assert!(preview.world.metadata_error);
        assert_eq!(preview.world.name, "Broken");
    }
    #[test]
    fn multiple_worlds_foreign_roots_traversal_and_case_aliases_are_rejected() {
        let level = level();
        for entries in [
            vec![
                ("One/level.dat", level.as_slice()),
                ("Two/level.dat", level.as_slice()),
            ],
            vec![
                ("World/level.dat", level.as_slice()),
                ("outside.txt", b"outside"),
            ],
            vec![("level.dat", level.as_slice()), ("../escape", b"escape")],
            vec![
                ("level.dat", level.as_slice()),
                ("region/a.mca", b"a"),
                ("REGION/b.mca", b"b"),
            ],
            vec![("deeper/World/level.dat", level.as_slice())],
        ] {
            assert!(inspect_world_archive(&mut zip(&entries), "World").is_err());
        }
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("level.dat", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&level).unwrap();
        writer
            .add_symlink(
                "external",
                "../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        let mut archive =
            zip::ZipArchive::new(Cursor::new(writer.finish().unwrap().into_inner())).unwrap();
        assert!(inspect_world_archive(&mut archive, "World").is_err());
    }
}

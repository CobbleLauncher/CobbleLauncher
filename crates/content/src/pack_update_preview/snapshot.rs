// SPDX-License-Identifier: MIT
//! Bounded local archive metadata snapshot. Reads final client override bytes only;
//! never extracts files or downloads declared artifacts.
use super::*;
use crate::{BoxError, read_mrpack_file};
use sha2::{Digest, Sha512};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const MAX_OVERRIDE: u64 = 64 * 1024 * 1024;
const MAX_OVERRIDES: u64 = 512 * 1024 * 1024;
pub fn snapshot_mrpack(
    path: &Path,
    selected_optional: &[String],
) -> Result<PackManifestSnapshot, BoxError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Pack snapshot requires a regular archive".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Pack snapshot cannot use a reparse point".into());
        }
    }
    snapshot_mrpack_file(File::open(path)?, selected_optional)
}
pub fn snapshot_mrpack_file(
    mut source: File,
    selected_optional: &[String],
) -> Result<PackManifestSnapshot, BoxError> {
    let pack = read_mrpack_file(source.try_clone()?)?;
    let selected: std::collections::BTreeSet<_> =
        selected_optional.iter().map(String::as_str).collect();
    if selected.len() != selected_optional.len()
        || selected
            .iter()
            .any(|path| !pack.files.iter().any(|f| f.optional && f.path == *path))
    {
        return Err("Unknown or duplicate optional pack file".into());
    }
    let mut files: std::collections::BTreeMap<String, PackManifestFile> = pack
        .files
        .iter()
        .filter(|f| !f.optional || selected.contains(f.path.as_str()))
        .map(|f| {
            (
                f.path.clone(),
                PackManifestFile {
                    identity: f.path.clone(),
                    version_id: None,
                    path: f.path.clone(),
                    size_bytes: f.file_size,
                    sha1: Some(f.sha1.clone()),
                    sha512: Some(f.sha512.clone()),
                },
            )
        })
        .collect();
    let mut overrides = std::collections::BTreeMap::new();
    for entry in &pack.overrides {
        overrides.insert(entry.path.as_str(), entry);
    }
    let total = overrides
        .values()
        .try_fold(0u64, |sum, entry| {
            sum.checked_add(entry.size)
                .filter(|sum| *sum <= MAX_OVERRIDES && entry.size <= MAX_OVERRIDE)
        })
        .ok_or("Pack override preview exceeds read budget")?;
    let _ = total;
    source.seek(SeekFrom::Start(0))?;
    let mut archive = zip::ZipArchive::new(source)?;
    for entry in overrides.values() {
        let mut input = archive.by_index(entry.entry_index)?;
        if input.size() != entry.size {
            return Err("Pack archive changed while inspecting".into());
        }
        let mut hash = Sha512::new();
        let mut buffer = [0u8; 65536];
        let mut read = 0u64;
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > entry.size || read > MAX_OVERRIDE {
                return Err("Pack override exceeds read budget".into());
            }
            hash.update(&buffer[..count]);
        }
        if read != entry.size {
            return Err("Pack override size mismatch".into());
        }
        files.insert(
            entry.path.clone(),
            PackManifestFile {
                identity: entry.path.clone(),
                version_id: None,
                path: entry.path.clone(),
                size_bytes: read,
                sha1: None,
                sha512: Some(format!("{:x}", hash.finalize())),
            },
        );
    }
    let snapshot = PackManifestSnapshot {
        source: PackSource {
            provider: PackProvider::Mrpack,
            project_id: None,
        },
        version_id: pack.version_id,
        minecraft_version: pack.minecraft_version,
        loader_kind: pack.loader_kind,
        loader_version: pack.loader_version,
        files: files.into_values().collect(),
    };
    validate(&snapshot)?;
    Ok(snapshot)
}

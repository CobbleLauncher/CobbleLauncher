// SPDX-License-Identifier: MIT
//! Discard official ZIP timestamps/attributes before strict capability ingress.
use super::Error;
use crate::OperationControl;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::io::{Read, Seek};

pub(super) fn normalize<R: Read + Seek>(
    input: R,
    staging: &Dir,
    control: &OperationControl,
) -> Result<(), Error> {
    let mut archive = zip::ZipArchive::new(input)?;
    archive.validate_header_consistency()?;
    if archive.len() > 20_000 {
        return Err("JAVA_ARCHIVE_ENTRY_LIMIT".into());
    }
    let output = staging.open_with(
        ".normalized.zip",
        OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .follow(FollowSymlinks::No),
    )?;
    let mut writer = zip::ZipWriter::new(output);
    let mut total = 0u64;
    for index in 0..archive.len() {
        control.check_cancelled()?;
        let mut entry = archive.by_index(index)?;
        let name = std::str::from_utf8(entry.name_raw())?.to_owned();
        let directory = entry.is_dir();
        cobble_store::SafeRelativePath::parse(if directory {
            name.strip_suffix('/').ok_or("JAVA_ARCHIVE_PATH")?
        } else {
            &name
        })?;
        let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
        if entry.encrypted()
            || !matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
            || (kind != 0 && kind != if directory { 0o040000 } else { 0o100000 })
        {
            return Err("JAVA_ARCHIVE_OBJECT_UNSUPPORTED".into());
        }
        let size = entry.size();
        total = total.checked_add(size).ok_or("JAVA_ARCHIVE_SIZE_LIMIT")?;
        if total > 480 * 1024 * 1024
            || size > 256 * 1024 * 1024
            || (size > 0 && size / entry.compressed_size().max(1) > 200)
        {
            return Err("JAVA_ARCHIVE_SIZE_LIMIT".into());
        }
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        if directory {
            if size != 0 {
                return Err("JAVA_ARCHIVE_DIRECTORY_DATA".into());
            }
            writer.add_directory(name, options)?;
        } else {
            writer.start_file(name, options)?;
            let mut bounded = (&mut entry).take(size.saturating_add(1));
            let copied = std::io::copy(&mut bounded, &mut writer)?;
            if copied != size {
                return Err("JAVA_ARCHIVE_SIZE_MISMATCH".into());
            }
        }
    }
    writer.finish()?.sync_all()?;
    control.check_cancelled()?;
    Ok(())
}

/// Parse gzip/tar with maintained codecs; only regular files/directories enter
/// the shared strict ZIP ingress. Safe links become copies; OS links and special
/// files never reach disk.
pub(super) fn normalize_tar<R: Read>(
    input: R,
    staging: &Dir,
    control: &OperationControl,
) -> Result<Vec<String>, Error> {
    let decoder = flate2::read::GzDecoder::new(input);
    let mut archive = tar::Archive::new(decoder);
    let output = staging.open_with(
        ".normalized.zip",
        OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .follow(FollowSymlinks::No),
    )?;
    let mut writer = zip::ZipWriter::new(output);
    let mut total = 0u64;
    let mut executable = Vec::new();
    let mut regular = std::collections::HashSet::new();
    let mut aliases = Vec::new();
    for (index, entry) in archive.entries()?.enumerate() {
        control.check_cancelled()?;
        if index >= 20_000 {
            return Err("JAVA_ARCHIVE_ENTRY_LIMIT".into());
        }
        let mut entry = entry?;
        let name = std::str::from_utf8(&entry.path_bytes())?
            .trim_end_matches('/')
            .to_owned();
        cobble_store::SafeRelativePath::parse(&name)?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() {
            if entry.size() != 0 {
                return Err("JAVA_ARCHIVE_OBJECT_UNSUPPORTED".into());
            }
            let link = entry.link_name_bytes().ok_or("JAVA_ARCHIVE_LINK_TARGET")?;
            let target = relative_link_target(&name, std::str::from_utf8(&link)?)?;
            aliases.push((name, target));
            continue;
        }
        if !kind.is_dir() && !kind.is_file() {
            return Err("JAVA_ARCHIVE_OBJECT_UNSUPPORTED".into());
        }
        let size = entry.size();
        total = total.checked_add(size).ok_or("JAVA_ARCHIVE_SIZE_LIMIT")?;
        if total > 480 * 1024 * 1024 || size > 256 * 1024 * 1024 {
            return Err("JAVA_ARCHIVE_SIZE_LIMIT".into());
        }
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        if kind.is_dir() {
            if size != 0 {
                return Err("JAVA_ARCHIVE_DIRECTORY_DATA".into());
            }
            writer.add_directory(format!("{name}/"), options)?;
        } else {
            if entry.header().mode()? & 0o111 != 0 {
                executable.push(name.clone());
            }
            regular.insert(name.clone());
            writer.start_file(name, options)?;
            if std::io::copy(&mut (&mut entry).take(size.saturating_add(1)), &mut writer)? != size {
                return Err("JAVA_ARCHIVE_SIZE_MISMATCH".into());
            }
        }
    }
    writer.finish()?.sync_all()?;
    if !aliases.is_empty() {
        let options = OpenOptions::new()
            .read(true)
            .follow(FollowSymlinks::No)
            .clone();
        let mut source = zip::ZipArchive::new(staging.open_with(".normalized.zip", &options)?)?;
        let output = staging.open_with(
            ".normalized.zip",
            OpenOptions::new()
                .read(true)
                .write(true)
                .follow(FollowSymlinks::No),
        )?;
        let mut writer = zip::ZipWriter::new_append(output)?;
        for (name, target) in aliases {
            control.check_cancelled()?;
            // Resolve only to directly archived regular files, never links or directories.
            if !regular.contains(&target) {
                return Err("JAVA_ARCHIVE_LINK_TARGET".into());
            }
            let mut file = source.by_name(&target)?;
            total = total
                .checked_add(file.size())
                .ok_or("JAVA_ARCHIVE_SIZE_LIMIT")?;
            if total > 480 * 1024 * 1024 {
                return Err("JAVA_ARCHIVE_SIZE_LIMIT".into());
            }
            if executable.contains(&target) {
                executable.push(name.clone());
            }
            writer.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )?;
            std::io::copy(&mut file, &mut writer)?;
        }
        writer.finish()?.sync_all()?;
    }
    control.check_cancelled()?;
    Ok(executable)
}

// Tar links are materialized as independent files; no OS link is created.
fn relative_link_target(name: &str, link: &str) -> Result<String, Error> {
    if link.is_empty() || link.starts_with('/') || link.contains('\\') || link.contains(':') {
        return Err("JAVA_ARCHIVE_LINK_TARGET".into());
    }
    let mut parts = name.split('/').collect::<Vec<_>>();
    parts.pop();
    for component in link.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if parts.len() <= 1 {
                    return Err("JAVA_ARCHIVE_LINK_TARGET".into());
                }
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    let target = parts.join("/");
    cobble_store::SafeRelativePath::parse(&target)?;
    Ok(target)
}

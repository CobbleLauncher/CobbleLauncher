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

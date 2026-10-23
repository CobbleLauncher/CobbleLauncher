// SPDX-License-Identifier: MIT
//! Bounded, read-only inventory for report plugins. Never extracts overrides.
use crate::{BoxError, mrpack::Namespace};
use cobble_store::SafeRelativePath;
use serde::Serialize;
use std::io::{self, Read, Seek, SeekFrom};

const MAX_INDEX: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_NAMES: usize = 2 * 1024 * 1024;
const MAX_EXPANDED: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct MrpackInspectionSnapshot {
    pub index: serde_json::Value,
    pub overrides: Vec<MrpackInspectionOverride>,
}

/// CRC32 and size are ZIP metadata, not authenticated content identities.
#[derive(Debug, Serialize)]
pub struct MrpackInspectionOverride {
    pub path: String,
    pub size: u64,
    pub crc32: u32,
}

/// Reads the index and central directory from an owned/borrowed seekable file.
/// Unknown dependency keys remain available to the report, unlike installation.
pub fn inspect_mrpack_archive<R: Read + Seek>(
    mut source: R,
) -> Result<MrpackInspectionSnapshot, BoxError> {
    bound_central_directory(&mut source)?;
    let mut archive = zip::ZipArchive::new(source)?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("MRPACK contains too many entries").into());
    }
    archive.validate_header_consistency()?;
    let mut namespace = Namespace::default();
    let mut names_size = 0usize;
    let mut expanded = 0u64;
    let mut index = None;
    let mut overrides = Vec::new();
    for position in 0..archive.len() {
        let mut entry = archive.by_index(position)?;
        names_size = names_size.saturating_add(entry.name_raw().len());
        if names_size > MAX_NAMES {
            return Err(invalid("MRPACK path metadata exceeds 2 MiB").into());
        }
        let name = std::str::from_utf8(entry.name_raw())?.to_owned();
        let directory = entry.is_dir();
        let path = SafeRelativePath::parse(if directory {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        })?;
        namespace.insert(path.as_str(), directory, Some(0))?;
        if entry.encrypted() {
            return Err(invalid("Encrypted MRPACK entries are unsupported").into());
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != if directory { 0o040000 } else { 0o100000 } {
                return Err(invalid("MRPACK links and special files are forbidden").into());
            }
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_EXPANDED {
            return Err(invalid("MRPACK expanded content exceeds 16 GiB").into());
        }
        if path.as_str() == "modrinth.index.json" {
            if directory || index.is_some() || entry.size() > MAX_INDEX {
                return Err(
                    invalid("MRPACK index is duplicate, a directory, or exceeds 4 MiB").into(),
                );
            }
            let mut bytes = Vec::new();
            entry.by_ref().take(MAX_INDEX + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_INDEX {
                return Err(invalid("MRPACK index exceeds 4 MiB").into());
            }
            index = Some(serde_json::from_slice(&bytes)?);
        } else if !directory
            && ["overrides/", "client-overrides/", "server-overrides/"]
                .iter()
                .any(|prefix| path.as_str().starts_with(prefix))
        {
            overrides.push(MrpackInspectionOverride {
                path: path.as_str().to_owned(),
                size: entry.size(),
                crc32: entry.crc32(),
            });
        }
    }
    Ok(MrpackInspectionSnapshot {
        index: index.ok_or_else(|| invalid("Missing root modrinth.index.json"))?,
        overrides,
    })
}

// Bound ZIP metadata before ZipArchive allocates the central-directory inventory.
// ZIP64 is supported so a large archive does not require loading its payload.
fn bound_central_directory<R: Read + Seek>(source: &mut R) -> Result<(), BoxError> {
    let length = source.seek(SeekFrom::End(0))?;
    let tail_size = length.min(65_557) as usize;
    source.seek(SeekFrom::End(-(tail_size as i64)))?;
    let mut tail = vec![0; tail_size];
    source.read_exact(&mut tail)?;
    let position = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&position| {
            tail[position..].starts_with(b"PK\x05\x06")
                && position
                    + 22
                    + usize::from(u16::from_le_bytes([
                        tail[position + 20],
                        tail[position + 21],
                    ]))
                    == tail.len()
        })
        .ok_or_else(|| invalid("Missing ZIP end directory"))?;
    let end = &tail[position..];
    let entries = u16::from_le_bytes([end[10], end[11]]);
    let size = u32::from_le_bytes(end[12..16].try_into()?);
    let (entries, size) = if entries == u16::MAX || size == u32::MAX {
        let end_offset = length - tail_size as u64 + position as u64;
        if end_offset < 20 {
            return Err(invalid("Missing ZIP64 locator").into());
        }
        source.seek(SeekFrom::Start(end_offset - 20))?;
        let mut locator = [0; 20];
        source.read_exact(&mut locator)?;
        if !locator.starts_with(b"PK\x06\x07") {
            return Err(invalid("Missing ZIP64 locator").into());
        }
        let offset = u64::from_le_bytes(locator[8..16].try_into()?);
        source.seek(SeekFrom::Start(offset))?;
        let mut record = [0; 56];
        source.read_exact(&mut record)?;
        if !record.starts_with(b"PK\x06\x06") {
            return Err(invalid("Invalid ZIP64 directory").into());
        }
        (
            u64::from_le_bytes(record[32..40].try_into()?),
            u64::from_le_bytes(record[40..48].try_into()?),
        )
    } else {
        (u64::from(entries), u64::from(size))
    };
    if entries > MAX_ENTRIES as u64 || size > 16 * 1024 * 1024 {
        return Err(invalid("MRPACK ZIP directory exceeds analysis metadata limits").into());
    }
    source.seek(SeekFrom::Start(0))?;
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn fixture(extra: &str, payload: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("modrinth.index.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","dependencies":{"future-loader":"1"},"files":[]}"#).unwrap();
        zip.start_file(extra, SimpleFileOptions::default()).unwrap();
        zip.write_all(payload).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn inventories_large_archive_and_unknown_dependencies() {
        let bytes = fixture(
            "server-overrides/config/example.bin",
            &vec![7; 9 * 1024 * 1024],
        );
        let snapshot = inspect_mrpack_archive(Cursor::new(bytes)).unwrap();
        assert_eq!(snapshot.index["dependencies"]["future-loader"], "1");
        assert_eq!(snapshot.overrides[0].size, 9 * 1024 * 1024);
        assert_eq!(
            snapshot.overrides[0].path,
            "server-overrides/config/example.bin"
        );
    }

    #[test]
    fn rejects_unsafe_archive_paths() {
        assert!(inspect_mrpack_archive(Cursor::new(fixture("overrides/../escape", b"x"))).is_err());
    }

    #[test]
    fn rejects_oversized_directory_before_archive_parsing() {
        let mut bytes = fixture("overrides/config/example.txt", b"x");
        let end = bytes.len() - 22;
        bytes[end + 12..end + 16].copy_from_slice(&(17u32 * 1024 * 1024).to_le_bytes());
        let error = inspect_mrpack_archive(Cursor::new(bytes)).unwrap_err();
        assert!(error.to_string().contains("metadata limits"));
    }
}

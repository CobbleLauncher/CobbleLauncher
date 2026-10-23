// SPDX-License-Identifier: MIT
//! Only root pack.mcmeta is read; no assets are decoded or extracted.
use crate::BoxError;
use std::{fs::File, io::Read};
pub const MAX_PACK_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PACK_METADATA_BYTES: u64 = 1024 * 1024;
pub fn read_pack_metadata_zip(source: File) -> Result<Vec<u8>, BoxError> {
    let metadata = source.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_PACK_ARCHIVE_BYTES {
        return Err("Pack archive exceeds size limit or is not a regular file".into());
    }
    let mut archive = zip::ZipArchive::new(source)?;
    if archive.len() > 50_000 {
        return Err("Pack archive has too many entries".into());
    }
    archive.validate_header_consistency()?;
    let mut selected = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name()?;
        if name.eq_ignore_ascii_case("pack.mcmeta") {
            if name != "pack.mcmeta"
                || selected.is_some()
                || entry.is_dir()
                || entry.size() > MAX_PACK_METADATA_BYTES
            {
                return Err("Invalid or ambiguous root pack.mcmeta".into());
            }
            selected = Some(index);
        }
    }
    let index = selected.ok_or("Pack archive has no root pack.mcmeta")?;
    let mut entry = archive.by_index(index)?;
    let expected = entry.size();
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(MAX_PACK_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PACK_METADATA_BYTES || bytes.len() as u64 != expected {
        return Err("Pack metadata exceeds limit or has invalid size".into());
    }
    std::str::from_utf8(&bytes).map_err(|_| "Pack metadata is not UTF-8")?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn run(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, BoxError> {
        let path = std::env::temp_dir().join(format!(
            "cobble-metadata-{}-{}.zip",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut zip = zip::ZipWriter::new(File::create(&path)?);
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
        let result = read_pack_metadata_zip(File::open(&path)?);
        std::fs::remove_file(path)?;
        result
    }
    #[test]
    fn reads_only_root_and_preserves_invalid_json_for_guest_diagnosis() {
        assert_eq!(
            run(&[
                ("pack.mcmeta", b"invalid JSON"),
                ("assets/example/pack.mcmeta", b"nested"),
                ("assets/example/large.bin", &[0; 1000])
            ])
            .unwrap(),
            b"invalid JSON"
        );
    }
    #[test]
    fn rejects_missing_alias_invalid_utf8_and_oversized_metadata() {
        assert!(run(&[("nested/pack.mcmeta", b"{}")]).is_err());
        assert!(run(&[("pack.mcmeta", b"{}"), ("PACK.MCMETA", b"{}")]).is_err());
        assert!(run(&[("pack.mcmeta", &[255])]).is_err());
        assert!(
            run(&[(
                "pack.mcmeta",
                &vec![0; MAX_PACK_METADATA_BYTES as usize + 1]
            )])
            .is_err()
        );
    }
}

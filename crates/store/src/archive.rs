// SPDX-License-Identifier: MIT
//! Bounded ZIP32 ingress into a caller-owned staging directory capability.
//!
//! This is not an installer transaction or an executable-code sandbox. Only
//! single-disk Stored/Deflate ZIP32 without extra fields or data descriptors is
//! supported. No archive permissions, timestamps, links or attributes are applied.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};

use crate::SafeRelativePath;
use crate::safe_path::Namespace;

#[derive(Debug)]
pub enum ArchiveError {
    InvalidPath(&'static str),
    Collision,
    Unsupported(&'static str),
    InvalidArchive(&'static str),
    Limit(&'static str),
    UnsafeObject,
    Io(io::Error),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(reason) => write!(f, "invalid archive path: {reason}"),
            Self::Collision => f.write_str("archive path collision"),
            Self::Unsupported(reason) => write!(f, "unsupported archive: {reason}"),
            Self::InvalidArchive(reason) => write!(f, "invalid archive: {reason}"),
            Self::Limit(reason) => write!(f, "archive limit exceeded: {reason}"),
            Self::UnsafeObject => {
                f.write_str("link, reparse point, or non-regular object rejected")
            }
            Self::Io(error) => write!(f, "archive I/O failed: {error}"),
        }
    }
}

impl std::error::Error for ArchiveError {}
impl From<io::Error> for ArchiveError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Caller-approved installation budgets; defaults are deliberately conservative.
/// The 4 MiB entry default also protects individual metadata documents. Raising
/// it requires the caller to enforce its own metadata-specific 4 MiB budget.
#[derive(Clone, Copy, Debug)]
pub struct ArchiveLimits {
    pub max_archive_bytes: u64,
    pub max_entries: u32,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_ratio: u32,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 128 * 1024 * 1024,
            max_entries: 100_000,
            max_entry_bytes: 4 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArchiveSummary {
    pub entries: u32,
    pub expanded_bytes: u64,
}

struct Entry {
    path: SafeRelativePath,
    raw_name: Vec<u8>,
    directory: bool,
    method: u16,
    compressed: u64,
    expanded: u64,
    crc: u32,
}

/// Materialize validated files using an already registered, opened root.
///
/// Each directory segment is opened without following links and retained while
/// its child is opened; every leaf uses exclusive creation. Existing files,
/// including hardlinks, are never overwritten. The root itself is a trusted
/// capability supplied by Core, never an archive/renderer path. Use a dedicated
/// empty staging root. On failure some newly created staging files may remain;
/// callers must discard that staging transaction and must not commit it.
pub fn extract_zip<R: Read + Seek>(
    mut source: R,
    staging: &Dir,
    limits: ArchiveLimits,
) -> Result<ArchiveSummary, ArchiveError> {
    // Installation budgets cannot disable the process-wide ingestion ceilings.
    if limits.max_archive_bytes > 2 * 1024 * 1024 * 1024
        || limits.max_entries > 100_000
        || limits.max_entry_bytes > 2 * 1024 * 1024 * 1024
        || limits.max_total_bytes > 8 * 1024 * 1024 * 1024
        || limits.max_ratio == 0
        || limits.max_ratio > 1000
    {
        return Err(ArchiveError::Limit("invalid budget or hard ceiling"));
    }
    check_directory(staging)?;
    let entries = preflight(&mut source, limits)?;
    source.rewind()?;
    let mut archive =
        zip::ZipArchive::new(source).map_err(|_| ArchiveError::InvalidArchive("ZIP directory"))?;
    archive.validate_header_consistency().map_err(|_| {
        ArchiveError::InvalidArchive("local/central header mismatch or overlapping data")
    })?;
    if archive.len() != entries.len() {
        return Err(ArchiveError::InvalidArchive("entry count mismatch"));
    }
    let mut total = 0_u64;
    for (index, entry) in entries.iter().enumerate() {
        let mut raw = archive
            .by_index_raw(index)
            .map_err(|_| ArchiveError::InvalidArchive("ZIP entry"))?;
        if raw.name_raw() != entry.raw_name
            || raw.compressed_size() != entry.compressed
            || raw.size() != entry.expanded
            || raw.crc32() != entry.crc
            || raw.encrypted()
            || raw.is_dir() != entry.directory
        {
            return Err(ArchiveError::InvalidArchive(
                "preflight/reader metadata mismatch",
            ));
        }
        let parent = open_parent(staging, &entry.path, entry.directory)?;
        if entry.directory {
            if entry.expanded != 0 || entry.compressed != 0 {
                return Err(ArchiveError::InvalidArchive("directory contains data"));
            }
            continue;
        }
        let leaf = entry
            .path
            .as_str()
            .rsplit('/')
            .next()
            .expect("validated nonempty path");
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut output = parent.open_with(leaf, &options)?;
        check_regular(&output.metadata()?)?;
        let (expanded, crc, consumed) = if entry.method == 8 {
            let mut decoder = flate2::read::DeflateDecoder::new(&mut raw);
            let (expanded, crc) = copy_bounded(&mut decoder, &mut output, limits, &mut total)?;
            (expanded, crc, decoder.total_in())
        } else {
            let (expanded, crc) = copy_bounded(&mut raw, &mut output, limits, &mut total)?;
            (expanded, crc, expanded)
        };
        if expanded != entry.expanded || consumed != entry.compressed || crc != entry.crc {
            return Err(ArchiveError::InvalidArchive(
                "actual size, compressed consumption or CRC mismatch",
            ));
        }
        if expanded > consumed.max(1).saturating_mul(u64::from(limits.max_ratio)) {
            return Err(ArchiveError::Limit("actual compression ratio"));
        }
        output.flush()?;
    }
    Ok(ArchiveSummary {
        entries: entries.len() as u32,
        expanded_bytes: total,
    })
}

fn copy_bounded(
    input: &mut impl Read,
    output: &mut impl Write,
    limits: ArchiveLimits,
    total: &mut u64,
) -> Result<(u64, u32), ArchiveError> {
    let mut count = 0_u64;
    let mut crc = crc32fast::Hasher::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        // Read at most one byte beyond either remaining budget. Never write the
        // over-budget chunk, including when the header lied about the size.
        let allowed = limits
            .max_entry_bytes
            .saturating_sub(count)
            .min(limits.max_total_bytes.saturating_sub(*total));
        let chunk = allowed.saturating_add(1).min(buffer.len() as u64) as usize;
        let read = input.read(&mut buffer[..chunk])?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or(ArchiveError::Limit("size overflow"))?;
        *total = total
            .checked_add(read as u64)
            .ok_or(ArchiveError::Limit("total overflow"))?;
        if count > limits.max_entry_bytes || *total > limits.max_total_bytes {
            return Err(ArchiveError::Limit("actual expanded bytes"));
        }
        crc.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    Ok((count, crc.finalize()))
}

pub(crate) fn open_parent(
    root: &Dir,
    path: &SafeRelativePath,
    directory: bool,
) -> Result<Dir, ArchiveError> {
    open_parent_observing(root, path, directory, |_, _| Ok(()))
}

fn open_parent_observing(
    root: &Dir,
    path: &SafeRelativePath,
    directory: bool,
    mut before_open: impl FnMut(&Dir, &str) -> io::Result<()>,
) -> Result<Dir, ArchiveError> {
    let parts: Vec<_> = path.as_str().split('/').collect();
    let count = parts.len() - usize::from(!directory);
    let mut current = root.try_clone()?;
    for part in &parts[..count] {
        match current.create_dir(part) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.into()),
        }
        before_open(&current, part)?;
        // `part` is exactly one validated component, so no intermediate segment
        // is resolved by this operation. A replacement before open either fails
        // nofollow or opens the real directory; no path check/use race exists.
        let next = current.open_dir_nofollow(part)?;
        check_directory(&next)?;
        current = next;
    }
    Ok(current)
}

pub(crate) fn check_directory(dir: &Dir) -> Result<(), ArchiveError> {
    let metadata = dir.dir_metadata()?;
    if !metadata.is_dir() {
        return Err(ArchiveError::UnsafeObject);
    }
    check_reparse(&metadata)
}

pub(crate) fn check_regular(metadata: &cap_std::fs::Metadata) -> Result<(), ArchiveError> {
    if !metadata.is_file() {
        return Err(ArchiveError::UnsafeObject);
    }
    check_reparse(metadata)
}

fn check_reparse(metadata: &cap_std::fs::Metadata) -> Result<(), ArchiveError> {
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(ArchiveError::UnsafeObject);
        }
    }
    #[cfg(not(windows))]
    let _ = metadata;
    Ok(())
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("fixed header"))
}

fn preflight<R: Read + Seek>(
    source: &mut R,
    limits: ArchiveLimits,
) -> Result<Vec<Entry>, ArchiveError> {
    let size = source.seek(SeekFrom::End(0))?;
    if size > limits.max_archive_bytes {
        return Err(ArchiveError::Limit("archive input bytes"));
    }
    let tail_length = size.min(65_557) as usize;
    source.seek(SeekFrom::End(-(tail_length as i64)))?;
    let mut tail = vec![0_u8; tail_length];
    source.read_exact(&mut tail)?;
    let end = (0..tail_length.saturating_sub(21))
        .rev()
        .find(|&at| {
            tail[at..at + 4] == *b"PK\x05\x06"
                && at + 22 + usize::from(u16_at(&tail, at + 20)) == tail_length
        })
        .ok_or(ArchiveError::InvalidArchive("ZIP end record"))?;
    let end_offset = size - tail_length as u64 + end as u64;
    let end = &tail[end..end + 22];
    if u16_at(end, 4) != 0 || u16_at(end, 6) != 0 || u16_at(end, 8) != u16_at(end, 10) {
        return Err(ArchiveError::Unsupported("multi-disk ZIP"));
    }
    let count = u16_at(end, 10);
    let central_size = u32_at(end, 12);
    let offset = u32_at(end, 16);
    if count == u16::MAX || central_size == u32::MAX || offset == u32::MAX {
        return Err(ArchiveError::Unsupported("ZIP64"));
    }
    if u32::from(count) > limits.max_entries {
        return Err(ArchiveError::Limit("entry count"));
    }
    if u64::from(offset) + u64::from(central_size) != end_offset || central_size > 16 * 1024 * 1024
    {
        return Err(ArchiveError::InvalidArchive(
            "central directory offset/16 MiB budget",
        ));
    }
    source.seek(SeekFrom::Start(u64::from(offset)))?;
    let mut entries = Vec::with_capacity(usize::from(count));
    let mut namespace = Namespace::default();
    let mut declared_total = 0_u64;
    for _ in 0..count {
        if source.stream_position()?.saturating_add(46) > end_offset {
            return Err(ArchiveError::InvalidArchive("central directory truncated"));
        }
        let mut header = [0_u8; 46];
        source.read_exact(&mut header)?;
        if header[..4] != *b"PK\x01\x02" {
            return Err(ArchiveError::InvalidArchive("central entry signature"));
        }
        let flags = u16_at(&header, 8);
        let method = u16_at(&header, 10);
        if flags & !0x0806 != 0 {
            return Err(ArchiveError::Unsupported(
                "encryption, data descriptors or unknown ZIP flags",
            ));
        }
        if !matches!(method, 0 | 8) {
            return Err(ArchiveError::Unsupported("compression method"));
        }
        if u16_at(&header, 30) != 0 {
            return Err(ArchiveError::Unsupported("ZIP extra fields"));
        }
        if u16_at(&header, 34) != 0 {
            return Err(ArchiveError::Unsupported("multi-disk ZIP entry"));
        }
        let name_length = usize::from(u16_at(&header, 28));
        let comment_length = u16_at(&header, 32);
        if name_length == 0 || name_length > 1025 {
            return Err(ArchiveError::InvalidPath("archive name byte budget"));
        }
        if source
            .stream_position()?
            .saturating_add(name_length as u64 + u64::from(comment_length))
            > end_offset
        {
            return Err(ArchiveError::InvalidArchive("central entry bounds"));
        }
        let mut name = vec![0; name_length];
        source.read_exact(&mut name)?;
        source.seek(SeekFrom::Current(i64::from(comment_length)))?;
        let name =
            std::str::from_utf8(&name).map_err(|_| ArchiveError::InvalidPath("UTF-8 required"))?;
        let directory = name.ends_with('/');
        let path = SafeRelativePath::parse(if directory {
            &name[..name.len() - 1]
        } else {
            name
        })?;
        namespace.insert(&path, directory)?;
        let attributes = u32_at(&header, 38);
        let kind = attributes >> 16 & 0o170000;
        if !matches!(kind, 0 | 0o100000 | 0o040000)
            || (kind == 0o040000 && !directory)
            || (kind == 0o100000 && directory)
            || attributes & 0x400 != 0
        {
            return Err(ArchiveError::UnsafeObject);
        }
        let compressed = u64::from(u32_at(&header, 20));
        let expanded = u64::from(u32_at(&header, 24));
        if compressed == u64::from(u32::MAX) || expanded == u64::from(u32::MAX) {
            return Err(ArchiveError::Unsupported("ZIP64 entry"));
        }
        if expanded > limits.max_entry_bytes {
            return Err(ArchiveError::Limit("declared entry bytes"));
        }
        declared_total = declared_total
            .checked_add(expanded)
            .ok_or(ArchiveError::Limit("declared total overflow"))?;
        if declared_total > limits.max_total_bytes {
            return Err(ArchiveError::Limit("declared total bytes"));
        }
        if expanded
            > compressed
                .max(1)
                .saturating_mul(u64::from(limits.max_ratio))
        {
            return Err(ArchiveError::Limit("declared compression ratio"));
        }
        // Validate local extra-field absence before the general-purpose ZIP
        // parser can interpret metadata that our restricted profile disallows.
        let resume = source.stream_position()?;
        let local_offset = u64::from(u32_at(&header, 42));
        if local_offset.saturating_add(30) > u64::from(offset) {
            return Err(ArchiveError::InvalidArchive("local header bounds"));
        }
        source.seek(SeekFrom::Start(local_offset))?;
        let mut local = [0_u8; 30];
        source.read_exact(&mut local)?;
        if local[..4] != *b"PK\x03\x04" {
            return Err(ArchiveError::InvalidArchive("local signature"));
        }
        if u16_at(&local, 28) != 0 {
            return Err(ArchiveError::Unsupported("local ZIP extra fields"));
        }
        let data_end = local_offset + 30 + u64::from(u16_at(&local, 26)) + compressed;
        if data_end > u64::from(offset) {
            return Err(ArchiveError::InvalidArchive("entry data bounds"));
        }
        source.seek(SeekFrom::Start(resume))?;
        entries.push(Entry {
            path,
            raw_name: name.as_bytes().to_vec(),
            directory,
            method,
            compressed,
            expanded,
            crc: u32_at(&header, 16),
        });
    }
    if source.stream_position()? != end_offset {
        return Err(ArchiveError::InvalidArchive(
            "central directory count/size mismatch",
        ));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;
    use std::path::Path;

    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new(suffix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-race-{}-{}-{suffix}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn dir(&self) -> Dir {
            Dir::open_ambient_dir(&self.0, ambient_authority()).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn redirect(link: &Path, target: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
        #[cfg(windows)]
        {
            let result = std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "junction creation failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }

    fn remove_redirect(link: &Path) {
        #[cfg(unix)]
        std::fs::remove_file(link).unwrap();
        #[cfg(windows)]
        std::fs::remove_dir(link).unwrap();
    }

    #[test]
    fn sec004_replace_directory_between_creation_and_open_cannot_escape() {
        let staging = Temp::new("staging");
        let outside = Temp::new("outside");
        let root = staging.dir();
        let path = SafeRelativePath::parse("mods/escaped").unwrap();
        // The observer is exactly between mkdir/AlreadyExists and nofollow open,
        // reproducing the check/use window without probabilistic thread timing.
        let result = open_parent_observing(&root, &path, false, |current, component| {
            current.remove_dir(component)?;
            redirect(&staging.0.join(component), &outside.0);
            Ok(())
        });
        assert!(result.is_err());
        assert!(!outside.0.join("escaped").exists());
        remove_redirect(&staging.0.join("mods"));
    }

    #[test]
    fn sec004_opened_parent_handle_cannot_be_retargeted() {
        let staging = Temp::new("staging");
        let outside = Temp::new("outside");
        let root = staging.dir();
        let path = SafeRelativePath::parse("mods/escaped").unwrap();
        let parent = open_parent(&root, &path, false).unwrap();
        match std::fs::rename(staging.0.join("mods"), staging.0.join("held")) {
            Ok(()) => {
                redirect(&staging.0.join("mods"), &outside.0);
                let mut options = OpenOptions::new();
                options
                    .create_new(true)
                    .write(true)
                    .follow(FollowSymlinks::No);
                parent
                    .open_with("escaped", &options)
                    .unwrap()
                    .write_all(b"safe")
                    .unwrap();
                assert_eq!(
                    std::fs::read(staging.0.join("held/escaped")).unwrap(),
                    b"safe"
                );
                assert!(!outside.0.join("escaped").exists());
                remove_redirect(&staging.0.join("mods"));
            }
            Err(error) => {
                // Windows may deny directory rename while the safe parent
                // handle is held; that is also a successful containment result.
                #[cfg(windows)]
                assert!(matches!(error.raw_os_error(), Some(5 | 32)), "{error}");
                #[cfg(not(windows))]
                panic!("unexpected rename failure: {error}");
                #[cfg(windows)]
                assert!(staging.0.join("mods").is_dir());
            }
        }
        assert!(!outside.0.join("escaped").exists());
    }
}

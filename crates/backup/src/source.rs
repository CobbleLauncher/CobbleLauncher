// SPDX-License-Identifier: MIT
//! A restricted rustic source: handles are opened beneath an owned directory,
//! refusing links at every component. rustic handles chunking/storage itself.
use crate::{BackupControl, BackupError, Manifest, relative};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use rustic_core::{
    ErrorKind, Excludes, LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions, ReadSource,
    ReadSourceEntry, ReadSourceOpen, RusticError, RusticResult, XattrOption,
};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) struct WorldSource {
    entries: Vec<ReadSourceEntry<WorldOpen>>,
    failed: Arc<AtomicBool>,
    manifest: Manifest,
}

impl WorldSource {
    #[cfg(test)]
    pub(crate) fn new(world: &Path) -> Result<Self, BackupError> {
        Self::new_controlled(world, &BackupControl::default())
    }
    pub(crate) fn new_controlled(
        world: &Path,
        control: &BackupControl,
    ) -> Result<Self, BackupError> {
        control.check()?;
        let root = Arc::new(Dir::open_ambient_dir(world, cap_std::ambient_authority())?);
        let failed = Arc::new(AtomicBool::new(false));
        let source = LocalSource::new(
            LocalSourceSaveOptions::default().set_xattrs(XattrOption::No),
            &Excludes::default(),
            &LocalSourceFilterOptions::default(),
            &[world],
        )
        .map_err(|_| BackupError::Engine("scan source"))?;
        let mut entries = Vec::new();
        let mut manifest = Manifest::new();
        for item in source.entries() {
            control.check()?;
            let item = item.map_err(|_| BackupError::UnsafeSource)?;
            let path = item
                .path
                .strip_prefix(world)
                .map_err(|_| BackupError::UnsafeSource)?
                .to_owned();
            relative(&path)?;
            if !item.node.is_dir() && !item.node.is_file() {
                return Err(BackupError::UnsafeSource);
            }
            // Validate directory handles even for an empty directory/reparse point.
            let mut directory = root.try_clone()?;
            let parts: Vec<_> = path.components().collect();
            for part in &parts[..parts.len() - 1] {
                directory = directory.open_dir_nofollow(part.as_os_str())?;
            }
            let leaf = parts.last().ok_or(BackupError::UnsafeSource)?.as_os_str();
            if item.node.is_dir() {
                let _ = directory.open_dir_nofollow(leaf)?;
            } else {
                if path == Path::new("session.lock") {
                    continue;
                }
                let mut options = OpenOptions::new();
                options.read(true).follow(FollowSymlinks::No);
                let file = directory.open_with(leaf, &options)?;
                if !file.metadata()?.is_file() {
                    return Err(BackupError::UnsafeSource);
                }
            }
            manifest.insert(
                path.clone(),
                (
                    item.node.is_dir(),
                    if item.node.is_file() {
                        item.node.meta.size
                    } else {
                        0
                    },
                ),
            );
            let open = item.node.is_file().then(|| WorldOpen {
                root: root.clone(),
                path,
                failed: failed.clone(),
                size: item.node.meta.size,
                control: control.clone(),
            });
            entries.push(ReadSourceEntry {
                path: item.path,
                node: item.node,
                open,
            });
        }
        Ok(Self {
            entries,
            failed,
            manifest,
        })
    }
    pub(crate) fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub(crate) struct WorldOpen {
    root: Arc<Dir>,
    path: PathBuf,
    failed: Arc<AtomicBool>,
    size: u64,
    control: BackupControl,
}
pub(crate) struct WorldReader {
    file: cap_std::fs::File,
    failed: Arc<AtomicBool>,
    remaining: u64,
    control: BackupControl,
}
impl Read for WorldReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.control.is_cancelled() {
            self.failed.store(true, Ordering::Relaxed);
            return Err(io::Error::other("Backup operation cancelled"));
        }
        match self.file.read(buffer) {
            Ok(count) if count as u64 <= self.remaining => {
                self.remaining -= count as u64;
                self.control.read_bytes(count as u64);
                if count == 0 && self.remaining != 0 {
                    self.failed.store(true, Ordering::Relaxed);
                    return Err(io::Error::other("World file changed during backup"));
                }
                Ok(count)
            }
            Ok(_) => {
                self.failed.store(true, Ordering::Relaxed);
                Err(io::Error::other("World file changed during backup"))
            }
            Err(error) => {
                self.failed.store(true, Ordering::Relaxed);
                Err(error)
            }
        }
    }
}

impl ReadSourceOpen for WorldOpen {
    type Reader = WorldReader;
    fn open(self) -> RusticResult<WorldReader> {
        if self.control.is_cancelled() {
            self.failed.store(true, Ordering::Relaxed);
            return Err(RusticError::new(
                ErrorKind::InputOutput,
                "Backup operation cancelled",
            ));
        }
        let opened = (|| -> io::Result<_> {
            let mut directory = self.root.try_clone()?;
            let parts: Vec<_> = self.path.components().collect();
            for part in &parts[..parts.len() - 1] {
                directory = directory.open_dir_nofollow(part.as_os_str())?;
            }
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file =
                directory.open_with(parts.last().expect("validated path").as_os_str(), &options)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.len() != self.size {
                return Err(io::Error::other("World source changed"));
            }
            Ok(file)
        })();
        match opened {
            Ok(file) => Ok(WorldReader {
                file,
                failed: self.failed,
                remaining: self.size,
                control: self.control,
            }),
            Err(_) => {
                self.failed.store(true, Ordering::Relaxed);
                Err(RusticError::new(
                    ErrorKind::InputOutput,
                    "World source is unavailable or changed",
                ))
            }
        }
    }
}

impl ReadSource for WorldSource {
    type Open = WorldOpen;
    type Iter = std::vec::IntoIter<RusticResult<ReadSourceEntry<WorldOpen>>>;
    fn size(&self) -> RusticResult<Option<u64>> {
        Ok(None)
    }
    fn entries(&self) -> Self::Iter {
        self.entries
            .iter()
            .cloned()
            .map(Ok)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cancellation_stops_source_mid_read() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(temporary.path().join("level.dat"), vec![7; 1024 * 1024]).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let control = BackupControl::new(cancelled.clone(), move |event| {
            if event.source_bytes > 0 {
                signal.store(true, Ordering::Relaxed);
            }
        });
        let source = WorldSource::new_controlled(temporary.path(), &control).unwrap();
        let mut reader = source
            .entries()
            .next()
            .unwrap()
            .unwrap()
            .open
            .unwrap()
            .open()
            .unwrap();
        let mut buffer = [0; 64 * 1024];
        assert_eq!(reader.read(&mut buffer).unwrap(), buffer.len());
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(reader.read(&mut buffer).is_err());
        assert_eq!(reader.remaining, 1024 * 1024 - 64 * 1024);
        assert!(source.failed());
    }

    #[test]
    fn source_read_failure_is_visible_even_when_rustic_skips_a_file() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(temporary.path().join("level.dat"), b"synthetic metadata").unwrap();
        let source = WorldSource::new(temporary.path()).unwrap();
        let opener = source.entries().next().unwrap().unwrap().open.unwrap();
        fs::remove_file(temporary.path().join("level.dat")).unwrap();
        assert!(opener.open().is_err());
        assert!(source.failed());
    }

    #[test]
    fn directory_replacement_cannot_redirect_source_reads() {
        let temporary = tempfile::tempdir().unwrap();
        let world = temporary.path().join("world");
        let outside = temporary.path().join("outside");
        fs::create_dir(&world).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(world.join("region")).unwrap();
        fs::write(world.join("region/data"), b"approved").unwrap();
        fs::write(outside.join("data"), b"private!").unwrap();
        let source = WorldSource::new(&world).unwrap();
        let opener = source
            .entries()
            .filter_map(|item| item.unwrap().open)
            .next()
            .unwrap();
        fs::rename(world.join("region"), world.join("original-region")).unwrap();
        let link = world.join("region");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let result = std::process::Command::new("cmd")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(&link)
                .arg(&outside)
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            assert!(result.status.success(), "cannot create test junction");
        }
        assert!(opener.open().is_err());
        assert!(source.failed());
        assert!(WorldSource::new(&world).is_err());
        assert_eq!(fs::read(outside.join("data")).unwrap(), b"private!");
        #[cfg(unix)]
        fs::remove_file(link).unwrap();
        #[cfg(windows)]
        fs::remove_dir(link).unwrap();
    }
}

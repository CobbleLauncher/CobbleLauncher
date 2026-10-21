// SPDX-License-Identifier: MIT
//! Local world ZIP backups. Caller holds the stopped-instance operation lease.
use cobble_content::world_directory;
use cobble_store::SafeRelativePath;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io,
    path::Path,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

fn linked(metadata: &Metadata) -> bool {
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

/// Export independent bytes under the world's folder name. Existing archives
/// are never replaced, and failures remove only this call's new partial file.
pub fn export_world_backup(
    game_dir: &Path,
    folder_name: &str,
    destination: &Path,
) -> Result<(), Error> {
    let world = dunce::canonicalize(world_directory(game_dir, folder_name)?)?;
    let parent = dunce::canonicalize(
        destination
            .parent()
            .ok_or("World backup destination has no parent")?,
    )?;
    if parent.starts_with(&world) {
        return Err("World backup must be saved outside the source world".into());
    }
    let destination = parent.join(
        destination
            .file_name()
            .ok_or("World backup destination has no filename")?,
    );
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    let result = (|| -> Result<(), Error> {
        let mut archive = zip::ZipWriter::new(output);
        archive.add_directory(
            format!("{folder_name}/"),
            zip::write::SimpleFileOptions::default(),
        )?;
        backup_directory(&world, folder_name, &mut archive, 0)?;
        archive.finish()?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

fn backup_directory(
    directory: &Path,
    archive_path: &str,
    archive: &mut zip::ZipWriter<File>,
    depth: usize,
) -> Result<(), Error> {
    if depth > 128 {
        return Err("World folder nesting exceeds 128 levels".into());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) {
            return Err("World backup cannot include links or reparse points".into());
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("World backup filenames must be UTF-8")?;
        if depth == 0 && name == "session.lock" && metadata.is_file() {
            continue;
        }
        let relative = format!("{archive_path}/{name}");
        SafeRelativePath::parse(&relative)?;
        if metadata.is_dir() {
            archive.add_directory(
                format!("{relative}/"),
                zip::write::SimpleFileOptions::default(),
            )?;
            backup_directory(&entry.path(), &relative, archive, depth + 1)?;
        } else if metadata.is_file() {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(1))
                .large_file(metadata.len() >= u32::MAX as u64);
            archive.start_file(relative, options)?;
            io::copy(&mut File::open(entry.path())?, archive)?;
        } else {
            return Err("World backup contains an unsupported special file".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-world-backup-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("saves/World/region")).unwrap();
            fs::create_dir(path.join("saves/World/empty")).unwrap();
            fs::write(
                path.join("saves/World/level.dat"),
                b"original metadata bytes",
            )
            .unwrap();
            fs::write(path.join("saves/World/region/r.0.0.mca"), b"region bytes").unwrap();
            fs::write(path.join("saves/World/session.lock"), b"lock").unwrap();
            fs::write(path.join("saves/World/region/session.lock"), b"mod data").unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn backup_roundtrip_has_named_root_nested_bytes_and_does_not_mutate_world() {
        let fixture = Fixture::new();
        let output = fixture.0.join("world.zip");
        export_world_backup(&fixture.0, "World", &output).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
        assert!(archive.by_name("World/empty/").unwrap().is_dir());
        let mut bytes = Vec::new();
        archive
            .by_name("World/region/r.0.0.mca")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"region bytes");
        assert!(archive.by_name("World/session.lock").is_err());
        let mut nested_lock = String::new();
        archive
            .by_name("World/region/session.lock")
            .unwrap()
            .read_to_string(&mut nested_lock)
            .unwrap();
        assert_eq!(nested_lock, "mod data");
        assert_eq!(
            fs::read(fixture.0.join("saves/World/level.dat")).unwrap(),
            b"original metadata bytes"
        );
        assert_eq!(
            fs::read(fixture.0.join("saves/World/session.lock")).unwrap(),
            b"lock"
        );
        let original = fs::read(&output).unwrap();
        assert!(export_world_backup(&fixture.0, "World", &output).is_err());
        assert_eq!(fs::read(output).unwrap(), original);
    }
    #[test]
    fn backup_refuses_outputs_inside_source_and_traversal_before_creating_files() {
        let fixture = Fixture::new();
        let inside = fixture.0.join("saves/World/region/backup.zip");
        assert!(export_world_backup(&fixture.0, "World", &inside).is_err());
        assert!(!inside.exists());
        assert!(
            export_world_backup(&fixture.0, "../World", &fixture.0.join("escape.zip")).is_err()
        );
        assert!(!fixture.0.join("escape.zip").exists());
    }
    #[cfg(unix)]
    #[test]
    fn linked_world_content_fails_and_removes_owned_partial_archive() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink(
            fixture.0.join("saves/World/level.dat"),
            fixture.0.join("saves/World/linked.dat"),
        )
        .unwrap();
        let output = fixture.0.join("world.zip");
        assert!(export_world_backup(&fixture.0, "World", &output).is_err());
        assert!(!output.exists());
        assert!(fixture.0.join("saves/World/level.dat").exists());
    }
    #[cfg(windows)]
    #[test]
    fn nested_junction_fails_without_leaving_partial_backup() {
        use std::os::windows::process::CommandExt;
        let fixture = Fixture::new();
        let link = fixture.0.join("saves").join("World").join("linked");
        let target = fixture.0.join("saves").join("World");
        let output = std::process::Command::new("cmd")
            .args(["/D", "/C"])
            .raw_arg(format!(
                "mklink /J \"{}\" \"{}\"",
                link.display(),
                target.display()
            ))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = fixture.0.join("world.zip");
        assert!(export_world_backup(&fixture.0, "World", &output).is_err());
        assert!(!output.exists());
        assert_eq!(
            fs::read(fixture.0.join("saves/World/level.dat")).unwrap(),
            b"original metadata bytes"
        );
        fs::remove_dir(link).unwrap();
    }
}

// SPDX-License-Identifier: MIT
//! Local stopped-instance copies and self-contained MRPACK backups.
//! Callers must hold their instance operation guard and prevent game launches.
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn is_link(metadata: &Metadata) -> bool {
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

fn source_directory(source: &Path) -> Result<PathBuf, BoxError> {
    let metadata = fs::symlink_metadata(source)?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(invalid("Instance must be a real directory, not a link").into());
    }
    Ok(dunce::canonicalize(source)?)
}

fn transient(name: &str) -> bool {
    matches!(
        name,
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
}

/// Copy regular files into a newly created directory. No mutable files are linked.
/// A failed copy removes only the directory successfully created by this call.
pub fn copy_instance(source: &Path, target: &Path, include_saves: bool) -> Result<(), BoxError> {
    let source = source_directory(source)?;
    let parent = dunce::canonicalize(
        target
            .parent()
            .ok_or_else(|| invalid("Copy target has no parent"))?,
    )?;
    let target = parent.join(
        target
            .file_name()
            .ok_or_else(|| invalid("Copy target has no name"))?,
    );
    if target.starts_with(&source) {
        return Err(invalid("Copy target must be outside the original instance").into());
    }
    fs::create_dir(&target)?;
    let result = copy_tree(&source, &target, include_saves, 0);
    if result.is_err() {
        let _ = fs::remove_dir_all(&target);
    }
    result
}

fn copy_tree(
    source: &Path,
    target: &Path,
    include_saves: bool,
    depth: usize,
) -> Result<(), BoxError> {
    if depth > 128 {
        return Err(invalid("Instance directory nesting exceeds 128 levels").into());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if transient(&name.to_string_lossy()) || (depth == 0 && name == "saves" && !include_saves) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_link(&metadata) {
            continue;
        }
        let destination = target.join(name);
        if metadata.is_dir() {
            fs::create_dir(&destination)?;
            copy_tree(&entry.path(), &destination, include_saves, depth + 1)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(invalid("Instance contains an unsupported special file").into());
        }
    }
    Ok(())
}

/// Create a local backup with gameplay files embedded in overrides. This does
/// not grant redistribution rights for bundled Mods or resource packs.
#[allow(clippy::too_many_arguments)]
pub fn export_instance_mrpack(
    source: &Path,
    destination: &Path,
    name: &str,
    version: &str,
    loader_kind: Option<&str>,
    loader_version: Option<&str>,
    include_saves: bool,
) -> Result<(), BoxError> {
    let source = source_directory(source)?;
    if name.trim().is_empty() {
        return Err(invalid("Pack name cannot be empty").into());
    }
    let mut dependencies = serde_json::Map::new();
    dependencies.insert("minecraft".into(), version.into());
    match (loader_kind, loader_version) {
        (None, None) => {}
        (Some(kind), Some(loader_version)) => {
            let key = match kind {
                "fabric" => "fabric-loader",
                "forge" => "forge",
                "neoforge" => "neoforge",
                _ => return Err(invalid("Unsupported pack loader").into()),
            };
            dependencies.insert(key.into(), loader_version.into());
        }
        _ => return Err(invalid("Loader kind and version must be supplied together").into()),
    }
    let index = serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":"1.0.0","name":name,"files":[],"dependencies":dependencies});
    let parent = dunce::canonicalize(
        destination
            .parent()
            .ok_or_else(|| invalid("Export destination has no parent"))?,
    )?;
    let destination = parent.join(
        destination
            .file_name()
            .ok_or_else(|| invalid("Export destination has no name"))?,
    );
    if destination.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Export destination already exists",
        )
        .into());
    }
    let (temporary_path, temporary) = create_temporary(&parent)?;
    let result = (|| -> Result<(), BoxError> {
        let mut archive = zip::ZipWriter::new(temporary);
        archive.start_file(
            "modrinth.index.json",
            zip::write::SimpleFileOptions::default(),
        )?;
        serde_json::to_writer(&mut archive, &index)?;
        export_tree(
            &source,
            &source,
            &mut archive,
            include_saves,
            0,
            &mut ExportBudget::default(),
        )?;
        archive.finish()?.sync_all()?;
        // Validate the actual output using the import parser: destinations,
        // collision rules, supported dependency combinations and size limits.
        cobble_content::read_mrpack(&temporary_path)?;
        // create_new prevents replacing a user file even if it appeared after
        // the initial check. This also works on volumes without hard links.
        // Publishing is a checked copy, not an atomic rename; on error only
        // this newly created destination is removed.
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        let publish = (|| -> io::Result<()> {
            io::copy(&mut File::open(&temporary_path)?, &mut output)?;
            output.sync_all()
        })();
        drop(output);
        if let Err(error) = publish {
            let _ = fs::remove_file(&destination);
            return Err(error.into());
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temporary_path);
    result
}

fn create_temporary(parent: &Path) -> Result<(PathBuf, File), BoxError> {
    for _ in 0..100 {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".cobble-export-{}-{sequence}.part",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Unable to reserve export temporary file",
    )
    .into())
}

#[derive(Default)]
struct ExportBudget {
    files: usize,
    bytes: u64,
}

fn export_tree(
    source: &Path,
    directory: &Path,
    archive: &mut zip::ZipWriter<File>,
    include_saves: bool,
    depth: usize,
    budget: &mut ExportBudget,
) -> Result<(), BoxError> {
    if depth > 128 {
        return Err(invalid("Instance directory nesting exceeds 128 levels").into());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| invalid("Pack filenames must be UTF-8"))?;
        if transient(name)
            || matches!(
                name,
                "auth"
                    | "accounts"
                    | "launcher_profiles.json"
                    | "launcher_accounts.json"
                    | "offline-profile.json"
                    | "player-profiles.json"
                    | "launcher-preferences.json"
            )
        {
            continue;
        }
        if depth == 0
            && !matches!(
                name,
                "mods"
                    | "config"
                    | "resourcepacks"
                    | "shaderpacks"
                    | "scripts"
                    | "kubejs"
                    | "defaultconfigs"
                    | "options.txt"
                    | "servers.dat"
            )
            && !(name == "saves" && include_saves)
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_link(&metadata) {
            continue;
        }
        if metadata.is_dir() {
            export_tree(
                source,
                &entry.path(),
                archive,
                include_saves,
                depth + 1,
                budget,
            )?;
        } else if metadata.is_file() {
            if metadata.len() > 2 * 1024 * 1024 * 1024 {
                return Err(invalid("Pack file exceeds 2 GiB").into());
            }
            budget.files += 1;
            budget.bytes += metadata.len();
            if budget.files >= 50_000 || budget.bytes > 16 * 1024 * 1024 * 1024 {
                return Err(
                    invalid("Pack exceeds the supported file count or 16 GiB total size").into(),
                );
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(source)?
                .components()
                .map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("Pack filenames must be UTF-8"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            let relative = cobble_store::SafeRelativePath::parse(&relative)?;
            archive.start_file(
                format!("overrides/{}", relative.as_str()),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )?;
            io::copy(&mut File::open(path)?, archive)?;
        } else {
            return Err(invalid("Pack contains an unsupported special file").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-instances-test-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(directory: &Path) -> PathBuf {
        let source = directory.join("source");
        for (path, bytes) in [
            ("mods/original.jar", "synthetic mod"),
            ("config/game.txt", "original"),
            ("saves/World/level.dat", "world"),
            ("logs/latest.log", "log"),
            ("libraries/library.jar", "library"),
            (".cobble-install.json", "marker"),
            (".cobble-pack.json", "pack marker"),
        ] {
            let path = source.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        source
    }
    #[test]
    fn copy_preserves_markers_and_files_are_independent_without_worlds_by_default() {
        let test = TestDirectory::new();
        let source = fixture(&test.0);
        let target = test.0.join("copy");
        copy_instance(&source, &target, false).unwrap();
        assert!(!target.join("saves").exists());
        assert!(!target.join("logs").exists());
        assert_eq!(
            fs::read(target.join(".cobble-pack.json")).unwrap(),
            b"pack marker"
        );
        assert!(target.join("libraries/library.jar").exists());
        assert!(target.join(".cobble-install.json").exists());
        fs::write(target.join("config/game.txt"), "changed").unwrap();
        assert_eq!(
            fs::read(source.join("config/game.txt")).unwrap(),
            b"original"
        );
        assert!(copy_instance(&source, &target, true).is_err());
        assert!(copy_instance(&source, &source.join("nested"), true).is_err());
        copy_instance(&source, &test.0.join("with-worlds"), true).unwrap();
        assert!(test.0.join("with-worlds/saves/World/level.dat").exists());
    }
    #[test]
    fn export_roundtrip_excludes_installation_and_worlds_and_never_overwrites() {
        let test = TestDirectory::new();
        let source = fixture(&test.0);
        let destination = test.0.join("backup.mrpack");
        export_instance_mrpack(
            &source,
            &destination,
            "Original pack",
            "1.21.1",
            Some("fabric"),
            Some("0.16.0"),
            false,
        )
        .unwrap();
        let pack = cobble_content::read_mrpack(&destination).unwrap();
        assert_eq!(pack.name, "Original pack");
        assert_eq!(pack.minecraft_version, "1.21.1");
        assert_eq!(pack.loader_kind.as_deref(), Some("fabric"));
        assert!(pack.files.is_empty());
        assert_eq!(pack.overrides.len(), 2);
        let mut archive = zip::ZipArchive::new(File::open(&destination).unwrap()).unwrap();
        let mut exported_mod = Vec::new();
        std::io::Read::read_to_end(
            &mut archive.by_name("overrides/mods/original.jar").unwrap(),
            &mut exported_mod,
        )
        .unwrap();
        assert_eq!(exported_mod, b"synthetic mod");
        assert!(
            pack.overrides
                .iter()
                .all(|file| matches!(file.path.as_str(), "mods/original.jar" | "config/game.txt"))
        );
        let original = fs::read(&destination).unwrap();
        assert!(
            export_instance_mrpack(&source, &destination, "Other", "1.21.1", None, None, true)
                .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), original);
        let with_saves = test.0.join("worlds.mrpack");
        export_instance_mrpack(
            &source,
            &with_saves,
            "World backup",
            "1.21.1",
            None,
            None,
            true,
        )
        .unwrap();
        assert!(
            cobble_content::read_mrpack(&with_saves)
                .unwrap()
                .overrides
                .iter()
                .any(|file| file.path == "saves/World/level.dat")
        );
        let invalid = test.0.join("invalid.mrpack");
        assert!(
            export_instance_mrpack(&source, &invalid, "Invalid", "../escape", None, None, false)
                .is_err()
        );
        assert!(!invalid.exists());
        assert!(fs::read_dir(&test.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")
        }));
    }
    #[cfg(windows)]
    #[test]
    fn windows_junctions_are_skipped_in_copy_and_export() {
        use std::os::windows::process::CommandExt;
        let test = TestDirectory::new();
        let source = fixture(&test.0);
        let junction = source.join("config").join("external");
        // Directory junction creation needs no developer-mode/admin symlink
        // privilege. Both paths belong to this isolated test fixture.
        let result = std::process::Command::new("cmd")
            .args(["/D", "/C"])
            .raw_arg(format!(
                "mklink /J \"{}\" \"{}\"",
                junction.display(),
                test.0.display()
            ))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "could not create test junction: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(is_link(&fs::symlink_metadata(&junction).unwrap()));
        let target = test.0.join("copy");
        copy_instance(&source, &target, false).unwrap();
        assert!(!target.join("config/external").exists());
        let destination = test.0.join("pack.mrpack");
        export_instance_mrpack(&source, &destination, "Pack", "1.21.1", None, None, false).unwrap();
        assert_eq!(
            cobble_content::read_mrpack(&destination)
                .unwrap()
                .overrides
                .len(),
            2
        );
        fs::remove_dir(&junction).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn links_are_skipped_in_copy_and_export() {
        let test = TestDirectory::new();
        let source = fixture(&test.0);
        std::os::unix::fs::symlink(&test.0, source.join("config/external")).unwrap();
        let target = test.0.join("copy");
        copy_instance(&source, &target, false).unwrap();
        assert!(!target.join("config/external").exists());
        let destination = test.0.join("pack.mrpack");
        export_instance_mrpack(&source, &destination, "Pack", "1.21.1", None, None, false).unwrap();
        assert_eq!(
            cobble_content::read_mrpack(&destination)
                .unwrap()
                .overrides
                .len(),
            2
        );
    }
}

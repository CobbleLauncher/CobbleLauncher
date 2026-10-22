// SPDX-License-Identifier: MIT
//! Direct local world folders and bounded Java level.dat metadata.
use crate::BoxError;
use cobble_store::SafeRelativePath;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_METADATA: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct LocalWorld {
    pub folder_name: String,
    pub name: String,
    pub minecraft_version: Option<String>,
    pub game_mode: Option<i32>,
    pub hardcore: bool,
    pub last_played: Option<i64>,
    pub metadata_error: bool,
}

#[derive(Deserialize)]
struct Level {
    #[serde(rename = "Data")]
    data: WorldData,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WorldData {
    level_name: Option<String>,
    version: Option<WorldVersion>,
    game_type: Option<i32>,
    #[serde(default, rename = "hardcore")]
    hardcore: bool,
    last_played: Option<i64>,
}

#[derive(Deserialize)]
struct WorldVersion {
    #[serde(rename = "Name")]
    name: Option<String>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

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

fn real_directory(path: &Path) -> Result<(), BoxError> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err(invalid("World directory must be a real folder, not a link").into());
    }
    Ok(())
}

fn saves_directory(game_dir: &Path) -> Result<PathBuf, BoxError> {
    real_directory(game_dir)?;
    let saves = game_dir.join("saves");
    match fs::symlink_metadata(&saves) {
        Ok(metadata) if linked(&metadata) || !metadata.is_dir() => {
            return Err(invalid("saves must be a real folder, not a link").into());
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    Ok(saves)
}

/// Host-only location lookup. Renderer callers supply a world folder leaf,
/// never a path. Corrupt metadata does not prevent backing up a world.
pub fn world_directory(game_dir: &Path, folder_name: &str) -> Result<PathBuf, BoxError> {
    let name = SafeRelativePath::parse(folder_name)?;
    if name.as_str().contains('/') {
        return Err(invalid("Expected a direct world folder name").into());
    }
    let path = saves_directory(game_dir)?.join(folder_name);
    real_directory(&path)?;
    let metadata = fs::symlink_metadata(path.join("level.dat"))?;
    if linked(&metadata) || !metadata.is_file() {
        return Err(invalid("World must contain a regular level.dat file").into());
    }
    Ok(fs::canonicalize(path)?)
}

fn read_level(input: impl Read) -> Result<WorldData, BoxError> {
    let compressed = input.take(MAX_METADATA + 1);
    let mut bytes = Vec::new();
    GzDecoder::new(compressed)
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(invalid("World metadata exceeds 8 MiB").into());
    }
    Ok(fastnbt::from_bytes::<Level>(&bytes)?.data)
}

/// Bounded gzip/NBT metadata shared by local folders and ZIP previews.
pub fn read_world_metadata(input: impl Read, folder_name: &str) -> LocalWorld {
    let mut world = LocalWorld {
        folder_name: folder_name.into(),
        name: folder_name.into(),
        minecraft_version: None,
        game_mode: None,
        hardcore: false,
        last_played: None,
        metadata_error: false,
    };
    match read_level(input) {
        Ok(data) => {
            if let Some(name) = data.level_name.filter(|name| !name.trim().is_empty()) {
                world.name = name;
            }
            world.minecraft_version = data.version.and_then(|version| version.name);
            world.game_mode = data.game_type;
            world.hardcore = data.hardcore;
            world.last_played = data.last_played;
        }
        Err(_) => world.metadata_error = true,
    }
    world
}

/// Metadata only: no recursive sizing, icons, network requests or world writes.
pub fn list_local_worlds(game_dir: &Path) -> Result<Vec<LocalWorld>, BoxError> {
    let saves = saves_directory(game_dir)?;
    let entries = match fs::read_dir(saves) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut worlds = Vec::new();
    for entry in entries {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) || !metadata.is_dir() {
            continue;
        }
        let Some(folder_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let level = entry.path().join("level.dat");
        match fs::symlink_metadata(&level) {
            Ok(metadata) if linked(&metadata) || !metadata.is_file() => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
            _ => {}
        }
        let world = match File::open(&level) {
            Ok(file) => read_world_metadata(file, &folder_name),
            Err(_) => LocalWorld {
                folder_name: folder_name.clone(),
                name: folder_name,
                minecraft_version: None,
                game_mode: None,
                hardcore: false,
                last_played: None,
                metadata_error: true,
            },
        };
        worlds.push(world);
    }
    worlds.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then(a.folder_name.cmp(&b.folder_name))
    });
    Ok(worlds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-worlds-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("saves")).unwrap();
            Self(path)
        }
        fn world(&self, folder: &str, data: fastnbt::Value) -> PathBuf {
            let path = self.0.join("saves").join(folder);
            fs::create_dir(&path).unwrap();
            let bytes = fastnbt::to_bytes(&data).unwrap();
            let mut gzip = GzEncoder::new(
                File::create(path.join("level.dat")).unwrap(),
                Compression::default(),
            );
            gzip.write_all(&bytes).unwrap();
            gzip.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn gzip_metadata_partial_legacy_and_corrupt_worlds_stay_visible_and_sorted() {
        let fixture = Fixture::new();
        fixture.world(
            "old",
            fastnbt::nbt!({"Data":{"LevelName":"Old world","GameType":0i32,"LastPlayed":100i64}}),
        );
        fixture.world("new",fastnbt::nbt!({"Data":{"LevelName":"Newest world","Version":{"Name":"1.21.1"},"GameType":1i32,"hardcore":1i8,"LastPlayed":200i64}}));
        fs::create_dir(fixture.0.join("saves/broken")).unwrap();
        fs::write(fixture.0.join("saves/broken/level.dat"), b"broken gzip").unwrap();
        fs::create_dir(fixture.0.join("saves/not-a-world")).unwrap();
        let worlds = list_local_worlds(&fixture.0).unwrap();
        assert_eq!(worlds.len(), 3);
        assert_eq!(worlds[0].name, "Newest world");
        assert_eq!(worlds[0].minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(worlds[0].game_mode, Some(1));
        assert!(worlds[0].hardcore);
        assert!(!worlds[0].metadata_error);
        assert_eq!(worlds[1].minecraft_version, None);
        assert!(!worlds[1].metadata_error);
        assert_eq!(worlds[2].name, "broken");
        assert!(worlds[2].metadata_error);
        assert!(
            world_directory(&fixture.0, "new")
                .unwrap()
                .join("level.dat")
                .exists()
        );
        for name in ["../old", "new/child", "C:\\Windows", ".", ".."][..].iter() {
            assert!(world_directory(&fixture.0, name).is_err());
        }
        assert!(world_directory(&fixture.0, "not-a-world").is_err());
    }
    #[test]
    fn oversized_gzip_metadata_is_reported_as_unreadable_without_hiding_world() {
        let fixture = Fixture::new();
        let path = fixture.0.join("saves/large");
        fs::create_dir(&path).unwrap();
        let mut gzip = GzEncoder::new(
            File::create(path.join("level.dat")).unwrap(),
            Compression::default(),
        );
        gzip.write_all(&vec![0; MAX_METADATA as usize + 1]).unwrap();
        gzip.finish().unwrap();
        let worlds = list_local_worlds(&fixture.0).unwrap();
        assert_eq!(worlds.len(), 1);
        assert!(worlds[0].metadata_error);
    }
    #[cfg(unix)]
    #[test]
    fn linked_worlds_and_level_files_are_not_followed() {
        let fixture = Fixture::new();
        let path = fixture.world("real", fastnbt::nbt!({"Data":{}}));
        std::os::unix::fs::symlink(&path, fixture.0.join("saves/linked")).unwrap();
        fs::create_dir(fixture.0.join("saves/file-link")).unwrap();
        std::os::unix::fs::symlink(
            path.join("level.dat"),
            fixture.0.join("saves/file-link/level.dat"),
        )
        .unwrap();
        assert_eq!(list_local_worlds(&fixture.0).unwrap().len(), 1);
        assert!(world_directory(&fixture.0, "linked").is_err());
        assert!(world_directory(&fixture.0, "file-link").is_err());
    }
    #[cfg(windows)]
    #[test]
    fn world_directory_junction_is_not_followed() {
        use std::os::windows::process::CommandExt;
        let fixture = Fixture::new();
        let path = fixture.world("real", fastnbt::nbt!({"Data":{}}));
        let link = fixture.0.join("saves").join("linked");
        let output = std::process::Command::new("cmd")
            .args(["/D", "/C"])
            .raw_arg(format!(
                "mklink /J \"{}\" \"{}\"",
                link.display(),
                path.display()
            ))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(list_local_worlds(&fixture.0).unwrap().len(), 1);
        assert!(world_directory(&fixture.0, "linked").is_err());
        fs::remove_dir(link).unwrap();
    }
}

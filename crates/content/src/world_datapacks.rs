// SPDX-License-Identifier: MIT
//! World-specific ZIP availability; Minecraft owns enabled-pack selection.
use crate::{BoxError, LocalPack, LocalPackKind, local_packs, world_directory};
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
const ACTIVE: &str = "datapacks";
const REMOVED: &str = ".cobble-removed-datapacks";
#[derive(Debug, Serialize)]
pub struct WorldDatapacks {
    pub installed: Vec<LocalPack>,
    pub removed: Vec<LocalPack>,
}
pub fn list_world_datapacks(game: &Path, world: &str) -> Result<WorldDatapacks, BoxError> {
    let world = world_directory(game, world)?;
    Ok(WorldDatapacks {
        installed: local_packs::list_packs_in(&world, LocalPackKind::ResourcePack, ACTIVE)?,
        removed: local_packs::list_packs_in(&world, LocalPackKind::ResourcePack, REMOVED)?,
    })
}
pub fn import_world_datapacks(
    game: &Path,
    world: &str,
    files: &[PathBuf],
) -> Result<WorldDatapacks, BoxError> {
    let directory = world_directory(game, world)?;
    local_packs::import_packs_in(&directory, LocalPackKind::ResourcePack, files, ACTIVE)?;
    list_world_datapacks(game, world)
}
/// Move original bytes, never overwrite either generation. Removed packs remain
/// inside their world and are included in existing world backup workflows.
pub fn move_world_datapack(
    game: &Path,
    world: &str,
    file_name: &str,
    recover: bool,
) -> Result<WorldDatapacks, BoxError> {
    local_packs::validate_name(file_name)?;
    let directory = world_directory(game, world)?;
    let (from, to) = if recover {
        (REMOVED, ACTIVE)
    } else {
        (ACTIVE, REMOVED)
    };
    let source = local_packs::directory(&directory, from)?.join(file_name);
    let metadata = fs::symlink_metadata(&source)?;
    if local_packs::linked(&metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Datapack must be a regular ZIP file",
        )
        .into());
    }
    let destination = local_packs::directory(&directory, to)?;
    fs::create_dir_all(&destination)?;
    // Copy with create_new preserves an existing destination on every platform.
    let mut input = fs::File::open(&source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination.join(file_name))?;
    let copied = io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
    drop(output);
    if let Err(error) = copied {
        let _ = fs::remove_file(destination.join(file_name));
        return Err(error.into());
    }
    fs::remove_file(source)?;
    list_world_datapacks(game, world)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn exact_world_import_remove_recover_preserves_bytes_and_collisions() {
        let path = std::env::temp_dir().join(format!(
            "cobble-datapacks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        struct Fixture(PathBuf);
        impl Fixture {
            fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Fixture(path);
        for name in ["one", "two"] {
            fs::create_dir_all(root.path().join("saves").join(name)).unwrap();
            fs::write(
                root.path().join("saves").join(name).join("level.dat"),
                b"fixture",
            )
            .unwrap();
        }
        let source = root.path().join("example.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&source).unwrap());
        zip.start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"pack":{"pack_format":48,"description":"fixture"}}"#)
            .unwrap();
        zip.finish().unwrap();
        let bytes = fs::read(&source).unwrap();
        assert_eq!(
            import_world_datapacks(root.path(), "one", &[source])
                .unwrap()
                .installed
                .len(),
            1
        );
        assert!(
            list_world_datapacks(root.path(), "two")
                .unwrap()
                .installed
                .is_empty()
        );
        assert!(move_world_datapack(root.path(), "../one", "example.zip", false).is_err());
        assert_eq!(
            move_world_datapack(root.path(), "one", "example.zip", false)
                .unwrap()
                .removed
                .len(),
            1
        );
        let active = root.path().join("saves/one/datapacks/example.zip");
        fs::write(&active, b"new generation").unwrap();
        assert!(move_world_datapack(root.path(), "one", "example.zip", true).is_err());
        assert_eq!(fs::read(&active).unwrap(), b"new generation");
        fs::remove_file(&active).unwrap();
        assert_eq!(
            move_world_datapack(root.path(), "one", "example.zip", true)
                .unwrap()
                .installed
                .len(),
            1
        );
        assert_eq!(fs::read(active).unwrap(), bytes);
        assert!(
            list_world_datapacks(root.path(), "one")
                .unwrap()
                .removed
                .is_empty()
        );
    }
}

// SPDX-License-Identifier: MIT
use super::*;
use crate::storage_workspace as disk;
use std::sync::atomic::AtomicBool;
fn snapshot(path: &Path, saves: bool, cancel: &AtomicBool) -> Result<disk::Inventory, Error> {
    let mut inventory = disk::inventory(path, cancel)?;
    if saves {
        // The original importer omits only each world's ephemeral session lock.
        inventory.files.retain(|path, _| {
            !(path.components().count() == 2
                && path.file_name().is_some_and(|v| v == "session.lock"))
        });
        inventory.bytes = inventory.files.values().map(|stamp| stamp.bytes).sum();
    }
    Ok(inventory)
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LauncherImportMigration {
    pub recycled_roots: usize,
    pub retained_roots: usize,
    pub issue: Option<String>,
}

/// Host must independently reject an externally running/unknown source process.
/// Only copied isolated gameplay directories may be recycled, never shared game
/// roots, versions, libraries, assets, account stores, or arbitrary loose files.
pub fn release_launcher_import_source(
    target: &Path,
    control: &crate::OperationControl,
    mut recycle: impl FnMut(&Path) -> Result<(), Error>,
) -> Result<LauncherImportMigration, Error> {
    control.check_cancelled()?;
    let target = real_directory(target)?;
    let mut state = read_state(&target)?;
    if !state.complete || state.content_id != "isolated" {
        return Err("IMPORT_MOVE_NOT_ISOLATED".into());
    }
    let source = real_directory(&state.source)?;
    if target.starts_with(&source) || source.starts_with(&target) {
        return Err("IMPORT_MOVE_OVERLAP".into());
    }
    let _locks = world_locks(&source, true)?;
    let cancel = AtomicBool::new(false);
    let mut result = LauncherImportMigration {
        recycled_roots: state.released_roots.len(),
        ..Default::default()
    };
    // Validate all remaining roots before starting any recycling.
    let mut roots = Vec::new();
    for name in &state.entries {
        leaf(name)?;
        if !allowed(name, state.include_saves) {
            return Err("IMPORT_MOVE_INVALID_ROOT".into());
        }
        if state.released_roots.contains(name) {
            continue;
        }
        let from = source.join(name);
        let to = target.join(name);
        let metadata = fs::symlink_metadata(&from)?;
        if linked(&metadata) {
            return Err("IMPORT_MOVE_SOURCE_CHANGED".into());
        }
        if !metadata.is_dir() {
            result.retained_roots += 1;
            continue;
        }
        control.check_cancelled()?;
        let expected = snapshot(&from, name == "saves", &cancel)?;
        if snapshot(&to, name == "saves", &cancel)? != expected {
            return Err("IMPORT_MOVE_SOURCE_CHANGED".into());
        }
        roots.push((name.clone(), from, to, expected));
    }
    for (name, from, to, expected) in roots {
        control.check_cancelled()?;
        if snapshot(&from, name == "saves", &cancel)? != expected
            || snapshot(&to, name == "saves", &cancel)? != expected
        {
            result.retained_roots += 1;
            result.issue = Some("IMPORT_MOVE_SOURCE_CHANGED".into());
            continue;
        }
        match recycle(&from) {
            Ok(()) if !from.try_exists()? => {
                state.released_roots.push(name);
                // A later retry skips only recorded roots. Failure preserves all
                // other source directories and the already installed target.
                write_state(&target, &state, false)?;
                result.recycled_roots += 1;
            }
            _ => {
                result.retained_roots += 1;
                result.issue = Some("IMPORT_MOVE_SOURCE_RETAINED".into());
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        target: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "cobble-import-move-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let source = root.join("source");
            let target = root.join("target");
            for base in [&source, &target] {
                fs::create_dir_all(base.join("mods")).unwrap();
                fs::write(base.join("mods/example.jar"), b"verified fixture").unwrap();
                fs::write(base.join("options.txt"), b"personal preferences").unwrap();
            }
            fs::create_dir(root.join("recycle")).unwrap();
            let fixture = Self {
                root,
                source,
                target,
            };
            fixture.state("isolated", true);
            fixture
        }
        fn state(&self, id: &str, complete: bool) {
            write_state(
                &self.target,
                &ImportState {
                    minecraft_version: "1.21.1".into(),
                    loader_kind: None,
                    loader_version: None,
                    source: real_directory(&self.source).unwrap(),
                    content_id: id.into(),
                    include_saves: false,
                    entries: vec!["mods".into(), "options.txt".into()],
                    staging_complete: true,
                    content_complete: true,
                    complete,
                    runtime_source: None,
                    released_roots: vec![],
                },
                false,
            )
            .unwrap();
        }
        fn control() -> crate::OperationControl {
            crate::OperationControl::new(Arc::new(|_| {}))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn pcl_migration_recycles_only_verified_isolated_roots_after_completion_and_records_retry() {
        let f = Fixture::new();
        let report = release_launcher_import_source(&f.target, &Fixture::control(), |path| {
            fs::rename(path, f.root.join("recycle").join(path.file_name().unwrap()))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(report.recycled_roots, 1);
        assert_eq!(report.retained_roots, 1);
        assert!(f.source.join("options.txt").exists());
        assert!(f.target.join("mods/example.jar").exists());
        let report = release_launcher_import_source(&f.target, &Fixture::control(), |_| {
            panic!("recorded source must not be recycled twice")
        })
        .unwrap();
        assert_eq!(report.recycled_roots, 1);
    }
    #[test]
    fn pcl_migration_retains_changed_shared_incomplete_and_failed_recycle_sources() {
        let f = Fixture::new();
        f.state("shared", true);
        assert!(
            release_launcher_import_source(&f.target, &Fixture::control(), |_| panic!(
                "shared root"
            ))
            .is_err()
        );
        f.state("isolated", false);
        assert!(
            release_launcher_import_source(&f.target, &Fixture::control(), |_| panic!(
                "incomplete"
            ))
            .is_err()
        );
        f.state("isolated", true);
        fs::write(f.source.join("mods/example.jar"), b"later edit").unwrap();
        assert!(
            release_launcher_import_source(&f.target, &Fixture::control(), |_| panic!(
                "changed source"
            ))
            .is_err()
        );
        fs::write(f.source.join("mods/example.jar"), b"verified fixture").unwrap();
        let report = release_launcher_import_source(&f.target, &Fixture::control(), |_| {
            Err("fixture recycle unavailable".into())
        })
        .unwrap();
        assert_eq!(report.recycled_roots, 0);
        assert_eq!(report.retained_roots, 2);
        assert!(f.source.join("mods/example.jar").exists());
        fs::create_dir_all(f.source.join("saves/World")).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(f.source.join("saves/World/session.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            release_launcher_import_source(&f.target, &Fixture::control(), |_| panic!(
                "running world"
            ))
            .is_err()
        );
    }
}

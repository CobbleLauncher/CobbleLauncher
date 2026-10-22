// SPDX-License-Identifier: MIT
//! Explicit updates of locally recorded Modrinth Mods. The caller holds the
//! stopped-instance write guard during apply. Backups permit manual recovery
//! after abrupt termination; this is not an atomic multi-file transaction.
use crate::{
    InstallProgress,
    modrinth::{
        ManagedMod, ModDownloads, Provenance, read_provenance, validate_plan, write_provenance,
    },
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{
    ModVersionSelection, ModrinthPlan, ResolvedMod, find_modrinth_update, modrinth_version_number,
    resolve_modrinth_many, resolve_modrinth_selected,
};
use cobble_store::SafeRelativePath;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, Serialize)]
pub struct ModUpdateCandidate {
    pub project_id: String,
    pub name: String,
    pub current_version: String,
    pub version_number: String,
    pub version_id: String,
    pub filename: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagedModSummary {
    pub project_id: String,
    pub version_id: String,
    pub filename: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModUpdateChange {
    pub project_id: String,
    pub name: String,
    pub from_version: Option<String>,
    pub to_version: String,
    pub filename: String,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct ModUpdatePlan {
    pub changes: Vec<ModUpdateChange>,
    directory: PathBuf,
    provenance: Provenance,
    originals: BTreeMap<String, PresentMod>,
    graph: ModrinthPlan,
}

impl ModUpdatePlan {
    /// Original basename for adapters that retain pack-origin labels across updates.
    pub fn previous_filename(&self, project_id: &str) -> Option<&str> {
        self.originals
            .get(project_id)
            .map(|old| old.recorded.filename.as_str())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModUpdateOutcome {
    pub updated: usize,
    pub added: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PresentMod {
    recorded: ManagedMod,
    actual_filename: String,
    enabled: bool,
}

fn open_root(directory: &Path) -> Result<Dir, Error> {
    Ok(Dir::open_ambient_dir(
        directory,
        cap_std::ambient_authority(),
    )?)
}

fn basename(name: &str) -> Result<(), Error> {
    let safe = SafeRelativePath::parse(name)?;
    if safe.as_str().contains('/')
        || name.starts_with('.')
        || !name.to_ascii_lowercase().ends_with(".jar")
    {
        return Err("Invalid managed Modrinth filename".into());
    }
    Ok(())
}

fn regular_present(directory: &Dir, name: &str) -> Result<bool, Error> {
    match directory.symlink_metadata(name) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(format!("Mod path is not a regular file: {name}").into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn present_mods(
    root: &Dir,
    provenance: &Provenance,
) -> Result<BTreeMap<String, PresentMod>, Error> {
    let mods = match root.open_dir_nofollow("mods") {
        Ok(mods) => mods,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    let mut present = BTreeMap::new();
    for (project, recorded) in provenance {
        basename(&recorded.filename)?;
        let enabled = regular_present(&mods, &recorded.filename)?;
        let disabled_name = format!("{}.disabled", recorded.filename);
        let disabled = regular_present(&mods, &disabled_name)?;
        if enabled && disabled {
            return Err(format!(
                "Both enabled and disabled copies exist: {}",
                recorded.filename
            )
            .into());
        }
        if enabled || disabled {
            present.insert(
                project.clone(),
                PresentMod {
                    recorded: recorded.clone(),
                    actual_filename: if enabled {
                        recorded.filename.clone()
                    } else {
                        disabled_name
                    },
                    enabled,
                },
            );
        }
    }
    Ok(present)
}

pub fn managed_mod_count(directory: &Path) -> Result<usize, Error> {
    let root = open_root(directory)?;
    Ok(present_mods(&root, &read_provenance(&root)?)?.len())
}

pub fn list_managed_modrinth_mods(directory: &Path) -> Result<Vec<ManagedModSummary>, Error> {
    let root = open_root(directory)?;
    Ok(present_mods(&root, &read_provenance(&root)?)?
        .into_iter()
        .map(|(project_id, present)| ManagedModSummary {
            project_id,
            version_id: present.recorded.version_id,
            filename: present.recorded.filename,
            enabled: present.enabled,
        })
        .collect())
}

fn validate_selections(
    present: &BTreeMap<String, PresentMod>,
    selections: &[ModVersionSelection],
) -> Result<bool, Error> {
    if selections.is_empty() || selections.len() > 128 {
        return Err("请选择 1–128 个已管理的 Mod".into());
    }
    let mut ids = BTreeSet::new();
    let mut changed = false;
    for selection in selections {
        if !ids.insert(&selection.project_id) {
            return Err("重复选择了同一个 Mod".into());
        }
        let old = present
            .get(&selection.project_id)
            .ok_or("只能更改实际存在且已识别的 Modrinth Mod 版本")?;
        changed |= old.recorded.version_id != selection.version_id;
    }
    Ok(changed)
}

/// Explicit compatible version changes, including downgrades and prereleases.
/// Only present managed roots may be replaced; apply uses the same backup flow.
pub async fn prepare_modrinth_version_changes(
    directory: &Path,
    mc: &str,
    loader: &str,
    selections: &[ModVersionSelection],
) -> Result<ModUpdatePlan, Error> {
    let root = open_root(directory)?;
    let provenance = read_provenance(&root)?;
    let present = present_mods(&root, &provenance)?;
    if !validate_selections(&present, selections)? {
        return Ok(ModUpdatePlan {
            changes: Vec::new(),
            directory: dunce::canonicalize(directory)?,
            provenance,
            originals: BTreeMap::new(),
            graph: ModrinthPlan {
                project_id: selections[0].project_id.clone(),
                version_id: selections[0].version_id.clone(),
                mods: Vec::new(),
            },
        });
    }
    let mut graph = resolve_modrinth_selected(selections, mc, loader).await?;
    let mut labels = BTreeMap::new();
    for item in &mut graph.mods {
        if let Some(old) = present.get(&item.project_id) {
            // Recognized local JARs may have user-chosen basenames. Selecting
            // their existing version must not rename an otherwise equal file.
            if old.recorded.version_id == item.version_id
                && old.recorded.sha1.eq_ignore_ascii_case(&item.file.sha1)
            {
                item.file.filename = old.recorded.filename.clone();
            } else {
                labels.insert(
                    item.project_id.clone(),
                    modrinth_version_number(&item.project_id, &old.recorded.version_id).await?,
                );
            }
        }
    }
    build_plan(directory, provenance, present, graph, &labels)
}

pub async fn check_modrinth_updates(
    directory: &Path,
    mc: &str,
    loader: &str,
) -> Result<Vec<ModUpdateCandidate>, Error> {
    let root = open_root(directory)?;
    let present = present_mods(&root, &read_provenance(&root)?)?;
    let mut candidates = Vec::new();
    for (project, current) in present {
        if let Some(update) =
            find_modrinth_update(&project, &current.recorded.version_id, mc, loader).await?
        {
            let selected = update.selected;
            candidates.push(ModUpdateCandidate {
                project_id: project,
                name: selected.name,
                current_version: update.current_version,
                version_number: selected.version_number,
                version_id: selected.version_id,
                filename: current.recorded.filename,
                enabled: current.enabled,
            });
        }
    }
    Ok(candidates)
}

pub async fn prepare_modrinth_updates(
    directory: &Path,
    mc: &str,
    loader: &str,
    selected_projects: &[String],
) -> Result<ModUpdatePlan, Error> {
    if selected_projects.is_empty() || selected_projects.len() > 128 {
        return Err("Select between 1 and 128 managed Mods".into());
    }
    let root = open_root(directory)?;
    let provenance = read_provenance(&root)?;
    let present = present_mods(&root, &provenance)?;
    let mut selected = BTreeMap::new();
    for project in selected_projects {
        let current = present
            .get(project)
            .ok_or("Only present managed Modrinth Mods can be updated")?;
        let update = find_modrinth_update(project, &current.recorded.version_id, mc, loader)
            .await?
            .ok_or("Selected Mod has no newer compatible release; check updates again")?;
        if selected.insert(project.clone(), update).is_some() {
            return Err("Duplicate selected Mod".into());
        }
    }
    let graph = resolve_modrinth_many(selected_projects, mc, loader).await?;
    // Metadata may change while resolving. Never silently substitute a different
    // root version or turn an update into an older required pin.
    for (project, update) in &selected {
        if !graph.mods.iter().any(|item| {
            item.project_id == *project && item.version_id == update.selected.version_id
        }) {
            return Err("Modrinth releases changed during preview; check updates again".into());
        }
    }
    let mut labels: BTreeMap<_, _> = selected
        .into_iter()
        .map(|(id, update)| (id, update.current_version))
        .collect();
    for item in &graph.mods {
        if let Some(old) = present.get(&item.project_id)
            && !labels.contains_key(&item.project_id)
            && (old.recorded.version_id != item.version_id
                || old.recorded.filename != item.file.filename
                || !old.recorded.sha1.eq_ignore_ascii_case(&item.file.sha1))
        {
            labels.insert(
                item.project_id.clone(),
                modrinth_version_number(&item.project_id, &old.recorded.version_id).await?,
            );
        }
    }
    build_plan(directory, provenance, present, graph, &labels)
}

fn build_plan(
    directory: &Path,
    provenance: Provenance,
    present: BTreeMap<String, PresentMod>,
    graph: ModrinthPlan,
    labels: &BTreeMap<String, String>,
) -> Result<ModUpdatePlan, Error> {
    validate_plan(&graph)?;
    let any_enabled_root = graph
        .mods
        .iter()
        .filter(|item| item.required_by.is_none())
        .any(|item| present.get(&item.project_id).is_some_and(|old| old.enabled));
    let mut changes = Vec::new();
    let mut originals = BTreeMap::new();
    for item in &graph.mods {
        if let Some(old) = present.get(&item.project_id) {
            originals.insert(item.project_id.clone(), old.clone());
            if old.recorded.version_id == item.version_id
                && old.recorded.filename == item.file.filename
                && old.recorded.sha1.eq_ignore_ascii_case(&item.file.sha1)
            {
                continue;
            }
        }
        let old = present.get(&item.project_id);
        changes.push(ModUpdateChange {
            project_id: item.project_id.clone(),
            name: item.name.clone(),
            from_version: if old.is_some() {
                Some(
                    labels
                        .get(&item.project_id)
                        .ok_or("Missing installed version display metadata")?
                        .clone(),
                )
            } else {
                None
            },
            to_version: item.version_number.clone(),
            filename: item.file.filename.clone(),
            enabled: old.map_or(any_enabled_root, |old| old.enabled),
        });
    }
    let plan = ModUpdatePlan {
        changes,
        directory: dunce::canonicalize(directory)?,
        provenance,
        originals,
        graph,
    };
    preflight(&open_root(directory)?, &plan)?;
    Ok(plan)
}

fn verify_old(mods: &Dir, old: &PresentMod) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = mods.open_with(&old.actual_filename, &options)?;
    if !file.metadata()?.is_file() {
        return Err("Managed Mod is not a regular file".into());
    }
    let mut hash = Sha1::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&old.recorded.sha1) {
        return Err(format!(
            "Managed Mod changed locally: {}; preserve it or reinstall before updating",
            old.actual_filename
        )
        .into());
    }
    Ok(())
}

fn preflight(root: &Dir, plan: &ModUpdatePlan) -> Result<(), Error> {
    if read_provenance(root)? != plan.provenance {
        return Err("Managed Mods changed since preview; check updates again".into());
    }
    let present = present_mods(root, &plan.provenance)?;
    let mods = root.open_dir_nofollow("mods")?;
    for (id, old) in &plan.originals {
        if present.get(id) != Some(old) {
            return Err(
                "Managed Mod selection or enabled state changed; check updates again".into(),
            );
        }
        verify_old(&mods, old)?;
    }
    let owned_names: BTreeSet<_> = plan
        .changes
        .iter()
        .filter_map(|change| {
            plan.originals
                .get(&change.project_id)
                .map(|old| old.actual_filename.clone())
        })
        .collect();
    let mut names = BTreeSet::new();
    for change in &plan.changes {
        for name in [&change.filename, &format!("{}.disabled", change.filename)] {
            if !names.insert(name.to_ascii_lowercase()) {
                return Err("Update plan has conflicting filenames".into());
            }
            for entry in mods.entries()? {
                let name_on_disk = entry?.file_name().to_string_lossy().into_owned();
                if name_on_disk.eq_ignore_ascii_case(name) && !owned_names.contains(&name_on_disk) {
                    return Err(format!(
                        "Update would overwrite another local Mod: {name_on_disk}"
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

pub async fn apply_modrinth_updates(
    directory: &Path,
    plan: &ModUpdatePlan,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<ModUpdateOutcome, Error> {
    if dunce::canonicalize(directory)? != plan.directory {
        return Err("Update plan belongs to another game".into());
    }
    let root = open_root(directory)?;
    preflight(&root, plan)?;
    if plan.changes.is_empty() {
        on_progress(InstallProgress {
            phase: "complete".into(),
            completed: 0,
            total: 0,
            reused: 0,
        });
        return Ok(ModUpdateOutcome {
            updated: 0,
            added: 0,
        });
    }
    let downloads = ModDownloads::open()?;
    let mut artifacts = Vec::new();
    for (index, change) in plan.changes.iter().enumerate() {
        let item = graph_item(plan, &change.project_id)?;
        artifacts.push(downloads.fetch(item).await?);
        on_progress(InstallProgress {
            phase: "mod-download".into(),
            completed: index + 1,
            total: plan.changes.len(),
            reused: 0,
        });
    }
    preflight(&root, plan)?;
    let outcome = publish_updates(&root, plan, |change, staging| {
        let index = plan
            .changes
            .iter()
            .position(|entry| entry.project_id == change.project_id)
            .ok_or("Unknown update item")?;
        downloads.content.copy_to(
            &artifacts[index],
            staging,
            &SafeRelativePath::parse(&change.filename)?,
        )?;
        Ok(())
    })?;
    on_progress(InstallProgress {
        phase: "complete".into(),
        completed: plan.changes.len(),
        total: plan.changes.len(),
        reused: 0,
    });
    Ok(outcome)
}

fn graph_item<'a>(plan: &'a ModUpdatePlan, project: &str) -> Result<&'a ResolvedMod, Error> {
    plan.graph
        .mods
        .iter()
        .find(|item| item.project_id == project)
        .ok_or_else(|| "Update plan is missing an artifact".into())
}

fn publish_updates(
    root: &Dir,
    plan: &ModUpdatePlan,
    stage: impl FnMut(&ModUpdateChange, &Dir) -> Result<(), Error>,
) -> Result<ModUpdateOutcome, Error> {
    publish_updates_with_hook(root, plan, stage, |_| Ok(()))
}

fn publish_updates_with_hook(
    root: &Dir,
    plan: &ModUpdatePlan,
    mut stage: impl FnMut(&ModUpdateChange, &Dir) -> Result<(), Error>,
    mut after_publish: impl FnMut(usize) -> Result<(), Error>,
) -> Result<ModUpdateOutcome, Error> {
    preflight(root, plan)?;
    root.create_dir_all(".cobble-mod-backups")?;
    let backups = root.open_dir_nofollow(".cobble-mod-backups")?;
    let id = format!(
        "{}-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
        std::process::id()
    );
    backups.create_dir(&id)?;
    let backup = backups.open_dir_nofollow(&id)?;
    backup.create_dir("old")?;
    backup.create_dir("new")?;
    let old_files = backup.open_dir_nofollow("old")?;
    let new_files = backup.open_dir_nofollow("new")?;
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut snapshot = backup.open_with("provenance.json", &options)?;
    serde_json::to_writer_pretty(&mut snapshot, &plan.provenance)?;
    snapshot.write_all(b"\n")?;
    snapshot.sync_all()?;
    drop(snapshot);
    // All new JAR copies are staged before the first old file is moved.
    for change in &plan.changes {
        stage(change, &new_files)?;
    }
    preflight(root, plan)?;
    let mods = root.open_dir_nofollow("mods")?;
    let mut moved = Vec::new();
    let mut published = Vec::new();
    let result = (|| -> Result<ModUpdateOutcome, Error> {
        for change in &plan.changes {
            if let Some(old) = plan.originals.get(&change.project_id) {
                mods.rename(&old.actual_filename, &old_files, &old.actual_filename)?;
                moved.push(old.actual_filename.clone());
            }
        }
        for (index, change) in plan.changes.iter().enumerate() {
            let filename = if change.enabled {
                change.filename.clone()
            } else {
                format!("{}.disabled", change.filename)
            };
            new_files.rename(&change.filename, &mods, &filename)?;
            published.push(filename);
            after_publish(index)?;
        }
        let mut provenance = plan.provenance.clone();
        for change in &plan.changes {
            let selected = graph_item(plan, &change.project_id)?;
            provenance.insert(
                change.project_id.clone(),
                ManagedMod {
                    version_id: selected.version_id.clone(),
                    filename: selected.file.filename.clone(),
                    sha1: selected.file.sha1.clone(),
                },
            );
        }
        write_provenance(root, &provenance)?;
        Ok(ModUpdateOutcome {
            updated: plan
                .changes
                .iter()
                .filter(|change| change.from_version.is_some())
                .count(),
            added: plan
                .changes
                .iter()
                .filter(|change| change.from_version.is_none())
                .count(),
        })
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<(), Error> {
            for filename in published.iter().rev() {
                mods.remove_file(filename)?;
            }
            for filename in moved.iter().rev() {
                old_files.rename(filename, &mods, filename)?;
            }
            write_provenance(root, &plan.provenance)?;
            Ok(())
        })();
        return match rollback { Ok(()) => Err(error), Err(restore) => Err(format!("Update failed: {error}; rollback incomplete: {restore}. Recover from .cobble-mod-backups/{id}").into()) };
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_content::ModrinthFile;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-modupdates-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("mods")).unwrap();
            Self(path)
        }
        fn root(&self) -> Dir {
            open_root(&self.0).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn selected(project: &str, filename: &str, bytes: &[u8]) -> ResolvedMod {
        ResolvedMod {
            project_id: project.into(),
            version_id: format!("{project}-new"),
            name: project.into(),
            version_number: "2.0".into(),
            required_by: if project == "Root" {
                None
            } else {
                Some("Root".into())
            },
            file: ModrinthFile {
                filename: filename.into(),
                url: format!("https://cdn.modrinth.com/data/{project}/versions/new/{filename}"),
                size: bytes.len() as u64,
                sha1: format!("{:x}", Sha1::digest(bytes)),
                sha512: None,
            },
        }
    }

    fn fixture_plan(fixture: &Fixture, disabled: bool) -> ModUpdatePlan {
        let root = fixture.root();
        let recorded = ManagedMod {
            version_id: "Root-old".into(),
            filename: "root-old.jar".into(),
            sha1: format!("{:x}", Sha1::digest(b"old-one")),
        };
        let filename = if disabled {
            "root-old.jar.disabled"
        } else {
            "root-old.jar"
        };
        root.write(format!("mods/{filename}"), b"old-one").unwrap();
        root.write("mods/unmanaged.jar", b"leave-me").unwrap();
        root.create_dir_all("config").unwrap();
        root.write("config/settings.txt", b"keep configuration")
            .unwrap();
        let provenance = BTreeMap::from([("Root".into(), recorded)]);
        write_provenance(&root, &provenance).unwrap();
        let present = present_mods(&root, &provenance).unwrap();
        let graph = ModrinthPlan {
            project_id: "Root".into(),
            version_id: "Root-new".into(),
            mods: vec![
                selected("Root", "root-new.jar", b"new-one"),
                selected("Dep", "dep.jar", b"new-dep"),
            ],
        };
        build_plan(
            &fixture.0,
            provenance,
            present,
            graph,
            &BTreeMap::from([("Root".into(), "1.0".into())]),
        )
        .unwrap()
    }

    fn stage(change: &ModUpdateChange, target: &Dir) -> Result<(), Error> {
        target.write(
            &change.filename,
            if change.project_id == "Root" {
                b"new-one"
            } else {
                b"new-dep"
            },
        )?;
        Ok(())
    }

    #[test]
    fn disabled_update_keeps_state_adds_dependencies_and_preserves_unmanaged_content() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, true);
        assert_eq!(managed_mod_count(&fixture.0).unwrap(), 1);
        assert!(plan.changes.iter().all(|change| !change.enabled));
        let root = fixture.root();
        let result = publish_updates(&root, &plan, stage).unwrap();
        assert_eq!((result.updated, result.added), (1, 1));
        assert_eq!(root.read("mods/root-new.jar.disabled").unwrap(), b"new-one");
        assert_eq!(root.read("mods/dep.jar.disabled").unwrap(), b"new-dep");
        assert!(root.symlink_metadata("mods/root-old.jar.disabled").is_err());
        assert_eq!(root.read("mods/unmanaged.jar").unwrap(), b"leave-me");
        assert_eq!(
            root.read("config/settings.txt").unwrap(),
            b"keep configuration"
        );
        assert_eq!(
            read_provenance(&root).unwrap()["Root"].version_id,
            "Root-new"
        );
        let backups = root.open_dir_nofollow(".cobble-mod-backups").unwrap();
        let entry = backups.entries().unwrap().next().unwrap().unwrap();
        let backup = backups.open_dir_nofollow(entry.file_name()).unwrap();
        assert_eq!(
            backup.read("old/root-old.jar.disabled").unwrap(),
            b"old-one"
        );
        assert_eq!(managed_mod_count(&fixture.0).unwrap(), 2);
    }

    #[test]
    fn managed_summary_is_local_and_explicit_older_version_reuses_disabled_backup_flow() {
        let fixture = Fixture::new();
        let old_plan = fixture_plan(&fixture, true);
        let root = fixture.root();
        let mut provenance = old_plan.provenance.clone();
        provenance.insert(
            "Missing".into(),
            ManagedMod {
                version_id: "gone".into(),
                filename: "gone.jar".into(),
                sha1: "0".repeat(40),
            },
        );
        write_provenance(&root, &provenance).unwrap();
        let listed = list_managed_modrinth_mods(&fixture.0).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].project_id, "Root");
        assert_eq!(listed[0].version_id, "Root-old");
        assert_eq!(listed[0].filename, "root-old.jar");
        assert!(!listed[0].enabled);
        let mut older = selected("Root", "older.jar", b"new-one");
        older.version_id = "Root-older".into();
        older.version_number = "0.5-beta".into();
        let graph = ModrinthPlan {
            project_id: "Root".into(),
            version_id: "Root-older".into(),
            mods: vec![older],
        };
        let present = present_mods(&root, &provenance).unwrap();
        let plan = build_plan(
            &fixture.0,
            provenance,
            present,
            graph,
            &BTreeMap::from([("Root".into(), "1.0".into())]),
        )
        .unwrap();
        assert_eq!(plan.changes[0].to_version, "0.5-beta");
        assert!(!plan.changes[0].enabled);
        let outcome = publish_updates(&root, &plan, stage).unwrap();
        assert_eq!((outcome.updated, outcome.added), (1, 0));
        assert_eq!(root.read("mods/older.jar.disabled").unwrap(), b"new-one");
        assert_eq!(
            root.read("config/settings.txt").unwrap(),
            b"keep configuration"
        );
        assert_eq!(
            read_provenance(&root).unwrap()["Root"].version_id,
            "Root-older"
        );
        let backups = root.open_dir_nofollow(".cobble-mod-backups").unwrap();
        let entry = backups.entries().unwrap().next().unwrap().unwrap();
        assert_eq!(
            backups
                .open_dir_nofollow(entry.file_name())
                .unwrap()
                .read("old/root-old.jar.disabled")
                .unwrap(),
            b"old-one"
        );
    }

    #[test]
    fn selecting_installed_version_is_noop_without_network_or_empty_backups() {
        let fixture = Fixture::new();
        fixture_plan(&fixture, true);
        let selection = ModVersionSelection {
            project_id: "Root".into(),
            version_id: "Root-old".into(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let plan = runtime
            .block_on(prepare_modrinth_version_changes(
                &fixture.0,
                "1.21.1",
                "fabric",
                std::slice::from_ref(&selection),
            ))
            .unwrap();
        assert!(plan.changes.is_empty());
        let outcome = runtime
            .block_on(apply_modrinth_updates(&fixture.0, &plan, Arc::new(|_| {})))
            .unwrap();
        assert_eq!((outcome.updated, outcome.added), (0, 0));
        assert!(!fixture.0.join(".cobble-mod-backups").exists());
        assert_eq!(
            fixture.root().read("mods/root-old.jar.disabled").unwrap(),
            b"old-one"
        );
        let missing = ModVersionSelection {
            project_id: "unmanaged".into(),
            version_id: "any".into(),
        };
        assert!(
            runtime
                .block_on(prepare_modrinth_version_changes(
                    &fixture.0,
                    "1.21.1",
                    "fabric",
                    &[missing]
                ))
                .is_err()
        );
        assert!(
            runtime
                .block_on(prepare_modrinth_version_changes(
                    &fixture.0,
                    "1.21.1",
                    "fabric",
                    &[selection.clone(), selection]
                ))
                .is_err()
        );
    }

    #[test]
    fn publication_failure_rolls_back_old_jars_and_marker() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, false);
        let root = fixture.root();
        let error = publish_updates_with_hook(&root, &plan, stage, |_| {
            Err("synthetic failure after first publication".into())
        })
        .unwrap_err();
        assert!(error.to_string().contains("synthetic failure"));
        assert_eq!(root.read("mods/root-old.jar").unwrap(), b"old-one");
        assert!(root.symlink_metadata("mods/root-new.jar").is_err());
        assert!(root.symlink_metadata("mods/dep.jar").is_err());
        assert_eq!(read_provenance(&root).unwrap(), plan.provenance);
    }

    #[test]
    fn staging_failure_and_stale_or_colliding_files_never_replace_old_mods() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, false);
        let root = fixture.root();
        assert!(
            publish_updates(&root, &plan, |_, _| Err("synthetic disk failure".into())).is_err()
        );
        assert_eq!(root.read("mods/root-old.jar").unwrap(), b"old-one");
        assert_eq!(read_provenance(&root).unwrap(), plan.provenance);
        root.write("mods/root-new.jar.disabled", b"unmanaged collision")
            .unwrap();
        assert!(preflight(&root, &plan).is_err());
        assert_eq!(
            root.read("mods/root-new.jar.disabled").unwrap(),
            b"unmanaged collision"
        );
        root.remove_file("mods/root-new.jar.disabled").unwrap();
        root.rename("mods/root-old.jar", &root, "mods/root-old.jar.disabled")
            .unwrap();
        assert!(preflight(&root, &plan).is_err());
        root.rename("mods/root-old.jar.disabled", &root, "mods/root-old.jar")
            .unwrap();
        root.write("mods/root-old.jar", b"modified old mod")
            .unwrap();
        assert!(preflight(&root, &plan).is_err());
    }

    #[test]
    fn missing_managed_files_and_unmanaged_jars_are_not_counted() {
        let fixture = Fixture::new();
        let _plan = fixture_plan(&fixture, false);
        let root = fixture.root();
        root.remove_file("mods/root-old.jar").unwrap();
        assert_eq!(managed_mod_count(&fixture.0).unwrap(), 0);
    }
}

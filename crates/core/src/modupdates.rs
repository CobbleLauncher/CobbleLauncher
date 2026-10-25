// SPDX-License-Identifier: MIT
//! Explicit updates of locally recorded Modrinth Mods. The caller holds the
//! stopped-instance write guard during apply. One pending marker and backups
//! recover interrupted replacement; this is not an atomic multi-file transaction.
#[cfg(test)]
use crate::modrinth::write_provenance;
use crate::{
    InstallProgress,
    modrinth::{
        ManagedMod, ModDownloads, ModProvider, Provenance, read_provenance,
        read_provider_provenance, validate_provider_plan, write_provider_provenance,
    },
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{
    ModVersionSelection, ModrinthPlan, ResolvedMod, modrinth_version_number,
    resolve_modrinth_selected,
};
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

type Error = Box<dyn std::error::Error + Send + Sync>;
pub(crate) mod history;
mod policy;
pub use history::{
    ModOperationFile, ModOperationHistory, list_mod_operation_history, preview_mod_operation_undo,
    undo_mod_operation,
};
pub use policy::{
    ModProjectPolicy, ModUpdateChannel, ModUpdatePolicy, read_mod_update_policy,
    set_mod_update_policy,
};

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
    pub pinned: bool,
}

const PINS: &str = ".cobble-mod-pins.json";
type ModPins = BTreeMap<String, String>;
fn pin_key(provider: ModProvider, project: &str) -> String {
    match provider {
        ModProvider::Modrinth => project.into(),
        ModProvider::CurseForge => format!("curseforge:{project}"),
    }
}

fn read_pins(root: &Dir) -> Result<ModPins, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    match root.open_with(PINS, &options) {
        Ok(file) => Ok(serde_json::from_reader(file.take(1024 * 1024))?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ModPins::new()),
        Err(error) => Err(error.into()),
    }
}

/// Pin the exact installed Modrinth version for one game. Old games have no pins.
pub fn set_modrinth_mod_pinned(
    directory: &Path,
    project_id: &str,
    pinned: bool,
) -> Result<(), Error> {
    set_provider_mod_pinned(directory, project_id, pinned, ModProvider::Modrinth)
}
pub fn set_curseforge_mod_pinned(
    directory: &Path,
    project_id: &str,
    pinned: bool,
) -> Result<(), Error> {
    set_provider_mod_pinned(directory, project_id, pinned, ModProvider::CurseForge)
}
fn set_provider_mod_pinned(
    directory: &Path,
    project_id: &str,
    pinned: bool,
    provider: ModProvider,
) -> Result<(), Error> {
    let root = open_root(directory)?;
    let present = present_mods(&root, &read_provider_provenance(&root, provider)?)?;
    let installed = present
        .get(project_id)
        .ok_or("Only present managed Mods can be pinned")?;
    let mut pins = read_pins(&root)?;
    if pinned {
        pins.insert(
            pin_key(provider, project_id),
            installed.recorded.version_id.clone(),
        );
    } else {
        pins.remove(&pin_key(provider, project_id));
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(".cobble-mod-pins.json.tmp", &options)?;
    serde_json::to_writer_pretty(&mut file, &pins)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(".cobble-mod-pins.json.tmp", &root, PINS)?;
    Ok(())
}

fn reject_pinned_replacements(root: &Dir, plan: &ModUpdatePlan) -> Result<(), Error> {
    let pins = read_pins(root)?;
    for change in &plan.changes {
        if let Some(old) = plan.originals.get(&change.project_id)
            && pins.get(&pin_key(plan.provider, &change.project_id))
                == Some(&old.recorded.version_id)
        {
            return Err(format!("Unpin {} before changing its version", change.name).into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModUpdateChange {
    pub project_id: String,
    pub name: String,
    pub from_version: Option<String>,
    pub to_version: String,
    pub filename: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModUpdatePlan {
    #[serde(default)]
    provider: ModProvider,
    pub changes: Vec<ModUpdateChange>,
    directory: PathBuf,
    provenance: Provenance,
    originals: BTreeMap<String, PresentMod>,
    graph: ModrinthPlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    automatic_policy: Option<ModUpdatePolicy>,
}

impl ModUpdatePlan {
    pub fn is_curseforge(&self) -> bool {
        self.provider == ModProvider::CurseForge
    }
    pub(crate) fn automatic_with_policy(mut self, policy: ModUpdatePolicy) -> Result<Self, Error> {
        self.automatic_policy = Some(policy);
        policy::validate_plan(&open_root(&self.directory)?, &self)?;
        Ok(self)
    }
    pub fn version_selections(&self) -> Vec<ModVersionSelection> {
        self.graph
            .mods
            .iter()
            .filter(|item| item.required_by.is_none())
            .map(|item| ModVersionSelection {
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
            })
            .collect()
    }
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    let pins = read_pins(&root)?;
    Ok(present_mods(&root, &read_provenance(&root)?)?
        .into_iter()
        .map(|(project_id, present)| ManagedModSummary {
            pinned: pins.get(&project_id) == Some(&present.recorded.version_id),
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
            provider: ModProvider::Modrinth,
            changes: Vec::new(),
            directory: dunce::canonicalize(directory)?,
            provenance,
            originals: BTreeMap::new(),
            graph: ModrinthPlan {
                project_id: selections[0].project_id.clone(),
                version_id: selections[0].version_id.clone(),
                mods: Vec::new(),
            },
            automatic_policy: None,
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
    let pins = read_pins(&root)?;
    let policy = policy::read(&root)?;
    let mut candidates = Vec::new();
    for (project, current) in present {
        if pins.get(&project) == Some(&current.recorded.version_id) {
            continue;
        }
        if let Some((installed, selected)) =
            find_policy_update(&policy, &project, &current.recorded.version_id, mc, loader).await?
        {
            candidates.push(ModUpdateCandidate {
                project_id: project,
                name: selected.name,
                current_version: installed.version_number,
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
    let policy = policy::read(&root)?;
    let pins = read_pins(&root)?;
    let mut selected = BTreeMap::new();
    for project in selected_projects {
        let current = present
            .get(project)
            .ok_or("Only present managed Modrinth Mods can be updated")?;
        if pins.get(project) == Some(&current.recorded.version_id) {
            return Err("Unpin the selected Mod before changing its version".into());
        }
        let update = find_policy_update(&policy, project, &current.recorded.version_id, mc, loader)
            .await?
            .ok_or("Selected Mod has no newer compatible release; check updates again")?;
        if selected.insert(project.clone(), update).is_some() {
            return Err("Duplicate selected Mod".into());
        }
    }
    let selections: Vec<_> = selected
        .iter()
        .map(|(project, (_, next))| ModVersionSelection {
            project_id: project.clone(),
            version_id: next.version_id.clone(),
        })
        .collect();
    let graph = resolve_modrinth_selected(&selections, mc, loader).await?;
    // Metadata may change while resolving. Never silently substitute a different
    // root version or turn an update into an older required pin.
    for (project, (_, update)) in &selected {
        if !graph
            .mods
            .iter()
            .any(|item| item.project_id == *project && item.version_id == update.version_id)
        {
            return Err("Modrinth releases changed during preview; check updates again".into());
        }
    }
    let mut labels: BTreeMap<_, _> = selected
        .into_iter()
        .map(|(id, (installed, _))| (id, installed.version_number))
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
    let mut plan = build_plan(directory, provenance, present, graph, &labels)?;
    plan.automatic_policy = Some(policy);
    policy::validate_plan(&root, &plan)?;
    Ok(plan)
}

async fn find_policy_update(
    policy: &ModUpdatePolicy,
    project: &str,
    current_version: &str,
    mc: &str,
    loader: &str,
) -> Result<
    Option<(
        cobble_content::ModVersionDetails,
        cobble_content::ModVersion,
    )>,
    Error,
> {
    let current =
        cobble_content::modrinth_version_details(project, current_version, mc, loader).await?;
    let versions = cobble_content::list_modrinth_versions(project, mc, loader).await?;
    Ok(policy::select(policy, project, &current, &versions)
        .cloned()
        .map(|next| (current, next)))
}

fn build_plan(
    directory: &Path,
    provenance: Provenance,
    present: BTreeMap<String, PresentMod>,
    graph: ModrinthPlan,
    labels: &BTreeMap<String, String>,
) -> Result<ModUpdatePlan, Error> {
    build_provider_plan(
        directory,
        provenance,
        present,
        graph,
        labels,
        ModProvider::Modrinth,
    )
}
pub fn prepare_curseforge_update(
    directory: &Path,
    plan: &cobble_content::curseforge::InstallPlan,
) -> Result<ModUpdatePlan, Error> {
    crate::curseforge::check_curseforge_incompatibilities(directory, plan)?;
    let root = open_root(directory)?;
    let provenance = read_provider_provenance(&root, ModProvider::CurseForge)?;
    let present = present_mods(&root, &provenance)?;
    if !present.contains_key(&plan.graph.project_id) {
        return Err("Only present managed CurseForge Mods can be updated".into());
    }
    let labels = present
        .iter()
        .map(|(id, item)| (id.clone(), item.recorded.version_id.clone()))
        .collect();
    build_provider_plan(
        directory,
        provenance,
        present,
        plan.graph.clone(),
        &labels,
        ModProvider::CurseForge,
    )
}
pub fn list_managed_curseforge_mods(directory: &Path) -> Result<Vec<ManagedModSummary>, Error> {
    let root = open_root(directory)?;
    let pins = read_pins(&root)?;
    Ok(present_mods(
        &root,
        &read_provider_provenance(&root, ModProvider::CurseForge)?,
    )?
    .into_iter()
    .map(|(project_id, item)| ManagedModSummary {
        pinned: pins.get(&pin_key(ModProvider::CurseForge, &project_id))
            == Some(&item.recorded.version_id),
        project_id,
        version_id: item.recorded.version_id,
        filename: item.recorded.filename,
        enabled: item.enabled,
    })
    .collect())
}
/// Also covers a new root whose required graph would replace an existing pinned Mod.
pub fn validate_curseforge_mod_pins(directory: &Path, graph: &ModrinthPlan) -> Result<(), Error> {
    let root = open_root(directory)?;
    let pins = read_pins(&root)?;
    let present = present_mods(
        &root,
        &read_provider_provenance(&root, ModProvider::CurseForge)?,
    )?;
    for selected in &graph.mods {
        if let Some(old) = present.get(&selected.project_id)
            && pins.get(&pin_key(ModProvider::CurseForge, &selected.project_id))
                == Some(&old.recorded.version_id)
            && (old.recorded.version_id != selected.version_id
                || old.recorded.filename != selected.file.filename
                || !old.recorded.sha1.eq_ignore_ascii_case(&selected.file.sha1))
        {
            return Err(format!("Unpin {} before changing its version", selected.name).into());
        }
    }
    Ok(())
}
fn build_provider_plan(
    directory: &Path,
    provenance: Provenance,
    present: BTreeMap<String, PresentMod>,
    graph: ModrinthPlan,
    labels: &BTreeMap<String, String>,
    provider: ModProvider,
) -> Result<ModUpdatePlan, Error> {
    validate_provider_plan(&graph, provider)?;
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
        provider,
        changes,
        directory: dunce::canonicalize(directory)?,
        provenance,
        originals,
        graph,
        automatic_policy: None,
    };
    let root = open_root(directory)?;
    reject_pinned_replacements(&root, &plan)?;
    preflight(&root, &plan)?;
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
    crate::modrinth::check_provider_ownership(root, &plan.graph, plan.provider)?;
    if read_provider_provenance(root, plan.provider)? != plan.provenance {
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
    apply_modrinth_updates_controlled(directory, plan, crate::OperationControl::new(on_progress))
        .await
}

pub async fn apply_modrinth_updates_controlled(
    directory: &Path,
    plan: &ModUpdatePlan,
    control: crate::OperationControl,
) -> Result<ModUpdateOutcome, Error> {
    if plan.provider != ModProvider::Modrinth {
        return Err("incorrect update provider".into());
    }
    apply_provider_updates_controlled(directory, plan, control, None).await
}
pub async fn apply_curseforge_updates_controlled(
    directory: &Path,
    plan: &ModUpdatePlan,
    key: String,
    control: crate::OperationControl,
) -> Result<ModUpdateOutcome, Error> {
    if plan.provider != ModProvider::CurseForge {
        return Err("incorrect update provider".into());
    }
    apply_provider_updates_controlled(directory, plan, control, Some(key)).await
}
async fn apply_provider_updates_controlled(
    directory: &Path,
    plan: &ModUpdatePlan,
    control: crate::OperationControl,
    key: Option<String>,
) -> Result<ModUpdateOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    if dunce::canonicalize(directory)? != plan.directory {
        return Err("Update plan belongs to another game".into());
    }
    recover_modrinth_update(directory)?;
    let root = open_root(directory)?;
    policy::validate_plan(&root, plan)?;
    reject_pinned_replacements(&root, plan)?;
    if read_provider_provenance(&root, plan.provider)? != plan.provenance
        && already_applied(&root, plan)?
    {
        return Ok(requested_outcome(plan));
    }
    preflight(&root, plan)?;
    if plan.changes.is_empty() {
        on_progress(InstallProgress {
            detail: None,
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
    let downloads = ModDownloads::open_for(plan.provider, key)?;
    let mut artifacts = Vec::new();
    for (index, change) in plan.changes.iter().enumerate() {
        control.check_cancelled()?;
        let item = graph_item(plan, &change.project_id)?;
        artifacts.push(
            downloads
                .fetch_file_controlled(&item.file, &control)
                .await?,
        );
        on_progress(InstallProgress {
            detail: None,
            phase: "mod-download".into(),
            completed: index + 1,
            total: plan.changes.len(),
            reused: 0,
        });
    }
    control.check_cancelled()?;
    preflight(&root, plan)?;
    let change_count = plan.changes.len();
    on_progress(InstallProgress {
        detail: None,
        phase: "mod-install".into(),
        completed: 0,
        total: change_count,
        reused: 0,
    });
    let stage_progress = on_progress.clone();
    let plan = plan.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        control.check_cancelled()?;
        publish_updates_inner(
            &root,
            &plan,
            |change, staging| {
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
                stage_progress(InstallProgress {
                    detail: None,
                    phase: "mod-install".into(),
                    completed: index + 1,
                    total: change_count,
                    reused: 0,
                });
                Ok(())
            },
            |_| Ok(()),
            Some(&control),
        )
    })
    .await??;
    on_progress(InstallProgress {
        detail: None,
        phase: "complete".into(),
        completed: change_count,
        total: change_count,
        reused: 0,
    });
    Ok(outcome)
}

fn requested_outcome(plan: &ModUpdatePlan) -> ModUpdateOutcome {
    ModUpdateOutcome {
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
    }
}

fn already_applied(root: &Dir, plan: &ModUpdatePlan) -> Result<bool, Error> {
    let provenance = read_provider_provenance(root, plan.provider)?;
    for item in &plan.graph.mods {
        let Some(current) = provenance.get(&item.project_id) else {
            return Ok(false);
        };
        if current.version_id != item.version_id
            || current.filename != item.file.filename
            || !current.sha1.eq_ignore_ascii_case(&item.file.sha1)
        {
            return Ok(false);
        }
    }
    let present = present_mods(root, &provenance)?;
    Ok(plan.changes.iter().all(|change| {
        present.get(&change.project_id).is_some_and(|item| {
            item.enabled == change.enabled && item.recorded.filename == change.filename
        })
    }))
}

fn graph_item<'a>(plan: &'a ModUpdatePlan, project: &str) -> Result<&'a ResolvedMod, Error> {
    plan.graph
        .mods
        .iter()
        .find(|item| item.project_id == project)
        .ok_or_else(|| "Update plan is missing an artifact".into())
}

#[cfg(test)]
fn publish_updates(
    root: &Dir,
    plan: &ModUpdatePlan,
    stage: impl FnMut(&ModUpdateChange, &Dir) -> Result<(), Error>,
) -> Result<ModUpdateOutcome, Error> {
    publish_updates_with_hook(root, plan, stage, |_| Ok(()))
}

#[cfg(test)]
fn publish_updates_with_hook(
    root: &Dir,
    plan: &ModUpdatePlan,
    stage: impl FnMut(&ModUpdateChange, &Dir) -> Result<(), Error>,
    after_publish: impl FnMut(usize) -> Result<(), Error>,
) -> Result<ModUpdateOutcome, Error> {
    publish_updates_inner(root, plan, stage, after_publish, None)
}

fn publish_updates_inner(
    root: &Dir,
    plan: &ModUpdatePlan,
    mut stage: impl FnMut(&ModUpdateChange, &Dir) -> Result<(), Error>,
    mut after_publish: impl FnMut(usize) -> Result<(), Error>,
    control: Option<&crate::OperationControl>,
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
        if let Some(control) = control {
            control.check_cancelled()?;
        }
        stage(change, &new_files)?;
    }
    preflight(root, plan)?;
    if let Some(control) = control {
        control.check_cancelled()?;
    }
    let pending = PendingUpdate {
        provider: plan.provider,
        backup_id: id.clone(),
        changes: plan
            .changes
            .iter()
            .map(|change| PendingChange {
                old_filename: plan
                    .originals
                    .get(&change.project_id)
                    .map(|old| old.actual_filename.clone()),
                staged_filename: change.filename.clone(),
                new_filename: if change.enabled {
                    change.filename.clone()
                } else {
                    format!("{}.disabled", change.filename)
                },
            })
            .collect(),
    };
    write_pending(root, &pending)?;
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
        write_provider_provenance(root, &provenance, plan.provider)?;
        history::record_update(&backup, &id, plan)?;
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
            write_provider_provenance(root, &plan.provenance, plan.provider)?;
            Ok(())
        })();
        return match rollback { Ok(()) => { root.remove_file(PENDING)?; Err(error) }, Err(restore) => Err(format!("Update failed: {error}; rollback incomplete: {restore}. Recover from .cobble-mod-backups/{id}").into()) };
    }
    root.remove_file(PENDING)?;
    result
}

const PENDING: &str = ".cobble-mod-update.json";

pub(crate) fn ensure_update_complete(directory: &Path) -> Result<(), Error> {
    if list_mod_operation_history(directory)?
        .iter()
        .any(|record| record.status == "undoing")
    {
        return Err("Mod undo is incomplete; recover retained files from .cobble-mod-backups before continuing".into());
    }
    match std::fs::symlink_metadata(directory.join(PENDING)) {
        Ok(_) => Err("Mod 更新尚未恢复，请先重试任务或恢复更新".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[derive(Serialize, Deserialize)]
struct PendingChange {
    old_filename: Option<String>,
    staged_filename: String,
    new_filename: String,
}
#[derive(Serialize, Deserialize)]
struct PendingUpdate {
    #[serde(default)]
    provider: ModProvider,
    backup_id: String,
    changes: Vec<PendingChange>,
}

fn write_pending(root: &Dir, state: &PendingUpdate) -> Result<(), Error> {
    if root.symlink_metadata(PENDING).is_ok() {
        return Err("已有未恢复的 Mod 更新，请先恢复".into());
    }
    let temporary = ".cobble-mod-update.json.tmp";
    if root.symlink_metadata(temporary).is_ok() {
        root.remove_file(temporary)?;
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(temporary, &options)?;
    serde_json::to_writer(&mut file, state)?;
    file.sync_all()?;
    drop(file);
    root.rename(temporary, root, PENDING)?;
    Ok(())
}

fn recovery_name(directory: &Dir, name: &str) -> Result<String, Error> {
    for suffix in 0..10_000 {
        let name = format!("{suffix}-{name}");
        if directory
            .symlink_metadata(&name)
            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
        {
            return Ok(name);
        }
    }
    Err("恢复目录中的同名文件过多，请先查看 Mod 备份".into())
}

/// Roll back one interrupted replacement under the caller's stopped-game lease.
/// Published/conflicting current files are retained in the backup's recovery/
/// folder. This is a small file recovery protocol, not multi-file atomicity.
pub fn recover_modrinth_update(directory: &Path) -> Result<bool, Error> {
    let root = open_root(directory)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let input = match root.open_with(PENDING, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let pending: PendingUpdate = serde_json::from_reader(input.take(1024 * 1024))?;
    let backup_id = SafeRelativePath::parse(&pending.backup_id)?;
    if backup_id.as_str().contains('/') || pending.changes.len() > 256 {
        return Err("Mod 更新恢复记录无效".into());
    }
    for change in &pending.changes {
        basename(&change.staged_filename)?;
        basename(
            change
                .new_filename
                .strip_suffix(".disabled")
                .unwrap_or(&change.new_filename),
        )?;
        if let Some(old) = &change.old_filename {
            basename(old.strip_suffix(".disabled").unwrap_or(old))?;
        }
    }
    let backup = root
        .open_dir_nofollow(".cobble-mod-backups")?
        .open_dir_nofollow(backup_id.as_str())?;
    let old = backup.open_dir_nofollow("old")?;
    let new = backup.open_dir_nofollow("new")?;
    backup.create_dir_all("recovery")?;
    let preserved = backup.open_dir_nofollow("recovery")?;
    let mods = root.open_dir_nofollow("mods")?;
    // First preserve published outputs; only afterwards restore originals.
    for change in &pending.changes {
        let original_waiting = change
            .old_filename
            .as_ref()
            .map(|name| regular_present(&old, name))
            .transpose()?
            .unwrap_or(false);
        if !regular_present(&new, &change.staged_filename)?
            && regular_present(&mods, &change.new_filename)?
            && (change.old_filename.as_ref() != Some(&change.new_filename) || original_waiting)
        {
            mods.rename(
                &change.new_filename,
                &preserved,
                recovery_name(&preserved, &change.new_filename)?,
            )?;
        }
    }
    for change in &pending.changes {
        if let Some(name) = &change.old_filename
            && regular_present(&old, name)?
        {
            if regular_present(&mods, name)? {
                mods.rename(name, &preserved, recovery_name(&preserved, name)?)?;
            }
            old.rename(name, &mods, name)?;
        }
    }
    let provenance: Provenance = serde_json::from_reader(
        backup
            .open_with("provenance.json", &options)?
            .take(4 * 1024 * 1024),
    )?;
    write_provider_provenance(&root, &provenance, pending.provider)?;
    history::mark_recovered(&backup)?;
    root.remove_file(PENDING)?;
    Ok(true)
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
    fn update_policy_round_trips_and_stale_automatic_previews_are_rejected() {
        let fixture = Fixture::new();
        assert_eq!(
            read_mod_update_policy(&fixture.0).unwrap(),
            ModUpdatePolicy::default()
        );
        let mut plan = fixture_plan(&fixture, false);
        plan.automatic_policy = Some(ModUpdatePolicy::default());
        let mut policy = ModUpdatePolicy::default();
        policy.projects.insert(
            "modrinth:Dep".into(),
            ModProjectPolicy {
                channel: Some(ModUpdateChannel::Preview),
                ignored_versions: BTreeSet::from(["Dep-new".into()]),
            },
        );
        set_mod_update_policy(&fixture.0, &policy).unwrap();
        assert_eq!(read_mod_update_policy(&fixture.0).unwrap(), policy);
        assert!(policy::validate_plan(&fixture.root(), &plan).is_err());
        // Explicit selections keep the exact graph, including required ignored dependencies.
        plan.automatic_policy = None;
        assert!(policy::validate_plan(&fixture.root(), &plan).is_ok());
        assert!(plan.changes.iter().any(|change| change.project_id == "Dep"));
        publish_updates(&fixture.root(), &plan, stage).unwrap();
        assert!(fixture.root().try_exists("mods/dep.jar").unwrap());
    }

    #[test]
    fn ignored_root_can_be_manually_selected_but_cannot_bypass_a_pin() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, false);
        let mut policy = ModUpdatePolicy::default();
        policy.projects.insert(
            "modrinth:Root".into(),
            ModProjectPolicy {
                channel: None,
                ignored_versions: BTreeSet::from(["Root-new".into()]),
            },
        );
        set_mod_update_policy(&fixture.0, &policy).unwrap();
        assert!(policy::validate_plan(&fixture.root(), &plan).is_ok());
        set_modrinth_mod_pinned(&fixture.0, "Root", true).unwrap();
        assert!(reject_pinned_replacements(&fixture.root(), &plan).is_err());
        set_modrinth_mod_pinned(&fixture.0, "Root", false).unwrap();
        assert!(reject_pinned_replacements(&fixture.root(), &plan).is_ok());
    }

    #[test]
    fn pins_default_off_and_block_root_or_dependency_replacement() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, false);
        assert!(!list_managed_modrinth_mods(&fixture.0).unwrap()[0].pinned);
        set_modrinth_mod_pinned(&fixture.0, "Root", true).unwrap();
        assert!(list_managed_modrinth_mods(&fixture.0).unwrap()[0].pinned);
        assert!(reject_pinned_replacements(&fixture.root(), &plan).is_err());
        set_modrinth_mod_pinned(&fixture.0, "Root", false).unwrap();
        assert!(reject_pinned_replacements(&fixture.root(), &plan).is_ok());

        let root = fixture.root();
        root.write("mods/dep-old.jar", b"old-dep").unwrap();
        let mut provenance = read_provenance(&root).unwrap();
        provenance.insert(
            "Dep".into(),
            ManagedMod {
                version_id: "Dep-old".into(),
                filename: "dep-old.jar".into(),
                sha1: format!("{:x}", Sha1::digest(b"old-dep")),
            },
        );
        write_provenance(&root, &provenance).unwrap();
        set_modrinth_mod_pinned(&fixture.0, "Dep", true).unwrap();
        let present = present_mods(&root, &provenance).unwrap();
        let graph = plan.graph.clone();
        let error = build_plan(
            &fixture.0,
            provenance,
            present,
            graph,
            &BTreeMap::from([("Root".into(), "1.0".into()), ("Dep".into(), "1.0".into())]),
        );
        assert!(error.unwrap_err().to_string().contains("Unpin Dep"));
    }

    #[test]
    fn history_undo_restores_disabled_root_and_removes_only_added_dependency() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, true);
        publish_updates(&fixture.root(), &plan, stage).unwrap();
        let history = list_mod_operation_history(&fixture.0).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].files.len(), 2);
        undo_mod_operation(&fixture.0, &history[0].operation_id).unwrap();
        let root = fixture.root();
        assert_eq!(root.read("mods/root-old.jar.disabled").unwrap(), b"old-one");
        assert!(!root.exists("mods/root-new.jar.disabled"));
        assert!(!root.exists("mods/dep.jar.disabled"));
        assert_eq!(root.read("mods/unmanaged.jar").unwrap(), b"leave-me");
        assert_eq!(read_provenance(&root).unwrap(), plan.provenance);
        assert_eq!(
            list_mod_operation_history(&fixture.0).unwrap()[0].status,
            "undone"
        );
    }

    #[test]
    fn history_undo_keeps_unchanged_managed_dependency() {
        let fixture = Fixture::new();
        let original = fixture_plan(&fixture, true);
        let root = fixture.root();
        root.write("mods/dep.jar.disabled", b"new-dep").unwrap();
        let mut provenance = original.provenance.clone();
        provenance.insert(
            "Dep".into(),
            ManagedMod {
                version_id: "Dep-new".into(),
                filename: "dep.jar".into(),
                sha1: format!("{:x}", Sha1::digest(b"new-dep")),
            },
        );
        write_provenance(&root, &provenance).unwrap();
        let plan = build_plan(
            &fixture.0,
            provenance.clone(),
            present_mods(&root, &provenance).unwrap(),
            original.graph,
            &BTreeMap::from([("Root".into(), "1.0".into())]),
        )
        .unwrap();
        assert_eq!(plan.changes.len(), 1);
        publish_updates(&root, &plan, stage).unwrap();
        let id = list_mod_operation_history(&fixture.0).unwrap()[0]
            .operation_id
            .clone();
        undo_mod_operation(&fixture.0, &id).unwrap();
        assert_eq!(root.read("mods/dep.jar.disabled").unwrap(), b"new-dep");
        assert_eq!(read_provenance(&root).unwrap(), provenance);
    }

    #[test]
    fn history_undo_refuses_changed_dependency_and_new_local_collision_before_writes() {
        for changed_dependency in [true, false] {
            let fixture = Fixture::new();
            let plan = fixture_plan(&fixture, false);
            let root = fixture.root();
            publish_updates(&root, &plan, stage).unwrap();
            let id = list_mod_operation_history(&fixture.0).unwrap()[0]
                .operation_id
                .clone();
            if changed_dependency {
                root.write("mods/dep.jar", b"locally changed").unwrap();
            } else {
                root.write("mods/root-old.jar.disabled", b"local collision")
                    .unwrap();
            }
            assert!(undo_mod_operation(&fixture.0, &id).is_err());
            assert_eq!(root.read("mods/root-new.jar").unwrap(), b"new-one");
            assert_eq!(
                root.read("mods/dep.jar").unwrap(),
                if changed_dependency {
                    b"locally changed".as_slice()
                } else {
                    b"new-dep".as_slice()
                }
            );
        }
    }

    #[test]
    fn history_undo_failure_rolls_back_and_can_retry() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, true);
        let root = fixture.root();
        publish_updates(&root, &plan, stage).unwrap();
        let id = list_mod_operation_history(&fixture.0).unwrap()[0]
            .operation_id
            .clone();
        let error = history::undo_inner(&fixture.0, &id, |_| Err("synthetic disk failure".into()))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("original operation remains applied"),
            "{error}"
        );
        assert_eq!(root.read("mods/root-new.jar.disabled").unwrap(), b"new-one");
        assert_eq!(root.read("mods/dep.jar.disabled").unwrap(), b"new-dep");
        assert!(!root.exists("mods/root-old.jar.disabled"));
        undo_mod_operation(&fixture.0, &id).unwrap();
    }

    #[test]
    fn interrupted_undo_resumes_from_retained_outputs_without_overwriting_local_changes() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, true);
        let root = fixture.root();
        publish_updates(&root, &plan, stage).unwrap();
        let id = list_mod_operation_history(&fixture.0).unwrap()[0]
            .operation_id
            .clone();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = history::undo_inner(&fixture.0, &id, |_| {
                panic!("synthetic process interruption")
            });
        }));
        assert!(interrupted.is_err());
        assert_eq!(
            list_mod_operation_history(&fixture.0).unwrap()[0].status,
            "undoing"
        );
        assert!(ensure_update_complete(&fixture.0).is_err());
        root.write("mods/dep.jar.disabled", b"later-local-file")
            .unwrap();
        assert!(preview_mod_operation_undo(&fixture.0, &id).is_err());
        assert!(undo_mod_operation(&fixture.0, &id).is_err());
        assert_eq!(
            root.read("mods/dep.jar.disabled").unwrap(),
            b"later-local-file"
        );
        root.remove_file("mods/dep.jar.disabled").unwrap();
        preview_mod_operation_undo(&fixture.0, &id).unwrap();
        undo_mod_operation(&fixture.0, &id).unwrap();
        assert_eq!(root.read("mods/root-old.jar.disabled").unwrap(), b"old-one");
        assert!(!root.exists("mods/dep.jar.disabled"));
        assert_eq!(read_provenance(&root).unwrap(), plan.provenance);
        ensure_update_complete(&fixture.0).unwrap();
    }

    #[test]
    fn cf_policy_ignored_required_dependencies_are_preserved_in_publication() {
        let fixture = Fixture::new();
        let mut original = fixture_plan(&fixture, false);
        original.provider = ModProvider::CurseForge;
        for selected in &mut original.graph.mods {
            selected.file.url.clear();
        }
        write_provider_provenance(
            &fixture.root(),
            &original.provenance,
            ModProvider::CurseForge,
        )
        .unwrap();
        let mut policy = ModUpdatePolicy::default();
        policy.projects.insert(
            "curseforge:Dep".into(),
            ModProjectPolicy {
                channel: None,
                ignored_versions: BTreeSet::from(["Dep-new".into()]),
            },
        );
        set_mod_update_policy(&fixture.0, &policy).unwrap();
        let source = cobble_content::curseforge::InstallPlan {
            graph: original.graph,
            restricted: Vec::new(),
            incompatible: Vec::new(),
        };
        let plan = prepare_curseforge_update(&fixture.0, &source)
            .unwrap()
            .automatic_with_policy(policy)
            .unwrap();
        assert!(plan.changes.iter().any(|change| change.project_id == "Dep"));
        publish_updates(&fixture.root(), &plan, stage).unwrap();
        assert!(fixture.root().try_exists("mods/dep.jar").unwrap());
    }

    #[test]
    fn cf_policy_pins_are_provider_scoped_and_block_roots_and_required_replacements() {
        let fixture = Fixture::new();
        let mut original = fixture_plan(&fixture, false);
        for selected in &mut original.graph.mods {
            selected.file.url.clear();
        }
        write_provider_provenance(
            &fixture.root(),
            &original.provenance,
            ModProvider::CurseForge,
        )
        .unwrap();
        let source = cobble_content::curseforge::InstallPlan {
            graph: original.graph,
            restricted: Vec::new(),
            incompatible: Vec::new(),
        };
        set_modrinth_mod_pinned(&fixture.0, "Root", true).unwrap();
        assert!(prepare_curseforge_update(&fixture.0, &source).is_ok());
        set_curseforge_mod_pinned(&fixture.0, "Root", true).unwrap();
        assert!(list_managed_curseforge_mods(&fixture.0).unwrap()[0].pinned);
        assert!(prepare_curseforge_update(&fixture.0, &source).is_err());
        assert!(validate_curseforge_mod_pins(&fixture.0, &source.graph).is_err());
        set_curseforge_mod_pinned(&fixture.0, "Root", false).unwrap();
        let root = fixture.root();
        root.write("mods/dep-old.jar", b"old-dep").unwrap();
        let mut provenance = read_provider_provenance(&root, ModProvider::CurseForge).unwrap();
        provenance.insert(
            "Dep".into(),
            ManagedMod {
                version_id: "Dep-old".into(),
                filename: "dep-old.jar".into(),
                sha1: format!("{:x}", Sha1::digest(b"old-dep")),
            },
        );
        write_provider_provenance(&root, &provenance, ModProvider::CurseForge).unwrap();
        set_curseforge_mod_pinned(&fixture.0, "Dep", true).unwrap();
        assert!(prepare_curseforge_update(&fixture.0, &source).is_err());
        assert!(validate_curseforge_mod_pins(&fixture.0, &source.graph).is_err());
    }

    #[test]
    fn cf_policy_stale_automatic_preview_is_rejected_but_manual_exact_still_checks_pins() {
        let fixture = Fixture::new();
        let mut original = fixture_plan(&fixture, false);
        for selected in &mut original.graph.mods {
            selected.file.url.clear();
        }
        write_provider_provenance(
            &fixture.root(),
            &original.provenance,
            ModProvider::CurseForge,
        )
        .unwrap();
        let source = cobble_content::curseforge::InstallPlan {
            graph: original.graph,
            restricted: Vec::new(),
            incompatible: Vec::new(),
        };
        let automatic = prepare_curseforge_update(&fixture.0, &source)
            .unwrap()
            .automatic_with_policy(ModUpdatePolicy::default())
            .unwrap();
        let mut changed = ModUpdatePolicy::default();
        changed.projects.insert(
            "curseforge:Root".into(),
            ModProjectPolicy {
                channel: Some(ModUpdateChannel::Preview),
                ignored_versions: BTreeSet::from(["Root-new".into()]),
            },
        );
        set_mod_update_policy(&fixture.0, &changed).unwrap();
        assert!(policy::validate_plan(&fixture.root(), &automatic).is_err());
        let manual = prepare_curseforge_update(&fixture.0, &source).unwrap();
        assert!(policy::validate_plan(&fixture.root(), &manual).is_ok());
        set_curseforge_mod_pinned(&fixture.0, "Root", true).unwrap();
        assert!(reject_pinned_replacements(&fixture.root(), &manual).is_err());
    }

    #[test]
    fn curseforge_publication_keeps_provider_and_disabled_state_and_rolls_back_failure() {
        let fixture = Fixture::new();
        let mut plan = fixture_plan(&fixture, true);
        plan.provider = ModProvider::CurseForge;
        for item in &mut plan.graph.mods {
            item.file.url = String::new();
        }
        let root = fixture.root();
        write_provider_provenance(&root, &plan.provenance, ModProvider::CurseForge).unwrap();
        let other = BTreeMap::from([(
            "other-provider".into(),
            ManagedMod {
                version_id: "unchanged".into(),
                filename: "other.jar".into(),
                sha1: "a".repeat(40),
            },
        )]);
        write_provenance(&root, &other).unwrap();
        let failed = publish_updates_with_hook(&root, &plan, stage, |_| {
            Err("synthetic publication failure".into())
        });
        assert!(failed.is_err());
        assert_eq!(root.read("mods/root-old.jar.disabled").unwrap(), b"old-one");
        assert_eq!(
            read_provider_provenance(&root, ModProvider::CurseForge).unwrap(),
            plan.provenance
        );
        assert_eq!(read_provenance(&root).unwrap(), other);
        let outcome = publish_updates(&root, &plan, stage).unwrap();
        assert_eq!((outcome.updated, outcome.added), (1, 1));
        assert_eq!(root.read("mods/root-new.jar.disabled").unwrap(), b"new-one");
        assert_eq!(root.read("mods/dep.jar.disabled").unwrap(), b"new-dep");
        assert_eq!(read_provenance(&root).unwrap(), other);
        assert_eq!(list_managed_curseforge_mods(&fixture.0).unwrap().len(), 2);
        assert!(!root.symlink_metadata(PENDING).is_ok());
    }

    #[test]
    fn interrupted_curseforge_update_recovers_only_its_own_provenance() {
        let fixture = Fixture::new();
        let mut plan = fixture_plan(&fixture, true);
        plan.provider = ModProvider::CurseForge;
        let root = fixture.root();
        write_provider_provenance(&root, &plan.provenance, ModProvider::CurseForge).unwrap();
        let modrinth_before = root.read(crate::modrinth::MARKER).unwrap();
        root.create_dir_all(".cobble-mod-backups/interrupted/old")
            .unwrap();
        root.create_dir_all(".cobble-mod-backups/interrupted/new")
            .unwrap();
        let backup = root
            .open_dir_nofollow(".cobble-mod-backups/interrupted")
            .unwrap();
        backup
            .write(
                "provenance.json",
                serde_json::to_vec(&plan.provenance).unwrap(),
            )
            .unwrap();
        let old = backup.open_dir_nofollow("old").unwrap();
        let new = backup.open_dir_nofollow("new").unwrap();
        for change in &plan.changes {
            stage(change, &new).unwrap();
        }
        write_pending(
            &root,
            &PendingUpdate {
                provider: ModProvider::CurseForge,
                backup_id: "interrupted".into(),
                changes: plan
                    .changes
                    .iter()
                    .map(|change| PendingChange {
                        old_filename: plan
                            .originals
                            .get(&change.project_id)
                            .map(|item| item.actual_filename.clone()),
                        staged_filename: change.filename.clone(),
                        new_filename: format!("{}.disabled", change.filename),
                    })
                    .collect(),
            },
        )
        .unwrap();
        let mods = root.open_dir_nofollow("mods").unwrap();
        mods.rename("root-old.jar.disabled", &old, "root-old.jar.disabled")
            .unwrap();
        new.rename("root-new.jar", &mods, "root-new.jar.disabled")
            .unwrap();
        assert!(recover_modrinth_update(&fixture.0).unwrap());
        assert_eq!(mods.read("root-old.jar.disabled").unwrap(), b"old-one");
        assert!(!mods.symlink_metadata("root-new.jar.disabled").is_ok());
        assert_eq!(
            read_provider_provenance(&root, ModProvider::CurseForge).unwrap(),
            plan.provenance
        );
        assert_eq!(root.read(crate::modrinth::MARKER).unwrap(), modrinth_before);
        assert!(!recover_modrinth_update(&fixture.0).unwrap());
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
    fn interrupted_replacement_recovers_before_move_after_move_after_publish_and_after_provenance()
    {
        for interrupted_at in 0..=3 {
            let fixture = Fixture::new();
            let plan = fixture_plan(&fixture, true);
            let root = fixture.root();
            root.create_dir_all(".cobble-mod-backups/interrupted/old")
                .unwrap();
            root.create_dir_all(".cobble-mod-backups/interrupted/new")
                .unwrap();
            let backup = root
                .open_dir_nofollow(".cobble-mod-backups")
                .unwrap()
                .open_dir_nofollow("interrupted")
                .unwrap();
            backup
                .write(
                    "provenance.json",
                    serde_json::to_vec(&plan.provenance).unwrap(),
                )
                .unwrap();
            let old = backup.open_dir_nofollow("old").unwrap();
            let new = backup.open_dir_nofollow("new").unwrap();
            for change in &plan.changes {
                stage(change, &new).unwrap();
            }
            write_pending(
                &root,
                &PendingUpdate {
                    provider: plan.provider,
                    backup_id: "interrupted".into(),
                    changes: plan
                        .changes
                        .iter()
                        .map(|change| PendingChange {
                            old_filename: plan
                                .originals
                                .get(&change.project_id)
                                .map(|old| old.actual_filename.clone()),
                            staged_filename: change.filename.clone(),
                            new_filename: format!("{}.disabled", change.filename),
                        })
                        .collect(),
                },
            )
            .unwrap();
            let mods = root.open_dir_nofollow("mods").unwrap();
            if interrupted_at >= 1 {
                mods.rename("root-old.jar.disabled", &old, "root-old.jar.disabled")
                    .unwrap();
            }
            if interrupted_at >= 2 {
                new.rename("root-new.jar", &mods, "root-new.jar.disabled")
                    .unwrap();
                mods.write(
                    "root-new.jar.disabled",
                    b"user edit after interrupted publication",
                )
                .unwrap();
            }
            if interrupted_at >= 3 {
                new.rename("dep.jar", &mods, "dep.jar.disabled").unwrap();
                let mut changed = plan.provenance.clone();
                for item in &plan.graph.mods {
                    changed.insert(
                        item.project_id.clone(),
                        ManagedMod {
                            version_id: item.version_id.clone(),
                            filename: item.file.filename.clone(),
                            sha1: item.file.sha1.clone(),
                        },
                    );
                }
                write_provenance(&root, &changed).unwrap();
            }
            assert!(recover_modrinth_update(&fixture.0).unwrap());
            assert!(!recover_modrinth_update(&fixture.0).unwrap());
            assert_eq!(mods.read("root-old.jar.disabled").unwrap(), b"old-one");
            assert!(mods.symlink_metadata("root-new.jar.disabled").is_err());
            assert!(mods.symlink_metadata("dep.jar.disabled").is_err());
            assert_eq!(read_provenance(&root).unwrap(), plan.provenance);
            if interrupted_at >= 2 {
                let recovery = backup.open_dir_nofollow("recovery").unwrap();
                let mut preserved = false;
                for entry in recovery.entries().unwrap() {
                    let entry = entry.unwrap();
                    if recovery.read(entry.file_name()).unwrap()
                        == b"user edit after interrupted publication"
                    {
                        preserved = true;
                    }
                }
                assert!(preserved);
            }
        }
    }

    #[test]
    fn cancelled_staging_leaves_originals_and_exact_committed_plan_retry_is_idempotent() {
        let fixture = Fixture::new();
        let plan = fixture_plan(&fixture, true);
        let root = fixture.root();
        let control = crate::OperationControl::new(Arc::new(|_| {}));
        let result = publish_updates_inner(
            &root,
            &plan,
            |change, staging| {
                stage(change, staging)?;
                control.cancel();
                Ok(())
            },
            |_| Ok(()),
            Some(&control),
        );
        assert!(result.unwrap_err().is::<crate::OperationCancelled>());
        assert_eq!(root.read("mods/root-old.jar.disabled").unwrap(), b"old-one");
        assert!(root.symlink_metadata(PENDING).is_err());
        publish_updates(&root, &plan, stage).unwrap();
        let count = root
            .open_dir_nofollow(".cobble-mod-backups")
            .unwrap()
            .entries()
            .unwrap()
            .count();
        let persisted: ModUpdatePlan =
            serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime
            .block_on(apply_modrinth_updates_controlled(
                &fixture.0,
                &persisted,
                crate::OperationControl::new(Arc::new(|_| {})),
            ))
            .unwrap();
        assert_eq!((result.updated, result.added), (1, 1));
        assert_eq!(
            root.open_dir_nofollow(".cobble-mod-backups")
                .unwrap()
                .entries()
                .unwrap()
                .count(),
            count
        );
        assert_eq!(plan.version_selections().len(), 1);
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

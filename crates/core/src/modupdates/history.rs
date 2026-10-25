// SPDX-License-Identifier: MIT
//! Retained Mod updates. Undo validates every touched file before changing any.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModOperationFile {
    pub name: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModOperationHistory {
    pub operation_id: String,
    pub created_at: u64,
    pub files: Vec<ModOperationFile>,
    pub status: String,
    provider: ModProvider,
    originals: BTreeMap<String, PresentMod>,
    outputs: BTreeMap<String, PresentMod>,
}

fn save(backup: &Dir, record: &ModOperationHistory) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = backup.open_with("history.json.tmp", &options)?;
    serde_json::to_writer_pretty(&mut file, record)?;
    file.sync_all()?;
    drop(file);
    backup.rename("history.json.tmp", backup, "history.json")?;
    Ok(())
}

fn load(backup: &Dir) -> Result<ModOperationHistory, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    Ok(serde_json::from_reader(
        backup
            .open_with("history.json", &options)?
            .take(1024 * 1024),
    )?)
}

pub(super) fn record_update(backup: &Dir, id: &str, plan: &ModUpdatePlan) -> Result<(), Error> {
    let outputs = plan
        .changes
        .iter()
        .map(|change| {
            let selected = graph_item(plan, &change.project_id)?;
            Ok((
                change.project_id.clone(),
                PresentMod {
                    recorded: ManagedMod {
                        version_id: selected.version_id.clone(),
                        filename: change.filename.clone(),
                        sha1: selected.file.sha1.clone(),
                    },
                    actual_filename: if change.enabled {
                        change.filename.clone()
                    } else {
                        format!("{}.disabled", change.filename)
                    },
                    enabled: change.enabled,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    save(
        backup,
        &ModOperationHistory {
            operation_id: id.into(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            files: plan
                .changes
                .iter()
                .map(|change| ModOperationFile {
                    name: change.name.clone(),
                    before: plan
                        .originals
                        .get(&change.project_id)
                        .map(|old| old.actual_filename.clone()),
                    after: outputs
                        .get(&change.project_id)
                        .map(|new| new.actual_filename.clone()),
                    enabled: change.enabled,
                })
                .collect(),
            status: "applied".into(),
            provider: plan.provider,
            originals: plan
                .originals
                .iter()
                .filter(|(project, _)| {
                    plan.changes
                        .iter()
                        .any(|change| change.project_id == **project)
                })
                .map(|(project, original)| (project.clone(), original.clone()))
                .collect(),
            outputs,
        },
    )
}

fn valid_id(id: &str) -> Result<(), Error> {
    let path = SafeRelativePath::parse(id)?;
    if path.as_str().contains('/') || id.starts_with('.') {
        return Err("Invalid Mod operation ID".into());
    }
    Ok(())
}

pub(crate) fn begin_install(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
) -> Result<String, Error> {
    let digest = format!("{:x}", Sha1::digest(serde_json::to_vec(plan)?));
    let id = format!(
        "install-{}-{digest}",
        match provider {
            ModProvider::Modrinth => "mr",
            ModProvider::CurseForge => "cf",
        }
    );
    root.create_dir_all(".cobble-mod-backups")?;
    let backups = root.open_dir_nofollow(".cobble-mod-backups")?;
    if backups.symlink_metadata(&id).is_ok() {
        let previous = load(&backups.open_dir_nofollow(&id)?)?;
        if previous.status == "installing" {
            return Ok(id);
        }
        // Same selection installed again after an undo is a distinct operation.
        return begin_install_with_id(
            root,
            plan,
            provider,
            &format!(
                "{id}-{}",
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
            ),
        );
    }
    begin_install_with_id(root, plan, provider, &id)
}

fn begin_install_with_id(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
    id: &str,
) -> Result<String, Error> {
    let backups = root.open_dir_nofollow(".cobble-mod-backups")?;
    backups.create_dir(id)?;
    let backup = backups.open_dir_nofollow(id)?;
    backup.create_dir("old")?;
    let mods = root.open_dir_nofollow("mods")?;
    let mut outputs = BTreeMap::new();
    let mut files = Vec::new();
    for selected in &plan.mods {
        if crate::modrinth::existing_matches(&mods, selected)? {
            continue;
        }
        outputs.insert(
            selected.project_id.clone(),
            PresentMod {
                recorded: ManagedMod {
                    version_id: selected.version_id.clone(),
                    filename: selected.file.filename.clone(),
                    sha1: selected.file.sha1.clone(),
                },
                actual_filename: selected.file.filename.clone(),
                enabled: true,
            },
        );
        files.push(ModOperationFile {
            name: selected.name.clone(),
            before: None,
            after: Some(selected.file.filename.clone()),
            enabled: true,
        });
    }
    save(
        &backup,
        &ModOperationHistory {
            operation_id: id.into(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            files,
            status: "installing".into(),
            provider,
            originals: BTreeMap::new(),
            outputs,
        },
    )?;
    Ok(id.into())
}

pub(crate) fn finish_install(root: &Dir, id: &str) -> Result<(), Error> {
    let backup = root
        .open_dir_nofollow(".cobble-mod-backups")?
        .open_dir_nofollow(id)?;
    let mut record = load(&backup)?;
    record.status = "applied".into();
    save(&backup, &record)
}

pub(super) fn mark_recovered(backup: &Dir) -> Result<(), Error> {
    if regular_present(backup, "history.json")? {
        let mut record = load(backup)?;
        record.status = "undone".into();
        save(backup, &record)?;
    }
    Ok(())
}

pub fn list_mod_operation_history(directory: &Path) -> Result<Vec<ModOperationHistory>, Error> {
    let root = open_root(directory)?;
    let backups = match root.open_dir_nofollow(".cobble-mod-backups") {
        Ok(backups) => backups,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut records = Vec::new();
    for entry in backups.entries()? {
        let id = entry?.file_name().to_string_lossy().into_owned();
        valid_id(&id)?;
        let backup = backups.open_dir_nofollow(&id)?;
        if !regular_present(&backup, "history.json")? {
            continue;
        }
        let record = load(&backup)?;
        if record.operation_id != id {
            return Err("Mod history ID differs".into());
        }
        if !record.files.is_empty() {
            records.push(record);
        }
    }
    records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
    Ok(records)
}

/// Caller must hold the stopped-instance write lease. Local changes are conflicts.
/// Original backups remain retained, including if an interrupted undo needs recovery.
pub fn undo_mod_operation(directory: &Path, operation_id: &str) -> Result<(), Error> {
    undo_inner_mode(directory, operation_id, false, |_| Ok(()))
}

pub fn preview_mod_operation_undo(directory: &Path, operation_id: &str) -> Result<(), Error> {
    undo_inner_mode(directory, operation_id, true, |_| Ok(()))
}

#[cfg(test)]
pub(super) fn undo_inner(
    directory: &Path,
    operation_id: &str,
    mut after_restore: impl FnMut(usize) -> Result<(), Error>,
) -> Result<(), Error> {
    undo_inner_mode(directory, operation_id, false, &mut after_restore)
}

fn undo_inner_mode(
    directory: &Path,
    operation_id: &str,
    preview: bool,
    mut after_restore: impl FnMut(usize) -> Result<(), Error>,
) -> Result<(), Error> {
    crate::modrinth::ensure_mod_install_complete(directory)?;
    valid_id(operation_id)?;
    let root = open_root(directory)?;
    let backup = root
        .open_dir_nofollow(".cobble-mod-backups")?
        .open_dir_nofollow(operation_id)?;
    let mut record = load(&backup)?;
    if record.operation_id != operation_id
        || !["applied", "undoing"].contains(&record.status.as_str())
    {
        return Err("Mod operation cannot be undone in its current state".into());
    }
    if record.outputs.len() > 256 || record.originals.len() > 256 {
        return Err("Mod history is too large".into());
    }
    for item in record.outputs.values().chain(record.originals.values()) {
        basename(&item.recorded.filename)?;
        let expected = if item.enabled {
            item.recorded.filename.clone()
        } else {
            format!("{}.disabled", item.recorded.filename)
        };
        if item.actual_filename != expected {
            return Err("Invalid Mod history filename".into());
        }
    }
    if record.status == "undoing" {
        return resume_undo(&root, &backup, &mut record, preview);
    }
    ensure_update_complete(directory)?;
    let mods = root.open_dir_nofollow("mods")?;
    let old = backup.open_dir_nofollow("old")?;
    let mut provenance = read_provider_provenance(&root, record.provider)?;
    let before_provenance = provenance.clone();
    let present = present_mods(&root, &provenance)?;
    for (project, output) in &record.outputs {
        if present.get(project) != Some(output) {
            return Err(format!(
                "Mod changed after this operation: {}",
                output.actual_filename
            )
            .into());
        }
        verify_old(&mods, output)?;
    }
    for original in record.originals.values() {
        basename(&original.recorded.filename)?;
        verify_old(&old, original)?;
        for name in [
            &original.recorded.filename,
            &format!("{}.disabled", original.recorded.filename),
        ] {
            for entry in mods.entries()? {
                let local = entry?.file_name().to_string_lossy().into_owned();
                if local.eq_ignore_ascii_case(name)
                    && !record
                        .outputs
                        .values()
                        .any(|item| item.actual_filename == local)
                {
                    return Err(format!("Undo conflicts with a local Mod: {local}").into());
                }
            }
        }
    }
    if preview {
        return Ok(());
    }
    backup.create_dir_all("undo-current")?;
    let retained = backup.open_dir_nofollow("undo-current")?;
    prepare_originals(&backup, &old, &record)?;
    let prepared = backup.open_dir_nofollow("undo-originals")?;
    record.status = "undoing".into();
    save(&backup, &record)?;
    let mut moved = Vec::new();
    let mut restored = Vec::new();
    let result = (|| -> Result<(), Error> {
        for (project, output) in &record.outputs {
            mods.rename(&output.actual_filename, &retained, &output.actual_filename)?;
            moved.push(output.actual_filename.clone());
            provenance.remove(project);
        }
        for (index, (project, original)) in record.originals.iter().enumerate() {
            if regular_present(&mods, &original.actual_filename)? {
                return Err("Local Mod appeared during undo".into());
            }
            prepared.rename(&original.actual_filename, &mods, &original.actual_filename)?;
            restored.push(original.actual_filename.clone());
            provenance.insert(project.clone(), original.recorded.clone());
            after_restore(index)?;
        }
        write_provider_provenance(&root, &provenance, record.provider)?;
        record.status = "undone".into();
        save(&backup, &record)
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<(), Error> {
            for name in restored.iter().rev() {
                mods.remove_file(name)?;
            }
            for name in moved.iter().rev() {
                if regular_present(&mods, name)? {
                    return Err("Local file appeared during undo rollback".into());
                }
                retained.rename(name, &mods, name)?;
            }
            write_provider_provenance(&root, &before_provenance, record.provider)?;
            record.status = "applied".into();
            save(&backup, &record)
        })();
        return match rollback {
            Ok(()) => Err(format!("Undo failed; original operation remains applied: {error}").into()),
            Err(rollback) => Err(format!("Undo incomplete: {error}; rollback incomplete: {rollback}. Retained files: .cobble-mod-backups/{operation_id}").into()),
        };
    }
    Ok(())
}

fn prepare_originals(backup: &Dir, old: &Dir, record: &ModOperationHistory) -> Result<(), Error> {
    backup.create_dir_all("undo-originals")?;
    let prepared = backup.open_dir_nofollow("undo-originals")?;
    for original in record.originals.values() {
        verify_old(old, original)?;
        if regular_present(&prepared, &original.actual_filename)? {
            prepared.remove_file(&original.actual_filename)?;
        }
        let mut read = OpenOptions::new();
        read.read(true).follow(FollowSymlinks::No);
        let mut write = OpenOptions::new();
        write
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut target = prepared.open_with(&original.actual_filename, &write)?;
        io::copy(
            &mut old.open_with(&original.actual_filename, &read)?,
            &mut target,
        )?;
        target.sync_all()?;
    }
    Ok(())
}

fn resume_undo(
    root: &Dir,
    backup: &Dir,
    record: &mut ModOperationHistory,
    preview: bool,
) -> Result<(), Error> {
    let mods = root.open_dir_nofollow("mods")?;
    let retained = backup.open_dir_nofollow("undo-current")?;
    let old = backup.open_dir_nofollow("old")?;
    let current_provenance = read_provider_provenance(root, record.provider)?;
    for (project, output) in &record.outputs {
        if let Some(current) = current_provenance.get(project)
            && current != &output.recorded
            && record
                .originals
                .get(project)
                .is_none_or(|original| current != &original.recorded)
        {
            return Err(format!("Mod identity changed after interrupted undo: {project}").into());
        }
    }
    // Check every surviving current file and retained output before resuming.
    for output in record.outputs.values() {
        let in_backup = regular_present(&retained, &output.actual_filename)?;
        if in_backup {
            verify_old(&retained, output)?;
        }
        if regular_present(&mods, &output.actual_filename)? {
            if in_backup {
                let original = record
                    .originals
                    .values()
                    .find(|item| item.actual_filename == output.actual_filename)
                    .ok_or("Undo conflicts with a later local Mod")?;
                verify_old(&mods, original)?;
            } else {
                verify_old(&mods, output)?;
            }
        } else if !in_backup {
            return Err("Undo output is missing from both game and retained files".into());
        }
    }
    for original in record.originals.values() {
        verify_old(&old, original)?;
        for name in [
            &original.recorded.filename,
            &format!("{}.disabled", original.recorded.filename),
        ] {
            if regular_present(&mods, name)?
                && !record
                    .outputs
                    .values()
                    .any(|item| item.actual_filename == *name)
            {
                if *name != original.actual_filename {
                    return Err(format!("Undo conflicts with a later local Mod: {name}").into());
                }
                verify_old(&mods, original)?;
            }
        }
    }
    if preview {
        return Ok(());
    }
    prepare_originals(backup, &old, record)?;
    let prepared = backup.open_dir_nofollow("undo-originals")?;
    for output in record.outputs.values() {
        if !regular_present(&retained, &output.actual_filename)? {
            mods.rename(&output.actual_filename, &retained, &output.actual_filename)?;
        }
    }
    let mut provenance = read_provider_provenance(root, record.provider)?;
    for project in record.outputs.keys() {
        provenance.remove(project);
    }
    for (project, original) in &record.originals {
        if !regular_present(&mods, &original.actual_filename)? {
            prepared.rename(&original.actual_filename, &mods, &original.actual_filename)?;
        }
        provenance.insert(project.clone(), original.recorded.clone());
    }
    write_provider_provenance(root, &provenance, record.provider)?;
    record.status = "undone".into();
    save(backup, record)
}

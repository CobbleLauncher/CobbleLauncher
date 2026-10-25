// SPDX-License-Identifier: MIT
//! Exact, source-qualified environment installation. The host owns the game lease.
use crate::modrinth::{ModDownloads, ModProvider, validate_provider_plan};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::EnvironmentPlan;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
mod shared_config;
pub use shared_config::{SharedConfig, install_shared_configs_controlled, validate_shared_configs};
const MARKER: &str = ".cobble-environment-install.json";
const LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    schema_version: u32,
    complete: bool,
    plan: EnvironmentPlan,
}

pub fn validate_environment_plan(plan: &EnvironmentPlan) -> Result<(), Error> {
    if plan.roots.is_empty()
        || plan.roots.len() > 128
        || !cobble_content::query_compatibility(
            &plan.game_version,
            Some(&plan.loader),
            None,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .allowed()
    {
        return Err("ENVIRONMENT_TARGET_INVALID".into());
    }
    let mut names = BTreeSet::new();
    let mut digests = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut count = 0;
    for (source, graph, provider) in [
        ("modrinth", plan.modrinth.as_ref(), ModProvider::Modrinth),
        (
            "curseforge",
            plan.curseforge.as_ref().map(|p| &p.graph),
            ModProvider::CurseForge,
        ),
    ] {
        if let Some(graph) = graph {
            validate_provider_plan(graph, provider)?;
            for item in &graph.mods {
                count += 1;
                if !names.insert(item.file.filename.to_ascii_lowercase())
                    || !digests.insert(item.file.sha1.to_ascii_lowercase())
                    || !identities.insert((
                        source,
                        item.project_id.as_str(),
                        item.version_id.as_str(),
                    ))
                {
                    return Err("ENVIRONMENT_CONTENT_COLLISION".into());
                }
            }
        }
    }
    if count == 0 || count > 256 {
        return Err("ENVIRONMENT_PLAN_LIMIT".into());
    }
    for root in &plan.roots {
        let source = match root.source {
            cobble_content::EnvironmentSource::Modrinth => "modrinth",
            cobble_content::EnvironmentSource::Curseforge => "curseforge",
        };
        if !identities.contains(&(source, root.project_id.as_str(), root.version_id.as_str())) {
            return Err("ENVIRONMENT_ROOT_MISSING".into());
        }
    }
    if let Some(cf) = &plan.curseforge {
        let mut restricted = BTreeSet::new();
        for item in &cf.restricted {
            if !restricted.insert((&item.project_id, &item.file_id))
                || !cf.graph.mods.iter().any(|m| {
                    m.project_id == item.project_id
                        && m.version_id == item.file_id
                        && m.file.filename == item.file_name
                        && m.file.size == item.size_bytes
                })
            {
                return Err("ENVIRONMENT_RESTRICTED_IDENTITY_INVALID".into());
            }
        }
        if cf
            .graph
            .mods
            .iter()
            .any(|m| cf.incompatible.contains(&m.project_id))
        {
            return Err("ENVIRONMENT_INCOMPATIBLE".into());
        }
    }
    if serde_json::to_vec(plan)?.len() as u64 > LIMIT {
        return Err("ENVIRONMENT_PLAN_LIMIT".into());
    }
    Ok(())
}

fn read(root: &Dir) -> Result<Option<Receipt>, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = match root.open_with(MARKER, &options) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !file.metadata()?.is_file() || file.metadata()?.len() > LIMIT {
        return Err("ENVIRONMENT_RECEIPT_INVALID".into());
    }
    let receipt: Receipt = serde_json::from_reader(file.take(LIMIT + 1))?;
    if receipt.schema_version != 1 {
        return Err("ENVIRONMENT_RECEIPT_INVALID".into());
    }
    validate_environment_plan(&receipt.plan)?;
    Ok(Some(receipt))
}

fn write(root: &Dir, receipt: &Receipt) -> Result<(), Error> {
    let encoded = serde_json::to_vec(receipt)?;
    if encoded.len() as u64 >= LIMIT {
        return Err("ENVIRONMENT_PLAN_LIMIT".into());
    }
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let temporary = format!("{MARKER}.tmp");
    let mut file = root.open_with(&temporary, &options)?;
    file.write_all(&encoded)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(&temporary, root, MARKER)?;
    Ok(())
}

pub fn read_environment_plan(directory: &Path) -> Result<Option<EnvironmentPlan>, Error> {
    Ok(read(&Dir::open_ambient_dir(
        directory,
        cap_std::ambient_authority(),
    )?)?
    .map(|r| r.plan))
}

pub(crate) fn ensure_environment_complete(directory: &Path) -> Result<(), Error> {
    shared_config::ensure_complete(directory)?;
    crate::personal_options::ensure_complete(directory)?;
    if read(&Dir::open_ambient_dir(
        directory,
        cap_std::ambient_authority(),
    )?)?
    .is_some_and(|r| !r.complete)
    {
        return Err("ENVIRONMENT_INSTALL_INCOMPLETE".into());
    }
    Ok(())
}

pub fn missing_environment_manual_files(
    plan: &EnvironmentPlan,
) -> Result<Vec<cobble_content::curseforge::RestrictedFile>, Error> {
    validate_environment_plan(plan)?;
    let Some(cf) = &plan.curseforge else {
        return Ok(Vec::new());
    };
    let cache = ModDownloads::open()?;
    let mut missing = Vec::new();
    for item in &cf.restricted {
        let selected = cf
            .graph
            .mods
            .iter()
            .find(|m| m.project_id == item.project_id && m.version_id == item.file_id)
            .ok_or("ENVIRONMENT_RESTRICTED_IDENTITY_INVALID")?;
        if !cache.contains(&selected.file)? {
            missing.push(item.clone());
        }
    }
    Ok(missing)
}

/// Only a native file-picker handle may supply input; no path is accepted.
pub fn import_environment_manual_file(
    plan: &EnvironmentPlan,
    project: &str,
    version: &str,
    input: std::fs::File,
) -> Result<(), Error> {
    validate_environment_plan(plan)?;
    let cf = plan
        .curseforge
        .as_ref()
        .ok_or("ENVIRONMENT_MANUAL_FILE_INVALID")?;
    if !cf
        .restricted
        .iter()
        .any(|m| m.project_id == project && m.file_id == version)
    {
        return Err("ENVIRONMENT_MANUAL_FILE_INVALID".into());
    }
    let selected = cf
        .graph
        .mods
        .iter()
        .find(|m| m.project_id == project && m.version_id == version)
        .ok_or("ENVIRONMENT_MANUAL_FILE_INVALID")?;
    ModDownloads::open()?.ingest_manual(&selected.file, input)
}

pub async fn install_environment_plan_controlled(
    plan: &EnvironmentPlan,
    directory: &Path,
    curseforge_key: Option<String>,
    control: crate::OperationControl,
) -> Result<crate::ModrinthInstallOutcome, Error> {
    validate_environment_plan(plan)?;
    control.check_cancelled()?;
    if !missing_environment_manual_files(plan)?.is_empty() {
        return Err("ENVIRONMENT_MANUAL_FILES_REQUIRED".into());
    }
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    if let Some(previous) = read(&root)?
        && serde_json::to_value(previous.plan)? != serde_json::to_value(plan)?
    {
        return Err("ENVIRONMENT_PLAN_CHANGED".into());
    }
    write(
        &root,
        &Receipt {
            schema_version: 1,
            complete: false,
            plan: plan.clone(),
        },
    )?;
    let mut installed = 0;
    let mut reused = 0;
    if let Some(mr) = &plan.modrinth {
        let result =
            crate::install_modrinth_plan_controlled(mr, directory, control.clone()).await?;
        installed += result.installed;
        reused += result.reused;
    }
    if let Some(cf) = &plan.curseforge {
        let result = crate::install_curseforge_plan_controlled(
            cf,
            directory,
            curseforge_key.ok_or("ENVIRONMENT_CURSEFORGE_UNAVAILABLE")?,
            control.clone(),
        )
        .await?;
        installed += result.installed;
        reused += result.reused;
    }
    // A committed final source wins a late cancel, matching existing publication semantics.
    write(
        &root,
        &Receipt {
            schema_version: 1,
            complete: true,
            plan: plan.clone(),
        },
    )?;
    Ok(crate::ModrinthInstallOutcome { installed, reused })
}

#[cfg(test)]
#[path = "environment_install/tests.rs"]
mod tests;

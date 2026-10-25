// SPDX-License-Identifier: MIT
//! Policy-filtered official CurseForge roots. Required dependencies use the existing
//! exact-root resolver; ignore/channel preferences never remove its requirements.
use crate::{
    ModUpdateCandidate, ModUpdateChannel, ModUpdatePlan, ModUpdatePolicy,
    list_managed_curseforge_mods, prepare_curseforge_update, read_mod_update_policy,
};
use cobble_content::curseforge::{CurseForgeClient, DownloadAvailability, File, InstallPlan};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
type Error = Box<dyn std::error::Error + Send + Sync>;
const MAX_FILES: usize = 500;

fn identity(value: &str) -> Result<u32, Error> {
    value
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| "Invalid CurseForge identity".into())
}
fn date(value: &str) -> Result<(String, String), Error> {
    let value = value
        .strip_suffix('Z')
        .ok_or("Invalid CurseForge publication date")?;
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    if seconds.len() != 19
        || seconds.bytes().enumerate().any(|(i, b)| match i {
            4 | 7 => b != b'-',
            10 => b != b'T',
            13 | 16 => b != b':',
            _ => !b.is_ascii_digit(),
        })
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("Invalid CurseForge publication date".into());
    }
    Ok((seconds.into(), format!("{fraction:0<9}")))
}
fn select<'a>(
    policy: &ModUpdatePolicy,
    project: &str,
    current: &File,
    files: &'a [File],
) -> Result<Option<&'a File>, Error> {
    let current_date = date(&current.date_published)?;
    let ignored = policy.project("curseforge", project);
    let mut candidates = Vec::new();
    for file in files {
        if file.project_id != project
            || file.file_id == current.file_id
            || file.download_availability == DownloadAvailability::Unavailable
            || !matches!(file.release_type, "release" | "beta" | "alpha")
            || (policy.channel_for("curseforge", project) == ModUpdateChannel::Stable
                && file.release_type != "release")
            || ignored.is_some_and(|p| p.ignored_versions.contains(&file.file_id))
        {
            continue;
        }
        let published = date(&file.date_published)?;
        if published > current_date {
            candidates.push((published, file));
        }
    }
    Ok(candidates
        .into_iter()
        .max_by(|(a, left), (b, right)| a.cmp(b).then(left.file_id.cmp(&right.file_id)))
        .map(|(_, file)| file))
}
struct FoundUpdate {
    current_name: String,
    file_id: String,
    name: String,
}
async fn find(
    client: &CurseForgeClient,
    policy: &ModUpdatePolicy,
    project: &str,
    installed: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<FoundUpdate>, Error> {
    let project_id = identity(project)?;
    let current = client
        .file_metadata(project_id, identity(installed)?)
        .await?;
    let mut files = Vec::new();
    let mut offset = 0;
    loop {
        let page = client
            .files(project_id, Some(mc), Some(loader), offset)
            .await?;
        let count = page.files.len();
        files.extend(page.files);
        let next = offset.saturating_add(count as u32);
        if u64::from(next) >= page.total_hits {
            break;
        }
        if count == 0 || files.len() >= MAX_FILES {
            return Err(
                "CurseForge update discovery exceeds 500 files; choose a version manually".into(),
            );
        }
        offset = next;
    }
    Ok(
        select(policy, project, &current, &files)?.map(|next| FoundUpdate {
            current_name: current.name.clone(),
            file_id: next.file_id.clone(),
            name: next.name.clone(),
        }),
    )
}
pub async fn check_curseforge_updates(
    directory: &Path,
    client: &CurseForgeClient,
    mc: &str,
    loader: &str,
) -> Result<Vec<ModUpdateCandidate>, Error> {
    let managed = list_managed_curseforge_mods(directory)?;
    if managed.len() > 128 {
        return Err("Check at most 128 managed CurseForge Mods".into());
    }
    let policy = read_mod_update_policy(directory)?;
    let mut updates = Vec::new();
    for current in managed.into_iter().filter(|item| !item.pinned) {
        if let Some(selected) = find(
            client,
            &policy,
            &current.project_id,
            &current.version_id,
            mc,
            loader,
        )
        .await?
        {
            let project = client.project(identity(&current.project_id)?).await?;
            updates.push(ModUpdateCandidate {
                project_id: current.project_id,
                name: project.title,
                current_version: selected.current_name,
                version_number: selected.name,
                version_id: selected.file_id,
                filename: current.filename,
                enabled: current.enabled,
            });
        }
    }
    Ok(updates)
}
pub async fn prepare_curseforge_updates(
    directory: &Path,
    client: &CurseForgeClient,
    mc: &str,
    loader: &str,
    projects: &[String],
) -> Result<(ModUpdatePlan, InstallPlan), Error> {
    if projects.is_empty() || projects.len() > 128 {
        return Err("Select between 1 and 128 managed CurseForge Mods".into());
    }
    if projects.iter().collect::<BTreeSet<_>>().len() != projects.len() {
        return Err("Duplicate selected CurseForge Mod".into());
    }
    let policy = read_mod_update_policy(directory)?;
    let managed = list_managed_curseforge_mods(directory)?;
    let mut roots = Vec::new();
    let mut labels = BTreeMap::new();
    for project in projects {
        let current = managed
            .iter()
            .find(|item| item.project_id == *project)
            .ok_or("Only present managed CurseForge Mods can be updated")?;
        if current.pinned {
            return Err("Unpin the selected Mod before changing its version".into());
        }
        let selected = find(client, &policy, project, &current.version_id, mc, loader)
            .await?
            .ok_or("Selected Mod has no allowed newer CurseForge version; check updates again")?;
        roots.push((identity(project)?, identity(&selected.file_id)?));
        labels.insert(project.clone(), selected.current_name);
    }
    let downloads = client.resolve_pinned_files(&roots, mc, loader).await?;
    let mut plan =
        prepare_curseforge_update(directory, &downloads)?.automatic_with_policy(policy)?;
    for change in &mut plan.changes {
        if let Some(label) = labels.get(&change.project_id) {
            change.from_version = Some(label.clone());
        }
    }
    Ok((plan, downloads))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModProjectPolicy;
    fn file(id: &str, kind: &'static str, published: &str) -> File {
        File {
            file_id: id.into(),
            project_id: "42".into(),
            name: "Display name is not SemVer".into(),
            file_name: format!("{id}.jar"),
            release_type: kind,
            date_published: published.into(),
            size_bytes: 1,
            game_versions: vec!["1.21.1".into()],
            dependencies: Vec::new(),
            download_availability: DownloadAvailability::Available,
        }
    }
    #[test]
    fn cf_policy_stable_previews_ignore_and_restore_use_provider_metadata() {
        let current = file("1", "release", "2026-01-01T00:00:00Z");
        let files = [
            file("2", "release", "2026-02-01T00:00:00Z"),
            file("3", "beta", "2026-03-01T00:00:00Z"),
            file("4", "alpha", "2026-04-01T00:00:00Z"),
            file("5", "unknown", "2026-05-01T00:00:00Z"),
        ];
        let mut policy = ModUpdatePolicy::default();
        assert_eq!(
            select(&policy, "42", &current, &files)
                .unwrap()
                .unwrap()
                .file_id,
            "2"
        );
        policy.projects.insert(
            "curseforge:42".into(),
            ModProjectPolicy {
                channel: Some(ModUpdateChannel::Preview),
                ignored_versions: BTreeSet::from(["4".into()]),
            },
        );
        assert_eq!(
            select(&policy, "42", &current, &files)
                .unwrap()
                .unwrap()
                .file_id,
            "3"
        );
        policy
            .projects
            .get_mut("curseforge:42")
            .unwrap()
            .ignored_versions
            .clear();
        assert_eq!(
            select(&policy, "42", &current, &files)
                .unwrap()
                .unwrap()
                .file_id,
            "4"
        );
        assert_eq!(
            policy.channel_for("modrinth", "42"),
            ModUpdateChannel::Stable
        );
    }
    #[test]
    fn cf_policy_keeps_manual_routes_and_compares_fractional_dates_without_guessing() {
        let current = file("1", "release", "2026-01-01T00:00:00.0Z");
        let mut manual = file("2", "release", "2026-01-01T00:00:00.1Z");
        manual.download_availability = DownloadAvailability::Manual;
        let mut blocked = file("3", "release", "2026-02-01T00:00:00Z");
        blocked.download_availability = DownloadAvailability::Unavailable;
        assert_eq!(
            select(
                &ModUpdatePolicy::default(),
                "42",
                &current,
                &[manual, blocked]
            )
            .unwrap()
            .unwrap()
            .file_id,
            "2"
        );
        assert!(date("invalid").is_err());
    }
}

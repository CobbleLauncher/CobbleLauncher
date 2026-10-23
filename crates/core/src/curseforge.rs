// SPDX-License-Identifier: MIT
//! Official CurseForge artifacts use the shared verified CAS and publication
//! journal, with distinct provenance. Caller owns a stopped-instance write lease.
use crate::modrinth::{
    ModDownloads, ModProvider, check_provider_pending_plan, commit_provider_mod_plan,
    existing_matches, preflight, read_provider_provenance, validate_provider_plan,
};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use cobble_content::curseforge::InstallPlan;
use cobble_store::SafeRelativePath;
use std::{io, path::Path};
type Error = Box<dyn std::error::Error + Send + Sync>;

pub fn missing_curseforge_manual_files(
    plan: &InstallPlan,
    key: String,
) -> Result<Vec<cobble_content::curseforge::RestrictedFile>, Error> {
    validate_provider_plan(&plan.graph, ModProvider::CurseForge)?;
    let downloads = ModDownloads::open_for(ModProvider::CurseForge, Some(key))?;
    plan.restricted
        .iter()
        .filter_map(|restricted| {
            let selected = plan.graph.mods.iter().find(|item| {
                item.project_id == restricted.project_id && item.version_id == restricted.file_id
            });
            match selected {
                Some(selected) => match downloads.contains(&selected.file) {
                    Ok(true) => None,
                    Ok(false) => Some(Ok(restricted.clone())),
                    Err(error) => Some(Err(error)),
                },
                None => Some(Err("invalid restricted file identity".into())),
            }
        })
        .collect()
}
pub fn import_curseforge_manual_file(
    plan: &InstallPlan,
    project: &str,
    id: &str,
    input: std::fs::File,
    key: String,
) -> Result<(), Error> {
    validate_provider_plan(&plan.graph, ModProvider::CurseForge)?;
    if !plan
        .restricted
        .iter()
        .any(|item| item.project_id == project && item.file_id == id)
    {
        return Err("manual file is not part of the current plan".into());
    }
    let selected = plan
        .graph
        .mods
        .iter()
        .find(|item| item.project_id == project && item.version_id == id)
        .ok_or("invalid manual file")?;
    ModDownloads::open_for(ModProvider::CurseForge, Some(key))?.ingest_manual(&selected.file, input)
}
pub async fn install_curseforge_plan_controlled(
    plan: &InstallPlan,
    directory: &Path,
    key: String,
    control: crate::OperationControl,
) -> Result<crate::ModrinthInstallOutcome, Error> {
    validate_provider_plan(&plan.graph, ModProvider::CurseForge)?;
    control.check_cancelled()?;
    let downloads = ModDownloads::open_for(ModProvider::CurseForge, Some(key))?;
    install_with_downloads(plan, directory, downloads, control).await
}
async fn install_with_downloads(
    plan: &InstallPlan,
    directory: &Path,
    downloads: ModDownloads,
    control: crate::OperationControl,
) -> Result<crate::ModrinthInstallOutcome, Error> {
    validate_provider_plan(&plan.graph, ModProvider::CurseForge)?;
    control.check_cancelled()?;
    check_curseforge_incompatibilities(directory, plan)?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    check_provider_pending_plan(&root, &plan.graph, ModProvider::CurseForge)?;
    let mut provenance = read_provider_provenance(&root, ModProvider::CurseForge)?;
    let mods = match root.open_dir_nofollow("mods") {
        Ok(mods) => Some(mods),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(mods) = &mods {
        preflight(&plan.graph, mods, &mut provenance)?;
    }
    let mut artifacts = Vec::new();
    let on_progress = control.progress();
    let mut reused = 0;
    on_progress(crate::InstallProgress {
        detail: None,
        phase: "mod-download".into(),
        completed: 0,
        total: plan.graph.mods.len(),
        reused,
    });
    for (index, selected) in plan.graph.mods.iter().enumerate() {
        control.check_cancelled()?;
        if mods
            .as_ref()
            .map(|mods| existing_matches(mods, selected))
            .transpose()?
            .unwrap_or(false)
        {
            reused += 1;
            artifacts.push(None);
        } else {
            artifacts.push(Some(
                downloads
                    .fetch_file_controlled(&selected.file, &control)
                    .await?,
            ));
        }
        on_progress(crate::InstallProgress {
            detail: None,
            phase: "mod-download".into(),
            completed: index + 1,
            total: plan.graph.mods.len(),
            reused,
        });
    }
    control.check_cancelled()?;
    let graph = plan.graph.clone();
    tokio::task::spawn_blocking(move || {
        commit_provider_mod_plan(
            &root,
            &graph,
            ModProvider::CurseForge,
            &control,
            |index, selected, mods| {
                let artifact = artifacts[index]
                    .as_ref()
                    .ok_or("an existing mod disappeared; retry")?;
                downloads.content.copy_to(
                    artifact,
                    mods,
                    &SafeRelativePath::parse(&selected.file.filename)?,
                )?;
                Ok(())
            },
        )
    })
    .await?
}
pub(super) fn check_curseforge_incompatibilities(
    directory: &Path,
    plan: &InstallPlan,
) -> Result<(), Error> {
    let present = crate::list_managed_curseforge_mods(directory)?;
    if present
        .iter()
        .any(|item| item.enabled && plan.incompatible.contains(&item.project_id))
    {
        return Err("请先停用不兼容的 CurseForge Mod".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::{Digest, Sha1};
    #[test]
    fn archived_exact_pin_stays_blocked_until_verified_local_bytes_are_supplied() {
        let directory = std::env::temp_dir().join(format!(
            "cobble-cf-archived-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let game = directory.join("game");
        std::fs::create_dir(&game).unwrap();
        let file = cobble_content::ModrinthFile {
            url: String::new(),
            filename: "archived.jar".into(),
            size: 7,
            sha1: format!("{:x}", Sha1::digest(b"trusted")),
            sha512: None,
        };
        let plan = InstallPlan {
            graph: cobble_content::ModrinthPlan {
                project_id: "11".into(),
                version_id: "22".into(),
                mods: vec![cobble_content::ResolvedMod {
                    project_id: "11".into(),
                    version_id: "22".into(),
                    name: "Archived fixture".into(),
                    version_number: "Pinned".into(),
                    file: file.clone(),
                    required_by: None,
                }],
            },
            restricted: vec![cobble_content::curseforge::RestrictedFile {
                project_id: "11".into(),
                file_id: "22".into(),
                name: "Archived fixture".into(),
                file_name: "archived.jar".into(),
                size_bytes: 7,
                website_url: None,
                reason: "unavailable".into(),
            }],
            incompatible: Vec::new(),
        };
        let open = || {
            ModDownloads::open_at(
                &directory.join("cache"),
                ModProvider::CurseForge,
                Some("synthetic-key".into()),
            )
            .unwrap()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let control = || crate::OperationControl::new(std::sync::Arc::new(|_| {}));
        assert!(
            runtime
                .block_on(install_with_downloads(&plan, &game, open(), control()))
                .is_err()
        );
        assert!(!game.join("mods/archived.jar").exists());
        let selected = directory.join("selected.jar");
        std::fs::write(&selected, b"trusted").unwrap();
        let downloads = open();
        downloads
            .ingest_manual(&file, std::fs::File::open(&selected).unwrap())
            .unwrap();
        let result = runtime
            .block_on(install_with_downloads(&plan, &game, downloads, control()))
            .unwrap();
        assert_eq!(result.installed, 1);
        assert_eq!(
            std::fs::read(game.join("mods/archived.jar")).unwrap(),
            b"trusted"
        );
        let managed = crate::list_managed_curseforge_mods(&game).unwrap();
        assert_eq!(
            (
                managed[0].project_id.as_str(),
                managed[0].version_id.as_str()
            ),
            ("11", "22")
        );
        assert!(!game.join(".cobble-modrinth.json").exists());
    }
    #[test]
    #[cfg(debug_assertions)]
    #[ignore = "Explicit runtime credential and official artifact download; isolated temporary instance, no game launch"]
    fn official_small_mod_install_and_update_query_in_isolated_instance() {
        use std::sync::Arc;
        let key =
            std::env::var("COURSE_FORGE_API").expect("explicit development credential required");
        let client = cobble_content::curseforge::CurseForgeClient::new(key.clone())
            .expect("explicit development credential required");
        let base = std::env::temp_dir();
        let directory = base.join(format!(
            "cobble-cf-live-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let cleanup = Cleanup(directory.clone());
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let projects = client
                    .search(
                        "FerriteCore",
                        cobble_content::curseforge::ContentKind::Mod,
                        Some("1.21.1"),
                        Some("fabric"),
                        0,
                    )
                    .await
                    .expect("official search failed");
                let project = projects
                    .hits
                    .iter()
                    .find(|project| {
                        project
                            .title
                            .to_ascii_lowercase()
                            .starts_with("ferritecore")
                    })
                    .expect("official representative project not found");
                let project_id = project.project_id.parse().unwrap();
                let files = client
                    .files(project_id, Some("1.21.1"), Some("fabric"), 0)
                    .await
                    .expect("official file listing failed");
                let selected = files
                    .files
                    .iter()
                    .find(|file| {
                        file.download_availability
                            == cobble_content::curseforge::DownloadAvailability::Available
                            && file.size_bytes < 8 * 1024 * 1024
                    })
                    .expect("no small distributable representative file");
                let file_id = selected.file_id.parse().unwrap();
                let plan = client
                    .resolve_install(project_id, file_id, "1.21.1", "fabric")
                    .await
                    .expect("official dependency resolution failed");
                assert!(
                    plan.restricted.is_empty(),
                    "representative plan requires manual selection"
                );
                assert!(
                    plan.graph
                        .mods
                        .iter()
                        .map(|item| item.file.size)
                        .sum::<u64>()
                        < 32 * 1024 * 1024,
                    "representative plan exceeds smoke budget"
                );
                let downloads = ModDownloads::open_at(
                    &directory.join("artifact-cache"),
                    ModProvider::CurseForge,
                    Some(key),
                )
                .expect("isolated artifact cache failed");
                for item in &plan.graph.mods {
                    assert!(
                        !downloads.contains(&item.file).unwrap(),
                        "disposable artifact cache must start empty"
                    );
                }
                let outcome = install_with_downloads(
                    &plan,
                    &directory,
                    downloads,
                    crate::OperationControl::new(Arc::new(|_| {})),
                )
                .await
                .expect("official artifact installation failed");
                assert_eq!(outcome.installed, plan.graph.mods.len());
                let installed = crate::list_managed_curseforge_mods(&directory).unwrap();
                assert_eq!(installed.len(), plan.graph.mods.len());
                for item in &plan.graph.mods {
                    assert!(directory.join("mods").join(&item.file.filename).is_file());
                }
                assert!(!directory.join(".cobble-modrinth.json").exists());
                let _update = client
                    .find_update(project_id, file_id, "1.21.1", "fabric")
                    .await
                    .expect("official update query failed");
            });
        assert!(directory.starts_with(&base));
        drop(cleanup);
        assert!(
            !directory.exists(),
            "owned disposable instance cleanup failed"
        );
    }

    #[test]
    fn manual_file_requires_exact_official_digest_and_size_before_cas_reuse() {
        let directory = std::env::temp_dir().join(format!(
            "cobble-cf-manual-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let file = cobble_content::ModrinthFile {
            url: String::new(),
            filename: "manual.jar".into(),
            size: 7,
            sha1: format!("{:x}", Sha1::digest(b"trusted")),
            sha512: None,
        };
        let downloads = ModDownloads::open_at(
            &directory.join("cache"),
            ModProvider::CurseForge,
            Some("synthetic-key".into()),
        )
        .unwrap();
        assert!(!downloads.contains(&file).unwrap());
        let input = directory.join("selected.jar");
        std::fs::write(&input, b"changed").unwrap();
        assert!(
            downloads
                .ingest_manual(&file, std::fs::File::open(&input).unwrap())
                .is_err()
        );
        assert!(!downloads.contains(&file).unwrap());
        std::fs::write(&input, b"trusted").unwrap();
        downloads
            .ingest_manual(&file, std::fs::File::open(&input).unwrap())
            .unwrap();
        assert!(downloads.contains(&file).unwrap());
        let mut wrong_size = file.clone();
        wrong_size.size += 1;
        assert!(
            downloads
                .ingest_manual(&wrong_size, std::fs::File::open(&input).unwrap())
                .is_err()
        );
    }
}

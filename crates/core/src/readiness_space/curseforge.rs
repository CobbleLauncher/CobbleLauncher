// SPDX-License-Identifier: MIT
use super::*;

/// Inspect the retained archive and exact reviewed plan before any payload write.
pub fn curseforge_pack_space_artifacts(
    archive: &Path,
    plan: &crate::CurseForgePackPlan,
    directory: &Path,
    cache_volume: &str,
    target_volume: &str,
    control: OperationControl,
) -> Result<Vec<SpaceArtifact>, Error> {
    let inspected = crate::curseforge_pack::inspect(archive, &control)?;
    if inspected.archive_sha256 != plan.archive_sha256 {
        return Err("Selected CurseForge archive changed since preview".into());
    }
    let paths = crate::validate_curseforge_pack_plan(&inspected.pack, plan)?;
    let cache = ModDownloads::open()?;
    artifacts(
        &inspected.pack,
        plan,
        paths,
        directory,
        cache_volume,
        target_volume,
        &cache,
        &control,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn artifacts(
    pack: &cobble_content::CurseForgePack,
    plan: &crate::CurseForgePackPlan,
    paths: Vec<String>,
    directory: &Path,
    cache_volume: &str,
    target_volume: &str,
    cache: &ModDownloads,
    control: &OperationControl,
) -> Result<Vec<SpaceArtifact>, Error> {
    let root = match Dir::open_ambient_dir(directory, cap_std::ambient_authority()) {
        Ok(dir) => Some(dir),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let mut result = Vec::new();
    for (selected, path) in plan.files.files.iter().zip(paths) {
        control.check_cancelled()?;
        let file = &selected.file;
        let installed = match &root {
            Some(root) => {
                file_matches(root, &path, Some(file.size), Some(&file.sha1), control)? == Some(true)
            }
            None => false,
        };
        result.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(file.size),
            // The pack collector fetches CAS even when the target already
            // matches. Keep CAS demand independent of publication reuse.
            installed_bytes: Some(0),
            verified_cached: cache.contains(file)?,
            already_installed: false,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        });
        result.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(0),
            installed_bytes: Some(file.size),
            verified_cached: true,
            already_installed: installed,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        });
    }
    for item in &pack.overrides {
        control.check_cancelled()?;
        cobble_store::SafeRelativePath::parse(&item.path)?;
        // ZIP entries stream through CAS and its temporary file before materialization.
        // Their uncompressed length is known; cached override digests aren't in the manifest.
        result.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(item.size),
            installed_bytes: Some(item.size),
            verified_cached: false,
            already_installed: false,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        });
    }
    Ok(result)
}

// SPDX-License-Identifier: MIT
use super::*;
use cobble_core::space::{SpaceEstimate, SpaceVolume};
// DownloadManager caps every install collector at sixteen unique jobs.
// Reserve that ceiling rather than assuming the default per-origin budget.
const TRANSFER_STAGING_BOUND: usize = 16;
#[path = "native_worker_space.rs"]
pub(super) mod workers;

fn existing_ancestor(path: &std::path::Path) -> Result<PathBuf, String> {
    let mut path = path.to_owned();
    while !path.exists() {
        if !path.pop() {
            return Err("SPACE_VOLUME_UNAVAILABLE".into());
        }
    }
    dunce::canonicalize(path).map_err(|_| "SPACE_VOLUME_UNAVAILABLE".into())
}
fn volume(path: &std::path::Path) -> Result<SpaceVolume, String> {
    let existing = existing_ancestor(path)?;
    let (id, label) =
        cobble_platform::disk_volume_identity(&existing).ok_or("SPACE_VOLUME_UNAVAILABLE")?;
    Ok(SpaceVolume {
        id,
        label,
        available_bytes: cobble_platform::available_disk_space(&existing),
    })
}

pub(super) fn creation_directory(app: &AppHandle, use_default: bool) -> Result<PathBuf, String> {
    if use_default {
        return default_directory();
    }
    app.state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "游戏库不可用")?
        .picked_directory
        .clone()
        .ok_or("请先选择保存位置".into())
}

pub(super) fn include_shared_configs(
    estimate: &mut SpaceEstimate,
    directory: &std::path::Path,
    configs: &[cobble_core::environment_install::SharedConfig],
) -> Result<(), String> {
    cobble_core::environment_install::validate_shared_configs(configs)
        .map_err(|e| e.to_string())?;
    if configs.is_empty() {
        return Ok(());
    }
    let bytes = configs
        .iter()
        .try_fold(0u64, |sum, c| sum.checked_add(c.text.len() as u64))
        .ok_or("SPACE_OVERFLOW")?;
    include_target_bytes(estimate, directory, bytes)
}

pub(super) fn include_personal_options(
    estimate: &mut SpaceEstimate,
    directory: &std::path::Path,
    options: Option<&cobble_core::personal_options::PersonalOptions>,
) -> Result<(), String> {
    if let Some(options) = options {
        include_target_bytes(
            estimate,
            directory,
            cobble_core::personal_options::encoded_len(options)?,
        )?;
    }
    Ok(())
}

fn include_target_bytes(
    estimate: &mut SpaceEstimate,
    directory: &std::path::Path,
    bytes: u64,
) -> Result<(), String> {
    let id = volume(directory)?.id;
    let target = estimate
        .volumes
        .iter_mut()
        .find(|v| v.volume.id == id)
        .ok_or("SPACE_VOLUME_CHANGED")?;
    target.materialized_bytes = target
        .materialized_bytes
        .checked_add(bytes)
        .ok_or("SPACE_OVERFLOW")?;
    target.additional_peak_bytes = target
        .additional_peak_bytes
        .checked_add(bytes)
        .ok_or("SPACE_OVERFLOW")?;
    if target
        .volume
        .available_bytes
        .is_some_and(|free| free < target.additional_peak_bytes)
    {
        target.verdict = cobble_core::space::SpaceVerdict::Insufficient;
    }
    estimate.can_proceed = estimate
        .volumes
        .iter()
        .all(|v| v.verdict != cobble_core::space::SpaceVerdict::Insufficient);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn estimate(
    version: &str,
    loader: Option<&str>,
    environment: Option<&cobble_content::EnvironmentPlan>,
    directory: &std::path::Path,
    source: DownloadSettings,
    java: &cobble_minecraft::JavaOverride,
    control: cobble_core::OperationControl,
) -> Result<SpaceEstimate, String> {
    let cache = dirs::cache_dir()
        .ok_or("SPACE_CACHE_UNAVAILABLE")?
        .join("CobbleLauncher");
    let runtime = cobble_minecraft::managed_java_root().map_err(|_| "SPACE_RUNTIME_UNAVAILABLE")?;
    let cache_volume = volume(&cache)?;
    let target_volume = volume(directory)?;
    let runtime_volume = volume(&runtime)?;
    let mut volumes = std::collections::BTreeMap::new();
    for v in [&cache_volume, &target_volume, &runtime_volume] {
        volumes.entry(v.id.clone()).or_insert(v.clone());
    }
    let artifacts = cobble_core::readiness_space::environment_space_artifacts(
        version,
        loader,
        environment,
        source.download_source().map_err(|e| e.to_string())?,
        directory,
        java,
        &cache_volume.id,
        &target_volume.id,
        &runtime_volume.id,
        control,
    )
    .await
    .map_err(|e| e.to_string())?;
    let _ = (artifacts.required_java, artifacts.java_available);
    let estimate = cobble_core::space::estimate_space(
        &volumes.into_values().collect::<Vec<_>>(),
        &artifacts.artifacts,
        &std::collections::BTreeMap::new(),
        TRANSFER_STAGING_BOUND,
    )
    .map_err(str::to_owned)?;
    for (index, entry) in estimate.volumes.iter().enumerate() {
        tracing::info!(event="space_preflight",volume_index=index,cache_growth_bytes=entry.download_bytes,
            transfer_staging_bytes=entry.download_staging_bytes,target_bytes=entry.materialized_bytes,
            extraction_scratch_bytes=entry.extraction_scratch_bytes,backup_copy_bytes=entry.backup_copy_bytes,
            required_bytes=entry.additional_peak_bytes,available_bytes=?entry.volume.available_bytes,
            unknown_sizes=entry.unknown_sizes,verdict=?entry.verdict);
    }
    Ok(estimate)
}

pub(super) async fn estimate_java(
    required: u32,
    control: cobble_core::OperationControl,
) -> Result<SpaceEstimate, String> {
    let cache = dirs::cache_dir()
        .ok_or("SPACE_CACHE_UNAVAILABLE")?
        .join("CobbleLauncher");
    let runtime = cobble_minecraft::managed_java_root().map_err(|_| "SPACE_RUNTIME_UNAVAILABLE")?;
    let cache_volume = volume(&cache)?;
    let runtime_volume = volume(&runtime)?;
    let artifacts = cobble_core::readiness_space::managed_java_space_artifacts(
        required,
        &cache_volume.id,
        &runtime_volume.id,
        control,
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut volumes = std::collections::BTreeMap::new();
    for v in [cache_volume, runtime_volume] {
        volumes.entry(v.id.clone()).or_insert(v);
    }
    let estimate = cobble_core::space::estimate_space(
        &volumes.into_values().collect::<Vec<_>>(),
        &artifacts,
        &std::collections::BTreeMap::new(),
        1,
    )
    .map_err(str::to_owned)?;
    for (index, entry) in estimate.volumes.iter().enumerate() {
        tracing::info!(event="java_space_preflight",java_major=required,volume_index=index,
        cache_growth_bytes=entry.download_bytes,transfer_staging_bytes=entry.download_staging_bytes,
        runtime_staging_bytes=entry.extraction_scratch_bytes,required_bytes=entry.additional_peak_bytes,
        available_bytes=?entry.volume.available_bytes,unknown_sizes=entry.unknown_sizes,verdict=?entry.verdict);
    }
    Ok(estimate)
}

#[derive(Default)]
pub(super) struct PackSpaceExtras {
    pub archive_copy: Option<(PathBuf, u64)>,
    pub migration_bytes: u64,
    pub retained_source: Option<(PathBuf, u64)>,
}
fn local_copy_artifact(volume: &str, bytes: u64) -> cobble_core::space::SpaceArtifact {
    cobble_core::space::SpaceArtifact {
        cache_volume: volume.into(),
        target_volume: volume.into(),
        compressed_bytes: Some(0),
        installed_bytes: Some(bytes),
        verified_cached: true,
        already_installed: false,
        extraction_scratch_bytes: Some(0),
        backup_copy_bytes: 0,
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn estimate_pack(
    pack: &cobble_content::Mrpack,
    optional: &[String],
    directory: &std::path::Path,
    source: DownloadSettings,
    java: &cobble_minecraft::JavaOverride,
    extras: PackSpaceExtras,
    control: cobble_core::OperationControl,
) -> Result<SpaceEstimate, String> {
    let cache = dirs::cache_dir()
        .ok_or("SPACE_CACHE_UNAVAILABLE")?
        .join("CobbleLauncher");
    let runtime = cobble_minecraft::managed_java_root().map_err(|_| "SPACE_RUNTIME_UNAVAILABLE")?;
    let cache_volume = volume(&cache)?;
    let target_volume = volume(directory)?;
    let runtime_volume = volume(&runtime)?;
    let mut volumes = std::collections::BTreeMap::new();
    for v in [&cache_volume, &target_volume, &runtime_volume] {
        volumes.entry(v.id.clone()).or_insert(v.clone());
    }
    let mut artifacts = cobble_core::readiness_space::environment_space_artifacts(
        &pack.minecraft_version,
        pack.loader_kind.as_deref(),
        None,
        source.download_source().map_err(|e| e.to_string())?,
        directory,
        java,
        &cache_volume.id,
        &target_volume.id,
        &runtime_volume.id,
        control.clone(),
    )
    .await
    .map_err(|e| e.to_string())?
    .artifacts;
    artifacts.extend(
        cobble_core::readiness_space::mrpack_space_artifacts(
            pack,
            optional,
            directory,
            &cache_volume.id,
            &target_volume.id,
            control,
        )
        .map_err(|e| e.to_string())?,
    );
    artifacts.push(local_copy_artifact(
        &target_volume.id,
        extras.migration_bytes,
    ));
    if let Some((path, bytes)) = extras.archive_copy {
        let archive_volume = volume(&path)?;
        artifacts.push(local_copy_artifact(&archive_volume.id, bytes));
        volumes
            .entry(archive_volume.id.clone())
            .or_insert(archive_volume);
    }
    let mut retained = std::collections::BTreeMap::new();
    if let Some((path, bytes)) = extras.retained_source {
        let source_volume = volume(&path)?;
        retained.insert(source_volume.id.clone(), bytes);
        volumes
            .entry(source_volume.id.clone())
            .or_insert(source_volume);
    }
    let estimate = cobble_core::space::estimate_space(
        &volumes.into_values().collect::<Vec<_>>(),
        &artifacts,
        &retained,
        TRANSFER_STAGING_BOUND,
    )
    .map_err(str::to_owned)?;
    for (index, entry) in estimate.volumes.iter().enumerate() {
        tracing::info!(event="pack_space_preflight",volume_index=index,
        cache_growth_bytes=entry.download_bytes,transfer_staging_bytes=entry.download_staging_bytes,target_bytes=entry.materialized_bytes,
        retained_existing_bytes=entry.retained_existing_bytes,required_bytes=entry.additional_peak_bytes,available_bytes=?entry.volume.available_bytes,
        unknown_sizes=entry.unknown_sizes,verdict=?entry.verdict);
    }
    Ok(estimate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_core::space::{SpaceVerdict, VolumeSpace};
    use sha2::Digest;
    #[test]
    fn shared_configuration_space_is_charged_to_actual_volume_before_any_file_is_written() {
        let target = tempfile::tempdir().unwrap();
        let mut disk = volume(target.path()).unwrap();
        disk.available_bytes = Some(1);
        let mut estimate = SpaceEstimate {
            volumes: vec![VolumeSpace {
                volume: disk,
                download_bytes: 0,
                download_staging_bytes: 0,
                materialized_bytes: 0,
                extraction_scratch_bytes: 0,
                backup_copy_bytes: 0,
                retained_existing_bytes: 0,
                additional_peak_bytes: 0,
                unknown_sizes: 0,
                verdict: SpaceVerdict::Sufficient,
            }],
            can_proceed: true,
            exact: true,
        };
        let configs = [cobble_core::environment_install::SharedConfig {
            path: "config/selected.json".into(),
            text: "{}".into(),
            sha256: format!("{:x}", sha2::Sha256::digest(b"{}")),
        }];
        include_shared_configs(&mut estimate, target.path(), &configs).unwrap();
        assert!(!estimate.can_proceed);
        assert_eq!(estimate.volumes[0].additional_peak_bytes, 2);
        assert_eq!(estimate.volumes[0].materialized_bytes, 2);
        assert_eq!(estimate.volumes[0].verdict, SpaceVerdict::Insufficient);
        assert!(!target.path().join("config").exists());
    }
    #[test]
    fn mrpack_archive_and_selected_local_world_bytes_reserve_only_their_destination_volumes() {
        let volumes =
            [("archive", 50), ("target", 99), ("source", 0)].map(|(id, free)| SpaceVolume {
                id: id.into(),
                label: id.into(),
                available_bytes: Some(free),
            });
        let artifacts = [
            local_copy_artifact("archive", 50),
            local_copy_artifact("target", 100),
        ];
        let retained = std::collections::BTreeMap::from([("source".into(), 900)]);
        let estimate =
            cobble_core::space::estimate_space(&volumes, &artifacts, &retained, 4).unwrap();
        assert!(!estimate.can_proceed);
        let source = estimate
            .volumes
            .iter()
            .find(|v| v.volume.id == "source")
            .unwrap();
        assert_eq!(source.additional_peak_bytes, 0);
        assert_eq!(source.retained_existing_bytes, 900);
        assert!(
            estimate
                .volumes
                .iter()
                .all(|v| v.download_bytes == 0 && v.download_staging_bytes == 0)
        );
    }
}

// SPDX-License-Identifier: MIT
use super::*;
use cobble_core::space::{SpaceVerdict, estimate_space};

async fn base(
    game: &Game,
    control: cobble_core::OperationControl,
) -> Result<SpaceEstimate, String> {
    let source = game
        .download_source
        .clone()
        .map(Ok)
        .unwrap_or_else(cobble_minecraft::read_download_settings)
        .map_err(|e| e.to_string())?;
    let java = cobble_core::capture_java_choice(&game.runtime_overrides.java)
        .map_err(|e| e.to_string())?;
    estimate(
        &game.version,
        game.loader_kind.as_deref(),
        None,
        &game.directory,
        source,
        &java,
        control,
    )
    .await
}

// Adding separately measured phases conservatively retains both sets of
// staging buffers. It never reduces the known additional peak of either plan.
fn combine(mut base: SpaceEstimate, extra: SpaceEstimate) -> Result<SpaceEstimate, String> {
    base.exact &= extra.exact;
    for item in extra.volumes {
        if let Some(existing) = base
            .volumes
            .iter_mut()
            .find(|v| v.volume.id == item.volume.id)
        {
            macro_rules! add {
                ($field:ident) => {
                    existing.$field = existing
                        .$field
                        .checked_add(item.$field)
                        .ok_or("SPACE_OVERFLOW")?;
                };
            }
            add!(download_bytes);
            add!(download_staging_bytes);
            add!(materialized_bytes);
            add!(extraction_scratch_bytes);
            add!(backup_copy_bytes);
            add!(retained_existing_bytes);
            add!(additional_peak_bytes);
            add!(unknown_sizes);
            existing.verdict = if existing
                .volume
                .available_bytes
                .is_some_and(|v| v < existing.additional_peak_bytes)
            {
                SpaceVerdict::Insufficient
            } else if existing.unknown_sizes > 0 || existing.volume.available_bytes.is_none() {
                SpaceVerdict::Unknown
            } else {
                SpaceVerdict::Sufficient
            };
        } else {
            base.volumes.push(item);
        }
    }
    base.can_proceed = base
        .volumes
        .iter()
        .all(|v| v.verdict != SpaceVerdict::Insufficient);
    for (index, entry) in base.volumes.iter().enumerate() {
        tracing::info!(event="worker_space_preflight",volume_index=index,
            cache_growth_bytes=entry.download_bytes,transfer_staging_bytes=entry.download_staging_bytes,
            target_bytes=entry.materialized_bytes,extraction_scratch_bytes=entry.extraction_scratch_bytes,
            retained_existing_bytes=entry.retained_existing_bytes,required_bytes=entry.additional_peak_bytes,
            available_bytes=?entry.volume.available_bytes,unknown_sizes=entry.unknown_sizes,verdict=?entry.verdict);
    }
    if !base.can_proceed {
        return Err("SPACE_INSUFFICIENT".into());
    }
    Ok(base)
}

pub(crate) async fn curseforge_pack(
    game: &Game,
    archive: PathBuf,
    plan: cobble_core::CurseForgePackPlan,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let cache = volume(
        &dirs::cache_dir()
            .ok_or("SPACE_CACHE_UNAVAILABLE")?
            .join("CobbleLauncher"),
    )?;
    let target = volume(&game.directory)?;
    let mut volumes = std::collections::BTreeMap::new();
    for entry in [&cache, &target] {
        volumes.entry(entry.id.clone()).or_insert(entry.clone());
    }
    let directory = game.directory.clone();
    let cache_id = cache.id;
    let target_id = target.id;
    let measuring = control.clone();
    let artifacts = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::readiness_space::curseforge_pack_space_artifacts(
            &archive, &plan, &directory, &cache_id, &target_id, measuring,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "SPACE_MEASUREMENT_FAILED")??;
    let extra = estimate_space(
        &volumes.into_values().collect::<Vec<_>>(),
        &artifacts,
        &std::collections::BTreeMap::new(),
        TRANSFER_STAGING_BOUND,
    )
    .map_err(str::to_owned)?;
    combine(base(game, control).await?, extra)?;
    Ok(())
}

pub(crate) async fn launcher_import(
    game: &Game,
    candidate: cobble_core::LauncherImportCandidate,
    content_id: String,
    include_saves: bool,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let directory = game.directory.clone();
    let measuring = control.clone();
    let (source, bytes) = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::launcher_import_space_demand(
            &candidate,
            &content_id,
            &directory,
            include_saves,
            &measuring,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "SPACE_MEASUREMENT_FAILED")??;
    let target = volume(&game.directory)?;
    let source = volume(&source)?;
    let retained = std::collections::BTreeMap::from([(source.id.clone(), bytes)]);
    let artifacts = [local_copy_artifact(&target.id, bytes)];
    let mut volumes = std::collections::BTreeMap::new();
    for entry in [target, source] {
        volumes.entry(entry.id.clone()).or_insert(entry);
    }
    let extra = estimate_space(
        &volumes.into_values().collect::<Vec<_>>(),
        &artifacts,
        &retained,
        1,
    )
    .map_err(str::to_owned)?;
    combine(base(game, control).await?, extra)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_staging_covers_multiple_origins_above_the_default_per_origin_limit() {
        let volume = SpaceVolume {
            id: "cache".into(),
            label: "cache".into(),
            available_bytes: Some(159),
        };
        let artifacts = (0..8)
            .map(|_| cobble_core::space::SpaceArtifact {
                cache_volume: "cache".into(),
                target_volume: "cache".into(),
                compressed_bytes: Some(10),
                installed_bytes: Some(0),
                verified_cached: false,
                already_installed: false,
                extraction_scratch_bytes: Some(0),
                backup_copy_bytes: 0,
            })
            .collect::<Vec<_>>();
        let plan = estimate_space(
            &[volume],
            &artifacts,
            &Default::default(),
            TRANSFER_STAGING_BOUND,
        )
        .unwrap();
        assert_eq!(plan.volumes[0].download_staging_bytes, 80);
        assert_eq!(plan.volumes[0].additional_peak_bytes, 160);
        assert!(!plan.can_proceed);
    }
    #[test]
    fn base_and_pack_or_import_demands_share_one_volume_limit() {
        let disk = SpaceVolume {
            id: "same".into(),
            label: "same".into(),
            available_bytes: Some(99),
        };
        let demand = |bytes| {
            estimate_space(
                std::slice::from_ref(&disk),
                &[local_copy_artifact("same", bytes)],
                &Default::default(),
                4,
            )
            .unwrap()
        };
        assert_eq!(
            combine(demand(50), demand(50)).err().as_deref(),
            Some("SPACE_INSUFFICIENT")
        );
        let allowed = combine(demand(50), demand(49)).unwrap();
        assert_eq!(allowed.volumes.len(), 1);
        assert_eq!(allowed.volumes[0].materialized_bytes, 99);
    }
}

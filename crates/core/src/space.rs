// SPDX-License-Identifier: MIT
//! Additional peak disk demand. Existing retained bytes are not charged twice.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpaceVolume {
    pub id: String,
    pub label: String,
    pub available_bytes: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct SpaceArtifact {
    pub cache_volume: String,
    pub target_volume: String,
    pub compressed_bytes: Option<u64>,
    pub installed_bytes: Option<u64>,
    pub verified_cached: bool,
    pub already_installed: bool,
    pub extraction_scratch_bytes: Option<u64>,
    pub backup_copy_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceVerdict {
    Sufficient,
    Insufficient,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolumeSpace {
    pub volume: SpaceVolume,
    pub download_bytes: u64,
    pub download_staging_bytes: u64,
    pub materialized_bytes: u64,
    pub extraction_scratch_bytes: u64,
    pub backup_copy_bytes: u64,
    pub retained_existing_bytes: u64,
    pub additional_peak_bytes: u64,
    pub unknown_sizes: usize,
    pub verdict: SpaceVerdict,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpaceEstimate {
    pub volumes: Vec<VolumeSpace>,
    pub can_proceed: bool,
    pub exact: bool,
}

/// Cache growth + bounded in-flight transfers + final target copies + extraction
/// and explicit backup copies. Conservative: these phases may overlap.
pub fn estimate_space(
    volumes: &[SpaceVolume],
    artifacts: &[SpaceArtifact],
    retained_existing: &BTreeMap<String, u64>,
    concurrency: usize,
) -> Result<SpaceEstimate, &'static str> {
    if artifacts.len() > 300_000 || !(1..=32).contains(&concurrency) {
        return Err("SPACE_PLAN_LIMIT");
    }
    let mut output = BTreeMap::new();
    for volume in volumes {
        if output.contains_key(&volume.id) {
            return Err("SPACE_VOLUME_DUPLICATE");
        }
        output.insert(
            volume.id.clone(),
            VolumeSpace {
                volume: volume.clone(),
                download_bytes: 0,
                download_staging_bytes: 0,
                materialized_bytes: 0,
                extraction_scratch_bytes: 0,
                backup_copy_bytes: 0,
                retained_existing_bytes: *retained_existing.get(&volume.id).unwrap_or(&0),
                additional_peak_bytes: 0,
                unknown_sizes: 0,
                verdict: SpaceVerdict::Unknown,
            },
        );
    }
    let mut in_flight: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for item in artifacts {
        if !output.contains_key(&item.cache_volume) || !output.contains_key(&item.target_volume) {
            return Err("SPACE_VOLUME_MISSING");
        }
        if !item.verified_cached && !item.already_installed {
            let cache = output.get_mut(&item.cache_volume).unwrap();
            if let Some(size) = item.compressed_bytes {
                cache.download_bytes = cache
                    .download_bytes
                    .checked_add(size)
                    .ok_or("SPACE_OVERFLOW")?;
                in_flight
                    .entry(item.cache_volume.clone())
                    .or_default()
                    .push(size);
            } else {
                cache.unknown_sizes += 1;
            }
        }
        if !item.already_installed {
            let target = output.get_mut(&item.target_volume).unwrap();
            if let Some(size) = item.installed_bytes {
                target.materialized_bytes = target
                    .materialized_bytes
                    .checked_add(size)
                    .ok_or("SPACE_OVERFLOW")?;
            } else {
                target.unknown_sizes += 1;
            }
            if let Some(size) = item.extraction_scratch_bytes {
                target.extraction_scratch_bytes = target
                    .extraction_scratch_bytes
                    .checked_add(size)
                    .ok_or("SPACE_OVERFLOW")?;
            } else {
                target.unknown_sizes += 1;
            }
            target.backup_copy_bytes = target
                .backup_copy_bytes
                .checked_add(item.backup_copy_bytes)
                .ok_or("SPACE_OVERFLOW")?;
        }
    }
    for (id, mut sizes) in in_flight {
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        output.get_mut(&id).unwrap().download_staging_bytes = sizes
            .into_iter()
            .take(concurrency)
            .try_fold(0u64, |sum, size| sum.checked_add(size))
            .ok_or("SPACE_OVERFLOW")?;
    }
    for value in output.values_mut() {
        value.additional_peak_bytes = [
            value.download_bytes,
            value.download_staging_bytes,
            value.materialized_bytes,
            value.extraction_scratch_bytes,
            value.backup_copy_bytes,
        ]
        .into_iter()
        .try_fold(0u64, |sum, size| sum.checked_add(size))
        .ok_or("SPACE_OVERFLOW")?;
        value.verdict = match value.volume.available_bytes {
            Some(free) if free < value.additional_peak_bytes => SpaceVerdict::Insufficient,
            Some(_) if value.unknown_sizes == 0 => SpaceVerdict::Sufficient,
            _ => SpaceVerdict::Unknown,
        };
    }
    let volumes: Vec<_> = output.into_values().collect();
    Ok(SpaceEstimate {
        can_proceed: volumes
            .iter()
            .all(|v| v.verdict != SpaceVerdict::Insufficient),
        exact: volumes.iter().all(|v| v.unknown_sizes == 0),
        volumes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn artifact(cache: &str, target: &str, size: u64) -> SpaceArtifact {
        SpaceArtifact {
            cache_volume: cache.into(),
            target_volume: target.into(),
            compressed_bytes: Some(size),
            installed_bytes: Some(size),
            verified_cached: false,
            already_installed: false,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        }
    }
    #[test]
    fn same_volume_counts_cache_staging_and_target_without_charging_retained_source_twice() {
        let v = SpaceVolume {
            id: "a".into(),
            label: "a".into(),
            available_bytes: Some(29),
        };
        let plan = estimate_space(
            &[v],
            &[artifact("a", "a", 10)],
            &BTreeMap::from([("a".into(), 900)]),
            4,
        )
        .unwrap();
        assert_eq!(plan.volumes[0].additional_peak_bytes, 30);
        assert_eq!(plan.volumes[0].retained_existing_bytes, 900);
        assert!(!plan.can_proceed);
    }
    #[test]
    fn verified_cache_and_distinct_volumes_change_actual_additional_demand() {
        let vs = ["cache", "target"].map(|id| SpaceVolume {
            id: id.into(),
            label: id.into(),
            available_bytes: Some(100),
        });
        let mut a = artifact("cache", "target", 10);
        a.verified_cached = true;
        let p = estimate_space(&vs, &[a], &BTreeMap::new(), 4).unwrap();
        assert_eq!(p.volumes[0].additional_peak_bytes, 0);
        assert_eq!(p.volumes[1].additional_peak_bytes, 10);
        assert!(p.can_proceed && p.exact);
    }
    #[test]
    fn unknown_extraction_never_becomes_a_false_space_guarantee() {
        let v = SpaceVolume {
            id: "a".into(),
            label: "a".into(),
            available_bytes: Some(100),
        };
        let mut a = artifact("a", "a", 10);
        a.installed_bytes = None;
        a.extraction_scratch_bytes = None;
        let p = estimate_space(&[v], &[a], &BTreeMap::new(), 4).unwrap();
        assert!(p.can_proceed && !p.exact);
        assert_eq!(p.volumes[0].verdict, SpaceVerdict::Unknown);
    }
}

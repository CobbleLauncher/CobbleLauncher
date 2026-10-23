// SPDX-License-Identifier: MIT
//! Explicit, previewed snapshot deletion followed by safe-index-order rustic prune.
use crate::{
    BackupControl, BackupError, SnapshotInfo, WorldRepository, identity, info, is_complete, linked,
};
use rustic_core::{LimitOption, PruneOptions};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    pub keep_last: u32,
    pub keep_named: bool,
}
impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_last: 10,
            keep_named: true,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct RetentionPreview {
    pub policy: RetentionPolicy,
    pub remove: Vec<SnapshotInfo>,
    pub keep_count: usize,
    pub named_kept: usize,
    /// Packed blob estimate. Headers/indexes/recompression can change physical savings.
    pub estimated_reclaim_bytes: u64,
    pub repack_bytes: u64,
}
/// Cannot be deserialized from renderer data. Repository revision binds the approval.
pub struct RetentionPlan {
    pub preview: RetentionPreview,
    world_id: String,
    revision: Vec<String>,
    remove_ids: BTreeSet<String>,
}
#[derive(Debug, Serialize)]
pub struct RetentionResult {
    pub removed_count: usize,
    pub reclaimed_bytes: u64,
    pub cleanup_complete: bool,
    pub remaining: Vec<SnapshotInfo>,
}
fn options() -> PruneOptions {
    let mut options = PruneOptions::default();
    options.max_unused = LimitOption::Percentage(0);
    options.max_repack = LimitOption::Unlimited;
    options.instant_delete = true;
    options.early_delete_index = false;
    options.no_resize = true;
    options
}
fn select(
    mut snapshots: Vec<SnapshotInfo>,
    policy: &RetentionPolicy,
) -> Result<(Vec<SnapshotInfo>, usize, usize), BackupError> {
    if !(1..=1000).contains(&policy.keep_last) || snapshots.len() > 50_000 {
        return Err(BackupError::InvalidInput);
    }
    snapshots.sort_by_cached_key(|snapshot| {
        let timestamp = snapshot
            .created_at
            .parse::<rustic_core::jiff::Zoned>()
            .ok()
            .map(|time| time.timestamp())
            .or_else(|| {
                snapshot
                    .created_at
                    .parse::<rustic_core::jiff::Timestamp>()
                    .ok()
            });
        (std::cmp::Reverse(timestamp), snapshot.id.clone())
    });
    let named_kept = snapshots
        .iter()
        .filter(|snapshot| policy.keep_named && snapshot.protected)
        .count();
    let mut remove = vec![];
    let mut keep = 0;
    for (index, snapshot) in snapshots.into_iter().enumerate() {
        if index < policy.keep_last as usize || (policy.keep_named && snapshot.protected) {
            keep += 1;
        } else {
            remove.push(snapshot);
        }
    }
    Ok((remove, keep, named_kept))
}
pub(crate) fn bytes(path: &Path) -> Result<u64, BackupError> {
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if linked(&metadata) {
            return Err(BackupError::UnsafeSource);
        }
        if metadata.is_dir() {
            total = total.saturating_add(bytes(&path)?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        } else {
            return Err(BackupError::UnsafeSource);
        }
    }
    Ok(total)
}
impl WorldRepository {
    pub fn retention_preview(
        &self,
        world_id: &str,
        policy: &RetentionPolicy,
    ) -> Result<RetentionPlan, BackupError> {
        identity(world_id)?;
        let repo = self.repository()?;
        let snapshots = repo
            .get_all_snapshots()
            .map_err(|_| BackupError::Engine("list snapshots for cleanup"))?;
        if snapshots.len() > 50_000 {
            return Err(BackupError::InvalidInput);
        }
        let mut revision: Vec<_> = snapshots
            .iter()
            .map(|snapshot| snapshot.id.to_hex().to_string())
            .collect();
        revision.sort();
        let selected = snapshots
            .iter()
            .filter(|snapshot| snapshot.label == world_id && is_complete(snapshot))
            .cloned()
            .map(info)
            .collect();
        let (remove, keep_count, named_kept) = select(selected, policy)?;
        let remove_ids: BTreeSet<_> = remove.iter().map(|snapshot| snapshot.id.clone()).collect();
        let mut opts = options();
        opts.ignore_snaps = snapshots
            .iter()
            .filter(|snapshot| remove_ids.contains(&snapshot.id.to_hex().to_string()))
            .map(|snapshot| snapshot.id)
            .collect();
        // ignore_snaps is used ONLY for a read-only estimate, never for execution.
        let plan = repo
            .prune_plan(&opts)
            .map_err(|_| BackupError::Engine("preview repository cleanup"))?;
        let size = plan.stats.size_sum();
        let estimated_reclaim_bytes = size
            .remove
            .saturating_add(size.repackrm)
            .saturating_add(plan.stats.size_unref)
            .saturating_add(plan.stats.size_to_delete.remove);
        Ok(RetentionPlan {
            preview: RetentionPreview {
                policy: policy.clone(),
                remove,
                keep_count,
                named_kept,
                estimated_reclaim_bytes,
                repack_bytes: size.repack,
            },
            world_id: world_id.into(),
            revision,
            remove_ids,
        })
    }
    /// Only explicitly tagged automatic versions count toward this limit.
    pub fn retain_automatic_controlled(
        &mut self,
        world_id: &str,
        control: &BackupControl,
    ) -> Result<RetentionResult, BackupError> {
        control.check()?;
        let mut plan = self.retention_preview(world_id, &RetentionPolicy::default())?;
        plan.preview.remove = self
            .list(world_id)?
            .into_iter()
            .filter(|snapshot| snapshot.automatic && !snapshot.protected)
            .skip(10)
            .collect();
        plan.remove_ids = plan.preview.remove.iter().map(|s| s.id.clone()).collect();
        if plan.remove_ids.is_empty() {
            return Ok(RetentionResult {
                removed_count: 0,
                reclaimed_bytes: 0,
                cleanup_complete: true,
                remaining: self.list(world_id)?,
            });
        }
        self.apply_retention_controlled(plan, control)
    }
    pub fn apply_retention(&mut self, plan: RetentionPlan) -> Result<RetentionResult, BackupError> {
        self.apply_retention_controlled(plan, &BackupControl::default())
    }
    /// Cancellation is accepted before deletion. Once deletion begins, finish the
    /// short snapshot commit and prune without a cancellation-induced partial index.
    pub fn apply_retention_controlled(
        &mut self,
        plan: RetentionPlan,
        control: &BackupControl,
    ) -> Result<RetentionResult, BackupError> {
        control.check()?;
        let repo = self.repository()?;
        let snapshots = repo
            .get_all_snapshots()
            .map_err(|_| BackupError::Engine("verify cleanup approval"))?;
        let mut revision: Vec<_> = snapshots
            .iter()
            .map(|snapshot| snapshot.id.to_hex().to_string())
            .collect();
        revision.sort();
        if revision != plan.revision {
            return Err(BackupError::Engine("apply stale cleanup preview"));
        }
        let ids: Vec<_> = snapshots
            .iter()
            .filter(|snapshot| {
                plan.remove_ids.contains(&snapshot.id.to_hex().to_string())
                    && snapshot.label == plan.world_id
                    && is_complete(snapshot)
            })
            .map(|snapshot| snapshot.id)
            .collect();
        if ids.len() != plan.remove_ids.len() {
            return Err(BackupError::InvalidInput);
        }
        let before = bytes(&self.root)?;
        control.check()?;
        let deleted = repo.delete_snapshots(&ids).is_ok();
        // Always re-plan against the snapshots that actually remain. Never execute
        // an ignore_snaps estimate, even when deleting snapshots partially fails.
        let cleanup_complete = deleted
            && repo
                .prune_plan(&options())
                .and_then(|prune| repo.prune(&options(), prune))
                .is_ok();
        let remaining = self.list(&plan.world_id)?;
        let existing: BTreeSet<_> = remaining.iter().map(|snapshot| &snapshot.id).collect();
        let removed_count = plan
            .remove_ids
            .iter()
            .filter(|id| !existing.contains(id))
            .count();
        let after = bytes(&self.root)?;
        Ok(RetentionResult {
            removed_count,
            reclaimed_bytes: before.saturating_sub(after),
            cleanup_complete,
            remaining,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(index: u32, protected: bool) -> SnapshotInfo {
        SnapshotInfo {
            id: format!("{index:064x}"),
            world_id: "world".into(),
            name: index.to_string(),
            created_at: format!("2026-10-22T00:00:{index:02}Z"),
            processed_bytes: 1,
            added_bytes: 1,
            protected,
            automatic: false,
        }
    }
    #[test]
    fn keeps_latest_and_protected_and_rejects_zero_retention() {
        let input = vec![snapshot(1, false), snapshot(2, true), snapshot(3, false)];
        let (remove, kept, named) = select(
            input.clone(),
            &RetentionPolicy {
                keep_last: 1,
                keep_named: true,
            },
        )
        .unwrap();
        assert_eq!(remove[0].id, input[0].id);
        assert_eq!((kept, named), (2, 1));
        assert_eq!(
            select(
                input.clone(),
                &RetentionPolicy {
                    keep_last: 1,
                    keep_named: false
                }
            )
            .unwrap()
            .0
            .len(),
            2
        );
        assert!(
            select(
                input,
                &RetentionPolicy {
                    keep_last: 0,
                    keep_named: false
                }
            )
            .is_err()
        );
        let mut earlier = snapshot(4, false);
        earlier.created_at = "2026-10-22T10:00:00+08:00".into();
        let mut later = snapshot(5, false);
        later.created_at = "2026-10-22T04:00:00Z".into();
        assert_eq!(
            select(
                vec![earlier.clone(), later],
                &RetentionPolicy {
                    keep_last: 1,
                    keep_named: false
                }
            )
            .unwrap()
            .0[0]
                .id,
            earlier.id
        );
    }
    #[test]
    fn legacy_snapshots_stay_protected_until_explicit_policy_override() {
        let mut snapshot = rustic_core::SnapshotOptions::default()
            .to_snapshot()
            .unwrap();
        snapshot.tags = crate::COMPLETE_TAG.parse().unwrap();
        assert!(info(snapshot.clone()).protected);
        snapshot.tags = format!("{},{}", crate::COMPLETE_TAG, crate::RETENTION_TAG)
            .parse()
            .unwrap();
        assert!(!info(snapshot.clone()).protected);
        snapshot.tags = format!(
            "{},{},{}",
            crate::COMPLETE_TAG,
            crate::RETENTION_TAG,
            crate::NAMED_TAG
        )
        .parse()
        .unwrap();
        assert!(info(snapshot).protected);
    }
    #[test]
    fn actual_repository_cleanup_preserves_named_other_world_and_remaining_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("level.dat"), b"original-level").unwrap();
        let mut repo =
            WorldRepository::create(&temp.path().join("repo"), "original test password").unwrap();
        let mut seed = 7u64;
        let old_data: Vec<u8> = (0..65536)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                (seed >> 32) as u8
            })
            .collect();
        fs::write(source.join("data.bin"), old_data).unwrap();
        let old = repo.snapshot(&source, "world", "").unwrap();
        fs::write(source.join("data.bin"), vec![2; 65536]).unwrap();
        let named = repo.snapshot(&source, "world", "Before upgrade").unwrap();
        fs::write(source.join("data.bin"), vec![3; 65536]).unwrap();
        let latest = repo.snapshot(&source, "world", "").unwrap();
        let other = repo.snapshot(&source, "other", "Other world").unwrap();
        let policy = RetentionPolicy {
            keep_last: 1,
            keep_named: true,
        };
        let plan = repo.retention_preview("world", &policy).unwrap();
        assert_eq!(
            plan.preview
                .remove
                .iter()
                .map(|value| &value.id)
                .collect::<Vec<_>>(),
            vec![&old.id]
        );
        let result = repo.apply_retention(plan).unwrap();
        assert!(result.cleanup_complete);
        assert_eq!(result.removed_count, 1);
        assert!(result.reclaimed_bytes > 0);
        assert_eq!(repo.list("other").unwrap()[0].id, other.id);
        for (id, value) in [(named.id, 2), (latest.id, 3)] {
            let stage = temp.path().join(format!("restore-{value}"));
            repo.restore(&id, "world", &stage).unwrap();
            assert_eq!(
                fs::read(stage.join("data.bin")).unwrap(),
                vec![value; 65536]
            );
        }
        let other_stage = temp.path().join("restore-other");
        repo.restore(&other.id, "other", &other_stage).unwrap();
        assert_eq!(
            fs::read(other_stage.join("data.bin")).unwrap(),
            vec![3; 65536]
        );
        let stale = repo.retention_preview("world", &policy).unwrap();
        repo.snapshot(&source, "other", "").unwrap();
        assert!(repo.apply_retention(stale).is_err());
        let plan = repo
            .retention_preview(
                "world",
                &RetentionPolicy {
                    keep_last: 1,
                    keep_named: false,
                },
            )
            .unwrap();
        assert_eq!(plan.preview.remove.len(), 1);
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        assert!(matches!(
            repo.apply_retention_controlled(plan, &BackupControl::new(cancel, |_| {})),
            Err(BackupError::Cancelled)
        ));
        assert_eq!(repo.list("world").unwrap().len(), 2);
    }
}

// SPDX-License-Identifier: MIT
//! Read-only repository accounting and content-identity snapshot comparison.
use crate::{BackupControl, BackupError, WorldRepository, identity, is_complete, relative};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Debug, Serialize)]
pub struct RepositoryInsights {
    pub repository_stored_bytes: u64,
    pub total_snapshot_logical_bytes: u64,
    pub snapshot_count: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotChangeKind {
    Added,
    Modified,
    Deleted,
}
#[derive(Debug, Serialize)]
pub struct SnapshotChange {
    pub path: String,
    pub kind: SnapshotChangeKind,
    pub before_bytes: Option<u64>,
    pub after_bytes: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct SnapshotComparison {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub unchanged: usize,
    pub total: usize,
    pub offset: usize,
    pub rows: Vec<SnapshotChange>,
}
impl WorldRepository {
    /// Physical bytes include repository overhead and all worlds; logical bytes sum
    /// all complete snapshot versions and deliberately count reused files again.
    pub fn storage_insights(&self) -> Result<RepositoryInsights, BackupError> {
        let snapshots = self
            .repository()?
            .get_all_snapshots()
            .map_err(|_| BackupError::Engine("list storage insights"))?;
        if snapshots.len() > 50_000 {
            return Err(BackupError::InvalidInput);
        }
        let mut logical = 0u64;
        let mut count = 0;
        for snapshot in snapshots.iter().filter(|s| is_complete(s)) {
            logical = logical.saturating_add(
                snapshot
                    .summary
                    .as_ref()
                    .map_or(0, |s| s.total_bytes_processed),
            );
            count += 1;
        }
        Ok(RepositoryInsights {
            repository_stored_bytes: crate::retention::bytes(&self.root)?,
            total_snapshot_logical_bytes: logical,
            snapshot_count: count,
        })
    }
    /// Comparison reads bounded tree metadata, never restores or reads data blobs.
    pub fn compare_snapshots(
        &self,
        world_id: &str,
        before: &str,
        after: &str,
        offset: usize,
        limit: usize,
        control: &BackupControl,
    ) -> Result<SnapshotComparison, BackupError> {
        identity(world_id)?;
        if limit == 0 || limit > 100 || offset > 200_000 {
            return Err(BackupError::InvalidInput);
        }
        let repo = self
            .repository_controlled(control)?
            .to_indexed()
            .map_err(|_| control.error("load comparison index"))?;
        let mut trees = Vec::new();
        for id in [before, after] {
            control.check()?;
            if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(BackupError::InvalidInput);
            }
            let snapshot = repo
                .get_snapshot_from_str(id, |_| true)
                .map_err(|_| BackupError::SnapshotNotFound)?;
            if snapshot.label != world_id || !is_complete(&snapshot) {
                return Err(BackupError::SnapshotNotFound);
            }
            let node = repo
                .node_from_snapshot_and_path(&snapshot, "world")
                .map_err(|_| control.error("read comparison tree"))?;
            let mut files = BTreeMap::new();
            for entry in repo
                .ls(&node, &rustic_core::LsOptions::default())
                .map_err(|_| control.error("list comparison tree"))?
            {
                control.check()?;
                let (path, node) = entry.map_err(|_| control.error("read comparison entry"))?;
                if path.as_os_str().is_empty() {
                    continue;
                }
                relative(&path)?;
                if node.is_dir() {
                    continue;
                }
                if !node.is_file() {
                    return Err(BackupError::UnsafeSource);
                }
                let path = path
                    .to_str()
                    .ok_or(BackupError::UnsafeSource)?
                    .replace('\\', "/");
                if files.len() >= 100_000 || files.insert(path, node).is_some() {
                    return Err(BackupError::InvalidInput);
                }
            }
            trees.push(files);
        }
        let newer = trees.pop().unwrap();
        let older = trees.pop().unwrap();
        let mut rows = Vec::new();
        let (mut added, mut modified, mut deleted, mut unchanged) = (0, 0, 0, 0);
        for (path, node) in &older {
            match newer.get(path) {
                None => {
                    deleted += 1;
                    rows.push(SnapshotChange {
                        path: path.clone(),
                        kind: SnapshotChangeKind::Deleted,
                        before_bytes: Some(node.meta.size),
                        after_bytes: None,
                    });
                }
                Some(next) if node.meta.size == next.meta.size && node.content == next.content => {
                    unchanged += 1
                }
                Some(next) => {
                    modified += 1;
                    rows.push(SnapshotChange {
                        path: path.clone(),
                        kind: SnapshotChangeKind::Modified,
                        before_bytes: Some(node.meta.size),
                        after_bytes: Some(next.meta.size),
                    });
                }
            }
        }
        for (path, node) in &newer {
            if !older.contains_key(path) {
                added += 1;
                rows.push(SnapshotChange {
                    path: path.clone(),
                    kind: SnapshotChangeKind::Added,
                    before_bytes: None,
                    after_bytes: Some(node.meta.size),
                });
            }
        }
        rows.sort_by(|a, b| a.path.cmp(&b.path));
        let total = rows.len();
        let rows = rows.into_iter().skip(offset).take(limit).collect();
        Ok(SnapshotComparison {
            added,
            modified,
            deleted,
            unchanged,
            total,
            offset,
            rows,
        })
    }
}

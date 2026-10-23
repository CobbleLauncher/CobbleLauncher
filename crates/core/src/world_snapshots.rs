// SPDX-License-Identifier: MIT
//! Repository snapshots under the caller's stopped-instance operation lease.
use crate::{OperationControl, WorldImportOutcome};
pub use cobble_backup::SnapshotInfo as WorldSnapshot;
pub use cobble_backup::WorldRepository as WorldSnapshotRepository;
use cobble_backup::WorldRepository;
pub use cobble_backup::{RetentionPlan, RetentionPolicy, RetentionPreview, RetentionResult};
use sha2::{Digest, Sha256};
use std::path::Path;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Stable repository label for a validated folder, including Unicode world names.
pub fn world_snapshot_identity(folder_name: &str) -> String {
    Sha256::digest(folder_name.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn create_world_snapshot(
    game_dir: &Path,
    folder_name: &str,
    repository: &mut WorldRepository,
    name: &str,
) -> Result<WorldSnapshot, Error> {
    create_world_snapshot_controlled(
        game_dir,
        folder_name,
        repository,
        name,
        &OperationControl::new(std::sync::Arc::new(|_| {})),
    )
}

fn backup_control(control: &OperationControl) -> cobble_backup::BackupControl {
    let cancellation = control.clone();
    let progress = control.progress();
    cobble_backup::BackupControl::with_cancellation(
        move || cancellation.is_cancelled(),
        move |value| {
            let phase = match value.phase {
                cobble_backup::BackupPhase::Scan => "world-snapshot-scan",
                cobble_backup::BackupPhase::Snapshot => "world-snapshot-write",
                cobble_backup::BackupPhase::Verify => "world-snapshot-verify",
                cobble_backup::BackupPhase::Restore => "world-snapshot-restore",
                cobble_backup::BackupPhase::Publish => "world-snapshot-publish",
                cobble_backup::BackupPhase::Complete => "world-snapshot-complete",
            };
            progress(crate::InstallProgress {
                phase: phase.into(),
                completed: value.source_bytes.try_into().unwrap_or(usize::MAX),
                total: value
                    .total_bytes
                    .unwrap_or(0)
                    .try_into()
                    .unwrap_or(usize::MAX),
                reused: 0,
                detail: None,
            });
        },
    )
}

pub fn create_world_snapshot_controlled(
    game_dir: &Path,
    folder_name: &str,
    repository: &mut WorldRepository,
    name: &str,
    control: &OperationControl,
) -> Result<WorldSnapshot, Error> {
    control.check_cancelled()?;
    crate::world_restore::ensure_world_operations_complete(game_dir)?;
    let world = cobble_content::world_directory(game_dir, folder_name)?;
    let _lock = crate::world_restore::lock_world(&world)?;
    Ok(repository.snapshot_controlled(
        &world,
        &world_snapshot_identity(folder_name),
        name,
        &backup_control(control),
    )?)
}

/// Restore independent bytes through the existing recoverable world publication journal.
/// The caller must hold its instance lease until this future returns.
#[allow(clippy::too_many_arguments)]
pub async fn restore_world_snapshot(
    game_dir: &Path,
    folder_name: &str,
    snapshot_id: &str,
    repository: WorldRepository,
    replace_existing: bool,
    operation_id: &str,
    control: OperationControl,
) -> Result<WorldImportOutcome, Error> {
    control.check_cancelled()?;
    // Validates the selected existing folder without granting the repository a renderer path.
    cobble_content::world_directory(game_dir, folder_name)?;
    let folder = folder_name.to_owned();
    let snapshot_id = snapshot_id.to_owned();
    let game = game_dir.to_owned();
    let operation_id = operation_id.to_owned();
    tokio::task::spawn_blocking(move || -> Result<_, Error> {
        let mut repository = repository;
        crate::world_restore::publish_repository_snapshot(
            &game,
            &folder,
            &snapshot_id,
            replace_existing,
            &operation_id,
            &control,
            |stage| {
                Ok(repository.restore_controlled(
                    &snapshot_id,
                    &world_snapshot_identity(&folder),
                    stage,
                    &backup_control(&control),
                )?)
            },
        )
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn repository_snapshot_roundtrip_preserves_replaced_generation() {
        let root = std::env::temp_dir().join(format!(
            "cobble-core-snapshot-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let game = root.join("game");
        let world = game.join("saves/我的世界 1");
        fs::create_dir_all(&world).unwrap();
        fs::write(world.join("level.dat"), b"recoverable metadata").unwrap();
        fs::write(world.join("region.dat"), b"snapshot generation").unwrap();
        let mut repository =
            WorldRepository::create(&root.join("repository"), "temporary-test-password").unwrap();
        let snapshot =
            create_world_snapshot(&game, "我的世界 1", &mut repository, "Before changes").unwrap();
        fs::write(world.join("region.dat"), b"newer generation").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let cancellation = Arc::new(std::sync::OnceLock::<cobble_net::TransferCancellation>::new());
        let observer = cancellation.clone();
        let cancel_restore = OperationControl::new(Arc::new(move |event| {
            if event.phase == "world-snapshot-restore" {
                observer.get().unwrap().cancel();
            }
        }));
        assert!(cancellation.set(cancel_restore.cancellation()).is_ok());
        assert!(
            runtime
                .block_on(restore_world_snapshot(
                    &game,
                    "我的世界 1",
                    &snapshot.id,
                    repository,
                    true,
                    "cancelled-snapshot",
                    cancel_restore,
                ))
                .is_err()
        );
        assert_eq!(
            fs::read(world.join("region.dat")).unwrap(),
            b"newer generation"
        );
        assert!(
            !game
                .join(".cobble-world-operations/cancelled-snapshot/stage")
                .exists()
        );
        let repository =
            WorldRepository::open(&root.join("repository"), "temporary-test-password").unwrap();
        let result = runtime
            .block_on(restore_world_snapshot(
                &game,
                "我的世界 1",
                &snapshot.id,
                repository,
                true,
                "snapshot-roundtrip",
                OperationControl::new(Arc::new(|_| {})),
            ))
            .unwrap();
        assert_eq!(result.backup_id.as_deref(), Some("snapshot-roundtrip"));
        assert_eq!(
            fs::read(world.join("region.dat")).unwrap(),
            b"snapshot generation"
        );
        assert_eq!(
            fs::read(game.join(".cobble-world-backups/snapshot-roundtrip/world/region.dat"))
                .unwrap(),
            b"newer generation"
        );
    }
}

/// Caller holds the stopped-instance lease across snapshot and retention.
pub fn create_automatic_world_snapshot_controlled(
    game_dir: &Path,
    folder_name: &str,
    repository: &mut WorldRepository,
    control: &OperationControl,
) -> Result<Option<WorldSnapshot>, Error> {
    control.check_cancelled()?;
    crate::world_restore::ensure_world_operations_complete(game_dir)?;
    let world = cobble_content::world_directory(game_dir, folder_name)?;
    let _lock = crate::world_restore::lock_world(&world)?;
    let backup_control = backup_control(control);
    let snapshot = repository.snapshot_automatic_controlled(
        &world,
        &world_snapshot_identity(folder_name),
        &backup_control,
    )?;
    Ok(snapshot)
}

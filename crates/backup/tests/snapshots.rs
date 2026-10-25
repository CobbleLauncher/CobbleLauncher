// SPDX-License-Identifier: MIT
//! Original disposable data only; these tests never open installed/user worlds.
use cobble_backup::{BackupError, WorldRepository};
use std::{fs, path::Path, time::Instant};

#[test]
fn controlled_cancel_does_not_publish_generations_and_keeps_owned_restore_stage() {
    use cobble_backup::{BackupControl, BackupPhase};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let temporary = tempfile::tempdir().unwrap();
    let world = temporary.path().join("world");
    fs::create_dir(&world).unwrap();
    fs::write(world.join("level.dat"), vec![7; 2 * 1024 * 1024]).unwrap();
    let mut repository = WorldRepository::create(
        &temporary.path().join("repository"),
        "synthetic-control-password",
    )
    .unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let control = BackupControl::new(cancelled, move |progress| {
        if progress.phase == BackupPhase::Snapshot && progress.source_bytes > 0 {
            flag.store(true, Ordering::Relaxed);
        }
    });
    assert!(matches!(
        repository.snapshot_controlled(&world, "world-1", "Cancelled", &control),
        Err(BackupError::Cancelled)
    ));
    assert!(repository.list("world-1").unwrap().is_empty());
    let good = repository.snapshot(&world, "world-1", "Good").unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let control = BackupControl::new(cancelled, move |progress| {
        if progress.phase == BackupPhase::Publish {
            flag.store(true, Ordering::Relaxed);
        }
    });
    assert!(matches!(
        repository.snapshot_controlled(&world, "world-1", "Do not publish", &control),
        Err(BackupError::Cancelled)
    ));
    assert_eq!(repository.list("world-1").unwrap().len(), 1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let control = BackupControl::new(cancelled, move |progress| {
        if progress.phase == BackupPhase::Restore && progress.source_bytes > 0 {
            flag.store(true, Ordering::Relaxed);
        }
    });
    let stage = temporary.path().join("stage");
    assert!(matches!(
        repository.restore_controlled(&good.id, "world-1", &stage, &control),
        Err(BackupError::Cancelled)
    ));
    assert!(stage.is_dir());
    assert_eq!(
        fs::read(world.join("level.dat")).unwrap(),
        vec![7; 2 * 1024 * 1024]
    );
    assert_eq!(repository.list("world-1").unwrap().len(), 1);
}

fn bytes_on_disk(path: &Path) -> u64 {
    fs::read_dir(path)
        .unwrap()
        .map(|item| {
            let item = item.unwrap();
            if item.file_type().unwrap().is_dir() {
                bytes_on_disk(&item.path())
            } else {
                item.metadata().unwrap().len()
            }
        })
        .sum()
}

#[test]
fn synthetic_versions_deduplicate_and_restore_independent_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("repository");
    let world = temporary.path().join("SyntheticWorld");
    fs::create_dir(&world).unwrap();
    fs::create_dir(world.join("region")).unwrap();
    // Deterministic high-entropy fake region bytes, not a copied real world.
    let mut state = 0x4d59_5df4_d0f3_3173_u64;
    let original: Vec<u8> = (0..8 * 1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    fs::write(world.join("region/r.0.0.mca"), &original).unwrap();
    fs::write(world.join("level.dat"), b"synthetic metadata v1").unwrap();
    fs::write(world.join("removed.txt"), b"old file").unwrap();
    fs::write(world.join("session.lock"), b"must not be restored").unwrap();
    let init = Instant::now();
    let mut repository = WorldRepository::create(&root, "synthetic-test-password").unwrap();
    let init_ms = init.elapsed().as_millis();
    let start = Instant::now();
    let first = repository
        .snapshot(&world, "world-001", "Before edit")
        .unwrap();
    let first_ms = start.elapsed().as_millis();
    let first_disk = bytes_on_disk(&root);
    let start = Instant::now();
    let unchanged = repository
        .snapshot(&world, "world-001", "Unchanged")
        .unwrap();
    let unchanged_ms = start.elapsed().as_millis();
    let unchanged_disk = bytes_on_disk(&root);
    assert!(
        unchanged.added_bytes < 64 * 1024,
        "unchanged content added unexpectedly many bytes"
    );
    assert!(
        unchanged_disk - first_disk < 64 * 1024,
        "unchanged version physically duplicated data"
    );
    let mut edited = original.clone();
    edited[3 * 1024 * 1024..3 * 1024 * 1024 + 4096].fill(17);
    fs::write(world.join("region/r.0.0.mca"), &edited).unwrap();
    fs::write(world.join("level.dat"), b"synthetic metadata v2").unwrap();
    fs::remove_file(world.join("removed.txt")).unwrap();
    let start = Instant::now();
    let second = repository.snapshot(&world, "world-001", "").unwrap();
    let second_ms = start.elapsed().as_millis();
    let second_disk = bytes_on_disk(&root);
    assert!(!second.name.is_empty());
    assert_eq!(repository.list("world-001").unwrap().len(), 3);
    assert!(repository.list("other-world").unwrap().is_empty());
    let first_stage = temporary.path().join("first-stage");
    let second_stage = temporary.path().join("second-stage");
    let start = Instant::now();
    repository
        .restore(&first.id, "world-001", &first_stage)
        .unwrap();
    repository
        .restore(&second.id, "world-001", &second_stage)
        .unwrap();
    let restore_ms = start.elapsed().as_millis();
    assert_eq!(
        fs::read(first_stage.join("region/r.0.0.mca")).unwrap(),
        original
    );
    assert_eq!(
        fs::read(second_stage.join("region/r.0.0.mca")).unwrap(),
        edited
    );
    assert_eq!(
        fs::read(first_stage.join("level.dat")).unwrap(),
        b"synthetic metadata v1"
    );
    assert_eq!(
        fs::read(second_stage.join("level.dat")).unwrap(),
        b"synthetic metadata v2"
    );
    assert_eq!(
        fs::read(first_stage.join("removed.txt")).unwrap(),
        b"old file"
    );
    assert!(!second_stage.join("removed.txt").exists());
    assert!(!first_stage.join("session.lock").exists());
    assert!(!second_stage.join("session.lock").exists());
    fs::write(second_stage.join("level.dat"), b"independent restored edit").unwrap();
    assert_eq!(
        fs::read(world.join("level.dat")).unwrap(),
        b"synthetic metadata v2"
    );
    assert!(matches!(
        repository.restore(&first.id, "world-001", &first_stage),
        Err(BackupError::DestinationExists)
    ));
    assert!(matches!(
        repository.restore(
            &first.id,
            "other-world",
            &temporary.path().join("wrong-world")
        ),
        Err(BackupError::SnapshotNotFound)
    ));
    assert!(WorldRepository::create(&root, "another-password").is_err());
    assert!(WorldRepository::open(&root, "wrong-password").is_err());
    drop(repository);
    assert_eq!(
        WorldRepository::open(&root, "synthetic-test-password")
            .unwrap()
            .list("world-001")
            .unwrap()
            .len(),
        3
    );
    println!(
        "synthetic 8MiB region: init_ms={init_ms}; first_ms={first_ms}, packed_added={}, repo_bytes={first_disk}; unchanged_ms={unchanged_ms}, packed_added={}, repo_growth={}; edit_ms={second_ms}, packed_added={}, repo_growth={}; restore_both_ms={restore_ms}",
        first.added_bytes,
        unchanged.added_bytes,
        unchanged_disk - first_disk,
        second.added_bytes,
        second_disk - unchanged_disk
    );
}

#[test]
fn storage_insights_and_content_comparison_are_read_only_and_paged() {
    let temp = tempfile::tempdir().unwrap();
    let world = temp.path().join("world");
    fs::create_dir(&world).unwrap();
    fs::write(world.join("same"), b"same").unwrap();
    fs::write(world.join("changed"), b"aaaa").unwrap();
    fs::write(world.join("deleted"), b"old").unwrap();
    let mut repo =
        WorldRepository::create(&temp.path().join("repo"), "original-insights-test-password")
            .unwrap();
    let a = repo.snapshot(&world, "world-1", "Before").unwrap();
    fs::write(world.join("changed"), b"bbbb").unwrap();
    fs::remove_file(world.join("deleted")).unwrap();
    fs::write(world.join("added"), b"new").unwrap();
    let b = repo.snapshot(&world, "world-1", "After").unwrap();
    let stats = repo.storage_insights().unwrap();
    assert_eq!(stats.snapshot_count, 2);
    assert_eq!(
        stats.total_snapshot_logical_bytes,
        a.processed_bytes + b.processed_bytes
    );
    assert!(stats.repository_stored_bytes > 0);
    let control = cobble_backup::BackupControl::default();
    let page = repo
        .compare_snapshots("world-1", &a.id, &b.id, 1, 1, &control)
        .unwrap();
    assert_eq!(
        (
            page.added,
            page.modified,
            page.deleted,
            page.unchanged,
            page.total
        ),
        (1, 1, 1, 1, 3)
    );
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].path, "changed");
    let filtered = repo
        .compare_snapshots_filtered(
            "world-1",
            &a.id,
            &b.id,
            cobble_backup::SnapshotComparePage {
                offset: 0,
                limit: 1,
                query: "CHANG",
            },
            &control,
        )
        .unwrap();
    assert_eq!(
        (
            filtered.added,
            filtered.modified,
            filtered.deleted,
            filtered.total
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(filtered.rows[0].path, "changed");
    assert!(
        repo.compare_snapshots_filtered(
            "world-1",
            &a.id,
            &b.id,
            cobble_backup::SnapshotComparePage {
                offset: 0,
                limit: 1,
                query: &"x".repeat(121)
            },
            &control
        )
        .is_err()
    );
    assert!(
        repo.compare_snapshots("other-world", &a.id, &b.id, 0, 50, &control)
            .is_err()
    );
    assert!(
        repo.compare_snapshots("world-1", &a.id, &b.id, 0, 101, &control)
            .is_err()
    );
    assert_eq!(repo.list("world-1").unwrap().len(), 2);
}

#[test]
fn automatic_generations_skip_unchanged_preserve_manual_and_keep_ten() {
    let temporary = tempfile::tempdir().unwrap();
    let world = temporary.path().join("world");
    fs::create_dir(&world).unwrap();
    fs::write(world.join("level.dat"), b"first").unwrap();
    let mut repo = WorldRepository::create(&temporary.path().join("repo"), "fixture-only").unwrap();
    let manual = repo.snapshot(&world, "world", "").unwrap();
    let named = repo.snapshot(&world, "world", "Keep this").unwrap();
    let control = cobble_backup::BackupControl::default();
    assert!(
        repo.snapshot_automatic_controlled(&world, "world", &control)
            .unwrap()
            .is_some()
    );
    assert!(
        repo.snapshot_automatic_controlled(&world, "world", &control)
            .unwrap()
            .is_none()
    );
    for generation in 0..11 {
        fs::write(world.join("level.dat"), format!("generation-{generation}")).unwrap();
        assert!(
            repo.snapshot_automatic_controlled(&world, "world", &control)
                .unwrap()
                .is_some()
        );
    }
    repo.retain_automatic_controlled("world", &control).unwrap();
    let snapshots = repo.list("world").unwrap();
    assert_eq!(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.automatic)
            .count(),
        10
    );
    assert!(snapshots.iter().any(|snapshot| snapshot.id == manual.id));
    assert!(snapshots.iter().any(|snapshot| snapshot.id == named.id));
}

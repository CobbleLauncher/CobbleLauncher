// SPDX-License-Identifier: MIT
use super::*;
fn fixture() -> (tempfile::TempDir, Plan) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("games");
    let parent = temp.path().join("other-disk");
    fs::create_dir_all(source.join("one/saves/world")).unwrap();
    fs::create_dir_all(source.join("one/config")).unwrap();
    fs::create_dir_all(&parent).unwrap();
    fs::write(source.join("one/saves/world/level.dat"), b"world").unwrap();
    fs::write(source.join("one/config/settings.json"), b"config").unwrap();
    fs::write(source.join("loose-file.txt"), b"keep root contents").unwrap();
    let source = real_directory(&source).unwrap();
    let target = destination(&source, &parent).unwrap();
    (
        temp,
        Plan {
            id: "abc123".into(),
            source,
            target,
            entries: vec![Entry {
                id: "one".into(),
                relative: "one".into(),
            }],
        },
    )
}
fn game(id: &str, path: &Path) -> Game {
    serde_json::from_value(serde_json::json!({"id":id,"name":id,"version":"1.21.1","directory":path,"status":"installed","error":null})).unwrap()
}
#[test]
fn directory_migration_preserves_worlds_configs_loose_files_and_independence() {
    let (_temp, plan) = fixture();
    let cancel = AtomicBool::new(false);
    assert!(!prepare(&plan, &AtomicBool::new(false)).unwrap());
    let inventory = disk::inventory(&plan.source, &cancel).unwrap();
    disk::copy_verified(&plan.source, &plan.target, &inventory, &cancel, |_| {}).unwrap();
    write_receipt(&plan, Some(inventory.clone())).unwrap();
    assert_eq!(
        read_receipt(&plan).unwrap().inventory,
        Some(fingerprint(&inventory).unwrap())
    );
    assert!(fs::metadata(receipt_path(&plan).unwrap()).unwrap().len() < 8192);
    assert!(prepare(&plan, &AtomicBool::new(false)).unwrap());
    verify_retained(&plan, &fingerprint(&inventory).unwrap(), &cancel).unwrap();
    assert_eq!(
        fs::read(plan.target.join("one/saves/world/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(
        fs::read(plan.target.join("loose-file.txt")).unwrap(),
        b"keep root contents"
    );
    fs::write(plan.target.join("one/config/settings.json"), b"changed").unwrap();
    assert_eq!(
        fs::read(plan.source.join("one/config/settings.json")).unwrap(),
        b"config"
    );
    assert_eq!(
        verify_retained(&plan, &fingerprint(&inventory).unwrap(), &cancel).unwrap_err(),
        "DIRECTORY_MIGRATION_CHANGED"
    );
    assert!(plan.source.exists());
}
#[test]
fn directory_migration_interrupted_partial_retry_owns_only_exact_target() {
    let (_temp, plan) = fixture();
    prepare(&plan, &AtomicBool::new(false)).unwrap();
    fs::create_dir(&plan.target).unwrap();
    fs::write(plan.target.join("loose-file.txt"), b"keep root contents").unwrap();
    assert!(!prepare(&plan, &AtomicBool::new(false)).unwrap());
    assert!(!plan.target.exists());
    assert!(plan.source.exists());
    fs::create_dir(&plan.target).unwrap();
    fs::write(plan.target.join("foreign"), b"keep").unwrap();
    let mut wrong = plan.clone();
    wrong.source = plan.source.join("one");
    assert!(prepare(&wrong, &AtomicBool::new(false)).is_err());
    assert!(plan.target.join("foreign").exists());
    assert_eq!(
        prepare(&plan, &AtomicBool::new(false)).unwrap_err(),
        "DIRECTORY_MIGRATION_CHANGED"
    );
    assert!(plan.target.join("foreign").exists());
    fs::remove_file(plan.target.join("foreign")).unwrap();
    fs::write(plan.target.join("loose-file.txt"), b"user edited this copy").unwrap();
    assert_eq!(
        prepare(&plan, &AtomicBool::new(false)).unwrap_err(),
        "DIRECTORY_MIGRATION_CHANGED"
    );
    assert_eq!(
        fs::read(plan.target.join("loose-file.txt")).unwrap(),
        b"user edited this copy"
    );
}
#[test]
fn directory_migration_rejects_overlap_existing_and_changed_originals() {
    let (_temp, plan) = fixture();
    assert!(destination(&plan.source, &plan.source).is_err());
    assert!(destination(&plan.source, plan.source.parent().unwrap()).is_err());
    fs::create_dir(&plan.target).unwrap();
    fs::write(plan.target.join("foreign"), b"keep").unwrap();
    assert_eq!(
        prepare(&plan, &AtomicBool::new(false)).unwrap_err(),
        "DIRECTORY_MIGRATION_EXISTS"
    );
    assert!(plan.target.join("foreign").exists());
    let root = plan.source.ancestors().last().unwrap();
    assert!(destination(root, plan.target.parent().unwrap()).is_err());
}
#[test]
fn directory_migration_cancel_keeps_original_and_retries_complete_inventory() {
    let (_temp, plan) = fixture();
    prepare(&plan, &AtomicBool::new(false)).unwrap();
    let cancel = AtomicBool::new(false);
    let inventory = disk::inventory(&plan.source, &cancel).unwrap();
    assert!(
        disk::copy_verified(&plan.source, &plan.target, &inventory, &cancel, |_| cancel
            .store(true, Ordering::Release))
        .is_err()
    );
    assert!(!plan.target.exists());
    assert!(plan.source.join("one/saves/world/level.dat").exists());
    cancel.store(false, Ordering::Release);
    prepare(&plan, &AtomicBool::new(false)).unwrap();
    disk::copy_verified(&plan.source, &plan.target, &inventory, &cancel, |_| {}).unwrap();
    write_receipt(&plan, Some(inventory.clone())).unwrap();
    fs::write(plan.source.join("loose-file.txt"), b"new original edit").unwrap();
    assert_eq!(
        verify_retained(&plan, &fingerprint(&inventory).unwrap(), &cancel).unwrap_err(),
        "DIRECTORY_MIGRATION_CHANGED"
    );
}
#[test]
fn directory_migration_retargets_stable_ids_idempotently_and_rejects_new_source_refs() {
    let (temp, plan) = fixture();
    let original = vec![
        game("one", &plan.source.join("one")),
        game("external", &temp.path().join("external")),
    ];
    let updated = retarget(&original, &plan).unwrap();
    assert_eq!(updated[0].id, "one");
    assert_eq!(updated[0].directory, plan.target.join("one"));
    assert_eq!(updated[1].directory, original[1].directory);
    assert_eq!(
        retarget(&updated, &plan).unwrap()[0].directory,
        updated[0].directory
    );
    let mut changed = original.clone();
    changed.push(game("new", &plan.source.join("new")));
    assert!(retarget(&changed, &plan).is_err());
    assert_eq!(original[0].directory, plan.source.join("one"));
    changed = original.clone();
    changed[0].launch_state = "running".into();
    assert_eq!(
        entries(&changed, &plan.source).unwrap_err(),
        "DIRECTORY_MIGRATION_GAME_BUSY"
    );
}

#[test]
fn directory_migration_preference_write_gate_closes_check_write_race() {
    let preference = preference_change().unwrap();
    assert!(Guard::acquire().is_err());
    drop(preference);
    let migration = Guard::acquire().unwrap();
    assert!(preference_change().is_err());
    assert!(require_idle().is_err());
    drop(migration);
    assert!(preference_change().is_ok());
    assert!(require_idle().is_ok());
}

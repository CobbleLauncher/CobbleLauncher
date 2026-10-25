// SPDX-License-Identifier: MIT
use super::*;
fn owned(source: &Path) -> Ownership {
    Ownership {
        version: 1,
        source_id: "source".into(),
        target_id: "a".repeat(32),
        source_directory: dunce::canonicalize(source).unwrap(),
        name: "Copy".into(),
        include_worlds: false,
    }
}
#[test]
fn builtin_copy_uses_independent_bytes_and_excludes_worlds() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let base = temp.path().join("games");
    fs::create_dir_all(source.join("saves/world")).unwrap();
    fs::create_dir_all(source.join("config")).unwrap();
    fs::create_dir_all(&base).unwrap();
    fs::write(source.join("config/options.json"), b"original").unwrap();
    fs::write(source.join("saves/world/level.dat"), b"world").unwrap();
    let base = dunce::canonicalize(base).unwrap();
    let owned = owned(&source);
    let target = prepare(&base, &owned).unwrap();
    let cancel = AtomicBool::new(false);
    let inventory = disk::inventory_instance(&source, false, &cancel).unwrap();
    disk::copy_instance_verified(&source, &target, &inventory, false, &cancel, |_| {}).unwrap();
    assert!(!target.join("saves").exists());
    assert!(!target.join(RECEIPT).exists());
    receipt(&target, &owned).unwrap();
    assert_eq!(read_ownership(&target.join(RECEIPT)).unwrap(), owned);
    fs::write(target.join("config/options.json"), b"changed copy").unwrap();
    assert_eq!(
        fs::read(source.join("config/options.json")).unwrap(),
        b"original"
    );
    assert_eq!(
        fs::read(source.join("saves/world/level.dat")).unwrap(),
        b"world"
    );
}
#[test]
fn builtin_copy_cancel_and_interrupted_retry_clean_only_owned_target() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let base = temp.path().join("games");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&base).unwrap();
    fs::write(source.join("options.txt"), vec![7; 1024 * 1024]).unwrap();
    let base = dunce::canonicalize(base).unwrap();
    let owned = owned(&source);
    let target = prepare(&base, &owned).unwrap();
    let cancel = AtomicBool::new(false);
    let inventory = disk::inventory_instance(&source, false, &cancel).unwrap();
    assert!(
        disk::copy_instance_verified(&source, &target, &inventory, false, &cancel, |_| cancel
            .store(true, Ordering::Release))
        .is_err()
    );
    assert!(!target.exists());
    assert_eq!(
        fs::read(source.join("options.txt")).unwrap().len(),
        1024 * 1024
    );
    // A process interruption may leave a partial target and the host sidecar.
    fs::create_dir(&target).unwrap();
    fs::write(target.join("partial"), b"partial").unwrap();
    assert_eq!(prepare(&base, &owned).unwrap(), target);
    assert!(!target.exists());
    fs::create_dir(&target).unwrap();
    fs::write(target.join("foreign"), b"keep").unwrap();
    let mut other = owned.clone();
    other.source_id = "another".into();
    assert_eq!(
        remove_owned(&base, &other).unwrap_err(),
        "GAME_COPY_TARGET_UNOWNED"
    );
    assert_eq!(fs::read(target.join("foreign")).unwrap(), b"keep");
}
#[test]
fn builtin_copy_rejects_unowned_paths_and_overlapping_source_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let base = dunce::canonicalize(temp.path()).unwrap();
    let owned = owned(&source);
    let directory = target(&base, &owned.target_id).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("keep"), b"owned elsewhere").unwrap();
    assert_eq!(
        prepare(&base, &owned).unwrap_err(),
        "GAME_COPY_TARGET_UNOWNED"
    );
    assert!(target(&base, "../escape").is_err());
    assert!(valid("../game", "name").is_err());
    assert!(valid("game", "bad\nname").is_err());
    assert_eq!(
        fs::read(directory.join("keep")).unwrap(),
        b"owned elsewhere"
    );
    let mut nested = owned.clone();
    nested.target_id = "b".repeat(32);
    let partial = target(&base, &nested.target_id).unwrap();
    fs::create_dir_all(partial.join("source")).unwrap();
    fs::write(partial.join("source/keep"), b"source now inside target").unwrap();
    nested.source_directory = dunce::canonicalize(partial.join("source")).unwrap();
    fs::write(
        sidecar(&base, &nested.target_id),
        serde_json::to_vec(&nested).unwrap(),
    )
    .unwrap();
    assert_eq!(
        remove_owned(&base, &nested).unwrap_err(),
        "GAME_COPY_TARGET_UNOWNED"
    );
    assert_eq!(
        fs::read(partial.join("source/keep")).unwrap(),
        b"source now inside target"
    );
}

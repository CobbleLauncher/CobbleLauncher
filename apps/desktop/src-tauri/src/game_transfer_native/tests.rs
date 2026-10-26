// SPDX-License-Identifier: MIT
use super::*;

#[test]
fn incomplete_handoff_blocks_retry_and_only_its_target_can_be_discarded() {
    let root = std::env::temp_dir().join(format!(
        "cobble-transfer-native-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let id = "0123456789abcdef0123456789abcdef";
    let target = root.join(id);
    let source = root.join("source-game");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("level.dat"), b"source stays").unwrap();
    let marker = root.join(format!("{id}.incomplete.json"));
    let origin = root.join(format!("{id}.incomplete.origin.json"));
    write_marker_files(&marker, &origin, id, &target).unwrap();
    assert_eq!(
        ensure_retry_allowed_path(&marker).unwrap_err(),
        "TRANSFER_INCOMPLETE"
    );
    verify_discard_origin(&origin, id, &target).unwrap();
    assert!(verify_discard_origin(&origin, id, &source).is_err());
    assert!(verify_discard_origin(&origin, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &target).is_err());
    assert!(std::fs::remove_file(&marker).is_ok());
    assert!(
        ensure_retry_allowed_path(&marker).is_ok(),
        "runtime-only failure may retry"
    );
    assert!(
        origin.exists(),
        "a failed transfer remains identifiable for cleanup"
    );
    assert_eq!(
        std::fs::read(source.join("level.dat")).unwrap(),
        b"source stays"
    );
    std::fs::remove_dir_all(root).unwrap();
}

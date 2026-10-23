// SPDX-License-Identifier: MIT
use super::*;
fn manifest() -> PluginManifest {
    serde_json::from_str(include_str!(
        "../../../../examples/config-doctor/plugin.json"
    ))
    .unwrap()
}
#[test]
fn selected_folder_is_one_level_bounded_utf8_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let json = br#"{"secret":"original"}"#;
    std::fs::write(dir.path().join("settings.json"), json).unwrap();
    std::fs::write(dir.path().join("server.properties"), b"port=25565").unwrap();
    std::fs::write(dir.path().join("broken.yaml"), [255]).unwrap();
    std::fs::write(dir.path().join("ignored.txt"), b"ignored").unwrap();
    std::fs::write(
        dir.path().join("large.toml"),
        vec![b'a'; MAX_CONFIG_FOLDER_FILE_BYTES + 1],
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested/hidden.json"), b"{}").unwrap();
    let input = snapshot_selected(&manifest(), dir.path(), &AtomicBool::new(false)).unwrap();
    assert_eq!(input.snapshot.files.len(), 2);
    assert_eq!(input.snapshot.skipped, 4);
    assert!(input.snapshot.truncated);
    assert_eq!(
        std::fs::read(dir.path().join("settings.json")).unwrap(),
        json
    );
    assert!(
        !serde_json::to_string(&input.snapshot)
            .unwrap()
            .contains(&dir.path().display().to_string())
    );
    input.snapshot.validate(&manifest()).unwrap();
}
#[test]
fn selected_folder_count_and_escaped_envelope_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..65 {
        std::fs::write(dir.path().join(format!("{index:03}.json")), vec![0; 65536]).unwrap();
    }
    let input = snapshot_selected(&manifest(), dir.path(), &AtomicBool::new(false)).unwrap();
    assert!(input.snapshot.files.len() < 64);
    assert!(input.snapshot.truncated);
    assert_eq!(
        input.snapshot.files.len() as u32 + input.snapshot.skipped,
        65
    );
    assert!(serde_json::to_vec(&input.snapshot).unwrap().len() <= MAX_CONFIG_FOLDER_ENVELOPE_BYTES);
}
#[test]
fn permission_and_cancel_are_checked_before_folder_access() {
    let mut denied = manifest();
    denied.report_protocols.clear();
    assert_eq!(
        snapshot_selected(
            &denied,
            Path::new("missing-folder"),
            &AtomicBool::new(false)
        )
        .err()
        .unwrap(),
        "FOLDER_REPORT_PERMISSION_REQUIRED"
    );
    assert_eq!(
        snapshot_selected(
            &manifest(),
            Path::new("missing-folder"),
            &AtomicBool::new(true)
        )
        .err()
        .unwrap(),
        "CANCEL_REQUESTED"
    );
}
#[test]
fn stale_window_lease_cannot_revoke_replacement() {
    let window = "folder-test-owned-stale-lease".to_string();
    let old = Arc::new(AtomicBool::new(false));
    let lease = WindowLease::new(&window, old.clone()).unwrap();
    let current = Arc::new(AtomicBool::new(false));
    let replacement = WindowLease::new(&window, current.clone()).unwrap();
    assert!(old.load(Ordering::Acquire));
    drop(lease);
    revoke_window(&window);
    assert!(current.load(Ordering::Acquire));
    drop(replacement);
}
#[test]
#[ignore = "requires current Config Doctor component and private worker binary"]
fn actual_granted_folder_executes_config_doctor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let component = std::fs::read(root.join("target/config-doctor.component.wasm")).unwrap();
    let host = std::env::var_os("COBBLE_TEST_REPORT_HOST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target/debug/cobble-plugin-host.exe"));
    let mut plugin = manifest();
    plugin.component_sha256 = cobble_plugin_host::digest(&component);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("valid.json"), b"{}").unwrap();
    std::fs::write(dir.path().join("invalid.json"), br#"{"private-value": }"#).unwrap();
    let input = snapshot_selected(&plugin, dir.path(), &AtomicBool::new(false)).unwrap();
    let context =
        cobble_plugin_host::ReportHostContext::new(&plugin, "en", Default::default()).unwrap();
    let report = cobble_plugin_host::run_report_with_context(
        &host,
        &component,
        &serde_json::to_vec(&input.snapshot).unwrap(),
        CONFIG_FOLDER_MEDIA_TYPE,
        Arc::new(AtomicBool::new(false)),
        &context,
    )
    .unwrap();
    let result = serde_json::to_string(&report).unwrap();
    assert!(result.contains("invalid.json"));
    assert!(!result.contains("private-value"));
    assert_eq!(input.snapshot.files.len(), 2);
    assert_eq!(std::fs::read(dir.path().join("valid.json")).unwrap(), b"{}");
}

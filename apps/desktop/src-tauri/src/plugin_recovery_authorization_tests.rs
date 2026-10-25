// SPDX-License-Identifier: MIT
use super::*;
use std::sync::{Arc, atomic::AtomicBool};
#[test]
fn recovery_is_explicit_bound_to_the_game_and_revoked_with_its_view() {
    let mut manifest: cobble_plugin_api::PluginManifest = serde_json::from_str(include_str!(
        "../../../../examples/crash-doctor/plugin.json"
    ))
    .unwrap();
    manifest.permissions.as_mut().unwrap().retain(|node| {
        !matches!(
            node,
            PermissionNode::GameRecoveryRead {} | PermissionNode::GameRecoveryOpenRequest {}
        )
    });
    let mut auth = ViewAuthorization {
        manifest,
        instance_id: Some("isolated-game".into()),
        keybinding_target_id: None,
        handoff_id: None,
        config_restore_target: None,
        locale: "en".into(),
        theme: "light".into(),
        appearance: None,
        cancel: Arc::new(AtomicBool::new(false)),
        session_id: "synthetic-session".into(),
    };
    assert!(authorize(&auth, "isolated-game", false).is_err());
    auth.manifest
        .permissions
        .as_mut()
        .unwrap()
        .push(PermissionNode::GameRecoveryRead {});
    assert!(authorize(&auth, "isolated-game", false).is_ok());
    assert!(authorize(&auth, "isolated-game", true).is_err());
    auth.manifest
        .permissions
        .as_mut()
        .unwrap()
        .push(PermissionNode::GameRecoveryOpenRequest {});
    assert!(authorize(&auth, "isolated-game", true).is_ok());
    assert!(authorize(&auth, "other-game", false).is_err());
    auth.cancel.store(true, Ordering::Release);
    assert!(authorize(&auth, "isolated-game", false).is_err());
}
#[test]
fn recovery_public_runtime_has_no_settings_digest_and_payloads_are_bounded() {
    let summary = RuntimeSummary {
        java_mode: "custom".into(),
        selected_java_major: Some(21),
        memory_mb: 4096,
        jvm_argument_count: 2,
        settings_revision: "a".repeat(64),
    };
    assert!(summary.valid());
    let value = summary.public();
    assert_eq!(value["memoryMb"], 4096);
    assert!(value.get("settingsRevision").is_none());
    assert!(value.get("javaPath").is_none());
    assert!(value.get("jvmArgs").is_none());
    assert!(bounded(json!({"oversized":"x".repeat(1024*1024)})).is_err());
    assert!(serde_json::from_value::<RuntimeSummary>(json!({"javaMode":"custom","selectedJavaMajor":21,"memoryMb":4096,"jvmArgumentCount":2,"settingsRevision":"a".repeat(64),"executable":"/private"})).is_err());
}

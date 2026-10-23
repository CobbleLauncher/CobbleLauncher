// SPDX-License-Identifier: MIT
use cobble_plugin_api::{MessageComposerContext, MessageComposerRequest, PluginManifest};
use cobble_plugin_host::run_message_composer;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn manifest() -> PluginManifest {
    serde_json::from_value(serde_json::json!({"id":"test-composer","name":"Original composer fixture","version":"1.0.0","api_version":"0.1.0","publisher":"Original fixture","capabilities":["message-composer"],"file_extensions":[],"composer_permissions":["draft"],"component_sha256":"a".repeat(64)})).unwrap()
}
fn request() -> MessageComposerRequest {
    MessageComposerRequest {
        context: MessageComposerContext {
            locale: "en".into(),
            public_self: None,
            selected_peer: None,
            selected_session: None,
        },
        note: "Hello".into(),
    }
}
#[test]
fn composer_enforces_permissions_and_cancellation_before_spawn() {
    let host = Path::new("missing-composer-host");
    let mut request = request();
    assert_eq!(
        run_message_composer(
            host,
            &[],
            &manifest(),
            &request,
            Arc::new(AtomicBool::new(true))
        )
        .unwrap_err()
        .code,
        "CANCEL_REQUESTED"
    );
    request.context.public_self = Some(cobble_plugin_api::PublicIdentity {
        id: "public-id".into(),
        display_name: None,
    });
    assert_eq!(
        run_message_composer(
            host,
            &[],
            &manifest(),
            &request,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_PERMISSION_DENIED"
    );
}
#[test]
fn composer_rejects_module_in_owned_real_worker() {
    let error = run_message_composer(
        Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host")),
        b"\0asm\x01\0\0\0",
        &manifest(),
        &request(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_err();
    assert_eq!(error.code, "PLUGIN_INVALID_PACKAGE");
}

#[test]
#[ignore = "requires built target/session-card-package/component.wasm"]
fn real_session_card_component_produces_reviewable_draft() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let component = std::fs::read(root.join("target/session-card-package/component.wasm")).unwrap();
    let manifest: PluginManifest = serde_json::from_slice(
        &std::fs::read(root.join("examples/session-card/plugin.json")).unwrap(),
    )
    .unwrap();
    let mut request = request();
    request.context.selected_session = Some(cobble_plugin_api::SelectedSession {
        game_name: "Original test world".into(),
        game_version: "1.21.1".into(),
        loader: Some("Fabric".into()),
    });
    let draft = run_message_composer(
        Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host")),
        &component,
        &manifest,
        &request,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(
        draft.text,
        "Session: Original test world 1.21.1 (Fabric)\nHello"
    );
    request.context.selected_session = None;
    assert_eq!(
        run_message_composer(
            Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host")),
            &component,
            &manifest,
            &request,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_COMPOSE_FAILED"
    );
}

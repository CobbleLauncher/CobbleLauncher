// SPDX-License-Identifier: MIT
use cobble_plugin_api::{PluginManifest, ToolFormRequest};
use cobble_plugin_host::run_tool_form;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn manifest() -> PluginManifest {
    serde_json::from_value(serde_json::json!({"id":"test-tool","name":"Original tool fixture","version":"1.0.0","api_version":"0.1.0","publisher":"Original fixture","capabilities":["tool-form"],"file_extensions":[],"permissions":[],"component_sha256":"a".repeat(64),"tool_form":{"schema_version":1,"schema":{"type":"object","additionalProperties":false,"properties":{"count":{"type":"number","default":1,"minimum":1,"maximum":64}},"required":["count"]}}})).unwrap()
}
#[test]
fn tool_form_validates_before_spawn_and_cancels_owned_worker() {
    let mut request = ToolFormRequest {
        locale: "en".into(),
        input: serde_json::json!({"count":65}),
    };
    let host = Path::new("missing-tool-host");
    assert_eq!(
        run_tool_form(
            host,
            &[],
            &manifest(),
            &request,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_INVALID_TOOL_FORM"
    );
    request.input = serde_json::json!({"count":1});
    assert_eq!(
        run_tool_form(
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
}
#[test]
fn tool_form_rejects_modules_in_actual_worker() {
    let request = ToolFormRequest {
        locale: "en".into(),
        input: serde_json::json!({"count":1}),
    };
    assert_eq!(
        run_tool_form(
            Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host")),
            b"\0asm\x01\0\0\0",
            &manifest(),
            &request,
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_INVALID_PACKAGE"
    );
}

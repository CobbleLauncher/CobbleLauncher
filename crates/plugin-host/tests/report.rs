// SPDX-License-Identifier: MIT
use cobble_plugin_host::run_report;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn host() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host"))
}
fn run_inspector(
    host: &Path,
    component: &[u8],
    data: &[u8],
    media: &str,
    cancel: Arc<AtomicBool>,
) -> Result<cobble_plugin_api::Report, cobble_plugin_api::PluginError> {
    let manifest: cobble_plugin_api::PluginManifest = serde_json::from_str(include_str!(
        "../../../examples/modpack-inspector/plugin.json"
    ))
    .unwrap();
    let context = cobble_plugin_host::ReportHostContext::new(
        &manifest,
        "en",
        std::collections::BTreeMap::new(),
    )?;
    cobble_plugin_host::run_report_with_context(host, component, data, media, cancel, &context)
}
#[test]
fn report_checks_bounds_and_cancellation_before_spawn() {
    let absent = Path::new("missing-report-host");
    assert_eq!(
        run_report(
            absent,
            &[],
            &[],
            "text/plain",
            Arc::new(AtomicBool::new(true))
        )
        .unwrap_err()
        .code,
        "CANCEL_REQUESTED"
    );
    assert_eq!(
        run_report(
            absent,
            &[],
            &[],
            "other/type",
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_UNSUPPORTED"
    );
    assert_eq!(
        run_report(
            absent,
            &[],
            &vec![0; 1024 * 1024 + 1],
            "text/plain",
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_LIMIT"
    );
}
#[test]
fn report_rejects_raw_module_in_real_owned_host() {
    let error = run_report(
        host(),
        b"\0asm\x01\0\0\0",
        b"test",
        "text/plain",
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_err();
    assert_eq!(error.code, "PLUGIN_INVALID_PACKAGE");
}
#[test]
fn report_denies_ungranted_imports() {
    let error = run_report(
        host(),
        include_bytes!("fixtures/denied-import.wasm"),
        b"test",
        "text/plain",
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_err();
    assert_eq!(error.code, "PLUGIN_PERMISSION_DENIED");
}
#[test]
fn report_cancels_non_cooperative_owned_component() {
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let worker = std::thread::spawn(move || {
        run_report(
            host(),
            include_bytes!("fixtures/infinite-start.wasm"),
            b"test",
            "text/plain",
            signal,
        )
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    cancel.store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(worker.join().unwrap().unwrap_err().code, "CANCEL_REQUESTED");
}
#[test]
#[ignore = "requires built target/crash-doctor.component.wasm"]
fn real_report_component_analyzes_log() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/crash-doctor.component.wasm"),
    )
    .unwrap();
    let report = run_report(
        host(),
        &bytes,
        b"java.lang.OutOfMemoryError: Java heap space\n",
        "text/plain",
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(report.schema_version, 1);
    assert!(!report.findings.is_empty());
}

#[test]
#[ignore = "requires built target/modpack-inspector.component.wasm"]
fn real_report_component_analyzes_index() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/modpack-inspector.component.wasm"),
    )
    .unwrap();
    let data = br#"{"formatVersion":1,"game":"minecraft","versionId":"test","name":"Original fixture","files":[],"dependencies":{"minecraft":"1.21.1"}}"#;
    let report = run_inspector(
        host(),
        &bytes,
        data,
        "application/json",
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(report.schema_version, 1);
    assert!(!report.tables.is_empty());
}

#[test]
#[ignore = "requires built target/modpack-inspector.component.wasm"]
fn real_report_component_compares_pack_snapshots() {
    let component = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/modpack-inspector.component.wasm"),
    )
    .unwrap();
    let index = serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":"1","name":"Original fixture","files":[],"dependencies":{"minecraft":"1.21.1"}});
    let request = serde_json::json!({
        "current":{"index":index,"overrides":[{"path":"overrides/config/changed.txt","size":10,"crc32":2}]},
        "baseline":{"index":index,"overrides":[{"path":"overrides/config/changed.txt","size":9,"crc32":1},{"path":"overrides/config/removed.txt","size":3,"crc32":4}]}
    });
    let report = run_inspector(
        host(),
        &component,
        &serde_json::to_vec(&request).unwrap(),
        "application/vnd.cobble.mrpack-inspection+json",
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let text = serde_json::to_string(&report).unwrap();
    assert!(text.contains("changed.txt"));
    assert!(text.contains("removed.txt"));
    assert_eq!(report.schema_version, 1);
    assert!(report.tables.len() <= 4);
}

#[test]
#[ignore = "requires newly built context-aware target/modpack-inspector.component.wasm"]
fn real_context_guest_uses_typed_setting_and_locale() {
    use cobble_plugin_host::{ReportHostContext, run_report_with_context};
    let manifest: cobble_plugin_api::PluginManifest = serde_json::from_str(include_str!(
        "../../../examples/modpack-inspector/plugin.json"
    ))
    .unwrap();
    let component = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/modpack-inspector.component.wasm"),
    )
    .unwrap();
    let data = include_bytes!("../../../examples/modpack-inspector/fixtures/modrinth.index.json");
    let en = ReportHostContext::new(&manifest, "en", std::collections::BTreeMap::new()).unwrap();
    let english = run_report_with_context(
        host(),
        &component,
        data,
        "application/json",
        Arc::new(AtomicBool::new(false)),
        &en,
    )
    .unwrap();
    assert_eq!(english.tables.len(), 3);
    assert_eq!(english.tables[0].columns, ["Field", "Value"]);
    let zh = ReportHostContext::new(
        &manifest,
        "zh-CN",
        std::collections::BTreeMap::from([("show_dependencies".into(), serde_json::json!(false))]),
    )
    .unwrap();
    let chinese = run_report_with_context(
        host(),
        &component,
        data,
        "application/json",
        Arc::new(AtomicBool::new(false)),
        &zh,
    )
    .unwrap();
    assert_eq!(chinese.tables.len(), 2);
    assert_eq!(chinese.tables[0].columns, ["字段", "值"]);
    assert_eq!(chinese.tables[0].rows[2][1], english.tables[0].rows[2][1]);
    assert!(chinese.findings[0].message.contains("整合包"));
    // The legacy entrypoint provides no context import, even to a guest that asks.
    assert_eq!(
        run_report(
            host(),
            &component,
            data,
            "application/json",
            Arc::new(AtomicBool::new(false))
        )
        .unwrap_err()
        .code,
        "PLUGIN_PERMISSION_DENIED"
    );
}

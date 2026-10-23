// SPDX-License-Identifier: MIT
//! Explicit real-component tests; build the guest component before --ignored.
use cobble_plugin_api::{API_VERSION, PluginManifest};
use cobble_plugin_host::{PluginRegistry, PluginSession};
use fastnbt::Value;
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn fixture(width: i16, length: i16) -> Vec<u8> {
    let volume = width as usize * length as usize;
    let palette = Value::Compound(HashMap::from([("minecraft:stone".into(), Value::Int(0))]));
    let root = HashMap::from([
        ("Version", Value::Int(2)),
        ("Width", Value::Short(width)),
        ("Height", Value::Short(1)),
        ("Length", Value::Short(length)),
        ("Palette", palette),
        (
            "BlockData",
            Value::ByteArray(fastnbt::ByteArray::new(vec![0; volume])),
        ),
    ]);
    fastnbt::to_bytes(&root).unwrap()
}
fn component() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/schematic-viewer.component.wasm"),
    )
    .expect("Build and componentize guest before this ignored integration test")
}
fn host() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host"))
}

#[test]
#[ignore = "requires built target/schematic-viewer.component.wasm; actually runs Wasmtime child"]
fn real_component_pages_and_reaps_process() {
    let mut session = PluginSession::open(
        host(),
        &component(),
        &fixture(64, 64),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(session.summary().chunk_count, 4);
    assert_eq!(session.summary().materials[0].count, 4096);
    let pid = session.process_id();
    assert!(pid > 0);
    for index in 0..4 {
        let chunk = session
            .read_chunk(index, Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert_eq!(chunk.blocks.len(), 1024);
        assert_eq!(chunk.blocks[0].z, index * 16);
    }
    assert_eq!(
        session
            .read_chunk(4, Arc::new(AtomicBool::new(false)))
            .unwrap_err()
            .code,
        "VIEWER_INVALID_CHUNK"
    );
    session.close();
    assert!(session.has_exited().unwrap());
}

#[test]
#[ignore = "requires built component; actually compiles a guest with denied host import"]
fn denied_import_and_cancellation_are_real_process_results() {
    let bytes = include_bytes!("fixtures/denied-import.wasm").to_vec();
    let error = PluginSession::open(host(), &bytes, &[], Arc::new(AtomicBool::new(false)))
        .err()
        .unwrap();
    assert_eq!(error.code, "PLUGIN_PERMISSION_DENIED");
    let mut session = PluginSession::open(
        host(),
        &component(),
        &fixture(2, 2),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    let error = session.read_chunk(0, cancel).unwrap_err();
    assert_eq!(error.code, "CANCEL_REQUESTED");
    assert!(session.has_exited().unwrap());
    let loop_component = include_bytes!("fixtures/infinite-start.wasm").to_vec();
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        trigger.store(true, Ordering::Relaxed);
    });
    let error = PluginSession::open(host(), &loop_component, &[], cancel)
        .err()
        .unwrap();
    assert!(matches!(
        error.code.as_str(),
        "CANCEL_REQUESTED" | "PLUGIN_PERMISSION_DENIED"
    ));
}

#[test]
fn registry_lifecycle_verifies_digest_and_never_executes_on_install() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let bytes = b"\0asm\x0d\0\x01\0";
    let manifest = PluginManifest {
        frontend: None,
        permissions: None,
        description: None,
        tags: Vec::new(),
        icon: None,
        id: "synthetic-viewer".into(),
        name: "Synthetic".into(),
        version: "0.1.0".into(),
        api_version: API_VERSION.into(),
        publisher: "Original test fixture".into(),
        capabilities: vec!["viewer".into()],
        composer_permissions: Default::default(),
        interfaces: Default::default(),
        file_extensions: vec!["schem".into()],
        report_protocols: vec![],
        contributions: None,
        provider_origins: vec![],
        component_sha256: cobble_plugin_host::digest(bytes),
    };
    std::fs::write(
        source.join("plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(source.join("component.wasm"), bytes).unwrap();
    let registry = PluginRegistry::new(directory.path().join("plugins"));
    registry.install_directory(&source).unwrap();
    assert_eq!(registry.list().unwrap().len(), 1);
    registry.set_enabled("synthetic-viewer", false).unwrap();
    assert!(!registry.list().unwrap()[0].enabled);
    registry.set_enabled("synthetic-viewer", true).unwrap();
    let installed = directory
        .path()
        .join("plugins/synthetic-viewer/component.wasm");
    std::fs::write(&installed, b"damaged").unwrap();
    let listed = registry.list().unwrap();
    assert_eq!(
        listed.len(),
        1,
        "valid identity remains manageable after payload corruption"
    );
    assert_eq!(listed[0].problem.as_deref(), Some("PLUGIN_INVALID_PACKAGE"));
    std::fs::remove_file(installed).unwrap();
    assert!(registry.list().unwrap()[0].problem.is_some());
    registry.uninstall("synthetic-viewer").unwrap();
    assert!(registry.list().unwrap().is_empty());
    std::fs::write(source.join("component.wasm"), b"tampered").unwrap();
    assert!(registry.install_directory(&source).is_err());
    assert!(registry.uninstall("../source").is_err());
}

#[test]
fn forced_owner_child_entry() {
    let Some(signal) = std::env::var_os("COBBLE_PLUGIN_OWNER_SIGNAL") else {
        return;
    };
    let session = PluginSession::open(
        host(),
        &component(),
        &fixture(2, 2),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    std::fs::write(signal, session.process_id().to_string()).unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[cfg(windows)]
#[test]
#[ignore = "requires built component; kills only a test-owned parent to verify Windows job cleanup"]
fn forced_owner_exit_terminates_its_exact_plugin_worker() {
    use std::{
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct Owned(Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let signal = directory.path().join("owned-plugin-pid");
    let mut owner = Owned(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "forced_owner_child_entry", "--nocapture"])
            .env("COBBLE_PLUGIN_OWNER_SIGNAL", &signal)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !signal.exists() {
        assert!(
            Instant::now() < deadline,
            "test-owned host never became ready"
        );
        assert!(
            owner.0.try_wait().unwrap().is_none(),
            "test-owned parent exited early"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let worker: u32 = std::fs::read_to_string(signal).unwrap().parse().unwrap();
    let worker_exists = || {
        let output = Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &format!("Get-Process -Id {worker} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id")]).creation_flags(0x0800_0000).output().unwrap();
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .ok()
            == Some(worker)
    };
    assert!(
        worker_exists(),
        "the exact plugin worker must be alive before owner termination"
    );
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if !worker_exists() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the exact test-owned plugin worker survived owner termination"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

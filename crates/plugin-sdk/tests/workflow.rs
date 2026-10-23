// SPDX-License-Identifier: MIT
//! Filesystem/CLI regressions; real Rust/Wasm compilation remains a smoke check.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cobble-sdk-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn sdk(command: &str, args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cobble-plugin-sdk"))
        .arg(command)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn report_scaffold_is_standalone_and_preserves_existing_directory() {
    let scratch = Scratch::new();
    let project = scratch.0.join("report");
    let result = sdk("new-report", &[Path::new("123"), &project]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: cobble_plugin_api::PluginManifest =
        serde_json::from_slice(&fs::read(project.join("plugin.json")).unwrap()).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.api_version, cobble_plugin_api::REPORT_API_VERSION);
    assert_eq!(manifest.capabilities, ["report"]);
    assert_eq!(
        manifest.interfaces.provides[0].id,
        cobble_plugin_api::REPORT_INTERFACE
    );
    assert_eq!(manifest.file_extensions, ["txt"]);
    assert_eq!(
        fs::read_to_string(project.join("wit/plugin.wit")).unwrap(),
        include_str!("../../../contracts/report/report.wit")
    );
    let cargo = fs::read_to_string(project.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("cobble-plugin-123"));
    assert!(cargo.contains("wit-bindgen = \"=0.63.0\""));
    assert!(cargo.contains("[workspace]"));
    assert!(!cargo.contains("path ="));
    fs::write(project.join("src/lib.rs"), "user code").unwrap();
    assert!(
        !sdk("new-report", &[Path::new("123"), &project])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).unwrap(),
        "user code"
    );
    let invalid = scratch.0.join("invalid");
    assert!(
        !sdk("new-report", &[Path::new("../bad"), &invalid])
            .status
            .success()
    );
    assert!(!invalid.exists());
}

#[test]
fn catalog_check_accepts_empty_and_reports_mixed_compatibility() {
    let scratch = Scratch::new();
    let index = scratch.0.join("index.json");
    fs::write(
        &index,
        br#"{"schema_version":1,"name":"Empty catalog","plugins":[]}"#,
    )
    .unwrap();
    let result = sdk("catalog-check", &[&index]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("Empty catalog: 0 entries, 0 unsupported")
    );
    let manifest = serde_json::json!({
        "id":"current-viewer", "name":"Viewer", "version":"0.1.0", "api_version":cobble_plugin_api::API_VERSION,
        "publisher":"Fixture author", "capabilities":["viewer"], "file_extensions":["schem"], "component_sha256":"a".repeat(64)
    });
    let mut future = manifest.clone();
    future["id"] = "future-tool".into();
    future["api_version"] = "2.0.0".into();
    future["capabilities"] = serde_json::json!(["future-tool"]);
    let entries: Vec<_> = [manifest, future].into_iter().map(|manifest| serde_json::json!({
        "manifest":manifest,"description":"Original fixture","component_url":"https://example.invalid/component.wasm"
    })).collect();
    fs::write(
        &index,
        serde_json::to_vec(
            &serde_json::json!({"schema_version":1,"name":"Mixed catalog","plugins":entries}),
        )
        .unwrap(),
    )
    .unwrap();
    let result = sdk("catalog-check", &[&index, Path::new("--json")]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report,
        serde_json::json!({"name":"Mixed catalog","entries":2,"unsupported":1})
    );
}

#[test]
fn catalog_check_rejects_malformed_and_oversized_indexes() {
    let scratch = Scratch::new();
    let index = scratch.0.join("index.json");
    for bytes in [
        b"invalid JSON".to_vec(),
        br#"{"schema_version":99,"name":"Future schema","plugins":[]}"#.to_vec(),
        vec![b' '; cobble_plugin_host::catalog::MAX_CATALOG_BYTES + 1],
    ] {
        fs::write(&index, bytes).unwrap();
        let result = sdk("catalog-check", &[&index]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}

#[test]
fn scaffold_preserves_api_and_refuses_overwrite() {
    let scratch = Scratch::new();
    let project = scratch.0.join("project");
    let result = sdk("new", &[Path::new("123"), &project]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: cobble_plugin_api::PluginManifest =
        serde_json::from_slice(&fs::read(project.join("plugin.json")).unwrap()).unwrap();
    manifest.validate().unwrap();
    assert_eq!(manifest.id, "123");
    assert_eq!(manifest.api_version, cobble_plugin_api::API_VERSION);
    assert!(
        fs::read_to_string(project.join("Cargo.toml"))
            .unwrap()
            .contains("cobble-plugin-123")
    );
    assert_eq!(
        fs::read_to_string(project.join("wit/plugin.wit")).unwrap(),
        include_str!("../../../contracts/plugin.wit")
    );
    fs::write(project.join("src/lib.rs"), "developer changes").unwrap();
    assert!(!sdk("new", &[Path::new("123"), &project]).status.success());
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).unwrap(),
        "developer changes"
    );
}

#[test]
fn packing_core_module_fails_before_creating_output() {
    let scratch = Scratch::new();
    let project = scratch.0.join("project");
    assert!(
        sdk("new", &[Path::new("sample"), &project])
            .status
            .success()
    );
    let module = scratch.0.join("core.wasm");
    fs::write(&module, b"\0asm\x01\0\0\0").unwrap();
    let output = scratch.0.join("package");
    let result = sdk("pack", &[&module, &project.join("plugin.json"), &output]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("use build"));
    assert!(!output.exists());
}

#[test]
fn report_packages_check_and_reject_wrong_commands_inputs_and_size() {
    let scratch = Scratch::new();
    let component = scratch.0.join("component.wasm");
    // A raw component header suffices for packaging; execution is tested separately.
    fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
    let manifest = scratch.0.join("plugin.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "id":"report-fixture", "name":"Report", "version":"0.1.0",
            "api_version":cobble_plugin_api::REPORT_API_VERSION, "publisher":"Fixture author",
            "capabilities":["report"], "file_extensions":["log","txt","json"],
            "component_sha256":"0".repeat(64)
        }))
        .unwrap(),
    )
    .unwrap();
    let package = scratch.0.join("package");
    assert!(
        sdk("pack", &[&component, &manifest, &package])
            .status
            .success()
    );
    assert!(sdk("check", &[&package]).status.success());
    let missing = scratch.0.join("missing");
    let viewer = sdk("test", &[&package, &missing, &missing]);
    assert!(String::from_utf8_lossy(&viewer.stderr).contains("requires a viewer package"));
    let unsupported = sdk(
        "test-report",
        &[&package, &missing, &missing.with_extension("schem")],
    );
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("PLUGIN_UNSUPPORTED_INPUT"));
    let input = scratch.0.join("oversized.log");
    fs::File::create(&input)
        .unwrap()
        .set_len(cobble_plugin_api::MAX_FILE_BYTES as u64 + 1)
        .unwrap();
    let oversized = sdk("test-report", &[&package, &missing, &input]);
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("PLUGIN_INPUT_TOO_LARGE"));
    assert!(oversized.stdout.is_empty());
}

#[test]
#[ignore = "requires target/crash-doctor.component.wasm, target/modpack-inspector.component.wasm and built host; executes real Wasmtime"]
fn real_reports_emit_validated_structured_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let host = root.join(format!(
        "target/debug/cobble-plugin-host{}",
        std::env::consts::EXE_SUFFIX
    ));
    let scratch = Scratch::new();
    for (id, input, media) in [
        ("crash-doctor", "synthetic-crash.txt", "text/plain"),
        (
            "modpack-inspector",
            "modrinth.index.json",
            "application/json",
        ),
    ] {
        let package = scratch.0.join(id);
        let packed = sdk(
            "pack",
            &[
                &root.join(format!("target/{id}.component.wasm")),
                &root.join(format!("examples/{id}/plugin.json")),
                &package,
            ],
        );
        assert!(
            packed.status.success(),
            "{}",
            String::from_utf8_lossy(&packed.stderr)
        );
        let input = root.join(format!("examples/{id}/fixtures/{input}"));
        let output = sdk(
            "test-report",
            &[&package, &host, &input, Path::new("--json")],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["report_version"], 1);
        assert_eq!(report["plugin_id"], id);
        assert_eq!(
            report["api_version"],
            if id == "modpack-inspector" {
                cobble_plugin_api::REPORT_CONTEXT_API_VERSION
            } else {
                cobble_plugin_api::REPORT_API_VERSION
            }
        );
        assert_eq!(report["media_type"], media);
        assert_eq!(report["report"]["schema_version"], 1);
        if id == "crash-doctor" {
            assert!(report["report"]["findings"].as_array().unwrap().len() >= 2);
        } else {
            assert_eq!(report["report"]["tables"][0]["rows"][2][1], "2");
        }
        let plain = sdk("test-report", &[&package, &host, &input]);
        assert!(
            plain.status.success(),
            "{}",
            String::from_utf8_lossy(&plain.stderr)
        );
        assert!(String::from_utf8_lossy(&plain.stdout).contains("findings;"));
    }
}

#[test]
#[ignore = "requires existing target/schematic-viewer.component.wasm and target/debug/cobble-plugin-host; executes real Wasmtime"]
fn real_viewer_validates_four_pages_and_emits_json() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let host = root.join(format!(
        "target/debug/cobble-plugin-host{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(host.is_file(), "Build the plugin host before this test");
    let scratch = Scratch::new();
    let package = scratch.0.join("package");
    let packed = sdk(
        "pack",
        &[
            &root.join("target/schematic-viewer.component.wasm"),
            &root.join("examples/schematic-viewer/plugin.json"),
            &package,
        ],
    );
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    // Original synthetic uncompressed NBT: a 64 x 1 x 64 plane of stone.
    let mut data = vec![10, 0, 0];
    fn tag(data: &mut Vec<u8>, kind: u8, name: &str) {
        data.push(kind);
        data.extend_from_slice(&(name.len() as u16).to_be_bytes());
        data.extend_from_slice(name.as_bytes());
    }
    tag(&mut data, 3, "Version");
    data.extend_from_slice(&2i32.to_be_bytes());
    for (name, value) in [("Width", 64i16), ("Height", 1), ("Length", 64)] {
        tag(&mut data, 2, name);
        data.extend_from_slice(&value.to_be_bytes());
    }
    tag(&mut data, 10, "Palette");
    tag(&mut data, 3, "minecraft:stone");
    data.extend_from_slice(&0i32.to_be_bytes());
    data.push(0);
    tag(&mut data, 7, "BlockData");
    data.extend_from_slice(&4096i32.to_be_bytes());
    data.extend_from_slice(&[0; 4096]);
    data.push(0);
    let input = scratch.0.join("synthetic-plane.schem");
    fs::write(&input, data).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cobble-plugin-sdk"))
        .arg("test")
        .args([&package, &host, &input])
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["report_version"], 1);
    assert_eq!(report["api_version"], cobble_plugin_api::API_VERSION);
    assert_eq!(report["validated_chunks"], 4);
    assert_eq!(report["validated_blocks"], 4096);
    assert_eq!(report["scene"]["materials"][0]["count"], 4096);
    let plain = sdk("test", &[&package, &host, &input]);
    assert!(
        plain.status.success(),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );
    assert!(String::from_utf8_lossy(&plain.stdout).contains("4 chunks, 4096 blocks validated"));
}

#[test]
fn provider_scaffold_pins_standalone_wit_origin_and_offline_fixtures() {
    let scratch = Scratch::new();
    let project = scratch.0.join("provider");
    let args = [
        Path::new("independent-catalog"),
        project.as_path(),
        Path::new("https://catalog.example"),
    ];
    let result = sdk("new-provider", &args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: cobble_plugin_api::PluginManifest =
        serde_json::from_slice(&fs::read(project.join("plugin.json")).unwrap()).unwrap();
    manifest.validate().unwrap();
    assert_eq!(
        manifest.api_version,
        cobble_plugin_api::PROVIDER_API_VERSION
    );
    assert_eq!(manifest.capabilities, ["blueprint-provider"]);
    assert_eq!(manifest.interfaces.provides.len(), 1);
    assert_eq!(
        manifest.interfaces.optional,
        [cobble_plugin_api::InterfaceRequirement::viewer()]
    );
    assert!(manifest.interfaces.requires.is_empty());
    assert_eq!(
        manifest.interfaces.provides[0].id,
        cobble_plugin_api::BLUEPRINT_PROVIDER_INTERFACE
    );
    assert_eq!(
        manifest.interfaces.provides[0].version,
        cobble_plugin_api::PROVIDER_API_VERSION
    );
    assert_eq!(manifest.provider_origins, ["https://catalog.example"]);
    assert!(manifest.description.is_some());
    assert_eq!(manifest.tags, ["blueprints", "catalog"]);
    let icon = fs::read(project.join("icon.png")).unwrap();
    assert_eq!(
        manifest.icon.as_ref().unwrap().sha256,
        cobble_plugin_host::digest(&icon)
    );
    cobble_plugin_host::validate_icon_png(&icon, &manifest.icon.as_ref().unwrap().sha256).unwrap();
    let toolchain = fs::read_to_string(project.join("rust-toolchain.toml")).unwrap();
    assert!(toolchain.contains("1.99.0") && toolchain.contains("wasm32-unknown-unknown"));
    assert!(manifest.file_extensions.is_empty());
    assert_eq!(
        fs::read_to_string(project.join("wit/provider.wit")).unwrap(),
        include_str!("../../../contracts/provider/provider.wit")
    );
    let cargo = fs::read_to_string(project.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[workspace]"));
    assert!(!cargo.contains("workspace = true"));
    assert!(cargo.contains("wit-bindgen = \"=0.63.0\""));
    for operation in ["search", "detail", "download"] {
        let request: cobble_plugin_host::provider::ProviderRequest = serde_json::from_slice(
            &fs::read(project.join(format!("fixtures/{operation}.json"))).unwrap(),
        )
        .unwrap();
        request.validate().unwrap();
    }
    fs::write(project.join("src/lib.rs"), "user code").unwrap();
    assert!(!sdk("new-provider", &args).status.success());
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).unwrap(),
        "user code"
    );
    let invalid = scratch.0.join("invalid");
    assert!(
        !sdk(
            "new-provider",
            &[
                Path::new("catalog"),
                &invalid,
                Path::new("http://catalog.example")
            ]
        )
        .status
        .success()
    );
    assert!(!invalid.exists());
}

#[test]
fn interface_diagnostics_report_required_optional_and_installed_state() {
    let scratch = Scratch::new();
    let component = scratch.0.join("component.wasm");
    fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
    let manifest = scratch.0.join("plugin.json");
    let metadata = serde_json::json!({
        "id":"consumer", "name":"Consumer", "version":"0.1.0",
        "api_version":cobble_plugin_api::REPORT_API_VERSION, "publisher":"Fixture author",
        "capabilities":["report"], "file_extensions":["txt"], "component_sha256":"0".repeat(64),
        "interfaces":{"requires":[{"id":"launcher:viewer","version":"^1.0.0"}],"optional":[{"id":"launcher:blueprint-provider","version":"^0.2.0"}]}
    });
    fs::write(&manifest, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let package = scratch.0.join("consumer");
    assert!(
        sdk("pack", &[&component, &manifest, &package])
            .status
            .success()
    );
    let result = sdk("interfaces", &[&package, Path::new("--json")]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let missing: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(missing["ready"], false);
    assert_eq!(missing["dependencies"][0]["available"], false);
    assert_eq!(missing["dependencies"][1]["optional"], true);
    let registry_path = scratch.0.join("registry");
    let registry = cobble_plugin_host::PluginRegistry::new(registry_path.clone());
    registry.install_directory(&package).unwrap();
    let mut viewer = metadata;
    viewer["id"] = "viewer-peer".into();
    viewer["api_version"] = cobble_plugin_api::API_VERSION.into();
    viewer["capabilities"] = serde_json::json!(["viewer"]);
    viewer["file_extensions"] = serde_json::json!(["schem"]);
    viewer["interfaces"] = serde_json::json!({});
    fs::write(&manifest, serde_json::to_vec(&viewer).unwrap()).unwrap();
    let peer = scratch.0.join("peer");
    assert!(
        sdk("pack", &[&component, &manifest, &peer])
            .status
            .success()
    );
    registry.install_directory(&peer).unwrap();
    let result = sdk(
        "interfaces",
        &[
            Path::new("--installed"),
            Path::new("consumer"),
            Path::new("--registry"),
            &registry_path,
            Path::new("--json"),
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let available: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(available["ready"], true);
    assert_eq!(
        available["dependencies"][0]["resolution"]["implementations"][0]["version"],
        cobble_plugin_api::API_VERSION
    );
    assert_eq!(available["dependencies"][1]["available"], false);
    viewer["id"] = "second-viewer".into();
    fs::write(&manifest, serde_json::to_vec(&viewer).unwrap()).unwrap();
    let second = scratch.0.join("second");
    assert!(
        sdk("pack", &[&component, &manifest, &second])
            .status
            .success()
    );
    registry.install_directory(&second).unwrap();
    let result = sdk(
        "interfaces",
        &[
            &package,
            Path::new("--registry"),
            &registry_path,
            Path::new("--json"),
        ],
    );
    let ambiguous: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(ambiguous["dependencies"][0]["state"], "selection_required");
    assert_eq!(
        ambiguous["dependencies"][0]["code"],
        "INTERFACE_SELECTION_REQUIRED"
    );
    registry.set_enabled("second-viewer", false).unwrap();
    let mut mismatch: serde_json::Value =
        serde_json::from_slice(&fs::read(package.join("plugin.json")).unwrap()).unwrap();
    mismatch["interfaces"]["requires"][0]["version"] = "^2.0.0".into();
    fs::write(
        package.join("plugin.json"),
        serde_json::to_vec(&mismatch).unwrap(),
    )
    .unwrap();
    let result = sdk(
        "interfaces",
        &[
            &package,
            Path::new("--registry"),
            &registry_path,
            Path::new("--json"),
        ],
    );
    let incompatible: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        incompatible["dependencies"][0]["code"],
        "PLUGIN_MISSING_INTERFACE"
    );
    mismatch["interfaces"]["requires"][0]["version"] = "^1.0.0".into();
    fs::write(
        package.join("plugin.json"),
        serde_json::to_vec(&mismatch).unwrap(),
    )
    .unwrap();
    registry.set_enabled("viewer-peer", false).unwrap();
    let result = sdk(
        "interfaces",
        &[
            &package,
            Path::new("--registry"),
            &registry_path,
            Path::new("--json"),
        ],
    );
    let disabled: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(disabled["ready"], false);
    assert!(
        !sdk(
            "interfaces",
            &[
                Path::new("--installed"),
                Path::new("consumer"),
                Path::new("--json")
            ]
        )
        .status
        .success()
    );
}

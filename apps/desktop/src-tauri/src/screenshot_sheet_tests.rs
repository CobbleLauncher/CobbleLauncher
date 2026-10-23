// SPDX-License-Identifier: MIT
use super::*;
fn source(id: &str, width: u32, height: u32, color: [u8; 4]) -> Source {
    Source {
        descriptor: LayoutImage {
            id: id.into(),
            name: format!("{id}.png"),
            width: width as u16,
            height: height as u16,
            modified_at: None,
        },
        pixels: RgbaImage::from_pixel(width, height, image::Rgba(color)),
    }
}
fn request_for(sources: &[Source]) -> ImageLayoutRequest {
    ImageLayoutRequest {
        locale: "en".into(),
        images: sources.iter().map(|s| s.descriptor.clone()).collect(),
        columns: 2,
        ordering: ImageOrdering::Selected,
    }
}
fn sheet_for(sources: &[Source], columns: u8) -> ImageSheet {
    ImageSheet {
        columns,
        gap: 8,
        background: cobble_plugin_api::ImageRgb {
            red: 3,
            green: 5,
            blue: 7,
        },
        image_ids: sources.iter().map(|s| s.descriptor.id.clone()).collect(),
    }
}
fn manifest() -> PluginManifest {
    serde_json::from_str(include_str!(
        "../../../../examples/screenshot-sheet/plugin.json"
    ))
    .unwrap()
}
#[test]
fn png_render_preserves_aspect_and_bounds_all_selected_images() {
    let sources = vec![
        source("wide", 240, 120, [255, 0, 0, 255]),
        source("tall", 120, 240, [0, 255, 0, 255]),
    ];
    let request = request_for(&sources);
    let sheet = sheet_for(&sources, 2);
    let cancel = AtomicBool::new(false);
    let (bytes, width, height) = render::render(&sources, &request, &sheet, &cancel).unwrap();
    assert_eq!((width, height), (536, 272));
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(8, 8).0, [3, 5, 7, 255]);
    assert_eq!(image.get_pixel(8, 72).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(336, 8).0, [0, 255, 0, 255]);
    let sources: Vec<_> = (0..12)
        .map(|i| source(&format!("s{i}"), 256, 128, [10, 20, 30, 255]))
        .collect();
    let mut sheet = sheet_for(&sources, 1);
    sheet.gap = 32;
    let (bytes, width, height) =
        render::render(&sources, &request_for(&sources), &sheet, &cancel).unwrap();
    assert!(width <= 2048 && height <= 2048);
    assert_eq!(height, 2048);
    assert!(bytes.len() < MAX_PNG);
}
#[test]
fn render_rejects_guest_resource_substitution_and_cancellation() {
    let sources = vec![source("owned", 20, 10, [1, 2, 3, 255])];
    let request = request_for(&sources);
    let mut sheet = sheet_for(&sources, 1);
    sheet.image_ids[0] = "foreign".into();
    assert!(render::render(&sources, &request, &sheet, &AtomicBool::new(false)).is_err());
    sheet.image_ids[0] = "owned".into();
    assert_eq!(
        render::render(&sources, &request, &sheet, &AtomicBool::new(true)).unwrap_err(),
        "CANCEL_REQUESTED"
    );
}
#[test]
fn save_keeps_sources_unchanged_and_cancel_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.png");
    let source_bytes = b"owned original bytes";
    fs::write(&original, source_bytes).unwrap();
    let source = vec![source("owned", 20, 10, [1, 2, 3, 255])];
    let (bytes, width, height) = render::render(
        &source,
        &request_for(&source),
        &sheet_for(&source, 1),
        &AtomicBool::new(false),
    )
    .unwrap();
    let artifact = Artifact {
        id: "preview".into(),
        manifest: manifest(),
        game_id: "fixture".into(),
        bytes,
        width,
        height,
        sources: vec![original.clone()],
    };
    assert_eq!(
        save(&original, &artifact, &AtomicBool::new(false)).unwrap_err(),
        "OUTPUT_IS_SOURCE"
    );
    let output = directory.path().join("sheet.png");
    assert!(!save(&output, &artifact, &AtomicBool::new(true)).unwrap());
    assert!(!output.exists());
    assert!(save(&output, &artifact, &AtomicBool::new(false)).unwrap());
    assert_eq!(fs::read(&output).unwrap(), artifact.bytes);
    assert_eq!(fs::read(original).unwrap(), source_bytes);
}
#[test]
fn stale_request_cannot_cancel_new_window_work() {
    let window = format!("sheet-{}", next_pack_id().unwrap());
    let first = begin(&window, "request-a", "layout-a").unwrap();
    let second = begin(&window, "request-b", "layout-b").unwrap();
    assert!(first.load(Ordering::Acquire));
    revoke_matching(&window, Some("request-a"));
    assert!(!second.load(Ordering::Acquire));
    revoke_plugin("layout-b");
    assert!(second.load(Ordering::Acquire));
    assert!(!store().lock().unwrap().contains_key(&window));
}
#[test]
#[ignore = "requires the built original screenshot-sheet component and plugin-host executable"]
fn actual_guest_to_native_png_save_uses_only_selected_owned_images() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let host = root.join(if cfg!(windows) {
        "target/debug/cobble-plugin-host.exe"
    } else {
        "target/debug/cobble-plugin-host"
    });
    let component = fs::read(root.join("target/screenshot-sheet.component.wasm")).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let game = fixture.path().join("game");
    fs::create_dir_all(game.join("screenshots")).unwrap();
    for (name, width, height, color) in [
        ("wide.png", 60, 30, [255, 0, 0, 255]),
        ("tall.png", 30, 60, [0, 255, 0, 255]),
    ] {
        RgbaImage::from_pixel(width, height, image::Rgba(color))
            .save(game.join("screenshots").join(name))
            .unwrap();
    }
    let listed = super::super::screenshots::register_sheet_fixture(&game).unwrap();
    let ids = listed.iter().map(|s| s.id.clone()).collect();
    let originals: Vec<_> = listed
        .iter()
        .map(|s| fs::read(game.join("screenshots").join(&s.name)).unwrap())
        .collect();
    let package = fixture.path().join("package");
    fs::create_dir(&package).unwrap();
    let mut manifest = manifest();
    manifest.id = "independent-image-layout".into();
    manifest.component_sha256 = cobble_plugin_host::digest(&component);
    fs::write(
        package.join("plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(package.join("component.wasm"), component).unwrap();
    let plugins = fixture.path().join("plugins");
    let registry = PluginRegistry::new(plugins.clone());
    registry.install_directory(&package).unwrap();
    let artifact = compose_owned(
        &plugins,
        &host,
        ComposeJob {
            game: game.clone(),
            game_id: "fixture".into(),
            screenshot_ids: ids,
            plugin_id: manifest.id.clone(),
            locale: "en".into(),
            columns: 2,
            ordering: ImageOrdering::Selected,
        },
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let decoded = image::load_from_memory(&artifact.bytes).unwrap().to_rgba8();
    assert!(decoded.pixels().any(|p| p.0 == [255, 0, 0, 255]));
    assert!(decoded.pixels().any(|p| p.0 == [0, 255, 0, 255]));
    assert!(artifact.width <= 2048 && artifact.height <= 2048);
    let output = fixture.path().join("contact-sheet.png");
    assert!(save(&output, &artifact, &AtomicBool::new(false)).unwrap());
    assert_eq!(fs::read(output).unwrap(), artifact.bytes);
    for (screenshot, before) in listed.iter().zip(originals) {
        assert_eq!(
            fs::read(game.join("screenshots").join(&screenshot.name)).unwrap(),
            before
        );
    }
    registry.set_enabled(&manifest.id, false).unwrap();
    assert!(selected(&registry, &manifest.id).is_err());
}

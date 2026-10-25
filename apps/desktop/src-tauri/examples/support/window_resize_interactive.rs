// SPDX-License-Identifier: MIT
//! Explicit standalone UI probe: no production setup, business IPC or user profile.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn run(scale: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(scale, "1" | "1.25") {
        return Err("resize probe supports scale 1 or 1.25".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()?;
    let parent = root.join("target/window-geometry-probe");
    std::fs::create_dir_all(&parent)?;
    let directory = tempfile::Builder::new()
        .prefix("resize-")
        .tempdir_in(parent)?
        .keep();
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = "cobble-launcher-resize-probe".into();
    context.config_mut().app.windows.clear();
    let url = format!(
        "http://127.0.0.1:1423/e2e/fixtures/window-interaction.html?native=1&scale={scale}"
    );
    let stopped = Arc::new(AtomicBool::new(false));
    let running = stopped.clone();
    let setup_directory = directory.clone();
    let app = tauri::Builder::default()
        .setup(move |app| {
            // Remote access exists only in this standalone debug example and
            // grants window operations to the fixed local fixture origin.
            app.add_capability(
                serde_json::to_string(&json!({
                    "identifier":"resize-probe-window-only","windows":["main"],
                    "local":false,"remote":{"urls":["http://127.0.0.1:1423"]},
                    "permissions":[
                        "core:event:allow-listen","core:event:allow-unlisten",
                        "core:window:allow-is-maximized","core:window:allow-is-fullscreen",
                        "core:window:allow-is-resizable","core:window:allow-start-resize-dragging",
                        "core:window:allow-start-dragging","core:window:allow-toggle-maximize",
                        "core:window:allow-set-fullscreen","core:window:allow-set-resizable",
                        "core:window:allow-close"
                    ]
                }))?
                .as_str(),
            )?;
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(url.parse()?),
            )
            .title("Cobble isolated resize probe")
            .decorations(false)
            .resizable(true)
            .inner_size(900.0, 620.0)
            .min_inner_size(560.0, 420.0)
            .center()
            .data_directory(setup_directory.join("webview-data"))
            .build()?;
            let handle = app.handle().clone();
            let trace_path = setup_directory.join("bounds.json");
            std::thread::spawn(move || {
                let started = Instant::now();
                let mut trace = Vec::new();
                let mut previous = Value::Null;
                while !running.load(Ordering::Acquire) {
                    if started.elapsed() > Duration::from_secs(600) {
                        handle.exit(2);
                        break;
                    }
                    if let Some(window) = handle.get_webview_window("main")
                        && let Ok(mut value) = snapshot(&window)
                    {
                        value["fullscreen"] = json!(window.is_fullscreen().unwrap_or(false));
                        value["resizable"] = json!(window.is_resizable().unwrap_or(false));
                        if value != previous && trace.len() < 1024 {
                            previous = value.clone();
                            trace.push(
                                json!({"elapsed_ms":started.elapsed().as_millis(),"bounds":value}),
                            );
                            let _ = std::fs::write(
                                &trace_path,
                                serde_json::to_vec_pretty(&trace).unwrap(),
                            );
                        }
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            });
            Ok(())
        })
        .build(context)?;
    println!(
        "RESIZE_PROBE {}",
        json!({"owned_pid":std::process::id(),"directory":directory,"appearance_scale":scale,"business_ipc":false,"explicit_webview_data_directory":true})
    );
    app.run(move |_, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            stopped.store(true, Ordering::Release);
        }
    });
    Ok(())
}

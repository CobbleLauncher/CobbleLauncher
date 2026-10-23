// SPDX-License-Identifier: MIT
//! Standalone native geometry regression. No account/game state or single-instance plugin.
#![allow(dead_code)]
#[path = "../src/window_geometry.rs"]
mod window_geometry;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tauri::Manager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child") {
        return child(args[2].parse()?, PathBuf::from(&args[3]));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()?;
    let scratch_parent = root.join("target/window-geometry-probe");
    std::fs::create_dir_all(&scratch_parent)?;
    let scratch = tempfile::tempdir_in(scratch_parent)?;
    let mut stages = Vec::new();
    for stage in 1..=4 {
        if stage == 4 {
            let path = scratch.path().join("launcher-window.json");
            let mut state: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            state["x"] = json!(i32::MAX);
            state["y"] = json!(i32::MIN);
            state["monitor"] = json!("disconnected-probe-monitor");
            state["maximized"] = json!(false);
            std::fs::write(path, serde_json::to_vec(&state)?)?;
        }
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["--child", &stage.to_string()])
            .arg(scratch.path())
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut process = command.spawn()?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = process.try_wait()? {
                break status;
            }
            if started.elapsed() > Duration::from_secs(20) {
                // This Child handle owns only the process started for this stage.
                process.kill()?;
                process.wait()?;
                return Err(format!("geometry probe stage {stage} timed out").into());
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let receipt: Value = serde_json::from_slice(&std::fs::read(
            scratch.path().join(format!("stage-{stage}.json")),
        )?)?;
        let passed = receipt["passed"] == true;
        stages.push(receipt);
        if !status.success() || !passed {
            return Err(format!("geometry probe stage {stage} failed: {stages:?}").into());
        }
    }
    let output = root.join("validation/implementation/window-geometry-native-2026-10-23.json");
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(
            &json!({"passed":true,"platform":std::env::consts::OS,"stages":stages,"owned_processes_exited":true}),
        )?,
    )?;
    println!("Geometry probe passed: {}", output.display());
    Ok(())
}

fn snapshot(window: &tauri::WebviewWindow) -> Result<Value, Box<dyn std::error::Error>> {
    let size = window.inner_size()?;
    let position = window.outer_position()?;
    let scale = window.scale_factor()?;
    let monitor = window.current_monitor()?.ok_or("no current monitor")?;
    let work = monitor.work_area();
    Ok(
        json!({"width": f64::from(size.width)/scale,"height": f64::from(size.height)/scale,
        "x":position.x,"y":position.y,"physical_width":size.width,"physical_height":size.height,
        "scale":scale,"maximized":window.is_maximized()?,"visible":window.is_visible()?,
        "work_x":work.position.x,"work_y":work.position.y,"work_width":work.size.width,"work_height":work.size.height}),
    )
}
fn in_work_area(value: &Value) -> bool {
    let n = |name: &str| value[name].as_i64().unwrap_or(0);
    n("x") >= n("work_x")
        && n("y") >= n("work_y")
        && n("x") + n("physical_width") <= n("work_x") + n("work_width")
        && n("y") + n("physical_height") <= n("work_y") + n("work_height")
}
fn near(a: &Value, b: &Value, key: &str) -> bool {
    (a[key].as_f64().unwrap_or(0.0) - b[key].as_f64().unwrap_or(0.0)).abs() < 2.0
}
fn drive(
    stage: u32,
    directory: &Path,
    app: &tauri::AppHandle,
) -> Result<Value, Box<dyn std::error::Error>> {
    let pause = || std::thread::sleep(Duration::from_millis(700));
    pause();
    let window = app
        .get_webview_window("main")
        .ok_or("main window missing")?;
    let initial = snapshot(&window)?;
    if !initial["visible"].as_bool().unwrap_or(false) {
        return Err("window remained hidden".into());
    }
    match stage {
        1 => {
            if !in_work_area(&initial) {
                return Err("initial window outside work area".into());
            }
            window.set_size(tauri::LogicalSize::new(920.0, 640.0))?;
            pause();
            let normal = snapshot(&window)?;
            std::fs::write(
                directory.join("expected.json"),
                serde_json::to_vec(&normal)?,
            )?;
        }
        2 => {
            let normal: Value =
                serde_json::from_slice(&std::fs::read(directory.join("expected.json"))?)?;
            if !near(&initial, &normal, "width") || !near(&initial, &normal, "height") {
                return Err(format!(
                    "normal size was not restored; actual={initial}; expected={normal}; saved={}",
                    std::fs::read_to_string(directory.join("launcher-window.json"))?
                )
                .into());
            }
            window.maximize()?;
            pause();
            if !window.is_maximized()? {
                return Err("native maximize failed".into());
            }
        }
        3 => {
            if !window.is_maximized()? {
                return Err("maximized state was not restored".into());
            }
            window.unmaximize()?;
            pause();
            let restored = snapshot(&window)?;
            let normal: Value =
                serde_json::from_slice(&std::fs::read(directory.join("expected.json"))?)?;
            if !near(&restored, &normal, "width") || !near(&restored, &normal, "height") {
                return Err(format!(
                    "maximize overwrote normal bounds; actual={restored}; expected={normal}"
                )
                .into());
            }
        }
        4 => {
            if !in_work_area(&initial) {
                return Err("offscreen window was not recovered".into());
            }
        }
        _ => return Err("invalid stage".into()),
    }
    let final_state = snapshot(&window)?;

    Ok(json!({"stage":stage,"passed":true,"initial":initial,"final":final_state}))
}
fn child(stage: u32, directory: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = "cobble-launcher-geometry-probe".into();
    context.config_mut().app.windows.clear();

    let app = tauri::Builder::default()
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External("about:blank".parse()?),
            )
            .title("Cobble window geometry probe")
            .visible(false)
            .decorations(false)
            .inner_size(1100.0, 760.0)
            .data_directory(directory.join(format!("webview-{stage}")))
            .build()?;
            window_geometry::install_at(app, directory.join("launcher-window.json"))?;
            let handle = app.handle().clone();
            let receipt_directory = directory.clone();
            std::thread::spawn(move || {
                let result = drive(stage, &receipt_directory, &handle);
                let (receipt, code) = match result {
                    Ok(receipt) => (receipt, 0),
                    Err(error) => (
                        json!({"stage":stage,"passed":false,"error":error.to_string()}),
                        1,
                    ),
                };
                let _ = std::fs::write(
                    receipt_directory.join(format!("stage-{stage}.json")),
                    serde_json::to_vec_pretty(&receipt).unwrap(),
                );
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.close();
                }
                window_geometry::flush(&handle);
                handle.exit(code);
            });
            Ok(())
        })
        .build(context)?;
    app.run(move |app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            window_geometry::flush(app);
        }
    });
    Ok(())
}

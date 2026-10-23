// SPDX-License-Identifier: MIT
//! Native-only launcher window geometry; independent of Minecraft window settings.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Bounds {
    width: f64,
    height: f64,
    x: i32,
    y: i32,
    monitor: Option<String>,
    maximized: bool,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            width: 1100.0,
            height: 760.0,
            x: 0,
            y: 0,
            monitor: None,
            maximized: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

fn fit(bounds: &Bounds, area: Area, center: bool) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let scale = if area.scale.is_finite() && area.scale > 0.0 {
        area.scale
    } else {
        1.0
    };
    let dimension = |value: f64, fallback: f64, minimum: f64, maximum: u32| {
        let logical = if value.is_finite() && value > 0.0 {
            value
        } else {
            fallback
        };
        (logical.max(minimum) * scale)
            .round()
            .min(f64::from(maximum.max(1))) as u32
    };
    let width = dimension(bounds.width, 1100.0, 560.0, area.width);
    let height = dimension(bounds.height, 760.0, 420.0, area.height);
    let position = |saved: i32, origin: i32, space: u32, extent: u32| {
        let remaining = i64::from(space.saturating_sub(extent));
        let low = i64::from(origin);
        let value = if center {
            low + remaining / 2
        } else {
            i64::from(saved).clamp(low, low + remaining)
        };
        value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    (
        PhysicalPosition::new(
            position(bounds.x, area.x, area.width, width),
            position(bounds.y, area.y, area.height, height),
        ),
        PhysicalSize::new(width, height),
    )
}

fn read(path: &std::path::Path) -> Option<Bounds> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(8193)
        .read_to_end(&mut data)
        .ok()?;
    if data.len() > 8192 {
        return None;
    }
    serde_json::from_slice(&data).ok()
}

fn write(path: &std::path::Path, bounds: &Bounds) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("window state has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut file, bounds)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

enum Message {
    Changed(Bounds),
    Flush(mpsc::Sender<()>),
}
pub struct Persistence(mpsc::Sender<Message>);

fn worker(path: PathBuf, receiver: mpsc::Receiver<Message>) {
    let mut pending = None;
    loop {
        match receiver.recv_timeout(Duration::from_millis(350)) {
            Ok(Message::Changed(bounds)) => pending = Some(bounds),
            Ok(Message::Flush(done)) => {
                if let Some(bounds) = pending.take() {
                    save(&path, &bounds);
                }
                let _ = done.send(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(bounds) = pending.take() {
                    save(&path, &bounds);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(bounds) = pending {
                    save(&path, &bounds);
                }
                break;
            }
        }
    }
}

fn save(path: &std::path::Path, bounds: &Bounds) {
    if let Err(error) = write(path, bounds) {
        tracing::warn!(%error, "could not save launcher window geometry");
    }
}

pub fn flush(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Persistence>() {
        let (sender, receiver) = mpsc::channel();
        if state.0.send(Message::Flush(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(2));
        }
    }
}

fn capture(window: &WebviewWindow, bounds: &mut Bounds) {
    // Minimized native sizes/positions are not restorable normal bounds.
    if window.is_minimized().unwrap_or(true) || window.is_fullscreen().unwrap_or(true) {
        return;
    }
    let Ok(maximized) = window.is_maximized() else {
        return;
    };
    bounds.maximized = maximized;
    if maximized {
        return;
    }
    if let (Ok(size), Ok(position), Ok(scale)) = (
        window.inner_size(),
        window.outer_position(),
        window.scale_factor(),
    ) {
        if size.width == 0 || size.height == 0 || scale <= 0.0 {
            return;
        }
        bounds.width = f64::from(size.width) / scale;
        bounds.height = f64::from(size.height) / scale;
        bounds.x = position.x;
        bounds.y = position.y;
        bounds.monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .and_then(|monitor| monitor.name().cloned());
    }
}

pub fn install(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    match app.path().app_local_data_dir() {
        Ok(directory) => install_at(app, directory.join("launcher-window.json")),
        Err(error) => {
            tracing::warn!(%error, "could not locate launcher window state");
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        }
    }
}
pub fn install_at(app: &tauri::App, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let result = restore(app, &window, path);
    // A filesystem/monitor failure must never strand the launcher invisibly.
    let shown = window.show();

    shown?;
    match result {
        // Show normal bounds first: Win32 must update WINDOWPLACEMENT before
        // maximizing or restore uses the original hidden creation rectangle.
        Ok(true) => window.maximize()?,
        Ok(false) => {}
        Err(error) => tracing::warn!(%error, "could not restore launcher window geometry"),
    }
    Ok(())
}

fn restore(
    app: &tauri::App,
    window: &WebviewWindow,
    path: PathBuf,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Tao's Windows shadow sizing uses the current non-client insets. Refresh
    // the hidden frameless window first so its initial caption metrics cannot
    // be added to the requested client size (observed as +30 logical pixels).
    #[cfg(target_os = "windows")]
    {
        window.set_shadow(false)?;
        window.set_shadow(true)?;
    }
    let saved = read(&path);
    let mut bounds = saved.clone().unwrap_or_default();
    let monitors = window.available_monitors()?;
    let monitor = monitors
        .iter()
        .find(|monitor| {
            bounds
                .monitor
                .as_ref()
                .is_some_and(|name| monitor.name() == Some(name))
        })
        .or_else(|| {
            monitors.iter().find(|monitor| {
                let area = monitor.work_area();
                i64::from(bounds.x) >= i64::from(area.position.x)
                    && i64::from(bounds.y) >= i64::from(area.position.y)
                    && i64::from(bounds.x) < i64::from(area.position.x) + i64::from(area.size.width)
                    && i64::from(bounds.y)
                        < i64::from(area.position.y) + i64::from(area.size.height)
            })
        });
    let current = window.current_monitor()?.or(window.primary_monitor()?);
    let selected = monitor.or(current.as_ref()).or(monitors.first());
    if let Some(monitor) = selected {
        let work = monitor.work_area();
        let area = Area {
            x: work.position.x,
            y: work.position.y,
            width: work.size.width,
            height: work.size.height,
            scale: monitor.scale_factor(),
        };
        let (position, size) = fit(
            &bounds,
            area,
            saved.is_none() || monitor.name() != bounds.monitor.as_ref(),
        );
        window.set_min_size(Some(PhysicalSize::new(
            (560.0 * area.scale).round().min(f64::from(area.width)) as u32,
            (420.0 * area.scale).round().min(f64::from(area.height)) as u32,
        )))?;
        window.set_size(size)?;
        window.set_position(position)?;
        bounds.x = position.x;
        bounds.y = position.y;
        bounds.width = f64::from(size.width) / area.scale;
        bounds.height = f64::from(size.height) / area.scale;
        bounds.monitor = monitor.name().cloned();
    }
    let maximized = bounds.maximized;
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("window-geometry".into())
        .spawn(move || worker(path, receiver))?;
    app.manage(Persistence(sender.clone()));
    let snapshot = Arc::new(Mutex::new(bounds));
    let observed = window.clone();
    let handle = app.handle().clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
                | tauri::WindowEvent::CloseRequested { .. }
        ) {
            if let Ok(mut bounds) = snapshot.lock() {
                capture(&observed, &mut bounds);
                let _ = sender.send(Message::Changed(bounds.clone()));
            }
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                flush(&handle);
            }
        }
    });
    Ok(maximized)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn area() -> Area {
        Area {
            x: -1920,
            y: 40,
            width: 1920,
            height: 1040,
            scale: 1.0,
        }
    }
    #[test]
    fn first_launch_centers_usable_size() {
        let (position, size) = fit(&Bounds::default(), area(), true);
        assert_eq!(size, PhysicalSize::new(1100, 760));
        assert_eq!(position, PhysicalPosition::new(-1510, 180));
    }
    #[test]
    fn dpi_and_small_work_area_are_clamped() {
        let (position, size) = fit(
            &Bounds::default(),
            Area {
                scale: 2.0,
                width: 1280,
                height: 720,
                ..area()
            },
            true,
        );
        assert_eq!(size, PhysicalSize::new(1280, 720));
        assert_eq!(position, PhysicalPosition::new(-1920, 40));
    }
    #[test]
    fn disconnected_monitor_position_is_recovered() {
        let bounds = Bounds {
            x: 8000,
            y: -4000,
            width: 900.0,
            height: 650.0,
            ..Bounds::default()
        };
        let (position, size) = fit(&bounds, area(), false);
        assert_eq!(position, PhysicalPosition::new(-900, 40));
        assert_eq!(size, PhysicalSize::new(900, 650));
    }
    #[test]
    fn malformed_and_oversized_files_fall_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("window.json");
        std::fs::write(&path, b"invalid").unwrap();
        assert!(read(&path).is_none());
        std::fs::write(&path, vec![b' '; 8193]).unwrap();
        assert!(read(&path).is_none());
    }
    #[test]
    fn persistence_replaces_previous_state_and_keeps_maximized() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("window.json");
        write(&path, &Bounds::default()).unwrap();
        write(
            &path,
            &Bounds {
                maximized: true,
                width: 980.0,
                ..Bounds::default()
            },
        )
        .unwrap();
        let restored = read(&path).unwrap();
        assert!(restored.maximized);
        assert_eq!(restored.width, 980.0);
    }
}

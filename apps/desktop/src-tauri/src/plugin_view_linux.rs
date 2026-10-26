// SPDX-License-Identifier: MIT
//! WebKitGTK child WebView configuration for plugin-only frontend isolation.
use super::ViewBounds;
use gtk::{
    OverlaySignals, gdk,
    glib::{Cast, ObjectExt, ObjectType},
    prelude::{BoxExt, ContainerExt, OverlayExt, WidgetExt},
};
use std::{cell::RefCell, collections::HashMap};
use webkit2gtk::{
    NetworkProxyMode, NetworkProxySettings, SettingsExt, WebViewExt, WebsiteDataManagerExt,
};

const OVERLAY_NAME: &str = "cobble-plugin-view-overlay";

thread_local! {
    // GTK and WebKit widgets are owned by the UI thread. Keep exact overlay
    // allocations there as well, without placing non-Send GObjects in globals.
    static VIEW_BOUNDS: RefCell<HashMap<usize, ViewBounds>> = RefCell::new(HashMap::new());
}

fn widget_key(widget: &gtk::Widget) -> usize {
    widget.as_ptr() as usize
}

fn remember_bounds(view: &webkit2gtk::WebView, bounds: ViewBounds) {
    let widget = view.upcast_ref::<gtk::Widget>();
    let key = widget_key(widget);
    let inserted = VIEW_BOUNDS.with(|entries| entries.borrow_mut().insert(key, bounds).is_none());
    if inserted {
        view.connect_destroy(move |_| {
            VIEW_BOUNDS.with(|entries| {
                entries.borrow_mut().remove(&key);
            });
        });
    }
}

fn requested_allocation(widget: &gtk::Widget) -> Option<gdk::Rectangle> {
    VIEW_BOUNDS.with(|entries| {
        entries.borrow().get(&widget_key(widget)).map(|bounds| {
            gdk::Rectangle::new(
                bounds.x.round() as i32,
                bounds.y.round() as i32,
                bounds.width.round() as i32,
                bounds.height.round() as i32,
            )
        })
    })
}

// Tauri 2.12/Wry 0.57 packs Linux child views into the window's vertical box.
// Keep that box and its other widgets, wrapping only its main browser once.
fn attach(view: &webkit2gtk::WebView, bounds: ViewBounds) -> Result<(), String> {
    let container = view
        .parent()
        .and_then(|widget| widget.downcast::<gtk::Box>().ok())
        .ok_or("PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?;
    let children = container.children();
    let existing = children.iter().find_map(|widget| {
        widget
            .clone()
            .downcast::<gtk::Overlay>()
            .ok()
            .filter(|overlay| overlay.widget_name() == OVERLAY_NAME)
    });
    let overlay = if let Some(overlay) = existing {
        container.remove(view);
        overlay
    } else {
        let main = children
            .into_iter()
            .filter(|widget| widget != view)
            .filter(|widget| widget.is::<webkit2gtk::WebView>())
            .collect::<Vec<_>>();
        if main.len() != 1 {
            return Err("PLUGIN_VIEW_RUNTIME_UNSUPPORTED".into());
        }
        let overlay = gtk::Overlay::new();
        overlay.set_widget_name(OVERLAY_NAME);
        container.remove(view);
        container.remove(&main[0]);
        overlay.add(&main[0]);
        container.pack_start(&overlay, true, true, 0);
        overlay.connect_get_child_position(|_, child| requested_allocation(child));
        overlay.show();
        overlay
    };
    remember_bounds(view, bounds);
    overlay.add_overlay(view);
    resize_view(view, bounds);
    Ok(())
}

fn resize_view(view: &webkit2gtk::WebView, bounds: ViewBounds) {
    remember_bounds(view, bounds);
    view.set_halign(gtk::Align::Fill);
    view.set_valign(gtk::Align::Fill);
    view.set_margin_start(0);
    view.set_margin_top(0);
    view.set_margin_end(0);
    view.set_margin_bottom(0);
    view.set_size_request(bounds.width.round() as i32, bounds.height.round() as i32);
    if let Some(parent) = view.parent() {
        parent.queue_resize();
    }
    view.queue_resize();
}

pub(super) fn resize(native: tauri::webview::PlatformWebview, bounds: ViewBounds) {
    resize_view(&native.inner(), bounds);
}

pub(super) fn configure(
    native: tauri::webview::PlatformWebview,
    bounds: ViewBounds,
) -> Result<String, String> {
    // Read the loaded WebKit library, not the compile-time pkg-config version.
    let version = cobble_plugin_webview_linux::browser_version();
    if version < (2, 52, 6) {
        return Err("PLUGIN_VIEW_RUNTIME_UNSUPPORTED".into());
    }

    let view = native.inner();
    let settings = WebViewExt::settings(&view).ok_or("PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?;
    let manager = view
        .website_data_manager()
        .ok_or("PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?;

    // Port zero is not a connectable proxy endpoint. Every ordinary network
    // request must go through it; no host is exempted. The custom plugin URI
    // scheme remains handled locally by Tauri's registered protocol.
    let mut proxy = NetworkProxySettings::new(Some("http://127.0.0.1:0"), &[]);
    manager.set_network_proxy_settings(NetworkProxyMode::Custom, Some(&mut proxy));
    settings.set_enable_webrtc(false);
    settings.set_enable_media_stream(false);
    settings.set_enable_dns_prefetching(false);
    settings.set_javascript_can_open_windows_automatically(false);
    attach(&view, bounds)?;

    Ok(format!("{}.{}.{}", version.0, version.1, version.2))
}

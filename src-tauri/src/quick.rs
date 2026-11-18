//! The quick-add window: a small command-bar that appears over whatever you're doing.
//!
//! Unlike the alert popup it takes keyboard focus (you type into it) and closes itself when you click away.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::screen::{self, Area};
use crate::glass;

pub const LABEL: &str = "quick";

/// Must match the quick window `width` in `tauri.conf.json`.
const WIDTH: f64 = 608.0;
/// How much of the screen the glass theme copies (CSS px) — tall enough for the card with its hints.
const GLASS_HEIGHT: f64 = 300.0;
/// The window's top edge sits this far down the work area, like a launcher.
const TOP_FRACTION: f64 = 0.16;

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn toggle(app: &AppHandle) {
    if window(app).and_then(|w| w.is_visible().ok()).unwrap_or(false) {
        hide(app);
    } else {
        show(app);
    }
}

pub fn show(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let Some(monitor) = screen::cursor_monitor(app) else { return };

    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let width = (WIDTH * scale).round() as i32;
    let x = work.position.x + (work.size.width as i32 - width) / 2;
    let y = work.position.y + (work.size.height as f64 * TOP_FRACTION).round() as i32;
    let _ = win.set_position(PhysicalPosition::new(x, y));

    // Glass theme: snapshot what's behind the window before it covers it.
    let area = Area {
        x,
        y,
        width,
        height: (GLASS_HEIGHT * scale).round() as i32,
    };
    let snapshot = glass::for_theme(app, &area, WIDTH, GLASS_HEIGHT);
    let _ = app.emit_to(LABEL, "glass-snapshot", snapshot);
    // Tells the page to clear its text and replay the entrance animation.
    let _ = app.emit_to(LABEL, "quick-open", ());

    let _ = win.show();
    let _ = win.set_always_on_top(true);
    let _ = win.set_focus();
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = window(app) {
        let _ = win.hide();
    }
}

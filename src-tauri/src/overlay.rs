//! The popup that drops in at the top-centre of the screen.
//!
//! The window itself is declared in `tauri.conf.json` (frameless, transparent, always-on-top,
//! non-activating so it never steals keyboard focus from whatever you're typing in).
//! This module decides *where* it appears and when it is shown or hidden, and — for the glass
//! theme — grabs the screen behind it first.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::screen::{self, Area};
use crate::glass;

pub const LABEL: &str = "overlay";

/// Must match the overlay `width` in `tauri.conf.json`.
const WIDTH: f64 = 528.0;
/// Logical pixels between the top of the work area and the top of the (transparent) window.
/// The page's own top padding already keeps the card 16 px clear of the edge.
const TOP_MARGIN: f64 = 0.0;
/// How much of the screen the glass theme copies: the window's width and this height (CSS px).
/// Tall enough to cover the card with its note opened and its snooze options expanded.
const GLASS_HEIGHT: f64 = 480.0;

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Position the (still hidden) window and, for the glass theme, snapshot the screen behind it.
/// The page receives the snapshot — or `null` when there isn't one — before it is shown.
pub fn prepare(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    if win.is_visible().unwrap_or(false) {
        return;
    }

    let area = place(app, &win);

    let snapshot = area.and_then(|a| glass::for_theme(app, &a, WIDTH, GLASS_HEIGHT));
    let _ = app.emit_to(LABEL, "glass-snapshot", snapshot);
}

pub fn show(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    if win.is_visible().unwrap_or(false) {
        return;
    }
    let _ = win.show();
    // Re-assert topmost: some full-screen apps knock a window off the top of the z-order.
    let _ = win.set_always_on_top(true);
}

/// Put a visible alert back on top of anything that has covered it since (used when it chimes again).
pub fn raise(app: &AppHandle) {
    if let Some(win) = window(app) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.set_always_on_top(true);
        }
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = window(app) {
        let _ = win.hide();
    }
}

/// Top-centre of the work area of the monitor the mouse is on, so the alert appears where you're looking.
fn place(app: &AppHandle, win: &WebviewWindow) -> Option<Area> {
    let monitor = screen::cursor_monitor(app)?;

    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let width = (WIDTH * scale).round() as i32;
    let x = work.position.x + (work.size.width as i32 - width) / 2;
    let y = work.position.y + (TOP_MARGIN * scale).round() as i32;
    let _ = win.set_position(PhysicalPosition::new(x, y));

    Some(Area {
        x,
        y,
        width,
        height: (GLASS_HEIGHT * scale).round() as i32,
    })
}

//! Where on the desktop the floating windows go.

use tauri::{AppHandle, Monitor};

/// A rectangle in physical pixels on the virtual desktop.
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The monitor the mouse is on (or the primary one), so floating windows appear where you're looking.
pub fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    app.cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
}

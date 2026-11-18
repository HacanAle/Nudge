//! "Quiet when presenting": hold alerts while a full-screen app, game or presentation is on screen, and keep
//! Nudge's floating windows out of screen shares and recordings.
//!
//! * Full screen / presentations: Windows already tracks this for its own notifications
//!   (`SHQueryUserNotificationState`); the scheduler asks once a second.
//! * Screen sharing has no "is anyone capturing?" API, so instead the alert and quick-add windows are excluded
//!   from capture (`WDA_EXCLUDEFROMCAPTURE`): you still see them, Teams / Zoom / OBS viewers don't.

use tauri::{AppHandle, Manager};

use crate::{overlay, quick, settings, AppState};

/// True while Windows says notifications should stay out of the way: a full-screen app or game is in front,
/// or presentation mode is on.
pub fn presenting() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
        };
        // SAFETY: no arguments, no pointers; just reads shell state.
        match unsafe { SHQueryUserNotificationState() } {
            Ok(state) => state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE,
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    false
}

/// Apply the setting to the floating windows: hidden from capture while it is on.
pub fn apply_capture_exclusion(app: &AppHandle) {
    let on = settings::quiet_when_presenting(&app.state::<AppState>().db);
    for label in [overlay::LABEL, quick::LABEL] {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.set_content_protected(on);
        }
    }
}

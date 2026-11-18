//! System-tray icon: a state-aware icon, a live tooltip and a menu.
//!
//! The icon changes with what Nudge is doing (idle / do-not-disturb / an alert waiting) and the tooltip
//! says what's next, so a glance at the tray answers "is it running, and what's coming up?".

use std::sync::atomic::{AtomicU8, Ordering};

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::{commands, now_ms, quick, settings, AppState};

const TRAY_ID: &str = "nudge";
const MAIN: &str = "main";

/// The checkable / retitled menu entries, kept so `refresh` can keep them in sync.
/// Cheap to clone: each one is a handle to the native item.
#[derive(Clone)]
pub struct TrayItems {
    dnd: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    quick: MenuItem<Wry>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Idle = 0,
    DoNotDisturb = 1,
    Alert = 2,
}

/// The look currently applied (255 = none yet), so the icon is only swapped when it actually changes.
static APPLIED: AtomicU8 = AtomicU8::new(255);

fn icon(look: Look) -> Image<'static> {
    let png: &[u8] = match look {
        Look::Idle => include_bytes!("../icons/tray.png"),
        Look::DoNotDisturb => include_bytes!("../icons/tray-dnd.png"),
        Look::Alert => include_bytes!("../icons/tray-alert.png"),
    };
    Image::from_bytes(png).expect("bundled tray icon is a valid PNG")
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Nudge", true, None::<&str>)?;
    let quick_item = MenuItem::with_id(app, "quick", quick_label(app), true, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, "dnd", "Do not disturb", true, false, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start with Windows",
        true,
        app.autolaunch().is_enabled().unwrap_or(false),
        None::<&str>,
    )?;
    let test = MenuItem::with_id(app, "test", "Send test alert", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Nudge", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &quick_item,
            &PredefinedMenuItem::separator(app)?,
            &dnd,
            &autostart,
            &test,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    *app.state::<AppState>().tray_items.lock().unwrap() = Some(TrayItems {
        dnd,
        autostart,
        quick: quick_item,
    });

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(Look::Idle))
        .tooltip("Nudge")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quick" => quick::show(app),
            "dnd" => {
                let enabled = !app.state::<AppState>().dnd.load(Ordering::Relaxed);
                set_dnd(app, enabled);
            }
            "autostart" => {
                let enabled = !app.autolaunch().is_enabled().unwrap_or(false);
                let _ = commands::apply_autostart(app, enabled);
                refresh(app);
            }
            "test" => {
                let _ = commands::fire_test_alert(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    APPLIED.store(Look::Idle as u8, Ordering::Relaxed);
    refresh(app);
    Ok(())
}

pub fn set_dnd(app: &AppHandle, enabled: bool) {
    app.state::<AppState>().dnd.store(enabled, Ordering::Relaxed);
    let _ = app.emit("dnd-changed", enabled);
    refresh(app);
}

pub fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Bring the icon, tooltip and menu in line with the current state. Cheap; safe to call from anywhere.
pub fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();
    let dnd = state.dnd.load(Ordering::Relaxed);
    let waiting = state.db.active().map(|a| a.len()).unwrap_or(0);

    let look = if waiting > 0 {
        Look::Alert
    } else if dnd {
        Look::DoNotDisturb
    } else {
        Look::Idle
    };

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if APPLIED.swap(look as u8, Ordering::Relaxed) != look as u8 {
            let _ = tray.set_icon(Some(icon(look)));
        }
        let _ = tray.set_tooltip(Some(tooltip(&state, dnd, waiting)));
    }

    // Copy the handles out and release the lock before using them. Off the main thread each setter waits
    // for the main thread; holding the lock meanwhile deadlocks against a main-thread caller (a command,
    // a tray click) that is waiting for the same lock, and the whole app freezes.
    let items = state.tray_items.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(items) = items {
        let _ = items.dnd.set_checked(dnd);
        let _ = items.autostart.set_checked(app.autolaunch().is_enabled().unwrap_or(false));
        let _ = items.quick.set_text(quick_label(app));
    }
}

/// "New reminder        Ctrl+Alt+N" — the tab right-aligns the shortcut like a native menu.
fn quick_label(app: &AppHandle) -> String {
    format!("New reminder\t{}", settings::hotkey(&app.state::<AppState>().db))
}

fn tooltip(state: &AppState, dnd: bool, waiting: usize) -> String {
    if waiting > 0 {
        let plural = if waiting == 1 { "" } else { "s" };
        return format!("Nudge · {waiting} alert{plural} waiting");
    }
    if dnd {
        return "Nudge · Do not disturb".to_string();
    }
    let now = now_ms();
    let next = state
        .db
        .upcoming()
        .ok()
        .and_then(|list| list.into_iter().find(|r| r.due_at > now));
    match next {
        Some(r) => format!("Nudge · {} {}", shorten(&r.title, 48), relative(r.due_at - now)),
        None => "Nudge · Nothing scheduled".to_string(),
    }
}

/// "in 12 min", "in 2 h", "in 3 d".
fn relative(ms: i64) -> String {
    let minutes = (ms.max(0) + 30_000) / 60_000;
    match minutes {
        0 => "now".to_string(),
        1..=59 => format!("in {minutes} min"),
        60..=1439 => format!("in {} h", (minutes + 30) / 60),
        _ => format!("in {} d", (minutes + 720) / 1440),
    }
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times_read_naturally() {
        assert_eq!(relative(10_000), "now");
        assert_eq!(relative(90_000), "in 2 min");
        assert_eq!(relative(12 * 60_000), "in 12 min");
        assert_eq!(relative(59 * 60_000), "in 59 min");
        assert_eq!(relative(2 * 3_600_000), "in 2 h");
        assert_eq!(relative(3 * 86_400_000), "in 3 d");
        assert_eq!(relative(-5), "now");
    }

    #[test]
    fn long_titles_are_shortened_without_breaking_characters() {
        assert_eq!(shorten("short", 10), "short");
        assert_eq!(shorten("a very long reminder title indeed", 12), "a very long…");
        assert_eq!(shorten("日本語のとても長いタイトル", 5).chars().count(), 5);
    }
}

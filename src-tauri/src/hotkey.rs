//! Global shortcuts: the quick-add one (rebindable), and Done / Snooze for the alert on screen.

use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

use crate::db::{AppError, Result};
use crate::{settings, AppState};

/// Answer the alert on screen from anywhere. Only registered while an alert is showing, so the rest of the
/// time these keys still reach other apps.
pub const ALERT_DONE: &str = "Ctrl+Alt+D";
pub const ALERT_SNOOZE: &str = "Ctrl+Alt+S";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKey {
    Done,
    Snooze,
}

/// Parse a shortcut like `Ctrl+Alt+N` and make sure it is one we are willing to grab globally.
/// A bare letter, or Shift + letter, would steal ordinary typing from every app, so one of Ctrl, Alt
/// or Win is required.
pub fn parse(spec: &str) -> Result<Shortcut> {
    let shortcut = Shortcut::from_str(spec.trim())
        .map_err(|_| AppError(format!("“{spec}” isn't a shortcut Nudge understands.")))?;
    if !shortcut.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err("Use Ctrl, Alt or Win together with another key.".into());
    }
    Ok(shortcut)
}

/// Register the saved shortcut at startup. If Windows refuses (another app owns it), remember why so
/// Settings can say so, instead of failing silently.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let spec = settings::hotkey(&state.db);
    let outcome = parse(&spec).and_then(|shortcut| {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|_| AppError(taken_message(&spec)))
    });
    *state.hotkey_error.lock().unwrap() = outcome.err().map(|e| e.0);
}

/// Switch to a new shortcut. The old one stays in place if the new one can't be registered.
pub fn change(app: &AppHandle, spec: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let new = parse(spec)?;
    if [ALERT_DONE, ALERT_SNOOZE].iter().any(|k| parse(k).is_ok_and(|k| k.id() == new.id())) {
        return Err(format!("{spec} is used to answer alerts. Pick another one.").into());
    }
    let old = parse(&settings::hotkey(&state.db)).ok();
    let shortcuts = app.global_shortcut();

    if !shortcuts.is_registered(new) {
        shortcuts.register(new).map_err(|_| AppError(taken_message(spec)))?;
    }
    if let Some(old) = old.filter(|o| o.id() != new.id()) {
        let _ = shortcuts.unregister(old);
    }

    settings::set_hotkey(&state.db, spec)?;
    *state.hotkey_error.lock().unwrap() = None;
    Ok(())
}

/// Register or release the alert shortcuts. Never touches one that is also the quick-add shortcut.
pub fn alert_keys(app: &AppHandle, on: bool) {
    let quick = parse(&settings::hotkey(&app.state::<AppState>().db)).ok().map(|q| q.id());
    let shortcuts = app.global_shortcut();
    for spec in [ALERT_DONE, ALERT_SNOOZE] {
        let Ok(key) = parse(spec) else { continue };
        if Some(key.id()) == quick {
            continue;
        }
        let registered = shortcuts.is_registered(key);
        if on && !registered {
            let _ = shortcuts.register(key); // if another app owns it, the buttons still work
        } else if !on && registered {
            let _ = shortcuts.unregister(key);
        }
    }
}

/// Which alert action a pressed shortcut means, or `None` for the quick-add shortcut.
pub fn alert_key(app: &AppHandle, pressed: &Shortcut) -> Option<AlertKey> {
    let quick = parse(&settings::hotkey(&app.state::<AppState>().db)).ok();
    if quick.is_some_and(|q| q.id() == pressed.id()) {
        return None;
    }
    [(ALERT_DONE, AlertKey::Done), (ALERT_SNOOZE, AlertKey::Snooze)]
        .into_iter()
        .find(|(spec, _)| parse(spec).is_ok_and(|k| k.id() == pressed.id()))
        .map(|(_, action)| action)
}

fn taken_message(spec: &str) -> String {
    format!("Windows won't let Nudge use {spec}. Another app is probably using it.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_combinations() {
        assert!(parse("Ctrl+Alt+N").is_ok());
        assert!(parse("ctrl+shift+space").is_ok());
        assert!(parse("Alt+F9").is_ok());
        // The Windows key is called "Super" by the shortcut parser.
        assert!(parse("Super+Shift+N").is_ok());
    }

    #[test]
    fn refuses_combinations_that_would_steal_typing() {
        assert!(parse("N").is_err(), "a bare letter");
        assert!(parse("Shift+N").is_err(), "shift alone only capitalises");
        assert!(parse("F5").is_err(), "no modifier");
    }

    #[test]
    fn the_alert_shortcuts_are_valid_and_distinct() {
        let done = parse(ALERT_DONE).unwrap();
        let snooze = parse(ALERT_SNOOZE).unwrap();
        assert_ne!(done.id(), snooze.id());
        assert_ne!(done.id(), parse(crate::settings::DEFAULT_HOTKEY).unwrap().id());
    }

    #[test]
    fn refuses_nonsense() {
        assert!(parse("").is_err());
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Ctrl+Alt+Banana").is_err());
        assert!(parse("Win+N").is_err(), "\"Win\" is spelled \"Super\"");
    }
}

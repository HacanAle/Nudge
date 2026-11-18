//! Glue between the database and the two webviews.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};

use crate::db::{Priority, Reminder};
use crate::{hotkey, overlay, tray, AppState};

/// Re-read what is ringing and tell the windows about it.
///
/// * the popup gets the exact list it should render (empty list ⇒ it animates out and asks to be hidden);
/// * while a presentation holds alerts back, the popup is tucked away instead (unless one is high priority);
/// * the manager window is told its lists are stale.
pub fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let active = state.db.active().unwrap_or_default();
    let show = should_show(state.quiet.load(Ordering::Relaxed), &active);

    // Position and (glass theme) snapshot first, so the page has its backdrop before the card animates in.
    if show {
        overlay::prepare(app);
    }

    let _ = app.emit_to(overlay::LABEL, "alerts-changed", &active);
    let _ = app.emit("reminders-changed", ());

    if show {
        overlay::show(app);
    } else if !active.is_empty() {
        overlay::hide(app);
    }
    // Ctrl+Alt+D / Ctrl+Alt+S only belong to Nudge while an alert is actually on screen.
    hotkey::alert_keys(app, show);

    tray::refresh(app);
}

/// The alerts currently on screen (empty when nothing is, or when a presentation is holding them back).
pub fn on_screen(app: &AppHandle) -> Vec<Reminder> {
    let state = app.state::<AppState>();
    let active = state.db.active().unwrap_or_default();
    if should_show(state.quiet.load(Ordering::Relaxed), &active) {
        active
    } else {
        Vec::new()
    }
}

/// Something is ringing and nothing is holding it back. High priority comes through even during a presentation.
fn should_show(quiet: bool, active: &[Reminder]) -> bool {
    !active.is_empty() && (!quiet || active.iter().any(|r| r.priority == Priority::High))
}

#[cfg(test)]
mod tests {
    use super::should_show;
    use crate::db::{Db, NewReminder, Priority, Repeat};

    fn ringing(priorities: &[Priority]) -> Vec<crate::db::Reminder> {
        let db = Db::open_in_memory().unwrap();
        for (i, &priority) in priorities.iter().enumerate() {
            let input = NewReminder { title: format!("r{i}"), note: String::new(), priority, due_at: 1_000, repeat: Repeat::None };
            db.add(input, 0).unwrap();
        }
        db.take_due(2_000, false).unwrap();
        db.active().unwrap()
    }

    #[test]
    fn a_presentation_tucks_alerts_away_unless_one_is_high_priority() {
        assert!(!should_show(false, &[]), "nothing ringing");
        assert!(should_show(false, &ringing(&[Priority::Low])));
        assert!(!should_show(true, &ringing(&[Priority::Normal, Priority::Low])));
        assert!(should_show(true, &ringing(&[Priority::Normal, Priority::High])));
    }
}

//! Fires reminders when they come due.
//!
//! A one-second tick over an indexed column is cheap, and — unlike sleeping until the next due
//! time — it is naturally correct across system sleep, clock changes and edits from other windows.

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::time::{interval, MissedTickBehavior};

use crate::{alerts, now_ms, overlay, quiet, settings, sound, tray, AppState};

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut ticks: u32 = 0;
        // When the chime last sounded for what is on screen (0 = nothing on screen), for "ring again".
        let mut last_chime: i64 = 0;
        loop {
            ticker.tick().await;
            let state = app.state::<AppState>();
            let now = now_ms();

            // Keep the tray tooltip's "in 12 min" fresh.
            ticks = ticks.wrapping_add(1);
            if ticks % 30 == 0 {
                tray::refresh(&app);
            }

            // Quiet when presenting: re-checked every tick. When it changes, what is ringing is tucked away
            // (a presentation started) or brought back (it ended).
            let quiet = settings::quiet_when_presenting(&state.db) && quiet::presenting();
            if state.quiet.swap(quiet, Ordering::Relaxed) != quiet {
                alerts::sync(&app);
            }

            // Do-not-disturb leaves reminders pending; they fire the moment it is switched off.
            if state.dnd.load(Ordering::Relaxed) {
                continue;
            }

            // While presenting only high-priority reminders get through; the rest stay pending until it ends.
            match state.db.take_due(now, quiet) {
                Ok(due) if !due.is_empty() => {
                    if let Some(loudest) = due.iter().map(|r| r.priority).max_by_key(|p| p.rank()) {
                        sound::play(loudest);
                        last_chime = now;
                    }
                    alerts::sync(&app);
                }
                Ok(_) => {}
                Err(e) => eprintln!("scheduler: {}", e.0),
            }

            // Ring again: an alert left unanswered on screen chimes every few minutes, if the user asked for it.
            let on_screen = alerts::on_screen(&app);
            let every = i64::from(settings::rechime_minutes(&state.db)) * 60_000;
            let (ring, next) = ring_again(last_chime, now, every, !on_screen.is_empty());
            last_chime = next;
            if ring {
                if let Some(loudest) = on_screen.iter().map(|r| r.priority).max_by_key(|p| p.rank()) {
                    sound::play(loudest);
                }
                overlay::raise(&app);
            }
        }
    });
}

/// Whether an unanswered alert should chime again now, and the new "last chimed" time.
/// `last` is 0 while nothing is on screen; an alert that reappears silently (a presentation ended) starts the
/// count from then rather than chiming at once.
fn ring_again(last: i64, now: i64, every: i64, on_screen: bool) -> (bool, i64) {
    if !on_screen {
        return (false, 0);
    }
    if last == 0 {
        return (false, now);
    }
    if every > 0 && now - last >= every {
        return (true, now);
    }
    (false, last)
}

#[cfg(test)]
mod tests {
    use super::ring_again;

    const MIN: i64 = 60_000;

    #[test]
    fn rings_again_on_schedule_while_unanswered() {
        let fired = 1_000 * MIN;
        assert_eq!(ring_again(fired, fired + 4 * MIN, 5 * MIN, true), (false, fired));
        assert_eq!(ring_again(fired, fired + 5 * MIN, 5 * MIN, true), (true, fired + 5 * MIN));
    }

    #[test]
    fn never_rings_when_off_or_nothing_is_showing() {
        assert_eq!(ring_again(MIN, 100 * MIN, 0, true), (false, MIN), "off");
        assert_eq!(ring_again(MIN, 100 * MIN, 5 * MIN, false), (false, 0), "answered: the count resets");
    }

    #[test]
    fn an_alert_that_reappears_silently_counts_from_then() {
        assert_eq!(ring_again(0, 50 * MIN, 5 * MIN, true), (false, 50 * MIN));
    }
}

//! IPC surface used by the two webviews.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::db::{AppError, BackupReminder, NewReminder, Priority, Reminder, Repeat, Result, Status};
use crate::hotkey::AlertKey;
use crate::settings::{self, Settings};
use crate::{alerts, hotkey, now_ms, overlay, quick, quiet, shell, tray, AppState, STARTUP_FLAG};

/// What the alert's "Snooze" button and Ctrl+Alt+S give you (the popup's DEFAULT_SNOOZE_MIN).
const DEFAULT_SNOOZE_MS: i64 = 10 * 60_000;

#[tauri::command]
pub fn list_upcoming(state: State<'_, AppState>) -> Result<Vec<Reminder>> {
    state.db.upcoming()
}

#[tauri::command]
pub fn list_history(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<Reminder>> {
    state.db.history(limit.unwrap_or(50).min(500))
}

/// What the popup should be showing right now (it asks once on load; afterwards it listens for events).
#[tauri::command]
pub fn get_active(state: State<'_, AppState>) -> Result<Vec<Reminder>> {
    state.db.active()
}

#[tauri::command]
pub fn add_reminder(app: AppHandle, state: State<'_, AppState>, input: NewReminder) -> Result<Reminder> {
    let reminder = state.db.add(input, now_ms())?;
    let _ = app.emit("reminders-changed", ());
    tray::refresh(&app);
    Ok(reminder)
}

/// Edit a reminder that is still scheduled or ringing. Takes the same fields as a new one.
#[tauri::command]
pub fn update_reminder(app: AppHandle, state: State<'_, AppState>, id: i64, input: NewReminder) -> Result<Reminder> {
    let reminder = state.db.update(id, input, now_ms())?;
    // Re-reads what is ringing: the popup shows the new text, or closes if it was moved to later.
    alerts::sync(&app);
    Ok(reminder)
}

#[tauri::command]
pub fn delete_reminder(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.delete(id)?;
    alerts::sync(&app);
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    state.db.clear_history()?;
    let _ = app.emit("reminders-changed", ());
    tray::refresh(&app);
    Ok(())
}

/// "Done" (`dismissed = false`) or the ✕ button (`dismissed = true`).
/// Repeating reminders re-arm themselves for the next occurrence either way.
#[tauri::command]
pub fn finish_reminder(app: AppHandle, state: State<'_, AppState>, id: i64, dismissed: bool) -> Result<()> {
    let outcome = if dismissed { Status::Dismissed } else { Status::Done };
    state.db.complete(id, outcome, now_ms())?;
    alerts::sync(&app);
    Ok(())
}

/// `until` is an absolute unix-ms time; the UI turns "10 min" / "tomorrow 9 AM" into one.
#[tauri::command]
pub fn snooze_reminder(app: AppHandle, state: State<'_, AppState>, id: i64, until: i64) -> Result<()> {
    state.db.snooze(id, until, now_ms())?;
    alerts::sync(&app);
    Ok(())
}

/// Called by the popup once its exit animation has finished. Only hides if nothing is ringing,
/// so a reminder that fired during the animation is never hidden by mistake.
#[tauri::command]
pub fn hide_overlay(app: AppHandle, state: State<'_, AppState>) {
    if state.db.active().map(|a| a.is_empty()).unwrap_or(true) {
        overlay::hide(&app);
    }
}

#[tauri::command]
pub fn get_dnd(state: State<'_, AppState>) -> bool {
    state.dnd.load(std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn set_dnd(app: AppHandle, enabled: bool) {
    tray::set_dnd(&app, enabled);
}

#[tauri::command]
pub fn send_test_alert(app: AppHandle) -> Result<()> {
    fire_test_alert(&app)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    current_settings(&app)
}

/// Saved settings plus live state the database doesn't know about (did the shortcut actually register?).
pub fn current_settings(app: &AppHandle) -> Settings {
    let state = app.state::<AppState>();
    let hotkey_active = state.hotkey_error.lock().unwrap().is_none();
    Settings {
        hotkey_active,
        ..settings::load(&state.db)
    }
}

fn broadcast_settings(app: &AppHandle) -> Settings {
    let current = current_settings(app);
    let _ = app.emit("settings-changed", &current);
    current
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<Settings> {
    change_theme(&app, &theme)
}

/// Save a theme and tell every window to restyle itself.
pub fn change_theme(app: &AppHandle, id: &str) -> Result<Settings> {
    settings::set_theme(&app.state::<AppState>().db, id)?;
    Ok(broadcast_settings(app))
}

/// Change the quick-add shortcut. The old one keeps working if the new one can't be registered.
#[tauri::command]
pub fn set_hotkey(app: AppHandle, hotkey: String) -> Result<Settings> {
    hotkey::change(&app, hotkey.trim())?;
    tray::refresh(&app);
    Ok(broadcast_settings(&app))
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool> {
    let now_enabled = apply_autostart(&app, enabled)?;
    tray::refresh(&app);
    Ok(now_enabled)
}

/// Turn "start with Windows" on or off and report the real state afterwards (the registry is the truth).
///
/// Turning it on always writes the entry, so it points at *this* executable even if a stale one was there.
/// Only a "no" is remembered (see `ensure_autostart`).
pub fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<bool> {
    let launcher = app.autolaunch();
    let current = launcher.is_enabled().unwrap_or(false);
    if enabled || current {
        let outcome = if enabled { launcher.enable() } else { launcher.disable() };
        outcome.map_err(|e| AppError(format!("Couldn't change the Windows startup setting: {e}")))?;
    }
    let _ = settings::set_autostart_declined(&app.state::<AppState>().db, !enabled);
    let actual = launcher.is_enabled().unwrap_or(enabled);
    let _ = app.emit("autostart-changed", actual);
    Ok(actual)
}

/// Keep "Start with Windows" working without anyone having to think about it. Runs on every launch of a
/// release build (a dev build must never write its own path into the real entry: that is how a dead
/// `target\debug\nudge.exe` entry got there once).
///
/// * no entry — a new PC, or a reinstall (uninstalling removes it): turn it on, unless the user turned it off;
/// * an entry that points at some other copy of Nudge: re-point it at this one. The plugin's `is_enabled` only
///   checks that an entry *exists*, so a stale one used to show as "on" in Settings while Windows launched nothing;
/// * an entry the user disabled in Task Manager: leave it alone.
pub fn ensure_autostart(app: &AppHandle) {
    if cfg!(debug_assertions) || settings::autostart_declined(&app.state::<AppState>().db) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let needs_writing = match shell::startup_command(&app.package_info().name) {
        None => true,
        Some(stored) => {
            app.autolaunch().is_enabled().unwrap_or(false)
                && !startup_entry_matches(&stored, &exe.display().to_string())
        }
    };
    if needs_writing {
        let _ = apply_autostart(app, true);
    }
}

/// Whether a stored sign-in command is exactly what turning "Start with Windows" on from `exe` would write.
fn startup_entry_matches(stored: &str, exe: &str) -> bool {
    stored.trim().eq_ignore_ascii_case(&format!("{exe} {STARTUP_FLAG}"))
}

/// Hold alerts during presentations and keep them out of screen shares.
#[tauri::command]
pub fn set_quiet_when_presenting(app: AppHandle, on: bool) -> Result<Settings> {
    settings::set_quiet_when_presenting(&app.state::<AppState>().db, on)?;
    quiet::apply_capture_exclusion(&app);
    Ok(broadcast_settings(&app))
}

/// Chime again every `minutes` while an alert waits (0 = off).
#[tauri::command]
pub fn set_rechime(app: AppHandle, minutes: u32) -> Result<Settings> {
    settings::set_rechime_minutes(&app.state::<AppState>().db, minutes)?;
    Ok(broadcast_settings(&app))
}

/// A link in a reminder's note.
#[tauri::command]
pub fn open_link(url: String) -> Result<()> {
    shell::open_url(&url).map_err(AppError)
}

/// The keyboard way to answer the alert on top (Ctrl+Alt+D / Ctrl+Alt+S), same as its buttons.
pub fn answer_top_alert(app: &AppHandle, key: AlertKey) {
    let Some(top) = alerts::on_screen(app).into_iter().next() else { return };
    let state = app.state::<AppState>();
    let now = now_ms();
    let _ = match key {
        AlertKey::Done => state.db.complete(top.id, Status::Done, now),
        AlertKey::Snooze => state.db.snooze(top.id, now + DEFAULT_SNOOZE_MS, now),
    };
    alerts::sync(app);
}

// ───────────────────────────── backup ─────────────────────────────

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    app: String,
    version: u32,
    exported_at: i64,
    reminders: Vec<BackupReminder>,
}

/// Write every reminder to a JSON file in Downloads and show it in Explorer. Returns the file name.
#[tauri::command]
pub fn export_reminders(app: AppHandle, state: State<'_, AppState>) -> Result<String> {
    let backup = Backup {
        app: "Nudge".into(),
        version: 1,
        exported_at: now_ms(),
        reminders: state.db.all()?.into_iter().map(Into::into).collect(),
    };
    let dir = app
        .path()
        .download_dir()
        .map_err(|_| AppError::from("Couldn't find your Downloads folder"))?;
    let name = format!("nudge-backup-{}.json", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
    let path = dir.join(&name);
    let json = serde_json::to_string_pretty(&backup).map_err(|e| AppError(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| AppError(format!("Couldn't save the backup: {e}")))?;
    shell::reveal(&path);
    Ok(name)
}

#[derive(Serialize)]
pub struct ImportResult {
    added: usize,
    skipped: usize,
}

/// Add the reminders from a backup file's text (a Nudge backup, or a bare list of reminders).
#[tauri::command]
pub fn import_reminders(app: AppHandle, state: State<'_, AppState>, json: String) -> Result<ImportResult> {
    let items = serde_json::from_str::<Backup>(&json)
        .map(|b| b.reminders)
        .or_else(|_| serde_json::from_str::<Vec<BackupReminder>>(&json))
        .map_err(|_| AppError::from("That file isn't a Nudge backup"))?;
    let (added, skipped) = state.db.import(items, now_ms())?;
    alerts::sync(&app);
    Ok(ImportResult { added, skipped })
}

#[tauri::command]
pub fn hide_quick(app: AppHandle) {
    quick::hide(&app);
}

/// A reminder that is due immediately — lets you preview the popup and the chime.
pub fn fire_test_alert(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    state.db.add(
        NewReminder {
            title: "Time to stretch your legs".into(),
            note: "This is what a Nudge alert looks like. Snooze it, or mark it done.".into(),
            priority: Priority::Normal,
            due_at: now_ms(),
            repeat: Repeat::None,
        },
        now_ms(),
    )?;
    let _ = app.emit("reminders-changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::startup_entry_matches;

    const INSTALLED: &str = r"C:\Users\user\AppData\Local\Nudge\nudge.exe";

    #[test]
    fn a_current_startup_entry_is_left_alone() {
        assert!(startup_entry_matches(&format!("{INSTALLED} --hidden"), INSTALLED));
        assert!(startup_entry_matches(&format!("{} --hidden", INSTALLED.to_uppercase()), INSTALLED), "paths ignore case");
    }

    #[test]
    fn an_entry_for_another_build_or_without_the_flag_is_stale() {
        let debug = r"C:\Users\user\Projects\nudge\src-tauri\target\debug\nudge.exe --hidden";
        assert!(!startup_entry_matches(debug, INSTALLED));
        assert!(!startup_entry_matches(INSTALLED, INSTALLED), "would open the window at sign-in");
        assert!(!startup_entry_matches("", INSTALLED));
    }
}

mod alerts;
mod commands;
mod db;
mod glass;
mod hotkey;
mod overlay;
mod quick;
mod quiet;
mod scheduler;
mod screen;
mod settings;
mod shell;
mod sound;
mod tray;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::ShortcutState;

use db::Db;

pub struct AppState {
    pub db: Db,
    /// Do-not-disturb: reminders stay pending (and fire later) instead of popping up.
    pub dnd: AtomicBool,
    /// A full-screen app or presentation is up and "quiet when presenting" is on: only high priority pops up.
    pub quiet: AtomicBool,
    /// Tray menu entries that need to follow state (checkmarks, the shortcut shown beside "New reminder").
    pub tray_items: Mutex<Option<tray::TrayItems>>,
    /// Why the quick-add shortcut isn't active, if Windows refused it. `None` = working.
    pub hotkey_error: Mutex<Option<String>>,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// What "Start with Windows" passes, so a sign-in launch goes straight to the tray.
pub const STARTUP_FLAG: &str = "--hidden";

/// Command-line flags, from this launch or from a second launch forwarded by the single-instance plugin.
///
/// * `--test`           pop a test alert (and stay in the tray)
/// * `--quick`          open the quick-add window
/// * `--hidden`         start in the tray without opening the main window (what "Start with Windows" passes)
/// * `--theme <id>`     switch theme (also `--theme=<id>`)
fn handle_args(app: &AppHandle, args: &[String], first_launch: bool) {
    let has = |flag: &str| args.iter().any(|a| a == flag);

    if let Some(id) = flag_value(args, "--theme") {
        let _ = commands::change_theme(app, &id);
    }

    let mut handled = false;
    if has("--test") {
        let _ = commands::fire_test_alert(app);
        handled = true;
    }
    if has("--quick") {
        quick::show(app);
        handled = true;
    }
    if !handled && !(first_launch && has(STARTUP_FLAG)) {
        tray::show_main(app);
    }
}

/// `--flag value` or `--flag=value`.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    args.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix(&prefix)
            .map(str::to_string)
            .or_else(|| (a == flag).then(|| args.get(i + 1).cloned()).flatten())
    })
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch just forwards its arguments here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_args(app, &args, false);
        }))
        // "Start with Windows" launches us quietly into the tray.
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![STARTUP_FLAG])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    // Ctrl+Alt+D / Ctrl+Alt+S answer the alert on screen; the other shortcut is quick add.
                    match hotkey::alert_key(app, shortcut) {
                        // Not inline: the plugin holds its shortcut table locked while this handler runs, and
                        // answering releases the alert shortcuts, which needs that lock (inline = frozen app).
                        Some(action) => {
                            let app = app.clone();
                            tauri::async_runtime::spawn_blocking(move || commands::answer_top_alert(&app, action));
                        }
                        None => quick::toggle(app),
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("nudge.db")).map_err(|e| e.0)?;

            app.manage(AppState {
                db,
                dnd: AtomicBool::new(false),
                quiet: AtomicBool::new(false),
                tray_items: Mutex::new(None),
                hotkey_error: Mutex::new(None),
            });

            // Windows are created here, not by Tauri at startup (`create: false` in tauri.conf.json), so a
            // page can never call a command before the state above exists.
            for window in &app.config().app.windows {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window)?.build()?;
            }

            quiet::apply_capture_exclusion(app.handle());
            sound::warm_up();
            hotkey::start(app.handle());
            // Before the tray menu reads the setting: start-with-Windows is on by default and repaired if stale.
            commands::ensure_autostart(app.handle());
            tray::setup(app.handle())?;
            shell::promote_tray_icon(app.handle().clone());
            scheduler::spawn(app.handle().clone());

            let args: Vec<String> = std::env::args().collect();
            handle_args(app.handle(), &args, true);
            // Anything left ringing from a previous run (or overdue) is picked up by the scheduler's first tick.
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_upcoming,
            commands::list_history,
            commands::get_active,
            commands::add_reminder,
            commands::update_reminder,
            commands::delete_reminder,
            commands::clear_history,
            commands::finish_reminder,
            commands::snooze_reminder,
            commands::hide_overlay,
            commands::get_dnd,
            commands::set_dnd,
            commands::send_test_alert,
            commands::get_settings,
            commands::set_theme,
            commands::set_hotkey,
            commands::get_autostart,
            commands::set_autostart,
            commands::hide_quick,
            commands::set_quiet_when_presenting,
            commands::set_rechime,
            commands::open_link,
            commands::export_reminders,
            commands::import_reminders,
        ])
        .on_window_event(|window, event| match event {
            // The ✕ on the main window tucks Nudge into the tray; reminders keep firing.
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Quick-add behaves like a launcher: click anywhere else and it goes away.
            WindowEvent::Focused(false) if window.label() == quick::LABEL => {
                quick::hide(window.app_handle());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("failed to build Nudge");

    app.run(|_app, event| {
        // Closing the last visible window must not quit the app — only "Quit" in the tray does.
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::flag_value;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_both_flag_spellings() {
        assert_eq!(flag_value(&args(&["nudge.exe", "--theme", "punk"]), "--theme").as_deref(), Some("punk"));
        assert_eq!(flag_value(&args(&["nudge.exe", "--theme=voltage", "--test"]), "--theme").as_deref(), Some("voltage"));
    }

    #[test]
    fn a_missing_or_dangling_flag_is_none() {
        assert_eq!(flag_value(&args(&["nudge.exe", "--test"]), "--theme"), None);
        assert_eq!(flag_value(&args(&["nudge.exe", "--theme"]), "--theme"), None);
    }
}

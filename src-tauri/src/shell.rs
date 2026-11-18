//! Windows shell niceties.

use std::path::Path;

use tauri::AppHandle;

/// Only web and mail links from a note are opened: never files, scripts or other protocol handlers.
pub fn is_openable(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["https://", "http://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
        && url.len() <= 4096
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"')
}

/// Open a link in the default browser (or mail app).
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_openable(url) {
        return Err("Only web and email links can be opened".into());
    }
    #[cfg(windows)]
    return windows_impl::open(url);
    #[cfg(not(windows))]
    Err("Opening links is only supported on Windows".into())
}

/// Open the folder holding `path` in Explorer with the file selected.
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Explorer wants the quotes around the path only, not around the whole switch.
        let _ = std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// Windows 11 parks every new tray icon in the "hidden icons" overflow, where a small dark icon is easy
/// to never find. Once per install, a few seconds after the icon first appears, ask Windows to show it on
/// the taskbar — the same flag the "show icon" switch in Settings → Taskbar sets. If you hide it again
/// later it stays hidden: this only ever runs once per executable.
pub fn promote_tray_icon(app: AppHandle) {
    #[cfg(windows)]
    windows_impl::promote(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// The command Windows runs at sign-in for the "Start with Windows" entry called `name`, if there is one.
pub fn startup_command(name: &str) -> Option<String> {
    #[cfg(windows)]
    return windows_impl::startup_command(name);
    #[cfg(not(windows))]
    {
        let _ = name;
        None
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::time::Duration;

    use tauri::{AppHandle, Manager};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    use crate::{settings, AppState};

    const SETTINGS_KEY: &str = r"Control Panel\NotifyIconSettings";
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn open(url: &str) -> Result<(), String> {
        use windows::core::{w, HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        // SAFETY: every string is a valid, NUL-terminated wide string that outlives the call.
        let result = unsafe {
            ShellExecuteW(None, w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
        };
        // ShellExecute reports success as a value above 32.
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err("Windows couldn't open that link".into())
        }
    }

    pub fn startup_command(name: &str) -> Option<String> {
        let run = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ).ok()?;
        run.get_value(name).ok()
    }

    pub fn promote(app: AppHandle) {
        let Some(exe) = exe_path() else { return };
        if settings::tray_promoted_for(&app.state::<AppState>().db).as_deref() == Some(exe.as_str()) {
            return;
        }

        // Windows creates our entry only after the icon has been on screen for a moment.
        std::thread::spawn(move || {
            for _ in 0..30 {
                std::thread::sleep(Duration::from_secs(1));
                if promote_entry(&exe) {
                    let _ = settings::set_tray_promoted_for(&app.state::<AppState>().db, &exe);
                    return;
                }
            }
        });
    }

    fn exe_path() -> Option<String> {
        let path = std::env::current_exe().ok()?.to_string_lossy().to_string();
        Some(path.trim_start_matches(r"\\?\").to_string())
    }

    /// Set `IsPromoted = 1` on the notify-icon entry for this executable. Returns whether the entry exists.
    fn promote_entry(exe: &str) -> bool {
        let Ok(root) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(SETTINGS_KEY, KEY_READ) else {
            return false;
        };
        for name in root.enum_keys().flatten() {
            let Ok(entry) = root.open_subkey_with_flags(&name, KEY_READ | KEY_SET_VALUE) else {
                continue;
            };
            let path: String = entry.get_value("ExecutablePath").unwrap_or_default();
            if path.eq_ignore_ascii_case(exe) {
                if entry.get_value::<u32, _>("IsPromoted").unwrap_or(0) == 0 {
                    let _ = entry.set_value("IsPromoted", &1u32);
                }
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::is_openable;

    #[test]
    fn opens_web_and_mail_links_only() {
        assert!(is_openable("https://teams.microsoft.com/l/meetup-join/19%3ameeting"));
        assert!(is_openable("http://example.com"));
        assert!(is_openable("mailto:someone@example.com"));
        assert!(!is_openable("https://"), "nothing after the scheme");
        assert!(!is_openable("file:///C:/Windows/System32/cmd.exe"));
        assert!(!is_openable("ms-settings:privacy"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("https://example.com/a b"), "no spaces");
        assert!(!is_openable("https://example.com/\"x"), "no quotes");
    }
}

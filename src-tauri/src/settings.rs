//! User preferences, persisted in the `settings` table.

use serde::Serialize;

use crate::db::{AppError, Db, Result};

/// Theme ids. The names and looks live in the frontend (`src/lib/themes.ts`, `src/themes.css`);
/// the backend only needs the ids so it can reject junk and know when to prepare the glass backdrop.
pub const THEMES: [&str; 8] = ["slab", "orchard", "vitrine", "beige", "punk", "voltage", "handbill", "nudge"];
pub const DEFAULT_THEME: &str = "slab";
pub const GLASS_THEME: &str = "vitrine";

pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+N";

const THEME_KEY: &str = "theme";
const HOTKEY_KEY: &str = "hotkey";
const TRAY_PROMOTED_KEY: &str = "tray_promoted";
const AUTOSTART_OFF_KEY: &str = "autostart_off";
const QUIET_KEY: &str = "quiet_when_presenting";
const RECHIME_KEY: &str = "rechime_minutes";

/// How often an unanswered alert may chime again. 0 = never.
pub const RECHIME_CHOICES: [u32; 5] = [0, 2, 5, 10, 15];

/// What the Settings tab shows.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
    /// The quick-add shortcut, e.g. `Ctrl+Alt+N`.
    pub hotkey: String,
    /// False when Windows refused the shortcut (another app already owns it).
    pub hotkey_active: bool,
    /// Hold alerts while a full-screen app or presentation is up, and keep them out of screen shares.
    pub quiet_when_presenting: bool,
    /// Chime again every this many minutes while an alert waits for an answer (0 = off).
    pub rechime_minutes: u32,
}

pub fn is_theme(id: &str) -> bool {
    THEMES.contains(&id)
}

/// The saved theme, or the default if nothing valid is stored.
pub fn theme(db: &Db) -> String {
    db.get_setting(THEME_KEY)
        .ok()
        .flatten()
        .filter(|t| is_theme(t))
        .unwrap_or_else(|| DEFAULT_THEME.to_string())
}

/// The saved shortcut, or the default. (Whether it still parses is checked where it is registered.)
pub fn hotkey(db: &Db) -> String {
    db.get_setting(HOTKEY_KEY)
        .ok()
        .flatten()
        .filter(|h| !h.trim().is_empty() && h.len() <= 40)
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
}

pub fn load(db: &Db) -> Settings {
    Settings {
        theme: theme(db),
        hotkey: hotkey(db),
        hotkey_active: true,
        quiet_when_presenting: quiet_when_presenting(db),
        rechime_minutes: rechime_minutes(db),
    }
}

/// On unless the user turned it off.
pub fn quiet_when_presenting(db: &Db) -> bool {
    db.get_setting(QUIET_KEY).ok().flatten().as_deref() != Some("0")
}

pub fn set_quiet_when_presenting(db: &Db, on: bool) -> Result<()> {
    db.set_setting(QUIET_KEY, if on { "1" } else { "0" })
}

/// Off unless the user picked an interval.
pub fn rechime_minutes(db: &Db) -> u32 {
    db.get_setting(RECHIME_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .filter(|m| RECHIME_CHOICES.contains(m))
        .unwrap_or(0)
}

pub fn set_rechime_minutes(db: &Db, minutes: u32) -> Result<()> {
    if !RECHIME_CHOICES.contains(&minutes) {
        return Err(AppError(format!("Can't chime every {minutes} min")));
    }
    db.set_setting(RECHIME_KEY, &minutes.to_string())
}

pub fn set_theme(db: &Db, id: &str) -> Result<Settings> {
    if !is_theme(id) {
        return Err(AppError(format!("Unknown theme: {id}")));
    }
    db.set_setting(THEME_KEY, id)?;
    Ok(load(db))
}

pub fn set_hotkey(db: &Db, spec: &str) -> Result<()> {
    db.set_setting(HOTKEY_KEY, spec)
}

/// The exe path whose tray icon we last asked Windows to show on the taskbar.
pub fn tray_promoted_for(db: &Db) -> Option<String> {
    db.get_setting(TRAY_PROMOTED_KEY).ok().flatten()
}

pub fn set_tray_promoted_for(db: &Db, exe: &str) -> Result<()> {
    db.set_setting(TRAY_PROMOTED_KEY, exe)
}

/// True once the user has turned "Start with Windows" off. Only the "no" is remembered: the default is on,
/// so a new PC or a reinstall (which removes the Windows entry) comes back on unless they said otherwise.
pub fn autostart_declined(db: &Db) -> bool {
    db.get_setting(AUTOSTART_OFF_KEY).ok().flatten().as_deref() == Some("1")
}

pub fn set_autostart_declined(db: &Db, declined: bool) -> Result<()> {
    db.set_setting(AUTOSTART_OFF_KEY, if declined { "1" } else { "0" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn defaults_to_the_flat_theme() {
        assert_eq!(load(&memory_db()).theme, DEFAULT_THEME);
    }

    #[test]
    fn saves_a_known_theme_and_rejects_unknown_ones() {
        let db = memory_db();
        assert_eq!(set_theme(&db, "punk").unwrap().theme, "punk");
        assert_eq!(theme(&db), "punk");
        assert_eq!(set_theme(&db, "nudge").unwrap().theme, "nudge");
        set_theme(&db, "punk").unwrap();
        assert!(set_theme(&db, "neon-purple").is_err());
        assert!(set_theme(&db, "").is_err());
        assert_eq!(theme(&db), "punk", "a rejected value must not overwrite the saved theme");
    }

    #[test]
    fn a_corrupt_stored_value_falls_back_to_the_default() {
        let db = memory_db();
        db.set_setting("theme", "../../etc").unwrap();
        assert_eq!(theme(&db), DEFAULT_THEME);
    }

    #[test]
    fn the_glass_theme_is_a_real_theme() {
        assert!(is_theme(GLASS_THEME));
        assert!(is_theme(DEFAULT_THEME));
    }

    #[test]
    fn the_hotkey_defaults_and_round_trips() {
        let db = memory_db();
        assert_eq!(hotkey(&db), DEFAULT_HOTKEY);
        set_hotkey(&db, "Ctrl+Shift+K").unwrap();
        assert_eq!(hotkey(&db), "Ctrl+Shift+K");
        db.set_setting("hotkey", "   ").unwrap();
        assert_eq!(hotkey(&db), DEFAULT_HOTKEY, "a blank stored value falls back to the default");
    }

    #[test]
    fn autostart_is_on_unless_the_user_declined() {
        let db = memory_db();
        assert!(!autostart_declined(&db), "a new install defaults to starting with Windows");
        set_autostart_declined(&db, true).unwrap();
        assert!(autostart_declined(&db));
        set_autostart_declined(&db, false).unwrap();
        assert!(!autostart_declined(&db), "turning it back on clears the refusal");
    }

    #[test]
    fn quiet_is_on_and_rechime_off_by_default_and_both_round_trip() {
        let db = memory_db();
        assert!(quiet_when_presenting(&db));
        assert_eq!(rechime_minutes(&db), 0);
        set_quiet_when_presenting(&db, false).unwrap();
        assert!(!quiet_when_presenting(&db));
        set_rechime_minutes(&db, 5).unwrap();
        assert_eq!(rechime_minutes(&db), 5);
        assert!(set_rechime_minutes(&db, 7).is_err(), "only the offered choices");
        db.set_setting("rechime_minutes", "abc").unwrap();
        assert_eq!(rechime_minutes(&db), 0, "junk falls back to off");
    }

    #[test]
    fn tray_promotion_is_remembered_per_exe() {
        let db = memory_db();
        assert_eq!(tray_promoted_for(&db), None);
        set_tray_promoted_for(&db, r"C:\a\nudge.exe").unwrap();
        assert_eq!(tray_promoted_for(&db).as_deref(), Some(r"C:\a\nudge.exe"));
    }
}

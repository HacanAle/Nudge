//! SQLite persistence and recurrence rules.
//!
//! Every timestamp is unix milliseconds (UTC). Recurrence is evaluated in *local* wall-clock time
//! so "every weekday at 09:00" stays at 09:00 across daylight-saving changes.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use chrono::{Datelike, Duration, Local, Months, NaiveDateTime, TimeZone, Weekday};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize, Serializer};

// ───────────────────────────── errors ─────────────────────────────

/// Error type that crosses the IPC boundary as a plain string.
#[derive(Debug)]
pub struct AppError(pub String);

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError(e.to_string())
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

// ───────────────────────────── model ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Normal => "normal",
            Priority::High => "high",
        }
    }

    fn from_db(s: &str) -> Self {
        match s {
            "low" => Priority::Low,
            "high" => Priority::High,
            _ => Priority::Normal,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
        }
    }
}

/// How a reminder repeats. Crosses IPC / the database as a short string:
/// `none`, `daily`, `weekdays`, `weekly`, `monthly` or `interval:<minutes>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(try_from = "String", into = "String")]
pub enum Repeat {
    #[default]
    None,
    Daily,
    Weekdays,
    Weekly,
    Monthly,
    Interval(u32),
}

const MAX_INTERVAL_MINUTES: u32 = 525_600; // one year

impl From<Repeat> for String {
    fn from(r: Repeat) -> String {
        match r {
            Repeat::None => "none".into(),
            Repeat::Daily => "daily".into(),
            Repeat::Weekdays => "weekdays".into(),
            Repeat::Weekly => "weekly".into(),
            Repeat::Monthly => "monthly".into(),
            Repeat::Interval(m) => format!("interval:{m}"),
        }
    }
}

impl TryFrom<String> for Repeat {
    type Error = String;

    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        match s.as_str() {
            "none" | "" => Ok(Repeat::None),
            "daily" => Ok(Repeat::Daily),
            "weekdays" => Ok(Repeat::Weekdays),
            "weekly" => Ok(Repeat::Weekly),
            "monthly" => Ok(Repeat::Monthly),
            other => other
                .strip_prefix("interval:")
                .and_then(|m| m.parse::<u32>().ok())
                .filter(|m| (1..=MAX_INTERVAL_MINUTES).contains(m))
                .map(Repeat::Interval)
                .ok_or_else(|| format!("invalid repeat rule: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Waiting for `due_at`.
    Pending,
    /// Fired; the popup is (or should be) on screen waiting for an answer.
    Active,
    Done,
    Dismissed,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Active => "active",
            Status::Done => "done",
            Status::Dismissed => "dismissed",
        }
    }

    fn from_db(s: &str) -> Self {
        match s {
            "active" => Status::Active,
            "done" => Status::Done,
            "dismissed" => Status::Dismissed,
            _ => Status::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: i64,
    pub title: String,
    pub note: String,
    pub priority: Priority,
    /// When the reminder will (next) fire. Moves forward when snoozed.
    pub due_at: i64,
    /// The scheduled occurrence. Recurrence is computed from this, so snoozing
    /// a daily 09:00 reminder never drags tomorrow's alert to 09:10.
    pub anchor_at: i64,
    pub repeat: Repeat,
    pub status: Status,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReminder {
    pub title: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub priority: Priority,
    pub due_at: i64,
    #[serde(default)]
    pub repeat: Repeat,
}

/// One reminder in a backup file: the same fields the app shows, all but the title and time optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupReminder {
    pub title: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub priority: Priority,
    pub due_at: i64,
    #[serde(default)]
    pub anchor_at: i64,
    #[serde(default)]
    pub repeat: Repeat,
    #[serde(default = "pending")]
    pub status: Status,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub completed_at: Option<i64>,
}

fn pending() -> Status {
    Status::Pending
}

impl From<Reminder> for BackupReminder {
    fn from(r: Reminder) -> Self {
        BackupReminder {
            title: r.title,
            note: r.note,
            priority: r.priority,
            due_at: r.due_at,
            anchor_at: r.anchor_at,
            repeat: r.repeat,
            status: r.status,
            created_at: r.created_at,
            completed_at: r.completed_at,
        }
    }
}

const MAX_IMPORT: usize = 20_000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 2000;

// ───────────────────────────── database ─────────────────────────────

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS reminders (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    title        TEXT    NOT NULL,
    note         TEXT    NOT NULL DEFAULT '',
    priority     TEXT    NOT NULL DEFAULT 'normal',
    due_at       INTEGER NOT NULL,
    anchor_at    INTEGER NOT NULL,
    repeat       TEXT    NOT NULL DEFAULT 'none',
    status       TEXT    NOT NULL DEFAULT 'pending',
    created_at   INTEGER NOT NULL,
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_reminders_status_due ON reminders (status, due_at);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

const COLUMNS: &str =
    "id, title, note, priority, due_at, anchor_at, repeat, status, created_at, completed_at";

const PRIORITY_ORDER: &str =
    "CASE priority WHEN 'high' THEN 2 WHEN 'normal' THEN 1 ELSE 0 END DESC, due_at ASC, id ASC";

fn map_row(row: &Row) -> rusqlite::Result<Reminder> {
    let priority: String = row.get("priority")?;
    let repeat: String = row.get("repeat")?;
    let status: String = row.get("status")?;
    Ok(Reminder {
        id: row.get("id")?,
        title: row.get("title")?,
        note: row.get("note")?,
        priority: Priority::from_db(&priority),
        due_at: row.get("due_at")?,
        anchor_at: row.get("anchor_at")?,
        repeat: Repeat::try_from(repeat).unwrap_or_default(),
        status: Status::from_db(&status),
        created_at: row.get("created_at")?,
        completed_at: row.get("completed_at")?,
    })
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        // An alert that was on screen when the app last exited never got an answer.
        // Re-arm it so it pops again instead of silently vanishing.
        conn.execute("UPDATE reminders SET status = 'pending' WHERE status = 'active'", [])?;
        Ok(Db { conn: Mutex::new(conn) })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Trimmed `(title, note)` of user input, or why it can't be saved. Shared by `add` and `update`.
    fn checked_text(input: &NewReminder) -> Result<(String, String)> {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("A reminder needs a title".into());
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!("Title is too long (max {MAX_TITLE_CHARS} characters)").into());
        }
        let note = input.note.trim().to_string();
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(format!("Note is too long (max {MAX_NOTE_CHARS} characters)").into());
        }
        if input.due_at <= 0 {
            return Err("Pick a valid date and time".into());
        }
        Ok((title, note))
    }

    pub fn add(&self, new: NewReminder, now: i64) -> Result<Reminder> {
        let (title, note) = Self::checked_text(&new)?;

        let conn = self.lock();
        conn.execute(
            "INSERT INTO reminders (title, note, priority, due_at, anchor_at, repeat, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5, 'pending', ?6)",
            params![title, note, new.priority.as_str(), new.due_at, String::from(new.repeat), now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM reminders WHERE id = ?1"),
            [id],
            map_row,
        )?)
    }

    /// Replace the text, time, priority and repeat rule of a reminder that is still waiting or ringing.
    ///
    /// The new time becomes the recurrence anchor, so "every day" continues from what you just set.
    /// A ringing reminder keeps ringing if its time is still due (a typo fix shouldn't re-pop the alert);
    /// moved into the future, it goes back to waiting and the popup closes.
    pub fn update(&self, id: i64, edit: NewReminder, now: i64) -> Result<Reminder> {
        let (title, note) = Self::checked_text(&edit)?;

        let conn = self.lock();
        let changed = conn.execute(
            "UPDATE reminders
             SET title = ?2, note = ?3, priority = ?4, due_at = ?5, anchor_at = ?5, repeat = ?6,
                 status = CASE WHEN status = 'active' AND ?5 <= ?7 THEN 'active' ELSE 'pending' END
             WHERE id = ?1 AND status IN ('pending', 'active')",
            params![id, title, note, edit.priority.as_str(), edit.due_at, String::from(edit.repeat), now],
        )?;
        if changed == 0 {
            return Err("That reminder is no longer scheduled".into());
        }
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM reminders WHERE id = ?1"),
            [id],
            map_row,
        )?)
    }

    /// Pending and currently-ringing reminders, soonest first.
    pub fn upcoming(&self) -> Result<Vec<Reminder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM reminders WHERE status IN ('pending', 'active') ORDER BY due_at ASC, id ASC"
        ))?;
        let rows = stmt.query_map([], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn history(&self, limit: u32) -> Result<Vec<Reminder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM reminders WHERE status IN ('done', 'dismissed')
             ORDER BY completed_at DESC, id DESC LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limit], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Reminders whose popup should be on screen, most urgent first.
    pub fn active(&self) -> Result<Vec<Reminder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM reminders WHERE status = 'active' ORDER BY {PRIORITY_ORDER}"
        ))?;
        let rows = stmt.query_map([], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Atomically flips every due `pending` reminder to `active` and returns them.
    /// Anything overdue (laptop asleep, app was closed) is caught here too.
    /// `high_only` (while presenting) lets only high-priority reminders through; the rest wait.
    pub fn take_due(&self, now: i64, high_only: bool) -> Result<Vec<Reminder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "UPDATE reminders SET status = 'active'
             WHERE status = 'pending' AND due_at <= ?1 AND (?2 = 0 OR priority = 'high')
             RETURNING {COLUMNS}"
        ))?;
        let rows = stmt
            .query_map(params![now, high_only], map_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Finish a reminder. One-off reminders move to history; repeating ones re-arm
    /// for their next occurrence after `now`.
    pub fn complete(&self, id: i64, outcome: Status, now: i64) -> Result<()> {
        debug_assert!(matches!(outcome, Status::Done | Status::Dismissed));
        let conn = self.lock();
        let (anchor, repeat): (i64, String) = conn
            .query_row(
                "SELECT anchor_at, repeat FROM reminders WHERE id = ?1 AND status IN ('pending', 'active')",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| AppError::from("That reminder is no longer active"))?;

        let repeat = Repeat::try_from(repeat).unwrap_or_default();
        if repeat == Repeat::None {
            conn.execute(
                "UPDATE reminders SET status = ?2, completed_at = ?3 WHERE id = ?1",
                params![id, outcome.as_str(), now],
            )?;
        } else {
            let next = next_occurrence(anchor, &repeat, now);
            conn.execute(
                "UPDATE reminders SET status = 'pending', due_at = ?2, anchor_at = ?2, completed_at = ?3 WHERE id = ?1",
                params![id, next, now],
            )?;
        }
        Ok(())
    }

    /// Hide an alert and bring it back at `until` (never sooner than a second from now).
    pub fn snooze(&self, id: i64, until: i64, now: i64) -> Result<()> {
        let until = until.max(now + 1_000);
        let changed = self.lock().execute(
            "UPDATE reminders SET status = 'pending', due_at = ?2 WHERE id = ?1 AND status IN ('pending', 'active')",
            params![id, until],
        )?;
        if changed == 0 {
            return Err("That reminder is no longer active".into());
        }
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.lock().execute("DELETE FROM reminders WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        self.lock()
            .execute("DELETE FROM reminders WHERE status IN ('done', 'dismissed')", [])?;
        Ok(())
    }

    /// Every reminder, scheduled and finished, oldest first (for a backup).
    pub fn all(&self) -> Result<Vec<Reminder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM reminders ORDER BY id ASC"))?;
        let rows = stmt.query_map([], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Add reminders from a backup. Ones already here (same title, note, schedule, repeat and creation time)
    /// and ones that aren't valid are skipped. Returns `(added, skipped)`.
    pub fn import(&self, items: Vec<BackupReminder>, now: i64) -> Result<(usize, usize)> {
        if items.len() > MAX_IMPORT {
            return Err(format!("That backup has more than {MAX_IMPORT} reminders").into());
        }
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let (mut added, mut skipped) = (0, 0);
        for item in items {
            let input = NewReminder {
                title: item.title,
                note: item.note,
                priority: item.priority,
                due_at: item.due_at,
                repeat: item.repeat,
            };
            let Ok((title, note)) = Self::checked_text(&input) else {
                skipped += 1;
                continue;
            };
            let anchor = if item.anchor_at > 0 { item.anchor_at } else { input.due_at };
            let created = if item.created_at > 0 { item.created_at } else { now };
            let repeat = String::from(input.repeat);
            let exists: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM reminders WHERE title = ?1 AND note = ?2 AND anchor_at = ?3
                                AND repeat = ?4 AND created_at = ?5)",
                params![title, note, anchor, repeat, created],
                |r| r.get(0),
            )?;
            if exists {
                skipped += 1;
                continue;
            }
            // Something that was ringing when it was backed up rings again; finished ones go to history.
            let (status, completed) = match item.status {
                Status::Done | Status::Dismissed => (item.status, item.completed_at.or(Some(now))),
                Status::Pending | Status::Active => (Status::Pending, None),
            };
            tx.execute(
                "INSERT INTO reminders (title, note, priority, due_at, anchor_at, repeat, status, created_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![title, note, input.priority.as_str(), input.due_at, anchor, repeat, status.as_str(), created, completed],
            )?;
            added += 1;
        }
        tx.commit()?;
        Ok((added, skipped))
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        match self
            .lock()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

// ───────────────────────────── recurrence ─────────────────────────────

/// First occurrence of `repeat` strictly after `max(now, anchor)`.
pub fn next_occurrence(anchor: i64, repeat: &Repeat, now: i64) -> i64 {
    let base = now.max(anchor);
    match repeat {
        Repeat::None => anchor,
        Repeat::Interval(minutes) => {
            let step = i64::from(*minutes) * 60_000;
            anchor + ((base - anchor) / step + 1) * step
        }
        _ => {
            let mut cur = anchor;
            // Bounded so a corrupt row can never spin forever (4000 days ≈ 11 years of misses).
            for _ in 0..4_000 {
                if cur > base {
                    return cur;
                }
                cur = step_calendar(cur, repeat);
            }
            cur
        }
    }
}

fn step_calendar(ms: i64, repeat: &Repeat) -> i64 {
    let Some(dt) = Local.timestamp_millis_opt(ms).single() else {
        return ms + 86_400_000;
    };
    let naive = dt.naive_local();
    let next = match repeat {
        Repeat::Weekly => naive + Duration::days(7),
        Repeat::Weekdays => {
            let mut n = naive + Duration::days(1);
            while matches!(n.weekday(), Weekday::Sat | Weekday::Sun) {
                n += Duration::days(1);
            }
            n
        }
        Repeat::Monthly => naive
            .checked_add_months(Months::new(1))
            .unwrap_or(naive + Duration::days(30)),
        _ => naive + Duration::days(1),
    };
    local_to_ms(next)
}

/// Local wall-clock → unix ms. Times that don't exist (spring-forward gap) slide an hour later.
fn local_to_ms(n: NaiveDateTime) -> i64 {
    Local
        .from_local_datetime(&n)
        .earliest()
        .or_else(|| Local.from_local_datetime(&(n + Duration::hours(1))).earliest())
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| n.and_utc().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        local_to_ms(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap())
    }

    fn new(title: &str, due_at: i64, repeat: Repeat) -> NewReminder {
        NewReminder {
            title: title.into(),
            note: String::new(),
            priority: Priority::Normal,
            due_at,
            repeat,
        }
    }

    #[test]
    fn daily_rolls_to_tomorrow_even_when_answered_late() {
        let anchor = at(2026, 3, 10, 9, 0);
        let now = at(2026, 3, 10, 9, 30);
        assert_eq!(next_occurrence(anchor, &Repeat::Daily, now), at(2026, 3, 11, 9, 0));
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        // 2026-03-13 is a Friday.
        let anchor = at(2026, 3, 13, 9, 0);
        let now = at(2026, 3, 13, 9, 1);
        assert_eq!(next_occurrence(anchor, &Repeat::Weekdays, now), at(2026, 3, 16, 9, 0));
    }

    #[test]
    fn missed_days_are_skipped_not_replayed() {
        let anchor = at(2026, 3, 1, 9, 0);
        let now = at(2026, 3, 10, 12, 0);
        assert_eq!(next_occurrence(anchor, &Repeat::Daily, now), at(2026, 3, 11, 9, 0));
    }

    #[test]
    fn interval_stays_on_its_grid() {
        let anchor = at(2026, 3, 10, 9, 0);
        let now = at(2026, 3, 10, 9, 50);
        // Grid is 09:00, 09:45, 10:30 … — 09:50 has already passed 09:45.
        assert_eq!(next_occurrence(anchor, &Repeat::Interval(45), now), at(2026, 3, 10, 10, 30));
    }

    #[test]
    fn monthly_adds_a_calendar_month() {
        let anchor = at(2026, 1, 15, 8, 0);
        assert_eq!(next_occurrence(anchor, &Repeat::Monthly, anchor), at(2026, 2, 15, 8, 0));
    }

    #[test]
    fn repeat_round_trips_through_strings() {
        for r in [Repeat::None, Repeat::Daily, Repeat::Weekdays, Repeat::Weekly, Repeat::Monthly, Repeat::Interval(45)] {
            assert_eq!(Repeat::try_from(String::from(r.clone())).unwrap(), r);
        }
        assert!(Repeat::try_from("interval:0".to_string()).is_err());
        assert!(Repeat::try_from("interval:abc".to_string()).is_err());
        assert!(Repeat::try_from("hourly".to_string()).is_err());
    }

    #[test]
    fn due_reminders_fire_once_and_overdue_ones_are_recovered() {
        let db = Db::open_in_memory().unwrap();
        db.add(new("past", 1_000, Repeat::None), 0).unwrap();
        db.add(new("future", 9_000_000, Repeat::None), 0).unwrap();

        let fired = db.take_due(5_000, false).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].title, "past");
        assert!(db.take_due(5_000, false).unwrap().is_empty(), "must not fire twice");
        assert_eq!(db.active().unwrap().len(), 1);
    }

    #[test]
    fn snooze_moves_due_but_not_the_recurrence_anchor() {
        let db = Db::open_in_memory().unwrap();
        let anchor = at(2026, 3, 10, 9, 0);
        let r = db.add(new("standup", anchor, Repeat::Daily), 0).unwrap();
        db.take_due(anchor, false).unwrap();

        db.snooze(r.id, anchor + 600_000, anchor).unwrap();
        let snoozed = db.upcoming().unwrap().remove(0);
        assert_eq!(snoozed.due_at, anchor + 600_000);
        assert_eq!(snoozed.anchor_at, anchor);

        // Answer the re-fired alert at 09:12 — tomorrow must still be 09:00, not 09:10.
        let now = anchor + 12 * 60_000;
        db.take_due(now, false).unwrap();
        db.complete(r.id, Status::Done, now).unwrap();
        let next = db.upcoming().unwrap().remove(0);
        assert_eq!(next.due_at, at(2026, 3, 11, 9, 0));
        assert_eq!(next.status, Status::Pending);
    }

    #[test]
    fn one_off_reminders_move_to_history() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add(new("once", 1_000, Repeat::None), 0).unwrap();
        db.take_due(2_000, false).unwrap();
        db.complete(r.id, Status::Done, 2_000).unwrap();
        assert!(db.upcoming().unwrap().is_empty());
        assert_eq!(db.history(10).unwrap().len(), 1);
        assert!(db.complete(r.id, Status::Done, 3_000).is_err(), "already finished");
    }

    #[test]
    fn editing_rewrites_every_field_and_moves_the_anchor() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add(new("call mom", 10_000_000, Repeat::None), 0).unwrap();

        let edit = NewReminder {
            title: "  call dad  ".into(),
            note: "about the trip".into(),
            priority: Priority::High,
            due_at: 20_000_000,
            repeat: Repeat::Weekly,
        };
        let saved = db.update(r.id, edit, 1_000).unwrap();

        assert_eq!(saved.id, r.id);
        assert_eq!(saved.title, "call dad");
        assert_eq!(saved.note, "about the trip");
        assert_eq!(saved.priority, Priority::High);
        assert_eq!(saved.repeat, Repeat::Weekly);
        assert_eq!((saved.due_at, saved.anchor_at), (20_000_000, 20_000_000));
        assert_eq!(saved.status, Status::Pending);
        assert_eq!(saved.created_at, 0, "editing is not re-creating");
        assert_eq!(db.upcoming().unwrap().len(), 1);
    }

    #[test]
    fn a_ringing_reminder_keeps_ringing_unless_moved_to_the_future() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add(new("stretch", 1_000, Repeat::None), 0).unwrap();
        db.take_due(2_000, false).unwrap();
        assert_eq!(db.active().unwrap().len(), 1);

        // Fixing a typo while it's on screen must not make it vanish and pop up again.
        db.update(r.id, new("stretch legs", 1_000, Repeat::None), 2_000).unwrap();
        let still = db.active().unwrap();
        assert_eq!(still.len(), 1);
        assert_eq!(still[0].title, "stretch legs");

        // Rescheduling it for later puts it back to waiting, and it fires again at the new time.
        let moved = db.update(r.id, new("stretch legs", 9_000, Repeat::None), 2_000).unwrap();
        assert_eq!(moved.status, Status::Pending);
        assert!(db.active().unwrap().is_empty());
        assert_eq!(db.take_due(9_000, false).unwrap().len(), 1);
    }

    #[test]
    fn editing_a_repeating_reminder_recurs_from_the_new_time() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add(new("standup", at(2026, 3, 10, 9, 0), Repeat::Daily), 0).unwrap();

        let new_time = at(2026, 3, 10, 10, 30);
        db.update(r.id, new("standup", new_time, Repeat::Daily), 0).unwrap();
        db.take_due(new_time, false).unwrap();
        db.complete(r.id, Status::Done, new_time).unwrap();

        assert_eq!(db.upcoming().unwrap()[0].due_at, at(2026, 3, 11, 10, 30));
    }

    #[test]
    fn finished_missing_and_invalid_edits_are_rejected() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add(new("once", 1_000, Repeat::None), 0).unwrap();

        assert!(db.update(r.id, new("   ", 1_000, Repeat::None), 0).is_err());
        assert!(db.update(r.id, new("ok", 0, Repeat::None), 0).is_err());
        assert!(db.update(r.id, new(&"x".repeat(201), 1_000, Repeat::None), 0).is_err());
        assert!(db.update(999, new("ghost", 1_000, Repeat::None), 0).is_err());
        assert_eq!(db.upcoming().unwrap()[0].title, "once", "a rejected edit changes nothing");

        db.take_due(2_000, false).unwrap();
        db.complete(r.id, Status::Done, 2_000).unwrap();
        assert!(db.update(r.id, new("too late", 5_000, Repeat::None), 3_000).is_err(), "history is read-only");
    }

    #[test]
    fn while_presenting_only_high_priority_fires() {
        let db = Db::open_in_memory().unwrap();
        let normal = db.add(new("normal", 1_000, Repeat::None), 0).unwrap();
        let mut urgent = new("urgent", 1_000, Repeat::None);
        urgent.priority = Priority::High;
        db.add(urgent, 0).unwrap();

        let fired = db.take_due(2_000, true).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].title, "urgent");
        // The normal one waited and arrives once the presentation ends.
        let later = db.take_due(3_000, false).unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].id, normal.id);
    }

    #[test]
    fn a_backup_round_trips_and_importing_twice_adds_nothing() {
        let db = Db::open_in_memory().unwrap();
        db.add(new("daily", 5_000, Repeat::Daily), 100).unwrap();
        let once = db.add(new("once", 1_000, Repeat::None), 100).unwrap();
        db.take_due(2_000, false).unwrap();
        db.complete(once.id, Status::Done, 2_000).unwrap();

        let backup: Vec<BackupReminder> = db.all().unwrap().into_iter().map(Into::into).collect();
        let json = serde_json::to_string(&backup).unwrap();

        let other = Db::open_in_memory().unwrap();
        let items: Vec<BackupReminder> = serde_json::from_str(&json).unwrap();
        assert_eq!(other.import(items.clone(), 9_000).unwrap(), (2, 0));
        assert_eq!(other.upcoming().unwrap()[0].repeat, Repeat::Daily);
        assert_eq!(other.history(10).unwrap()[0].completed_at, Some(2_000));
        assert_eq!(other.import(items, 9_000).unwrap(), (0, 2), "duplicates are skipped");
    }

    #[test]
    fn importing_skips_invalid_rows_and_rearms_ringing_ones() {
        let db = Db::open_in_memory().unwrap();
        let json = r#"[
            {"title": "  ", "dueAt": 1000},
            {"title": "no time", "dueAt": 0},
            {"title": "ringing", "dueAt": 1000, "status": "active", "priority": "high"},
            {"title": "minimal", "dueAt": 5000}
        ]"#;
        let items: Vec<BackupReminder> = serde_json::from_str(json).unwrap();
        assert_eq!(db.import(items, 50).unwrap(), (2, 2));
        let up = db.upcoming().unwrap();
        assert!(up.iter().all(|r| r.status == Status::Pending));
        assert_eq!(up.iter().find(|r| r.title == "minimal").unwrap().anchor_at, 5000);
    }

    #[test]
    fn settings_round_trip_and_overwrite() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), None);
        db.set_setting("theme", "punk").unwrap();
        assert_eq!(db.get_setting("theme").unwrap().as_deref(), Some("punk"));
        db.set_setting("theme", "slab").unwrap();
        assert_eq!(db.get_setting("theme").unwrap().as_deref(), Some("slab"));
    }

    #[test]
    fn rejects_blank_and_oversized_input() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.add(new("   ", 1_000, Repeat::None), 0).is_err());
        assert!(db.add(new(&"x".repeat(201), 1_000, Repeat::None), 0).is_err());
    }
}

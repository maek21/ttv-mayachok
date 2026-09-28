//! Маячки: хранение, планировщик, закреплённые на острове.

use crate::remind_parse::{Parsed, Repeat};
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: i64,
    pub text: String,
    /// RFC3339, UTC
    pub due_at: String,
    pub created_at: String,
    /// Как сказал голосом (пусто — создан вручную)
    pub phrase: String,
    pub app: String,
    pub urgent: bool,
    /// none | daily | weekdays | weekly | monthly
    pub repeat: String,
    /// За сколько минут предупредить; -1 — как в настройках
    pub lead_min: i64,
    pub toast: bool,
    pub done_at: Option<String>,
    pub fired: bool,
    pub pre_notified: bool,
}

impl Reminder {
    pub fn due(&self) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&self.due_at).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
    }
}

/// Что показывает остров: закреплённые, «Пора!», «Поставил»
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IslandReminders {
    pub pins: Vec<Reminder>,
    pub firing: Option<Reminder>,
    pub just_set: Option<Reminder>,
}

pub struct Reminders {
    conn: Mutex<Connection>,
}

const COLS: &str = "id, text, due_at, created_at, phrase, app, urgent, repeat, lead_min, toast, done_at, fired, pre_notified";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: r.get(0)?,
        text: r.get(1)?,
        due_at: r.get(2)?,
        created_at: r.get(3)?,
        phrase: r.get(4)?,
        app: r.get(5)?,
        urgent: r.get::<_, i64>(6)? != 0,
        repeat: r.get(7)?,
        lead_min: r.get(8)?,
        toast: r.get::<_, i64>(9)? != 0,
        done_at: r.get(10)?,
        fired: r.get::<_, i64>(11)? != 0,
        pre_notified: r.get::<_, i64>(12)? != 0,
    })
}

pub fn local_to_utc(n: NaiveDateTime) -> DateTime<Utc> {
    Local
        .from_local_datetime(&n)
        .earliest()
        .unwrap_or_else(|| Local.from_utc_datetime(&n))
        .with_timezone(&Utc)
}

impl Reminders {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let conn = Connection::open(dir.join("history.db")).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                due_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                phrase TEXT NOT NULL DEFAULT '',
                app TEXT NOT NULL DEFAULT '',
                urgent INTEGER NOT NULL DEFAULT 0,
                repeat TEXT NOT NULL DEFAULT 'none',
                lead_min INTEGER NOT NULL DEFAULT -1,
                toast INTEGER NOT NULL DEFAULT 1,
                done_at TEXT,
                fired INTEGER NOT NULL DEFAULT 0,
                pre_notified INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_rem_due ON reminders(due_at);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn create(&self, p: &Parsed, phrase: &str, app: &str) -> Result<Reminder, String> {
        let due = local_to_utc(p.due).to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO reminders (text, due_at, created_at, phrase, app, urgent, repeat) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![p.text, due, Utc::now().to_rfc3339(), phrase, app, p.urgent as i64, p.repeat.as_str()],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        conn.query_row(&format!("SELECT {COLS} FROM reminders WHERE id = ?1"), [id], row).map_err(|e| e.to_string())
    }

    pub fn get(&self, id: i64) -> Option<Reminder> {
        let conn = self.conn.lock();
        conn.query_row(&format!("SELECT {COLS} FROM reminders WHERE id = ?1"), [id], row).optional().ok().flatten()
    }

    /// Активные — по сроку; выполненные — последние сверху
    pub fn list(&self, done: bool) -> Result<Vec<Reminder>, String> {
        let conn = self.conn.lock();
        let sql = if done {
            format!("SELECT {COLS} FROM reminders WHERE done_at IS NOT NULL ORDER BY done_at DESC LIMIT 300")
        } else {
            format!("SELECT {COLS} FROM reminders WHERE done_at IS NULL ORDER BY due_at ASC")
        };
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], row).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    /// Правка из интерфейса. Сменился срок — снова ждём срабатывания.
    pub fn update(&self, r: &Reminder) -> Result<Reminder, String> {
        let old = self.get(r.id).ok_or("Маячок не найден")?;
        let reset = old.due_at != r.due_at;
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE reminders SET text=?2, due_at=?3, urgent=?4, repeat=?5, lead_min=?6, toast=?7,
                 fired = CASE WHEN ?8 THEN 0 ELSE fired END, pre_notified = CASE WHEN ?8 THEN 0 ELSE pre_notified END
                 WHERE id=?1",
                params![r.id, r.text, r.due_at, r.urgent as i64, r.repeat, r.lead_min, r.toast as i64, reset],
            )
            .map_err(|e| e.to_string())?;
        }
        self.get(r.id).ok_or_else(|| "Маячок не найден".into())
    }

    /// «Готово». Повторяющийся переезжает на следующий раз.
    pub fn done(&self, id: i64) -> Result<(), String> {
        let r = self.get(id).ok_or("Маячок не найден")?;
        let rep = Repeat::parse(&r.repeat);
        let conn = self.conn.lock();
        let now_local = Local::now().naive_local();
        let due_local = r.due().with_timezone(&Local).naive_local();
        if let Some(next) = rep.next_after(due_local, now_local) {
            conn.execute(
                "UPDATE reminders SET due_at=?2, fired=0, pre_notified=0 WHERE id=?1",
                params![id, local_to_utc(next).to_rfc3339()],
            )
        } else {
            conn.execute("UPDATE reminders SET done_at=?2 WHERE id=?1", params![id, Utc::now().to_rfc3339()])
        }
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn undone(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("UPDATE reminders SET done_at=NULL WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn snooze(&self, id: i64, minutes: i64) -> Result<(), String> {
        let due = (Utc::now() + Duration::minutes(minutes)).to_rfc3339();
        let conn = self.conn.lock();
        conn.execute("UPDATE reminders SET due_at=?2, fired=0, pre_notified=1 WHERE id=?1", params![id, due])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Отложить до завтра на то же время (или на час по умолчанию)
    pub fn snooze_tomorrow(&self, id: i64, default_hour: u32) -> Result<(), String> {
        let r = self.get(id).ok_or("Маячок не найден")?;
        let local = r.due().with_timezone(&Local);
        let tomorrow = (Local::now() + Duration::days(1)).date_naive();
        let t = if r.due() > Utc::now() - Duration::days(1) { local.time() } else { chrono::NaiveTime::from_hms_opt(default_hour, 0, 0).unwrap() };
        let due = local_to_utc(tomorrow.and_time(t)).to_rfc3339();
        let conn = self.conn.lock();
        conn.execute("UPDATE reminders SET due_at=?2, fired=0, pre_notified=0 WHERE id=?1", params![id, due])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM reminders WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_fired(&self, id: i64) {
        let conn = self.conn.lock();
        let _ = conn.execute("UPDATE reminders SET fired=1, pre_notified=1 WHERE id=?1", [id]);
    }

    pub fn mark_pre(&self, id: i64) {
        let conn = self.conn.lock();
        let _ = conn.execute("UPDATE reminders SET pre_notified=1 WHERE id=?1", [id]);
    }
}

/// Какие маячки держать на острове: срочные, просроченные и те, до которых меньше `pin_before_min`.
pub fn pins(active: &[Reminder], now: DateTime<Utc>, pin_before_min: u32, urgent_pins: bool) -> Vec<Reminder> {
    let mut out: Vec<Reminder> = active
        .iter()
        .filter(|r| {
            let left = r.due() - now;
            r.fired || (urgent_pins && r.urgent) || (pin_before_min > 0 && left <= Duration::minutes(pin_before_min as i64))
        })
        .cloned()
        .collect();
    out.sort_by_key(|r| r.due());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rem(id: i64, mins: i64, urgent: bool, fired: bool) -> Reminder {
        Reminder {
            id,
            text: format!("r{id}"),
            due_at: (Utc::now() + Duration::minutes(mins)).to_rfc3339(),
            created_at: String::new(),
            phrase: String::new(),
            app: String::new(),
            urgent,
            repeat: "none".into(),
            lead_min: -1,
            toast: true,
            done_at: None,
            fired,
            pre_notified: false,
        }
    }

    #[test]
    fn pins_selection() {
        let list = vec![rem(1, 300, true, false), rem(2, 30, false, false), rem(3, 600, false, false), rem(4, -5, false, true)];
        let p = pins(&list, Utc::now(), 60, true);
        let ids: Vec<i64> = p.iter().map(|r| r.id).collect();
        // просроченный первым, потом ближайший, срочный — хоть и через 5 часов
        assert_eq!(ids, vec![4, 2, 1]);
        assert_eq!(pins(&list, Utc::now(), 0, false).len(), 1);
    }

    #[test]
    fn store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mayachok-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Reminders::open(&dir).unwrap();
        let now = Local::now().naive_local();
        let p = Parsed { text: "Тест".into(), due: now + Duration::hours(1), urgent: true, repeat: Repeat::Daily };
        let r = s.create(&p, "маячок тест", "Telegram").unwrap();
        assert!(r.urgent);
        assert_eq!(s.list(false).unwrap().len(), 1);
        // повторяющийся после «Готово» переезжает на завтра, а не закрывается
        s.done(r.id).unwrap();
        let r2 = s.get(r.id).unwrap();
        assert!(r2.done_at.is_none());
        assert!(r2.due() > r.due() + Duration::hours(23));
        s.delete(r.id).unwrap();
        assert!(s.list(false).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

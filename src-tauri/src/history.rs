use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub created_at: String,
    pub text: String,
    pub raw_text: String,
    pub app: String,
    pub duration_ms: i64,
    pub words: i64,
    pub language: String,
    pub model: String,
    pub engine: String,
    pub process_ms: i64,
    pub has_audio: bool,
    pub favorite: bool,
}

pub struct NewEntry {
    pub text: String,
    pub raw_text: String,
    pub app: String,
    pub duration_ms: i64,
    pub language: String,
    pub model: String,
    pub engine: String,
    pub process_ms: i64,
    pub audio_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub label: String,
    pub words: i64,
    pub today: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub words_today: i64,
    pub words_avg: i64,
    pub duration_today_ms: i64,
    pub count_today: i64,
    pub saved_minutes_today: i64,
    pub streak: i64,
    pub best_streak: i64,
    pub week: Vec<DayStat>,
    pub week_total: i64,
    pub total_count: i64,
}

pub struct History {
    conn: Mutex<Connection>,
    pub audio_dir: PathBuf,
}

const COLS: &str = "id, created_at, text, raw_text, app, duration_ms, words, language, model, engine, process_ms, audio_path, favorite";

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    let audio: Option<String> = r.get(11)?;
    Ok(Entry {
        id: r.get(0)?,
        created_at: r.get(1)?,
        text: r.get(2)?,
        raw_text: r.get(3)?,
        app: r.get(4)?,
        duration_ms: r.get(5)?,
        words: r.get(6)?,
        language: r.get(7)?,
        model: r.get(8)?,
        engine: r.get(9)?,
        process_ms: r.get(10)?,
        has_audio: audio.map(|p| std::path::Path::new(&p).exists()).unwrap_or(false),
        favorite: r.get::<_, i64>(12)? != 0,
    })
}

pub fn count_words(text: &str) -> i64 {
    text.split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .count() as i64
}

impl History {
    pub fn open(dir: &PathBuf) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let audio_dir = dir.join("audio");
        fs::create_dir_all(&audio_dir).map_err(|e| e.to_string())?;
        let conn = Connection::open(dir.join("history.db")).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS dictations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                text TEXT NOT NULL,
                raw_text TEXT NOT NULL DEFAULT '',
                app TEXT NOT NULL DEFAULT '',
                duration_ms INTEGER NOT NULL DEFAULT 0,
                words INTEGER NOT NULL DEFAULT 0,
                language TEXT NOT NULL DEFAULT '',
                model TEXT NOT NULL DEFAULT '',
                engine TEXT NOT NULL DEFAULT '',
                process_ms INTEGER NOT NULL DEFAULT 0,
                audio_path TEXT,
                favorite INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_created ON dictations(created_at);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn: Mutex::new(conn), audio_dir })
    }

    pub fn insert(&self, e: NewEntry) -> Result<Entry, String> {
        let now = Utc::now().to_rfc3339();
        let words = count_words(&e.text);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO dictations (created_at, text, raw_text, app, duration_ms, words, language, model, engine, process_ms, audio_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![now, e.text, e.raw_text, e.app, e.duration_ms, words, e.language, e.model, e.engine, e.process_ms, e.audio_path],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        conn.query_row(&format!("SELECT {COLS} FROM dictations WHERE id = ?1"), [id], row_to_entry)
            .map_err(|e| e.to_string())
    }

    pub fn list(&self, query: &str, favorites: bool, app: Option<&str>, limit: i64, offset: i64) -> Result<Vec<Entry>, String> {
        let conn = self.conn.lock();
        let mut sql = format!("SELECT {COLS} FROM dictations WHERE 1=1");
        let mut args: Vec<String> = vec![];
        if favorites {
            sql.push_str(" AND favorite = 1");
        }
        if let Some(a) = app {
            args.push(a.to_string());
            sql.push_str(&format!(" AND app = ?{}", args.len()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), row_to_entry)
            .map_err(|e| e.to_string())?;
        // SQLite lower()/LIKE регистронезависимы только для ASCII, поэтому кириллицу фильтруем здесь
        let q = query.trim().to_lowercase();
        let mut out = vec![];
        let mut skipped = 0;
        for e in rows {
            let e = e.map_err(|e| e.to_string())?;
            if !q.is_empty() && !e.text.to_lowercase().contains(&q) {
                continue;
            }
            if skipped < offset {
                skipped += 1;
                continue;
            }
            out.push(e);
            if out.len() as i64 >= limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn apps(&self) -> Result<Vec<String>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT app, COUNT(*) c FROM dictations WHERE app != '' GROUP BY app ORDER BY c DESC")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn last(&self) -> Option<Entry> {
        let conn = self.conn.lock();
        conn.query_row(&format!("SELECT {COLS} FROM dictations ORDER BY created_at DESC LIMIT 1"), [], row_to_entry)
            .optional()
            .ok()
            .flatten()
    }

    pub fn audio_path(&self, id: i64) -> Option<String> {
        let conn = self.conn.lock();
        conn.query_row("SELECT audio_path FROM dictations WHERE id = ?1", [id], |r| r.get::<_, Option<String>>(0))
            .optional()
            .ok()
            .flatten()
            .flatten()
    }

    pub fn toggle_favorite(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock();
        conn.execute("UPDATE dictations SET favorite = 1 - favorite WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        conn.query_row("SELECT favorite FROM dictations WHERE id = ?1", [id], |r| r.get::<_, i64>(0))
            .map(|v| v != 0)
            .map_err(|e| e.to_string())
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        if let Some(p) = self.audio_path(id) {
            let _ = fs::remove_file(p);
        }
        let conn = self.conn.lock();
        conn.execute("DELETE FROM dictations WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        {
            let conn = self.conn.lock();
            conn.execute("DELETE FROM dictations", []).map_err(|e| e.to_string())?;
        }
        if let Ok(rd) = fs::read_dir(&self.audio_dir) {
            for f in rd.flatten() {
                let _ = fs::remove_file(f.path());
            }
        }
        Ok(())
    }

    /// Удаляет записи старше `days` дней (0 — хранить всегда).
    pub fn apply_retention(&self, days: u32) {
        if days == 0 {
            return;
        }
        let cutoff = (Utc::now() - Duration::days(days as i64)).to_rfc3339();
        let paths: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt = match conn.prepare("SELECT audio_path FROM dictations WHERE created_at < ?1 AND audio_path IS NOT NULL") {
                Ok(s) => s,
                Err(_) => return,
            };
            let rows = stmt.query_map([&cutoff], |r| r.get::<_, String>(0));
            match rows {
                Ok(r) => r.flatten().collect(),
                Err(_) => vec![],
            }
        };
        for p in paths {
            let _ = fs::remove_file(p);
        }
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM dictations WHERE created_at < ?1 AND favorite = 0", [&cutoff]);
    }

    pub fn drop_audio(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute("UPDATE dictations SET audio_path = NULL", []);
        if let Ok(rd) = fs::read_dir(&self.audio_dir) {
            for f in rd.flatten() {
                let _ = fs::remove_file(f.path());
            }
        }
    }

    pub fn all(&self) -> Result<Vec<Entry>, String> {
        self.list("", false, None, i64::MAX, 0)
    }

    pub fn stats(&self) -> Result<Stats, String> {
        let conn = self.conn.lock();
        // Агрегаты по локальным дням
        let mut stmt = conn
            .prepare("SELECT created_at, words, duration_ms FROM dictations")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))
            .map_err(|e| e.to_string())?;
        let mut per_day: std::collections::BTreeMap<NaiveDate, (i64, i64, i64)> = Default::default();
        let mut total_count = 0;
        for row in rows.flatten() {
            let Ok(dt) = DateTime::parse_from_rfc3339(&row.0) else { continue };
            let day = dt.with_timezone(&Local).date_naive();
            let e = per_day.entry(day).or_default();
            e.0 += row.1;
            e.1 += row.2;
            e.2 += 1;
            total_count += 1;
        }
        let today = Local::now().date_naive();
        let (words_today, duration_today_ms, count_today) = per_day.get(&today).copied().unwrap_or_default();

        let past: Vec<i64> = per_day.iter().filter(|(d, _)| **d != today).map(|(_, v)| v.0).collect();
        let words_avg = if past.is_empty() { 0 } else { past.iter().sum::<i64>() / past.len() as i64 };

        // Серия: подряд идущие дни с диктовкой, заканчивая сегодня или вчера
        let mut streak = 0;
        let mut d = if per_day.contains_key(&today) { today } else { today - Duration::days(1) };
        while per_day.contains_key(&d) {
            streak += 1;
            d -= Duration::days(1);
        }
        let mut best_streak = 0;
        let mut run = 0;
        let mut prev: Option<NaiveDate> = None;
        for day in per_day.keys() {
            run = match prev {
                Some(p) if *day - p == Duration::days(1) => run + 1,
                _ => 1,
            };
            best_streak = best_streak.max(run);
            prev = Some(*day);
        }

        const LABELS: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
        use chrono::Datelike;
        let mut week = vec![];
        for i in (0..7).rev() {
            let day = today - Duration::days(i);
            week.push(DayStat {
                label: LABELS[day.weekday().num_days_from_monday() as usize].to_string(),
                words: per_day.get(&day).map(|v| v.0).unwrap_or(0),
                today: i == 0,
            });
        }
        let week_total = week.iter().map(|d| d.words).sum();
        // Экономия: сколько заняло бы набрать на клавиатуре со скоростью 40 слов/мин минус время диктовки
        let typing_ms = words_today * 60_000 / 40;
        let saved_minutes_today = ((typing_ms - duration_today_ms).max(0)) / 60_000;

        Ok(Stats {
            words_today,
            words_avg,
            duration_today_ms,
            count_today,
            saved_minutes_today,
            streak,
            best_streak,
            week,
            week_total,
            total_count,
        })
    }
}

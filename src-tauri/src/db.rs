use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::Utc;

use crate::models::*;

pub struct Db {
    pub conn: Mutex<Connection>,
}

impl Db {
    pub fn open() -> Result<Self, String> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                quadrant INTEGER NOT NULL,
                priority REAL NOT NULL DEFAULT 50,
                importance_score REAL NOT NULL DEFAULT 2.5,
                urgency_score REAL NOT NULL DEFAULT 2.5,
                done INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                completed_at INTEGER
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    // ---- Settings ----

    pub fn get_api_key(&self) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = 'api_key'")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query(params![])
            .map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(row.get(0).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    pub fn set_api_key(&self, key: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('api_key', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![key]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(row.get(0).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---- Backup / Restore ----

    pub fn export_all(&self) -> Result<String, String> {
        let tasks = self.get_tasks()?;
        let backup = BackupData {
            version: 1,
            exported_at: Utc::now().timestamp_millis(),
            tasks,
        };
        serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())
    }

    pub fn import_all(&self, json: &str) -> Result<usize, String> {
        let backup: BackupData = serde_json::from_str(json).map_err(|e| format!("JSON 解析失败: {}", e))?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM tasks", params![]).map_err(|e| e.to_string())?;
        let count = backup.tasks.len();
        for t in &backup.tasks {
            conn.execute(
                "INSERT INTO tasks (id, title, description, quadrant, priority,
                 importance_score, urgency_score, done, created_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    t.id, t.title, t.description, t.quadrant, t.priority,
                    t.importance_score, t.urgency_score, t.done as i64,
                    t.created_at, t.completed_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(count)
    }

    // ---- Tasks ----

    pub fn get_tasks(&self) -> Result<Vec<Task>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, description, quadrant, priority,
                        importance_score, urgency_score, done,
                        created_at, completed_at
                 FROM tasks
                 ORDER BY done ASC, priority DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    quadrant: row.get(3)?,
                    priority: row.get(4)?,
                    importance_score: row.get(5)?,
                    urgency_score: row.get(6)?,
                    done: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                    completed_at: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.map_err(|e| e.to_string())?);
        }
        Ok(tasks)
    }

    pub fn create_task(&self, input: &TaskInput) -> Result<Task, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO tasks
                (title, description, quadrant, priority, importance_score, urgency_score, done, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                input.title,
                input.description,
                input.quadrant,
                input.priority,
                input.importance_score,
                input.urgency_score,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        Ok(Task {
            id,
            title: input.title.clone(),
            description: input.description.clone(),
            quadrant: input.quadrant,
            priority: input.priority,
            importance_score: input.importance_score,
            urgency_score: input.urgency_score,
            done: false,
            created_at: now,
            completed_at: None,
        })
    }

    pub fn toggle_done(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let done: i64 = conn
            .query_row("SELECT done FROM tasks WHERE id = ?1", params![id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let new_done = 1 - done;
        let completed_at = if new_done == 1 {
            Some(Utc::now().timestamp_millis())
        } else {
            None
        };
        conn.execute(
            "UPDATE tasks SET done = ?1, completed_at = ?2 WHERE id = ?3",
            params![new_done, completed_at, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_task(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn move_task(&self, id: i64, quadrant: i64, priority: f64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tasks SET quadrant = ?1, priority = ?2 WHERE id = ?3",
            params![quadrant, priority, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_done(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM tasks WHERE done = 1", params![])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---- Stats ----

    pub fn get_stats(&self) -> Result<StatsSummary, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks", params![], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let done: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE done = 1", params![], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let pending = total - done;
        let rate = if total > 0 { done as f64 / total as f64 } else { 0.0 };

        let mut by_quadrant = std::collections::HashMap::new();
        for q in 1..=4i64 {
            let qtotal: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE quadrant = ?1",
                    params![q],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let qdone: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE quadrant = ?1 AND done = 1",
                    params![q],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            by_quadrant.insert(q, QuadrantStat { total: qtotal, done: qdone });
        }

        // Last 7 days completion counts
        let mut recent = Vec::new();
        for i in (0..7).rev() {
            let day = Utc::now() - chrono::Duration::days(i);
            let start = day.date_naive().and_hms_opt(0, 0, 0).unwrap();
            let start_ms = start.and_utc().timestamp_millis();
            let end_ms = start_ms + 86_400_000;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE done = 1 AND completed_at >= ?1 AND completed_at < ?2",
                    params![start_ms, end_ms],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            recent.push(DailyCount {
                date: day.format("%m-%d").to_string(),
                count,
            });
        }

        Ok(StatsSummary {
            total,
            done,
            pending,
            completion_rate: rate,
            by_quadrant,
            recent_completed: recent,
        })
    }
}

fn db_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir()
        .ok_or("cannot determine data dir")?
        .join("eisenhower-matrix");
    Ok(base.join("tasks.db"))
}

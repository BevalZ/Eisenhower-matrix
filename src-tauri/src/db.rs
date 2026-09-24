use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::{Local, TimeZone, Utc};

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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_title TEXT NOT NULL DEFAULT '',
                ai_importance REAL NOT NULL,
                ai_urgency REAL NOT NULL,
                ai_quadrant INTEGER NOT NULL,
                user_importance REAL NOT NULL,
                user_urgency REAL NOT NULL,
                user_quadrant INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

        ensure_sync_columns(&conn)?;
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
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tasks = backup_tasks(&conn, false)?;
        let backup = BackupData {
            version: 2,
            exported_at: Utc::now().timestamp_millis(),
            tasks,
        };
        serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())
    }

    pub fn import_all(&self, json: &str) -> Result<usize, String> {
        let backup: BackupData = serde_json::from_str(json).map_err(|e| format!("JSON 解析失败: {}", e))?;
        if backup.version != 1 && backup.version != 2 {
            return Err(format!("不支持的备份版本: {}", backup.version));
        }
        let mut seen_ids = std::collections::HashSet::new();
        let mut seen_uids = std::collections::HashSet::new();
        for task in &backup.tasks {
            if !seen_ids.insert(task.id) {
                return Err(format!("备份中存在重复任务 id: {}", task.id));
            }
            if !task.uid.is_empty() && !seen_uids.insert(task.uid.clone()) {
                return Err(format!("备份中存在重复任务 uid: {}", task.uid));
            }
            validate_quadrant(task.quadrant)?;
            validate_priority(task.priority)?;
            validate_unit_score("重要性", task.importance_score)?;
            validate_unit_score("紧急性", task.urgency_score)?;
        }
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let device = device_id_locked(&tx)?;
        let now = Utc::now().timestamp_millis();
        tx.execute("DELETE FROM tasks", params![]).map_err(|e| e.to_string())?;
        for t in &backup.tasks {
            let uid = if t.uid.trim().is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                t.uid.trim().to_string()
            };
            let updated_at = if t.updated_at > 0 { t.updated_at } else { now };
            let updated_by = if t.updated_by.trim().is_empty() { device.clone() } else { t.updated_by.clone() };
            tx.execute(
                "INSERT INTO tasks (id, title, description, quadrant, priority,
                 importance_score, urgency_score, done, created_at, completed_at,
                 uid, updated_at, updated_by, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    t.id, t.title, t.description, t.quadrant, t.priority,
                    t.importance_score, t.urgency_score, t.done as i64,
                    t.created_at, t.completed_at, uid, updated_at, updated_by, t.deleted_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        let count = backup.tasks.len();
        tx.commit().map_err(|e| e.to_string())?;
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
                 WHERE deleted_at IS NULL
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
        let title = input.title.trim();
        if title.is_empty() {
            return Err("任务标题不能为空".into());
        }
        validate_quadrant(input.quadrant)?;
        validate_priority(input.priority)?;
        validate_unit_score("重要性", input.importance_score)?;
        validate_unit_score("紧急性", input.urgency_score)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let uid = uuid::Uuid::new_v4().to_string();
        let device = device_id_locked(&conn)?;
        conn.execute(
            "INSERT INTO tasks
                (title, description, quadrant, priority, importance_score, urgency_score, done, created_at,
                 uid, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9, ?10)",
            params![
                title,
                input.description,
                input.quadrant,
                input.priority,
                input.importance_score,
                input.urgency_score,
                now,
                uid,
                now,
                device
            ],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        Ok(Task {
            id,
            title: title.to_string(),
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
            .query_row(
                "SELECT done FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let new_done = 1 - done;
        let now = Utc::now().timestamp_millis();
        let completed_at = if new_done == 1 { Some(now) } else { None };
        let device = device_id_locked(&conn)?;
        conn.execute(
            "UPDATE tasks
             SET done = ?1, completed_at = ?2,
                 updated_at = max(updated_at + 1, ?3), updated_by = ?4
             WHERE id = ?5 AND deleted_at IS NULL",
            params![new_done, completed_at, now, device, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_task(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&conn)?;
        conn.execute(
            "UPDATE tasks
             SET deleted_at = max(updated_at + 1, ?1),
                 updated_at = max(updated_at + 1, ?1),
                 updated_by = ?2
             WHERE id = ?3 AND deleted_at IS NULL",
            params![now, device, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn move_task(&self, id: i64, quadrant: i64, priority: f64) -> Result<(), String> {
        validate_quadrant(quadrant)?;
        validate_priority(priority)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&conn)?;
        conn.execute(
            "UPDATE tasks
             SET quadrant = ?1, priority = ?2,
                 updated_at = max(updated_at + 1, ?3), updated_by = ?4
             WHERE id = ?5 AND deleted_at IS NULL",
            params![quadrant, priority, now, device, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_done(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&conn)?;
        conn.execute(
            "UPDATE tasks
             SET deleted_at = max(updated_at + 1, ?1),
                 updated_at = max(updated_at + 1, ?1),
                 updated_by = ?2
             WHERE done = 1 AND deleted_at IS NULL",
            params![now, device],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ---- Stats ----

    pub fn get_stats(&self) -> Result<StatsSummary, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL", params![], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let done: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE done = 1 AND deleted_at IS NULL", params![], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let pending = total - done;
        let rate = if total > 0 { done as f64 / total as f64 } else { 0.0 };

        let mut by_quadrant = std::collections::HashMap::new();
        for q in 1..=4i64 {
            let qtotal: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE quadrant = ?1 AND deleted_at IS NULL",
                    params![q],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let qdone: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE quadrant = ?1 AND done = 1 AND deleted_at IS NULL",
                    params![q],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            by_quadrant.insert(q, QuadrantStat { total: qtotal, done: qdone });
        }

        // Last 7 local days. A calendar day is not always 86_400_000 ms.
        let mut recent = Vec::new();
        let today = Local::now().date_naive();
        for i in (0..7).rev() {
            let date = today - chrono::Duration::days(i);
            let start_ms = local_midnight_ms(date)?;
            let next = date.succ_opt().ok_or("日期溢出")?;
            let end_ms = local_midnight_ms(next)?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE done = 1 AND deleted_at IS NULL AND completed_at >= ?1 AND completed_at < ?2",
                    params![start_ms, end_ms],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            recent.push(DailyCount {
                date: date.format("%m-%d").to_string(),
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

    // ---- AI Learning / Feedback ----

    pub fn record_feedback(
        &self,
        task_title: &str,
        ai_importance: f64,
        ai_urgency: f64,
        ai_quadrant: i64,
        user_importance: f64,
        user_urgency: f64,
        user_quadrant: i64,
    ) -> Result<(), String> {
        validate_quadrant(ai_quadrant)?;
        validate_quadrant(user_quadrant)?;
        validate_unit_score("AI 重要性", ai_importance)?;
        validate_unit_score("AI 紧急性", ai_urgency)?;
        validate_unit_score("用户重要性", user_importance)?;
        validate_unit_score("用户紧急性", user_urgency)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO feedback
                (task_title, ai_importance, ai_urgency, ai_quadrant,
                 user_importance, user_urgency, user_quadrant, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                task_title, ai_importance, ai_urgency, ai_quadrant,
                user_importance, user_urgency, user_quadrant,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get calibration bias from accumulated feedback.
    /// Returns (importance_bias, urgency_bias) — how much to shift future AI scores.
    pub fn get_calibration_bias(&self) -> Result<(f64, f64), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        calibration_bias(&conn)
    }

    pub fn get_learning_stats(&self) -> Result<LearningStats, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM feedback", params![], |r| r.get(0))
            .map_err(|e| e.to_string())?;

        let (imp_bias, urg_bias) = calibration_bias(&conn)?;

        let correct: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM feedback WHERE ai_quadrant = user_quadrant",
                params![],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let accuracy = if total > 0 {
            correct as f64 / total as f64
        } else {
            1.0
        };

        let mut stmt = conn
            .prepare(
                "SELECT id, task_title, ai_importance, ai_urgency, ai_quadrant,
                        user_importance, user_urgency, user_quadrant, created_at
                 FROM feedback ORDER BY created_at DESC LIMIT 10",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![], |row| {
                Ok(FeedbackRecord {
                    id: row.get(0)?,
                    task_title: row.get(1)?,
                    ai_importance: row.get(2)?,
                    ai_urgency: row.get(3)?,
                    ai_quadrant: row.get(4)?,
                    user_importance: row.get(5)?,
                    user_urgency: row.get(6)?,
                    user_quadrant: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut recent = Vec::new();
        for row in rows {
            recent.push(row.map_err(|e| e.to_string())?);
        }

        Ok(LearningStats {
            total_corrections: total,
            importance_bias: imp_bias,
            urgency_bias: urg_bias,
            accuracy_rate: accuracy,
            recent_corrections: recent,
        })
    }

    pub fn device_id(&self) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        device_id_locked(&conn)
    }

    pub fn sync_port(&self) -> Result<u16, String> {
        match self.get_setting("sync_port")? {
            Some(value) => parse_sync_port(&value),
            None => Ok(47321),
        }
    }

    pub fn save_sync_config(&self, secret: &str, port: u16) -> Result<(), String> {
        let secret = secret.trim();
        if secret.len() < 8 || secret.len() > 128 {
            return Err("同步密钥需要 8 到 128 位".into());
        }
        if !secret.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
            return Err("同步密钥只能使用英文字母、数字和符号".into());
        }
        if !(1024..=65535).contains(&port) {
            return Err("端口需要在 1024 到 65535 之间".into());
        }
        self.set_setting("sync_secret", secret)?;
        self.set_setting("sync_port", &port.to_string())
    }

    pub fn require_sync_secret(&self) -> Result<String, String> {
        let secret = self.get_setting("sync_secret")?.unwrap_or_default();
        if secret.len() < 8 || secret.len() > 128 {
            return Err("请先在两端设置相同的同步密钥（8-128 位）".into());
        }
        Ok(secret)
    }

    pub fn sync_snapshot(&self) -> Result<Vec<SyncTask>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        sync_tasks(&conn)
    }

    pub fn merge_remote(&self, incoming: &[SyncTask]) -> Result<usize, String> {
        if incoming.len() > 5000 {
            return Err("一次同步的任务不能超过 5000 条".into());
        }
        for task in incoming {
            validate_sync_task(task)?;
        }
        let incoming = newest_by_uid(incoming);
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut applied = 0usize;
        for remote in incoming {
            let local = tx
                .query_row(
                    "SELECT uid, title, description, quadrant, priority,
                            importance_score, urgency_score, done, created_at, completed_at,
                            updated_at, updated_by, deleted_at
                     FROM tasks WHERE uid = ?1",
                    params![remote.uid],
                    read_sync_task,
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if local.as_ref().is_some_and(|item| !remote_wins(item, remote)) {
                continue;
            }
            if local.is_none() {
                tx.execute(
                    "INSERT INTO tasks
                        (title, description, quadrant, priority, importance_score, urgency_score,
                         done, created_at, completed_at, uid, updated_at, updated_by, deleted_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        remote.title, remote.description, remote.quadrant, remote.priority,
                        remote.importance_score, remote.urgency_score, remote.done as i64,
                        remote.created_at, remote.completed_at, remote.uid, remote.updated_at,
                        remote.updated_by, remote.deleted_at
                    ],
                )
                .map_err(|e| e.to_string())?;
            } else {
                tx.execute(
                    "UPDATE tasks
                     SET title = ?1, description = ?2, quadrant = ?3, priority = ?4,
                         importance_score = ?5, urgency_score = ?6, done = ?7,
                         created_at = ?8, completed_at = ?9, updated_at = ?10,
                         updated_by = ?11, deleted_at = ?12
                     WHERE uid = ?13",
                    params![
                        remote.title, remote.description, remote.quadrant, remote.priority,
                        remote.importance_score, remote.urgency_score, remote.done as i64,
                        remote.created_at, remote.completed_at, remote.updated_at,
                        remote.updated_by, remote.deleted_at, remote.uid
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
            applied += 1;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(applied)
    }
}

fn ensure_sync_columns(conn: &Connection) -> Result<(), String> {
    for (column, ddl) in [
        ("uid", "uid TEXT"),
        ("updated_at", "updated_at INTEGER NOT NULL DEFAULT 0"),
        ("updated_by", "updated_by TEXT NOT NULL DEFAULT ''"),
        ("deleted_at", "deleted_at INTEGER"),
    ] {
        if !has_column(conn, column)? {
            conn.execute(&format!("ALTER TABLE tasks ADD COLUMN {ddl}"), [])
                .map_err(|e| e.to_string())?;
        }
    }
    let missing: Vec<(i64, i64)> = {
        let mut stmt = conn
            .prepare("SELECT id, created_at FROM tasks WHERE uid IS NULL OR uid = ''")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    for (id, created_at) in missing {
        conn.execute(
            "UPDATE tasks SET uid = ?1, updated_at = CASE WHEN updated_at = 0 THEN ?2 ELSE updated_at END WHERE id = ?3",
            params![uuid::Uuid::new_v4().to_string(), created_at, id],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_uid ON tasks(uid)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn has_column(conn: &Connection, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(tasks)")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    for name in rows {
        if name.map_err(|e| e.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn device_id_locked(conn: &Connection) -> Result<String, String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'device_id'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('device_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

fn parse_sync_port(value: &str) -> Result<u16, String> {
    let port: u16 = value.parse().map_err(|_| "同步端口无效".to_string())?;
    if !(1024..=65535).contains(&port) {
        return Err("端口需要在 1024 到 65535 之间".into());
    }
    Ok(port)
}

fn backup_tasks(conn: &Connection, include_deleted: bool) -> Result<Vec<BackupTask>, String> {
    let sql = if include_deleted {
        "SELECT id, title, description, quadrant, priority, importance_score, urgency_score,
                done, created_at, completed_at, uid, updated_at, updated_by, deleted_at
         FROM tasks"
    } else {
        "SELECT id, title, description, quadrant, priority, importance_score, urgency_score,
                done, created_at, completed_at, uid, updated_at, updated_by, deleted_at
         FROM tasks WHERE deleted_at IS NULL"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BackupTask {
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
                uid: row.get(10)?,
                updated_at: row.get(11)?,
                updated_by: row.get(12)?,
                deleted_at: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn sync_tasks(conn: &Connection) -> Result<Vec<SyncTask>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT uid, title, description, quadrant, priority, importance_score, urgency_score,
                    done, created_at, completed_at, updated_at, updated_by, deleted_at
             FROM tasks",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], read_sync_task).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn read_sync_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<SyncTask> {
    Ok(SyncTask {
        uid: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        quadrant: row.get(3)?,
        priority: row.get(4)?,
        importance_score: row.get(5)?,
        urgency_score: row.get(6)?,
        done: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        completed_at: row.get(9)?,
        updated_at: row.get(10)?,
        updated_by: row.get(11)?,
        deleted_at: row.get(12)?,
    })
}

fn newest_by_uid(incoming: &[SyncTask]) -> Vec<&SyncTask> {
    let mut chosen: Vec<&SyncTask> = Vec::new();
    for task in incoming {
        if let Some(existing) = chosen.iter_mut().find(|item| item.uid == task.uid) {
            if remote_wins(existing, task) {
                *existing = task;
            }
        } else {
            chosen.push(task);
        }
    }
    chosen
}

fn remote_wins(local: &SyncTask, remote: &SyncTask) -> bool {
    match remote.updated_at.cmp(&local.updated_at) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => remote.updated_by > local.updated_by,
    }
}

fn validate_sync_task(task: &SyncTask) -> Result<(), String> {
    let uid = task.uid.trim();
    if uid.len() < 8 || uid.len() > 80 || uid.chars().any(char::is_whitespace) {
        return Err("同步任务 uid 无效".into());
    }
    if task.title.trim().is_empty() {
        return Err("同步任务标题不能为空".into());
    }
    if task.title.chars().count() > 2000 || task.description.chars().count() > 20_000 {
        return Err("同步任务内容过长".into());
    }
    if task.updated_by.chars().count() > 80 {
        return Err("同步设备标识过长".into());
    }
    validate_quadrant(task.quadrant)?;
    validate_priority(task.priority)?;
    validate_unit_score("重要性", task.importance_score)?;
    validate_unit_score("紧急性", task.urgency_score)?;
    if task.created_at < 0 || task.updated_at < 0 {
        return Err("同步时间无效".into());
    }
    if task.completed_at.is_some_and(|value| value < 0) || task.deleted_at.is_some_and(|value| value < 0) {
        return Err("同步时间无效".into());
    }
    Ok(())
}


fn calibration_bias(conn: &Connection) -> Result<(f64, f64), String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM feedback", params![], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok((0.0, 0.0));
    }
    let avg: (f64, f64) = conn
        .query_row(
            "SELECT AVG(user_importance - ai_importance),
                    AVG(user_urgency - ai_urgency)
             FROM feedback",
            params![],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    // Soften bias: with few samples, trust AI more; with many, trust user more.
    let weight = (count as f64 / (count as f64 + 5.0)).min(0.6);
    Ok((avg.0 * weight, avg.1 * weight))
}

fn validate_quadrant(quadrant: i64) -> Result<(), String> {
    if (1..=4).contains(&quadrant) {
        Ok(())
    } else {
        Err("象限必须在 1 到 4 之间".into())
    }
}

fn validate_priority(priority: f64) -> Result<(), String> {
    if priority.is_finite() && (0.0..=100.0).contains(&priority) {
        Ok(())
    } else {
        Err("优先级必须在 0 到 100 之间".into())
    }
}

fn validate_unit_score(label: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=4.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{label}必须在 0 到 4 之间"))
    }
}

fn local_midnight_ms(date: chrono::NaiveDate) -> Result<i64, String> {
    let naive = date.and_hms_opt(0, 0, 0).ok_or("无效日期")?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .ok_or_else(|| "无法换算本地时间".into())
}

fn db_path() -> Result<PathBuf, String> {
    let base = dirs::data_dir()
        .ok_or("cannot determine data dir")?
        .join("eisenhower-matrix");
    Ok(base.join("tasks.db"))
}

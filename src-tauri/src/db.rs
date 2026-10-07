use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::{Local, TimeZone, Utc};

use crate::models::*;

pub struct Db {
    pub conn: Mutex<Connection>,
}

impl Db {
    /// `path` is resolved by the caller: the desktop keeps its historical `dirs` location so
    /// an existing install still finds its tasks, while mobile uses the app's private data
    /// directory (resolved through Tauri, which knows the Android package).
    pub fn open(path: PathBuf) -> Result<Self, String> {
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

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
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
        prune_tombstones(&conn)?;
        Ok(())
    }

    // ---- Settings ----

    // API key / WebDAV password: see secrets.rs (OS credential store).

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

    pub fn delete_setting(&self, key: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])
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
        self.import_backup(&backup)
    }

    /// Overwrites local tasks with a WebDAV file in either format (see `parse_webdav_payload`).
    /// Tombstones are kept so a later merge can't resurrect deleted tasks. Returns visible tasks.
    pub fn restore_webdav(&self, json: &str) -> Result<usize, String> {
        let backup = match serde_json::from_str::<SyncEnvelope>(json) {
            Ok(envelope) => BackupData {
                version: 2,
                exported_at: Utc::now().timestamp_millis(),
                tasks: envelope
                    .tasks
                    .into_iter()
                    .enumerate()
                    .map(|(i, t)| BackupTask {
                        id: i as i64 + 1,
                        title: t.title,
                        description: t.description,
                        quadrant: t.quadrant,
                        priority: t.priority,
                        importance_score: t.importance_score,
                        urgency_score: t.urgency_score,
                        done: t.done,
                        created_at: t.created_at,
                        completed_at: t.completed_at,
                        uid: t.uid,
                        updated_at: t.updated_at,
                        updated_by: t.updated_by,
                        deleted_at: t.deleted_at,
                        due_at: t.due_at,
                    })
                    .collect(),
            },
            Err(_) => serde_json::from_str(json).map_err(|e| format!("WebDAV 文件解析失败: {}", e))?,
        };
        self.import_backup(&backup)?;
        Ok(backup.tasks.iter().filter(|t| t.deleted_at.is_none()).count())
    }

    fn import_backup(&self, backup: &BackupData) -> Result<usize, String> {
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
            validate_text(&task.title, &task.description)?;
            validate_quadrant(task.quadrant)?;
            validate_priority(task.priority)?;
            validate_due(task.due_at)?;
            validate_import_times(task)?;
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
                 uid, updated_at, updated_by, deleted_at, due_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    t.id, t.title, t.description, t.quadrant, t.priority,
                    t.importance_score, t.urgency_score, t.done as i64,
                    t.created_at, t.completed_at, uid, updated_at, updated_by, t.deleted_at,
                    t.due_at
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
                        created_at, completed_at, due_at
                 FROM tasks
                 WHERE deleted_at IS NULL
                 ORDER BY done ASC, priority DESC, id ASC",
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
                    due_at: row.get(10)?,
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
        validate_text(title, &input.description)?;
        validate_quadrant(input.quadrant)?;
        validate_priority(input.priority)?;
        validate_due(input.due_at)?;
        validate_unit_score("重要性", input.importance_score)?;
        validate_unit_score("紧急性", input.urgency_score)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let uid = uuid::Uuid::new_v4().to_string();
        let device = device_id_locked(&conn)?;
        conn.execute(
            "INSERT INTO tasks
                (title, description, quadrant, priority, importance_score, urgency_score, done, created_at,
                 uid, updated_at, updated_by, due_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9, ?10, ?11)",
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
                device,
                input.due_at
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
            due_at: input.due_at,
        })
    }

    pub fn update_task(&self, input: &TaskUpdate) -> Result<(), String> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err("任务标题不能为空".into());
        }
        validate_text(title, &input.description)?;
        validate_quadrant(input.quadrant)?;
        validate_priority(input.priority)?;
        validate_due(input.due_at)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&conn)?;
        let changed = conn
            .execute(
                "UPDATE tasks
                 SET title = ?1, description = ?2, quadrant = ?3, priority = ?4, due_at = ?5,
                     updated_at = max(updated_at + 1, ?6), updated_by = ?7
                 WHERE id = ?8 AND deleted_at IS NULL",
                params![title, input.description, input.quadrant, input.priority, input.due_at, now, device, input.id],

            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err("任务不存在或已被删除".into());
        }
        Ok(())
    }

    pub fn toggle_done(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let done: i64 = conn
            .query_row(
                "SELECT done FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or("任务不存在或已被删除")?;
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

    /// Apply a drag-and-drop reorder atomically: either every row moves or none does.
    pub fn reorder_tasks(&self, moves: &[TaskMove]) -> Result<(), String> {
        if moves.is_empty() {
            return Ok(());
        }
        if moves.len() > 5000 {
            return Err("一次移动的任务过多".into());
        }
        for m in moves {
            validate_quadrant(m.quadrant)?;
            validate_priority(m.priority)?;
        }
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&tx)?;
        for m in moves {
            let changed = tx
                .execute(
                    "UPDATE tasks
                     SET quadrant = ?1, priority = ?2,
                         updated_at = max(updated_at + 1, ?3), updated_by = ?4
                     WHERE id = ?5 AND deleted_at IS NULL",
                    params![m.quadrant, m.priority, now, device, m.id],
                )
                .map_err(|e| e.to_string())?;
            if changed == 0 {
                return Err("任务不存在或已被删除，请刷新后重试".into());
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Soft-delete all finished tasks; returns their ids so the UI can undo.
    pub fn clear_done(&self) -> Result<Vec<i64>, String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let ids: Vec<i64> = {
            let mut stmt = tx
                .prepare("SELECT id FROM tasks WHERE done = 1 AND deleted_at IS NULL")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
        };
        if ids.is_empty() {
            return Ok(ids);
        }
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&tx)?;
        tx.execute(
            "UPDATE tasks
             SET deleted_at = max(updated_at + 1, ?1),
                 updated_at = max(updated_at + 1, ?1),
                 updated_by = ?2
             WHERE done = 1 AND deleted_at IS NULL",
            params![now, device],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(ids)
    }

    /// Undo a delete. Clearing the tombstone with a newer timestamp also propagates to peers.
    pub fn restore_tasks(&self, ids: &[i64]) -> Result<usize, String> {
        if ids.len() > 5000 {
            return Err("一次恢复的任务过多".into());
        }
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let now = Utc::now().timestamp_millis();
        let device = device_id_locked(&tx)?;
        let mut restored = 0usize;
        for &id in ids {
            restored += tx
                .execute(
                    "UPDATE tasks
                     SET deleted_at = NULL,
                         updated_at = max(updated_at + 1, ?1),
                         updated_by = ?2
                     WHERE id = ?3 AND deleted_at IS NOT NULL",
                    params![now, device, id],
                )
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(restored)
    }

    // ---- Stats ----

    /// Finished tasks stay in the statistics after "清除已完成" (a tombstone keeps
    /// done/completed_at), so clearing the board does not wipe completion history.
    /// Deleted unfinished tasks are dropped: they were abandoned, not pending.
    pub fn get_stats(&self) -> Result<StatsSummary, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // One grouped pass covers the totals and every quadrant. A cleared task keeps its row
        // as a finished tombstone, so it still counts; an abandoned unfinished task does not.
        let mut by_quadrant = std::collections::HashMap::new();
        let (mut total, mut done) = (0i64, 0i64);
        {
            let mut stmt = conn
                .prepare(
                    "SELECT quadrant, COUNT(*), COALESCE(SUM(done), 0)
                     FROM tasks
                     WHERE deleted_at IS NULL OR done = 1
                     GROUP BY quadrant",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            for row in rows {
                let (q, qtotal, qdone) = row.map_err(|e| e.to_string())?;
                total += qtotal;
                done += qdone;
                by_quadrant.insert(q, QuadrantStat { total: qtotal, done: qdone });
            }
        }
        for q in 1..=4i64 {
            by_quadrant.entry(q).or_insert(QuadrantStat { total: 0, done: 0 });
        }
        let pending = total - done;
        let rate = if total > 0 { done as f64 / total as f64 } else { 0.0 };

        // Last 7 local days. A calendar day is not always 86_400_000 ms, so bucket by date
        // rather than by fixed offsets. The same rows also answer the weekly review.
        let today = Local::now().date_naive();
        let first = today - chrono::Duration::days(6);
        let week_start = local_midnight_ms(first)?;
        let mut per_day = [0i64; 7];
        let mut week_by_quadrant: std::collections::HashMap<i64, i64> =
            (1..=4i64).map(|q| (q, 0i64)).collect();
        {
            let mut stmt = conn
                .prepare(
                    "SELECT quadrant, completed_at
                     FROM tasks
                     WHERE done = 1 AND completed_at IS NOT NULL AND completed_at >= ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![week_start], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| e.to_string())?;
            for row in rows {
                let (q, completed_at) = row.map_err(|e| e.to_string())?;
                *week_by_quadrant.entry(q).or_insert(0) += 1;
                if let Some(dt) = Local.timestamp_millis_opt(completed_at).single() {
                    let offset = (dt.date_naive() - first).num_days();
                    if (0..7).contains(&offset) {
                        per_day[offset as usize] += 1;
                    }
                }
            }
        }
        let recent_completed = (0..7)
            .map(|i| {
                let date = first + chrono::Duration::days(i);
                DailyCount {
                    date: date.format("%m-%d").to_string(),
                    count: per_day[i as usize],
                }
            })
            .collect();

        let overdue: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks
                 WHERE done = 0 AND deleted_at IS NULL AND due_at IS NOT NULL AND due_at < ?1",
                params![Utc::now().timestamp_millis()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;

        Ok(StatsSummary {
            total,
            done,
            pending,
            completion_rate: rate,
            by_quadrant,
            recent_completed,
            week_by_quadrant,
            overdue,
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
        let (_, imp_bias, urg_bias) = feedback_calibration(&conn)?;
        Ok((imp_bias, urg_bias))
    }

    pub fn get_learning_stats(&self) -> Result<LearningStats, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // Count and both averages come from the same pass over the feedback table.
        let (total, imp_bias, urg_bias) = feedback_calibration(&conn)?;

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

    /// An empty `secret` keeps the saved one, so the settings page can save a new port (or
    /// start listening) without the user having to type the shared secret again. The secret
    /// itself is never sent to the UI unasked.
    pub fn save_sync_config(&self, secret: &str, port: u16) -> Result<(), String> {
        let secret = secret.trim();
        if !(1024..=65535).contains(&port) {
            return Err("端口需要在 1024 到 65535 之间".into());
        }
        if secret.is_empty() {
            self.require_sync_secret()?;
            return self.set_setting("sync_port", &port.to_string());
        }
        if secret.len() < 8 || secret.len() > 128 {
            return Err("同步密钥需要 8 到 128 位".into());
        }
        if !secret.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
            return Err("同步密钥只能使用英文字母、数字和符号".into());
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

    /// Whether a usable sync secret is saved, without handing the value out.
    pub fn sync_secret_configured(&self) -> Result<bool, String> {
        Ok(self.require_sync_secret().is_ok())
    }

    // ---- AI config that travels with sync ----
    //
    // 换设备时不必重新填 Key：AI 配置随同步载荷一起走（默认开启）。
    // 密钥是明文，会落在 WebDAV 文件里，因此可以在设置里关掉。

    pub fn sync_ai_key_enabled(&self) -> Result<bool, String> {
        Ok(self.get_setting("sync_ai_key")?.as_deref() != Some("0"))
    }

    pub fn set_sync_ai_key(&self, enabled: bool) -> Result<(), String> {
        self.set_setting("sync_ai_key", if enabled { "1" } else { "0" })
    }

    /// (provider, base_url, model, saved_at) —— 本机当前的 AI 配置。
    pub fn ai_config_snapshot(&self) -> Result<(String, String, String, i64), String> {
        let provider = self.get_setting("ai_provider")?.unwrap_or_else(|| "jevai".into());
        let base_url = self.get_setting("ai_base_url")?.unwrap_or_default();
        let model = self.get_setting("ai_model")?.unwrap_or_default();
        let saved_at = self
            .get_setting("ai_saved_at")?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        Ok((provider, base_url, model, saved_at))
    }

    /// 记下「AI 配置是什么时候在这台设备上保存的」，同步时用来比较新旧。
    pub fn mark_ai_saved(&self) -> Result<(), String> {
        self.set_setting("ai_saved_at", &Utc::now().timestamp_millis().to_string())
    }

    /// 用同步过来的 AI 配置覆盖本机：只有对端更新（`saved_at` 更大）才生效。
    /// 返回是否真的改了本机配置。
    pub fn adopt_remote_ai(&self, remote: &SyncAiSettings) -> Result<bool, String> {
        let (local_provider, local_base, local_model, local_saved) = self.ai_config_snapshot()?;
        let has_local = !local_provider.is_empty() && local_saved > 0;
        if has_local && remote.saved_at <= local_saved {
            return Ok(false);
        }
        if remote.provider.is_empty() && remote.api_key.is_none() {
            return Ok(false);
        }
        if !remote.provider.is_empty() {
            self.set_setting("ai_provider", remote.provider.trim())?;
        }
        if !remote.base_url.is_empty() || !remote.provider.is_empty() {
            self.set_setting("ai_base_url", remote.base_url.trim())?;
        }
        if !remote.model.is_empty() || !remote.provider.is_empty() {
            self.set_setting("ai_model", remote.model.trim())?;
        }
        if remote.saved_at > 0 {
            self.set_setting("ai_saved_at", &remote.saved_at.to_string())?;
        }
        let _ = (local_base, local_model);
        Ok(true)
    }

    pub fn sync_snapshot(&self) -> Result<Vec<SyncTask>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        sync_tasks(&conn)
    }

    /// `remote_schema` < 1 means the peer predates due dates and sends none, so an
    /// update from it must not clear the local `due_at`.
    ///
    /// The payload is merged in slices: a long history (thousands of tombstones) used to be
    /// rejected outright, which left two devices unable to sync at all, and one transaction
    /// over every row would hold the write lock for the whole merge.
    pub fn merge_remote(&self, incoming: &[SyncTask], remote_schema: u32) -> Result<usize, String> {
        /// Must stay above the row count of any realistic history; the real size guard is
        /// the sync payload limit in the HTTP layer.
        const MAX_INCOMING: usize = 200_000;
        /// Rows per transaction.
        const MERGE_CHUNK: usize = 2_000;

        let keep_local_due = remote_schema < 1;
        if incoming.len() > MAX_INCOMING {
            return Err(format!("一次同步的任务不能超过 {MAX_INCOMING} 条"));
        }
        for task in incoming {
            validate_sync_task(task)?;
        }
        let incoming = newest_by_uid(incoming);
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut applied = 0usize;
        for chunk in incoming.chunks(MERGE_CHUNK) {
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for &remote in chunk {
                let local = tx
                    .query_row(
                        "SELECT uid, title, description, quadrant, priority,
                                importance_score, urgency_score, done, created_at, completed_at,
                                updated_at, updated_by, deleted_at, due_at
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
                             done, created_at, completed_at, uid, updated_at, updated_by, deleted_at, due_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                        params![
                            remote.title, remote.description, remote.quadrant, remote.priority,
                            remote.importance_score, remote.urgency_score, remote.done as i64,
                            remote.created_at, remote.completed_at, remote.uid, remote.updated_at,
                            remote.updated_by, remote.deleted_at, remote.due_at
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                } else {
                    tx.execute(
                        "UPDATE tasks
                         SET title = ?1, description = ?2, quadrant = ?3, priority = ?4,
                             importance_score = ?5, urgency_score = ?6, done = ?7,
                             created_at = ?8, completed_at = ?9, updated_at = ?10,
                             updated_by = ?11, deleted_at = ?12,
                             due_at = CASE WHEN ?14 THEN due_at ELSE ?15 END
                         WHERE uid = ?13",
                        params![
                            remote.title, remote.description, remote.quadrant, remote.priority,
                            remote.importance_score, remote.urgency_score, remote.done as i64,
                            remote.created_at, remote.completed_at, remote.updated_at,
                            remote.updated_by, remote.deleted_at, remote.uid,
                            keep_local_due, remote.due_at
                        ],

                    )
                    .map_err(|e| e.to_string())?;
                }
                applied += 1;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        Ok(applied)
    }
}

fn ensure_sync_columns(conn: &Connection) -> Result<(), String> {
    for (column, ddl) in [
        ("uid", "uid TEXT"),
        ("updated_at", "updated_at INTEGER NOT NULL DEFAULT 0"),
        ("updated_by", "updated_by TEXT NOT NULL DEFAULT ''"),
        ("deleted_at", "deleted_at INTEGER"),
        ("due_at", "due_at INTEGER"),
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
    // Only a database without the unique index can hold duplicate uids (a hand-edited or
    // half-migrated file). Re-mint them up front: creating the index would otherwise fail
    // and `Db::open` would abort before the window ever appears.
    if !has_index(conn, "idx_tasks_uid")? {
        for id in duplicate_uid_rows(conn)? {
            conn.execute(
                "UPDATE tasks SET uid = ?1 WHERE id = ?2",
                params![uuid::Uuid::new_v4().to_string(), id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_uid ON tasks(uid)",
        [],
    )
    .map_err(|e| e.to_string())?;
    // Statistics scan by completion date and quadrant; keep those off a full table scan.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_done_completed ON tasks(done, completed_at)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_quadrant ON tasks(quadrant, done)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Finished tasks cleared from the board keep their row as a tombstone: the statistics count
/// them and peers learn about the deletion from it. An abandoned unfinished task is neither
/// (see `get_stats`), so its tombstone is only kept long enough that a device which has been
/// offline since the deletion cannot resurrect it by syncing its own copy back.
const TOMBSTONE_RETENTION_DAYS: i64 = 180;

fn prune_tombstones(conn: &Connection) -> Result<(), String> {
    let cutoff = Utc::now().timestamp_millis() - TOMBSTONE_RETENTION_DAYS * 24 * 60 * 60 * 1000;
    conn.execute(
        "DELETE FROM tasks WHERE done = 0 AND deleted_at IS NOT NULL AND deleted_at < ?1",
        params![cutoff],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn has_index(conn: &Connection, name: &str) -> Result<bool, String> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(found.is_some())
}

/// Ids of every row whose uid repeats an earlier one (newest `updated_at` wins, oldest is
/// re-minted). Empty unless the table actually holds duplicates.
fn duplicate_uid_rows(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, uid FROM tasks
             WHERE uid IS NOT NULL AND uid != ''
             ORDER BY updated_at DESC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let mut duplicates = Vec::new();
    for row in rows {
        let (id, uid) = row.map_err(|e| e.to_string())?;
        if !seen.insert(uid) {
            duplicates.push(id);
        }
    }
    Ok(duplicates)
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
                done, created_at, completed_at, uid, updated_at, updated_by, deleted_at, due_at
         FROM tasks"
    } else {
        "SELECT id, title, description, quadrant, priority, importance_score, urgency_score,
                done, created_at, completed_at, uid, updated_at, updated_by, deleted_at, due_at
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
                due_at: row.get(14)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn sync_tasks(conn: &Connection) -> Result<Vec<SyncTask>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT uid, title, description, quadrant, priority, importance_score, urgency_score,
                    done, created_at, completed_at, updated_at, updated_by, deleted_at, due_at
             FROM tasks",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], read_sync_task).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Reads a WebDAV file for merging: `None` (file missing) is an empty remote, a
/// `SyncEnvelope` is used as-is, and a legacy `BackupData` upload is merged at
/// schema 0 because older builds may not have written `due_at`. Legacy tasks
/// without a uid can't be matched to local ones and are skipped.
pub fn parse_webdav_payload(
    json: Option<&str>,
) -> Result<(Vec<SyncTask>, u32, Option<SyncAiSettings>), String> {
    let Some(json) = json else {
        return Ok((Vec::new(), SYNC_SCHEMA, None));
    };
    if let Ok(envelope) = serde_json::from_str::<SyncEnvelope>(json) {
        return Ok((envelope.tasks, envelope.schema, envelope.ai));
    }
    let backup: BackupData =
        serde_json::from_str(json).map_err(|e| format!("WebDAV 文件解析失败: {}", e))?;
    let tasks = backup
        .tasks
        .into_iter()
        .filter(|t| !t.uid.trim().is_empty())
        .map(|t| SyncTask {
            uid: t.uid.trim().to_string(),
            title: t.title,
            description: t.description,
            quadrant: t.quadrant,
            priority: t.priority,
            importance_score: t.importance_score,
            urgency_score: t.urgency_score,
            done: t.done,
            created_at: t.created_at,
            completed_at: t.completed_at,
            updated_at: if t.updated_at > 0 { t.updated_at } else { t.created_at },
            updated_by: t.updated_by,
            deleted_at: t.deleted_at,
            due_at: t.due_at,
        })
        .collect();
    Ok((tasks, 0, None))
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
        due_at: row.get(13)?,
    })
}

fn newest_by_uid(incoming: &[SyncTask]) -> Vec<&SyncTask> {
    // One payload can carry 5000 rows; scanning the winners per row would be O(n²) string
    // comparisons on every sync round, so index them by uid instead.
    let mut at_by_uid: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut chosen: Vec<&SyncTask> = Vec::new();
    for task in incoming {
        match at_by_uid.get(task.uid.as_str()) {
            Some(&at) => {
                if remote_wins(chosen[at], task) {
                    chosen[at] = task;
                }
            }
            None => {
                at_by_uid.insert(task.uid.as_str(), chosen.len());
                chosen.push(task);
            }
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
    if task.completed_at.is_some_and(|value| value < 0)
        || task.deleted_at.is_some_and(|value| value < 0)
        || task.due_at.is_some_and(|value| value < 0)
    {
        return Err("同步时间无效".into());
    }
    Ok(())
}


/// (corrections, importance bias, urgency bias) in one pass over the feedback table.
fn feedback_calibration(conn: &Connection) -> Result<(i64, f64, f64), String> {
    let (count, imp_avg, urg_avg): (i64, Option<f64>, Option<f64>) = conn
        .query_row(
            "SELECT COUNT(*),
                    AVG(user_importance - ai_importance),
                    AVG(user_urgency - ai_urgency)
             FROM feedback",
            params![],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Ok((0, 0.0, 0.0));
    }
    // Soften bias: with few samples, trust AI more; with many, trust user more.
    let weight = (count as f64 / (count as f64 + 5.0)).min(0.6);
    Ok((
        count,
        imp_avg.unwrap_or(0.0) * weight,
        urg_avg.unwrap_or(0.0) * weight,
    ))
}

/// Same limits as validate_sync_task, so a local task can never be rejected by a peer.
fn validate_text(title: &str, description: &str) -> Result<(), String> {
    if title.chars().count() > 2000 || description.chars().count() > 20_000 {
        return Err("任务标题或描述过长".into());
    }
    Ok(())
}

/// A backup is user-supplied input: reject timestamps the sync path would refuse, so a
/// corrupt file cannot install rows a peer can never merge.
fn validate_import_times(task: &BackupTask) -> Result<(), String> {
    let negative = task.created_at < 0
        || task.updated_at < 0
        || task.completed_at.is_some_and(|v| v < 0)
        || task.deleted_at.is_some_and(|v| v < 0);
    if negative {
        return Err("备份中的时间戳无效".into());
    }
    Ok(())
}

fn validate_due(due_at: Option<i64>) -> Result<(), String> {
    if due_at.is_some_and(|v| v < 0) {
        return Err("截止时间无效".into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str, quadrant: i64, priority: f64) -> TaskInput {
        TaskInput {
            title: title.into(),
            description: String::new(),
            quadrant,
            priority,
            importance_score: 3.0,
            urgency_score: 3.0,
            due_at: None,
        }
    }

    fn mv(id: i64, quadrant: i64, priority: f64) -> TaskMove {
        TaskMove { id, quadrant, priority }
    }

    fn find(db: &Db, id: i64) -> Task {
        db.get_tasks().unwrap().into_iter().find(|t| t.id == id).unwrap()
    }

    #[test]
    fn reorder_moves_all_rows_or_none() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_task(&input("a", 1, 80.0)).unwrap();
        let b = db.create_task(&input("b", 1, 60.0)).unwrap();

        db.reorder_tasks(&[mv(a.id, 2, 55.0), mv(b.id, 1, 90.0)]).unwrap();
        assert_eq!((find(&db, a.id).quadrant, find(&db, a.id).priority), (2, 55.0));
        assert_eq!((find(&db, b.id).quadrant, find(&db, b.id).priority), (1, 90.0));

        // An unknown id rolls back the whole batch.
        assert!(db.reorder_tasks(&[mv(a.id, 3, 10.0), mv(9999, 1, 50.0)]).is_err());
        assert_eq!(find(&db, a.id).quadrant, 2);

        assert!(db.reorder_tasks(&[mv(a.id, 5, 10.0)]).is_err());
        assert!(db.reorder_tasks(&[mv(a.id, 1, 100.5)]).is_err());
        assert!(db.reorder_tasks(&[mv(a.id, 1, f64::NAN)]).is_err());
        assert!(db.reorder_tasks(&[]).is_ok());
    }

    #[test]
    fn update_task_edits_fields_and_validates() {
        let db = Db::open_in_memory().unwrap();
        let t = db.create_task(&input("old", 1, 50.0)).unwrap();
        let edit = |title: &str, quadrant: i64| TaskUpdate {
            id: t.id,
            title: title.into(),
            description: "desc".into(),
            quadrant,
            priority: 42.0,
            due_at: Some(1_900_000_000_000),
        };
        db.update_task(&edit("  new  ", 3)).unwrap();
        let got = find(&db, t.id);
        assert_eq!((got.title.as_str(), got.quadrant, got.priority), ("new", 3, 42.0));
        assert_eq!(got.description, "desc");
        assert_eq!(got.due_at, Some(1_900_000_000_000));


        assert!(db.update_task(&edit("   ", 3)).is_err());
        assert!(db.update_task(&edit("x", 0)).is_err());
        assert!(db.update_task(&edit(&"长".repeat(2001), 3)).is_err());
        db.delete_task(t.id).unwrap();
        assert!(db.update_task(&edit("gone", 3)).is_err());
    }

    #[test]
    fn get_tasks_breaks_priority_ties_by_id() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_task(&input("a", 1, 50.0)).unwrap();
        let b = db.create_task(&input("b", 1, 50.0)).unwrap();
        let ids: Vec<i64> = db.get_tasks().unwrap().iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![a.id, b.id]);
    }

    #[test]
    fn clearing_done_keeps_history_and_can_be_undone() {
        let db = Db::open_in_memory().unwrap();
        let done = db.create_task(&input("done", 2, 50.0)).unwrap();
        let open = db.create_task(&input("open", 2, 40.0)).unwrap();
        db.toggle_done(done.id).unwrap();

        let cleared = db.clear_done().unwrap();
        assert_eq!(cleared, vec![done.id]);
        let visible: Vec<i64> = db.get_tasks().unwrap().iter().map(|t| t.id).collect();
        assert_eq!(visible, vec![open.id]);

        let stats = db.get_stats().unwrap();
        assert_eq!((stats.total, stats.done, stats.pending), (2, 1, 1));
        assert_eq!(stats.by_quadrant[&2].done, 1);
        assert_eq!(stats.recent_completed.last().unwrap().count, 1);

        assert_eq!(db.restore_tasks(&cleared).unwrap(), 1);
        assert!(find(&db, done.id).done);
        assert_eq!(db.restore_tasks(&cleared).unwrap(), 0);
        assert_eq!(db.clear_done().unwrap().len(), 1);
    }

    #[test]
    fn weekly_stats_count_quadrants_and_overdue() {
        let db = Db::open_in_memory().unwrap();
        let q2 = db.create_task(&input("plan", 2, 50.0)).unwrap();
        db.create_task(&input("fire", 1, 50.0)).unwrap();
        let mut late = input("late", 3, 50.0);
        late.due_at = Some(1_000);
        db.create_task(&late).unwrap();
        db.toggle_done(q2.id).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.week_by_quadrant[&2], 1);
        assert_eq!(stats.week_by_quadrant[&1], 0);
        assert_eq!(stats.overdue, 1);
    }

    #[test]
    fn deleted_unfinished_tasks_leave_the_stats() {

        let db = Db::open_in_memory().unwrap();
        let t = db.create_task(&input("drop me", 4, 20.0)).unwrap();
        db.delete_task(t.id).unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!((stats.total, stats.pending), (0, 0));

        db.restore_tasks(&[t.id]).unwrap();
        assert_eq!(db.get_stats().unwrap().pending, 1);
    }

    #[test]
    fn merge_accepts_a_history_larger_than_one_chunk() {
        let db = Db::open_in_memory().unwrap();
        // Split over several merge chunks: a long history must not be rejected any more.
        let incoming: Vec<SyncTask> = (0..4_500)
            .map(|i| SyncTask {
                uid: format!("uid-{i:08}"),
                title: format!("task {i}"),
                description: String::new(),
                quadrant: 1,
                priority: 50.0,
                importance_score: 3.0,
                urgency_score: 3.0,
                done: false,
                created_at: 1_700_000_000_000,
                completed_at: None,
                updated_at: 1_700_000_000_000,
                updated_by: "peer-device".into(),
                deleted_at: None,
                due_at: None,
            })
            .collect();
        assert_eq!(db.merge_remote(&incoming, SYNC_SCHEMA).unwrap(), incoming.len());
        assert_eq!(db.sync_snapshot().unwrap().len(), incoming.len());
    }

    #[test]
    fn old_unfinished_tombstones_are_pruned_but_finished_ones_stay() {
        let db = Db::open_in_memory().unwrap();
        let abandoned = db.create_task(&input("abandoned", 1, 50.0)).unwrap();
        let finished = db.create_task(&input("finished", 1, 40.0)).unwrap();
        db.delete_task(abandoned.id).unwrap();
        db.toggle_done(finished.id).unwrap();
        db.delete_task(finished.id).unwrap();

        let old = Utc::now().timestamp_millis() - 400 * 24 * 60 * 60 * 1000;
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET deleted_at = ?1", params![old]).unwrap();
            prune_tombstones(&conn).unwrap();
        }

        let uids: Vec<String> = db.sync_snapshot().unwrap().into_iter().map(|t| t.uid).collect();
        assert_eq!(uids.len(), 1, "the abandoned tombstone should be gone");
        assert!(db.get_stats().unwrap().done == 1, "completion history is kept");
    }

    #[test]
    fn saving_an_empty_secret_keeps_the_saved_one() {
        let db = Db::open_in_memory().unwrap();
        assert!(!db.sync_secret_configured().unwrap());
        assert!(db.save_sync_config("", 47321).is_err());

        db.save_sync_config("shared-secret", 47321).unwrap();
        assert!(db.sync_secret_configured().unwrap());
        // Saving the port from the settings page must not require re-typing the secret.
        db.save_sync_config("", 48000).unwrap();
        assert_eq!(db.sync_port().unwrap(), 48000);
        assert_eq!(db.require_sync_secret().unwrap(), "shared-secret");
    }

    #[test]
    fn remote_ai_config_wins_only_when_newer() {
        let db = Db::open_in_memory().unwrap();
        // 本机先没配置：对端的配置会被采纳
        assert!(db.adopt_remote_ai(&SyncAiSettings {
            provider: "openai".into(),
            base_url: "http://localhost:11434/v1".into(),
            model: "qwen2.5:7b".into(),
            api_key: None,
            saved_at: 1000,
        })
        .unwrap());
        let (provider, base_url, model, saved_at) = db.ai_config_snapshot().unwrap();
        assert_eq!((provider.as_str(), base_url.as_str(), model.as_str(), saved_at),
                   ("openai", "http://localhost:11434/v1", "qwen2.5:7b", 1000));

        // 对端更旧：不动本机
        assert!(!db.adopt_remote_ai(&SyncAiSettings {
            provider: "jevai".into(),
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            saved_at: 500,
        })
        .unwrap());
        assert_eq!(db.ai_config_snapshot().unwrap().0, "openai");

        // 对端更新：采纳
        assert!(db.adopt_remote_ai(&SyncAiSettings {
            provider: "jevai".into(),
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            saved_at: 2000,
        })
        .unwrap());
        assert_eq!(db.ai_config_snapshot().unwrap().0, "jevai");
    }

    #[test]
    fn ai_key_sync_can_be_turned_off() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.sync_ai_key_enabled().unwrap(), "默认开启");
        db.set_sync_ai_key(false).unwrap();
        assert!(!db.sync_ai_key_enabled().unwrap());
        db.set_sync_ai_key(true).unwrap();
        assert!(db.sync_ai_key_enabled().unwrap());
    }

    #[test]
    fn duplicate_uids_are_repaired_before_the_unique_index() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_task(&input("a", 1, 50.0)).unwrap();
        let b = db.create_task(&input("b", 1, 40.0)).unwrap();
        {
            // A database written by an older build (or edited by hand) may hold duplicates.
            let conn = db.conn.lock().unwrap();
            conn.execute("DROP INDEX idx_tasks_uid", []).unwrap();
            conn.execute(
                "UPDATE tasks SET uid = 'duplicated-uid' WHERE id IN (?1, ?2)",
                params![a.id, b.id],
            )
            .unwrap();
            assert_eq!(duplicate_uid_rows(&conn).unwrap().len(), 1);
        }
        // migrate() re-mints the loser instead of failing to create the unique index.
        db.migrate().unwrap();
        let uids: Vec<String> = db
            .sync_snapshot()
            .unwrap()
            .into_iter()
            .map(|t| t.uid)
            .collect();
        assert_eq!(uids.len(), 2);
        assert_ne!(uids[0], uids[1]);
    }

    #[test]
    fn failed_import_leaves_existing_tasks_untouched() {
        let db = Db::open_in_memory().unwrap();
        db.create_task(&input("keep", 1, 70.0)).unwrap();
        let good = db.export_all().unwrap();

        let mut bad: serde_json::Value = serde_json::from_str(&good).unwrap();
        bad["tasks"][0]["quadrant"] = serde_json::json!(7);
        assert!(db.import_all(&bad.to_string()).is_err());
        assert_eq!(db.get_tasks().unwrap().len(), 1);

        assert_eq!(db.import_all(&good).unwrap(), 1);
        assert_eq!(db.get_tasks().unwrap()[0].title, "keep");
    }

    #[test]
    fn sync_merge_keeps_the_newest_version() {
        let db = Db::open_in_memory().unwrap();
        db.create_task(&input("local", 1, 50.0)).unwrap();
        let mut remote = db.sync_snapshot().unwrap().remove(0);

        remote.title = "remote".into();
        remote.updated_at += 10;
        remote.updated_by = "peer-device".into();
        assert_eq!(db.merge_remote(std::slice::from_ref(&remote), SYNC_SCHEMA).unwrap(), 1);
        assert_eq!(db.get_tasks().unwrap()[0].title, "remote");

        let mut stale = remote.clone();
        stale.title = "stale".into();
        stale.updated_at -= 100;
        assert_eq!(db.merge_remote(&[stale], SYNC_SCHEMA).unwrap(), 0);
        assert_eq!(db.get_tasks().unwrap()[0].title, "remote");
    }

    #[test]
    fn old_peers_do_not_clear_due_dates() {
        let db = Db::open_in_memory().unwrap();
        let mut due = input("due", 1, 50.0);
        due.due_at = Some(1_900_000_000_000);
        db.create_task(&due).unwrap();

        // A pre-due-date peer edits the title; its payload has no due_at.
        let mut remote = db.sync_snapshot().unwrap().remove(0);
        remote.title = "edited on old peer".into();
        remote.due_at = None;
        remote.updated_at += 10;
        db.merge_remote(std::slice::from_ref(&remote), 0).unwrap();
        let t = db.get_tasks().unwrap().remove(0);
        assert_eq!((t.title.as_str(), t.due_at), ("edited on old peer", Some(1_900_000_000_000)));

        // A current peer clearing the date does clear it.
        remote.updated_at += 10;
        db.merge_remote(&[remote], SYNC_SCHEMA).unwrap();
        assert_eq!(db.get_tasks().unwrap()[0].due_at, None);
    }

    /// One WebDAV round as `sync_to_webdav` does it, with `file` standing in for the server.
    fn webdav_round(db: &Db, file: &mut Option<String>) -> usize {
        let (incoming, schema, _ai) = parse_webdav_payload(file.as_deref()).unwrap();
        let applied = db.merge_remote(&incoming, schema).unwrap();
        let envelope = SyncEnvelope {
            device_id: db.device_id().unwrap(),
            tasks: db.sync_snapshot().unwrap(),
            schema: SYNC_SCHEMA,
            ai: None,
        };
        *file = Some(serde_json::to_string(&envelope).unwrap());
        applied
    }

    fn titles(db: &Db) -> Vec<String> {
        let mut t: Vec<String> = db.get_tasks().unwrap().into_iter().map(|t| t.title).collect();
        t.sort();
        t
    }

    #[test]
    fn webdav_sync_merges_instead_of_overwriting() {
        let a = Db::open_in_memory().unwrap();
        let b = Db::open_in_memory().unwrap();
        let mut file = None;

        // Missing remote file is an empty remote, not an error.
        a.create_task(&input("from a", 1, 50.0)).unwrap();
        assert_eq!(webdav_round(&a, &mut file), 0);

        // B has its own task; neither side loses anything.
        b.create_task(&input("from b", 2, 50.0)).unwrap();
        assert_eq!(webdav_round(&b, &mut file), 1);
        assert_eq!(webdav_round(&a, &mut file), 1);
        assert_eq!(titles(&a), vec!["from a", "from b"]);
        assert_eq!(titles(&b), vec!["from a", "from b"]);
    }

    #[test]
    fn webdav_sync_keeps_newest_edit_and_tombstones() {
        let a = Db::open_in_memory().unwrap();
        let b = Db::open_in_memory().unwrap();
        let mut file = None;
        a.create_task(&input("shared", 1, 50.0)).unwrap();
        a.create_task(&input("doomed", 1, 40.0)).unwrap();
        webdav_round(&a, &mut file);
        webdav_round(&b, &mut file);

        // Both edit "shared"; B's edit is newer. A deletes "doomed".
        let mut snap = a.sync_snapshot().unwrap();
        snap.sort_by(|x, y| x.title.cmp(&y.title));
        let (mut doomed, mut shared) = (snap[0].clone(), snap[1].clone());
        shared.title = "old edit on a".into();
        shared.updated_at += 10;
        a.merge_remote(&[shared.clone()], SYNC_SCHEMA).unwrap();
        shared.title = "new edit on b".into();
        shared.updated_at += 10;
        b.merge_remote(&[shared], SYNC_SCHEMA).unwrap();
        doomed.deleted_at = Some(doomed.updated_at + 10);
        doomed.updated_at += 10;
        a.merge_remote(&[doomed], SYNC_SCHEMA).unwrap();

        webdav_round(&a, &mut file);
        webdav_round(&b, &mut file);
        webdav_round(&a, &mut file);
        assert_eq!(titles(&a), vec!["new edit on b"]);
        assert_eq!(titles(&b), vec!["new edit on b"]);
    }

    #[test]
    fn webdav_legacy_backup_merges_without_clearing_due_dates() {
        let db = Db::open_in_memory().unwrap();
        let mut due = input("due", 1, 50.0);
        due.due_at = Some(1_900_000_000_000);
        db.create_task(&due).unwrap();

        // An old build uploaded BackupData (no schema, no due_at) with a newer title.
        let mut legacy: serde_json::Value = serde_json::from_str(&db.export_all().unwrap()).unwrap();
        let task = &mut legacy["tasks"][0];
        task["title"] = "edited on old build".into();
        task["updated_at"] = (task["updated_at"].as_i64().unwrap() + 10).into();
        task.as_object_mut().unwrap().remove("due_at");
        let legacy = legacy.to_string();

        let (incoming, schema, _ai) = parse_webdav_payload(Some(&legacy)).unwrap();
        assert_eq!(schema, 0);
        db.merge_remote(&incoming, schema).unwrap();
        let t = db.get_tasks().unwrap().remove(0);
        assert_eq!((t.title.as_str(), t.due_at), ("edited on old build", Some(1_900_000_000_000)));

        // Restore still accepts the legacy format.
        assert_eq!(db.restore_webdav(&legacy).unwrap(), 1);
    }

    #[test]
    fn webdav_restore_keeps_tombstones() {
        let a = Db::open_in_memory().unwrap();
        let t = a.create_task(&input("gone", 1, 50.0)).unwrap();
        a.create_task(&input("kept", 1, 40.0)).unwrap();
        a.delete_task(t.id).unwrap();
        let mut file = None;
        webdav_round(&a, &mut file);

        let b = Db::open_in_memory().unwrap();
        assert_eq!(b.restore_webdav(file.as_deref().unwrap()).unwrap(), 1);
        assert_eq!(titles(&b), vec!["kept"]);
        assert_eq!(b.sync_snapshot().unwrap().len(), 2);
    }

}

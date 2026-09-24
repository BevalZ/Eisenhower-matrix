use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub quadrant: i64,
    pub priority: f64,
    pub importance_score: f64,
    pub urgency_score: f64,
    pub done: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TaskInput {
    pub title: String,
    pub description: String,
    pub quadrant: i64,
    pub priority: f64,
    pub importance_score: f64,
    pub urgency_score: f64,
}

#[derive(Debug, Serialize)]
pub struct ClassificationResult {
    pub quadrant: i64,
    pub priority: f64,
    pub importance_score: f64,
    pub urgency_score: f64,
    pub importance_label: String,
    pub urgency_label: String,
}

#[derive(Debug, Serialize)]
pub struct Settings {
    pub api_key_configured: bool,
    pub theme: String,
    pub webdav_configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct WebdavConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTask {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub quadrant: i64,
    pub priority: f64,
    pub importance_score: f64,
    pub urgency_score: f64,
    pub done: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub updated_by: String,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub version: u32,
    pub exported_at: i64,
    pub tasks: Vec<BackupTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTask {
    pub uid: String,
    pub title: String,
    pub description: String,
    pub quadrant: i64,
    pub priority: f64,
    pub importance_score: f64,
    pub urgency_score: f64,
    pub done: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub updated_at: i64,
    pub updated_by: String,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncEnvelope {
    pub device_id: String,
    pub tasks: Vec<SyncTask>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TailscalePeer {
    pub hostname: String,
    pub ip: String,
    pub online: bool,
    pub os: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TailscaleStatus {
    pub running: bool,
    pub hostname: String,
    pub ip: String,
    pub peers: Vec<TailscalePeer>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerSyncStatus {
    pub listening: bool,
    pub address: String,
    pub port: u16,
    pub secret: String,
    pub device_id: String,
    pub last_error: String,
}

#[derive(Debug, Deserialize)]
pub struct PeerSyncConfig {
    pub secret: String,
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub task_count: usize,
}

#[derive(Debug, Serialize)]
pub struct QuadrantStat {
    pub total: i64,
    pub done: i64,
}

#[derive(Debug, Serialize)]
pub struct StatsSummary {
    pub total: i64,
    pub done: i64,
    pub pending: i64,
    pub completion_rate: f64,
    pub by_quadrant: std::collections::HashMap<i64, QuadrantStat>,
    pub recent_completed: Vec<DailyCount>,
}

#[derive(Debug, Serialize)]
pub struct DailyCount {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub id: i64,
    pub task_title: String,
    pub ai_importance: f64,
    pub ai_urgency: f64,
    pub ai_quadrant: i64,
    pub user_importance: f64,
    pub user_urgency: f64,
    pub user_quadrant: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct LearningStats {
    pub total_corrections: i64,
    pub importance_bias: f64,
    pub urgency_bias: f64,
    pub accuracy_rate: f64,
    pub recent_corrections: Vec<FeedbackRecord>,
}

pub const IMPORTANCE_LABELS: [&str; 5] = [
    "无关紧要，没有实际影响",
    "影响较小，锦上添花",
    "中等影响，支撑部分目标",
    "重大影响，直接支撑关键目标",
    "至关重要，决定长期成功",
];

pub const URGENCY_LABELS: [&str; 5] = [
    "没有截止时间，随时可做",
    "宽松截止，可等待数周",
    "合理截止，近几天内",
    "紧迫截止，24-48 小时内",
    "必须立刻，已经逾期",
];

pub fn label_for(score: f64, labels: &[&str; 5]) -> String {
    let idx = score.round().clamp(0.0, 4.0) as usize;
    labels[idx].to_string()
}

/// Map importance (0-4) and urgency (0-4) scores to quadrant (1-4).
/// Threshold is 2.5.
pub fn scores_to_quadrant(importance: f64, urgency: f64) -> i64 {
    let important = importance >= 2.5;
    let urgent = urgency >= 2.5;
    match (important, urgent) {
        (true, true) => 1,
        (true, false) => 2,
        (false, true) => 3,
        (false, false) => 4,
    }
}

/// Compute combined priority 0-100 within a quadrant.
/// Importance weight 0.6, urgency weight 0.4.
pub fn compute_priority(importance: f64, urgency: f64) -> f64 {
    (importance / 4.0 * 0.6 + urgency / 4.0 * 0.4) * 100.0
}
